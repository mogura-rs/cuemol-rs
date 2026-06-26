use cuemol_molstr::{
    read_mmcif_file, read_mmcif_map_file, read_opendx_file, read_pdb_file, read_sdf_file,
    write_sdf_string, MmcifMapStatus,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn mmcif_reader_matches_reference_fixture_counts() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    // Bond counts come from the residue-topology dictionary (the same path the PDB
    // reader uses); 1crn.cif's 337 matches 1CRN.pdb exactly — a cross-format
    // consistency check on the same molecule (template bonds + correct orders +
    // peptide links + the 3 disulfides + the C-terminal OXT distance bond).
    for (file, atoms, bonds) in [
        ("1crn.cif", 327, 337),
        ("3RXN_mod.cif", 9, 7),
        ("label_conn.mmcif", 13, 12),
    ] {
        let mol = read_mmcif_file(root.join(file)).unwrap();
        assert_eq!(mol.atom_count(), atoms, "{file}");
        assert_eq!(mol.bond_count(), bonds, "{file}");
    }
}

#[test]
fn mmcif_reader_loads_secondary_structure_from_struct_conf_and_sheet() {
    let Some(root) = reference_test_data_root() else {
        return;
    };
    let mol = read_mmcif_file(root.join("1crn.cif")).unwrap();
    let ss = |resid: i32| {
        mol.get_residue_prop_str("A", &resid.to_string(), "secondary")
            .ok()
            .flatten()
            .map(str::to_string)
    };
    // _struct_conf helices H1 (auth seq 7-19) and H2 (23-30).
    assert_eq!(ss(7).as_deref(), Some("helix"));
    assert_eq!(ss(10).as_deref(), Some("helix"));
    assert_eq!(ss(19).as_deref(), Some("helix"));
    assert_eq!(ss(25).as_deref(), Some("helix"));
    // _struct_sheet_range strands (1-4) and (32-35).
    assert_eq!(ss(1).as_deref(), Some("sheet"));
    assert_eq!(ss(4).as_deref(), Some("sheet"));
    assert_eq!(ss(33).as_deref(), Some("sheet"));
    // Coil residues between the SSEs carry no secondary assignment.
    assert_eq!(ss(5), None);
    assert_eq!(ss(22), None);
}

#[test]
fn sdf_reader_writer_matches_reference_fixture_shape() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mol = read_sdf_file(root.join("test1.sdf")).unwrap();
    assert_eq!(mol.atom_count(), 37);
    assert_eq!(mol.bond_count(), 40);
    let output = write_sdf_string(&mol);
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 83);
    assert_eq!(lines[3], " 37 40  0  0  0  0  0  0  0  0999 V2000");
    assert_eq!(lines[81], "M  END");
    assert_eq!(lines[82], "$$$$");

    let charged = read_sdf_file(root.join("test_chg1.sdf")).unwrap();
    assert_eq!(charged.atom_count(), 59);
    assert_eq!(charged.bond_count(), 62);
    let output = write_sdf_string(&charged);
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 128);
    assert_eq!(lines[3], " 59 62  0  0  0  0  0  0  0  0999 V2000");
    assert_eq!(lines[125], "M  CHG  1  17   1");
    assert_eq!(lines[126], "M  END");
    assert_eq!(lines[127], "$$$$");
}

#[test]
fn opendx_reader_matches_reference_fixture_shape() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let map = read_opendx_file(root.join("small_tabsep_opendx.dx")).unwrap();
    assert_eq!(map.counts, [3, 3, 3]);
    assert_eq!(map.value_count(), 27);
    assert_eq!(map.origin.x, 0.420002);
    assert_eq!(map.origin.y, 0.260003);
    assert_eq!(map.origin.z, 4.505003);
}

#[test]
fn pdb_reader_accepts_gzip_compressed_file() {
    let raw_path = temp_file("compressed-source.pdb");
    let gz_path = temp_file("compressed.pdb.gz");
    fs::write(
        &raw_path,
        "ATOM      1  N   GLY A   1       1.000   2.000   3.000  1.00 10.00           N\nEND\n",
    )
    .unwrap();

    let output = Command::new("gzip").arg("-c").arg(&raw_path).output();
    let Ok(output) = output else {
        let _ = fs::remove_file(raw_path);
        return;
    };
    if !output.status.success() {
        let _ = fs::remove_file(raw_path);
        return;
    }
    fs::write(&gz_path, output.stdout).unwrap();

    let mol = read_pdb_file(&gz_path).unwrap();
    assert_eq!(mol.atom_count(), 1);
    assert_eq!(mol.atoms()[0].name, "N");

    let _ = fs::remove_file(raw_path);
    let _ = fs::remove_file(gz_path);
}

#[test]
fn pdb_reader_accepts_bzip2_compressed_file() {
    let raw_path = temp_file("compressed-source-bzip2.pdb");
    let bz2_path = temp_file("compressed.pdb.bz2");
    fs::write(
        &raw_path,
        "ATOM      1  N   GLY A   1       1.000   2.000   3.000  1.00 10.00           N\nEND\n",
    )
    .unwrap();

    let output = Command::new("bzip2").arg("-c").arg(&raw_path).output();
    let Ok(output) = output else {
        let _ = fs::remove_file(raw_path);
        return;
    };
    if !output.status.success() {
        let _ = fs::remove_file(raw_path);
        return;
    }
    fs::write(&bz2_path, output.stdout).unwrap();

    let mol = read_pdb_file(&bz2_path).unwrap();
    assert_eq!(mol.atom_count(), 1);
    assert_eq!(mol.atoms()[0].name, "N");

    let _ = fs::remove_file(raw_path);
    let _ = fs::remove_file(bz2_path);
}

#[test]
fn mmcif_map_reader_reports_lfs_pointer_fixture() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let status = read_mmcif_map_file(root.join("2ydo_test.cif.gz")).unwrap();
    match status {
        MmcifMapStatus::GitLfsPointer { oid, size } => {
            assert_eq!(
                oid,
                "26d7f08d58a4c9bd7b8d31212a8387650d56cf5342813a1bfb06e4552580433e"
            );
            assert_eq!(size, 124063);
        }
        MmcifMapStatus::ParsedPlaceholder => panic!("expected Git LFS pointer"),
    }
}

fn temp_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cuemol2-rs-importer-{}-{name}", std::process::id()))
}

fn reference_test_data_root() -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data");
    path.exists().then_some(path)
}
