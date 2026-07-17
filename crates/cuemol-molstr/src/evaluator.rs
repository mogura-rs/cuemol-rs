//! Numeric atom and residue evaluator interfaces.

use crate::{Atom, ResidueView};

/// Floating-point property evaluator corresponding to `RealNumEvaluator`.
pub trait RealNumEvaluator {
    fn atom_value(&self, atom: &Atom) -> Option<f64>;
    fn residue_value(&self, residue: &ResidueView<'_>) -> Option<f64>;
}

/// Integer property evaluator corresponding to `IntNumEvaluator`.
pub trait IntNumEvaluator {
    fn atom_value(&self, atom: &Atom) -> Option<i32>;
    fn residue_value(&self, residue: &ResidueView<'_>) -> Option<i32>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtomIterator, MolCoord, ResidIterator, Selector};
    use cuemol_core::Vector4;
    use std::collections::BTreeMap;

    struct BFactorEvaluator;

    impl RealNumEvaluator for BFactorEvaluator {
        fn atom_value(&self, atom: &Atom) -> Option<f64> {
            Some(f64::from(atom.b_factor))
        }

        fn residue_value(&self, residue: &ResidueView<'_>) -> Option<f64> {
            let atoms = residue.atoms();
            (!atoms.is_empty()).then(|| {
                atoms
                    .iter()
                    .map(|atom| f64::from(atom.b_factor))
                    .sum::<f64>()
                    / atoms.len() as f64
            })
        }
    }

    struct SerialEvaluator;

    impl IntNumEvaluator for SerialEvaluator {
        fn atom_value(&self, atom: &Atom) -> Option<i32> {
            Some(atom.serial)
        }

        fn residue_value(&self, residue: &ResidueView<'_>) -> Option<i32> {
            residue.atoms().first().map(|atom| atom.res_id)
        }
    }

    fn atom(serial: i32, b_factor: f32) -> Atom {
        Atom {
            serial,
            name: "CA".to_string(),
            alt_loc: None,
            res_name: "GLY".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(f64::from(serial), 0.0, 0.0),
            occupancy: 1.0,
            b_factor,
            element: "C".to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
        }
    }

    #[test]
    fn numeric_evaluators_cover_atoms_and_residues() {
        let molecule = MolCoord::with_atoms(vec![atom(1, 10.0), atom(2, 20.0)]);
        let atom = AtomIterator::all(&molecule).get().unwrap();
        let residue = ResidIterator::new(&molecule, Selector::All)
            .get()
            .unwrap()
            .clone();
        assert_eq!(BFactorEvaluator.atom_value(atom), Some(10.0));
        assert_eq!(BFactorEvaluator.residue_value(&residue), Some(15.0));
        assert_eq!(SerialEvaluator.atom_value(atom), Some(1));
        assert_eq!(SerialEvaluator.residue_value(&residue), Some(1));
    }
}
