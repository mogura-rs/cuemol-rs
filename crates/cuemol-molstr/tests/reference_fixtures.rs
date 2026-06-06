use cuemol_molstr::{parse_selector, read_pdb_file};
use std::path::PathBuf;

#[test]
fn reads_1crn_from_reference_test_data_when_available() {
    let Some(path) = reference_test_data("1CRN.pdb") else {
        // The sibling C++ checkout is a development-time reference, not a crate fixture.
        return;
    };

    let mol = read_pdb_file(path).unwrap();
    assert_eq!(mol.atom_count(), 327);
    assert_eq!(mol.residue_count(), 46);
    assert_eq!(mol.chain_ids(), vec!["A"]);

    let selector = parse_selector("chain A and elem C").unwrap();
    let selected = mol.select_indices(&selector);
    assert!(!selected.is_empty());
    assert!(selected.len() < mol.atom_count());
}

fn reference_test_data(file_name: &str) -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data").join(file_name);
    path.exists().then_some(path)
}
