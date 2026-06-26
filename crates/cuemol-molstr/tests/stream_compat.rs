use cuemol_molstr::{HandlerCategory, HandlerSniffer, StreamHandlerInfo, StreamManager};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn stream_manager_constants_match_reference_values() {
    let manager = StreamManager::new();
    assert_eq!(manager.cat_objreader(), 0);
    assert_eq!(manager.cat_objwriter(), 1);
    assert_eq!(manager.cat_rendtofile(), 2);
    assert_eq!(manager.cat_scereader(), 3);
    assert_eq!(manager.cat_scewriter(), 4);
    assert_eq!(manager.comp_none(), 0);
    assert_eq!(manager.comp_gzip(), 1);
    assert_eq!(manager.comp_bzip2(), 2);
    assert_eq!(manager.comp_xzip(), 3);
}

#[test]
fn stream_manager_default_registry_matches_reference_handlers() {
    let manager = StreamManager::new();
    assert!(manager.is_reader_registered("cuemol_rs::SceneXMLReader"));
    assert!(manager.is_reader_registered("cuemol_rs::SceneXMLWriter"));
    assert!(!manager.is_reader_registered("__nonexistent__"));
    assert!(manager
        .handler_by_nickname("qsc_xml", HandlerCategory::SceneReader)
        .is_some());
    assert!(manager
        .handler_by_nickname("psefile", HandlerCategory::SceneReader)
        .is_some());
    assert!(manager
        .handler_by_nickname("qsc_xml", HandlerCategory::ObjReader)
        .is_none());

    let info_json = manager.get_info_json2();
    assert!(info_json.contains("\"name\":\"pdb\""));
    assert!(info_json.contains("\"name\":\"qdfpdb\""));
    assert!(info_json.contains("\"name\":\"mmcif\""));
    assert!(info_json.contains("\"name\":\"mol2\""));
    assert!(info_json.contains("\"name\":\"gro\""));
    assert!(info_json.contains("\"name\":\"psf\""));
    assert!(info_json.contains("\"name\":\"amberprm\""));
    assert!(info_json.contains("\"name\":\"namdcoor\""));
    assert!(info_json.contains("\"name\":\"ccp4map\""));
    assert!(info_json.contains("\"name\":\"xplormap\""));
    assert!(info_json.contains("\"name\":\"brix\""));
    assert!(info_json.contains("\"name\":\"mtzmap\""));
    assert!(info_json.contains("\"name\":\"msms\""));
    assert!(info_json.contains("\"name\":\"qdfsurf\""));
    assert!(info_json.contains("\"name\":\"qdfpot\""));
    assert!(info_json.contains("\"name\":\"qsc_xml\""));
    assert!(info_json.contains("\"name\":\"psefile\""));
    assert!(info_json.contains("\"category\":3"));

    let reader_info = manager.get_reader_info_json();
    assert!(reader_info.contains("size: 18"));
    assert!(reader_info.contains("\"PDB Coordinates (*.pdb;*.ent;*.pdb.gz)\""));
    assert!(reader_info.contains("\"QDF-PDB (*.qdfpdb)\""));
    assert!(reader_info.contains("\"mmcifmap\""));
    assert!(reader_info.contains("\"mol2\""));
    assert!(reader_info.contains("\"gro\""));
    assert!(reader_info.contains("\"psf\""));
    assert!(reader_info.contains("\"amberprm\""));
    assert!(reader_info.contains("\"namdcoor\""));
    assert!(reader_info.contains("\"ccp4map\""));
    assert!(reader_info.contains("\"CCP4 Density Map(*.map;*.ccp4;*.mrc;*.ccp4.gz)\""));
    assert!(reader_info.contains("\"xplormap\""));
    assert!(reader_info.contains("\"XPLOR/CNS Density Map(*.map;*.cns)\""));
    assert!(reader_info.contains("\"brix\""));
    assert!(reader_info.contains("\"BRIX Density Map(*.brix;*.omap)\""));
    assert!(reader_info.contains("\"mtzmap\""));
    assert!(reader_info.contains("\"MTZ Structure Factor (*.mtz)\""));
    assert!(reader_info.contains("\"msms\""));
    assert!(reader_info.contains("\"CueMol molsurf file (*.qdf)\""));
    assert!(reader_info.contains("\"CueMol elepot file (*.qdf)\""));

    let writer_info = manager.get_writer_info_json();
    assert!(writer_info.contains("size: 6"));
    assert!(writer_info.contains("\"PDB coordinates (*.pdb)\""));
    assert!(writer_info.contains("\"XYZR coordinates(*.xyzr)\""));
    assert!(writer_info.contains("\"PQR coordinates(*.pqr)\""));
    assert!(writer_info.contains("\"qdfsurf\""));
    assert!(writer_info.contains("\"qdfpot\""));
}

#[test]
fn stream_manager_duplicate_registration_errors_and_unregister_removes_handlers() {
    let mut manager = StreamManager::new();
    let duplicate = manager.handlers()[0].clone();
    assert!(manager.register_handler(duplicate).is_err());
    assert!(manager.unregister_reader("cuemol_rs::PDBFileReader", false));
    assert!(!manager.is_reader_registered("cuemol_rs::PDBFileReader"));
    assert!(manager.unregister_reader("cuemol_rs::PDBFileWriter", true));
    assert!(!manager.is_reader_registered("cuemol_rs::PDBFileWriter"));
    assert!(!manager.unregister_reader("cuemol_rs::PDBFileReader", false));
}

#[test]
fn stream_manager_searches_reference_fixture_content() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let manager = StreamManager::new();
    assert_eq!(
        manager.search_reader_by_content(
            root.join("1CRN.pdb"),
            "",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "pdb"
    );
    assert_eq!(
        manager.search_reader_by_content(
            root.join("1crn.cif"),
            "",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "mmcif"
    );
    assert_eq!(
        manager.search_reader_by_content(
            root.join("test1.sdf"),
            "",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "sdf"
    );
    assert_eq!(
        manager.search_reader_by_content(
            root.join("small_tabsep_opendx.dx"),
            "",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "apbs"
    );
}

#[test]
fn stream_manager_searches_pse_scene_content() {
    let manager = StreamManager::new();
    let path = temp_file("stream-session.pse");
    fs::write(&path, [0x80_u8, 0x04, b'}', b'q', 0x00, b'.']).unwrap();

    assert_eq!(
        manager.search_reader_by_content(&path, "", HandlerCategory::SceneReader, false, 0),
        "psefile"
    );
    assert_eq!(
        manager.sniff_content(&path, HandlerSniffer::Pse, 0),
        cuemol_molstr::ContentVerdict::Yes
    );

    let _ = fs::remove_file(path);
}

#[test]
fn stream_manager_search_filters_and_edge_cases_match_reference_semantics() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let manager = StreamManager::new();
    assert_eq!(
        manager.search_reader_by_content(
            root.join("1crn.cif"),
            " pdb, mmcif ",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "mmcif"
    );
    assert_eq!(
        manager.search_reader_by_content(
            root.join("1CRN.pdb"),
            "__unknown__",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        ""
    );
    assert_eq!(
        manager.search_reader_by_content(
            root.join("missing.pdb"),
            "",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        ""
    );

    let empty = temp_file("stream-empty.tmp");
    fs::write(&empty, "").unwrap();
    assert_eq!(
        manager.search_reader_by_content(&empty, "", HandlerCategory::ObjReader, false, 0),
        ""
    );
    let _ = fs::remove_file(empty);

    let gz_path = temp_file("stream-sample.pdb.gz");
    fs::write(&gz_path, "HEADER    TEST\n").unwrap();
    assert_eq!(
        manager.search_reader_by_content(&gz_path, "", HandlerCategory::ObjReader, false, 0),
        ""
    );
    let _ = fs::remove_file(gz_path);
}

#[test]
fn stream_manager_search_decompresses_gzip_content_when_enabled() {
    let manager = StreamManager::new();
    let raw_path = temp_file("stream-compressed-source.pdb");
    let gz_path = temp_file("stream-compressed.pdb.gz");
    let bz2_path = temp_file("stream-compressed.pdb.bz2");
    fs::write(&raw_path, "HEADER    TEST\n").unwrap();
    let Some(compressed) = gzip_compress(&raw_path) else {
        let _ = fs::remove_file(raw_path);
        return;
    };
    fs::write(&gz_path, compressed).unwrap();

    assert_eq!(
        manager.search_reader_by_content(&gz_path, "", HandlerCategory::ObjReader, false, 0),
        ""
    );
    assert_eq!(
        manager.search_reader_by_content(&gz_path, "", HandlerCategory::ObjReader, true, 0),
        "pdb"
    );

    if let Some(compressed) = bzip2_compress(&raw_path) {
        fs::write(&bz2_path, compressed).unwrap();
        assert_eq!(
            manager.search_reader_by_content(&bz2_path, "", HandlerCategory::ObjReader, false, 0),
            ""
        );
        assert_eq!(
            manager.search_reader_by_content(&bz2_path, "", HandlerCategory::ObjReader, true, 0),
            "pdb"
        );
        let _ = fs::remove_file(&bz2_path);
    }

    let _ = fs::remove_file(raw_path);
    let _ = fs::remove_file(gz_path);
}

#[test]
fn stream_manager_searches_mmcif_map_content() {
    let manager = StreamManager::new();
    let path = temp_file("stream-map.cif");
    fs::write(&path, "data_x\n#\n_refln.index_h\n").unwrap();

    assert_eq!(
        manager.search_reader_by_content(&path, "", HandlerCategory::ObjReader, false, 0),
        "mmcifmap"
    );
    assert_eq!(
        manager.search_readers_by_content(
            &path,
            "mmcif,mmcifmap",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "mmcifmap"
    );
    let _ = fs::remove_file(path);
}

#[test]
fn stream_manager_searches_mol2_content() {
    let manager = StreamManager::new();
    let path = temp_file("stream-ligand.mol2");
    fs::write(
        &path,
        "# generated\n@<TRIPOS>MOLECULE\nligand\n3 2 1 0 0\nSMALL\n",
    )
    .unwrap();

    assert_eq!(
        manager.search_reader_by_content(&path, "", HandlerCategory::ObjReader, false, 0),
        "mol2"
    );
    assert_eq!(
        manager.sniff_content(&path, HandlerSniffer::Mol2, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(path);
}

#[test]
fn stream_manager_searches_gro_content() {
    let manager = StreamManager::new();
    let path = temp_file("stream-water.gro");
    fs::write(
        &path,
        "water\n    1\n    1SOL     OW    1   1.000   2.000   3.000\n   2.00000   2.00000   2.00000\n",
    )
    .unwrap();

    assert_eq!(
        manager.search_reader_by_content(&path, "", HandlerCategory::ObjReader, false, 0),
        "gro"
    );
    assert_eq!(
        manager.sniff_content(&path, HandlerSniffer::Gro, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(path);
}

#[test]
fn stream_manager_searches_psf_content() {
    let manager = StreamManager::new();
    let path = temp_file("stream-water.psf");
    fs::write(
        &path,
        "PSF\n\n       1 !NTITLE\n REMARKS test\n\n       1 !NATOM\n       1 SYS      1        WAT      OH2    OT     -0.834000       15.9994           0\n",
    )
    .unwrap();

    assert_eq!(
        manager.search_reader_by_content(&path, "", HandlerCategory::ObjReader, false, 0),
        "psf"
    );
    assert_eq!(
        manager.sniff_content(&path, HandlerSniffer::Psf, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(path);
}

#[test]
fn stream_manager_searches_reference_importer_sniffers() {
    let manager = StreamManager::new();

    let amber = temp_file("stream-topology.prmtop");
    fs::write(
        &amber,
        "%FLAG POINTERS\n%FORMAT(10I8)\n       3       2       2\n",
    )
    .unwrap();
    assert_eq!(
        manager.search_reader_by_content(&amber, "", HandlerCategory::ObjReader, false, 0),
        "amberprm"
    );
    assert_eq!(
        manager.sniff_content(&amber, HandlerSniffer::AmberPrmtop, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(amber);

    let namd = temp_file("stream-coordinates.coor");
    fs::write(&namd, namd_header(100, 1.0, -2.0, 3.0, false)).unwrap();
    assert_eq!(
        manager.search_reader_by_content(&namd, "", HandlerCategory::ObjReader, false, 0),
        "namdcoor"
    );
    assert_eq!(
        manager.sniff_content(&namd, HandlerSniffer::NamdCoor, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(namd);

    let swapped_namd = temp_file("stream-coordinates-swapped.coor");
    fs::write(&swapped_namd, namd_header(100, 1.0, -2.0, 3.0, true)).unwrap();
    assert_eq!(
        manager.sniff_content(&swapped_namd, HandlerSniffer::NamdCoor, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(swapped_namd);

    let ccp4 = temp_file("stream-density.map");
    let mut ccp4_bytes = vec![0_u8; 216];
    ccp4_bytes[208..212].copy_from_slice(b"MAP ");
    fs::write(&ccp4, ccp4_bytes).unwrap();
    assert_eq!(
        manager.search_reader_by_content(
            &ccp4,
            "ccp4map,xplormap",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "ccp4map"
    );
    let _ = fs::remove_file(ccp4);

    let xplor = temp_file("stream-density-xplor.map");
    fs::write(
        &xplor,
        "\n       2\n REMARKS test xplor map\n REMARKS more\n       1       1       1       1       1       1       1       1       1\nZYX\n",
    )
    .unwrap();
    assert_eq!(
        manager.search_reader_by_content(
            &xplor,
            "ccp4map,xplormap",
            HandlerCategory::ObjReader,
            false,
            0
        ),
        "xplormap"
    );
    let _ = fs::remove_file(xplor);

    let brix = temp_file("stream-density.brix");
    fs::write(&brix, "   :-) origin 0 0 0 extent 10 10 10\n").unwrap();
    assert_eq!(
        manager.search_reader_by_content(&brix, "", HandlerCategory::ObjReader, false, 0),
        "brix"
    );
    let _ = fs::remove_file(brix);

    let mtz = temp_file("stream-coeffs.mtz");
    let mut mtz_bytes = b"MTZ ".to_vec();
    mtz_bytes.resize(64, 0);
    fs::write(&mtz, mtz_bytes).unwrap();
    assert_eq!(
        manager.search_reader_by_content(&mtz, "", HandlerCategory::ObjReader, false, 0),
        "mtzmap"
    );
    let _ = fs::remove_file(mtz);

    let msms = temp_file("stream-surface.vert");
    fs::write(
        &msms,
        "# MSMS solvent excluded surface vertices ... radius = 1.500\n#faces  #sphere density probe_r\n",
    )
    .unwrap();
    assert_eq!(
        manager.search_reader_by_content(&msms, "", HandlerCategory::ObjReader, false, 0),
        "msms"
    );
    let _ = fs::remove_file(msms);

    let qdfsurf = temp_file("stream-surface.qdf");
    fs::write(&qdfsurf, qdf_header("SRF1")).unwrap();
    assert_eq!(
        manager.search_reader_by_content(&qdfsurf, "", HandlerCategory::ObjReader, false, 0),
        "qdfsurf"
    );
    assert_eq!(
        manager.sniff_content(&qdfsurf, HandlerSniffer::QdfSurf, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    assert_eq!(
        manager.sniff_content(&qdfsurf, HandlerSniffer::QdfPot, 0),
        cuemol_molstr::ContentVerdict::No
    );
    let _ = fs::remove_file(qdfsurf);

    let qdfpot = temp_file("stream-potential.qdf");
    fs::write(&qdfpot, qdf_header("POT1")).unwrap();
    assert_eq!(
        manager.search_reader_by_content(&qdfpot, "", HandlerCategory::ObjReader, false, 0),
        "qdfpot"
    );
    assert_eq!(
        manager.sniff_content(&qdfpot, HandlerSniffer::QdfPot, 0),
        cuemol_molstr::ContentVerdict::Yes
    );
    let _ = fs::remove_file(qdfpot);
}

#[test]
fn stream_manager_accepts_custom_handler_registration() {
    let mut manager = StreamManager::empty();
    manager
        .register_handler(StreamHandlerInfo::new(
            "custom::Pdb",
            "pdb",
            "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)",
            "*.pdb; *.ent; *.pdb.gz",
            HandlerCategory::ObjReader,
            Some(HandlerSniffer::Pdb),
        ))
        .unwrap();
    assert!(manager.is_reader_registered("custom::Pdb"));
    assert!(manager
        .register_handler(manager.handlers()[0].clone())
        .is_err());
}

#[test]
fn stream_manager_async_load_buffers_supplied_chunks() {
    let mut manager = StreamManager::new();
    let reader = "pdb";
    let id1 = manager.load_object_async(&reader);
    let id2 = manager.load_object_async(&reader);
    assert!(id1 > 0);
    assert!(id2 > id1);
    assert!(manager.supply_data_async(id1, b"ATOM"));
    assert!(manager.supply_data_async(id1, b" END"));
    assert_eq!(manager.async_load(id1).unwrap().byte_len(), 8);

    let data = manager.wait_load_async(id1).unwrap();

    assert_eq!(data.as_bytes(), b"ATOM END");
    assert!(manager.async_load(id1).is_none());
    assert!(manager.wait_load_async(99999).is_none());
    assert!(!manager.supply_data_async(99999, b"ignored"));
    assert!(manager.wait_load_async(id2).is_some());
}

#[test]
fn stream_manager_xml_helpers_preserve_basic_metadata() {
    let manager = StreamManager::new();
    let value = 10_i32;
    assert_eq!(manager.find_compatible_writer_names_for_obj(0), "");
    assert!(manager
        .find_compatible_writer_names_for_obj(1)
        .contains("pdb"));
    let xml = manager.to_xml_2(&value, "Camera");
    assert_eq!(
        manager.from_xml(&xml, 0).unwrap(),
        "<object type=\"Camera\"/>"
    );
    let array_xml = manager.array_to_xml(&[1_i32, 2_i32]);
    assert_eq!(
        manager.from_xml(&array_xml, 0).unwrap(),
        "<array type=\"i32\" size=\"2\"/>"
    );
    let group_xml = manager.rend_grp_to_xml(&[1_i32], "grp&1");
    assert_eq!(manager.rend_array_from_xml(&group_xml, 0), vec!["grp&1"]);
}

fn reference_test_data_root() -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data");
    path.exists().then_some(path)
}

fn temp_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cuemol2-rs-{}-{name}", std::process::id()))
}

fn gzip_compress(path: &Path) -> Option<Vec<u8>> {
    let output = Command::new("gzip").arg("-c").arg(path).output().ok()?;
    output.status.success().then_some(output.stdout)
}

fn bzip2_compress(path: &Path) -> Option<Vec<u8>> {
    let output = Command::new("bzip2").arg("-c").arg(path).output().ok()?;
    output.status.success().then_some(output.stdout)
}

fn namd_header(natoms: i32, x: f64, y: f64, z: f64, swapped: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(28);
    if swapped {
        bytes.extend_from_slice(&natoms.to_be_bytes());
        bytes.extend_from_slice(&x.to_be_bytes());
        bytes.extend_from_slice(&y.to_be_bytes());
        bytes.extend_from_slice(&z.to_be_bytes());
    } else {
        bytes.extend_from_slice(&natoms.to_le_bytes());
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
        bytes.extend_from_slice(&z.to_le_bytes());
    }
    bytes
}

fn qdf_header(file_type: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"QDF0");
    bytes.push(0);
    bytes.extend_from_slice(&12_345_678_i32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&1.234_567_8_f32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&(file_type.len() as i32).to_le_bytes());
    bytes.extend_from_slice(file_type.as_bytes());
    bytes
}
