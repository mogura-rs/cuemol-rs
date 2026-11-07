use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiRendererSetup};

const PDB: &[u8] = b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  O1  LIG A   1       3.000   0.000   0.000  1.00 10.00           O\nEND\n";

fn xtc_frame(offset: f32) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [1995_i32, 2, 0] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&0_f32.to_be_bytes());
    for value in [1_f32, 0., 0., 0., 1., 0., 0., 0., 1.] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&2_i32.to_be_bytes());
    for value in [offset, 0., 0., offset + 1., 0., 0.] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

#[test]
fn failed_trajectory_block_edits_preserve_order_frame_and_coordinates() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xtc");
    let second = directory.path().join("second.xtc");
    std::fs::write(&first, xtc_frame(0.)).unwrap();
    std::fs::write(&second, xtc_frame(10.)).unwrap();
    let mut original = DesktopAppState::new();
    let GuiLoadedPath::Object { object_uid, .. } = original.open_bytes("atoms.pdb", PDB).unwrap()
    else {
        panic!("expected molecule");
    };
    original
        .append_trajectory_block(object_uid, &first, 1)
        .unwrap();
    original
        .append_trajectory_block(object_uid, &second, 1)
        .unwrap();
    std::fs::write(&second, b"truncated").unwrap();
    for operation in ["remove", "move", "frame"] {
        let mut app = original.clone();
        let result = match operation {
            "remove" => app.remove_trajectory_block(object_uid, 0).map(|_| ()),
            "move" => app.move_trajectory_block(object_uid, 1, 0).map(|_| ()),
            _ => app.apply_trajectory_frame(object_uid, 1),
        };
        assert!(result.is_err(), "{operation}");
        assert_eq!(
            app.active_trajectory_summaries(),
            original.active_trajectory_summaries(),
            "{operation}"
        );
        assert!(app.same_undo_state(&original), "{operation}");
    }
}

#[test]
fn failed_file_load_does_not_create_a_scene() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.gro");
    std::fs::write(&path, b"invalid").unwrap();
    let mut app = DesktopAppState::new();
    let before = app.clone();
    assert!(app.open_path(&path).is_err());
    assert!(app.same_undo_state(&before));
    assert!(app.scene_manager().scene_uids().is_empty());
}

#[test]
fn failed_renderer_setup_does_not_insert_a_molecule_or_scene() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("atoms.pdb");
    std::fs::write(&path, PDB).unwrap();
    for existing_scene in [false, true] {
        let mut app = DesktopAppState::new();
        if existing_scene {
            app.open_bytes("existing.pdb", PDB).unwrap();
        }
        let before = app.clone();
        let reader = app.molecule_reader_options_for_path(&path).unwrap();
        let setup = GuiRendererSetup {
            object_name: "new".into(),
            renderer_type: "missing-renderer".into(),
            renderer_name: "invalid".into(),
            selection: None,
            recenter: false,
            map_center_policy: None,
        };
        assert!(app
            .open_path_with_reader_and_renderer_setup(&path, reader.as_ref(), &setup)
            .is_err());
        assert!(
            app.same_undo_state(&before),
            "existing_scene={existing_scene}"
        );
    }
}

#[test]
fn failed_scene_save_preserves_morph_metadata_and_edit_history() {
    let mut app = DesktopAppState::new();
    let GuiLoadedPath::Object { object_uid, .. } = app.open_bytes("atoms.pdb", PDB).unwrap() else {
        panic!("expected molecule");
    };
    app.initialize_morph_object(object_uid).unwrap();
    let scene = app.active_scene().unwrap();
    app.mark_scene_modified(scene);
    let before = app.clone();
    let directory = tempfile::tempdir().unwrap();
    assert!(app.save_active_scene_qsc(directory.path()).is_err());
    assert!(app.same_undo_state(&before));
    assert!(app.scene_is_modified(scene));
    let exported = app.active_scene_bytes().unwrap();
    assert!(!exported.is_empty());
    assert!(app.same_undo_state(&before));
    assert!(app.scene_is_modified(scene));
}
