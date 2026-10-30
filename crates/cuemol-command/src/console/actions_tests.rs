use super::super::{Dialect, Submission};
use super::*;
use cuemol_qsys::Camera;
use serde_json::json;

fn run(
    submission: &mut Submission,
    state: &mut DesktopAppState,
    session: &mut Session,
) -> Result<(), String> {
    let mut camera = Camera::new();
    while let Some(action) = submission.next(session, state, &camera)? {
        if let Action::Tool { name, args } = action {
            session.prepare_record(state, &name, &args);
            crate::call_tool(state, &mut camera, &name, &args)
                .map_err(|error| error.to_string())?;
            session.complete_record(true);
        }
    }
    Ok(())
}

#[test]
fn inspector_presets_and_default_resets_export_replayable_operations() {
    let mut state = super::super::tests::state();
    let sid = state.active_scene().unwrap();
    let object = state.active_object_summaries()[0].uid;
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ribbon", "body");
    state
        .set_renderer_property(object, renderer, "alpha", "0.4")
        .unwrap();
    state
        .set_renderer_child_property(object, renderer, "helix", "width", "2.75")
        .unwrap();
    let preset = Record::operation(
        &state,
        "set_node_props",
        &json!({"nodeType":"scene","nodeId":sid.get(),"cascadeGroupVisibility":true,"updates":[
            {"prop":"aa_method","value":"fxaa"},{"prop":"aaJitterLevel","value":"5"}
        ]}),
    );
    assert_eq!(preset.entry_count(), 2);
    let reset = Record::operation(
        &state,
        "reset_node_props",
        &json!({"nodeType":"renderer","nodeId":renderer.get(),"props":["alpha","helix.width"]}),
    );
    assert_eq!(reset.entry_count(), 2);
    let source = export(&[preset, reset]);
    assert!(!source.contains("set_node_props"));
    assert!(!source.contains("cascadeGroupVisibility"));
    state.new_scene("other", true);
    let first = state.scene_manager().object(object).unwrap().mol().clone();
    let new_scene = state.active_scene().unwrap();
    let new_object = state
        .scene_manager_mut()
        .scene_mut(new_scene)
        .unwrap()
        .add_molecule("protein", first);
    let new_renderer = state
        .scene_manager_mut()
        .object_mut(new_object)
        .unwrap()
        .add_renderer("ribbon", "body");
    state
        .set_renderer_property(new_object, new_renderer, "alpha", "0.7")
        .unwrap();
    state
        .set_renderer_child_property(new_object, new_renderer, "helix", "width", "3.25")
        .unwrap();
    assert_ne!(sid, new_scene);
    run(
        &mut Submission::library(&source, None, &[]).unwrap(),
        &mut state,
        &mut Session::default(),
    )
    .unwrap();
    let scene = state.scene_manager().scene(new_scene).unwrap();
    assert_eq!(scene.scene_attribute("aaJitterLevel"), Some("5"));
    let result = crate::call_tool(
        &mut state,
        &mut Camera::new(),
        "list_node_props",
        &json!({"nodeType":"renderer","nodeId":new_renderer.get()}),
    )
    .unwrap();
    for key in ["alpha", "helix.width"] {
        let row = result["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == key)
            .unwrap();
        assert_eq!(row["isdefault"], true, "{key}");
    }
    assert_eq!(
        state
            .scene_manager()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .unwrap()
            .qsc_attr("alpha"),
        Some("0.4")
    );
}

#[test]
fn recording_survives_shared_undo_handles_and_exports_portable_commands() {
    let mut state = super::super::tests::state();
    let mut session = Session::default();
    session.begin_submission("Hide and rename", Dialect::CueMol);
    let recorded = session.action_record.clone();
    let mut submission =
        Submission::new("hide protein; rename protein, renamed", Dialect::CueMol).unwrap();
    run(&mut submission, &mut state, &mut session).unwrap();
    assert_eq!(recorded.entry_count(), 2);
    let exported = export(&[recorded.clone()]);
    let command_source = commands(&[recorded.clone()]);
    session.begin_submission("Next", Dialect::CueMol);
    assert_ne!(recorded.id(), session.action_record.id());
    assert_eq!(recorded.entry_count(), 2);
    for mut submission in [
        Submission::library(&exported, None, &[]).unwrap(),
        Submission::new(&command_source, Dialect::CueMol).unwrap(),
    ] {
        let mut other = super::super::tests::state();
        run(&mut submission, &mut other, &mut session).unwrap();
        let summary = &other.active_object_summaries()[0];
        assert_eq!(summary.name, "renamed");
        assert!(!summary.visible);
    }
}

#[test]
fn molecular_presets_accept_derived_molecules_and_export_the_base_parameter() {
    for class in ["AnimMol", "MorphMol", "Trajectory"] {
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .set_object_type(class);
        let mut session = Session::default();
        session.begin_submission("Preset", Dialect::CueMol);
        run(
            &mut Submission::library(PRESETS, Some("cartoon-ligand"), &[id.get()]).unwrap(),
            &mut state,
            &mut session,
        )
        .unwrap();
        assert!(state
            .renderer_summaries(id)
            .iter()
            .any(|renderer| renderer.name == "p_cartoon"));
        let source =
            make_macro(&[session.action_record.clone()], "again", "Again", &state).unwrap();
        assert!(source.contains("type=\"MolCoord\""), "{class}: {source}");
    }
}

#[test]
fn library_edits_use_exact_external_filenames() {
    let directory = tempfile::tempdir().unwrap();
    let source = "<cuemol-actions version=\"1\"><action label=\"Original\"/></cuemol-actions>";
    for name in ["Upper.QAX", " spaced .qax"] {
        std::fs::write(directory.path().join(name), source).unwrap();
    }
    let files = read_library(directory.path());
    let upper = files.iter().find(|file| file.name == "Upper").unwrap();
    let spaced = files.iter().find(|file| file.name == " spaced ").unwrap();
    delete_library_file(directory.path(), &upper.key).unwrap();
    assert!(!directory.path().join("Upper.QAX").exists());
    rename_library_file(directory.path(), &spaced.key, "Renamed").unwrap();
    assert!(!directory.path().join(" spaced .qax").exists());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("Renamed.qax")).unwrap(),
        source
    );
    assert!(delete_library_file(directory.path(), "../Renamed").is_err());
    #[cfg(target_os = "linux")]
    {
        for name in ["same.qax", "same.QAX", "a:b.qax"] {
            std::fs::write(directory.path().join(name), source).unwrap();
        }
        let files = read_library(directory.path());
        let upper = files
            .iter()
            .find(|file| {
                file.path
                    .as_ref()
                    .is_some_and(|path| path.file_name().unwrap() == "same.QAX")
            })
            .unwrap();
        delete_library_file(directory.path(), &upper.key).unwrap();
        assert!(directory.path().join("same.qax").exists());
        assert!(!directory.path().join("same.QAX").exists());
        let colon = files.iter().find(|file| file.name == "a:b").unwrap();
        rename_library_file(directory.path(), &colon.key, "Portable").unwrap();
        assert!(!directory.path().join("a:b.qax").exists());
        assert!(directory.path().join("Portable.qax").exists());
    }
}

#[test]
fn macro_parameterizes_only_renderers_created_by_its_steps() {
    let mut state = super::super::tests::state();
    let mut session = Session::default();
    session.begin_submission("Create cartoon", Dialect::CueMol);
    run(
        &mut Submission::new(
            "create_renderer protein, cartoon, name=custom; set protein/custom.visible, false",
            Dialect::CueMol,
        )
        .unwrap(),
        &mut state,
        &mut session,
    )
    .unwrap();
    let source = make_macro(
        &[session.action_record.clone()],
        "custom",
        "Custom cartoon",
        &state,
    )
    .unwrap();
    assert!(source.contains("param=\"target\""));
    assert!(source.contains("sub=\"custom\""));
    let mut other = super::super::tests::state();
    let id = other.active_object_summaries()[0].uid;
    other
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .set_name("different molecule");
    run(
        &mut Submission::library(&source, Some("custom"), &[id.get()]).unwrap(),
        &mut other,
        &mut session,
    )
    .unwrap();
    let renderer = other
        .scene_manager()
        .object(id)
        .unwrap()
        .renderers()
        .iter()
        .find(|renderer| renderer.name() == "custom")
        .unwrap();
    assert!(!renderer.is_visible());
    let args = json!({"nodeType":"renderer", "nodeId":renderer.uid().get(), "visible":true});
    let record = Record::operation(&other, "set_visible", &args);
    assert!(make_macro(&[record], "invalid", "Invalid", &other)
        .unwrap_err()
        .contains("Include the step"));
}

#[test]
fn preset_targets_recover_after_failure_without_replaying_prior_operations() {
    let file = LibraryFile::presets();
    assert!(file.error.is_none());
    assert_eq!(file.macros.len(), 12);
    let mut state = super::super::tests::state();
    let id = state.active_object_summaries()[0].uid;
    let mut submission = Submission::library(
        PRESETS,
        Some("cartoon-ligand"),
        &[u64::MAX, id.get(), id.get()],
    )
    .unwrap();
    let mut session = Session::default();
    assert!(submission
        .next(&mut session, &state, &Camera::new())
        .is_err());
    assert!(submission.skip_failed_target());
    run(&mut submission, &mut state, &mut session).unwrap();
    let renderers = state.scene_manager().object(id).unwrap().renderers();
    assert_eq!(
        renderers
            .iter()
            .filter(|renderer| renderer.name() == "p_cartoon")
            .count(),
        1
    );
    assert_eq!(
        renderers
            .iter()
            .filter(|renderer| renderer.name() == "p_ligand")
            .count(),
        1
    );
    assert!(!submission.skip_failed_target());
}

#[test]
fn library_refuses_console_lines_and_preserves_existing_user_files() {
    let source =
        "<cuemol-actions version=\"1\"><action><cmd>hide protein</cmd></action></cuemol-actions>";
    assert!(Submission::library(source, None, &[]).is_err());
    let directory = tempfile::tempdir().unwrap();
    let source = LibraryFile::presets()
        .copy_macro("white-background")
        .unwrap();
    save_library(directory.path(), "example", &source).unwrap();
    assert!(save_library(directory.path(), "example", PRESETS).is_err());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("example.qax")).unwrap(),
        source
    );
    assert_eq!(read_library(directory.path()).len(), 2);
    assert!(save_library(directory.path(), "../escape", &source).is_err());
    let record = Record::new("Large", "test");
    let mut note = LDom2Node::new();
    note.set_tag_name("note");
    note.set_contents("x".repeat(MAX_RECORD_BYTES + 1));
    record.append(note);
    assert!(make_macro(
        &[record],
        "incomplete",
        "Incomplete",
        &super::super::tests::state()
    )
    .is_err());
}
