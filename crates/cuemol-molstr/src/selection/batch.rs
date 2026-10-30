use super::{is_main_chain_atom, is_side_chain_atom, same_atom, DistanceOp, Selector, UnaryOp};
use crate::{Atom, MolCoord};
use cuemol_core::Vector4;
use std::collections::{HashMap, HashSet, VecDeque};

impl Selector {
    pub(crate) fn indices_in_molecule(&self, molecule: &MolCoord) -> Vec<usize> {
        if self.needs_batch() {
            self.batch_mask(molecule)
                .into_iter()
                .enumerate()
                .filter_map(|(index, selected)| selected.then_some(index))
                .collect()
        } else {
            molecule
                .atoms()
                .iter()
                .enumerate()
                .filter_map(|(index, atom)| {
                    self.matches_in_molecule(atom, molecule).then_some(index)
                })
                .collect()
        }
    }

    pub(crate) fn count_in_molecule(&self, molecule: &MolCoord) -> usize {
        if self.needs_batch() {
            self.batch_mask(molecule)
                .into_iter()
                .filter(|selected| *selected)
                .count()
        } else {
            molecule
                .atoms()
                .iter()
                .filter(|atom| self.matches_in_molecule(atom, molecule))
                .count()
        }
    }

    fn needs_batch(&self) -> bool {
        match self {
            Self::Distance { .. }
            | Self::Unary {
                op: UnaryOp::ByRes, ..
            } => true,
            Self::Unary { inner, .. } | Self::Not(inner) => inner.needs_batch(),
            Self::And(lhs, rhs) | Self::Or(lhs, rhs) => lhs.needs_batch() || rhs.needs_batch(),
            _ => false,
        }
    }

    fn batch_mask(&self, molecule: &MolCoord) -> Vec<bool> {
        let atoms = molecule.atoms();
        match self {
            Self::Not(inner) => {
                let mut mask = inner.batch_mask(molecule);
                for selected in &mut mask {
                    *selected = !*selected;
                }
                mask
            }
            Self::And(lhs, rhs) | Self::Or(lhs, rhs) => {
                let mut mask = lhs.batch_mask(molecule);
                let intersect = matches!(self, Self::And(..));
                if mask.iter().all(|value| *value != intersect) {
                    return mask;
                }
                for (left, right) in mask.iter_mut().zip(rhs.batch_mask(molecule)) {
                    *left = if intersect {
                        *left && right
                    } else {
                        *left || right
                    };
                }
                mask
            }
            Self::Unary { op, inner } => {
                let mut mask = inner.batch_mask(molecule);
                if *op == UnaryOp::ByRes {
                    let residues: HashSet<_> = atoms
                        .iter()
                        .zip(&mask)
                        .filter(|(_, selected)| **selected)
                        .map(|(atom, _)| (&atom.chain_id, atom.res_id, atom.ins_code))
                        .collect();
                    for (atom, selected) in atoms.iter().zip(&mut mask) {
                        *selected =
                            residues.contains(&(&atom.chain_id, atom.res_id, atom.ins_code));
                    }
                } else {
                    for (atom, selected) in atoms.iter().zip(&mut mask) {
                        *selected &= if *op == UnaryOp::ByMainChain {
                            is_main_chain_atom(atom)
                        } else {
                            is_side_chain_atom(atom)
                        };
                    }
                }
                mask
            }
            Self::Distance {
                op,
                inner,
                distance,
                molecule: target,
            } => {
                if target.is_some() {
                    return vec![false; atoms.len()];
                }
                let selected = inner.batch_mask(molecule);
                if matches!(op, DistanceOp::Neighbor | DistanceOp::Extend) {
                    graph_mask(molecule, selected, *op, *distance)
                } else {
                    distance_mask(atoms, selected, *op, *distance)
                }
            }
            _ => atoms
                .iter()
                .map(|atom| self.matches_in_molecule(atom, molecule))
                .collect(),
        }
    }
}

fn distance_mask(atoms: &[Atom], mut mask: Vec<bool>, op: DistanceOp, distance: f32) -> Vec<bool> {
    let include_inner = op == DistanceOp::Expand;
    let cutoff = f64::from(distance.max(0.0));
    let selected: Vec<_> = atoms
        .iter()
        .zip(&mask)
        .filter_map(|(atom, selected)| selected.then_some(atom))
        .collect();
    if cutoff == 0.0 || selected.is_empty() {
        if !include_inner {
            mask.fill(false);
        }
        return mask;
    }
    let near = |atom: &Atom, other: &Atom| {
        !same_atom(atom, other) && (atom.position - other.position).length3d() < cutoff
    };
    if selected.len() < 16 || !cutoff.is_finite() {
        for (atom, selected_self) in atoms.iter().zip(&mut mask) {
            *selected_self = if *selected_self {
                include_inner
            } else {
                selected.iter().any(|other| near(atom, other))
            };
        }
        return mask;
    }
    // Sparse cells keep storage proportional to selected atoms, including distant outliers.
    let mut grid = HashMap::<[i64; 3], Vec<&Atom>>::new();
    for atom in selected {
        if let Some(cell) = cell(atom.position, cutoff) {
            grid.entry(cell).or_default().push(atom);
        }
    }
    for (atom, selected_self) in atoms.iter().zip(&mut mask) {
        if *selected_self {
            *selected_self = include_inner;
            continue;
        }
        let Some(center) = cell(atom.position, cutoff) else {
            continue;
        };
        'neighbors: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = [
                        center[0].saturating_add(dx),
                        center[1].saturating_add(dy),
                        center[2].saturating_add(dz),
                    ];
                    if grid
                        .get(&key)
                        .is_some_and(|bucket| bucket.iter().any(|other| near(atom, other)))
                    {
                        *selected_self = true;
                        break 'neighbors;
                    }
                }
            }
        }
    }
    mask
}

fn cell(position: Vector4, spacing: f64) -> Option<[i64; 3]> {
    let values = [position.x, position.y, position.z];
    values
        .iter()
        .all(|value| value.is_finite())
        .then(|| values.map(|value| (value / spacing).floor() as i64))
}

fn graph_mask(
    molecule: &MolCoord,
    selected: Vec<bool>,
    op: DistanceOp,
    distance: f32,
) -> Vec<bool> {
    let atoms = molecule.atoms();
    let depth_limit = distance.max(0.0).ceil() as usize;
    let mut mask = selected.clone();
    if depth_limit > 0 && selected.iter().any(|value| *value) {
        let mut edges = vec![Vec::new(); atoms.len()];
        for bond in molecule.bonds() {
            if bond.atom1 < edges.len() && bond.atom2 < edges.len() {
                edges[bond.atom1].push(bond.atom2);
                edges[bond.atom2].push(bond.atom1);
            }
        }
        let mut queue: VecDeque<_> = selected
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.then_some((index, 0)))
            .collect();
        while let Some((index, depth)) = queue.pop_front() {
            if depth >= depth_limit {
                continue;
            }
            for next in &edges[index] {
                if !mask[*next] {
                    mask[*next] = true;
                    queue.push_back((*next, depth + 1));
                }
            }
        }
    }
    if op == DistanceOp::Neighbor {
        for (value, inner) in mask.iter_mut().zip(selected) {
            *value &= !inner;
        }
    }
    let mut identities = HashMap::new();
    atoms
        .iter()
        .enumerate()
        .map(|(index, atom)| {
            let key = (
                atom.serial,
                &atom.chain_id,
                atom.res_id,
                atom.ins_code,
                &atom.name,
                atom.alt_loc,
            );
            let first = *identities.entry(key).or_insert(index);
            mask[first]
        })
        .collect()
}
