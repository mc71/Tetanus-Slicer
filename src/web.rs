use std::time::Instant;
use base64::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Response, Server, StatusCode};

use crate::bvh::TriangleIntervalIndex;
use crate::gcode::{GCodeWriter, PrintConfig, ProcessedLayer};
use crate::geom::{Point3, Polygon2};
use crate::infill::{InfillGenerator, InfillPattern};
use crate::perimeter::PerimeterGenerator;
use crate::slicer::{ContourRole, Slicer};
use crate::stl::Mesh;
use crate::support::{SupportConfig, SupportGenerator};
use crate::web_ui::INDEX_HTML;

#[derive(Deserialize)]
pub struct SliceRequest {
    pub stl_base64: Option<String>,
    #[serde(default = "default_layer_height")]
    pub layer_height: f64,
    #[serde(default = "default_perimeters")]
    pub perimeters: usize,
    #[serde(default = "default_infill")]
    pub infill_density: f64,
    #[serde(default = "default_speed")]
    pub print_speed: f64,
    #[serde(default = "default_nozzle_temp")]
    pub nozzle_temp: u32,
    #[serde(default = "default_bed_temp")]
    pub bed_temp: u32,
    #[serde(default = "default_top_solid")]
    pub top_solid_layers: usize,
    #[serde(default = "default_bottom_solid")]
    pub bottom_solid_layers: usize,
    #[serde(default = "default_skirt_loops")]
    pub skirt_loops: usize,
    #[serde(default = "default_brim_width")]
    pub brim_width: f64,
    #[serde(default = "default_z_hop")]
    pub z_hop: f64,
    #[serde(default = "default_fan_speed")]
    pub fan_speed: u8,
    #[serde(default = "default_infill_pattern")]
    pub infill_pattern: InfillPattern,
    #[serde(default = "default_support_enabled")]
    pub support_enabled: bool,
    #[serde(default = "default_support_angle")]
    pub support_angle: f64,
}

fn default_layer_height() -> f64 { 0.20 }
fn default_perimeters() -> usize { 2 }
fn default_infill() -> f64 { 0.20 }
fn default_speed() -> f64 { 50.0 }
fn default_nozzle_temp() -> u32 { 210 }
fn default_bed_temp() -> u32 { 60 }
fn default_top_solid() -> usize { 4 }
fn default_bottom_solid() -> usize { 4 }
fn default_skirt_loops() -> usize { 2 }
fn default_brim_width() -> f64 { 0.0 }
fn default_z_hop() -> f64 { 0.2 }
fn default_fan_speed() -> u8 { 255 }
fn default_infill_pattern() -> InfillPattern { InfillPattern::Rectilinear }
fn default_support_enabled() -> bool { false }
fn default_support_angle() -> f64 { 45.0 }

#[derive(Serialize)]
pub struct SliceResponse {
    pub success: bool,
    pub error: Option<String>,
    pub stats: Option<SliceStats>,
    pub layers: Vec<WebLayer>,
    pub gcode: String,
}

#[derive(Serialize)]
pub struct WebPerimeterPath {
    pub points: Vec<[f64; 2]>,
    pub is_outer: bool,
}

#[derive(Serialize)]
pub struct WebLayer {
    pub index: usize,
    pub z: f64,
    pub skirt_brim: Vec<Vec<[f64; 2]>>,
    pub supports: Vec<Vec<[f64; 2]>>,
    pub support_infill: Vec<[[f64; 2]; 2]>,
    pub perimeters: Vec<WebPerimeterPath>,
    pub infill: Vec<[[f64; 2]; 2]>,
}

#[derive(Serialize)]
pub struct SliceStats {
    pub triangle_count: usize,
    pub layer_count: usize,
    pub elapsed_ms: f64,
    pub dimensions: [f64; 3],
    pub print_time_formatted: String,
    pub filament_meters: f64,
    pub filament_grams: f64,
}

pub fn start_web_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr).map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    println!("============================================================");
    println!("       TETANUS SLICER - Web GUI Engine Running!             ");
    println!("============================================================");
    println!("  URL: http://localhost:{}", port);
    println!("  Threads: {} Rayon CPU Workers", rayon::current_num_threads());
    println!("  Press Ctrl+C to stop.");
    println!("============================================================");

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/");

        let header_json = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
        let header_html = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
        let header_cors = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();

        match (request.method(), path) {
            (&tiny_http::Method::Get, "/") | (&tiny_http::Method::Get, "/index.html") => {
                let response = Response::from_string(INDEX_HTML)
                    .with_header(header_html)
                    .with_header(header_cors);
                let _ = request.respond(response);
            }
            (&tiny_http::Method::Get, "/api/default_cube") => {
                let cube_stl = create_cube_stl_bytes();
                let header_stl = Header::from_bytes(&b"Content-Type"[..], &b"model/stl"[..]).unwrap();
                let response = Response::from_data(cube_stl)
                    .with_header(header_stl)
                    .with_header(header_cors);
                let _ = request.respond(response);
            }
            (&tiny_http::Method::Post, "/api/slice") => {
                let mut body = String::new();
                if let Err(e) = request.as_reader().read_to_string(&mut body) {
                    let err_resp = serde_json::to_string(&SliceResponse {
                        success: false,
                        error: Some(format!("Failed to read request body: {}", e)),
                        stats: None,
                        layers: Vec::new(),
                        gcode: String::new(),
                    }).unwrap();
                    let response = Response::from_string(err_resp)
                        .with_status_code(StatusCode(400))
                        .with_header(header_json)
                        .with_header(header_cors);
                    let _ = request.respond(response);
                    continue;
                }

                let req_payload: SliceRequest = match serde_json::from_str(&body) {
                    Ok(p) => p,
                    Err(e) => {
                        let err_resp = serde_json::to_string(&SliceResponse {
                            success: false,
                            error: Some(format!("Invalid JSON: {}", e)),
                            stats: None,
                            layers: Vec::new(),
                            gcode: String::new(),
                        }).unwrap();
                        let response = Response::from_string(err_resp)
                            .with_status_code(StatusCode(400))
                            .with_header(header_json)
                            .with_header(header_cors);
                        let _ = request.respond(response);
                        continue;
                    }
                };

                // Perform Slicing
                let slice_res = handle_slice_request(req_payload);
                let res_json = serde_json::to_string(&slice_res).unwrap();
                let response = Response::from_string(res_json)
                    .with_header(header_json)
                    .with_header(header_cors);
                let _ = request.respond(response);
            }
            _ => {
                let response = Response::from_string("Not Found")
                    .with_status_code(StatusCode(404))
                    .with_header(header_cors);
                let _ = request.respond(response);
            }
        }
    }

    Ok(())
}

fn handle_slice_request(req: SliceRequest) -> SliceResponse {
    let t_start = Instant::now();

    // 1. Decode bytes (STL or 3MF)
    let raw_bytes = if let Some(ref b64) = req.stl_base64 {
        match BASE64_STANDARD.decode(b64.trim()) {
            Ok(b) => b,
            Err(e) => {
                return SliceResponse {
                    success: false,
                    error: Some(format!("Base64 decoding failed: {}", e)),
                    stats: None,
                    layers: Vec::new(),
                    gcode: String::new(),
                };
            }
        }
    } else {
        create_cube_stl_bytes()
    };

    // 2. Parse Mesh (supports both STL and 3MF)
    let mut mesh = match Mesh::from_slice(&raw_bytes) {
        Ok(m) => m,
        Err(e) => {
            return SliceResponse {
                success: false,
                error: Some(format!("Failed to parse model: {}", e)),
                stats: None,
                layers: Vec::new(),
                gcode: String::new(),
            };
        }
    };

    if mesh.triangles.is_empty() {
        return SliceResponse {
            success: false,
            error: Some("Mesh contains 0 triangles".to_string()),
            stats: None,
            layers: Vec::new(),
            gcode: String::new(),
        };
    }

    // 3. Center model on 220x220 build plate, resting on Z = 0
    let center_x = (mesh.min_bound.x + mesh.max_bound.x) * 0.5;
    let center_y = (mesh.min_bound.y + mesh.max_bound.y) * 0.5;
    let min_z = mesh.min_bound.z;

    let offset_x = 110.0 - center_x;
    let offset_y = 110.0 - center_y;
    let offset_z = -min_z;

    let offset = Point3::new(offset_x, offset_y, offset_z);
    for tri in &mut mesh.triangles {
        tri.v[0] = tri.v[0] + offset;
        tri.v[1] = tri.v[1] + offset;
        tri.v[2] = tri.v[2] + offset;
    }

    let dim_x = mesh.max_bound.x - mesh.min_bound.x;
    let dim_y = mesh.max_bound.y - mesh.min_bound.y;
    let dim_z = mesh.max_bound.z - mesh.min_bound.z;

    // 4. Build Spatial Index
    let bvh = TriangleIntervalIndex::build(&mesh.triangles);

    // 5. Slice Layers in Parallel
    let layer_height = req.layer_height.max(0.04);
    let layer_count = (dim_z / layer_height).ceil().max(1.0) as usize;

    let config = PrintConfig {
        nozzle_temp: req.nozzle_temp,
        bed_temp: req.bed_temp,
        layer_height,
        line_width: 0.45,
        filament_diameter: 1.75,
        print_speed_perimeter: req.print_speed * 0.8,
        print_speed_infill: req.print_speed,
        first_layer_speed: (req.print_speed * 0.5).max(15.0),
        travel_speed: 150.0,
        retract_dist: 0.8,
        retract_speed: 35.0,
        z_hop: req.z_hop,
        top_solid_layers: req.top_solid_layers,
        bottom_solid_layers: req.bottom_solid_layers,
        skirt_loops: req.skirt_loops,
        skirt_distance: 4.0,
        brim_width: req.brim_width,
        fan_speed: req.fan_speed,
        fan_below_layer: 1,
        infill_pattern: req.infill_pattern,
        support_enabled: req.support_enabled,
        support_angle: req.support_angle,
        support_density: 0.15,
    };

    // Step 5a: Slicing layer contours in parallel
    let layer_contours: Vec<Vec<crate::slicer::ClassifiedContour>> = (0..layer_count)
        .into_par_iter()
        .map(|layer_idx| {
            let z = (layer_idx as f64 + 0.5) * config.layer_height;
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

    // Step 5b: Extract outer polygons and precompute layer areas
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

    let layer_areas: Vec<f64> = layers_outer_polys
        .iter()
        .map(|polys| polys.iter().map(|p| p.signed_area().abs()).sum())
        .collect();

    // Identify ALL top solid roof and bottom solid floor layers throughout the model
    let mut is_solid_layer = vec![false; layer_count];
    for idx in 0..config.bottom_solid_layers.min(layer_count) {
        is_solid_layer[idx] = true;
    }
    for idx in layer_count.saturating_sub(config.top_solid_layers)..layer_count {
        is_solid_layer[idx] = true;
    }
    // Intermediate roofs (e.g. deck, cabin roof, cargo steps)
    for i in 0..(layer_count.saturating_sub(1)) {
        let curr_area = layer_areas[i];
        let next_area = layer_areas[i + 1];
        if curr_area > next_area + 15.0 {
            let start = i.saturating_sub(config.top_solid_layers.saturating_sub(1));
            for k in start..=i {
                is_solid_layer[k] = true;
            }
        }
    }
    // Intermediate floors
    for i in 1..layer_count {
        let curr_area = layer_areas[i];
        let prev_area = layer_areas[i - 1];
        if curr_area > prev_area + 15.0 {
            let end = (i + config.bottom_solid_layers).min(layer_count);
            for k in i..end {
                is_solid_layer[k] = true;
            }
        }
    }

    // Step 5c: Support generation across layers if enabled
    let support_data: Vec<(Vec<Polygon2>, Vec<crate::geom::Segment2>)> = if config.support_enabled {
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

    // Step 5d: Parallel toolpath generation
    let processed_layers: Vec<ProcessedLayer> = (0..layer_count)
        .into_par_iter()
        .map(|layer_idx| {
            let z = (layer_idx as f64 + 0.5) * config.layer_height;
            let contours = &layer_contours[layer_idx];
            let (supports, support_infill) = support_data[layer_idx].clone();

            let mut perimeters = Vec::new();
            let mut infill_boundaries = Vec::new();

            for contour in contours {
                let perim_loops = PerimeterGenerator::generate_perimeters(
                    &contour.polygon,
                    &contour.role,
                    req.perimeters,
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
            let is_solid = is_solid_layer[layer_idx];
            let (pattern, layer_infill_density) = if is_solid {
                (InfillPattern::Rectilinear, 1.0)
            } else {
                (config.infill_pattern, req.infill_density)
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

    // 6. Generate G-code & Compute Print Estimates
    let mut gcode_buf = Vec::new();
    let mut gcode_writer = GCodeWriter::new(&config);
    let estimates = gcode_writer.write_gcode(&mut gcode_buf, &processed_layers).unwrap();
    let gcode_str = String::from_utf8_lossy(&gcode_buf).to_string();

    // 7. Convert Layers to Web Format for Three.js
    let web_layers: Vec<WebLayer> = processed_layers
        .into_iter()
        .map(|l| {
            let skirt_brim = l
                .skirt_brim
                .into_iter()
                .map(|p| p.points.into_iter().map(|pt| [pt.x, pt.y]).collect::<Vec<[f64; 2]>>())
                .collect();

            let supports = l
                .supports
                .into_iter()
                .map(|p| p.points.into_iter().map(|pt| [pt.x, pt.y]).collect::<Vec<[f64; 2]>>())
                .collect();

            let support_infill = l
                .support_infill
                .into_iter()
                .map(|seg| [[seg.p1.x, seg.p1.y], [seg.p2.x, seg.p2.y]])
                .collect();

            let perimeters: Vec<WebPerimeterPath> = l
                .perimeters
                .into_iter()
                .flat_map(|poly_group| {
                    poly_group
                        .into_iter()
                        .enumerate()
                        .map(|(idx, p)| WebPerimeterPath {
                            points: p.points.into_iter().map(|pt| [pt.x, pt.y]).collect(),
                            is_outer: idx == 0,
                        })
                })
                .collect();

            let infill = l
                .infill
                .into_iter()
                .map(|seg| [[seg.p1.x, seg.p1.y], [seg.p2.x, seg.p2.y]])
                .collect();

            WebLayer {
                index: l.layer_index,
                z: l.z,
                skirt_brim,
                supports,
                support_infill,
                perimeters,
                infill,
            }
        })
        .collect();

    let elapsed = t_start.elapsed();
    let total_secs = estimates.print_time_seconds;
    let hours = (total_secs / 3600.0).floor() as u64;
    let mins = ((total_secs % 3600.0) / 60.0).floor() as u64;
    let secs = (total_secs % 60.0).floor() as u64;
    let time_formatted = if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m {}s", mins, secs)
    };

    SliceResponse {
        success: true,
        error: None,
        stats: Some(SliceStats {
            triangle_count: mesh.triangles.len(),
            layer_count: web_layers.len(),
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            dimensions: [dim_x, dim_y, dim_z],
            print_time_formatted: time_formatted,
            filament_meters: estimates.filament_used_mm / 1000.0,
            filament_grams: estimates.filament_used_grams,
        }),
        layers: web_layers,
        gcode: gcode_str,
    }
}

pub fn create_cube_stl_bytes() -> Vec<u8> {
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

    let mut out = String::from("solid cube\n");
    for (v1, v2, v3) in faces {
        out.push_str("  facet normal 0 0 0\n    outer loop\n");
        out.push_str(&format!("      vertex {} {} {}\n", v1[0], v1[1], v1[2]));
        out.push_str(&format!("      vertex {} {} {}\n", v2[0], v2[1], v2[2]));
        out.push_str(&format!("      vertex {} {} {}\n", v3[0], v3[1], v3[2]));
        out.push_str("    endloop\n  endfacet\n");
    }
    out.push_str("endsolid cube\n");
    out.into_bytes()
}
