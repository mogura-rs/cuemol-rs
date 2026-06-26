use cuemol_molstr::{MolCommandManager, MolSceneManager, QscLoadStatus};
use cuemol_qsys::{CenterMark, StereoMode};
use std::path::PathBuf;

#[test]
fn new_scene_load_object_and_new_renderer_match_reference_flow() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let scene_uid = manager.new_scene("new scene");
    let scene = manager.scene_mut(scene_uid).unwrap();
    assert_eq!(scene.name(), "new scene");

    let object_uid = scene
        .load_object(root.join("1CRN.pdb"), "1CRN", "pdb")
        .unwrap();
    let object = scene.object_mut(object_uid).unwrap();
    assert_eq!(object.name(), "1CRN");
    assert_eq!(object.mol().atom_count(), 327);

    let renderer_uid = object.add_renderer("simple", "simple0");
    let renderer = object
        .renderers()
        .iter()
        .find(|renderer| renderer.uid() == renderer_uid)
        .unwrap();
    assert_eq!(renderer.name(), "simple0");
    assert_eq!(renderer.renderer_type(), "simple");
}

#[test]
fn scene_load_object_accepts_mol2_format() {
    let dir = std::env::temp_dir().join(format!("cuemol-qsys-mol2-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ligand.mol2");
    std::fs::write(
        &path,
        "@<TRIPOS>MOLECULE\nligand\n2 1 1 0 0\nSMALL\nUSER_CHARGES\n@<TRIPOS>ATOM\n1 C1 0.0 0.0 0.0 C.3 1 LIG 0.0\n2 O1 1.2 0.0 0.0 O.2 1 LIG 0.0\n@<TRIPOS>BOND\n1 1 2 1\n",
    )
    .unwrap();

    let mut manager = MolSceneManager::new();
    let scene_uid = manager.new_scene("mol2 scene");
    let object_uid = manager
        .scene_mut(scene_uid)
        .unwrap()
        .load_object(&path, "ligand", "mol2")
        .unwrap();
    let object = manager
        .scene(scene_uid)
        .unwrap()
        .object(object_uid)
        .unwrap();
    assert_eq!(object.mol().atom_count(), 2);
    assert_eq!(object.mol().bond_count(), 1);

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn command_manager_matches_reference_command_flow() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolCommandManager::new();
    let new_scene = manager.new_scene_command("new scene", true);
    let scene_uid = new_scene.result_scene;
    assert_eq!(
        manager.scene_manager().scene(scene_uid).unwrap().name(),
        "new scene"
    );
    let view_uid = new_scene.result_view.unwrap();
    assert_eq!(manager.view(view_uid).unwrap().scene_uid(), scene_uid);

    let loaded = manager
        .load_object_command(scene_uid, root.join("1CRN.pdb"), "1CRN", "pdb")
        .unwrap();
    let object_uid = loaded.result_object;
    let object = manager
        .scene_manager()
        .scene(scene_uid)
        .unwrap()
        .object(object_uid)
        .unwrap();
    assert_eq!(object.name(), "1CRN");
    assert_eq!(object.mol().atom_count(), 327);

    let rendered = manager
        .new_renderer_command(scene_uid, object_uid, "simple", "simple0")
        .unwrap();
    let renderer = manager
        .scene_manager()
        .scene(scene_uid)
        .unwrap()
        .object(object_uid)
        .unwrap()
        .renderer(rendered.result_renderer)
        .unwrap();
    assert_eq!(renderer.name(), "simple0");
    assert_eq!(renderer.renderer_type(), "simple");
}

#[test]
fn command_manager_load_scene_matches_qsc_reference_flow() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolCommandManager::new();
    let result = manager
        .load_scene_command(root.join("number_chain_atomintr1.qsc"), None)
        .unwrap();
    let scene_uid = result.result_scene.unwrap();
    assert_eq!(result.status, QscLoadStatus::Loaded { scene_uid });

    let scene = manager.scene_manager().scene(scene_uid).unwrap();
    let mol = scene.object_by_name("number_chain_260112.pdb").unwrap();
    let measure = mol.renderer_by_name("measure").unwrap();
    assert_eq!(measure.group(), Some(""));
    assert_eq!(measure.style(), Some("DefaultLabel,DefaultAtomIntr"));
    assert!(mol.renderer_by_name("measure2").is_some());
    assert!(scene.camera_by_name("__current").is_some());
}

#[test]
fn qsc_loader_reads_camera_attributes() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let scene_uid = manager
        .load_qsc_file(root.join("number_chain_atomintr1.qsc"))
        .unwrap();
    let camera = manager
        .scene(scene_uid)
        .unwrap()
        .camera_by_name("__current")
        .unwrap();
    assert_eq!(camera.center_mark(), CenterMark::Cross);
    assert_eq!(camera.stereo_mode(), StereoMode::None);
    assert!(!camera.is_perspective());
    assert_eq!(camera.cam_dist(), 200.0);
    assert_eq!(camera.slab_depth(), 50.0);
    assert_eq!(camera.zoom(), 50.0);
    assert!((camera.center().x - 7.2782).abs() < 1.0e-6);
    assert!((camera.center().y - 10.1857).abs() < 1.0e-6);
    assert!((camera.center().z - 6.8747).abs() < 1.0e-6);
}

#[test]
fn qsc_loader_resolves_atom_interaction_renderer_defs() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let scene_uid = manager
        .load_qsc_file(root.join("number_chain_atomintr1.qsc"))
        .unwrap();
    let scene = manager.scene(scene_uid).unwrap();
    let mol = scene.object_by_name("number_chain_260112.pdb").unwrap();

    let measure = mol.renderer_by_name("measure").unwrap();
    assert_eq!(
        measure.get_defs_json(),
        "[{\"a0\":\"9 ALA 27 CA\",\"a1\":\"A CYS 3 CA\"}]"
    );

    let measure2 = mol.renderer_by_name("measure2").unwrap();
    assert_eq!(
        measure2.get_defs_json(),
        "[{\"a0\":\"9 ALA 27 CA\",\"a1\":\"A CYS 3 CA\"}]"
    );

    // The measured distance is the actual CA(9/27)-CA(A/3) separation. Cross-check
    // it against the two atoms found by an independent scan of the molecule.
    let find_ca = |chain: &str, res_id: i32| {
        mol.mol()
            .atoms()
            .iter()
            .find(|a| a.chain_id == chain && a.res_id == res_id && a.name.trim() == "CA")
            .map(|a| a.position)
            .expect("CA atom present")
    };
    let p0 = find_ca("9", 27);
    let p1 = find_ca("A", 3);
    let expected = ((p0.x - p1.x).powi(2) + (p0.y - p1.y).powi(2) + (p0.z - p1.z).powi(2)).sqrt();

    let distances = mol.atom_intr_distances(measure);
    assert_eq!(distances.len(), 1);
    assert!((distances[0].unwrap() - expected).abs() < 1.0e-6);
    assert!(expected > 0.0);
    assert!(mol
        .atom_intr_measurements_json(measure)
        .contains(&format!("\"distance\":{expected:.3}")));
}

#[test]
fn qsc_loader_reports_lfs_pointer_fixture() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let status = manager
        .load_qsc_file_checked(root.join("1crn_test1.qsc"))
        .unwrap();
    match status {
        QscLoadStatus::GitLfsPointer { oid, size } => {
            assert_eq!(
                oid,
                "5e719f2cd94d659db76a7027a3f4c3ab5e302eb8e5ed05d820645445e693c9e9"
            );
            assert_eq!(size, 6622);
        }
        QscLoadStatus::Loaded { .. } => panic!("expected Git LFS pointer"),
    }

    let mut command_manager = MolCommandManager::new();
    let result = command_manager
        .load_scene_command(root.join("1crn_test1.qsc"), None)
        .unwrap();
    assert_eq!(result.result_scene, None);
    assert!(matches!(result.status, QscLoadStatus::GitLfsPointer { .. }));
}

#[test]
fn qsc_writer_roundtrips_loaded_scene_subset() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let scene_uid = manager
        .load_qsc_file(root.join("number_chain_atomintr1.qsc"))
        .unwrap();
    let original_atoms = manager
        .scene(scene_uid)
        .unwrap()
        .object_by_name("number_chain_260112.pdb")
        .unwrap()
        .mol()
        .atom_count();
    let written = manager.scene(scene_uid).unwrap().write_qsc_string();
    assert!(written.contains("style=\"DefaultCPKColoring\""));
    assert!(written.contains("<line aid1=\"43\" aid2=\"88\"/>"));

    let mut reloaded = MolSceneManager::new();
    let reloaded_uid = reloaded.new_scene("roundtrip");
    reloaded
        .scene_mut(reloaded_uid)
        .unwrap()
        .load_qsc_str(&written)
        .unwrap();

    let scene = reloaded.scene(reloaded_uid).unwrap();
    let mol = scene.object_by_name("number_chain_260112.pdb").unwrap();
    assert_eq!(mol.mol().atom_count(), original_atoms);
    assert!(mol.renderer_by_name("trace1").is_some());
    assert!(scene.camera_by_name("__current").is_some());

    let measure = mol.renderer_by_name("measure").unwrap();
    assert_eq!(
        measure.get_defs_json(),
        "[{\"a0\":\"9 ALA 27 CA\",\"a1\":\"A CYS 3 CA\"}]"
    );
}

#[test]
fn qsc_writer_and_reader_roundtrip_multiple_objects() {
    let Some(root) = reference_test_data_root() else {
        return;
    };

    let mut manager = MolSceneManager::new();
    let scene_uid = manager.new_scene("multi");
    let scene = manager.scene_mut(scene_uid).unwrap();
    let object_a = scene
        .load_object(root.join("1CRN.pdb"), "first", "pdb")
        .unwrap();
    scene
        .object_mut(object_a)
        .unwrap()
        .add_renderer("simple", "r0");
    let object_b = scene
        .load_object(root.join("1CRN.pdb"), "second", "pdb")
        .unwrap();
    scene
        .object_mut(object_b)
        .unwrap()
        .add_renderer("cpk", "r1");

    let written = scene.write_qsc_string();
    let mut reloaded = MolSceneManager::new();
    let reloaded_uid = reloaded.new_scene("roundtrip");
    reloaded
        .scene_mut(reloaded_uid)
        .unwrap()
        .load_qsc_str(&written)
        .unwrap();

    let scene = reloaded.scene(reloaded_uid).unwrap();
    assert_eq!(scene.objects().len(), 2);
    assert_eq!(
        scene.object_by_name("first").unwrap().mol().atom_count(),
        327
    );
    assert_eq!(
        scene.object_by_name("second").unwrap().mol().atom_count(),
        327
    );
    assert_eq!(
        scene
            .object_by_name("second")
            .unwrap()
            .renderer_by_name("r1")
            .unwrap()
            .renderer_type(),
        "cpk"
    );
}

fn reference_test_data_root() -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data");
    path.exists().then_some(path)
}
