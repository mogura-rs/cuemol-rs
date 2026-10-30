use cuemol_command::call_tool;
use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::{LoadedLWObject, LoadedLWRenderer, LoadedSurface};
use cuemol_qsys::{Camera, CenterMark, StereoMode};
use serde_json::{json, Value};

fn call(state: &mut DesktopAppState, camera: &mut Camera, op: &str, args: Value) -> Value {
    call_tool(state, camera, op, &args).unwrap_or_else(|error| panic!("{op} {args}: {error}"))
}

fn fixture() -> (DesktopAppState, Camera, Vec<(Uid, Uid)>) {
    let mut state = DesktopAppState::new();
    let sid = state.new_scene("properties", true).result_scene;
    state.open_bytes("density.dx", b"object 1 class gridpositions counts 2 2 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n").unwrap();
    let map = state.density_map_summaries(sid)[0].clone();
    let data = "v 0 0 0\nv 2 0 0\nv 0 2 0\nf 1 2 3\n";
    state
        .scene_manager_mut()
        .scene_mut(sid)
        .unwrap()
        .add_loaded_surface(LoadedSurface {
            uid: Uid::new(0),
            name: "surface".into(),
            object_type: "MolSurfObj".into(),
            srctype: "obj".into(),
            data: data.into(),
            raw_data: data.as_bytes().to_vec(),
            qsc_node: None,
        });
    let surface = state.surface_summaries(sid)[0].clone();
    let lw = state
        .scene_manager_mut()
        .scene_mut(sid)
        .unwrap()
        .add_loaded_lw_object(LoadedLWObject {
            uid: Uid::new(0),
            name: "lightweight".into(),
            srctype: "qdflwobj".into(),
            raw_data: cuemol_lwview::write_qdf_lwobj_bytes(&cuemol_lwview::LWObject::new()),
            visible: true,
            xform: Matrix4::IDENTITY,
            renderers: vec![LoadedLWRenderer {
                name: "draw".into(),
                data_id: 0,
                visible: true,
                xform: Matrix4::IDENTITY,
            }],
            qsc_node: None,
        });
    let lw_renderer = state.lw_object_summary(lw).unwrap().renderers[0].uid;
    (
        state,
        Camera::new(),
        vec![
            (map.uid, map.renderers[0].uid),
            (surface.uid, surface.renderers[0].uid),
            (lw, lw_renderer),
        ],
    )
}

#[test]
fn density_kind_controls_region_policy_and_roundtrips_through_scene_storage() {
    let (mut state, mut camera, objects) = fixture();
    let sid = state.active_scene().unwrap();
    assert!(state.density_map_kind(objects[0].0).is_none());
    assert!(state.set_density_map_type(objects[0].0, "em").is_err());
    let mut map = cuemol_xtal::DensityMap::new();
    map.set_map_byte_array(&[0, 1, 2, 3, 4, 5, 6, 7], [2; 3], 0., 7., 3.5, 2.);
    map.set_map_params([0; 3], [2; 3]);
    map.set_xtal_params(2., 2., 2., 90., 90., 90.);
    let uid = state.add_density_map(sid, "crystal", map);
    let renderer = state.add_density_renderer(uid, "contour").unwrap();
    let metadata = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get()}),
    );
    let rows = metadata["properties"].as_array().unwrap();
    assert_eq!(
        rows.iter().find(|row| row["key"] == "width").unwrap()["type"],
        "real"
    );
    assert_eq!(
        rows.iter().find(|row| row["key"] == "autoupdate").unwrap()["hasdefault"],
        true
    );
    call(
        &mut state,
        &mut camera,
        "set_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get(),"updates":[{"prop":"autoupdate","value":"true"},{"prop":"dragupdate","value":"true"}]}),
    );
    let changed = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get()}),
    );
    assert_eq!(
        changed["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == "autoupdate")
            .unwrap()["isdefault"],
        false
    );
    call(
        &mut state,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get(),"props":["autoupdate","dragupdate"]}),
    );
    let reset = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get()}),
    );
    assert_eq!(
        reset["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == "autoupdate")
            .unwrap()["isdefault"],
        true
    );
    assert_eq!(
        state.auxiliary_renderer_properties(uid, renderer).unwrap()["dragupdate"],
        "false"
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"object","nodeId":uid.get(),"prop":"map_type","value":"em"}),
    );
    assert_eq!(state.density_map_kind(uid), Some(("em", "em")));
    let fields = |state: &mut DesktopAppState, camera: &mut Camera| {
        let result = call(
            state,
            camera,
            "list_node_props",
            json!({"nodeType":"renderer","nodeId":renderer.get()}),
        );
        result["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|section| section["rows"].as_array().unwrap())
            .map(|row| row["key"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let full = fields(&mut state, &mut camera);
    assert!(full.contains(&"lod_budget".into()));
    assert!(!full.contains(&"use_pbc".into()) && !full.contains(&"bufsize".into()));
    let before = state.clone();
    assert!(call_tool(&mut state,&mut camera,"set_node_props",&json!({"nodeType":"object","nodeId":uid.get(),"updates":[{"prop":"map_type","value":"xtal"},{"prop":"map_type_resolved","value":"em"}]})).is_err());
    assert!(before.same_undo_state(&state));
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("density.qsc", &bytes).unwrap();
    let restored_map = restored
        .active_density_map_summaries()
        .into_iter()
        .find(|map| map.name == "crystal")
        .unwrap();
    assert_eq!(
        restored.density_map_kind(restored_map.uid),
        Some(("em", "em"))
    );
    call(
        &mut state,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"object","nodeId":uid.get(),"props":["map_type"]}),
    );
    assert_eq!(state.density_map_kind(uid), Some(("auto", "xtal")));
    let boxed = fields(&mut state, &mut camera);
    assert!(!boxed.contains(&"lod_budget".into()));
    assert!(boxed.contains(&"use_pbc".into()) && boxed.contains(&"bufsize".into()));
}

#[test]
fn renderer_pages_gate_controls_and_keep_custom_detail_levels() {
    let mut state = DesktopAppState::new();
    state.open_bytes("pages.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let object = state.active_object_summaries()[0].uid;
    let mut camera = Camera::new();
    let mut list = |state: &mut DesktopAppState, renderer: Uid| {
        call(
            state,
            &mut camera,
            "list_node_props",
            json!({"nodeType":"renderer","nodeId":renderer.get()}),
        )
    };
    let fields = |result: &Value| {
        result["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|section| section["rows"].as_array().unwrap().iter().cloned())
            .map(|row| (row["key"].as_str().unwrap().to_string(), row))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ballstick", "atoms");
    state
        .set_renderer_property(object, renderer, "detail", "24")
        .unwrap();
    state
        .set_renderer_property(object, renderer, "ring", "false")
        .unwrap();
    let before = fields(&list(&mut state, renderer));
    assert_eq!(before["thickness"]["disabled"], true);
    assert_eq!(before["ringcolor"]["disabled"], true);
    assert_eq!(before["eglinew"]["disabled"], true);
    assert_eq!(before["alpha"]["max"], 1.);
    assert_eq!(before["bondw"]["unit"], "Å");
    let detail = before["detail"]["choices"].as_array().unwrap();
    assert!(detail.contains(&json!("24")) && detail.contains(&json!("16")));
    assert!(before["material"]["choices"]
        .as_array()
        .unwrap()
        .contains(&json!("")));
    state
        .set_renderer_property(object, renderer, "ring", "true")
        .unwrap();
    state
        .set_renderer_property(object, renderer, "egtype", "edges")
        .unwrap();
    let after = fields(&list(&mut state, renderer));
    assert_eq!(after["thickness"]["disabled"], false);
    assert_eq!(after["eglinew"]["disabled"], false);
    for kind in ["simple", "trace", "spline", "*namelabel", "*group"] {
        let renderer = state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer(kind, kind);
        let rows = fields(&list(&mut state, renderer));
        assert!(!rows.contains_key("egtype"), "{kind}");
        if kind == "*group" {
            assert_eq!(
                rows.keys().map(String::as_str).collect::<Vec<_>>(),
                ["locked", "name", "visible"]
            );
        }
    }
}

#[test]
fn interaction_arrow_caps_roundtrip_through_the_property_editor() {
    let mut state = DesktopAppState::new();
    state.open_bytes("measure.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let object = state.active_object_summaries()[0].uid;
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("atomintr", "measure");
    let mut camera = Camera::new();
    call(
        &mut state,
        &mut camera,
        "set_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get(),"updates":[{"prop":"mode","value":"fancy"},{"prop":"captype_start","value":"arrow"}]}),
    );
    let result = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get()}),
    );
    let row = result["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["key"] == "captype_start")
        .unwrap();
    assert_eq!(row["value"], "arrow");
    assert_eq!(row["allowed"], json!(["flat", "sphere", "arrow"]));
    let arrow_width = result["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|section| section["rows"].as_array().unwrap())
        .find(|row| row["key"] == "arrowwidth")
        .unwrap();
    assert_eq!(arrow_width["disabled"], false);
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("measure.qsc", &bytes).unwrap();
    let object = restored.active_object_summaries()[0].uid;
    let renderer = restored
        .scene_manager()
        .object(object)
        .unwrap()
        .renderers()
        .iter()
        .find(|renderer| renderer.name() == "measure")
        .unwrap()
        .uid();
    assert_eq!(
        restored
            .molecule_renderer_properties(object, renderer)
            .unwrap()["captype_start"],
        "arrow"
    );
}

#[test]
fn scene_context_properties_use_actual_ids_and_reject_cross_scene_targets() {
    let (mut state, mut camera, objects) = fixture();
    let sid = state.active_scene().unwrap();
    let list = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"scene","nodeId":sid.get()}),
    );
    let row = |key: &str| {
        list["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == key)
            .unwrap()
    };
    assert_eq!(row("name")["readonly"], true);
    assert_eq!(row("uid")["value"], sid.get().to_string());
    let ids = row("obj_uids")["value"]
        .as_str()
        .unwrap()
        .split(',')
        .map(|id| id.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    for (object, renderer) in objects {
        assert!(ids.contains(&object.get()));
        for (key, id) in [("activeObjID", object), ("activeRendID", renderer)] {
            call(
                &mut state,
                &mut camera,
                "set_node_prop",
                json!({"nodeType":"scene","prop":key,"value":id.get().to_string()}),
            );
            let scene = state.scene_manager().scene(sid).unwrap();
            assert_eq!(
                if key == "activeObjID" {
                    scene.active_object_uid()
                } else {
                    scene.active_renderer_uid()
                },
                Some(id)
            );
        }
    }
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_node_prop",
        &json!({"nodeType":"scene","prop":"activeObjID","value":"0"})
    )
    .is_err());
    let old = state.clone();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_node_prop",
        &json!({"nodeType":"scene","prop":"obj_uids","value":"7"})
    )
    .is_err());
    assert!(old.same_undo_state(&state));
    state.new_scene("second", true);
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_node_prop",
        &json!({"nodeType":"scene","nodeId":sid.get(),"prop":"name","value":"stale"})
    )
    .is_err());
    assert_eq!(
        state.scene_manager().scene(sid).unwrap().name(),
        "properties"
    );
}

#[test]
fn property_batches_roll_back_scene_and_camera_on_invalid_updates() {
    let mut state = DesktopAppState::new();
    state.new_scene("scene", true);
    let mut camera = Camera::new();
    for (kind, updates) in [
        (
            "scene",
            json!([{"prop":"aoRadius","value":"12"},{"prop":"aoSteps","value":"NaN"}]),
        ),
        (
            "view",
            json!([{"prop":"zoom","value":"25"},{"prop":"center","value":"invalid"}]),
        ),
    ] {
        let before = state.clone();
        let before_camera = camera.clone();
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_node_props",
            &json!({"nodeType":kind,"updates":updates})
        )
        .is_err());
        assert!(before.same_undo_state(&state));
        assert_eq!(before_camera, camera);
    }
}

#[test]
fn auxiliary_properties_edit_render_state_and_roundtrip() {
    let (mut state, mut camera, objects) = fixture();
    let transform = Matrix4::make_trans_mat(Vector4::new3(5.0, 2.0, -1.0));
    for (object, renderer) in &objects {
        for (kind, uid) in [("object", object), ("renderer", renderer)] {
            call(
                &mut state,
                &mut camera,
                "set_node_prop",
                json!({"nodeType":kind,"nodeId":uid.get(),"prop":"xformMat","value":transform.to_script_string()}),
            );
            assert_eq!(
                call(
                    &mut state,
                    &mut camera,
                    "get_prop",
                    json!({"path":format!("#{}.xformMat",uid.get())})
                )["value"],
                transform.to_script_string()
            );
        }
        for (key, value) in [
            ("alpha", "0.4"),
            ("egtype", "edges"),
            ("egroup", "shared"),
            ("material", "matte"),
        ] {
            call(
                &mut state,
                &mut camera,
                "set_node_prop",
                json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":key,"value":value}),
            );
        }
        assert_eq!(
            state.workspace_object_transform(*object).unwrap(),
            transform
        );
        assert_eq!(
            state
                .auxiliary_renderer_properties(*object, *renderer)
                .unwrap()["alpha"],
            "0.4"
        );
    }
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("properties.qsc", &bytes).unwrap();
    let sid = restored.active_scene().unwrap();
    let map = restored.density_map_summaries(sid)[0].clone();
    let surface = restored.surface_summaries(sid)[0].clone();
    let lw = restored.lw_object_summaries(sid)[0].clone();
    for (object, renderer) in [
        (map.uid, map.renderers[0].uid),
        (surface.uid, surface.renderers[0].uid),
        (lw.uid, lw.renderers[0].uid),
    ] {
        assert_eq!(
            restored.workspace_object_transform(object).unwrap(),
            transform
        );
        let values = restored
            .auxiliary_renderer_properties(object, renderer)
            .unwrap();
        assert_eq!(values["alpha"], "0.4");
        assert_eq!(values["egroup"], "shared");
        assert_eq!(values["xformMat"], transform.to_script_string());
        call(
            &mut restored,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.alpha",renderer.get())}),
        );
        call(
            &mut restored,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.xformMat",object.get())}),
        );
        assert_eq!(
            restored
                .auxiliary_renderer_properties(object, renderer)
                .unwrap()["alpha"],
            "1"
        );
        assert_eq!(
            restored.workspace_object_transform(object).unwrap(),
            Matrix4::IDENTITY
        );
        call(
            &mut restored,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.*",renderer.get())}),
        );
        let values = restored
            .auxiliary_renderer_properties(object, renderer)
            .unwrap();
        assert_eq!(values["egroup"], "");
        assert_eq!(values["xformMat"], Matrix4::IDENTITY.to_script_string());
    }
}

#[test]
fn auxiliary_property_errors_are_atomic_and_readonly_values_are_reported() {
    let (mut state, mut camera, objects) = fixture();
    let (map, renderer) = objects[0];
    let before = state.auxiliary_renderer_properties(map, renderer).unwrap();
    for (key, value) in [
        ("alpha", "NaN"),
        ("alpha", "2"),
        ("width", "wrong"),
        ("use_pbc", "maybe"),
        ("minLevel", "7"),
        ("missing", "1"),
        ("cap_mode", "wrong"),
    ] {
        assert!(
            call_tool(
                &mut state,
                &mut camera,
                "set_node_prop",
                &json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":key,"value":value})
            )
            .is_err(),
            "{key}"
        );
        assert_eq!(
            state.auxiliary_renderer_properties(map, renderer).unwrap(),
            before
        );
    }
    let properties = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"renderer","nodeId":renderer.get()}),
    );
    assert_eq!(
        properties["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["key"] == "minLevel")
            .unwrap()["readonly"],
        true
    );
    assert!(call_tool(
        &mut state,
        &mut camera,
        "get_prop",
        &json!({"path":format!("#{}.missing",renderer.get())})
    )
    .is_err());
}

#[test]
fn view_properties_update_the_live_view_and_camera() {
    let (mut state, mut camera, _) = fixture();
    for (key, value) in [
        ("stereoMode", "cross"),
        ("stereoDist", "2.5"),
        ("centerMark", "axis"),
        ("trans_mms", "true"),
        ("rot_mms", "true"),
        ("swapStereoEyes", "true"),
    ] {
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"view","prop":key,"value":value}),
        );
        assert_eq!(
            call(
                &mut state,
                &mut camera,
                "get_prop",
                json!({"path":format!("view.{key}")})
            )["value"],
            value
        );
    }
    assert_eq!(camera.stereo_mode(), StereoMode::Cross);
    assert_eq!(camera.stereo_distance(), 2.5);
    assert_eq!(camera.center_mark(), CenterMark::Axis);
    let uid = state.active_view_summary().unwrap().uid;
    let view = state.command_manager().view(uid).unwrap();
    assert_eq!(view.stereo_mode(), StereoMode::Cross);
    assert!(view.swap_stereo_eyes() && view.trans_mms() && view.rot_mms());
    for (key, value) in [
        ("stereoMode", "invalid"),
        ("stereoDist", "NaN"),
        ("width", "4"),
        ("missing", "true"),
    ] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_node_prop",
            &json!({"nodeType":"view","prop":key,"value":value})
        )
        .is_err());
    }
}

#[test]
fn auxiliary_renderer_groups_use_the_same_property_interface() {
    let (mut state, mut camera, objects) = fixture();
    for (object, _) in objects {
        let group = state.create_renderer_group(object, "group").unwrap();
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":group.get(),"prop":"alpha","value":"0.3"}),
        );
        assert_eq!(
            call(
                &mut state,
                &mut camera,
                "get_prop",
                json!({"path":format!("#{}.alpha",group.get())})
            )["value"],
            "0.3"
        );
        let objects = call(&mut state, &mut camera, "get_scene_state", json!({}));
        let object_name = objects["objects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == object.get())
            .unwrap()["name"]
            .as_str()
            .unwrap();
        let typed_path = format!("{object_name}/group@*group#{}.alpha", group.get());
        assert_eq!(
            call(
                &mut state,
                &mut camera,
                "get_prop",
                json!({"path":typed_path})
            )["value"],
            "0.3"
        );
        call(
            &mut state,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.alpha",group.get())}),
        );
        assert_eq!(
            call(
                &mut state,
                &mut camera,
                "get_prop",
                json!({"path":format!("#{}.alpha",group.get())})
            )["value"],
            "1"
        );
        call(
            &mut state,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.*",group.get())}),
        );
        assert_eq!(
            state.auxiliary_renderer_properties(object, group).unwrap()["name"],
            "group"
        );
    }
}

#[test]
fn auxiliary_renderers_can_be_listed_created_and_named() {
    let (mut state, mut camera, objects) = fixture();
    for ((object, _), kind) in objects.into_iter().zip(["isosurf", "molsurf", "lwrend"]) {
        let listed = call(
            &mut state,
            &mut camera,
            "list_renderer_types",
            json!({"objId":object.get()}),
        );
        assert!(listed["rendererTypes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == kind));
        let result = call(
            &mut state,
            &mut camera,
            "create_renderer",
            json!({"objId":object.get(),"rendererType":kind,"name":"created"}),
        );
        let renderer = Uid::new(result["rendererId"].as_u64().unwrap());
        assert_eq!(
            state
                .auxiliary_renderer_properties(object, renderer)
                .unwrap()["name"],
            "created"
        );
    }
}

#[cfg(feature = "scripting")]
#[test]
fn scripts_preserve_auxiliary_groups_and_duplicate_renderer_identities() {
    let (mut state, mut camera, objects) = fixture();
    for ((object, first), kind) in objects.into_iter().zip(["isosurf", "molsurf", "lwrend"]) {
        let group = state.create_renderer_group(object, "kept").unwrap();
        let result = call(
            &mut state,
            &mut camera,
            "create_renderer",
            json!({"objId":object.get(),"rendererType":kind,"name":""}),
        );
        let second = Uid::new(result["rendererId"].as_u64().unwrap());
        state
            .set_auxiliary_renderer_property(object, first, "name", "")
            .unwrap();
        state
            .set_auxiliary_renderer_property(object, first, "alpha", "0.2")
            .unwrap();
        state
            .set_auxiliary_renderer_property(object, second, "alpha", "0.8")
            .unwrap();
        state
            .set_auxiliary_renderer_property(object, group, "alpha", "0.6")
            .unwrap();
        let sid = state.active_scene().unwrap();
        let (log,_)=state.run_script_js(&format!("var scene=cuemol.getScene({}); var a=scene.getRenderer({}); var b=scene.getRenderer({}); var g=scene.getRenderer({}); print(a.alpha+':'+b.alpha+':'+g.alpha); print(a.name+':'+b.name); b.alpha=0.7;",sid.get(),first.get(),second.get(),group.get())).unwrap();
        assert_eq!(log, "0.2:0.8:0.6\n:\n");
        assert_eq!(
            state.auxiliary_renderer_properties(object, first).unwrap()["alpha"],
            "0.2"
        );
        assert_eq!(
            state.auxiliary_renderer_properties(object, second).unwrap()["alpha"],
            "0.7"
        );
        assert_eq!(
            state.auxiliary_renderer_properties(object, group).unwrap()["alpha"],
            "0.6"
        );
        if kind != "lwrend" {
            state.run_script_js(&format!("var a=cuemol.getRenderer({});var b=cuemol.getRenderer({});a.multi_grad.setNodesJSON('[{{\"value\":0,\"color\":\"red\"}}]');b.multi_grad.setNodesJSON('[{{\"value\":0,\"color\":\"blue\"}}]');", first.get(), second.get())).unwrap();
            let (log,_) = state.run_script_js(&format!("print(cuemol.getRenderer({}).multi_grad.getColorAt(0).r());print(cuemol.getRenderer({}).multi_grad.getColorAt(0).b());", first.get(),second.get())).unwrap();
            assert_eq!(log, "255\n255\n", "{kind}");
        }
    }
}

#[test]
fn molecule_property_reset_updates_typed_selection_and_name() {
    let mut state = DesktopAppState::new();
    state.open_bytes("molecule.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nHETATM    2  O1  LIG A   1       1.000   0.000   0.000  1.00 10.00           O  \nEND\n").unwrap();
    let sid = state.active_scene().unwrap();
    let object = state.scene_manager().scene(sid).unwrap().objects()[0].uid();
    let renderer = state.scene_manager().object(object).unwrap().renderers()[0].uid();
    let mut camera = Camera::new();
    for (key, value) in [("sel", "aid 1"), ("name", "changed"), ("visible", "false")] {
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":key,"value":value}),
        );
        call(
            &mut state,
            &mut camera,
            "reset_prop",
            json!({"path":format!("#{}.{key}",renderer.get())}),
        );
    }
    let target = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap();
    assert_eq!(target.selection(), &cuemol_molstr::Selector::All);
    assert_eq!(target.name(), "");
    assert!(target.is_visible());
    for (key, value) in [
        ("alpha", "NaN"),
        ("alpha", "2"),
        ("egtype", "wrong"),
        ("xformMat", "invalid"),
    ] {
        assert!(
            call_tool(
                &mut state,
                &mut camera,
                "set_node_prop",
                &json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":key,"value":value})
            )
            .is_err(),
            "{key}"
        );
    }
}
