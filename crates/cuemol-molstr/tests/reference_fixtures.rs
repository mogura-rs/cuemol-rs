use cuemol_molstr::{parse_selector, read_mmcif_file, read_pdb_file};
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

#[test]
fn pdb_and_mmcif_agree_on_1crn_crystal_cell() {
    // The PDB CRYST1 record and the mmCIF _cell/_symmetry describe the same
    // crystal, so the two readers must produce the same unit cell + space group.
    // (The PDB reader used to drop CRYST1, so cell_params() was None for a .pdb.)
    let (Some(pdb), Some(cif)) = (
        reference_test_data("1CRN.pdb"),
        reference_test_data("1crn.cif"),
    ) else {
        return;
    };
    let pdb_cell = read_pdb_file(pdb)
        .unwrap()
        .cell_params()
        .cloned()
        .expect("PDB CRYST1 -> cell params");
    let cif_cell = read_mmcif_file(cif)
        .unwrap()
        .cell_params()
        .cloned()
        .expect("mmCIF _cell -> cell params");
    for (p, c) in [
        (pdb_cell.a, cif_cell.a),
        (pdb_cell.b, cif_cell.b),
        (pdb_cell.c, cif_cell.c),
        (pdb_cell.alpha, cif_cell.alpha),
        (pdb_cell.beta, cif_cell.beta),
        (pdb_cell.gamma, cif_cell.gamma),
    ] {
        assert!((p - c).abs() < 1.0e-3, "cell mismatch: {p} vs {c}");
    }
    assert_eq!(pdb_cell.space_group_name, cif_cell.space_group_name);
    assert_eq!(pdb_cell.space_group_name, "P 1 21 1");
}

fn reference_test_data(file_name: &str) -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data").join(file_name);
    path.exists().then_some(path)
}
