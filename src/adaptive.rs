use crate::bvh::TriangleIntervalIndex;

pub struct AdaptiveLayers;

impl AdaptiveLayers {
    /// Compute variable layer heights across Z range [min_z, max_z]
    /// based on surface curvature and slope angle with the horizontal plane.
    ///
    /// - Steep vertical walls (|n_z| near 0) receive thicker layers (up to `max_h`) for speed.
    /// - Shallow slopes and domes (|n_z| approaching 1) receive thinner layers (down to `min_h`) to eliminate stair-stepping.
    pub fn compute_layer_heights(
        bvh: &TriangleIntervalIndex,
        min_z: f64,
        max_z: f64,
        base_h: f64,
        min_h: f64,
        max_h: f64,
    ) -> Vec<(f64, f64)> {
        let mut layers = Vec::new();
        let total_height = max_z - min_z;
        if total_height <= 0.001 {
            return vec![(min_z + base_h * 0.5, base_h)];
        }

        // Layer 0 is anchored to first layer height
        let first_h = base_h;
        let mut cur_z = min_z;
        layers.push((cur_z + first_h * 0.5, first_h));
        cur_z += first_h;

        while cur_z < max_z - 1e-4 {
            let intersecting = bvh.query_z(cur_z);

            // Compute maximum horizontal slope factor |n_z| among intersecting triangles
            let mut max_nz = 0.0;
            for tri in &intersecting {
                let nz = tri.normal.z.abs();
                // Exclude flat horizontal planar caps (|nz| > 0.985)
                if nz < 0.985 && nz > max_nz {
                    max_nz = nz;
                }
            }

            // factor: 0.0 = shallow slope (needs thin slice), 1.0 = vertical wall (can take thick slice)
            let factor = (1.0 - max_nz).powf(1.5);
            let h = (min_h + factor * (max_h - min_h)).clamp(min_h, max_h);

            // If remaining height to max_z is small, adjust to avoid an ultra-thin sliver
            let remaining = max_z - cur_z;
            let final_h = if remaining < h * 1.3 && remaining >= min_h {
                remaining
            } else {
                h
            };

            let z_center = cur_z + final_h * 0.5;
            layers.push((z_center, final_h));
            cur_z += final_h;

            if remaining <= final_h + 1e-4 {
                break;
            }
        }

        layers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Point3, Triangle};

    #[test]
    fn test_adaptive_layers_cube_vs_sloped() {
        // Vertical cube wall: normal = (1, 0, 0), normal.z = 0
        let tri1 = Triangle {
            normal: Point3::new(1.0, 0.0, 0.0),
            v: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 10.0),
            ],
        };
        let tri2 = Triangle {
            normal: Point3::new(1.0, 0.0, 0.0),
            v: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 10.0),
                Point3::new(0.0, 0.0, 10.0),
            ],
        };
        let tris = vec![tri1, tri2];
        let bvh = TriangleIntervalIndex::build(&tris);
        let layers = AdaptiveLayers::compute_layer_heights(&bvh, 0.0, 10.0, 0.20, 0.08, 0.28);
        assert!(!layers.is_empty());
        // For pure vertical walls, layers should be near max_h (0.28)
        assert!(layers[1].1 >= 0.25);
    }
}
