use cuemol_mdtools::{read_psf_str, sniff_psf_content, PsfReader};
use cuemol_molstr::ContentVerdict;

const WATER_PSF: &str = "\
PSF

       1 !NTITLE
 REMARKS water topology

       3 !NATOM
       1 SYS      1        WAT      OH2    OT     -0.834000       15.9994           0
       2 SYS      1        WAT      H1     HT      0.417000        1.0080           0
       3 SYS      1        WAT      H2     HT      0.417000        1.0080           0

       2 !NBOND: bonds
       1       2       1       3

       0 !NTHETA: angles
";

#[test]
fn psf_reader_matches_reference_atom_fields_and_mass_elements() {
    let mol = read_psf_str(WATER_PSF).unwrap();

    assert_eq!(mol.atom_count(), 3);
    assert_eq!(mol.bond_count(), 2);
    assert_eq!(mol.atoms()[0].serial, 1);
    assert_eq!(mol.atoms()[0].chain_id, "SYS");
    assert_eq!(mol.atoms()[0].res_id, 1);
    assert_eq!(mol.atoms()[0].res_name, "WAT");
    assert_eq!(mol.atoms()[0].name, "OH2");
    assert_eq!(mol.atoms()[0].element, "O");
    assert_eq!(mol.atoms()[1].element, "H");
    assert_eq!(mol.bonds()[0].atom1, 0);
    assert_eq!(mol.bonds()[0].atom2, 1);
    assert_eq!(mol.bonds()[1].atom1, 0);
    assert_eq!(mol.bonds()[1].atom2, 2);
}

#[test]
fn psf_reader_metadata_and_sniffer_match_qif_shape() {
    let reader = PsfReader::new();

    assert_eq!(reader.handler().name(), "psf");
    assert_eq!(
        reader.handler().type_descr(),
        "CHARMM/NAMD PSF topology (*.psf)"
    );
    assert_eq!(reader.handler().file_ext(), "*.psf");
    assert_eq!(sniff_psf_content(WATER_PSF), ContentVerdict::Yes);
    assert_eq!(sniff_psf_content("HEADER    TEST\n"), ContentVerdict::No);
}

#[test]
fn psf_reader_rejects_bad_counts_and_unknown_bond_atoms() {
    let err = read_psf_str("PSF\n\n       X !NATOM\n").unwrap_err();
    assert!(err.to_string().contains("NATOM"));

    let bad_bond = WATER_PSF.replace(
        "       1       2       1       3",
        "       1       2       1       4",
    );
    let err = read_psf_str(&bad_bond).unwrap_err();
    assert!(err.to_string().contains("unknown atom"));
}
