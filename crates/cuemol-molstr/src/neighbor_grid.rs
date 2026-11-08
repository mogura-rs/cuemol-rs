use cuemol_core::Vector4;
use std::collections::HashMap;

pub(crate) struct NeighborGrid<T> {
    spacing: f64,
    cells: HashMap<[i64; 3], Vec<T>>,
}

impl<T: Copy> NeighborGrid<T> {
    pub fn new(spacing: f64) -> Option<Self> {
        (spacing.is_finite() && spacing > 0.).then(|| Self {
            spacing,
            cells: HashMap::new(),
        })
    }

    fn cell(&self, position: Vector4) -> Option<[i64; 3]> {
        let coordinates = [position.x, position.y, position.z];
        coordinates
            .iter()
            .all(|value| value.is_finite())
            .then(|| coordinates.map(|value| (value / self.spacing).floor() as i64))
    }

    pub fn insert(&mut self, position: Vector4, value: T) {
        if let Some(cell) = self.cell(position) {
            self.cells.entry(cell).or_default().push(value);
        }
    }

    /// Collect a conservative neighborhood; the caller applies its exact distance rule.
    pub fn nearby(&self, position: Vector4, output: &mut Vec<T>) {
        output.clear();
        let Some([x, y, z]) = self.cell(position) else {
            return;
        };
        for dx in -1..=1 {
            let Some(x) = x.checked_add(dx) else { continue };
            for dy in -1..=1 {
                let Some(y) = y.checked_add(dy) else { continue };
                for dz in -1..=1 {
                    let Some(z) = z.checked_add(dz) else { continue };
                    if let Some(values) = self.cells.get(&[x, y, z]) {
                        output.extend_from_slice(values);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighborhoods_contain_all_finite_points_within_the_cutoff() {
        let mut random = 23_u64;
        let mut points = Vec::new();
        for _ in 0..160 {
            let mut coordinate = || {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((random >> 32) as i32 as f64) / i32::MAX as f64 * 12.
            };
            points.push(Vector4::new3(coordinate(), coordinate(), coordinate()));
        }
        points.extend([
            Vector4::new3(0., 0., 0.),
            Vector4::new3(1., 0., 0.),
            Vector4::new3(-1., 0., 0.),
            Vector4::new3(f64::MAX, 0., 0.),
            Vector4::new3(-f64::MAX, 0., 0.),
        ]);
        for cutoff in [f64::MIN_POSITIVE, 0.5, 1., 3.7, 100.] {
            let mut grid = NeighborGrid::new(cutoff).unwrap();
            for (index, point) in points.iter().enumerate() {
                grid.insert(*point, index);
            }
            let mut neighbors = Vec::new();
            for query in &points {
                grid.nearby(*query, &mut neighbors);
                for (index, point) in points.iter().enumerate() {
                    if (*query - *point).length3d() <= cutoff {
                        assert!(neighbors.contains(&index), "{query:?} {point:?} {cutoff}");
                    }
                }
                let mut unique = neighbors.clone();
                unique.sort_unstable();
                unique.dedup();
                assert_eq!(unique.len(), neighbors.len());
            }
        }
        assert!(NeighborGrid::<usize>::new(0.).is_none());
        assert!(NeighborGrid::<usize>::new(f64::INFINITY).is_none());
    }
}
