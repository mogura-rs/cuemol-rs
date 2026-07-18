use cuemol_core::Vector4;
use cuemol_mdtools::{
    apply_namd_coor_bytes, read_namd_coor_bytes, read_namd_coor_with_psf_bytes, NamdCoorReader,
};
use cuemol_molstr::{Atom, MolCoord};

#[test]
fn namd_coor_reader_reads_native_and_swapped_headers() {
    let native = read_namd_coor_bytes(&namd_bytes(&[[1.0, -2.0, 3.0]], false)).unwrap();
    assert_eq!(native.atom_count(), 1);
    assert!(!native.byte_swapped);
    assert_eq!(native.positions[0], Vector4::new3(1.0, -2.0, 3.0));

    let swapped = read_namd_coor_bytes(&namd_bytes(&[[4.0, 5.0, -6.0]], true)).unwrap();
    assert_eq!(swapped.atom_count(), 1);
    assert!(swapped.byte_swapped);
    assert_eq!(swapped.positions[0], Vector4::new3(4.0, 5.0, -6.0));
}

#[test]
fn namd_coor_reader_applies_positions_to_existing_molecule() {
    let mut molecule = MolCoord::with_atoms(vec![test_atom("CA"), test_atom("CB")]);
    let frame = apply_namd_coor_bytes(
        &mut molecule,
        &namd_bytes(&[[1.0, 2.0, 3.0], [-1.0, -2.0, -3.0]], false),
    )
    .unwrap();

    assert_eq!(frame.atom_count(), 2);
    assert_eq!(molecule.atoms()[0].position, Vector4::new3(1.0, 2.0, 3.0));
    assert_eq!(
        molecule.atoms()[1].position,
        Vector4::new3(-1.0, -2.0, -3.0)
    );
}

#[test]
fn namd_coor_reader_rejects_atom_count_mismatch() {
    let mut molecule = MolCoord::with_atoms(vec![test_atom("CA")]);
    let err = apply_namd_coor_bytes(&mut molecule, &namd_bytes(&[[1.0, 2.0, 3.0]; 2], false))
        .unwrap_err();

    assert!(err.to_string().contains("atom count mismatch"));
}

#[test]
fn namd_coor_reader_builds_molecule_from_psf_substream() {
    let load = read_namd_coor_with_psf_bytes(
        &namd_bytes(&[[1.0, 2.0, 3.0], [-1.0, -2.0, -3.0]], false),
        WATER_PSF,
    )
    .unwrap();

    assert_eq!(load.molecule.atom_count(), 2);
    assert_eq!(load.molecule.bond_count(), 1);
    assert_eq!(load.molecule.atoms()[0].name, "CA");
    assert_eq!(
        load.molecule.atoms()[0].position,
        Vector4::new3(1.0, 2.0, 3.0)
    );
    assert_eq!(
        load.molecule.atoms()[1].position,
        Vector4::new3(-1.0, -2.0, -3.0)
    );
    assert!(!load.frame.byte_swapped);
}

#[test]
fn namd_coor_reader_psf_substream_accessors_match_reference_key() {
    let mut reader = NamdCoorReader::new();

    assert_eq!(reader.topology_path(), "");
    reader.set_topology_path("/tmp/test.psf");
    assert_eq!(reader.topology_path(), "/tmp/test.psf");
}

#[test]
fn namd_coor_reader_rejects_psf_coor_atom_count_mismatch() {
    let err = read_namd_coor_with_psf_bytes(&namd_bytes(&[[1.0, 2.0, 3.0]], false), WATER_PSF)
        .unwrap_err();

    assert!(err.to_string().contains("atom count mismatch"));
}

fn namd_bytes(coords: &[[f64; 3]], swapped: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let natoms = coords.len() as i32;
    if swapped {
        bytes.extend_from_slice(&natoms.to_be_bytes());
        for [x, y, z] in coords {
            bytes.extend_from_slice(&x.to_be_bytes());
            bytes.extend_from_slice(&y.to_be_bytes());
            bytes.extend_from_slice(&z.to_be_bytes());
        }
    } else {
        bytes.extend_from_slice(&natoms.to_le_bytes());
        for [x, y, z] in coords {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&z.to_le_bytes());
        }
    }
    bytes
}

fn test_atom(name: &str) -> Atom {
    Atom {
        serial: 1,
        name: name.to_string(),
        alt_loc: None,
        res_name: "ALA".to_string(),
        chain_id: "A".to_string(),
        res_id: 1,
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

const WATER_PSF: &str = "\
PSF

       1 !NTITLE
 REMARKS two atom topology

       2 !NATOM
       1 A        1        ALA      CA     CT1     0.100000       12.0110           0
       2 A        1        ALA      CB     CT1    -0.100000       12.0110           0

       1 !NBOND: bonds
       1       2
";
