use cuemol_command::{
    call_tool,
    console::{Action, Session},
    validate_args,
};
use cuemol_core::{Matrix4, Quat, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn fixture() -> (DesktopAppState, Uid) {
    let mut state = DesktopAppState::new();
    state.open_bytes("bounds.pdb", b"ATOM      1  CA  ALA A   1       2.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       8.000   4.000  10.000  1.00 10.00           C\nEND\n").unwrap();
    let id = state.active_object_summaries()[0].uid;
    state
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(0., 0., 100.)));
    state.set_molecule_selection_text(id, "none").unwrap();
    (state, id)
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn fit_slab_uses_current_center_rotation_and_transformed_all_atom_bounds() {
    let (mut state, id) = fixture();
    let mut camera = Camera::new();
    camera.set_center(Vector4::new3(0., 0., 150.));
    camera.set_cam_dist(200.);
    camera.set_zoom(47.);
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"fitSlab":true,"objId":id.get()}),
    )
    .unwrap();
    close(camera.slab_depth(), 110.);
    close(camera.zoom(), 47.);
    assert_eq!(camera.center(), Vector4::new3(0., 0., 150.));
    let q = Quat::from_axis_angle(Vector4::new3(0., 1., 0.), std::f64::consts::FRAC_PI_2).unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"fitSlab":true,"center":[0,0,0],"rotation":[q.w,q.x,q.y,q.z]}),
    )
    .unwrap();
    close(camera.slab_depth(), 110.);
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"fitSlab":true}),
    )
    .unwrap();
    close(camera.slab_depth(), 17.6);
}

#[test]
fn set_view_validates_before_commit_and_normalizes_scalar_first_rotation() {
    let (mut state, _) = fixture();
    let mut camera = Camera::new();
    for args in [
        json!({"zoom":30,"center":[1,2]}),
        json!({"zoom":30,"rotation":[1,0,0]}),
        json!({"zoom":30,"rotation":[1,0,"bad",0]}),
        json!({"zoom":30,"fitSlab":true,"slab":12}),
        json!({"zoom":30,"fitSlab":true,"objId":99999999}),
    ] {
        let before = camera.clone();
        assert!(call_tool(&mut state, &mut camera, "set_view", &args).is_err());
        assert_eq!(camera, before);
    }
    for (input, expected) in [
        ([2., 0., 0., 0.], Quat::IDENTITY),
        ([0., 0., 0., 0.], Quat::IDENTITY),
        ([0., 0., 2., 0.], Quat::new(0., 1., 0., 0.)),
    ] {
        let args = json!({"rotation":input});
        validate_args("set_view", &args).unwrap();
        call_tool(&mut state, &mut camera, "set_view", &args).unwrap();
        assert_eq!(camera.rotation(), expected);
    }
    let actions = Session::default()
        .plan(&state, "view rotation=1 0 0 0")
        .unwrap();
    assert!(actions.iter().any(|action| matches!(action, Action::Tool {name,args} if name=="set_view" && args["rotation"]==json!([1.,0.,0.,0.]))));
}

#[test]
fn set_view_clamps_values_in_reference_order_and_skips_map_only_scenes() {
    let (mut state, _) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"distance":500,"slab":800}),
    )
    .unwrap();
    close(camera.slab_depth(), 800.);
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"distance":10,"slab":800}),
    )
    .unwrap();
    close(camera.slab_depth(), 20.);
    call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"zoom":-3,"distance":-2,"slab":-1}),
    )
    .unwrap();
    close(camera.zoom(), 0.01);
    close(camera.cam_dist(), 0.1);
    close(camera.slab_depth(), 0.1);
    let scene = state.new_scene("maps", true).result_scene;
    let mut map = cuemol_xtal::DensityMap::new();
    map.set_map_byte_array(&[0, 255], [2, 1, 1], 0., 1., 0.5, 0.5);
    map.set_map_params([0; 3], [2, 1, 1]);
    map.set_xtal_params(20., 10., 10., 90., 90., 90.);
    state.add_density_map(scene, "density", map);
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_view",
        &json!({"fitSlab":true})
    )
    .is_err());
}
