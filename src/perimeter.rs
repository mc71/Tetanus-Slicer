use crate::geom::{Point2, Polygon2};

pub struct PerimeterGenerator;

impl PerimeterGenerator {
    /// Inset a polygon by a distance `d` (positive `d` moves boundaries inwards).
    /// Returns the inset polygon if valid.
    pub fn inset_polygon(poly: &Polygon2, d: f64) -> Option<Polygon2> {
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

            // Inward normals for CCW polygon
            let n1 = Point2::new(-e1.y, e1.x);
            let n2 = Point2::new(-e2.y, e2.x);

            // Average bisector normal
            let bisector = (n1 + n2).normalized();
            let cos_half_angle = bisector.dot(n1);

            if cos_half_angle.abs() < 1e-4 {
                continue;
            }

            // Miter length clamped to avoid extreme spikes at sharp corners
            let miter_len = (d / cos_half_angle).clamp(-3.0 * d, 3.0 * d);
            let offset_pt = curr + bisector * miter_len;
            offset_pts.push(offset_pt);
        }

        if offset_pts.len() < 3 {
            return None;
        }

        let result = Polygon2::new(offset_pts);
        // Verify polygon hasn't inverted or degenerated
        if result.signed_area() > 0.0 && result.signed_area() < poly.signed_area() {
            Some(result)
        } else {
            None
        }
    }

    /// Generate multiple concentric perimeter shells for a contour.
    pub fn generate_perimeters(contour: &Polygon2, perimeter_count: usize, line_width: f64) -> Vec<Polygon2> {
        let mut perimeters = Vec::with_capacity(perimeter_count);
        let mut current_boundary = contour.clone();

        // First perimeter: inset by half line_width (centerline of the outer bead)
        if let Some(outer_p) = Self::inset_polygon(&current_boundary, line_width * 0.5) {
            perimeters.push(outer_p.clone());
            current_boundary = outer_p;

            // Inner perimeters: inset each subsequent wall by a full line_width
            for _ in 1..perimeter_count {
                if let Some(inner_p) = Self::inset_polygon(&current_boundary, line_width) {
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
