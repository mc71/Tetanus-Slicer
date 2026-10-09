use crate::geom::{Point2, Polygon2};
use crate::slicer::ContourRole;

pub struct PerimeterGenerator;

impl PerimeterGenerator {
    /// Inset/offset a polygon by distance `d` into the solid material.
    /// For Outer contours (CCW), shifts inwards (shrinks).
    /// For Hole contours (CW), shifts outwards into solid (expands hole cavity).
    pub fn inset_polygon(poly: &Polygon2, d: f64, role: &ContourRole) -> Option<Polygon2> {
        let pts = &poly.points;
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
                // Nearly 180-degree reversal
                offset_pts.push(curr + n1 * d);
                continue;
            }

            let bisector_dir = bisector / b_len;
            let cos_half_angle = bisector_dir.dot(n1);

            // Miter must always be strictly positive and tightly clamped to avoid spikes
            let miter_len = if cos_half_angle < 0.3 {
                d
            } else {
                (d / cos_half_angle).clamp(d * 0.5, d * 1.35)
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let result = Polygon2::new(offset_pts);
        let old_area = poly.signed_area();
        let new_area = result.signed_area();

        match role {
            ContourRole::Outer => {
                if new_area > 0.0 && new_area < old_area * 0.999 {
                    Some(result)
                } else {
                    None
                }
            }
            ContourRole::Hole => {
                if new_area < 0.0 && new_area.abs() > old_area.abs() * 1.001 {
                    Some(result)
                } else {
                    None
                }
            }
        }
    }

    /// Offset an outer polygon outward (away from model) by distance `d`.
    pub fn offset_outward(poly: &Polygon2, d: f64) -> Option<Polygon2> {
        let pts = &poly.points;
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

            let miter_len = if cos_half_angle < 0.3 {
                d
            } else {
                (d / cos_half_angle).clamp(d * 0.5, d * 1.35)
            };

            let offset_pt = curr + bisector_dir * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let result = Polygon2::new(offset_pts);
        if result.signed_area() > poly.signed_area() {
            Some(result)
        } else {
            None
        }
    }

    /// Generate multiple concentric perimeter shells for a contour.
    /// The outer wall is GUARANTEED to exist (falls back to contour if inset fails).
    pub fn generate_perimeters(
        contour: &Polygon2,
        role: &ContourRole,
        perimeter_count: usize,
        line_width: f64,
    ) -> Vec<Polygon2> {
        let mut perimeters = Vec::with_capacity(perimeter_count);

        // Perimeter 0 (Outer Wall): Inset by half line width.
        // If insetting fails on a delicate/thin geometry, fall back to contour itself!
        let outer_p = match Self::inset_polygon(contour, line_width * 0.5, role) {
            Some(p) => p,
            None => contour.clone(),
        };
        perimeters.push(outer_p.clone());

        // Inner perimeters (Wall 2, 3, 4, ...):
        let mut current_boundary = outer_p;
        for _ in 1..perimeter_count {
            if let Some(inner_p) = Self::inset_polygon(&current_boundary, line_width, role) {
                // Only keep inner perimeter if it meaningfully shrinks without collapsing
                let cur_area = current_boundary.signed_area().abs();
                let inner_area = inner_p.signed_area().abs();
                if inner_area < cur_area * 0.98 {
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
