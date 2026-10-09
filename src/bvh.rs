use crate::geom::Triangle;

pub struct TriangleIntervalIndex<'a> {
    entries: Vec<IndexEntry<'a>>,
}

struct IndexEntry<'a> {
    min_z: f64,
    max_z: f64,
    triangle: &'a Triangle,
}

impl<'a> TriangleIntervalIndex<'a> {
    pub fn build(triangles: &'a [Triangle]) -> Self {
        let mut entries: Vec<IndexEntry<'a>> = triangles
            .iter()
            .map(|t| IndexEntry {
                min_z: t.min_z(),
                max_z: t.max_z(),
                triangle: t,
            })
            .collect();

        // Sort by min_z to enable early exit or binary search
        entries.sort_unstable_by(|a, b| a.min_z.partial_cmp(&b.min_z).unwrap());

        Self { entries }
    }

    /// Query all triangles that intersect the horizontal plane at Z.
    /// Excludes triangles lying entirely above or below Z.
    pub fn query_z(&self, z: f64) -> Vec<&'a Triangle> {
        let mut result = Vec::new();

        for entry in &self.entries {
            // Because entries are sorted by min_z, once min_z > z, no future triangles can intersect
            if entry.min_z > z {
                break;
            }
            if entry.max_z >= z {
                result.push(entry.triangle);
            }
        }

        result
    }
}
