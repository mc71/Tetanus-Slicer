use std::collections::HashMap;
use std::io::{Read, Seek};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use zip::ZipArchive;

use crate::geom::{Point3, Triangle};
use crate::stl::Mesh;

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub m: [f64; 12],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            m: [
                1.0, 0.0, 0.0,
                0.0, 1.0, 0.0,
                0.0, 0.0, 1.0,
                0.0, 0.0, 0.0,
            ],
        }
    }
}

impl Transform {
    pub fn parse(s: &str) -> Self {
        let nums: Vec<f64> = s
            .split_whitespace()
            .filter_map(|w| w.parse::<f64>().ok())
            .collect();

        if nums.len() == 12 {
            let mut m = [0.0; 12];
            m.copy_from_slice(&nums);
            Self { m }
        } else {
            Self::default()
        }
    }

    pub fn apply(&self, p: Point3) -> Point3 {
        let x = self.m[0] * p.x + self.m[1] * p.y + self.m[2] * p.z + self.m[9];
        let y = self.m[3] * p.x + self.m[4] * p.y + self.m[5] * p.z + self.m[10];
        let z = self.m[6] * p.x + self.m[7] * p.y + self.m[8] * p.z + self.m[11];
        Point3::new(x, y, z)
    }

    pub fn mul(&self, rhs: &Self) -> Self {
        let a = &self.m;
        let b = &rhs.m;
        let mut res = [0.0; 12];

        // 3x3 rotation part
        res[0] = a[0] * b[0] + a[1] * b[3] + a[2] * b[6];
        res[1] = a[0] * b[1] + a[1] * b[4] + a[2] * b[7];
        res[2] = a[0] * b[2] + a[1] * b[5] + a[2] * b[8];

        res[3] = a[3] * b[0] + a[4] * b[3] + a[5] * b[6];
        res[4] = a[3] * b[1] + a[4] * b[4] + a[5] * b[7];
        res[5] = a[3] * b[2] + a[4] * b[5] + a[5] * b[8];

        res[6] = a[6] * b[0] + a[7] * b[3] + a[8] * b[6];
        res[7] = a[6] * b[1] + a[7] * b[4] + a[8] * b[7];
        res[8] = a[6] * b[2] + a[7] * b[5] + a[8] * b[8];

        // 3D translation part
        res[9] = a[0] * b[9] + a[1] * b[10] + a[2] * b[11] + a[9];
        res[10] = a[3] * b[9] + a[4] * b[10] + a[5] * b[11] + a[10];
        res[11] = a[6] * b[9] + a[7] * b[10] + a[8] * b[11] + a[11];

        Self { m: res }
    }
}

enum ObjectContent {
    Mesh {
        vertices: Vec<Point3>,
        triangles: Vec<[usize; 3]>,
    },
    Components(Vec<ComponentDef>),
}

struct ComponentDef {
    path: Option<String>,
    objectid: String,
    transform: Transform,
}

struct BuildItem {
    objectid: String,
    transform: Transform,
}

struct ModelFileDef {
    unit_scale: f64,
    objects: HashMap<String, ObjectContent>,
    build_items: Vec<BuildItem>,
}

pub struct ThreeMfParser;

impl ThreeMfParser {
    pub fn parse<R: Read + Seek>(reader: R) -> Result<Mesh, String> {
        let mut archive = ZipArchive::new(reader).map_err(|e| format!("Invalid 3MF archive: {}", e))?;

        let mut model_files: HashMap<String, ModelFileDef> = HashMap::new();

        // 1. Read and parse all .model XML files in the 3MF container
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = file.name().to_string();
            if name.ends_with(".model") {
                let mut xml_text = String::new();
                file.read_to_string(&mut xml_text).map_err(|e| e.to_string())?;

                let model_def = Self::parse_model_xml(&xml_text)?;
                let norm_name = format!("/{}", name.trim_start_matches('/'));
                model_files.insert(norm_name.clone(), model_def);
                model_files.insert(name, Self::parse_model_xml(&xml_text)?);
            }
        }

        if model_files.is_empty() {
            return Err("No .model files found in 3MF archive".to_string());
        }

        // 2. Locate root model (typically /3D/3dmodel.model)
        let root_key = model_files
            .keys()
            .find(|k| k.ends_with("3dmodel.model"))
            .cloned()
            .unwrap_or_else(|| model_files.keys().next().unwrap().clone());

        // 3. Instantiate triangles according to <build> and <components>
        let mut final_triangles: Vec<Triangle> = Vec::new();
        let mut min_bound = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_bound = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        if let Some(root_def) = model_files.get(&root_key) {
            for item in &root_def.build_items {
                Self::collect_object_triangles(
                    &model_files,
                    &root_key,
                    &item.objectid,
                    &item.transform,
                    &mut final_triangles,
                    &mut min_bound,
                    &mut max_bound,
                );
            }
        }

        // 4. Fallback if build was empty or referenced external files not matched:
        // Collect all meshes directly from all object definitions
        if final_triangles.is_empty() {
            for (_, model_def) in &model_files {
                for (_, obj) in &model_def.objects {
                    if let ObjectContent::Mesh { ref vertices, ref triangles } = obj {
                        for t in triangles {
                            let p1 = vertices[t[0]];
                            let p2 = vertices[t[1]];
                            let p3 = vertices[t[2]];

                            min_bound.x = min_bound.x.min(p1.x).min(p2.x).min(p3.x);
                            min_bound.y = min_bound.y.min(p1.y).min(p2.y).min(p3.y);
                            min_bound.z = min_bound.z.min(p1.z).min(p2.z).min(p3.z);

                            max_bound.x = max_bound.x.max(p1.x).max(p2.x).max(p3.x);
                            max_bound.y = max_bound.y.max(p1.y).max(p2.y).max(p3.y);
                            max_bound.z = max_bound.z.max(p1.z).max(p2.z).max(p3.z);

                            let e1 = p2 - p1;
                            let e2 = p3 - p1;
                            let normal = Point3::new(
                                e1.y * e2.z - e1.z * e2.y,
                                e1.z * e2.x - e1.x * e2.z,
                                e1.x * e2.y - e1.y * e2.x,
                            );

                            final_triangles.push(Triangle {
                                normal,
                                v: [p1, p2, p3],
                            });
                        }
                    }
                }
            }
        }

        if final_triangles.is_empty() {
            return Err("3MF archive contains 0 renderable triangles".to_string());
        }

        Ok(Mesh {
            triangles: final_triangles,
            min_bound,
            max_bound,
        })
    }

    fn collect_object_triangles(
        model_files: &HashMap<String, ModelFileDef>,
        current_file: &str,
        object_id: &str,
        current_transform: &Transform,
        out_triangles: &mut Vec<Triangle>,
        min_bound: &mut Point3,
        max_bound: &mut Point3,
    ) {
        let model_def = match model_files.get(current_file) {
            Some(m) => m,
            None => {
                // Try finding by suffix if path starts with or without leading slash
                let alt = format!("/{}", current_file.trim_start_matches('/'));
                match model_files.get(&alt) {
                    Some(m) => m,
                    None => return,
                }
            }
        };

        let obj = match model_def.objects.get(object_id) {
            Some(o) => o,
            None => return,
        };

        match obj {
            ObjectContent::Mesh { ref vertices, ref triangles } => {
                for t in triangles {
                    let p1 = current_transform.apply(vertices[t[0]]);
                    let p2 = current_transform.apply(vertices[t[1]]);
                    let p3 = current_transform.apply(vertices[t[2]]);

                    min_bound.x = min_bound.x.min(p1.x).min(p2.x).min(p3.x);
                    min_bound.y = min_bound.y.min(p1.y).min(p2.y).min(p3.y);
                    min_bound.z = min_bound.z.min(p1.z).min(p2.z).min(p3.z);

                    max_bound.x = max_bound.x.max(p1.x).max(p2.x).max(p3.x);
                    max_bound.y = max_bound.y.max(p1.y).max(p2.y).max(p3.y);
                    max_bound.z = max_bound.z.max(p1.z).max(p2.z).max(p3.z);

                    let e1 = p2 - p1;
                    let e2 = p3 - p1;
                    let normal = Point3::new(
                        e1.y * e2.z - e1.z * e2.y,
                        e1.z * e2.x - e1.x * e2.z,
                        e1.x * e2.y - e1.y * e2.x,
                    );

                    out_triangles.push(Triangle {
                        normal,
                        v: [p1, p2, p3],
                    });
                }
            }
            ObjectContent::Components(ref comps) => {
                for c in comps {
                    let target_file = c.path.as_deref().unwrap_or(current_file);
                    let combined_trans = current_transform.mul(&c.transform);
                    Self::collect_object_triangles(
                        model_files,
                        target_file,
                        &c.objectid,
                        &combined_trans,
                        out_triangles,
                        min_bound,
                        max_bound,
                    );
                }
            }
        }
    }

    fn parse_model_xml(xml: &str) -> Result<ModelFileDef, String> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut unit_scale = 1.0;
        let mut objects: HashMap<String, ObjectContent> = HashMap::new();
        let mut build_items: Vec<BuildItem> = Vec::new();

        let mut current_obj_id: Option<String> = None;
        let mut current_vertices: Vec<Point3> = Vec::new();
        let mut current_triangles: Vec<[usize; 3]> = Vec::new();
        let mut current_components: Vec<ComponentDef> = Vec::new();
        let mut in_mesh = false;
        let mut in_components = false;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    let name_str = String::from_utf8_lossy(local_name.as_ref());

                    if name_str == "model" {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"unit" {
                                let unit_val = String::from_utf8_lossy(&attr.value);
                                unit_scale = match unit_val.as_ref() {
                                    "meter" => 1000.0,
                                    "centimeter" => 10.0,
                                    "inch" => 25.4,
                                    "micron" => 0.001,
                                    _ => 1.0,
                                };
                            }
                        }
                    } else if name_str == "object" {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"id" {
                                current_obj_id = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                        current_vertices.clear();
                        current_triangles.clear();
                        current_components.clear();
                        in_mesh = false;
                        in_components = false;
                    } else if name_str == "mesh" {
                        in_mesh = true;
                    } else if name_str == "components" {
                        in_components = true;
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let local_name = e.local_name();
                    let name_str = String::from_utf8_lossy(local_name.as_ref());

                    if name_str == "vertex" && in_mesh {
                        let mut x = 0.0;
                        let mut y = 0.0;
                        let mut z = 0.0;
                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let val_str = String::from_utf8_lossy(&attr.value);
                            match key.as_ref() {
                                b"x" => x = val_str.parse::<f64>().unwrap_or(0.0) * unit_scale,
                                b"y" => y = val_str.parse::<f64>().unwrap_or(0.0) * unit_scale,
                                b"z" => z = val_str.parse::<f64>().unwrap_or(0.0) * unit_scale,
                                _ => {}
                            }
                        }
                        current_vertices.push(Point3::new(x, y, z));
                    } else if name_str == "triangle" && in_mesh {
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
                        current_triangles.push([v1, v2, v3]);
                    } else if name_str == "component" && in_components {
                        let mut path = None;
                        let mut objectid = String::new();
                        let mut transform = Transform::default();

                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let val_str = String::from_utf8_lossy(&attr.value);
                            if key.as_ref() == b"path" {
                                path = Some(val_str.to_string());
                            } else if key.as_ref() == b"objectid" {
                                objectid = val_str.to_string();
                            } else if key.as_ref() == b"transform" {
                                transform = Transform::parse(&val_str);
                            }
                        }
                        if !objectid.is_empty() {
                            current_components.push(ComponentDef {
                                path,
                                objectid,
                                transform,
                            });
                        }
                    } else if name_str == "item" {
                        let mut objectid = String::new();
                        let mut transform = Transform::default();
                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let val_str = String::from_utf8_lossy(&attr.value);
                            if key.as_ref() == b"objectid" {
                                objectid = val_str.to_string();
                            } else if key.as_ref() == b"transform" {
                                transform = Transform::parse(&val_str);
                            }
                        }
                        if !objectid.is_empty() {
                            build_items.push(BuildItem {
                                objectid,
                                transform,
                            });
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    let name_str = String::from_utf8_lossy(local_name.as_ref());

                    if name_str == "object" {
                        if let Some(id) = current_obj_id.take() {
                            if !current_vertices.is_empty() {
                                objects.insert(
                                    id,
                                    ObjectContent::Mesh {
                                        vertices: std::mem::take(&mut current_vertices),
                                        triangles: std::mem::take(&mut current_triangles),
                                    },
                                );
                            } else if !current_components.is_empty() {
                                objects.insert(
                                    id,
                                    ObjectContent::Components(std::mem::take(&mut current_components)),
                                );
                            }
                        }
                        in_mesh = false;
                        in_components = false;
                    } else if name_str == "mesh" {
                        in_mesh = false;
                    } else if name_str == "components" {
                        in_components = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(format!("Error parsing XML: {:?}", err)),
                _ => {}
            }
            buf.clear();
        }

        Ok(ModelFileDef {
            unit_scale,
            objects,
            build_items,
        })
    }
}
