use std::io::{Read, Seek};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use zip::ZipArchive;

use crate::geom::{Point3, Triangle};
use crate::stl::Mesh;

pub struct ThreeMfParser;

impl ThreeMfParser {
    pub fn parse<R: Read + Seek>(reader: R) -> Result<Mesh, String> {
        let mut archive = ZipArchive::new(reader).map_err(|e| format!("Invalid 3MF archive: {}", e))?;

        // Find .model file (typically "3D/3dmodel.model")
        let model_filename = (0..archive.len())
            .find_map(|i| {
                let file = archive.by_index(i).ok()?;
                let name = file.name().to_string();
                if name.ends_with(".model") {
                    Some(name)
                } else {
                    None
                }
            })
            .ok_or_else(|| "No .model file found in 3MF archive".to_string())?;

        let mut model_file = archive
            .by_name(&model_filename)
            .map_err(|e| format!("Failed to read {}: {}", model_filename, e))?;

        let mut xml_content = String::new();
        model_file
            .read_to_string(&mut xml_content)
            .map_err(|e| format!("Failed to read XML: {}", e))?;

        Self::parse_model_xml(&xml_content)
    }

    fn parse_model_xml(xml: &str) -> Result<Mesh, String> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut scale = 1.0; // Default to millimeter
        let mut vertices: Vec<Point3> = Vec::new();
        let mut triangles: Vec<Triangle> = Vec::new();

        let mut min_bound = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_bound = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local_name = e.local_name();
                    let name_str = String::from_utf8_lossy(local_name.as_ref());

                    if name_str == "model" {
                        // Extract unit
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"unit" {
                                let unit_val = String::from_utf8_lossy(&attr.value);
                                scale = match unit_val.as_ref() {
                                    "meter" => 1000.0,
                                    "centimeter" => 10.0,
                                    "inch" => 25.4,
                                    "micron" => 0.001,
                                    _ => 1.0, // millimeter
                                };
                            }
                        }
                    } else if name_str == "vertex" {
                        let mut x = 0.0;
                        let mut y = 0.0;
                        let mut z = 0.0;

                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let val_str = String::from_utf8_lossy(&attr.value);
                            match key.as_ref() {
                                b"x" => x = val_str.parse::<f64>().unwrap_or(0.0) * scale,
                                b"y" => y = val_str.parse::<f64>().unwrap_or(0.0) * scale,
                                b"z" => z = val_str.parse::<f64>().unwrap_or(0.0) * scale,
                                _ => {}
                            }
                        }

                        vertices.push(Point3::new(x, y, z));
                    } else if name_str == "triangle" {
                        let mut v1 = 0;
                        let mut v2 = 0;
                        let mut v3 = 0;

                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let val_str = String::from_utf8_lossy(&attr.value);
                            match key.as_ref() {
                                b"v1" => v1 = val_str.parse::<usize>().unwrap_or(0),
                                b"v2" => v2 = val_str.parse::<usize>().unwrap_or(0),
                                b"v3" => v3 = val_str.parse::<usize>().unwrap_or(0),
                                _ => {}
                            }
                        }

                        if v1 < vertices.len() && v2 < vertices.len() && v3 < vertices.len() {
                            let p1 = vertices[v1];
                            let p2 = vertices[v2];
                            let p3 = vertices[v3];

                            min_bound.x = min_bound.x.min(p1.x).min(p2.x).min(p3.x);
                            min_bound.y = min_bound.y.min(p1.y).min(p2.y).min(p3.y);
                            min_bound.z = min_bound.z.min(p1.z).min(p2.z).min(p3.z);

                            max_bound.x = max_bound.x.max(p1.x).max(p2.x).max(p3.x);
                            max_bound.y = max_bound.y.max(p1.y).max(p2.y).max(p3.y);
                            max_bound.z = max_bound.z.max(p1.z).max(p2.z).max(p3.z);

                            // Calculate normal from right-hand rule
                            let e1 = p2 - p1;
                            let e2 = p3 - p1;
                            let normal = Point3::new(
                                e1.y * e2.z - e1.z * e2.y,
                                e1.z * e2.x - e1.x * e2.z,
                                e1.x * e2.y - e1.y * e2.x,
                            );

                            triangles.push(Triangle {
                                normal,
                                v: [p1, p2, p3],
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(format!("Error parsing XML at position {}: {:?}", reader.buffer_position(), err)),
                _ => {}
            }
            buf.clear();
        }

        if triangles.is_empty() {
            return Err("3MF model contains 0 triangles".to_string());
        }

        Ok(Mesh {
            triangles,
            min_bound,
            max_bound,
        })
    }
}
