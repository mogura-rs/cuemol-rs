//! Molecule edit/undo support — port of the C++ `qsys::UndoManager` + `EditInfo`
//! pattern, adapted to Rust ownership.
//!
//! The C++ design registers `EditInfo` subclasses (e.g. `Prot2ndryEditInfo`,
//! `MolBondEditInfo`) with a global `UndoManager`; each `EditInfo::undo()` reaches
//! its target object through a global UID registry. Rust has no such global, so
//! the edit-info's `undo`/`redo` take the [`MolCoord`] explicitly and the manager
//! is owned alongside the molecule it edits. [`MolEditInfo`] mirrors `EditInfo`
//! (a reversible record of one edit) and [`MolUndoManager`] mirrors the manager's
//! undo/redo stacks.
//!
//! (cuemol-qsys already has a generic `UndoManager`/`EditInfo`, but its
//! `undo(&mut self)` cannot reach a molecule — it was orphaned for exactly this
//! reason. This molecule-reaching variant is the consumer-backed version.)

use std::collections::BTreeMap;

use crate::molecule::{MolCoord, ResidueKey};
use crate::Selector;
use cuemol_core::Vector4;

/// A reversible record of a single molecule edit (port of C++ `EditInfo`).
pub trait MolEditInfo {
    /// Revert the edit on `mol`. Returns `false` if it cannot be applied.
    fn undo(&mut self, mol: &mut MolCoord) -> bool;
    /// Re-apply the edit on `mol`. Returns `false` if it cannot be applied.
    fn redo(&mut self, mol: &mut MolCoord) -> bool;
    /// Human-readable description (e.g. "set secondary structure").
    fn description(&self) -> &str;
}

/// Undo/redo stacks for molecule edits (port of C++ `qsys::UndoManager`, made
/// molecule-reaching). Record an edit after performing it; `undo`/`redo` apply the
/// recorded edits to the molecule.
#[derive(Default)]
pub struct MolUndoManager {
    undo_stack: Vec<Box<dyn MolEditInfo>>,
    redo_stack: Vec<Box<dyn MolEditInfo>>,
    disabled: bool,
}

impl MolUndoManager {
    pub fn new() -> Self {
        MolUndoManager::default()
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn is_undoable(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn is_redoable(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_size(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_size(&self) -> usize {
        self.redo_stack.len()
    }

    /// Record a freshly-performed edit. Clears the redo stack (a new edit
    /// invalidates the redo history), matching C++ `UndoManager::addEditInfo`.
    pub fn record(&mut self, edit: Box<dyn MolEditInfo>) {
        if self.disabled {
            return;
        }
        self.undo_stack.push(edit);
        self.redo_stack.clear();
    }

    /// Undo the most recent edit, moving it to the redo stack.
    pub fn undo(&mut self, mol: &mut MolCoord) -> bool {
        let Some(mut edit) = self.undo_stack.pop() else {
            return false;
        };
        let ok = edit.undo(mol);
        self.redo_stack.push(edit);
        ok
    }

    /// Redo the most recently undone edit, moving it back to the undo stack.
    pub fn redo(&mut self, mol: &mut MolCoord) -> bool {
        let Some(mut edit) = self.redo_stack.pop() else {
            return false;
        };
        let ok = edit.redo(mol);
        self.undo_stack.push(edit);
        ok
    }

    pub fn clear_all(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo_desc(&self) -> Option<&str> {
        self.undo_stack.last().map(|e| e.description())
    }

    pub fn redo_desc(&self) -> Option<&str> {
        self.redo_stack.last().map(|e| e.description())
    }
}

/// Undo/redo record for an atom-coordinate transformation.
///
/// This is the Rust ownership-model counterpart of C++ `MolXformEditInfo`.
/// Atom serials are stored instead of array indices so the record remains valid
/// when unrelated atoms are reordered. Missing atoms are skipped, matching the
/// reference implementation's partial-restore behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct MolXformEditInfo {
    before: Vec<(i32, Vector4)>,
    after: Vec<(i32, Vector4)>,
    description: String,
}

impl Default for MolXformEditInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl MolXformEditInfo {
    pub fn new() -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            description: "transform atoms".to_string(),
        }
    }

    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            ..Self::new()
        }
    }

    pub fn save_before_pos(&mut self, molecule: &MolCoord) {
        self.save_before_pos_selected(molecule, None);
    }

    pub fn save_before_pos_sel(&mut self, molecule: &MolCoord, selector: &Selector) {
        self.save_before_pos_selected(molecule, Some(selector));
    }

    pub fn save_before_pos_selected(&mut self, molecule: &MolCoord, selector: Option<&Selector>) {
        self.clear();
        self.before = snapshot_positions(molecule, selector);
    }

    pub fn save_after_pos(&mut self, molecule: &MolCoord) {
        self.save_after_pos_selected(molecule, None);
    }

    pub fn save_after_pos_sel(&mut self, molecule: &MolCoord, selector: &Selector) {
        self.save_after_pos_selected(molecule, Some(selector));
    }

    pub fn save_after_pos_selected(&mut self, molecule: &MolCoord, selector: Option<&Selector>) {
        self.after = snapshot_positions(molecule, selector);
    }

    pub fn clear(&mut self) {
        self.before.clear();
        self.after.clear();
    }

    pub fn before_positions(&self) -> &[(i32, Vector4)] {
        &self.before
    }

    pub fn after_positions(&self) -> &[(i32, Vector4)] {
        &self.after
    }

    pub fn is_undoable(&self) -> bool {
        true
    }

    pub fn is_redoable(&self) -> bool {
        true
    }

    fn move_impl(molecule: &mut MolCoord, positions: &[(i32, Vector4)]) -> bool {
        for &(serial, position) in positions {
            if let Some(atom) = molecule.atom_mut_by_serial(serial) {
                atom.position = position;
            }
        }
        molecule.fire_atoms_moved();
        true
    }
}

impl MolEditInfo for MolXformEditInfo {
    fn undo(&mut self, molecule: &mut MolCoord) -> bool {
        Self::move_impl(molecule, &self.before)
    }

    fn redo(&mut self, molecule: &mut MolCoord) -> bool {
        Self::move_impl(molecule, &self.after)
    }

    fn description(&self) -> &str {
        &self.description
    }
}

fn snapshot_positions(molecule: &MolCoord, selector: Option<&Selector>) -> Vec<(i32, Vector4)> {
    let indices = selector
        .map(|selector| molecule.select_indices(selector))
        .unwrap_or_else(|| (0..molecule.atom_count()).collect());
    indices
        .into_iter()
        .filter_map(|index| {
            molecule
                .atoms()
                .get(index)
                .map(|atom| (atom.serial, atom.position))
        })
        .collect()
}

type ResiduePropsMap = BTreeMap<ResidueKey, BTreeMap<String, String>>;

/// An edit that changed the residue-property table (e.g. a secondary-structure
/// assignment). Port of C++ `Prot2ndryEditInfo`: it snapshots the table before and
/// after, restoring either on undo/redo.
pub struct ResiduePropsEdit {
    before: ResiduePropsMap,
    after: ResiduePropsMap,
    description: String,
}

/// Undo/redo record for a persistent bond edit.
///
/// This mirrors C++ `MolBondEditInfo`: undoing a made bond removes it, while
/// undoing a removed bond recreates it as a persistent single bond.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MolBondEditInfo {
    make: bool,
    atom1: i32,
    atom2: i32,
    description: String,
}

impl MolBondEditInfo {
    pub fn new(make: bool, atom1: i32, atom2: i32) -> Self {
        Self {
            make,
            atom1,
            atom2,
            description: if make { "make bond" } else { "remove bond" }.to_string(),
        }
    }

    fn apply(&self, molecule: &mut MolCoord, make: bool) -> bool {
        if make {
            molecule.make_bond(self.atom1, self.atom2, true)
        } else {
            molecule.remove_bond(self.atom1, self.atom2)
        }
    }
}

impl MolEditInfo for MolBondEditInfo {
    fn undo(&mut self, molecule: &mut MolCoord) -> bool {
        self.apply(molecule, !self.make)
    }

    fn redo(&mut self, molecule: &mut MolCoord) -> bool {
        self.apply(molecule, self.make)
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Undo/redo record for chain-name and residue-index changes.
///
/// C++ uses `ChgChnameEditInfo` and `ShiftResIndexEditInfo`; both store old/new
/// residue keys and replay the relabel operation in the required direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidueRelabelEdit {
    changes: Vec<(ResidueKey, ResidueKey)>,
    description: String,
}

impl ResidueRelabelEdit {
    pub fn new(changes: Vec<(ResidueKey, ResidueKey)>, description: impl Into<String>) -> Self {
        Self {
            changes,
            description: description.into(),
        }
    }

    fn apply(&self, molecule: &mut MolCoord, reverse: bool) -> bool {
        let mapping = self
            .changes
            .iter()
            .map(|(old, new)| {
                if reverse {
                    (new.clone(), old.clone())
                } else {
                    (old.clone(), new.clone())
                }
            })
            .collect::<BTreeMap<_, _>>();
        let changed = molecule.relabel_residue_keys(|atom| {
            let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
            mapping.get(&key).cloned()
        });
        if changed {
            molecule.apply_topology();
        }
        changed
    }
}

impl MolEditInfo for ResidueRelabelEdit {
    fn undo(&mut self, molecule: &mut MolCoord) -> bool {
        self.apply(molecule, true)
    }

    fn redo(&mut self, molecule: &mut MolCoord) -> bool {
        self.apply(molecule, false)
    }

    fn description(&self) -> &str {
        &self.description
    }
}

impl ResiduePropsEdit {
    pub fn new(
        before: ResiduePropsMap,
        after: ResiduePropsMap,
        description: impl Into<String>,
    ) -> Self {
        ResiduePropsEdit {
            before,
            after,
            description: description.into(),
        }
    }
}

impl MolEditInfo for ResiduePropsEdit {
    fn undo(&mut self, mol: &mut MolCoord) -> bool {
        mol.set_all_residue_props(self.before.clone());
        true
    }

    fn redo(&mut self, mol: &mut MolCoord) -> bool {
        mol.set_all_residue_props(self.after.clone());
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::molecule::Atom;
    use cuemol_core::Vector4;

    fn atom(serial: i32, chain: &str, res_id: i32, name: &str) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: chain.to_string(),
            res_id,
            ins_code: None,
            position: Vector4::new3(0.0, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
            real_props: Default::default(),
        }
    }

    #[test]
    fn residue_props_edit_undoes_and_redoes() {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "A", 1, "CA"));
        mol.set_residue_prop_str("A", "1", "secondary", "coil")
            .unwrap();

        let mut undo = MolUndoManager::new();
        assert!(!undo.is_undoable());

        // Perform an edit, recording before/after snapshots.
        let before = mol.residue_props().clone();
        mol.set_residue_prop_str("A", "1", "secondary", "helix")
            .unwrap();
        let after = mol.residue_props().clone();
        undo.record(Box::new(ResiduePropsEdit::new(
            before,
            after,
            "set secondary structure",
        )));

        assert!(undo.is_undoable());
        assert_eq!(undo.undo_desc(), Some("set secondary structure"));
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );

        // Undo reverts to the pre-edit value.
        assert!(undo.undo(&mut mol));
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("coil")
        );
        assert!(!undo.is_undoable());
        assert!(undo.is_redoable());

        // Redo re-applies the edit.
        assert!(undo.redo(&mut mol));
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );

        // A fresh edit clears the redo stack.
        undo.record(Box::new(ResiduePropsEdit::new(
            mol.residue_props().clone(),
            mol.residue_props().clone(),
            "noop",
        )));
        assert!(!undo.is_redoable());
    }

    #[test]
    fn empty_manager_undo_redo_are_noops() {
        let mut mol = MolCoord::new();
        let mut undo = MolUndoManager::new();
        assert!(!undo.undo(&mut mol));
        assert!(!undo.redo(&mut mol));
    }

    #[test]
    fn disabled_manager_records_nothing() {
        let mut undo = MolUndoManager::new();
        undo.set_disabled(true);
        undo.record(Box::new(ResiduePropsEdit::new(
            BTreeMap::new(),
            BTreeMap::new(),
            "x",
        )));
        assert!(!undo.is_undoable());
    }

    #[test]
    fn xform_edit_restores_only_selected_atom_positions() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "A", 1, "CA"));
        molecule.push_atom(atom(2, "B", 1, "CA"));
        molecule.atoms_mut()[0].position = Vector4::new3(1.0, 0.0, 0.0);
        molecule.atoms_mut()[1].position = Vector4::new3(2.0, 0.0, 0.0);
        let selector = crate::parse_selector("chain A").unwrap();

        let mut edit = MolXformEditInfo::new();
        edit.save_before_pos_sel(&molecule, &selector);
        molecule.atoms_mut()[0].position = Vector4::new3(10.0, 0.0, 0.0);
        molecule.atoms_mut()[1].position = Vector4::new3(20.0, 0.0, 0.0);
        edit.save_after_pos_sel(&molecule, &selector);

        assert_eq!(edit.before_positions().len(), 1);
        assert!(edit.undo(&mut molecule));
        assert_eq!(
            molecule.atom_by_serial(1).unwrap().position,
            Vector4::new3(1.0, 0.0, 0.0)
        );
        assert_eq!(
            molecule.atom_by_serial(2).unwrap().position,
            Vector4::new3(20.0, 0.0, 0.0)
        );
        assert_eq!(molecule.atoms_moved_generation(), 1);

        assert!(edit.redo(&mut molecule));
        assert_eq!(
            molecule.atom_by_serial(1).unwrap().position,
            Vector4::new3(10.0, 0.0, 0.0)
        );
        assert_eq!(molecule.atoms_moved_generation(), 2);
    }

    #[test]
    fn xform_edit_skips_atoms_removed_after_capture() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "A", 1, "CA"));
        molecule.push_atom(atom(2, "A", 1, "CB"));
        let mut edit = MolXformEditInfo::new();
        edit.save_before_pos(&molecule);
        molecule.delete_selected_atoms(&crate::parse_selector("aid 2").unwrap());

        assert!(edit.undo(&mut molecule));
        assert_eq!(molecule.atom_count(), 1);
        assert_eq!(molecule.atoms_moved_generation(), 1);
    }

    #[test]
    fn molecule_transform_records_xform_edit_in_undo_manager() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "A", 1, "CA"));
        molecule.atoms_mut()[0].position = Vector4::new3(1.0, 2.0, 3.0);
        let mut undo = MolUndoManager::new();

        let count = molecule.xform_by_mat_with_undo(
            cuemol_core::Matrix4::make_trans_mat(Vector4::new3(4.0, 0.0, 0.0)),
            None,
            &mut undo,
        );
        assert_eq!(count, 1);
        assert_eq!(undo.undo_desc(), Some("transform atoms"));
        assert_eq!(
            molecule.atom_by_serial(1).unwrap().position,
            Vector4::new3(5.0, 2.0, 3.0)
        );

        assert!(undo.undo(&mut molecule));
        assert_eq!(
            molecule.atom_by_serial(1).unwrap().position,
            Vector4::new3(1.0, 2.0, 3.0)
        );
        assert!(undo.redo(&mut molecule));
        assert_eq!(
            molecule.atom_by_serial(1).unwrap().position,
            Vector4::new3(5.0, 2.0, 3.0)
        );
    }

    #[test]
    fn bond_edit_undoes_and_redoes_both_directions() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "A", 1, "CA"));
        molecule.push_atom(atom(2, "A", 2, "CA"));

        let mut make = MolBondEditInfo::new(true, 1, 2);
        assert!(molecule.make_bond(1, 2, true));
        assert!(make.undo(&mut molecule));
        assert_eq!(molecule.bond_count(), 0);
        assert!(make.redo(&mut molecule));
        assert!(molecule.bonds()[0].persist);

        let mut remove = MolBondEditInfo::new(false, 1, 2);
        assert!(molecule.remove_bond(1, 2));
        assert!(remove.undo(&mut molecule));
        assert!(molecule.bonds()[0].persist);
        assert!(remove.redo(&mut molecule));
        assert_eq!(molecule.bond_count(), 0);
    }

    #[test]
    fn residue_relabel_edit_moves_properties_on_undo_and_redo() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "A", 1, "CA"));
        molecule
            .set_residue_prop_str("A", "1", "secondary", "Hs")
            .unwrap();
        let old = ResidueKey::new("A", 1, None);
        let new = ResidueKey::new("B", 5, None);
        assert!(molecule.relabel_residue_keys(|_| Some(new.clone())));

        let mut edit = ResidueRelabelEdit::new(vec![(old.clone(), new.clone())], "relabel");
        assert!(edit.undo(&mut molecule));
        assert_eq!(molecule.atoms()[0].chain_id, "A");
        assert_eq!(molecule.atoms()[0].res_id, 1);
        assert_eq!(
            molecule
                .get_residue_prop_str("A", "1", "secondary")
                .unwrap(),
            Some("Hs")
        );
        assert!(edit.redo(&mut molecule));
        assert_eq!(molecule.atoms()[0].chain_id, "B");
        assert_eq!(molecule.atoms()[0].res_id, 5);
        assert_eq!(
            molecule
                .get_residue_prop_str("B", "5", "secondary")
                .unwrap(),
            Some("Hs")
        );
    }
}
