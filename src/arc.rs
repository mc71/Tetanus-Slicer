use crate::geom::Point2;

#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    Linear {
        end: Point2,
    },
    Arc {
        end: Point2,
        i: f64,          // X offset from start to center
        j: f64,          // Y offset from start to center
        radius: f64,
        clockwise: bool, // true = G2, false = G3
    },
}

pub struct ArcFitter;

impl ArcFitter {
    /// Fit a sequence of 2D points into linear segments and G2/G3 circular arcs.
    /// `tolerance`: maximum radial deviation allowed in mm (default ~0.02 mm).
    pub fn fit_arcs(points: &[Point2], is_closed: bool, tolerance: f64) -> Vec<PathSegment> {
        let n = points.len();
        if n < 3 {
            return points.iter().skip(1).map(|&p| PathSegment::Linear { end: p }).collect();
        }

        let mut commands = Vec::new();
        let total = if is_closed { n + 1 } else { n };
        let get_pt = |idx: usize| points[idx % n];

        let mut i = 0;
        while i < total - 1 {
            let start = get_pt(i);

            // Attempt to fit a circular arc starting from start
            if i + 2 < total {
                let p1 = get_pt(i + 1);
                let p2 = get_pt(i + 2);

                if let Some((center, radius, clockwise)) = Self::fit_circle(start, p1, p2) {
                    if radius >= 0.8 && radius <= 1500.0 {
                        let mut last_idx = i + 2;
                        let mut max_angle = 0.0;
                        let mut last_angle = (start - center).y.atan2((start - center).x);

                        while last_idx + 1 < total {
                            let next_pt = get_pt(last_idx + 1);
                            let r_next = next_pt.distance_to(center);
                            if (r_next - radius).abs() > tolerance {
                                break;
                            }

                            // Angular step verification
                            let angle = (next_pt - center).y.atan2((next_pt - center).x);
                            let mut d_angle = if clockwise {
                                last_angle - angle
                            } else {
                                angle - last_angle
                            };
                            while d_angle < 0.0 {
                                d_angle += std::f64::consts::TAU;
                            }
                            while d_angle >= std::f64::consts::TAU {
                                d_angle -= std::f64::consts::TAU;
                            }

                            // Prevent direction reversals or jumps > 135 deg in one step
                            if d_angle > std::f64::consts::PI * 0.75 || d_angle < 1e-4 {
                                break;
                            }

                            max_angle += d_angle;
                            if max_angle > std::f64::consts::PI {
                                // Cap arc span to 180 degrees to avoid full-circle ambiguity in G-code
                                break;
                            }

                            last_angle = angle;
                            last_idx += 1;
                        }

                        // Collapse into arc command if at least 3 segments (4 points) fit
                        if last_idx >= i + 3 {
                            let end = get_pt(last_idx);
                            let offset_i = center.x - start.x;
                            let offset_j = center.y - start.y;
                            commands.push(PathSegment::Arc {
                                end,
                                i: offset_i,
                                j: offset_j,
                                radius,
                                clockwise,
                            });
                            i = last_idx;
                            continue;
                        }
                    }
                }
            }

            // Fallback to standard linear segment
            let next = get_pt(i + 1);
            commands.push(PathSegment::Linear { end: next });
            i += 1;
        }

        commands
    }

    /// Calculate circumcenter, radius, and rotation direction (clockwise) for 3 points.
    pub fn fit_circle(a: Point2, b: Point2, c: Point2) -> Option<(Point2, f64, bool)> {
        let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
        if d.abs() < 1e-6 {
            return None; // Collinear points
        }

        let a2 = a.x * a.x + a.y * a.y;
        let b2 = b.x * b.x + b.y * b.y;
        let c2 = c.x * c.x + c.y * c.y;

        let cx = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
        let cy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
        let center = Point2::new(cx, cy);
        let radius = a.distance_to(center);

        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        let clockwise = cross < 0.0;

        Some((center, radius, clockwise))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_fitting_circle() {
        // Generate a 16-segment approximation of a circle with R = 10.0
        let radius = 10.0;
        let segments = 16;
        let mut pts = Vec::new();
        for i in 0..segments {
            let theta = (i as f64) * std::f64::consts::TAU / (segments as f64);
            pts.push(Point2::new(radius * theta.cos(), radius * theta.sin()));
        }

        let cmds = ArcFitter::fit_arcs(&pts, true, 0.05);
        println!("Fitted {} points into {} commands", pts.len(), cmds.len());
        // Should produce 2 arcs (180 deg each) instead of 16 linear commands
        let arc_count = cmds.iter().filter(|c| matches!(c, PathSegment::Arc { .. })).count();
        assert!(arc_count >= 2, "Expected at least 2 arcs fitted from circle, got {}", arc_count);
    }
}
