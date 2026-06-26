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

type ResiduePropsMap = BTreeMap<ResidueKey, BTreeMap<String, String>>;

/// An edit that changed the residue-property table (e.g. a secondary-structure
/// assignment). Port of C++ `Prot2ndryEditInfo`: it snapshots the table before and
/// after, restoring either on undo/redo.
pub struct ResiduePropsEdit {
    before: ResiduePropsMap,
    after: ResiduePropsMap,
    description: String,
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
}
