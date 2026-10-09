use crate::geom::{Point2, Polygon2};
use crate::slicer::ContourRole;

pub struct PerimeterGenerator;

impl PerimeterGenerator {
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
            // Keep vertex if not collinear (turn angle > 0.5 degrees)
            if (v1.cross(v2)).abs() > 1e-3 || v1.dot(v2) < 0.999 {
                result.push(curr);
            }
        }

        if result.len() < 3 {
            Polygon2::new(clean)
        } else {
            Polygon2::new(result)
        }
    }

    /// Calculate intersection point between two line segments (p1..p2) and (p3..p4).
    pub fn segment_intersection(p1: Point2, p2: Point2, p3: Point2, p4: Point2) -> Option<Point2> {
        let d = (p1.x - p2.x) * (p3.y - p4.y) - (p1.y - p2.y) * (p3.x - p4.x);
        if d.abs() < 1e-9 {
            return None;
        }

        let t = ((p1.x - p3.x) * (p3.y - p4.y) - (p1.y - p3.y) * (p3.x - p4.x)) / d;
        let u = -((p1.x - p2.x) * (p1.y - p3.y) - (p1.y - p2.y) * (p1.x - p3.x)) / d;

        if t > 1e-4 && t < 1.0 - 1e-4 && u > 1e-4 && u < 1.0 - 1e-4 {
            Some(Point2::new(
                p1.x + t * (p2.x - p1.x),
                p1.y + t * (p2.y - p1.y),
            ))
        } else {
            None
        }
    }

    /// Untangle self-intersecting loops ("ears" / bowties / swallowtails) in an offset polygon.
    pub fn untangle_polygon(poly: &Polygon2) -> Polygon2 {
        let mut pts = poly.points.clone();
        let mut changed = true;
        let mut passes = 0;

        while changed && passes < 16 {
            changed = false;
            passes += 1;
            let n = pts.len();
            if n < 3 {
                break;
            }

            'outer: for i in 0..n {
                let a1 = pts[i];
                let a2 = pts[(i + 1) % n];

                for j in (i + 2)..n {
                    if i == 0 && j == n - 1 {
                        continue;
                    }
                    let b1 = pts[j];
                    let b2 = pts[(j + 1) % n];

                    if let Some(int_pt) = Self::segment_intersection(a1, a2, b1, b2) {
                        let mut new_pts = Vec::with_capacity(n);
                        for k in 0..=i {
                            new_pts.push(pts[k]);
                        }
                        new_pts.push(int_pt);
                        for k in (j + 1)..n {
                            new_pts.push(pts[k]);
                        }

                        let mut cut_pts = Vec::with_capacity(j - i + 1);
                        cut_pts.push(int_pt);
                        for k in (i + 1)..=j {
                            cut_pts.push(pts[k]);
                        }

                        let p_main = Polygon2::new(new_pts);
                        let p_cut = Polygon2::new(cut_pts);

                        // Keep the polygon with larger absolute area (main body, rejecting the ear)
                        if p_main.signed_area().abs() >= p_cut.signed_area().abs() && p_main.points.len() >= 3 {
                            pts = p_main.points;
                        } else if p_cut.points.len() >= 3 {
                            pts = p_cut.points;
                        }
                        changed = true;
                        break 'outer;
                    }
                }
            }
        }
        Polygon2::new(pts)
    }

    /// Inset/offset a polygon by distance `d` into the solid material.
    /// For Outer contours (CCW), shifts inwards (shrinks).
    /// For Hole contours (CW), shifts outwards into solid (expands hole cavity).
    pub fn inset_polygon(poly: &Polygon2, d: f64, role: &ContourRole) -> Option<Polygon2> {
        let simplified = Self::simplify_polygon(poly, 0.04);
        let pts = &simplified.points;
        let n = pts.len();
        if n < 3 {
            return None;
        }

        let mut offset_pts = Vec::with_capacity(n);

        for i in 0..n {
            let prev = pts[(i + n - 1) % n];
            let curr = pts[i];
            let next = pts[(i + 1) % n];

            let v1 = curr - prev;
            let v2 = next - curr;
            let l1 = v1.length();
            let l2 = v2.length();
            if l1 < 1e-6 || l2 < 1e-6 {
                continue;
            }

            let e1 = v1 / l1;
            let e2 = v2 / l2;

            // Inward normal for CCW Outer: (-e.y, e.x)
            // Inward normal for CW Hole: (e.y, -e.x)
            let (n1, n2) = match role {
                ContourRole::Outer => (
                    Point2::new(-e1.y, e1.x),
                    Point2::new(-e2.y, e2.x),
                ),
                ContourRole::Hole => (
                    Point2::new(e1.y, -e1.x),
                    Point2::new(e2.y, -e2.x),
                ),
            };

            let bisector = n1 + n2;
            let b_len = bisector.length();
            if b_len < 1e-3 {
                offset_pts.push(curr + n1 * d);
                continue;
            }

            let bisector_dir = bisector / b_len;
            let cos_half_angle = bisector_dir.dot(n1);

            // Miter is constrained by neighbor edge lengths so vertex never overshoots adjacent edges
            let max_miter = (l1.min(l2) * 0.75).max(d * 0.4);
            let miter_len = if cos_half_angle < 0.25 {
                d
            } else {
                (d / cos_half_angle).clamp(d * 0.4, (d * 1.4).min(max_miter))
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let raw_result = Polygon2::new(offset_pts);
        let untangled = Self::untangle_polygon(&raw_result);
        if untangled.points.len() < 3 {
            return None;
        }

        let old_area = poly.signed_area();
        let new_area = untangled.signed_area();

        match role {
            ContourRole::Outer => {
                if new_area > 0.0 && new_area < old_area * 0.999 {
                    Some(untangled)
                } else {
                    None
                }
            }
            ContourRole::Hole => {
                if new_area < 0.0 && new_area.abs() > old_area.abs() * 1.001 {
                    Some(untangled)
                } else {
                    None
                }
            }
        }
    }

    /// Offset an outer polygon outward (away from model) by distance `d`.
    pub fn offset_outward(poly: &Polygon2, d: f64) -> Option<Polygon2> {
        let simplified = Self::simplify_polygon(poly, 0.04);
        let pts = &simplified.points;
        let n = pts.len();
        if n < 3 {
            return None;
        }

        let mut offset_pts = Vec::with_capacity(n);

        for i in 0..n {
            let prev = pts[(i + n - 1) % n];
            let curr = pts[i];
            let next = pts[(i + 1) % n];

            let v1 = curr - prev;
            let v2 = next - curr;
            let l1 = v1.length();
            let l2 = v2.length();
            if l1 < 1e-6 || l2 < 1e-6 {
                continue;
            }

            let e1 = v1 / l1;
            let e2 = v2 / l2;

            // Outward normal for CCW: (e.y, -e.x)
            let n1 = Point2::new(e1.y, -e1.x);
            let n2 = Point2::new(e2.y, -e2.x);

            let bisector = n1 + n2;
            let b_len = bisector.length();
            if b_len < 1e-3 {
                offset_pts.push(curr + n1 * d);
                continue;
            }

            let bisector_dir = bisector / b_len;
            let cos_half_angle = bisector_dir.dot(n1);

            let max_miter = (l1.min(l2) * 0.75).max(d * 0.4);
            let miter_len = if cos_half_angle < 0.25 {
                d
            } else {
                (d / cos_half_angle).clamp(d * 0.4, (d * 1.4).min(max_miter))
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let raw_result = Polygon2::new(offset_pts);
        let untangled = Self::untangle_polygon(&raw_result);
        if untangled.signed_area() > poly.signed_area() && untangled.points.len() >= 3 {
            Some(untangled)
        } else {
            None
        }
    }

    /// Generate multiple concentric perimeter shells for a contour.
    /// The outer wall is GUARANTEED to exist (falls back to simplified contour if inset fails).
    pub fn generate_perimeters(
        contour: &Polygon2,
        role: &ContourRole,
        perimeter_count: usize,
        line_width: f64,
    ) -> Vec<Polygon2> {
        let mut perimeters = Vec::with_capacity(perimeter_count);

        // Perimeter 0 (Outer Wall): Inset by half line width.
        let simple_contour = Self::simplify_polygon(contour, 0.04);
        let outer_p = match Self::inset_polygon(&simple_contour, line_width * 0.5, role) {
            Some(p) => p,
            None => simple_contour.clone(),
        };
        perimeters.push(outer_p.clone());

        // Inner perimeters (Wall 2, 3, 4, ...):
        let mut current_boundary = outer_p;
        for _ in 1..perimeter_count {
            if let Some(inner_p) = Self::inset_polygon(&current_boundary, line_width, role) {
                let cur_area = current_boundary.signed_area().abs();
                let inner_area = inner_p.signed_area().abs();
                if inner_area < cur_area * 0.95 {
                    perimeters.push(inner_p.clone());
                    current_boundary = inner_p;
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
                if let Some(b) = Self::offset_outward(contour, dist) {
                    brim_loops.push(b);
                }
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
                if let Some(s) = Self::offset_outward(contour, dist) {
                    skirt.push(s);
                }
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
                for (p_idx, p) in perims.iter().enumerate() {
                    let mut self_intersections = 0;
                    let n = p.points.len();
                    for i in 0..n {
                        let a1 = p.points[i];
                        let a2 = p.points[(i + 1) % n];
                        for j in (i + 2)..n {
                            if i == 0 && j == n - 1 { continue; }
                            let b1 = p.points[j];
                            let b2 = p.points[(j + 1) % n];
                            if PerimeterGenerator::segment_intersection(a1, a2, b1, b2).is_some() {
                                self_intersections += 1;
                            }
                        }
                    }
                    println!("Contour {} Perim {}: {} self-intersections! (pts={})", c_idx, p_idx, self_intersections, n);
                    assert_eq!(self_intersections, 0, "Perimeter should have 0 self-intersections");
                }
            }
        }
    }
}
