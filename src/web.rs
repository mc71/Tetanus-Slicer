use std::time::Instant;
use base64::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Response, Server, StatusCode};

use crate::bvh::TriangleIntervalIndex;
use crate::gcode::{GCodeWriter, PrintConfig, ProcessedLayer};
use crate::geom::Point3;
use crate::infill::InfillGenerator;
use crate::perimeter::PerimeterGenerator;
use crate::slicer::Slicer;
use crate::stl::Mesh;
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
}

fn default_layer_height() -> f64 { 0.20 }
fn default_perimeters() -> usize { 2 }
fn default_infill() -> f64 { 0.20 }
fn default_speed() -> f64 { 50.0 }
fn default_nozzle_temp() -> u32 { 210 }
fn default_bed_temp() -> u32 { 60 }

#[derive(Serialize)]
pub struct SliceResponse {
    pub success: bool,
    pub error: Option<String>,
    pub stats: Option<SliceStats>,
    pub layers: Vec<WebLayer>,
    pub gcode: String,
}

#[derive(Serialize)]
pub struct WebLayer {
    pub index: usize,
    pub z: f64,
    pub perimeters: Vec<Vec<[f64; 2]>>,
    pub infill: Vec<[[f64; 2]; 2]>,
}

#[derive(Serialize)]
pub struct SliceStats {
    pub triangle_count: usize,
    pub layer_count: usize,
    pub elapsed_ms: f64,
    pub dimensions: [f64; 3],
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

    // 1. Decode STL bytes
    let raw_stl_bytes = if let Some(ref b64) = req.stl_base64 {
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

    // 2. Parse Mesh
    let mut mesh = match Mesh::from_slice(&raw_stl_bytes) {
        Ok(m) => m,
        Err(e) => {
            return SliceResponse {
                success: false,
                error: Some(format!("Failed to parse STL: {}", e)),
                stats: None,
                layers: Vec::new(),
                gcode: String::new(),
            };
        }
    };

    if mesh.triangles.is_empty() {
        return SliceResponse {
            success: false,
            error: Some("STL mesh contains 0 triangles".to_string()),
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
        travel_speed: 150.0,
        retract_dist: 0.8,
        retract_speed: 35.0,
    };

    let processed_layers: Vec<ProcessedLayer> = (0..layer_count)
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

            let contours = Slicer::chain_segments(&segments, 1e-4);

            let mut perimeters = Vec::new();
            let mut inner_boundaries = Vec::new();

            for contour in &contours {
                let perim_loops = PerimeterGenerator::generate_perimeters(
                    contour,
                    req.perimeters,
                    config.line_width,
                );
                if let Some(innermost) = perim_loops.last() {
                    inner_boundaries.push(innermost.clone());
                }
                perimeters.push(perim_loops);
            }

            let mut infill = Vec::new();
            for inner in &inner_boundaries {
                let infill_segs = InfillGenerator::generate_rectilinear(
                    inner,
                    req.infill_density,
                    config.line_width,
                    layer_idx,
                );
                infill.extend(infill_segs);
            }

            ProcessedLayer {
                layer_index: layer_idx,
                z,
                perimeters,
                infill,
            }
        })
        .collect();

    // 6. Generate G-code
    let mut gcode_buf = Vec::new();
    let mut gcode_writer = GCodeWriter::new(&config);
    let _ = gcode_writer.write_gcode(&mut gcode_buf, &processed_layers);
    let gcode_str = String::from_utf8_lossy(&gcode_buf).to_string();

    // 7. Convert Layers to Web Format for Three.js
    let web_layers: Vec<WebLayer> = processed_layers
        .into_iter()
        .map(|l| {
            let perimeters = l
                .perimeters
                .into_iter()
                .flat_map(|poly_group| {
                    poly_group
                        .into_iter()
                        .map(|p| p.points.into_iter().map(|pt| [pt.x, pt.y]).collect::<Vec<[f64; 2]>>())
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
                perimeters,
                infill,
            }
        })
        .collect();

    let elapsed = t_start.elapsed();

    SliceResponse {
        success: true,
        error: None,
        stats: Some(SliceStats {
            triangle_count: mesh.triangles.len(),
            layer_count: web_layers.len(),
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            dimensions: [dim_x, dim_y, dim_z],
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
