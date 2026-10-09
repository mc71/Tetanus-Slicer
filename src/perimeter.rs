use crate::geom::{Point2, Polygon2};
use crate::slicer::ContourRole;

pub struct PerimeterGenerator;

impl PerimeterGenerator {
    /// Inset/offset a polygon by distance `d` into the solid material.
    /// For Outer contours (CCW), shifts inwards (shrinks).
    /// For Hole contours (CW), shifts outwards into solid (expands hole cavity).
    pub fn inset_polygon(poly: &Polygon2, d: f64, role: &ContourRole) -> Option<Polygon2> {
        let simplified = simplify_polygon(poly, 0.05);
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
            if l1 < 1e-5 || l2 < 1e-5 {
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
                // Nearly 180-degree reversal
                offset_pts.push(curr + n1 * d);
                continue;
            }

            let bisector_dir = bisector / b_len;
            let cos_half_angle = bisector_dir.dot(n1);

            // Never allow negative or oversized miter lengths that cause spikes
            let miter_len = if cos_half_angle < 0.2 {
                d // Cap sharp/reflex corners to avoid projecting wild spikes
            } else {
                (d / cos_half_angle).clamp(d * 0.5, d * 1.5)
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let result = simplify_polygon(&Polygon2::new(offset_pts), 0.05);
        if result.points.len() < 3 {
            return None;
        }

        let old_area = poly.signed_area();
        let new_area = result.signed_area();

        match role {
            ContourRole::Outer => {
                // Outer must remain CCW and area must strictly decrease
                if new_area <= 0.0 || new_area >= old_area * 0.999 {
                    return None;
                }
                // Discard if loop has self-intersections (walls collided)
                if has_self_intersections(&result) {
                    return None;
                }
                Some(result)
            }
            ContourRole::Hole => {
                // Hole must remain CW and absolute area must expand into solid
                if new_area >= 0.0 || new_area.abs() <= old_area.abs() * 0.999 {
                    return None;
                }
                if has_self_intersections(&result) {
                    return None;
                }
                Some(result)
            }
        }
    }

    /// Offset an outer polygon outward (away from model) by distance `d`.
    pub fn offset_outward(poly: &Polygon2, d: f64) -> Option<Polygon2> {
        let simplified = simplify_polygon(poly, 0.05);
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
            if l1 < 1e-5 || l2 < 1e-5 {
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

            let miter_len = if cos_half_angle < 0.2 {
                d
            } else {
                (d / cos_half_angle).clamp(d * 0.5, d * 1.5)
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let result = simplify_polygon(&Polygon2::new(offset_pts), 0.05);
        if result.points.len() < 3 {
            return None;
        }

        if result.signed_area() > poly.signed_area() {
            Some(result)
        } else {
            None
        }
    }

    /// Generate multiple concentric perimeter shells for a contour.
    pub fn generate_perimeters(
        contour: &Polygon2,
        role: &ContourRole,
        perimeter_count: usize,
        line_width: f64,
    ) -> Vec<Polygon2> {
        let mut perimeters = Vec::with_capacity(perimeter_count);
        let mut current_boundary = contour.clone();

        // First perimeter: centerline of the outer bead
        if let Some(outer_p) = Self::inset_polygon(&current_boundary, line_width * 0.5, role) {
            perimeters.push(outer_p.clone());
            current_boundary = outer_p;

            // Inner perimeters: inset each subsequent wall by full line_width
            for _ in 1..perimeter_count {
                if let Some(inner_p) = Self::inset_polygon(&current_boundary, line_width, role) {
                    perimeters.push(inner_p.clone());
                    current_boundary = inner_p;
                } else {
                    // Wall collapsed (too thin to fit more perimeters without self-intersection)
                    break;
                }
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

/// Simplify polygon by removing redundant vertices closer than `min_dist`
fn simplify_polygon(poly: &Polygon2, min_dist: f64) -> Polygon2 {
    let n = poly.points.len();
    if n < 3 {
        return poly.clone();
    }
    let mut pts = Vec::with_capacity(n);
    pts.push(poly.points[0]);

    for i in 1..n {
        if poly.points[i].distance_to(*pts.last().unwrap()) >= min_dist {
            pts.push(poly.points[i]);
        }
    }
    if pts.len() > 2 && pts.last().unwrap().distance_to(pts[0]) < min_dist {
        pts.pop();
    }
    if pts.len() < 3 {
        poly.clone()
    } else {
        Polygon2::new(pts)
    }
}

/// Check if non-adjacent segments of a polygon cross over each other
fn has_self_intersections(poly: &Polygon2) -> bool {
    let n = poly.points.len();
    if n < 4 {
        return false;
    }
    for i in 0..n {
        let p1 = poly.points[i];
        let p2 = poly.points[(i + 1) % n];
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let p3 = poly.points[j];
            let p4 = poly.points[(j + 1) % n];
            if segments_intersect(p1, p2, p3, p4) {
                return true;
            }
        }
    }
    false
}

/// Test 2D segment intersection
fn segments_intersect(a1: Point2, a2: Point2, b1: Point2, b2: Point2) -> bool {
    let ccw = |p1: Point2, p2: Point2, p3: Point2| -> f64 {
        (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
    };
    let d1 = ccw(a1, a2, b1);
    let d2 = ccw(a1, a2, b2);
    let d3 = ccw(b1, b2, a1);
    let d4 = ccw(b1, b2, a2);

    if ((d1 > 1e-6 && d2 < -1e-6) || (d1 < -1e-6 && d2 > 1e-6))
        && ((d3 > 1e-6 && d4 < -1e-6) || (d3 < -1e-6 && d4 > 1e-6))
    {
        return true;
    }
    false
}
