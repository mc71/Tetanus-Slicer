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
    pub fn load_stl<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut reader = BufReader::new(file);
        let file_len = reader.get_ref().metadata().map_err(|e| e.to_string())?.len();
        Self::from_reader(&mut reader, file_len)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, String> {
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
}
