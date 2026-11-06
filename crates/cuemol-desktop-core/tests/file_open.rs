use cuemol_desktop_core::{DesktopAppState, FileOpenBatch, FileOpenTarget, PreparedFile};

const PDB: &[u8] =
    b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n";

fn molecule(batch: &mut FileOpenBatch, app: &mut DesktopAppState, name: &str) {
    batch
        .open_prepared(app, PreparedFile::parse(name, PDB).unwrap())
        .unwrap();
}

#[test]
fn new_target_reuses_untouched_scene_then_pins_successful_batch() {
    let mut app = DesktopAppState::new();
    let empty = app.new_scene("empty", true).result_scene;
    let mut batch = FileOpenBatch::new(FileOpenTarget::New);
    molecule(&mut batch, &mut app, "first.pdb");
    assert_eq!(app.active_scene(), Some(empty));
    molecule(&mut batch, &mut app, "second.pdb");
    assert_eq!(app.active_object_summaries().len(), 2);
    assert_eq!(app.scene_manager().scene_uids().len(), 1);

    let mut next = FileOpenBatch::new(FileOpenTarget::New);
    molecule(&mut next, &mut app, "third.pdb");
    let target = app.active_scene().unwrap();
    assert_ne!(target, empty);
    app.set_active_scene(Some(empty));
    molecule(&mut next, &mut app, "fourth.pdb");
    assert_eq!(app.active_scene(), Some(target));
    assert_eq!(app.active_object_summaries().len(), 2);
}

#[test]
fn invalid_file_and_closed_destination_leave_scene_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let bad = directory.path().join("broken.gro");
    std::fs::write(&bad, b"invalid").unwrap();
    let mut app = DesktopAppState::new();
    app.open_bytes("existing.pdb", PDB).unwrap();
    let before = app.clone();
    let mut batch = FileOpenBatch::new(FileOpenTarget::New);
    assert!(batch.open_path(&mut app, &bad).is_err());
    assert!(app.same_undo_state(&before));
    assert_eq!(batch.scene(), None);
    molecule(&mut batch, &mut app, "first.pdb");
    app.destroy_scene(batch.scene().unwrap());
    let before = app.clone();
    assert!(batch
        .open_prepared(&mut app, PreparedFile::parse("second.pdb", PDB).unwrap())
        .is_err());
    assert!(app.same_undo_state(&before));
}

#[test]
fn scene_file_ignores_object_target_and_does_not_replace_first_pin() {
    let mut app = DesktopAppState::new();
    let empty = app.new_scene("empty", true).result_scene;
    let mut exported = DesktopAppState::new();
    exported.open_bytes("scene.pdb", PDB).unwrap();
    let scene = exported.active_scene_bytes().unwrap();
    let mut first = FileOpenBatch::new(FileOpenTarget::Active);
    first
        .open_prepared(&mut app, PreparedFile::parse("first.qsc", &scene).unwrap())
        .unwrap();
    assert_eq!(app.active_scene(), Some(empty));
    assert_eq!(app.scene_manager().scene_uids().len(), 1);
    first
        .open_prepared(&mut app, PreparedFile::parse("second.qsc", &scene).unwrap())
        .unwrap();
    assert_ne!(app.active_scene(), Some(empty));
    molecule(&mut first, &mut app, "attached.pdb");
    assert_eq!(app.active_scene(), Some(empty));
    assert_eq!(app.active_object_summaries().len(), 2);
    assert_eq!(app.scene_manager().scene_uids().len(), 2);
}

#[test]
fn native_paths_and_prepared_files_use_the_same_destination_rules() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("atoms.pdb");
    std::fs::write(&path, PDB).unwrap();
    for new in [false, true] {
        let mut app = DesktopAppState::new();
        app.open_bytes("existing.pdb", PDB).unwrap();
        let original = app.active_scene().unwrap();
        let mut batch = FileOpenBatch::new(if new {
            FileOpenTarget::New
        } else {
            FileOpenTarget::Active
        });
        batch.open_path(&mut app, &path).unwrap();
        molecule(&mut batch, &mut app, "more.pdb");
        assert_eq!(app.active_scene() != Some(original), new);
        assert_eq!(app.active_object_summaries().len(), if new { 2 } else { 3 });
        let saved = directory.path().join("saved.qsc");
        std::fs::write(&saved, app.active_scene_bytes().unwrap()).unwrap();
        let count = app.scene_manager().scene_uids().len();
        batch.open_path(&mut app, &saved).unwrap();
        assert_eq!(app.scene_manager().scene_uids().len(), count + 1);
    }
}
