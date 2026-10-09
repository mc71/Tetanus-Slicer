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

            let e1 = (curr - prev).normalized();
            let e2 = (next - curr).normalized();

            // Left normal of edge
            let n1 = Point2::new(-e1.y, e1.x);
            let n2 = Point2::new(-e2.y, e2.x);

            let bisector = (n1 + n2).normalized();
            let cos_half_angle = bisector.dot(n1);

            if cos_half_angle.abs() < 1e-4 {
                continue;
            }

            let miter_len = (d / cos_half_angle).clamp(-2.5 * d, 2.5 * d);
            let offset_pt = curr + bisector * miter_len;
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
                if new_area > 0.0 && new_area < old_area {
                    Some(result)
                } else {
                    None
                }
            }
            ContourRole::Hole => {
                if new_area < 0.0 && new_area.abs() > old_area.abs() {
                    Some(result)
                } else {
                    None
                }
            }
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
                    break;
                }
            }
        }

        perimeters
    }
}
