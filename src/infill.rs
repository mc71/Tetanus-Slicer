use crate::geom::{Point2, Polygon2, Segment2};

pub struct InfillGenerator;

impl InfillGenerator {
    /// Generate rectilinear infill lines for an interior polygon boundary.
    /// Alternates direction by 90 degrees on alternate layers.
    pub fn generate_rectilinear(
        boundary: &Polygon2,
        density: f64,
        line_width: f64,
        layer_index: usize,
    ) -> Vec<Segment2> {
        if density <= 0.001 || boundary.points.len() < 3 {
            return Vec::new();
        }

        let spacing = line_width / density.clamp(0.01, 1.0);
        let angle = if layer_index % 2 == 0 {
            std::f64::consts::FRAC_PI_4 // 45 deg
        } else {
            3.0 * std::f64::consts::FRAC_PI_4 // 135 deg
        };

        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Rotate boundary points so scanlines are horizontal
        let rotated_pts: Vec<Point2> = boundary
            .points
            .iter()
            .map(|p| Point2::new(p.x * cos_a + p.y * sin_a, -p.x * sin_a + p.y * cos_a))
            .collect();

        // Find Y bounds in rotated space
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for p in &rotated_pts {
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }

        let mut segments = Vec::new();
        let n = rotated_pts.len();

        let mut y = min_y + spacing * 0.5;
        while y < max_y {
            let mut x_intercepts = Vec::new();

            for i in 0..n {
                let p1 = rotated_pts[i];
                let p2 = rotated_pts[(i + 1) % n];

                if (p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y) {
                    let t = (y - p1.y) / (p2.y - p1.y);
                    let x = p1.x + t * (p2.x - p1.x);
                    x_intercepts.push(x);
                }
            }

            x_intercepts.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

            // Pair up intercepts into segments
            for chunk in x_intercepts.chunks_exact(2) {
                let x1 = chunk[0];
                let x2 = chunk[1];

                // Rotate back to original coordinate system
                let orig_p1 = Point2::new(x1 * cos_a - y * sin_a, x1 * sin_a + y * cos_a);
                let orig_p2 = Point2::new(x2 * cos_a - y * sin_a, x2 * sin_a + y * cos_a);

                segments.push(Segment2::new(orig_p1, orig_p2));
            }

            y += spacing;
        }

        segments
    }
}
