#![allow(dead_code)]

use std::collections::HashMap;
use crate::geom::{Point2, Polygon2, Segment2, Triangle};

#[derive(Clone, Debug, PartialEq)]
pub enum ContourRole {
    Outer,
    Hole,
}

#[derive(Clone, Debug)]
pub struct ClassifiedContour {
    pub polygon: Polygon2,
    pub role: ContourRole,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub layer_index: usize,
    pub z: f64,
    pub contours: Vec<ClassifiedContour>,
}

pub struct Slicer;

impl Slicer {
    /// Intersect a single triangle with a horizontal plane at height Z.
    /// Returns an unoriented line segment if an intersection exists.
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
                    if !pts.iter().any(|p: &Point2| p.distance_to(pt2) < 1e-5) {
                        pts.push(pt2);
                    }
                }
            }
        }

        if pts.len() == 2 {
            let p1 = pts[0];
            let p2 = pts[1];
            if p1.distance_to(p2) > 1e-5 {
                Some(Segment2::new(p1, p2))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Robust bidirectional segment chaining with spatial neighbor lookup.
    /// Traverses segments in either direction to handle flipped normals and non-manifold edges.
    pub fn chain_segments(segments: &[Segment2]) -> Vec<ClassifiedContour> {
        if segments.is_empty() {
            return Vec::new();
        }

        let cell_size = 0.05; // 50 microns spatial grid
        let connect_tol = 0.08; // 80 microns connection tolerance
        let close_tol = 0.50; // 500 microns gap closure for closing loops

        // Map grid cell -> list of (segment_index, end_index: 0=p1, 1=p2)
        let mut grid: HashMap<(i64, i64), Vec<(usize, u8)>> = HashMap::new();

        for (idx, seg) in segments.iter().enumerate() {
            let k1 = (
                (seg.p1.x / cell_size).floor() as i64,
                (seg.p1.y / cell_size).floor() as i64,
            );
            grid.entry(k1).or_default().push((idx, 0));

            let k2 = (
                (seg.p2.x / cell_size).floor() as i64,
                (seg.p2.y / cell_size).floor() as i64,
            );
            grid.entry(k2).or_default().push((idx, 1));
        }

        let mut visited = vec![false; segments.len()];
        let mut raw_polygons: Vec<Polygon2> = Vec::new();

        for i in 0..segments.len() {
            if visited[i] {
                continue;
            }

            visited[i] = true;
            let mut loop_pts = vec![segments[i].p1, segments[i].p2];
            let mut current_pt = segments[i].p2;

            loop {
                // Check if we can close the loop with the start point
                if loop_pts.len() >= 3 && current_pt.distance_to(loop_pts[0]) <= close_tol {
                    break;
                }

                // Look for the closest unvisited segment endpoint in 3x3 neighboring cells
                let cx = (current_pt.x / cell_size).floor() as i64;
                let cy = (current_pt.y / cell_size).floor() as i64;

                let mut best_match: Option<(usize, u8, f64)> = None;

                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if let Some(candidates) = grid.get(&(cx + dx, cy + dy)) {
                            for &(seg_idx, end_idx) in candidates {
                                if !visited[seg_idx] {
                                    let pt = if end_idx == 0 {
                                        segments[seg_idx].p1
                                    } else {
                                        segments[seg_idx].p2
                                    };
                                    let dist = current_pt.distance_to(pt);
                                    if dist <= connect_tol {
                                        if let Some((_, _, best_d)) = best_match {
                                            if dist < best_d {
                                                best_match = Some((seg_idx, end_idx, dist));
                                            }
                                        } else {
                                            best_match = Some((seg_idx, end_idx, dist));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some((next_seg_idx, next_end_idx, _)) = best_match {
                    visited[next_seg_idx] = true;
                    let next_seg = segments[next_seg_idx];
                    let next_pt = if next_end_idx == 0 {
                        // Connected at p1 -> traverse forward to p2
                        next_seg.p2
                    } else {
                        // Connected at p2 -> traverse backward to p1
                        next_seg.p1
                    };
                    loop_pts.push(next_pt);
                    current_pt = next_pt;
                } else {
                    // No connected segment found; check if close enough to start point to bridge
                    if loop_pts.len() >= 3 && current_pt.distance_to(loop_pts[0]) <= 1.0 {
                        // Bridge gap and close loop
                    }
                    break;
                }
            }

            if loop_pts.len() >= 3 {
                let poly = Polygon2::new(loop_pts);
                // Filter out zero-area degeneracies
                if poly.signed_area().abs() > 0.05 {
                    raw_polygons.push(poly);
                }
            }
        }

        // Classify each polygon as Outer Boundary vs Hole using Even-Odd Nesting
        let n = raw_polygons.len();
        let mut classified = Vec::with_capacity(n);

        for i in 0..n {
            let mut container_count = 0;
            let sample_pt = raw_polygons[i].points[0];

            for j in 0..n {
                if i != j && raw_polygons[j].contains_point(sample_pt) {
                    container_count += 1;
                }
            }

            let mut poly = raw_polygons[i].clone();
            if container_count % 2 == 0 {
                // Outer boundary: should be CCW
                if poly.signed_area() < 0.0 {
                    poly.points.reverse();
                }
                classified.push(ClassifiedContour {
                    polygon: poly,
                    role: ContourRole::Outer,
                });
            } else {
                // Hole: should be CW
                if poly.signed_area() > 0.0 {
                    poly.points.reverse();
                }
                classified.push(ClassifiedContour {
                    polygon: poly,
                    role: ContourRole::Hole,
                });
            }
        }

        classified
    }
}
