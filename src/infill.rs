use serde::{Deserialize, Serialize};
use crate::geom::{Point2, Polygon2, Segment2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InfillPattern {
    Rectilinear,
    Grid,
    Triangles,
    Gyroid,
}

impl Default for InfillPattern {
    fn default() -> Self {
        InfillPattern::Rectilinear
    }
}

pub struct InfillGenerator;

impl InfillGenerator {
    /// Generate infill paths based on the requested pattern.
    pub fn generate_infill(
        pattern: InfillPattern,
        boundaries: &[Polygon2],
        density: f64,
        line_width: f64,
        layer_index: usize,
        z: f64,
    ) -> Vec<Segment2> {
        if density <= 0.001 || boundaries.is_empty() {
            return Vec::new();
        }

        match pattern {
            InfillPattern::Rectilinear => {
                Self::generate_rectilinear(boundaries, density, line_width, layer_index)
            }
            InfillPattern::Grid => {
                let spacing = line_width / (density.clamp(0.01, 1.0) * 0.5);
                let mut segments = Self::generate_scanlines(boundaries, spacing, std::f64::consts::FRAC_PI_4);
                segments.extend(Self::generate_scanlines(
                    boundaries,
                    spacing,
                    3.0 * std::f64::consts::FRAC_PI_4,
                ));
                segments
            }
            InfillPattern::Triangles => {
                let spacing = line_width / (density.clamp(0.01, 1.0) / 3.0);
                let mut segments = Self::generate_scanlines(boundaries, spacing, 0.0);
                segments.extend(Self::generate_scanlines(
                    boundaries,
                    spacing,
                    std::f64::consts::PI / 3.0,
                ));
                segments.extend(Self::generate_scanlines(
                    boundaries,
                    spacing,
                    2.0 * std::f64::consts::PI / 3.0,
                ));
                segments
            }
            InfillPattern::Gyroid => {
                Self::generate_gyroid(boundaries, density, line_width, z)
            }
        }
    }

    /// Generate rectilinear infill lines across all boundaries on a layer.
    /// Uses Jordan curve even-odd scanline pairing to automatically hollow out holes.
    /// Alternates direction by 90 degrees on alternate layers.
    pub fn generate_rectilinear(
        boundaries: &[Polygon2],
        density: f64,
        line_width: f64,
        layer_index: usize,
    ) -> Vec<Segment2> {
        if density <= 0.001 || boundaries.is_empty() {
            return Vec::new();
        }

        let spacing = line_width / density.clamp(0.01, 1.0);
        let angle = if layer_index % 2 == 0 {
            std::f64::consts::FRAC_PI_4 // 45 deg
        } else {
            3.0 * std::f64::consts::FRAC_PI_4 // 135 deg
        };

        Self::generate_scanlines(boundaries, spacing, angle)
    }

    /// Intersect arbitrary angle scanlines with all boundaries, using even-odd pairing
    pub fn generate_scanlines(
        boundaries: &[Polygon2],
        spacing: f64,
        angle: f64,
    ) -> Vec<Segment2> {
        if boundaries.is_empty() || spacing <= 1e-4 {
            return Vec::new();
        }

        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Rotate all boundary polygons so scanlines are horizontal
        let mut rotated_boundaries: Vec<Vec<Point2>> = Vec::with_capacity(boundaries.len());
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for boundary in boundaries {
            if boundary.points.len() < 3 {
                continue;
            }
            let rot_pts: Vec<Point2> = boundary
                .points
                .iter()
                .map(|p| Point2::new(p.x * cos_a + p.y * sin_a, -p.x * sin_a + p.y * cos_a))
                .collect();

            for p in &rot_pts {
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
            rotated_boundaries.push(rot_pts);
        }

        if rotated_boundaries.is_empty() || min_y >= max_y {
            return Vec::new();
        }

        let mut segments = Vec::new();
        let mut y = min_y + spacing * 0.5;

        while y < max_y {
            let mut x_intercepts = Vec::new();

            // Intersect horizontal line Y = y with edges of ALL boundaries
            for poly in &rotated_boundaries {
                let n = poly.len();
                for i in 0..n {
                    let p1 = poly[i];
                    let p2 = poly[(i + 1) % n];

                    if (p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y) {
                        let t = (y - p1.y) / (p2.y - p1.y);
                        let x = p1.x + t * (p2.x - p1.x);
                        x_intercepts.push(x);
                    }
                }
            }

            // Sort intercepts from left to right
            x_intercepts.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Even-Odd rule: pairs [0,1], [2,3] are inside solid; gaps [1,2], [3,4] are holes or air
            for chunk in x_intercepts.chunks_exact(2) {
                let x1 = chunk[0];
                let x2 = chunk[1];

                // Rotate back to original coordinate system
                let orig_p1 = Point2::new(x1 * cos_a - y * sin_a, x1 * sin_a + y * cos_a);
                let orig_p2 = Point2::new(x2 * cos_a - y * sin_a, x2 * sin_a + y * cos_a);

                if orig_p1.distance_to(orig_p2) > 1e-4 {
                    segments.push(Segment2::new(orig_p1, orig_p2));
                }
            }

            y += spacing;
        }

        segments
    }

    /// Generate continuous 3D Gyroid infill curves across boundaries
    pub fn generate_gyroid(
        boundaries: &[Polygon2],
        density: f64,
        line_width: f64,
        z: f64,
    ) -> Vec<Segment2> {
        let spacing = line_width / density.clamp(0.01, 1.0);
        let wavelength = (spacing * 1.5).max(1.0);
        let amplitude = spacing * 0.35;
        let phase_z = (2.0 * std::f64::consts::PI * z) / wavelength;
        let angle = std::f64::consts::FRAC_PI_4; // 45 deg base angle
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let mut rotated_boundaries: Vec<Vec<Point2>> = Vec::with_capacity(boundaries.len());
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for boundary in boundaries {
            if boundary.points.len() < 3 {
                continue;
            }
            let rot_pts: Vec<Point2> = boundary
                .points
                .iter()
                .map(|p| Point2::new(p.x * cos_a + p.y * sin_a, -p.x * sin_a + p.y * cos_a))
                .collect();

            for p in &rot_pts {
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
            rotated_boundaries.push(rot_pts);
        }

        if rotated_boundaries.is_empty() || min_y >= max_y {
            return Vec::new();
        }

        let mut segments = Vec::new();
        let mut y = min_y + spacing * 0.5;
        let mut line_idx = 0usize;

        while y < max_y {
            let mut x_intercepts = Vec::new();

            for poly in &rotated_boundaries {
                let n = poly.len();
                for i in 0..n {
                    let p1 = poly[i];
                    let p2 = poly[(i + 1) % n];

                    if (p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y) {
                        let t = (y - p1.y) / (p2.y - p1.y);
                        let x = p1.x + t * (p2.x - p1.x);
                        x_intercepts.push(x);
                    }
                }
            }

            x_intercepts.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let line_phase = if line_idx % 2 == 0 { 0.0 } else { std::f64::consts::PI };

            for chunk in x_intercepts.chunks_exact(2) {
                let x1 = chunk[0];
                let x2 = chunk[1];
                let seg_len = x2 - x1;
                if seg_len <= 1e-4 {
                    continue;
                }

                let step_size = 0.8; // mm sample step for smooth curve
                let steps = (seg_len / step_size).ceil().max(2.0) as usize;
                let dx = seg_len / steps as f64;

                let mut prev_pt: Option<Point2> = None;
                for s in 0..=steps {
                    let cur_x = x1 + (s as f64) * dx;
                    let k = (2.0 * std::f64::consts::PI * cur_x) / wavelength;
                    let cur_y = y + amplitude * (k + phase_z + line_phase).sin();

                    let orig_pt = Point2::new(
                        cur_x * cos_a - cur_y * sin_a,
                        cur_x * sin_a + cur_y * cos_a,
                    );

                    if let Some(p_prev) = prev_pt {
                        segments.push(Segment2::new(p_prev, orig_pt));
                    }
                    prev_pt = Some(orig_pt);
                }
            }

            y += spacing;
            line_idx += 1;
        }

        segments
    }
}
