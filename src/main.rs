mod bvh;
mod geom;
mod gcode;
mod infill;
mod perimeter;
mod slicer;
mod stl;
mod support;
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
use infill::{InfillGenerator, InfillPattern};
use perimeter::PerimeterGenerator;
use slicer::{ContourRole, Slicer};
use stl::Mesh;
use support::{SupportConfig, SupportGenerator};

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
            "--infill-pattern" if i + 1 < args.len() => {
                config.infill_pattern = match args[i + 1].to_lowercase().as_str() {
                    "grid" => InfillPattern::Grid,
                    "triangles" | "triangle" => InfillPattern::Triangles,
                    "gyroid" => InfillPattern::Gyroid,
                    _ => InfillPattern::Rectilinear,
                };
                i += 1;
            }
            "--support" | "--supports" => {
                config.support_enabled = true;
            }
            "--support-angle" if i + 1 < args.len() => {
                config.support_angle = args[i + 1].parse().unwrap_or(45.0);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    println!("============================================================");
    println!("           Tetanus-Slicer (High-Speed Slicing Engine)       ");
    println!("============================================================");
    println!("Input File:      {}", input_path);
    println!("Output G-code:   {}", output_path);
    println!("Layer Height:    {:.3} mm", config.layer_height);
    println!("Perimeters:      {}", perimeter_count);
    println!("Infill Density:  {:.1}% ({:?})", infill_density * 100.0, config.infill_pattern);
    println!("Supports:        {}", if config.support_enabled { format!("Enabled ({}° overhang)", config.support_angle) } else { "Disabled".to_string() });
    println!("Thread Pool:     {} threads (Rayon)", rayon::current_num_threads());
    println!("------------------------------------------------------------");

    // 1. Load Model (STL or 3MF)
    let t_start = Instant::now();
    let mesh = Mesh::load_stl(&input_path).unwrap_or_else(|e| {
        eprintln!("Error loading model: {}", e);
        std::process::exit(1);
    });
    let t_mesh = t_start.elapsed();
    println!(
        "[1/4] Loaded mesh in {:?}: {} triangles | Bounds: ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2})",
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

    // 3. Compute Layer Heights & Slice Contours in Parallel with Rayon
    let t_slice_start = Instant::now();
    let total_height = mesh.max_bound.z - mesh.min_bound.z;
    let layer_count = (total_height / config.layer_height).ceil().max(1.0) as usize;

    println!(
        "[3/4] Slicing {} layers in parallel across {} cores...",
        layer_count,
        rayon::current_num_threads()
    );

    let min_z = mesh.min_bound.z;

    // Step 3a: Parallel contour slicing
    let layer_contours: Vec<Vec<slicer::ClassifiedContour>> = (0..layer_count)
        .into_par_iter()
        .map(|layer_idx| {
            let z = min_z + (layer_idx as f64 + 0.5) * config.layer_height;
            let intersecting = bvh.query_z(z);
            let mut segments = Vec::with_capacity(intersecting.len());
            for tri in intersecting {
                if let Some(seg) = Slicer::intersect_triangle(tri, z) {
                    segments.push(seg);
                }
            }
            Slicer::chain_segments(&segments)
        })
        .collect();

    // Step 3b: Support generation across layers if enabled
    let support_data: Vec<(Vec<Polygon2>, Vec<geom::Segment2>)> = if config.support_enabled {
        let layers_outer_polys: Vec<Vec<Polygon2>> = layer_contours
            .iter()
            .map(|contours| {
                contours
                    .iter()
                    .filter(|c| c.role == ContourRole::Outer)
                    .map(|c| c.polygon.clone())
                    .collect()
            })
            .collect();

        let sup_cfg = SupportConfig {
            enabled: true,
            overhang_angle: config.support_angle,
            support_density: config.support_density,
            line_width: config.line_width,
            layer_height: config.layer_height,
            xy_gap: 0.6,
        };
        SupportGenerator::generate_supports(&layers_outer_polys, &sup_cfg)
    } else {
        vec![(Vec::new(), Vec::new()); layer_count]
    };

    // Step 3c: Parallel toolpath generation (perimeters, infill, skirt/brim, supports)
    let layers: Vec<ProcessedLayer> = (0..layer_count)
        .into_par_iter()
        .map(|layer_idx| {
            let z = min_z + (layer_idx as f64 + 0.5) * config.layer_height;
            let contours = &layer_contours[layer_idx];
            let (supports, support_infill) = support_data[layer_idx].clone();

            // Generate perimeters & inner boundaries
            let mut perimeters = Vec::new();
            let mut infill_boundaries = Vec::new();

            for contour in contours {
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
            let (pattern, layer_infill_density) = if is_solid {
                (InfillPattern::Rectilinear, 1.0)
            } else {
                (config.infill_pattern, infill_density)
            };

            let infill = InfillGenerator::generate_infill(
                pattern,
                &infill_boundaries,
                layer_infill_density,
                config.line_width,
                layer_idx,
                z,
            );

            ProcessedLayer {
                layer_index: layer_idx,
                z,
                skirt_brim,
                supports,
                support_infill,
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
    let estimates = gcode_writer.write_gcode(&mut writer, &layers).unwrap();
    let t_gcode = t_gcode_start.elapsed();

    let total_time = t_start.elapsed();
    let hours = (estimates.print_time_seconds / 3600.0).floor() as u64;
    let mins = ((estimates.print_time_seconds % 3600.0) / 60.0).floor() as u64;
    let secs = (estimates.print_time_seconds % 60.0).floor() as u64;
    let meters = estimates.filament_used_mm / 1000.0;

    println!("[4/4] G-code written in {:?}", t_gcode);
    println!("------------------------------------------------------------");
    println!("✓ Slicing Finished Successfully!");
    println!("  Total Elapsed Time: {:?}", total_time);
    println!("  Throughput:         {:.1} layers/sec", layer_count as f64 / total_time.as_secs_f64());
    println!("  Est. Print Time:    {}h {}m {}s", hours, mins, secs);
    println!("  Filament Used:      {:.2} m ({:.1} g)", meters, estimates.filament_used_grams);
    println!("  Generated:          {}", output_path);
    println!("============================================================");
}

/// Helper function to create an ASCII STL 20x20x20mm cube for testing
fn generate_test_cube_stl(path: &str) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "solid cube")?;

    let size = 20.0;
    let faces = [
        ([0.0, 0.0, 0.0], [size, 0.0, 0.0], [size, 0.0, size]),
        ([0.0, 0.0, 0.0], [size, 0.0, size], [0.0, 0.0, size]),
        ([size, size, 0.0], [0.0, size, 0.0], [0.0, size, size]),
        ([size, size, 0.0], [0.0, size, size], [size, size, size]),
        ([0.0, size, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, size]),
        ([0.0, size, 0.0], [0.0, 0.0, size], [0.0, size, size]),
        ([size, 0.0, 0.0], [size, size, 0.0], [size, size, size]),
        ([size, 0.0, 0.0], [size, size, size], [size, 0.0, size]),
        ([0.0, size, 0.0], [size, size, 0.0], [size, 0.0, 0.0]),
        ([0.0, size, 0.0], [size, 0.0, 0.0], [0.0, 0.0, 0.0]),
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
