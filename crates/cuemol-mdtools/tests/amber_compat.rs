use cuemol_core::Vector4;
use cuemol_mdtools::{
    apply_amber_mdcrd_frame, apply_amber_restart_str, read_amber_mdcrd_str, read_amber_prmtop_file,
    read_amber_prmtop_str, read_amber_prmtop_with_restart_files,
    read_amber_prmtop_with_restart_str, read_amber_restart_str,
};
use cuemol_molstr::{Atom, MolCoord};
use std::fs;
use std::process::Command;

const PRMTOP_WATER: &str = "\
%VERSION  VERSION_STAMP = V0001.000  DATE = 01/01/24  00:00:00
%FLAG TITLE
%FORMAT(20a4)
water test
%FLAG POINTERS
%FORMAT(10I8)
       3       2       2       0       0       0       0       0       0       0
       0       1       0       0       0       0       0       0       0       0
       0       0       0       0       0       0       0       0       3       0
       0
%FLAG ATOM_NAME
%FORMAT(20a4)
OW  HW1 HW2 
%FLAG CHARGE
%FORMAT(5E16.8)
 -8.34000000E-01  4.17000000E-01  4.17000000E-01
%FLAG ATOMIC_NUMBER
%FORMAT(10I8)
       8       1       1
%FLAG MASS
%FORMAT(5E16.8)
  1.60000000E+01  1.00800000E+00  1.00800000E+00
%FLAG RESIDUE_LABEL
%FORMAT(20a4)
WAT 
%FLAG RESIDUE_POINTER
%FORMAT(10I8)
       1
%FLAG BONDS_INC_HYDROGEN
%FORMAT(10I8)
       0       3       1       0       6       1
%FLAG BONDS_WITHOUT_HYDROGEN
%FORMAT(10I8)
%FLAG AMBER_ATOM_TYPE
%FORMAT(20a4)
OW  HW  HW  
";

const PRMTOP_WATER_NO_ATOMIC_NUMBER: &str = "\
%VERSION  VERSION_STAMP = V0001.000  DATE = 01/01/24  00:00:00
%FLAG POINTERS
%FORMAT(10I8)
       3       2       2       0       0       0       0       0       0       0
       0       1       0       0       0       0       0       0       0       0
       0       0       0       0       0       0       0       0       3       0
       0
%FLAG ATOM_NAME
%FORMAT(20a4)
OW  HW1 HW2 
%FLAG MASS
%FORMAT(5E16.8)
  1.60000000E+01  1.00800000E+00  1.00800000E+00
%FLAG RESIDUE_LABEL
%FORMAT(20a4)
WAT 
%FLAG RESIDUE_POINTER
%FORMAT(10I8)
       1
%FLAG BONDS_INC_HYDROGEN
%FORMAT(10I8)
       0       3       1       0       6       1
%FLAG AMBER_ATOM_TYPE
%FORMAT(20a4)
OW  HW  HW  
";

const INPCRD_WATER: &str = "\
water restart
    3
   1.0000000   2.0000000   3.0000000   1.5000000   2.0000000   3.0000000
   1.0000000   2.5000000   3.0000000
";

const INPCRD_WATER_BOX: &str = "\
water restart with box
    3
   1.0000000   2.0000000   3.0000000   1.5000000   2.0000000   3.0000000
   1.0000000   2.5000000   3.0000000
  10.0000000  11.0000000  12.0000000  90.0000000  90.0000000  90.0000000
";

const MDCRD_TWO_FRAMES_BOX: &str = "\
water trajectory
   1.000   2.000   3.000   1.500   2.000   3.000   1.000   2.500   3.000  10.000  11.000  12.000
   2.000   3.000   4.000   2.500   3.000   4.000   2.000   3.500   4.000  20.000  21.000  22.000
";

const MDCRD_D_EXPONENTS: &str = "\
two atoms
 1.0D+00 2.0D+00 3.0D+00 4.0D+00 5.0D+00 6.0D+00
";

#[test]
fn amber_restart_applies_positions_and_reads_box() {
    let mut molecule = three_atom_mol();
    let restart = apply_amber_restart_str(&mut molecule, INPCRD_WATER_BOX).unwrap();

    assert_eq!(restart.positions.len(), 3);
    assert_eq!(molecule.atoms()[0].position, Vector4::new3(1.0, 2.0, 3.0));
    assert_eq!(molecule.atoms()[1].position, Vector4::new3(1.5, 2.0, 3.0));
    assert_eq!(restart.cell.unwrap().a, 10.0);
}

#[test]
fn amber_restart_rejects_natom_mismatch() {
    let mut molecule = MolCoord::with_atoms(vec![test_atom("A0")]);
    let err = apply_amber_restart_str(&mut molecule, INPCRD_WATER).unwrap_err();

    assert!(err.to_string().contains("NATOM"));
}

#[test]
fn amber_prmtop_builds_molecule_bonds_and_coordinates() {
    let load = read_amber_prmtop_with_restart_str(PRMTOP_WATER, Some(INPCRD_WATER)).unwrap();
    let molecule = load.molecule;

    assert_eq!(molecule.atom_count(), 3);
    assert_eq!(molecule.bond_count(), 2);
    assert_eq!(molecule.atoms()[0].name, "OW");
    assert_eq!(molecule.atoms()[1].name, "HW1");
    assert_eq!(molecule.atoms()[0].element, "O");
    assert_eq!(molecule.atoms()[1].element, "H");
    assert_eq!(molecule.atoms()[0].res_name, "WAT");
    assert_eq!(molecule.atoms()[0].position, Vector4::new3(1.0, 2.0, 3.0));
}

#[test]
fn amber_prmtop_mass_fallback_resolves_elements() {
    let molecule = read_amber_prmtop_str(PRMTOP_WATER_NO_ATOMIC_NUMBER).unwrap();

    assert_eq!(molecule.atoms()[0].element, "O");
    assert_eq!(molecule.atoms()[1].element, "H");
    assert_eq!(molecule.atoms()[2].element, "H");
}

#[test]
fn amber_prmtop_file_accepts_gzip_compressed_input() {
    let raw_path = temp_file("amber-source.prmtop");
    let gz_path = temp_file("amber.prmtop.gz");
    fs::write(&raw_path, PRMTOP_WATER).unwrap();
    if !gzip_file(&raw_path, &gz_path) {
        let _ = fs::remove_file(raw_path);
        return;
    }

    let molecule = read_amber_prmtop_file(&gz_path).unwrap();
    assert_eq!(molecule.atom_count(), 3);
    assert_eq!(molecule.bond_count(), 2);

    let _ = fs::remove_file(raw_path);
    let _ = fs::remove_file(gz_path);
}

#[test]
fn amber_prmtop_file_accepts_bzip2_compressed_input() {
    let raw_path = temp_file("amber-source-bzip2.prmtop");
    let bz2_path = temp_file("amber.prmtop.bz2");
    fs::write(&raw_path, PRMTOP_WATER).unwrap();
    if !bzip2_file(&raw_path, &bz2_path) {
        let _ = fs::remove_file(raw_path);
        return;
    }

    let molecule = read_amber_prmtop_file(&bz2_path).unwrap();
    assert_eq!(molecule.atom_count(), 3);
    assert_eq!(molecule.bond_count(), 2);

    let _ = fs::remove_file(raw_path);
    let _ = fs::remove_file(bz2_path);
}

#[test]
fn amber_prmtop_with_restart_files_accepts_gzip_compressed_input() {
    let raw_prmtop_path = temp_file("amber-restart-source.prmtop");
    let raw_restart_path = temp_file("amber-restart-source.inpcrd");
    let gz_prmtop_path = temp_file("amber-with-restart.prmtop.gz");
    let gz_restart_path = temp_file("amber-with-restart.inpcrd.gz");
    fs::write(&raw_prmtop_path, PRMTOP_WATER).unwrap();
    fs::write(&raw_restart_path, INPCRD_WATER).unwrap();
    if !gzip_file(&raw_prmtop_path, &gz_prmtop_path)
        || !gzip_file(&raw_restart_path, &gz_restart_path)
    {
        let _ = fs::remove_file(raw_prmtop_path);
        let _ = fs::remove_file(raw_restart_path);
        return;
    }

    let load = read_amber_prmtop_with_restart_files(&gz_prmtop_path, &gz_restart_path).unwrap();
    assert_eq!(load.molecule.atom_count(), 3);
    assert_eq!(
        load.molecule.atoms()[0].position,
        Vector4::new3(1.0, 2.0, 3.0)
    );

    let _ = fs::remove_file(raw_prmtop_path);
    let _ = fs::remove_file(raw_restart_path);
    let _ = fs::remove_file(gz_prmtop_path);
    let _ = fs::remove_file(gz_restart_path);
}

#[test]
fn amber_mdcrd_reads_multiple_frames_and_box() {
    let trajectory = read_amber_mdcrd_str(MDCRD_TWO_FRAMES_BOX, 3, true).unwrap();

    assert_eq!(trajectory.frame_count(), 2);
    assert_eq!(trajectory.atom_count(), 3);
    assert!(trajectory.has_box);
    assert_eq!(
        trajectory.frames[0].positions[1],
        Vector4::new3(1.5, 2.0, 3.0)
    );
    assert_eq!(trajectory.frames[0].cell.as_ref().unwrap().a, 10.0);
    assert_eq!(trajectory.frames[1].cell.as_ref().unwrap().c, 22.0);
}

#[test]
fn amber_mdcrd_accepts_fortran_d_exponents_and_applies_frame() {
    let trajectory = read_amber_mdcrd_str(MDCRD_D_EXPONENTS, 2, false).unwrap();
    let mut molecule = MolCoord::with_atoms(vec![test_atom("A0"), test_atom("A1")]);

    apply_amber_mdcrd_frame(&mut molecule, &trajectory.frames[0]).unwrap();

    assert_eq!(trajectory.frame_count(), 1);
    assert_eq!(molecule.atoms()[0].position, Vector4::new3(1.0, 2.0, 3.0));
    assert_eq!(molecule.atoms()[1].position, Vector4::new3(4.0, 5.0, 6.0));
}

#[test]
fn amber_mdcrd_rejects_incomplete_frame_and_atom_mismatch() {
    let err = read_amber_mdcrd_str("bad\n   1.000   2.000\n", 1, false).unwrap_err();
    assert!(err.to_string().contains("value count"));

    let trajectory = read_amber_mdcrd_str(MDCRD_D_EXPONENTS, 2, false).unwrap();
    let mut molecule = MolCoord::with_atoms(vec![test_atom("A0")]);
    let err = apply_amber_mdcrd_frame(&mut molecule, &trajectory.frames[0]).unwrap_err();
    assert!(err.to_string().contains("NATOM"));
}

#[test]
fn amber_prmtop_accepts_fortran_d_exponents_in_real_sections() {
    let input = PRMTOP_WATER_NO_ATOMIC_NUMBER
        .replace("1.60000000E+01", "1.60000000D+01")
        .replace("1.00800000E+00", "1.00800000D+00");
    let molecule = read_amber_prmtop_str(&input).unwrap();

    assert_eq!(molecule.atoms()[0].element, "O");
    assert_eq!(molecule.atoms()[1].element, "H");
}

#[test]
fn amber_prmtop_rejects_old_label_less_format() {
    let err = read_amber_prmtop_str("test\n    3    1    2\n").unwrap_err();

    assert!(err.to_string().contains("Pre-Amber-7"));
}

#[test]
fn amber_restart_reader_reports_positions_without_applying() {
    let restart = read_amber_restart_str(INPCRD_WATER).unwrap();

    assert_eq!(restart.positions[2], Vector4::new3(1.0, 2.5, 3.0));
    assert!(restart.cell.is_none());
}

fn three_atom_mol() -> MolCoord {
    MolCoord::with_atoms(vec![test_atom("A0"), test_atom("A1"), test_atom("A2")])
}

fn temp_file(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cuemol2-rs-amber-{}-{name}", std::process::id()))
}

fn gzip_file(source_path: &std::path::Path, gz_path: &std::path::Path) -> bool {
    compress_file("gzip", source_path, gz_path)
}

fn bzip2_file(source_path: &std::path::Path, bz2_path: &std::path::Path) -> bool {
    compress_file("bzip2", source_path, bz2_path)
}

fn compress_file(
    program: &str,
    source_path: &std::path::Path,
    target_path: &std::path::Path,
) -> bool {
    let output = Command::new(program).arg("-c").arg(source_path).output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    fs::write(target_path, output.stdout).is_ok()
}

fn test_atom(name: &str) -> Atom {
    Atom {
        serial: 1,
        name: name.to_string(),
        alt_loc: None,
        res_name: "RES".to_string(),
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
