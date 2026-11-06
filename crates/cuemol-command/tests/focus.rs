use cuemol_command::{
    call_tool,
    console::{Action, Session},
    focus_nodes,
};
use cuemol_core::{Matrix4, Quat, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::{Camera, View};
use serde_json::json;

fn fixture() -> (DesktopAppState, View, Uid, Uid, Uid) {
    let mut state = DesktopAppState::new();
    let sid = state.new_scene("focus", true).result_scene;
    let pdb = "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
               ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C  \n\
               ATOM      3  CA  GLY B   1     100.000   0.000   0.000  1.00 10.00           C  \nEND\n";
    let molecule = cuemol_molstr::read_pdb_str(pdb).unwrap();
    let object = state
        .scene_manager_mut()
        .scene_mut(sid)
        .unwrap()
        .add_molecule("molecule", molecule);
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("simple", "chain_a");
    state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_selection(cuemol_molstr::parse_selector("chain A").unwrap());
    state.set_molecule_selection_text(object, "none").unwrap();
    let mut map = cuemol_xtal::DensityMap::new();
    map.set_map_byte_array(&[0, 255], [2, 1, 1], 0., 1., 0.5, 0.5);
    map.set_map_params([0; 3], [2, 1, 1]);
    map.set_xtal_params(20., 10., 10., 90., 90., 90.);
    let map_id = state.add_density_map(sid, "density", map);
    let mut view = View::new(Uid::new(1), sid);
    view.set_view_size(800, 400);
    (state, view, object, renderer, map_id)
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn mixed_focus_uses_renderer_selection_and_preserves_view_direction() {
    let (state, mut view, _, renderer, map) = fixture();
    view.set_view_dist(123.);
    assert!(focus_nodes(&state, &mut view, &[renderer, map, Uid::new(u64::MAX)]).unwrap());
    close(view.camera().center().x, 9.);
    close(view.camera().center().y, 5.);
    close(view.camera().zoom(), 11.);
    close(view.camera().slab_depth(), 10.);
    close(view.camera().cam_dist(), 123.);
    let rotation = Quat::new(
        0.,
        0.,
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    );
    let mut camera = view.camera();
    camera.set_rotation(rotation);
    view.set_camera(&camera);
    assert!(focus_nodes(&state, &mut view, &[renderer, map]).unwrap());
    close(view.camera().center().x, 9.);
    close(view.camera().center().y, 5.);
    close(view.camera().zoom(), 22.);
    assert_eq!(view.camera().rotation(), rotation);
}

#[test]
fn object_focus_ignores_live_selection_and_single_map_keeps_legacy_fit() {
    let (state, mut view, object, _, map) = fixture();
    assert!(focus_nodes(&state, &mut view, &[object]).unwrap());
    close(view.camera().center().x, 50.);
    close(view.camera().zoom(), 70.);
    let mut expected = view.clone();
    state.fit_density_map_view(map, &mut expected).unwrap();
    assert!(focus_nodes(&state, &mut view, &[map, Uid::new(u64::MAX)]).unwrap());
    assert_eq!(view.camera(), expected.camera());
    let before = view.camera();
    assert!(!focus_nodes(&state, &mut view, &[Uid::new(u64::MAX)]).unwrap());
    assert_eq!(view.camera(), before);
}

#[test]
fn focus_groups_transforms_and_empty_selections() {
    let (mut state, mut view, object, renderer, map) = fixture();
    let group = state.create_renderer_group(object, "group").unwrap();
    assert!(focus_nodes(&state, &mut view, &[group]).unwrap());
    close(view.camera().zoom(), 70.);
    let object = state.scene_manager_mut().object_mut(object).unwrap();
    object.set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(20., 0., 0.)));
    assert!(focus_nodes(&state, &mut view, &[renderer, map]).unwrap());
    close(view.camera().center().x, 16.);
    close(view.camera().zoom(), 16.);
    let object_id = state.active_object_summaries()[0].uid;
    state
        .scene_manager_mut()
        .object_mut(object_id)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_selection(cuemol_molstr::parse_selector("none").unwrap());
    let before = view.camera();
    assert!(!focus_nodes(&state, &mut view, &[renderer]).unwrap());
    assert_eq!(view.camera(), before);
    assert!(focus_nodes(&state, &mut view, &[renderer, map]).unwrap());
    close(view.camera().zoom(), 10.);
}

#[test]
fn focus_tool_and_console_resolve_lists_once_and_reject_missing_names() {
    let (mut state, _, object, renderer, map) = fixture();
    let mut camera = Camera::new();
    let result = call_tool(
        &mut state,
        &mut camera,
        "focus_nodes",
        &json!({"nodeIds":[renderer.get(),map.get()]}),
    )
    .unwrap();
    assert_eq!(result["ok"], true);
    close(camera.center().x, 9.);
    let mut session = Session::default();
    let actions = session
        .plan(&state, "focus_nodes molecule/chain_a * molecule")
        .unwrap();
    let calls = actions
        .iter()
        .filter_map(|action| {
            if let Action::Tool { name, args } = action {
                Some((name, args))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "focus_nodes");
    assert_eq!(
        calls[0].1["nodeIds"],
        json!([renderer.get(), object.get(), map.get()])
    );
    assert!(session
        .plan(&state, "focus_nodes molecule missing")
        .is_err());
    assert!(session
        .plan(&state, &format!("focus_nodes [{}]", object.get()))
        .is_ok());
}
