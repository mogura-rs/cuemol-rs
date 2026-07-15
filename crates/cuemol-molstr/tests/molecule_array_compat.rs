use cuemol_core::{ByteArray, ElementType};
use cuemol_molstr::{parse_selector, read_pdb_file};
use std::path::PathBuf;

#[test]
fn molecule_coordinate_array_matches_reference_shape_and_values() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mol = read_pdb_file(root.join("1CRN.pdb")).unwrap();
    let array = mol.crd_array();
    assert_eq!(array.elem_type(), ElementType::Float32);
    assert_eq!(array.elem_type_code(), ByteArray::FLOAT32);
    assert_eq!(array.elem_count(), mol.atom_count() * 3);
    assert_eq!(array.length(), mol.atom_count() * 3 * 4);
    assert_eq!(array.byte_len(), 327 * 3 * 4);

    let values = array.copy_to_f32().unwrap();
    assert_eq!(values.len(), 981);
    assert!((values[0] - 17.047).abs() < 1.0e-5);
    assert!((values[1] - 14.099).abs() < 1.0e-5);
    assert!((values[2] - 3.625).abs() < 1.0e-5);
}

#[test]
fn molecule_property_arrays_match_reference_values() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mol = read_pdb_file(root.join("1CRN.pdb")).unwrap();

    let bfac = mol.prop_array("bfac");
    assert_eq!(bfac.elem_type_code(), ByteArray::FLOAT32);
    assert_eq!(bfac.elem_count(), 327);
    assert_eq!(bfac.length(), 327 * 4);
    assert!((bfac.copy_to_f32().unwrap()[0] - 13.79).abs() < 1.0e-5);

    let occ = mol.prop_array("occ");
    assert_eq!(occ.elem_type_code(), ByteArray::FLOAT32);
    assert_eq!(occ.elem_count(), 327);
    assert_eq!(occ.length(), 327 * 4);
    assert!((occ.copy_to_f32().unwrap()[0] - 1.0).abs() < 1.0e-6);

    let unknown = mol.prop_array("unknown");
    assert_eq!(unknown.elem_count(), 327);
    assert_eq!(unknown.length(), 327 * 4);
    assert!(unknown
        .copy_to_f32()
        .unwrap()
        .iter()
        .all(|value| *value == 0.0));
}

#[test]
fn molecule_selection_array_uses_zero_based_atom_indices() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mol = read_pdb_file(root.join("1CRN.pdb")).unwrap();
    let selector = parse_selector("chain A and name CA").unwrap();
    let array = mol.sel_array(&selector);
    assert_eq!(array.elem_type_code(), ByteArray::INT32);
    let values = array.copy_to_i32().unwrap();
    assert_eq!(values.len(), 46);
    assert_eq!(values[0], 1);
    assert_eq!(values[1], 8);
}

#[test]
fn molecule_name_candidate_json_matches_reference_sets() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mol = read_pdb_file(root.join("1CRN.pdb")).unwrap();
    assert_eq!(mol.chain_name_candidates_json(), "[\"A\"]");
    assert_eq!(
        mol.element_name_candidates_json(),
        "[\"C\",\"N\",\"O\",\"S\"]"
    );
    assert!(mol.residue_name_candidates().contains(&"THR".to_string()));
    assert!(mol.atom_name_candidates().contains(&"CA".to_string()));
}

fn reference_test_data_root() -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data");
    path.exists().then_some(path)
}
