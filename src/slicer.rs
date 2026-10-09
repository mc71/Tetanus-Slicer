#![allow(dead_code)]

use std::collections::HashMap;
use crate::geom::{Point2, Polygon2, Segment2, Triangle};

#[derive(Clone, Debug)]
pub struct Layer {
    pub layer_index: usize,
    pub z: f64,
    pub contours: Vec<Polygon2>,
}

pub struct Slicer;

impl Slicer {
    /// Intersect a single triangle with a plane at height Z.
    /// Returns an oriented line segment if an intersection exists.
    pub fn intersect_triangle(triangle: &Triangle, z: f64) -> Option<Segment2> {
        let v = &triangle.v;
        let mut pts = Vec::with_capacity(3);

        // Check each of the 3 edges
        let edges = [(0, 1), (1, 2), (2, 0)];
        for &(i, j) in &edges {
            let v1 = v[i];
            let v2 = v[j];

            let min_z = v1.z.min(v2.z);
            let max_z = v1.z.max(v2.z);

            if z >= min_z && z <= max_z && (v2.z - v1.z).abs() > 1e-9 {
                let t = (z - v1.z) / (v2.z - v1.z);
                if (0.0..=1.0).contains(&t) {
                    let pt3 = v1 + (v2 - v1) * t;
                    let pt2 = pt3.to_2d();
                    // Avoid duplicate intersection point from sharing vertices
                    if !pts.iter().any(|p: &Point2| p.distance_to(pt2) < 1e-6) {
                        pts.push(pt2);
                    }
                }
            }
        }

        if pts.len() == 2 {
            // Determine orientation using normal projection
            // If normal.z is flat or general, ensure right-hand rule
            let p1 = pts[0];
            let p2 = pts[1];

            // Direct orientation based on triangle vertices
            let edge = p2 - p1;
            let normal_2d = Point2::new(-edge.y, edge.x);
            let face_center_2d = ((v[0] + v[1] + v[2]) * (1.0 / 3.0)).to_2d();
            let to_center = face_center_2d - p1;

            if normal_2d.dot(to_center) > 0.0 {
                Some(Segment2::new(p2, p1))
            } else {
                Some(Segment2::new(p1, p2))
            }
        } else {
            None
        }
    }

    /// Chains a collection of unordered line segments into closed 2D polygon loops.
    pub fn chain_segments(segments: &[Segment2], grid_size: f64) -> Vec<Polygon2> {
        if segments.is_empty() {
            return Vec::new();
        }

        // Map quantized start points to segment indices
        let mut start_map: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (idx, seg) in segments.iter().enumerate() {
            let key = seg.p1.quantize(grid_size);
            start_map.entry(key).or_default().push(idx);
        }

        let mut visited = vec![false; segments.len()];
        let mut polygons = Vec::new();

        for i in 0..segments.len() {
            if visited[i] {
                continue;
            }

            let mut loop_pts = Vec::new();
            let mut current_idx = i;
            let mut is_closed = false;

            while !visited[current_idx] {
                visited[current_idx] = true;
                let seg = segments[current_idx];
                loop_pts.push(seg.p1);

                let next_key = seg.p2.quantize(grid_size);
                let first_key = loop_pts[0].quantize(grid_size);

                if next_key == first_key {
                    is_closed = true;
                    break;
                }

                // Look for next connecting segment
                let mut found_next = None;
                if let Some(candidates) = start_map.get(&next_key) {
                    for &cand in candidates {
                        if !visited[cand] {
                            found_next = Some(cand);
                            break;
                        }
                    }
                }

                if let Some(next) = found_next {
                    current_idx = next;
                } else {
                    break;
                }
            }

            if is_closed && loop_pts.len() >= 3 {
                let mut poly = Polygon2::new(loop_pts);
                poly.ensure_ccw();
                polygons.push(poly);
            }
        }

        polygons
    }
}
