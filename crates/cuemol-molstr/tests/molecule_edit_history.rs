use cuemol_core::Vector4;
use cuemol_molstr::mol_undo::MolMergeEditInfo;
use cuemol_molstr::{
    parse_selector, write_pdb_string, Atom, MolCoord, MolEditInfo, MolUndoManager,
    MolXformEditInfo, Selector,
};
use std::collections::{BTreeMap, BTreeSet};

fn atom(serial: i32, chain: &str, residue: i32, name: &str, alt: Option<char>) -> Atom {
    Atom {
        serial,
        name: name.into(),
        alt_loc: alt,
        res_name: "ALA".into(),
        chain_id: chain.into(),
        res_id: residue,
        ins_code: None,
        position: Vector4::new3(2. * f64::from(residue), f64::from(serial) * 0.05, 0.),
        occupancy: 1.,
        b_factor: 10.,
        element: "C".into(),
        formal_charge: None,
        int_props: BTreeMap::new(),
        real_props: BTreeMap::new(),
    }
}

fn fixture() -> MolCoord {
    let mut molecule = MolCoord::new();
    for residue in 1..=5 {
        for (index, name) in ["N", "CA", "CB"].into_iter().enumerate() {
            molecule.push_atom(atom(
                (residue - 1) * 3 + index as i32 + 1,
                "A",
                residue,
                name,
                None,
            ));
        }
        molecule
            .set_residue_prop_str("A", &residue.to_string(), "secondary2", "H")
            .unwrap();
    }
    molecule
}

fn delete(molecule: &mut MolCoord, undo: &mut MolUndoManager, expression: &str) -> usize {
    let selector = parse_selector(expression).unwrap();
    let edit = MolMergeEditInfo::new_delete(molecule, &selector).unwrap();
    let count = molecule.delete_selected_atoms(&selector);
    undo.record(Box::new(edit));
    count
}

fn copy(molecule: &mut MolCoord, undo: &mut MolUndoManager, source: &MolCoord) -> Vec<i32> {
    let start = molecule.atom_count();
    molecule
        .copy_selected_atoms_from(source, &Selector::All)
        .unwrap();
    let ids = molecule.atoms()[start..]
        .iter()
        .map(|atom| atom.serial)
        .collect::<Vec<_>>();
    let selector = parse_selector(&format!(
        "aid {}",
        ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    ))
    .unwrap();
    undo.record(Box::new(
        MolMergeEditInfo::new_copy(molecule, &selector).unwrap(),
    ));
    ids
}

#[test]
fn partial_residue_undo_restores_identity_order_properties_and_coordinate_history() {
    let mut molecule = fixture();
    let before = molecule.clone();
    let pdb = write_pdb_string(&molecule);
    let mut undo = MolUndoManager::new();
    let mut moved = MolXformEditInfo::new();
    moved.save_before_pos(&molecule);
    for atom in molecule.atoms_mut() {
        atom.position.x += 10.;
    }
    moved.save_after_pos(&molecule);
    undo.record(Box::new(moved));
    assert_eq!(delete(&mut molecule, &mut undo, "resi 2:3 and name CA"), 2);
    assert!(molecule.atom_by_serial(8).is_none());
    assert!(undo.undo(&mut molecule));
    assert_eq!(
        molecule
            .atoms()
            .iter()
            .map(|atom| atom.serial)
            .collect::<Vec<_>>(),
        (1..=15).collect::<Vec<_>>()
    );
    assert_eq!(molecule.atom_by_serial(8).unwrap().position.x, 16.);
    assert_eq!(
        molecule
            .get_residue_prop_str("A", "3", "secondary2")
            .unwrap(),
        Some("H")
    );
    assert!(undo.undo(&mut molecule));
    assert_eq!(molecule.atoms(), before.atoms());
    assert_eq!(write_pdb_string(&molecule), pdb);
    assert!(undo.redo(&mut molecule));
    assert!(undo.redo(&mut molecule));
    assert!(molecule.atom_by_serial(8).is_none());
    assert_eq!(molecule.atom_count(), 13);
}

#[test]
fn delete_base_conformation_removes_and_restores_all_alternates() {
    let mut molecule = fixture();
    molecule.push_atom(atom(16, "A", 1, "CA", Some('A')));
    molecule.push_atom(atom(17, "A", 1, "CA", Some('B')));
    let before = molecule.clone();
    let mut undo = MolUndoManager::new();
    assert_eq!(delete(&mut molecule, &mut undo, "aid 2"), 3);
    for serial in [2, 16, 17] {
        assert!(molecule.atom_by_serial(serial).is_none());
    }
    assert!(undo.undo(&mut molecule));
    assert_eq!(molecule.atoms(), before.atoms());
    assert!(undo.redo(&mut molecule));
    assert_eq!(molecule.atom_count(), 14);
    let mut direct = before;
    assert!(direct.remove_atom(2));
    assert_eq!(direct.atom_count(), 14);
    assert!(!direct.remove_atom(2));
}

#[test]
fn merge_ids_stay_unique_across_coordinate_delete_and_merge_history() {
    let mut molecule = fixture();
    let source = MolCoord::with_atoms(vec![
        atom(1, "B", 1, "CA", None),
        atom(2, "B", 2, "CA", None),
    ]);
    let mut undo = MolUndoManager::new();
    let ids = copy(&mut molecule, &mut undo, &source);
    assert_eq!(
        molecule
            .atoms()
            .iter()
            .map(|atom| atom.serial)
            .collect::<BTreeSet<_>>()
            .len(),
        17
    );
    assert_eq!(ids, vec![16, 17]);
    let mut moved = MolXformEditInfo::new();
    moved.save_before_pos_sel(&molecule, &parse_selector("chain B").unwrap());
    molecule.atom_mut_by_serial(ids[0]).unwrap().position.x += 10.;
    moved.save_after_pos_sel(&molecule, &parse_selector("chain B").unwrap());
    undo.record(Box::new(moved));
    assert_eq!(delete(&mut molecule, &mut undo, "chain B"), 2);
    for _ in 0..3 {
        assert!(undo.undo(&mut molecule));
    }
    assert_eq!(molecule.atom_count(), 15);
    assert!(undo.redo(&mut molecule));
    assert_eq!(molecule.atom_by_serial(ids[0]).unwrap().position.x, 2.);
    assert!(undo.redo(&mut molecule));
    assert_eq!(molecule.atom_by_serial(ids[0]).unwrap().position.x, 12.);
    assert_eq!(molecule.atom_by_serial(1).unwrap().position.x, 2.);
    assert!(undo.redo(&mut molecule));
    assert_eq!(molecule.atom_count(), 15);
}

#[test]
fn atom_ids_are_not_reused_after_deletion_or_worker_transfer() {
    let mut molecule = fixture();
    assert!(molecule.remove_atom(15));
    let encoded = postcard::to_allocvec(&molecule.transfer_data()).unwrap();
    let data: cuemol_molstr::molecule::MoleculeData = postcard::from_bytes(&encoded).unwrap();
    let mut transferred = data.into_molecule().unwrap();
    for target in [&mut molecule, &mut transferred] {
        assert_eq!(
            target.append_atom1(atom(0, "B", 1, "X", None), "B", 1, "ALA"),
            16
        );
        assert_eq!(
            target.append_atom1(atom(15, "B", 2, "Y", None), "B", 2, "ALA"),
            17
        );
    }
}

#[test]
fn undo_restores_only_the_recorded_neighbourhood_and_rejects_collisions_atomically() {
    let mut molecule = fixture();
    let selector = parse_selector("(resi 3 around 2.1) and not resi 3").unwrap();
    let mut edit = MolMergeEditInfo::new_delete(&molecule, &selector).unwrap();
    let removed = molecule
        .select_indices(&selector)
        .into_iter()
        .map(|index| molecule.atoms()[index].serial)
        .collect::<BTreeSet<_>>();
    assert!(!removed.is_empty());
    molecule.delete_selected_atoms(&selector);
    molecule.atom_mut_by_serial(8).unwrap().position.x = 100.;
    assert!(edit.undo(&mut molecule));
    assert_eq!(molecule.atom_count(), 15);
    assert_eq!(molecule.atom_by_serial(8).unwrap().position.x, 100.);
    assert!(edit.redo(&mut molecule));
    assert!(removed
        .iter()
        .all(|serial| molecule.atom_by_serial(*serial).is_none()));
    let restored = fixture()
        .atoms()
        .iter()
        .find(|atom| removed.contains(&atom.serial))
        .unwrap()
        .clone();
    molecule.push_atom(restored);
    let unchanged = molecule.clone();
    assert!(!edit.undo(&mut molecule));
    assert_eq!(molecule, unchanged);
}

struct Step {
    fail_undo: bool,
    fail_redo: bool,
    panic: bool,
}
impl MolEditInfo for Step {
    fn undo(&mut self, molecule: &mut MolCoord) -> bool {
        self.run(molecule, self.fail_undo)
    }
    fn redo(&mut self, molecule: &mut MolCoord) -> bool {
        self.run(molecule, self.fail_redo)
    }
    fn description(&self) -> &str {
        "step"
    }
}
impl Step {
    fn run(&self, molecule: &mut MolCoord, fail: bool) -> bool {
        molecule.atoms_mut()[0].position.x += 1.;
        if fail && self.panic {
            panic!("test callback failure");
        }
        !fail
    }
}

#[test]
fn molecule_history_discards_failed_steps_and_can_record_again() {
    for redo in [false, true] {
        for panic in [false, true] {
            let mut molecule = fixture();
            let mut undo = MolUndoManager::new();
            undo.record(Box::new(Step {
                fail_undo: false,
                fail_redo: false,
                panic: false,
            }));
            undo.record(Box::new(Step {
                fail_undo: !redo,
                fail_redo: redo,
                panic,
            }));
            if redo {
                assert!(undo.undo(&mut molecule));
            }
            assert!(!if redo {
                undo.redo(&mut molecule)
            } else {
                undo.undo(&mut molecule)
            });
            assert_eq!((undo.undo_size(), undo.redo_size()), (0, 0));
            assert!(!undo.is_disabled());
            assert!(undo.take_history_dropped());
            assert!(!undo.take_history_dropped());
            undo.record(Box::new(Step {
                fail_undo: false,
                fail_redo: false,
                panic: false,
            }));
            assert!(undo.undo(&mut molecule));
        }
    }
}

#[test]
fn partial_restore_preserves_deposited_bonds_and_preflights_atom_names() {
    let mut molecule = fixture();
    molecule.add_bond_stereo(0, 4, cuemol_molstr::BondOrder::Double, 6);
    molecule.mark_bond_persistent(0, 4);
    let selector = parse_selector("aid 5").unwrap();
    let mut edit = MolMergeEditInfo::new_delete(&molecule, &selector).unwrap();
    molecule.delete_selected_atoms(&selector);
    assert!(edit.undo(&mut molecule));
    assert!(molecule.bonds().iter().any(|bond| bond.atom1 == 0
        && bond.atom2 == 4
        && bond.order == cuemol_molstr::BondOrder::Double
        && bond.stereo == 6
        && bond.persist));
    assert!(edit.redo(&mut molecule));
    molecule.push_atom(atom(99, "A", 2, "CA", None));
    let unchanged = molecule.clone();
    assert!(!edit.undo(&mut molecule));
    assert_eq!(molecule, unchanged);
}

#[test]
fn exhausted_ids_and_duplicate_copy_names_leave_the_target_unchanged() {
    let source = MolCoord::with_atoms(vec![atom(1, "B", 1, "CA", None)]);
    let mut molecule = MolCoord::with_atoms(vec![atom(i32::MAX, "A", 1, "N", None)]);
    let unchanged = molecule.clone();
    assert!(molecule
        .copy_selected_atoms_from(&source, &Selector::All)
        .is_err());
    assert_eq!(molecule, unchanged);
    assert_eq!(
        molecule.append_atom1(atom(0, "B", 1, "CA", None), "B", 1, "ALA"),
        -1
    );
    assert_eq!(molecule, unchanged);
    let mut molecule = fixture();
    let duplicate = MolCoord::with_atoms(vec![
        atom(1, "B", 1, "CA", None),
        atom(2, "B", 1, "CA", None),
    ]);
    let unchanged = molecule.clone();
    assert!(molecule
        .copy_selected_atoms_from(&duplicate, &Selector::All)
        .is_err());
    assert_eq!(molecule, unchanged);
}
