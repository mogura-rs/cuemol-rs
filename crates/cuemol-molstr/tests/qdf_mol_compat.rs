//! End-to-end check of the QDF molecule reader/writer (C++ `QdfMolReader` /
//! `QdfMolWriter` port) on a real protein: read 1CRN from PDB, round-trip it
//! through the native QDF container, and confirm the flat -> hierarchical ->
//! flat reconstruction preserves the structure.
//!
//! Self-skips when the C++ reference tree is absent (see CLAUDE.md). This is a
//! Rust-model round-trip — there is no `.qdf` fixture, so it does not prove
//! byte-compatibility with C++ CueMol.

use std::path::PathBuf;

use cuemol_molstr::{read_pdb_file, read_qdf_mol_bytes, write_qdf_mol_bytes};

fn reference_test_data(file_name: &str) -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data").join(file_name);
    path.exists().then_some(path)
}

#[test]
fn qdf_mol_roundtrips_1crn_through_the_native_container() {
    let Some(path) = reference_test_data("1CRN.pdb") else {
        return;
    };

    let src = read_pdb_file(path).unwrap();
    assert_eq!(src.atom_count(), 327);

    let bytes = write_qdf_mol_bytes(&src);
    let dst = read_qdf_mol_bytes(&bytes).expect("read back QDF molecule");

    // Atom/bond counts and the chain/residue grouping survive the round-trip.
    assert_eq!(dst.atom_count(), src.atom_count());
    assert_eq!(dst.bonds().len(), src.bonds().len());
    assert_eq!(dst.chain_ids(), src.chain_ids());
    assert_eq!(dst.residue_count(), src.residue_count());

    // DSSP secondary structure (computed by the PDB reader, Increment 1) is
    // carried through the native container as `$secondary` residue props.
    let src_helix = src
        .atoms()
        .iter()
        .filter(|a| src.residue_prop_for_atom(a, "secondary") == Some("helix"))
        .count();
    assert!(src_helix > 0, "1CRN should have helix residues");
    let dst_helix = dst
        .atoms()
        .iter()
        .filter(|a| dst.residue_prop_for_atom(a, "secondary") == Some("helix"))
        .count();
    assert_eq!(
        dst_helix, src_helix,
        "secondary structure lost on round-trip"
    );

    // Every atom's identity, residue assignment, and coordinates match.
    for (s, d) in src.atoms().iter().zip(dst.atoms().iter()) {
        assert_eq!(d.name, s.name);
        assert_eq!(d.chain_id, s.chain_id);
        assert_eq!(d.res_id, s.res_id);
        assert_eq!(d.res_name, s.res_name);
        assert_eq!(d.ins_code, s.ins_code);
        assert!((d.position.x - s.position.x).abs() < 1.0e-3);
        assert!((d.position.y - s.position.y).abs() < 1.0e-3);
        assert!((d.position.z - s.position.z).abs() < 1.0e-3);
        assert!((d.b_factor - s.b_factor).abs() < 1.0e-3);
        // Element survives as its canonical symbol (1CRN is C/N/O/S only).
        assert_eq!(
            d.element.to_ascii_uppercase(),
            s.element.to_ascii_uppercase()
        );
    }
}
