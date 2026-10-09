mod bvh;
mod geom;
mod gcode;
mod infill;
mod perimeter;
mod slicer;
mod stl;
mod threemf;
mod web;
mod web_ui;

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use rayon::prelude::*;

use bvh::TriangleIntervalIndex;
use gcode::{GCodeWriter, PrintConfig, ProcessedLayer};
use geom::Polygon2;
use infill::InfillGenerator;
use perimeter::PerimeterGenerator;
use slicer::{ContourRole, Slicer};
use stl::Mesh;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--web" || a == "--gui" || a == "-w") {
        let port: u16 = args
            .windows(2)
            .find(|w| w[0] == "--port" || w[0] == "-p")
            .and_then(|w| w[1].parse().ok())
            .unwrap_or(8080);
        if let Err(e) = web::start_web_server(port) {
            eprintln!("Web server error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let input_path = if args.len() > 1 && !args[1].starts_with("--") {
        args[1].clone()
    } else {
        println!("No STL input provided. Creating 'cube.stl' (20x20x20mm test cube)...");
        generate_test_cube_stl("cube.stl").expect("Failed to create test cube");
        "cube.stl".to_string()
    };

    let output_path = if args.len() > 2 && !args[2].starts_with("--") {
        args[2].clone()
    } else {
        let p = Path::new(&input_path);
        p.with_extension("gcode").to_str().unwrap().to_string()
    };

    let mut config = PrintConfig::default();
    let mut infill_density = 0.20; // 20%
    let mut perimeter_count = 2;

    // Simple arg parser
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--layer-height" if i + 1 < args.len() => {
                config.layer_height = args[i + 1].parse().unwrap_or(0.2);
                i += 1;
            }
            "--infill" if i + 1 < args.len() => {
                infill_density = args[i + 1].parse().unwrap_or(0.20);
                i += 1;
            }
            "--perimeters" if i + 1 < args.len() => {
                perimeter_count = args[i + 1].parse().unwrap_or(2);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    println!("============================================================");
    println!("           Tetanus-Slicer (High-Speed Slicing Engine)       ");
    println!("============================================================");
    println!("Input STL:       {}", input_path);
    println!("Output G-code:   {}", output_path);
    println!("Layer Height:    {:.3} mm", config.layer_height);
    println!("Perimeters:      {}", perimeter_count);
    println!("Infill Density:  {:.1}%", infill_density * 100.0);
    println!("Thread Pool:     {} threads (Rayon)", rayon::current_num_threads());
    println!("------------------------------------------------------------");

    // 1. Load STL
    let t_start = Instant::now();
    let mesh = Mesh::load_stl(&input_path).unwrap_or_else(|e| {
        eprintln!("Error loading STL: {}", e);
        std::process::exit(1);
    });
    let t_mesh = t_start.elapsed();
    println!(
        "[1/4] Loaded STL in {:?}: {} triangles | Bounds: ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2})",
        t_mesh,
        mesh.triangles.len(),
        mesh.min_bound.x, mesh.min_bound.y, mesh.min_bound.z,
        mesh.max_bound.x, mesh.max_bound.y, mesh.max_bound.z
    );

    // 2. Build Spatial Index (Z-Interval Tree)
    let t_bvh_start = Instant::now();
    let bvh = TriangleIntervalIndex::build(&mesh.triangles);
    let t_bvh = t_bvh_start.elapsed();
    println!("[2/4] Built spatial Z-index in {:?}", t_bvh);

    // 3. Compute Layer Heights & Slice in Parallel with Rayon
    let t_slice_start = Instant::now();
    let total_height = mesh.max_bound.z - mesh.min_bound.z;
    let layer_count = (total_height / config.layer_height).ceil() as usize;

    println!(
        "[3/4] Slicing {} layers in parallel across {} cores...",
        layer_count,
        rayon::current_num_threads()
    );

    let min_z = mesh.min_bound.z;
    let layers: Vec<ProcessedLayer> = (0..layer_count)
        .into_par_iter()
        .map(|layer_idx| {
            let z = min_z + (layer_idx as f64 + 0.5) * config.layer_height;

            // Query intersecting triangles
            let intersecting = bvh.query_z(z);

            // Intersect triangles with Z plane
            let mut segments = Vec::with_capacity(intersecting.len());
            for tri in intersecting {
                if let Some(seg) = Slicer::intersect_triangle(tri, z) {
                    segments.push(seg);
                }
            }

            // Chain segments into closed contours with hole classification
            let contours = Slicer::chain_segments(&segments);

            // Generate perimeters & inner boundaries
            let mut perimeters = Vec::new();
            let mut infill_boundaries = Vec::new();

            for contour in &contours {
                let perim_loops = PerimeterGenerator::generate_perimeters(
                    &contour.polygon,
                    &contour.role,
                    perimeter_count,
                    config.line_width,
                );
                if let Some(innermost) = perim_loops.last() {
                    infill_boundaries.push(innermost.clone());
                } else {
                    infill_boundaries.push(contour.polygon.clone());
                }
                perimeters.push(perim_loops);
            }

            // Skirt and Brim on layer 0
            let skirt_brim = if layer_idx == 0 {
                let mut sb = Vec::new();
                let outer_polys: Vec<Polygon2> = contours
                    .iter()
                    .filter(|c| c.role == ContourRole::Outer)
                    .map(|c| c.polygon.clone())
                    .collect();

                if config.brim_width > 0.0 {
                    sb.extend(PerimeterGenerator::generate_brim(
                        &outer_polys,
                        config.brim_width,
                        config.line_width,
                    ));
                }
                if config.skirt_loops > 0 {
                    sb.extend(PerimeterGenerator::generate_skirt(
                        &outer_polys,
                        config.skirt_loops,
                        config.skirt_distance,
                        config.line_width,
                    ));
                }
                sb
            } else {
                Vec::new()
            };

            // Top and Bottom Solid Shells (100% rectilinear density)
            let is_solid = layer_idx < config.bottom_solid_layers
                || layer_idx >= layer_count.saturating_sub(config.top_solid_layers);
            let layer_infill_density = if is_solid { 1.0 } else { infill_density };

            // Generate infill across all boundaries with automatic hole exclusion
            let infill = InfillGenerator::generate_rectilinear(
                &infill_boundaries,
                layer_infill_density,
                config.line_width,
                layer_idx,
            );

            ProcessedLayer {
                layer_index: layer_idx,
                z,
                skirt_brim,
                perimeters,
                infill,
            }
        })
        .collect();

    let t_slice = t_slice_start.elapsed();
    println!("[3/4] Parallel slicing complete in {:?}", t_slice);

    // 4. Output G-code
    let t_gcode_start = Instant::now();
    let out_file = File::create(&output_path).unwrap_or_else(|e| {
        eprintln!("Failed to create output file: {}", e);
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(out_file);
    let mut gcode_writer = GCodeWriter::new(&config);
    gcode_writer.write_gcode(&mut writer, &layers).unwrap();
    let t_gcode = t_gcode_start.elapsed();

    let total_time = t_start.elapsed();
    println!("[4/4] G-code written in {:?}", t_gcode);
    println!("------------------------------------------------------------");
    println!("✓ Slicing Finished Successfully!");
    println!("  Total Elapsed Time: {:?}", total_time);
    println!("  Throughput:         {:.1} layers/sec", layer_count as f64 / total_time.as_secs_f64());
    println!("  Generated:          {}", output_path);
    println!("============================================================");
}

/// Helper function to create an ASCII STL 20x20x20mm cube for testing
fn generate_test_cube_stl(path: &str) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "solid cube")?;

    let size = 20.0;
    let faces = [
        // Front (Z from 0 to size, Y = 0)
        ([0.0, 0.0, 0.0], [size, 0.0, 0.0], [size, 0.0, size]),
        ([0.0, 0.0, 0.0], [size, 0.0, size], [0.0, 0.0, size]),
        // Back (Y = size)
        ([size, size, 0.0], [0.0, size, 0.0], [0.0, size, size]),
        ([size, size, 0.0], [0.0, size, size], [size, size, size]),
        // Left (X = 0)
        ([0.0, size, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, size]),
        ([0.0, size, 0.0], [0.0, 0.0, size], [0.0, size, size]),
        // Right (X = size)
        ([size, 0.0, 0.0], [size, size, 0.0], [size, size, size]),
        ([size, 0.0, 0.0], [size, size, size], [size, 0.0, size]),
        // Bottom (Z = 0)
        ([0.0, size, 0.0], [size, size, 0.0], [size, 0.0, 0.0]),
        ([0.0, size, 0.0], [size, 0.0, 0.0], [0.0, 0.0, 0.0]),
        // Top (Z = size)
        ([0.0, 0.0, size], [size, 0.0, size], [size, size, size]),
        ([0.0, 0.0, size], [size, size, size], [0.0, size, size]),
    ];

    for (v1, v2, v3) in faces {
        writeln!(f, "  facet normal 0 0 0")?;
        writeln!(f, "    outer loop")?;
        writeln!(f, "      vertex {} {} {}", v1[0], v1[1], v1[2])?;
        writeln!(f, "      vertex {} {} {}", v2[0], v2[1], v2[2])?;
        writeln!(f, "      vertex {} {} {}", v3[0], v3[1], v3[2])?;
        writeln!(f, "    endloop")?;
        writeln!(f, "  endfacet")?;
    }

    writeln!(f, "endsolid cube")?;
    Ok(())
}
