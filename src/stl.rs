use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt};
use crate::geom::{Point3, Triangle};

pub struct Mesh {
    pub triangles: Vec<Triangle>,
    pub min_bound: Point3,
    pub max_bound: Point3,
}

impl Mesh {
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref();
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            if ext.eq_ignore_ascii_case("3mf") {
                let file = File::open(p).map_err(|e| format!("Failed to open file: {}", e))?;
                return crate::threemf::ThreeMfParser::parse(BufReader::new(file));
            }
        }
        let file = File::open(p).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut reader = BufReader::new(file);
        let mut magic = [0u8; 4];
        if reader.read_exact(&mut magic).is_ok() && &magic == b"PK\x03\x04" {
            reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            return crate::threemf::ThreeMfParser::parse(reader);
        }
        reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let file_len = reader.get_ref().metadata().map_err(|e| e.to_string())?.len();
        Self::from_reader(&mut reader, file_len)
    }

    pub fn load_stl<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        Self::load_file(path)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        if bytes.starts_with(b"PK\x03\x04") {
            let cursor = std::io::Cursor::new(bytes);
            return crate::threemf::ThreeMfParser::parse(cursor);
        }
        let mut cursor = std::io::Cursor::new(bytes);
        Self::from_reader(&mut cursor, bytes.len() as u64)
    }

    pub fn from_reader<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<Self, String> {
        let mut header = [0u8; 80];
        reader.read_exact(&mut header).map_err(|e| e.to_string())?;

        // If file starts with "solid" and has reasonable ASCII content
        let is_ascii_header = header.starts_with(b"solid");
        if is_ascii_header && file_len > 84 {
            // Read triangle count if it was binary
            let mut num_triangles_buf = [0u8; 4];
            reader.read_exact(&mut num_triangles_buf).map_err(|e| e.to_string())?;
            let num_triangles = u32::from_le_bytes(num_triangles_buf);
            let expected_binary_len = 84 + (num_triangles as u64) * 50;
            if expected_binary_len == file_len {
                // It's actually a binary STL with "solid" in the 80-byte header
                reader.seek(SeekFrom::Start(84)).map_err(|e| e.to_string())?;
                return Self::parse_binary_stl(reader, num_triangles);
            } else {
                // Parse as ASCII
                reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
                return Self::parse_ascii_stl(reader);
            }
        }

        let num_triangles = reader.read_u32::<LittleEndian>().map_err(|e| e.to_string())?;
        Self::parse_binary_stl(reader, num_triangles)
    }

    fn parse_binary_stl<R: Read>(reader: &mut R, count: u32) -> Result<Self, String> {
        let mut triangles = Vec::with_capacity(count as usize);
        let mut min_bound = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_bound = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        for _ in 0..count {
            let nx = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;
            let ny = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;
            let nz = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;

            let mut v = [Point3::new(0.0, 0.0, 0.0); 3];
            for i in 0..3 {
                let x = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;
                let y = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;
                let z = reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())? as f64;
                let pt = Point3::new(x, y, z);
                v[i] = pt;

                min_bound.x = min_bound.x.min(pt.x);
                min_bound.y = min_bound.y.min(pt.y);
                min_bound.z = min_bound.z.min(pt.z);

                max_bound.x = max_bound.x.max(pt.x);
                max_bound.y = max_bound.y.max(pt.y);
                max_bound.z = max_bound.z.max(pt.z);
            }

            let _attribute_byte_count = reader.read_u16::<LittleEndian>().map_err(|e| e.to_string())?;

            triangles.push(Triangle {
                normal: Point3::new(nx, ny, nz),
                v,
            });
        }

        Ok(Self {
            triangles,
            min_bound,
            max_bound,
        })
    }

    fn parse_ascii_stl<R: Read>(reader: &mut R) -> Result<Self, String> {
        let mut content = String::new();
        reader.read_to_string(&mut content).map_err(|e| e.to_string())?;

        let mut triangles = Vec::new();
        let mut min_bound = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_bound = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        let mut current_normal = Point3::new(0.0, 0.0, 0.0);
        let mut vertices = Vec::new();

        for line in content.lines() {
            let tokens: Vec<&str> = line.trim().split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            if tokens[0] == "facet" && tokens.len() >= 5 && tokens[1] == "normal" {
                let nx = tokens[2].parse::<f64>().unwrap_or(0.0);
                let ny = tokens[3].parse::<f64>().unwrap_or(0.0);
                let nz = tokens[4].parse::<f64>().unwrap_or(0.0);
                current_normal = Point3::new(nx, ny, nz);
                vertices.clear();
            } else if tokens[0] == "vertex" && tokens.len() >= 4 {
                let x = tokens[1].parse::<f64>().unwrap_or(0.0);
                let y = tokens[2].parse::<f64>().unwrap_or(0.0);
                let z = tokens[3].parse::<f64>().unwrap_or(0.0);
                let pt = Point3::new(x, y, z);
                vertices.push(pt);

                min_bound.x = min_bound.x.min(pt.x);
                min_bound.y = min_bound.y.min(pt.y);
                min_bound.z = min_bound.z.min(pt.z);

                max_bound.x = max_bound.x.max(pt.x);
                max_bound.y = max_bound.y.max(pt.y);
                max_bound.z = max_bound.z.max(pt.z);
            } else if tokens[0] == "endfacet" && vertices.len() == 3 {
                triangles.push(Triangle {
                    normal: current_normal,
                    v: [vertices[0], vertices[1], vertices[2]],
                });
                vertices.clear();
            }
        }

        Ok(Self {
            triangles,
            min_bound,
            max_bound,
        })
    }

    pub fn translate(&mut self, dx: f64, dy: f64, dz: f64) {
        let offset = Point3::new(dx, dy, dz);
        for tri in &mut self.triangles {
            tri.v[0] = tri.v[0] + offset;
            tri.v[1] = tri.v[1] + offset;
            tri.v[2] = tri.v[2] + offset;
        }
        self.min_bound = self.min_bound + offset;
        self.max_bound = self.max_bound + offset;
    }

    pub fn center_on_bed(&mut self, bed_center_x: f64, bed_center_y: f64) {
        let cur_cx = (self.min_bound.x + self.max_bound.x) * 0.5;
        let cur_cy = (self.min_bound.y + self.max_bound.y) * 0.5;
        let cur_min_z = self.min_bound.z;
        self.translate(bed_center_x - cur_cx, bed_center_y - cur_cy, -cur_min_z);
    }

    pub fn combine(meshes: &[Mesh]) -> Mesh {
        if meshes.is_empty() {
            return Mesh {
                triangles: Vec::new(),
                min_bound: Point3::new(0.0, 0.0, 0.0),
                max_bound: Point3::new(0.0, 0.0, 0.0),
            };
        }

        let mut all_triangles = Vec::new();
        let mut min_bound = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_bound = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        for m in meshes {
            all_triangles.extend_from_slice(&m.triangles);
            min_bound.x = min_bound.x.min(m.min_bound.x);
            min_bound.y = min_bound.y.min(m.min_bound.y);
            min_bound.z = min_bound.z.min(m.min_bound.z);
            max_bound.x = max_bound.x.max(m.max_bound.x);
            max_bound.y = max_bound.y.max(m.max_bound.y);
            max_bound.z = max_bound.z.max(m.max_bound.z);
        }

        Mesh {
            triangles: all_triangles,
            min_bound,
            max_bound,
        }
    }

    /// Auto-arrange multiple meshes on the print bed with a safety margin gap between parts.
    pub fn auto_arrange(meshes: &mut [Mesh], gap: f64) {
        if meshes.is_empty() {
            return;
        }
        if meshes.len() == 1 {
            meshes[0].center_on_bed(0.0, 0.0);
            return;
        }

        let cols = (meshes.len() as f64).sqrt().ceil() as usize;
        let mut row_max_h = 0.0;
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;
        let mut positions = Vec::with_capacity(meshes.len());

        for (i, m) in meshes.iter().enumerate() {
            let width = m.max_bound.x - m.min_bound.x;
            let depth = m.max_bound.y - m.min_bound.y;

            if i > 0 && i % cols == 0 {
                cur_x = 0.0;
                cur_y += row_max_h + gap;
                row_max_h = 0.0;
            }

            positions.push((cur_x + width * 0.5, cur_y + depth * 0.5));
            cur_x += width + gap;
            row_max_h = row_max_h.max(depth);
        }

        let total_w = cur_x;
        let total_d = cur_y + row_max_h;
        let offset_x = -total_w * 0.5;
        let offset_y = -total_d * 0.5;

        for (i, m) in meshes.iter_mut().enumerate() {
            let target_cx = positions[i].0 + offset_x;
            let target_cy = positions[i].1 + offset_y;
            m.center_on_bed(target_cx, target_cy);
        }
    }
}
