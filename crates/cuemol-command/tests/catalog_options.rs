use cuemol_command::{call_tool, scene_save_options};
use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

const PDB: &[u8] = b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       4.000   0.000   0.000  1.00 10.00           C\nATOM      3  CA  SER A   3       0.000   8.000   0.000  1.00 10.00           C\nEND\n";

fn fixture() -> (DesktopAppState, Camera, Uid) {
    let mut state = DesktopAppState::new();
    state.open_bytes("copy.pdb", PDB).unwrap();
    let object = state.active_object_summaries()[0].uid;
    (state, Camera::new(), object)
}

#[test]
fn renderer_creation_validates_group_and_selection_before_mutating() {
    let (mut state, mut camera, object) = fixture();
    call_tool(
        &mut state,
        &mut camera,
        "create_group",
        &json!({"objId":object.get(),"name":"bundle"}),
    )
    .unwrap();
    let before = state.clone();
    let preset = state
        .molecule_renderer_setup_options(state.active_scene().unwrap())
        .unwrap()
        .into_iter()
        .find(|option| option.preset)
        .unwrap()
        .name;
    for args in [
        json!({"objId":object.get(),"rendererType":"cpk","group":"missing"}),
        json!({"objId":object.get(),"rendererType":preset,"group":"bundle"}),
        json!({"objId":object.get(),"rendererType":"cpk","group":"bundle","selection":"invalid ((("}),
    ] {
        assert!(call_tool(&mut state, &mut camera, "create_renderer", &args).is_err());
        assert!(state.same_undo_state(&before));
    }
    let result = call_tool(&mut state, &mut camera, "create_renderer", &json!({
        "objId":object.get(),"rendererType":"cpk","name":"atoms","selection":"aid 1","group":"bundle","centerView":false
    })).unwrap();
    let renderer = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(Uid::new(result["rendererId"].as_u64().unwrap()))
        .unwrap();
    assert_eq!(renderer.group(), Some("bundle"));
    assert_eq!(renderer.selection().to_string(), "aid 1");
}

#[test]
fn new_renderer_framing_uses_its_selection_instead_of_the_current_molecule_selection() {
    let (mut state, mut camera, object) = fixture();
    state.set_molecule_selection_text(object, "aid 1").unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "create_renderer",
        &json!({
            "objId":object.get(),"rendererType":"cpk","centerView":true
        }),
    )
    .unwrap();
    assert_eq!([camera.center().x, camera.center().y], [2., 4.]);
    call_tool(
        &mut state,
        &mut camera,
        "create_renderer",
        &json!({
            "objId":object.get(),"rendererType":"cpk","selection":"aid 2","centerView":true
        }),
    )
    .unwrap();
    assert_eq!([camera.center().x, camera.center().y], [4., 0.]);
}

fn copy_and_paste(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    id: Uid,
    kind: &str,
    target: Value,
) -> Value {
    let before = state.clone();
    let mut args = call_tool(
        state,
        camera,
        "copy_node",
        &json!({"nodeId":id.get(),"nodeType":kind}),
    )
    .unwrap();
    assert!(state.same_undo_state(&before));
    assert!(!args["xml"].as_str().unwrap().contains('\u{fffd}'));
    args["target"] = target;
    call_tool(state, camera, "paste_node", &args).unwrap()
}

#[test]
fn copy_node_round_trips_objects_renderers_and_groups_as_json_text() {
    let (mut state, mut camera, id) = fixture();
    state
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(4., 6., 8.)));
    let result = copy_and_paste(&mut state, &mut camera, id, "object", Value::Null);
    let copy = state
        .scene_manager()
        .object(Uid::new(result["objectId"].as_u64().unwrap()))
        .unwrap();
    assert_eq!(copy.mol().atom_count(), 3);
    assert_eq!(copy.mol().atoms()[1].position, Vector4::new3(4., 0., 0.));
    assert_eq!(
        copy.xform_matrix(),
        state.scene_manager().object(id).unwrap().xform_matrix()
    );
    let (group, renderer) = {
        let object = state.scene_manager_mut().object_mut(id).unwrap();
        let group = object.add_renderer("*group", "bundle");
        let renderer = object.add_renderer("cpk", "spheres");
        object
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("group", "bundle");
        (group, renderer)
    };
    let single = copy_and_paste(
        &mut state,
        &mut camera,
        renderer,
        "renderer",
        json!(id.get()),
    );
    assert_eq!(single["rendererIds"].as_array().unwrap().len(), 1);
    let multi = copy_and_paste(&mut state, &mut camera, group, "rendGroup", json!(id.get()));
    assert_eq!(multi["rendererIds"].as_array().unwrap().len(), 1);
    assert!(multi["groupId"].as_u64().is_some());
    let dx=b"object 1 class gridpositions counts 1 1 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 1 1 2\nobject 3 class array type double rank 0 items 2 data follows\n-1 1\n";
    state.open_bytes("potential.dx", dx).unwrap();
    let map = state.active_density_map_summaries()[0].uid;
    let pasted = copy_and_paste(&mut state, &mut camera, map, "object", Value::Null);
    assert!(state
        .density_map_summary(Uid::new(pasted["objectId"].as_u64().unwrap()))
        .is_some());
}

#[test]
fn scene_save_options_round_trip_data_and_reject_incompatible_or_failed_writes() {
    for (version, compression, base64) in [
        ("QDF0", None, None),
        ("QDF1", Some("none"), Some(true)),
        ("QDF1", Some("gzip"), Some(false)),
        ("QDF1", Some("xzip"), Some(true)),
    ] {
        let (mut state, mut camera, _) = fixture();
        camera.set_center(Vector4::new3(10., 20., 30.));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.qsc");
        let args = json!({"path":path,"embedAll":true,"version":version,"compress":compression,"base64":base64});
        call_tool(&mut state, &mut camera, "save_scene", &args).unwrap();
        assert!(!state.scene_is_modified(state.active_scene().unwrap()));
        let mut loaded = DesktopAppState::new();
        loaded.open_path(&path).unwrap();
        assert_eq!(loaded.active_object_summaries()[0].atom_count, 3);
        assert_eq!(
            loaded.active_view_camera().unwrap().center(),
            camera.center()
        );
        let mut expected = scene_save_options(&state, &args).unwrap();
        expected.embed_all = false;
        assert_eq!(state.active_scene_save_options().unwrap(), expected);
        call_tool(&mut state, &mut camera, "close_scene", &json!({})).unwrap();
    }
    let (mut state, mut camera, _) = fixture();
    let before = state.clone();
    let directory = tempfile::tempdir().unwrap();
    camera.set_center(Vector4::new3(100., 200., 300.));
    for args in [
        json!({"path":directory.path().join("bad.qsc"),"version":"QDF0","base64":false}),
        json!({"path":directory.path().join("missing/bad.qsc"),"embedAll":true}),
    ] {
        assert!(call_tool(&mut state, &mut camera, "save_scene", &args).is_err());
        assert!(state.same_undo_state(&before));
        assert_eq!(state.active_view_camera(), before.active_view_camera());
    }
}

#[test]
fn superpose_recenters_only_when_requested() {
    let (mut state, mut camera, reference) = fixture();
    state.open_bytes("moving.pdb", PDB).unwrap();
    let moving = state
        .active_object_summaries()
        .iter()
        .find(|o| o.uid != reference)
        .unwrap()
        .uid;
    state
        .scene_manager_mut()
        .object_mut(moving)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(100., 0., 0.)));
    camera.set_center(Vector4::new3(-100., -200., -300.));
    let before = camera.clone();
    let mut args = json!({"refId":reference.get(),"movId":moving.get(),"algo":"LSQ"});
    call_tool(&mut state, &mut camera, "superpose", &args).unwrap();
    assert_eq!(camera, before);
    args["recenter"] = json!(true);
    call_tool(&mut state, &mut camera, "superpose", &args).unwrap();
    assert!((camera.center() - Vector4::new3(2., 4., 0.)).length() < 1e-6);
    camera.set_center(Vector4::new3(-100., -200., -300.));
    args["refSel"] = json!("");
    args["movSel"] = json!("  ");
    call_tool(&mut state, &mut camera, "superpose", &args).unwrap();
    assert!((camera.center() - Vector4::new3(2., 4., 0.)).length() < 1e-6);
}
