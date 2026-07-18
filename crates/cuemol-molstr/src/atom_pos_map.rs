//! Atom-position indices corresponding to CueMol2 `AtomPosMap` and
//! `AtomPosMap2`.

use crate::{MolCoord, Selector};
use cuemol_core::{BSPTree, CueMolError, Result, Vector4};

/// Uniform-grid nearest-atom index.
///
/// Small selections use the same brute-force fallback as the C++ class.
#[derive(Debug)]
pub struct AtomPosMap<'a> {
    target: Option<&'a MolCoord>,
    spacing: f64,
    min_atom_map: usize,
    atoms: Vec<(i32, Vector4)>,
    grid: Option<AtomGrid>,
}

#[derive(Debug)]
struct AtomGrid {
    minimum: Vector4,
    maximum: Vector4,
    dimensions: [usize; 3],
    buckets: Vec<Vec<usize>>,
}

impl Default for AtomPosMap<'_> {
    fn default() -> Self {
        Self {
            target: None,
            spacing: 10.0,
            min_atom_map: 100,
            atoms: Vec::new(),
            grid: None,
        }
    }
}

impl<'a> AtomPosMap<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_target(&mut self, molecule: &'a MolCoord) {
        self.target = Some(molecule);
        self.atoms.clear();
        self.grid = None;
    }

    pub fn target(&self) -> Option<&MolCoord> {
        self.target
    }

    pub fn spacing(&self) -> f64 {
        self.spacing
    }

    pub fn set_spacing(&mut self, spacing: f64) -> Result<()> {
        if !spacing.is_finite() || spacing <= 0.0 {
            return Err(CueMolError::invalid_argument(
                "atom-position map spacing must be positive",
            ));
        }
        self.spacing = spacing;
        self.grid = None;
        Ok(())
    }

    pub fn min_atom_map(&self) -> usize {
        self.min_atom_map
    }

    pub fn set_min_atom_map(&mut self, count: usize) {
        self.min_atom_map = count;
        self.grid = None;
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    pub fn uses_brute_force(&self) -> bool {
        self.grid.is_none()
    }

    /// Build the index from the target molecule and an optional atom selection.
    pub fn generate(&mut self, selection: Option<&Selector>) -> Result<()> {
        let molecule = self
            .target
            .ok_or_else(|| CueMolError::invalid_argument("atom-position map target is not set"))?;
        self.atoms = molecule
            .atoms()
            .iter()
            .filter(|atom| {
                selection.is_none_or(|selector| selector.matches_in_molecule(atom, molecule))
            })
            .map(|atom| (atom.serial, atom.position))
            .collect();
        self.grid = None;

        if self.atoms.is_empty() || self.atoms.len() < self.min_atom_map {
            return Ok(());
        }

        let mut minimum = Vector4::new3(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut maximum = Vector4::new3(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (_, position) in &self.atoms {
            minimum.x = minimum.x.min(position.x);
            minimum.y = minimum.y.min(position.y);
            minimum.z = minimum.z.min(position.z);
            maximum.x = maximum.x.max(position.x);
            maximum.y = maximum.y.max(position.y);
            maximum.z = maximum.z.max(position.z);
        }
        minimum.x -= 0.1;
        minimum.y -= 0.1;
        minimum.z -= 0.1;
        maximum.x += 0.1;
        maximum.y += 0.1;
        maximum.z += 0.1;

        let dimensions = [
            grid_dimension(maximum.x - minimum.x, self.spacing),
            grid_dimension(maximum.y - minimum.y, self.spacing),
            grid_dimension(maximum.z - minimum.z, self.spacing),
        ];
        let bucket_count = dimensions[0]
            .checked_mul(dimensions[1])
            .and_then(|value| value.checked_mul(dimensions[2]))
            .ok_or_else(|| CueMolError::invalid_argument("atom-position map is too large"))?;
        let mut grid = AtomGrid {
            minimum,
            maximum,
            dimensions,
            buckets: vec![Vec::new(); bucket_count],
        };
        for (index, (_, position)) in self.atoms.iter().enumerate() {
            let coordinate = grid.coordinate(*position);
            let bucket = grid.flat_index(coordinate);
            grid.buckets[bucket].push(index);
        }
        self.grid = Some(grid);
        Ok(())
    }

    /// Return the serial ID of the nearest selected atom.
    pub fn search_nearest_atom(&self, position: Vector4) -> Option<i32> {
        let Some(grid) = &self.grid else {
            return nearest_atom(&self.atoms, self.atoms.iter().enumerate(), position);
        };
        let center = grid.coordinate(position);
        let mut candidates = Vec::new();
        for dz in -1_isize..=1 {
            for dy in -1_isize..=1 {
                for dx in -1_isize..=1 {
                    let coordinate = [
                        center[0] as isize + dx,
                        center[1] as isize + dy,
                        center[2] as isize + dz,
                    ];
                    if coordinate
                        .iter()
                        .enumerate()
                        .any(|(axis, value)| *value < 0 || *value >= grid.dimensions[axis] as isize)
                    {
                        continue;
                    }
                    candidates.extend(
                        grid.buckets[grid.flat_index([
                            coordinate[0] as usize,
                            coordinate[1] as usize,
                            coordinate[2] as usize,
                        ])]
                        .iter()
                        .copied(),
                    );
                }
            }
        }
        if candidates.is_empty() {
            nearest_atom(&self.atoms, self.atoms.iter().enumerate(), position)
        } else {
            nearest_atom(
                &self.atoms,
                candidates
                    .into_iter()
                    .map(|index| (index, &self.atoms[index])),
                position,
            )
        }
    }

    /// Return the largest number of indexed atoms in a grid cell.
    pub fn max_bucket_size(&self) -> usize {
        self.grid
            .as_ref()
            .and_then(|grid| grid.buckets.iter().map(Vec::len).max())
            .unwrap_or(self.atoms.len())
    }
}

impl AtomGrid {
    fn coordinate(&self, position: Vector4) -> [usize; 3] {
        [
            grid_coordinate(
                position.x,
                self.minimum.x,
                self.maximum.x,
                self.dimensions[0],
            ),
            grid_coordinate(
                position.y,
                self.minimum.y,
                self.maximum.y,
                self.dimensions[1],
            ),
            grid_coordinate(
                position.z,
                self.minimum.z,
                self.maximum.z,
                self.dimensions[2],
            ),
        ]
    }

    fn flat_index(&self, coordinate: [usize; 3]) -> usize {
        coordinate[0] + (coordinate[1] + coordinate[2] * self.dimensions[1]) * self.dimensions[0]
    }
}

fn grid_dimension(extent: f64, spacing: f64) -> usize {
    (extent / spacing).ceil().max(1.0) as usize
}

fn grid_coordinate(value: f64, minimum: f64, maximum: f64, dimension: usize) -> usize {
    if value <= minimum {
        return 0;
    }
    if value >= maximum {
        return dimension - 1;
    }
    let cell_size = (maximum - minimum) / dimension as f64;
    (((value - minimum) / cell_size) as usize).min(dimension - 1)
}

fn nearest_atom<'a>(
    atoms: &[(i32, Vector4)],
    candidates: impl Iterator<Item = (usize, &'a (i32, Vector4))>,
    position: Vector4,
) -> Option<i32> {
    let mut nearest = None;
    for (index, (_, atom_position)) in candidates {
        let distance = (*atom_position - position).sqlen();
        if nearest.is_none_or(|(_, best)| distance < best) {
            nearest = Some((index, distance));
        }
    }
    nearest.map(|(index, _)| atoms[index].0)
}

/// BSP-tree implementation of the newer CueMol2 `AtomPosMap2`.
#[derive(Debug)]
pub struct AtomPosMap2<'a> {
    target: Option<&'a MolCoord>,
    tree: BSPTree<i32>,
}

impl Default for AtomPosMap2<'_> {
    fn default() -> Self {
        Self {
            target: None,
            tree: BSPTree::new(),
        }
    }
}

impl<'a> AtomPosMap2<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_target(&mut self, molecule: &'a MolCoord) {
        self.target = Some(molecule);
        self.tree.clear();
    }

    pub fn target(&self) -> Option<&MolCoord> {
        self.target
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn generate(&mut self, selection: Option<&Selector>) -> Result<()> {
        let molecule = self
            .target
            .ok_or_else(|| CueMolError::invalid_argument("atom-position map target is not set"))?;
        self.tree.clear();
        for atom in molecule.atoms().iter().filter(|atom| {
            selection.is_none_or(|selector| selector.matches_in_molecule(atom, molecule))
        }) {
            self.tree.push(atom.position, atom.serial);
        }
        self.tree.build();
        Ok(())
    }

    pub fn search_nearest_atom(&self, position: Vector4) -> Option<i32> {
        self.tree.nearest(position).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_selector, Atom};
    use std::collections::BTreeMap;

    fn atom(serial: i32, name: &str, x: f64, y: f64, z: f64) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "GLY".to_string(),
            chain_id: "A".to_string(),
            res_id: serial,
            ins_code: None,
            position: Vector4::new3(x, y, z),
            occupancy: 1.0,
            b_factor: 0.0,
            element: name.chars().next().unwrap().to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        }
    }

    #[test]
    fn atom_pos_map_matches_reference_brute_force_and_selection() {
        let molecule = MolCoord::with_atoms(vec![
            atom(10, "N", -5.0, 0.0, 0.0),
            atom(20, "CA", 1.0, 0.0, 0.0),
            atom(30, "N", 4.0, 0.0, 0.0),
        ]);
        let mut map = AtomPosMap::new();
        map.set_target(&molecule);
        map.generate(None).unwrap();
        assert!(map.uses_brute_force());
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(0.8, 0.0, 0.0)),
            Some(20)
        );

        let selector = parse_selector("name N").unwrap();
        map.generate(Some(&selector)).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(2.0, 0.0, 0.0)),
            Some(30)
        );
    }

    #[test]
    fn atom_pos_map_grid_handles_boundaries_and_empty_selections() {
        let atoms = (0..125)
            .map(|index| {
                atom(
                    index + 1,
                    "C",
                    (index % 5) as f64 * 3.0,
                    ((index / 5) % 5) as f64 * 3.0,
                    (index / 25) as f64 * 3.0,
                )
            })
            .collect();
        let molecule = MolCoord::with_atoms(atoms);
        let mut map = AtomPosMap::new();
        map.set_target(&molecule);
        map.set_spacing(2.0).unwrap();
        map.generate(None).unwrap();
        assert!(!map.uses_brute_force());
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(-100.0, -100.0, -100.0)),
            Some(1)
        );
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(100.0, 100.0, 100.0)),
            Some(125)
        );

        map.generate(Some(&Selector::None)).unwrap();
        assert!(map.is_empty());
        assert_eq!(map.search_nearest_atom(Vector4::ZERO), None);
    }

    #[test]
    fn atom_pos_map2_builds_and_queries_bsp_tree() {
        let molecule = MolCoord::with_atoms(vec![
            atom(1, "C", -20.0, 2.0, 0.0),
            atom(2, "N", 3.0, 4.0, 0.0),
            atom(3, "C", 100.0, 0.0, 0.0),
        ]);
        let selector = parse_selector("elem C").unwrap();
        let mut map = AtomPosMap2::new();
        map.set_target(&molecule);
        map.generate(Some(&selector)).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(0.0, 0.0, 0.0)),
            Some(1)
        );
        assert_eq!(
            map.search_nearest_atom(Vector4::new3(90.0, 0.0, 0.0)),
            Some(3)
        );
    }
}
