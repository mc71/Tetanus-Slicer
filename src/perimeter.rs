use clipper2_rust::core::{PointD, PathD};
use clipper2_rust::offset::{JoinType, EndType};
use clipper2_rust::clipper::inflate_paths_d;

use serde::{Deserialize, Serialize};

use crate::geom::{Point2, Polygon2};
use crate::slicer::ContourRole;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeamPosition {
    Nearest,
    Aligned,
    Rear,
    Random,
}

impl Default for SeamPosition {
    fn default() -> Self {
        Self::Aligned
    }
}

pub struct PerimeterGenerator;

impl PerimeterGenerator {
    /// Reorder a closed polygon's vertices so that vertex 0 is at the desired seam position.
    pub fn align_seam(
        poly: &Polygon2,
        seam: SeamPosition,
        current_pos: Point2,
        layer_idx: usize,
        contour_idx: usize,
    ) -> Polygon2 {
        let pts = &poly.points;
        let n = pts.len();
        if n < 3 {
            return poly.clone();
        }

        let best_idx = match seam {
            SeamPosition::Nearest => {
                let mut min_d = f64::MAX;
                let mut best = 0;
                for (i, &p) in pts.iter().enumerate() {
                    let d = current_pos.distance_to(p);
                    if d < min_d {
                        min_d = d;
                        best = i;
                    }
                }
                best
            }
            SeamPosition::Rear => {
                // Find vertex with maximum Y (towards rear of print bed)
                let mut max_y = f64::MIN;
                let mut best = 0;
                for (i, &p) in pts.iter().enumerate() {
                    if p.y > max_y {
                        max_y = p.y;
                        best = i;
                    }
                }
                best
            }
            SeamPosition::Aligned => {
                // Find the sharpest convex exterior corner (minimum dot product / sharp turn)
                let mut sharpest_dot = 1.0;
                let mut best = 0;
                for i in 0..n {
                    let prev = pts[(i + n - 1) % n];
                    let curr = pts[i];
                    let next = pts[(i + 1) % n];
                    let v1 = (curr - prev).normalized();
                    let v2 = (next - curr).normalized();
                    let cross = v1.cross(v2);
                    let dot = v1.dot(v2);

                    // For outer CCW contour, convex corner has cross > 0
                    if cross > 0.05 && dot < sharpest_dot {
                        sharpest_dot = dot;
                        best = i;
                    }
                }
                // Fallback to Rear if shape is smooth circle with no corners
                if sharpest_dot > 0.95 {
                    let mut max_y = f64::MIN;
                    for (i, &p) in pts.iter().enumerate() {
                        if p.y > max_y {
                            max_y = p.y;
                            best = i;
                        }
                    }
                }
                best
            }
            SeamPosition::Random => {
                // Pseudo-random index hashed by layer and contour index
                let hash = (layer_idx.wrapping_mul(7919) ^ contour_idx.wrapping_mul(104729)) % n;
                hash
            }
        };

        let mut reordered = Vec::with_capacity(n);
        for i in 0..n {
            reordered.push(pts[(best_idx + i) % n]);
        }
        Polygon2::new(reordered)
    }
    /// Simplify a polygon by collapsing points closer than `min_dist` and removing collinear vertices.
    pub fn simplify_polygon(poly: &Polygon2, min_dist: f64) -> Polygon2 {
        let pts = &poly.points;
        let n = pts.len();
        if n < 3 {
            return poly.clone();
        }

        let mut clean = Vec::with_capacity(n);
        let mut last = pts[0];
        clean.push(last);

        for i in 1..n {
            if pts[i].distance_to(last) >= min_dist {
                clean.push(pts[i]);
                last = pts[i];
            }
        }

        if clean.len() > 2 && clean.last().unwrap().distance_to(clean[0]) < min_dist {
            clean.pop();
        }

        let m = clean.len();
        if m < 3 {
            return Polygon2::new(clean);
        }

        let mut result = Vec::with_capacity(m);
        for i in 0..m {
            let prev = clean[(i + m - 1) % m];
            let curr = clean[i];
            let next = clean[(i + 1) % m];
            let v1 = (curr - prev).normalized();
            let v2 = (next - curr).normalized();
            if (v1.cross(v2)).abs() > 1e-3 || v1.dot(v2) < 0.9995 {
                result.push(curr);
            }
        }

        if result.len() < 3 {
            Polygon2::new(clean)
        } else {
            Polygon2::new(result)
        }
    }

    /// Inset/offset a polygon by distance `d` into the solid material using Clipper2 with Round (arc) joins.
    /// For Outer contours (CCW), shifts inwards (shrinks by -d).
    /// For Hole contours (CW), shifts outwards into solid (expands hole cavity by +d).
    pub fn inset_polygon(poly: &Polygon2, d: f64, role: &ContourRole) -> Vec<Polygon2> {
        let simplified = Self::simplify_polygon(poly, 0.02);
        if simplified.points.len() < 3 {
            return Vec::new();
        }

        let path: PathD = simplified.points.iter().map(|p| PointD { x: p.x, y: p.y }).collect();
        let delta = match role {
            ContourRole::Outer => -d,
            ContourRole::Hole => d,
        };

        // Round join with arc_tolerance = 0.015 produces smooth arcs around corners without wavy oscillation
        let paths = inflate_paths_d(&vec![path], delta, JoinType::Round, EndType::Polygon, 2.0, 4, 0.015);
        paths
            .into_iter()
            .filter_map(|p| {
                if p.len() >= 3 {
                    let pts: Vec<Point2> = p.into_iter().map(|pt| Point2::new(pt.x, pt.y)).collect();
                    let res = Polygon2::new(pts);
                    let area = res.signed_area();
                    match role {
                        ContourRole::Outer if area > 0.0 => Some(res),
                        ContourRole::Hole if area < 0.0 => Some(res),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Offset an outer polygon outward (away from model) by distance `d` with Round arc joins.
    pub fn offset_outward(poly: &Polygon2, d: f64) -> Vec<Polygon2> {
        let simplified = Self::simplify_polygon(poly, 0.02);
        if simplified.points.len() < 3 {
            return Vec::new();
        }

        let path: PathD = simplified.points.iter().map(|p| PointD { x: p.x, y: p.y }).collect();
        let paths = inflate_paths_d(&vec![path], d, JoinType::Round, EndType::Polygon, 2.0, 4, 0.015);
        paths
            .into_iter()
            .filter_map(|p| {
                if p.len() >= 3 {
                    let pts: Vec<Point2> = p.into_iter().map(|pt| Point2::new(pt.x, pt.y)).collect();
                    let res = Polygon2::new(pts);
                    if res.signed_area() > 0.0 {
                        Some(res)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Generate multiple concentric perimeter shells for a contour.
    pub fn generate_perimeters(
        contour: &Polygon2,
        role: &ContourRole,
        perimeter_count: usize,
        line_width: f64,
    ) -> Vec<Polygon2> {
        let mut perimeters = Vec::with_capacity(perimeter_count);

        // Perimeter 0 (Outer Wall): Inset by half line width with Round arc joins.
        let outer_walls = Self::inset_polygon(contour, line_width * 0.5, role);
        let first_wall = if let Some(w) = outer_walls.into_iter().max_by(|a, b| a.signed_area().abs().partial_cmp(&b.signed_area().abs()).unwrap()) {
            w
        } else {
            Self::simplify_polygon(contour, 0.02)
        };
        perimeters.push(first_wall.clone());

        // Inner perimeters (Wall 1, 2, ...):
        let mut current_boundary = first_wall;
        for _ in 1..perimeter_count {
            let insets = Self::inset_polygon(&current_boundary, line_width, role);
            if let Some(inner) = insets.into_iter().max_by(|a, b| a.signed_area().abs().partial_cmp(&b.signed_area().abs()).unwrap()) {
                let cur_area = current_boundary.signed_area().abs();
                let inner_area = inner.signed_area().abs();
                if inner_area < cur_area * 0.95 && inner.points.len() >= 3 {
                    perimeters.push(inner.clone());
                    current_boundary = inner;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        perimeters
    }

    /// Generate brim loops attached directly to the outer contours on layer 0.
    pub fn generate_brim(
        outer_contours: &[Polygon2],
        brim_width: f64,
        line_width: f64,
    ) -> Vec<Polygon2> {
        if brim_width <= 0.001 {
            return Vec::new();
        }
        let loop_count = (brim_width / line_width).ceil() as usize;
        let mut brim_loops = Vec::new();

        for contour in outer_contours {
            for i in 1..=loop_count {
                let dist = i as f64 * line_width;
                brim_loops.extend(Self::offset_outward(contour, dist));
            }
        }

        brim_loops
    }

    /// Generate skirt loops offset at a distance from the outer contours on layer 0.
    pub fn generate_skirt(
        outer_contours: &[Polygon2],
        skirt_loops: usize,
        skirt_distance: f64,
        line_width: f64,
    ) -> Vec<Polygon2> {
        if skirt_loops == 0 {
            return Vec::new();
        }
        let mut skirt = Vec::new();

        for contour in outer_contours {
            for i in 0..skirt_loops {
                let dist = skirt_distance + (i as f64 * line_width);
                skirt.extend(Self::offset_outward(contour, dist));
            }
        }

        skirt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stl::Mesh;
    use crate::slicer::Slicer;

    #[test]
    fn test_benchy_mid_perimeters() {
        if let Ok(mesh) = Mesh::load_stl("samples/3DBenchy.stl") {
            let bvh = crate::bvh::TriangleIntervalIndex::build(&mesh.triangles);
            // Mid-way down Benchy is around Z = 16.0mm
            let z = 16.0;
            let intersecting = bvh.query_z(z);
            let mut segments = Vec::new();
            for tri in intersecting {
                if let Some(seg) = Slicer::intersect_triangle(tri, z) {
                    segments.push(seg);
                }
            }
            let contours = Slicer::chain_segments(&segments);
            println!("At z={}, contours count: {}", z, contours.len());
            for (c_idx, c) in contours.iter().enumerate() {
                let perims = PerimeterGenerator::generate_perimeters(&c.polygon, &c.role, 2, 0.4);
                println!("Contour {} role={:?} generated {} perimeters", c_idx, c.role, perims.len());
            }

            // Test Clipper2 inflate_paths_d
            use clipper2_rust::core::{PointD, PathD};
            use clipper2_rust::offset::{JoinType, EndType};
            use clipper2_rust::clipper::inflate_paths_d;

            for (c_idx, c) in contours.iter().enumerate() {
                let path: PathD = c.polygon.points.iter().map(|p| PointD { x: p.x, y: p.y }).collect();
                let delta = match c.role {
                    ContourRole::Outer => -0.2, // half line width
                    ContourRole::Hole => 0.2,
                };
                let res = inflate_paths_d(&vec![path], delta, JoinType::Round, EndType::Polygon, 2.0, 4, 0.01);
                println!("Clipper2 Contour {} -> produced {} paths (pts: {:?})", c_idx, res.len(), res.iter().map(|p| p.len()).collect::<Vec<_>>());
            }
        }
    }
}
