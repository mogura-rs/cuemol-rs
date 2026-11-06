use cuemol_command::{call_tool, Protocol};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn protocol() -> (Protocol, DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("document", true);
    let mut camera = Camera::new();
    let mut protocol = Protocol::new();
    protocol.handle(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        &mut state,
        &mut camera,
    );
    (protocol, state, camera)
}

fn call(
    protocol: &mut Protocol,
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: Value,
) {
    let result = protocol
        .handle(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
                   "params":{"name":name,"arguments":args}}),
            state,
            camera,
        )
        .unwrap();
    assert_eq!(result["result"]["isError"], false, "{name}: {result}");
}

fn modified(state: &mut DesktopAppState, camera: &mut Camera) -> bool {
    let result = call_tool(
        state,
        camera,
        "list_node_props",
        &json!({"nodeType":"scene"}),
    )
    .unwrap();
    result["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["key"] == "modified")
        .unwrap()["value"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn command_edits_and_history_share_the_scene_modified_property() {
    let (mut protocol, mut state, mut camera) = protocol();
    assert!(!modified(&mut state, &mut camera));
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"scene","prop":"bgcolor","value":"red"}),
    );
    assert!(modified(&mut state, &mut camera));
    call(&mut protocol, &mut state, &mut camera, "undo", json!({}));
    assert!(!modified(&mut state, &mut camera));
    call(&mut protocol, &mut state, &mut camera, "redo", json!({}));
    assert!(modified(&mut state, &mut camera));
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "clear_undo",
        json!({}),
    );
    assert!(!modified(&mut state, &mut camera));
}

#[cfg(feature = "scripting")]
#[test]
fn script_transactions_report_object_changes_and_undo() {
    let (mut protocol, mut state, mut camera) = protocol();
    state.open_bytes("one.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let directory = tempfile::tempdir().unwrap();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "save_scene",
        json!({"path":directory.path().join("baseline.qsc"),"embedAll":true}),
    );
    let object = state.active_object_summaries()[0].uid.get();
    let (log, _) = state
        .run_script_js(&format!(
            "var s=cuemol.getScene(); s.startUndoTxn('rename object');
         cuemol.getObject({object}).name='changed'; s.commitUndoTxn();
         print('changed='+s.modified); s.undo(0); print('undo='+s.modified);
         s.redo(0); print('redo='+s.modified); s.clearUndoData(); print('clear='+s.modified);"
        ))
        .unwrap();
    assert!(log.contains("changed=true"), "{log}");
    assert!(log.contains("undo=false"), "{log}");
    assert!(log.contains("redo=true"), "{log}");
    assert!(log.contains("clear=false"), "{log}");
}

#[cfg(feature = "scripting")]
#[test]
fn successful_save_clears_modified_without_serializing_runtime_flags() {
    let (mut protocol, mut state, mut camera) = protocol();
    state.run_script_js("scene.startUndoTxn('background'); scene.bgcolor=cuemol.makeColor('red'); scene.commitUndoTxn();").unwrap();
    assert!(modified(&mut state, &mut camera));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("document.qsc");
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "save_scene",
        json!({"path":path}),
    );
    assert!(!modified(&mut state, &mut camera));
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("_script_modified"));
    let mut restored = DesktopAppState::new();
    restored.open_path(&path).unwrap();
    assert!(!modified(&mut restored, &mut camera));
}

#[test]
fn no_op_and_failed_edits_do_not_change_document_status() {
    let (mut protocol, mut state, mut camera) = protocol();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"scene","prop":"bgcolor","value":"red"}),
    );
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "clear_undo",
        json!({}),
    );
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"scene","prop":"bgcolor","value":"red"}),
    );
    assert!(!modified(&mut state, &mut camera));
    let before = state.clone();
    let result = call_tool(
        &mut state,
        &mut camera,
        "set_node_prop",
        &json!({"nodeType":"scene","prop":"aoRadius","value":"invalid"}),
    );
    assert!(result.is_err());
    assert!(state.same_undo_state(&before));
    assert!(!modified(&mut state, &mut camera));
}

#[cfg(feature = "scripting")]
#[test]
fn saving_one_scene_keeps_other_scene_edits_and_failed_save_keeps_history() {
    let (mut protocol, mut state, mut camera) = protocol();
    let first = state.active_scene().unwrap();
    let second = state.new_scene("other", true).result_scene;
    state.run_script_js(&format!("var a=cuemol.getScene({}); a.startUndoTxn('first'); a.setName('changed'); a.commitUndoTxn(); scene.startUndoTxn('second'); scene.setName('other changed'); scene.commitUndoTxn();", first.get())).unwrap();
    assert!(state.scene_is_modified(first));
    assert!(state.scene_is_modified(second));
    let directory = tempfile::tempdir().unwrap();
    let before = state.clone();
    assert!(state.save_active_scene_qsc(directory.path()).is_err());
    assert!(state.same_undo_state(&before));
    assert!(state.scene_is_modified(second));
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "save_scene",
        json!({"path":directory.path().join("scene.qsc")}),
    );
    assert!(state.scene_is_modified(first));
    assert!(!state.scene_is_modified(second));
}

#[test]
fn scene_styles_remain_modified_after_clear_history_until_embedded_save() {
    let (mut protocol, mut state, mut camera) = protocol();
    let scene = state.active_scene().unwrap();
    let index = state
        .import_scene_style(
            scene,
            "<styles id='local'><color id='accent' value='#ff0000'/></styles>",
            false,
        )
        .unwrap();
    state
        .edit_scene_style(
            scene,
            index,
            "<styles id='local'><color id='accent' value='#00ff00'/></styles>",
        )
        .unwrap();
    assert!(state.scene_is_modified(scene));
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "clear_undo",
        json!({}),
    );
    assert!(state.scene_is_modified(scene));
    let directory = tempfile::tempdir().unwrap();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "save_scene",
        json!({"path":directory.path().join("style.qsc")}),
    );
    assert!(!state.scene_is_modified(scene));
}

#[cfg(feature = "scripting")]
#[test]
fn script_transaction_status_matches_cpp_scene() {
    let Some(oracle) = std::env::var_os("CUEMOL_SCENE_STATE_ORACLE") else {
        eprintln!("skipping scene state oracle: run scripts/check-reference-scene-state.py");
        return;
    };
    let config = std::env::var_os("CUEMOL_SCENE_STATE_CONFIG").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("state.txt");
    assert!(std::process::Command::new(oracle)
        .arg(config)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    let (_, mut state, _) = protocol();
    let (actual, _) = state.run_script_js("function report(step) { print(step+' '+Number(scene.modified)+' '+Number(scene.isJustCreated())+' '+scene.getUndoSize()); } report('initial'); scene.setName('untracked'); report('untracked'); scene.startUndoTxn('rename'); scene.setName('edited'); report('pending'); scene.commitUndoTxn(); report('committed'); scene.undo(0); report('undo'); scene.redo(0); report('redo'); scene.clearUndoData(); report('cleared'); scene.startUndoTxn('rollback'); scene.setName('cancelled'); scene.rollbackUndoTxn(); report('rollback'); scene.startUndoTxn('empty'); scene.commitUndoTxn(); report('empty');").unwrap();
    assert_eq!(
        actual.trim(),
        std::fs::read_to_string(output).unwrap().trim()
    );
}

#[cfg(feature = "scripting")]
#[test]
fn caught_script_writer_failure_preserves_morph_save_metadata() {
    let (_, mut state, _) = protocol();
    state.open_bytes("one.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let object = state.active_object_summaries()[0].uid;
    state.initialize_morph_object(object).unwrap();
    let before = state.clone();
    let directory = tempfile::tempdir().unwrap();
    let (log, _) = state.run_script_js(&format!(
        "var writer=cuemol.SceneXMLWriter(); writer.attach(scene); writer.setPath({}); try {{ writer.write(); }} catch(e) {{ print('failed'); }}",
        serde_json::to_string(&directory.path()).unwrap(),
    )).unwrap();
    assert_eq!(log, "failed\n");
    assert!(state.same_undo_state(&before));
}

#[test]
fn loading_a_saved_scene_is_clean_and_history_restores_its_status() {
    let (mut protocol, mut state, mut camera) = protocol();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.qsc");
    std::fs::write(
        &path,
        b"<scene bgcolor='red' _script_modified='true'><camera name='stored' zoom='20'/></scene>",
    )
    .unwrap();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "load_file",
        json!({"path":path}),
    );
    assert!(!modified(&mut state, &mut camera));
    assert_eq!(
        state
            .scene_manager()
            .scene(state.active_scene().unwrap())
            .unwrap()
            .scene_attribute("_script_modified"),
        None
    );
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"scene","prop":"bgcolor","value":"green"}),
    );
    assert!(modified(&mut state, &mut camera));
    call(&mut protocol, &mut state, &mut camera, "undo", json!({}));
    assert!(!modified(&mut state, &mut camera));
}

#[cfg(feature = "scripting")]
#[test]
fn a_scene_with_a_runtime_map_is_not_just_created() {
    let (_, mut state, _) = protocol();
    let scene = state.active_scene().unwrap();
    state.add_density_map(scene, "map", cuemol_xtal::DensityMap::new());
    assert!(!state.scene_is_just_created(scene));
    let (log, _) = state
        .run_script_js("print(scene.isJustCreated());")
        .unwrap();
    assert_eq!(log, "false\n");
}
