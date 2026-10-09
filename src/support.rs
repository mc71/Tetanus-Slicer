use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::geom::{Point2, Polygon2, Segment2};
use crate::infill::{InfillGenerator, InfillPattern};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SupportConfig {
    pub enabled: bool,
    pub overhang_angle: f64, // degrees (e.g. 45.0)
    pub support_density: f64, // e.g. 0.15 (15%)
    pub line_width: f64,
    pub layer_height: f64,
    pub xy_gap: f64, // clearance to model walls (e.g. 0.6 mm)
}

impl Default for SupportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            overhang_angle: 45.0,
            support_density: 0.15,
            line_width: 0.45,
            layer_height: 0.20,
            xy_gap: 0.6,
        }
    }
}

pub struct SupportGenerator;

impl SupportGenerator {
    /// Generates support polygons and infill lines for every layer
    /// using downward projected column voxels.
    pub fn generate_supports(
        layers_outer_polys: &[Vec<Polygon2>],
        config: &SupportConfig,
    ) -> Vec<(Vec<Polygon2>, Vec<Segment2>)> {
        let num_layers = layers_outer_polys.len();
        if !config.enabled || num_layers < 2 {
            return vec![(Vec::new(), Vec::new()); num_layers];
        }

        let max_rad = config.overhang_angle.to_radians();
        let max_cantilever = config.layer_height * max_rad.tan() + config.line_width * 0.5;
        let cell_size = 2.5; // mm grid pillar size

        // Track active support cells per layer: layer_idx -> Set of (gx, gy)
        let mut layer_support_cells: Vec<HashSet<(i32, i32)>> = vec![HashSet::new(); num_layers];

        // 1. Detect overhangs from top to bottom
        for layer_idx in (1..num_layers).rev() {
            let curr_polys = &layers_outer_polys[layer_idx];
            let prev_polys = &layers_outer_polys[layer_idx - 1];

            if curr_polys.is_empty() {
                continue;
            }

            // Check sample points along perimeters of curr_polys
            for poly in curr_polys {
                let n = poly.points.len();
                for i in 0..n {
                    let pt1 = poly.points[i];
                    let pt2 = poly.points[(i + 1) % n];
                    let seg_len = pt1.distance_to(pt2);

                    let samples = (seg_len / 1.0).ceil().max(1.0) as usize;
                    for s in 0..samples {
                        let t = s as f64 / samples as f64;
                        let sample_pt = Point2::new(
                            pt1.x + t * (pt2.x - pt1.x),
                            pt1.y + t * (pt2.y - pt1.y),
                        );

                        // Distance to previous layer solid geometry
                        let min_dist_to_prev = prev_polys
                            .iter()
                            .map(|prev| prev.distance_to_solid(sample_pt))
                            .fold(f64::MAX, f64::min);

                        if min_dist_to_prev > max_cantilever {
                            // Overhang detected! Mark grid cell on layer_idx - 1
                            let gx = (sample_pt.x / cell_size).floor() as i32;
                            let gy = (sample_pt.y / cell_size).floor() as i32;
                            layer_support_cells[layer_idx - 1].insert((gx, gy));
                        }
                    }
                }
            }
        }

        // 2. Propagate support cells downward to bed or model surface
        for layer_idx in (1..num_layers).rev() {
            let cells_above = layer_support_cells[layer_idx].clone();
            if cells_above.is_empty() {
                continue;
            }

            let prev_polys = &layers_outer_polys[layer_idx - 1];

            for (gx, gy) in cells_above {
                let center_x = (gx as f64 + 0.5) * cell_size;
                let center_y = (gy as f64 + 0.5) * cell_size;
                let center = Point2::new(center_x, center_y);

                // If cell hits solid model surface on layer_idx - 1, support column lands there!
                let is_inside_model = prev_polys.iter().any(|poly| poly.contains_point(center));
                if !is_inside_model {
                    layer_support_cells[layer_idx - 1].insert((gx, gy));
                }
            }
        }

        // 3. For each layer, remove cells too close to model walls (xy_gap clearance)
        // and convert cells to bounding polygons and infill lines
        let mut results = Vec::with_capacity(num_layers);

        for (layer_idx, cells) in layer_support_cells.iter().enumerate() {
            let model_polys = &layers_outer_polys[layer_idx];
            let mut valid_polys = Vec::new();

            for &(gx, gy) in cells {
                let min_x = gx as f64 * cell_size;
                let min_y = gy as f64 * cell_size;
                let max_x = min_x + cell_size;
                let max_y = min_y + cell_size;

                let center = Point2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);

                // Clearance check: must be at least xy_gap away from model perimeters
                let too_close = model_polys.iter().any(|poly| {
                    poly.distance_to_solid(center) < config.xy_gap
                });

                if !too_close {
                    // Create cell box polygon (with slight inset so adjacent cells don't overlap)
                    let inset = 0.1;
                    valid_polys.push(Polygon2::new(vec![
                        Point2::new(min_x + inset, min_y + inset),
                        Point2::new(max_x - inset, min_y + inset),
                        Point2::new(max_x - inset, max_y - inset),
                        Point2::new(min_x + inset, max_y - inset),
                    ]));
                }
            }

            // Generate support infill lines across the valid support polygons
            let infill = InfillGenerator::generate_infill(
                InfillPattern::Rectilinear,
                &valid_polys,
                config.support_density,
                config.line_width,
                layer_idx,
                (layer_idx as f64 + 0.5) * config.layer_height,
            );

            results.push((valid_polys, infill));
        }

        results
    }
}
