use cuemol_command::call_tool;
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

const XML: &str = r##"<styles id="linked"><color id="accent" value="#ff0000"/><sel id="focus" value="name C1"/><style id="keepcpk" type="renderer"><sphr>0.7</sphr></style></styles>"##;

fn fixture(read_only: bool) -> (tempfile::TempDir, DesktopAppState, Uid, usize) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("linked.xml");
    std::fs::write(&path, XML).unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("styles", true).result_scene;
    let index = state
        .load_scene_style_file(scene, &path, read_only)
        .unwrap();
    (dir, state, scene, index)
}

#[test]
fn unlocking_and_renaming_a_linked_style_preserve_its_source_and_clean_state() {
    let (dir, mut state, scene, index) = fixture(true);
    call_tool(
        &mut state,
        &mut Camera::new(),
        "set_style_readonly",
        &json!({"styleSet":"linked","readonly":false}),
    )
    .unwrap();
    state
        .update_scene_style(scene, index, "renamed", false, XML)
        .unwrap();
    let summary = state
        .style_summaries(scene)
        .into_iter()
        .find(|s| s.local_index == Some(index))
        .unwrap();
    assert_eq!(
        summary.source,
        dir.path().join("linked.xml").to_string_lossy()
    );
    assert_eq!(summary.name, "renamed");
    assert!(!summary.read_only);
    assert!(!summary.modified);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("linked.xml")).unwrap(),
        XML
    );
}

#[test]
fn commands_reject_edits_to_read_only_linked_styles_without_losing_metadata() {
    let (_, mut state, scene, index) = fixture(true);
    let before = state.scene_manager().scene(scene).unwrap().loaded_styles()[index].clone();
    let result = call_tool(
        &mut state,
        &mut Camera::new(),
        "define_selection",
        &json!({"styleSet":"linked","name":"focus","selection":"name O1"}),
    );
    assert!(result.is_err());
    assert_eq!(
        state.scene_manager().scene(scene).unwrap().loaded_styles()[index],
        before
    );
}

#[test]
fn linked_style_edits_save_to_the_current_source_and_preserve_other_definitions() {
    let (dir, mut state, scene, index) = fixture(false);
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"styleSet":"linked","name":"focus","selection":"name O1"}),
    )
    .unwrap();
    let style = &state.scene_manager().scene(scene).unwrap().loaded_styles()[index];
    assert_eq!(
        style.node.str_attr("src").as_str(),
        dir.path().join("linked.xml").to_str().unwrap()
    );
    assert_eq!(style.node.str_attr("modified").as_str(), "true");
    assert!(style.data.contains("accent"));
    assert!(style.data.contains("keepcpk"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("linked.xml")).unwrap(),
        XML
    );
    call_tool(
        &mut state,
        &mut camera,
        "save_style_file",
        &json!({"styleSet":"linked"}),
    )
    .unwrap();
    let style = &state.scene_manager().scene(scene).unwrap().loaded_styles()[index];
    assert_eq!(style.node.str_attr("modified").as_str(), "true");
    assert!(std::fs::read_to_string(dir.path().join("linked.xml"))
        .unwrap()
        .contains("name O1"));
    let next = dir.path().join("new.xml");
    state.save_scene_style_file(scene, index, &next).unwrap();
    assert_ne!(
        state.scene_manager().scene(scene).unwrap().loaded_styles()[index]
            .node
            .str_attr("modified")
            .as_str(),
        "true"
    );
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"styleSet":"linked","name":"focus","selection":"name O1"}),
    )
    .unwrap();
    assert_eq!(
        state.scene_manager().scene(scene).unwrap().loaded_styles()[index]
            .node
            .str_attr("modified")
            .as_str(),
        "true"
    );
}

#[test]
fn saving_a_renderer_preset_into_a_linked_style_retains_existing_entries() {
    let (_, mut state, scene, index) = fixture(false);
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("probe", cuemol_molstr::MolCoord::new());
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("cpk", "atoms");
    state
        .create_scene_renderer_style(scene, index, object, renderer, "new", false)
        .unwrap();
    let style = &state.scene_manager().scene(scene).unwrap().loaded_styles()[index];
    for entry in ["accent", "focus", "keepcpk", "newcpk"] {
        assert!(style.data.contains(entry), "missing {entry}");
    }
    assert_eq!(style.node.str_attr("modified").as_str(), "true");
    assert!(!style.node.str_attr("src").is_empty());
    assert!(state
        .create_scene_renderer_style(scene, index, object, renderer, "keep", false)
        .is_err());
}

#[test]
fn modified_styles_cannot_be_locked_through_commands() {
    let (_, mut state, _, _) = fixture(false);
    call_tool(
        &mut state,
        &mut Camera::new(),
        "define_selection",
        &json!({"styleSet":"linked","name":"focus","selection":"name O1"}),
    )
    .unwrap();
    assert!(call_tool(
        &mut state,
        &mut Camera::new(),
        "set_style_readonly",
        &json!({"styleSet":"linked","readonly":true})
    )
    .is_err());
}

#[test]
#[cfg(feature = "scripting")]
fn command_style_lookup_uses_global_fallback_without_creating_a_local_shadow() {
    let mut state = DesktopAppState::new();
    let first = state.new_scene("first", true).result_scene;
    state
        .run_script_js(
            "var styles=cuemol.getService('StyleManager'); styles.createStyleSet('global-edit',0);",
        )
        .unwrap();
    let global = state
        .style_summaries(first)
        .into_iter()
        .find(|s| s.name == "global-edit")
        .unwrap();
    assert!(!global.read_only);
    call_tool(
        &mut state,
        &mut Camera::new(),
        "define_selection",
        &json!({"styleSet":"global-edit","name":"focus","selection":"name C1"}),
    )
    .unwrap();
    assert!(state
        .scene_manager()
        .scene(first)
        .unwrap()
        .loaded_styles()
        .is_empty());
    state.new_scene("second", true);
    assert_eq!(
        state
            .run_script_js(
                "print(cuemol.getService('StyleManager').getStrData('sel','focus',scene.uid));"
            )
            .unwrap()
            .0,
        "name C1\n"
    );
    call_tool(
        &mut state,
        &mut Camera::new(),
        "create_style_set",
        &json!({"name":"global-edit"}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut Camera::new(),
        "define_selection",
        &json!({"styleSet":"global-edit","name":"focus","selection":"name O1"}),
    )
    .unwrap();
    assert_eq!(state.run_script_js("print(cuemol.getService('StyleManager').getStrData('sel','focus',scene.uid)); print(cuemol.getService('StyleManager').getStrData('sel','focus',0));").unwrap().0, "name O1\nname C1\n");
}

#[test]
#[cfg(feature = "scripting")]
fn closing_all_scenes_preserves_global_styles() {
    let mut state = DesktopAppState::new();
    state.new_scene("first", true);
    state.run_script_js("var styles=cuemol.getService('StyleManager'); var id=styles.createStyleSet('global-edit',0); styles.setStrData('sel','focus','name C1',0,id);").unwrap();
    state.destroy_all_scenes();
    state.new_scene("next", true);
    assert_eq!(
        state
            .run_script_js(
                "print(cuemol.getService('StyleManager').getStrData('sel','focus',scene.uid));"
            )
            .unwrap()
            .0,
        "name C1\n"
    );
}

#[test]
fn global_style_file_edits_keep_identity_and_failed_reload_preserves_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.xml");
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("global", true).result_scene;
    let target = state.find_style_target(Uid::new(0), "user").unwrap();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"styleSet":"user","scope":"global","name":"focus","selection":"name C1"}),
    )
    .unwrap();
    state.save_style_file(target, scene, &path).unwrap();
    let summary = state.style_summary(target).unwrap();
    assert!(!summary.modified);
    assert_eq!(summary.source, path.to_string_lossy());
    state
        .set_style_properties(target, "renamed", false)
        .unwrap();
    assert_eq!(state.style_summary(target).unwrap().name, "renamed");
    assert!(!state.style_summary(target).unwrap().modified);
    assert!(state.set_style_properties(target, "system", false).is_err());
    assert_eq!(state.style_summary(target).unwrap().name, "renamed");

    std::fs::write(&path, XML).unwrap();
    state.reload_style_file(target, scene).unwrap();
    assert_eq!(state.style_summary(target).unwrap().name, "linked");
    assert!(state.style_xml(target).unwrap().contains("keepcpk"));
    assert!(!state.style_summary(target).unwrap().modified);
    assert_eq!(state.style_summary(target).unwrap().source, summary.source);
    let before = state.clone();
    std::fs::write(&path, "<styles").unwrap();
    assert!(state.reload_style_file(target, scene).is_err());
    assert!(state.same_undo_state(&before));
    assert!(state
        .save_style_file(target, scene, dir.path().join("missing/global.xml"))
        .is_err());
    assert!(state.same_undo_state(&before));
}

#[test]
fn style_worker_payload_round_trip_preserves_content_and_rejects_duplicate_import() {
    use cuemol_desktop_core::PreparedFile;

    let mut state = DesktopAppState::new();
    let scene = state.new_scene("import", true).result_scene;
    let parsed = PreparedFile::parse("palette.xml", XML.as_bytes()).unwrap();
    let transferred = PreparedFile::decode(&parsed.encode().unwrap()).unwrap();
    assert_eq!(transferred.style_name().as_deref(), Some("linked"));
    state.open_prepared(transferred).unwrap();
    let target = state.find_style_target(scene, "linked").unwrap();
    let summary = state.style_summary(target).unwrap();
    assert!(summary.read_only);
    assert!(!summary.modified);
    assert!(summary.source.is_empty());
    let xml = state.style_xml(target).unwrap();
    for entry in ["accent", "focus", "keepcpk"] {
        assert!(xml.contains(entry), "missing {entry}");
    }
    let before = state.clone();
    assert!(state.open_prepared(parsed).is_err());
    assert!(state.same_undo_state(&before));
    assert!(PreparedFile::parse("bad.xml", b"<scene/>").is_err());
    assert!(PreparedFile::parse("bad.xml", b"<styles/>").is_err());
}

#[test]
fn bundled_style_save_requires_a_real_destination_and_then_keeps_the_file_link() {
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("builtins", true).result_scene;
    let target = state.find_style_target(Uid::new(0), "system").unwrap();
    assert!(state.style_summary(target).unwrap().source.is_empty());
    let error = call_tool(
        &mut state,
        &mut Camera::new(),
        "save_style_file",
        &json!({"styleSet":"system","scope":"global"}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("specify path"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("system.xml");
    state.save_style_file(target, scene, &path).unwrap();
    assert_eq!(
        state.style_summary(target).unwrap().source,
        path.to_string_lossy()
    );
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("DefaultCPK"));
}

#[test]
#[cfg(feature = "scripting")]
fn editing_global_content_preserves_an_anonymous_or_untrimmed_script_name() {
    for name in ["", " padded "] {
        let mut state = DesktopAppState::new();
        state.new_scene("names", true);
        let encoded_name = serde_json::to_string(name).unwrap();
        let id = state
            .run_script_js(&format!(
                "var styles=cuemol.getService('StyleManager'); print(styles.createStyleSet({encoded_name},0));"
            ))
            .unwrap()
            .0
            .trim()
            .parse()
            .unwrap();
        let target = cuemol_desktop_core::GuiStyleTarget::Global(id);
        state.set_style_read_only(target, true).unwrap();
        state.set_style_read_only(target, false).unwrap();
        state
            .edit_style(
                target,
                r#"<styles><sel id="focus" value="name C1"/></styles>"#,
            )
            .unwrap();
        assert_eq!(
            state
                .run_script_js(&format!(
                    "print(cuemol.getService('StyleManager').getStyleSet({id}).name);"
                ))
                .unwrap()
                .0,
            format!("{name}\n")
        );
        assert!(state.style_xml(target).unwrap().contains("name C1"));
    }
}

#[test]
#[cfg(feature = "scripting")]
fn style_command_handles_disambiguate_anonymous_display_names() {
    let mut state = DesktopAppState::new();
    state.new_scene("handles", true);
    let ids: Vec<u64> = serde_json::from_str(&state.run_script_js(
        "var styles=cuemol.getService('StyleManager'); print(JSON.stringify([styles.createStyleSet('',0),styles.createStyleSet('(anonymous)',0)]));"
    ).unwrap().0).unwrap();
    let mut camera = Camera::new();
    for read_only in [true, false] {
        call_tool(
            &mut state,
            &mut camera,
            "set_style_readonly",
            &json!({
                "styleSet":"(anonymous)", "styleId":ids[0], "scope":"global", "readonly":read_only,
            }),
        )
        .unwrap();
    }
    call_tool(&mut state, &mut camera, "define_selection", &json!({
        "styleSet":"(anonymous)", "styleId":ids[0], "scope":"global", "name":"focus", "selection":"name C1",
    })).unwrap();
    let first = call_tool(
        &mut state,
        &mut camera,
        "get_style_set",
        &json!({
            "styleSet":"(anonymous)", "styleId":ids[0], "scope":"global",
        }),
    )
    .unwrap();
    let second = call_tool(
        &mut state,
        &mut camera,
        "get_style_set",
        &json!({
            "styleSet":"(anonymous)", "scope":"global",
        }),
    )
    .unwrap();
    assert_eq!(first["name"], "");
    assert_eq!(first["id"], ids[0]);
    assert_eq!(second["name"], "(anonymous)");
    assert_eq!(second["id"], ids[1]);
    assert!(first["xml"].as_str().unwrap().contains("focus"));
    assert!(!second["xml"].as_str().unwrap().contains("focus"));
    let before = state.clone();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({
            "styleSet":"(anonymous)", "styleId":ids[0], "name":"focus", "selection":"name O1",
        })
    )
    .is_err());
    assert!(state.same_undo_state(&before));
}
