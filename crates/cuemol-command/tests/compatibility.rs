use cuemol_command::call_tool;
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn call(state: &mut DesktopAppState, camera: &mut Camera, name: &str, args: Value) -> Value {
    call_tool(state, camera, name, &args).unwrap_or_else(|e| panic!("{name} {args}: {e}"))
}
fn molecule(state: &mut DesktopAppState, camera: &mut Camera, name: &str, shift: f64) -> u64 {
    let mut pdb = String::new();
    for i in 0..8 {
        let x = i as f64 * 3. + shift;
        let y = ((i as f64) * 1.7).sin() * 2.;
        let z = ((i as f64) * 0.9).cos() * 2.;
        pdb.push_str(&format!(
            "ATOM  {:5}  CA  ALA A{:4}    {:8.3}{:8.3}{:8.3}  1.00 10.00           C  \n",
            i + 1,
            i + 1,
            x,
            y,
            z
        ));
    }
    state
        .open_bytes(&format!("{name}.pdb"), pdb.as_bytes())
        .unwrap();
    let data = call(state, camera, "get_scene_state", json!({}));
    data["objects"].as_array().unwrap().last().unwrap()["id"]
        .as_u64()
        .unwrap()
}

#[test]
fn named_styles_paint_properties_and_saved_scene_roundtrip() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "protein", 0.);
    call(
        &mut state,
        &mut camera,
        "define_selection",
        json!({"name":"alpha","selection":"name CA"}),
    );
    assert_eq!(
        call(
            &mut state,
            &mut camera,
            "count_selection",
            json!({"objId":id,"selection":"alpha"})
        )["atomCount"],
        8
    );
    let r = call(
        &mut state,
        &mut camera,
        "create_renderer",
        json!({"objId":id,"rendererType":"ballstick","name":"atoms","selection":"alpha"}),
    )["rendererId"]
        .as_u64()
        .unwrap();
    call(
        &mut state,
        &mut camera,
        "add_paint",
        json!({"nodeId":r,"nodeType":"renderer","selection":"alpha","color":"red"}),
    );
    assert_eq!(
        call(
            &mut state,
            &mut camera,
            "list_paint",
            json!({"nodeId":r,"nodeType":"renderer"})
        )["entries"][0]["color"],
        "red"
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":r,"prop":"sphr","value":"0.6"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_prop",
        json!({"path":format!("#{r}.visible"),"value":"false"}),
    );
    assert_eq!(
        call(
            &mut state,
            &mut camera,
            "get_prop",
            json!({"path":format!("#{r}.visible")})
        )["value"],
        "false"
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"scene","prop":"bgcolor","value":"white"}),
    );
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("saved.qsc", &bytes).unwrap();
    let id = restored.active_object_summaries()[0].uid.get();
    assert_eq!(
        call(
            &mut restored,
            &mut camera,
            "count_selection",
            json!({"objId":id,"selection":"alpha"})
        )["atomCount"],
        8
    );
    assert_eq!(
        call(
            &mut restored,
            &mut camera,
            "get_prop",
            json!({"path":"bgcolor"})
        )["value"],
        "white"
    );
}

#[test]
fn measurement_references_follow_coordinate_changes() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "measure", 0.);
    let args = json!({"objId":id,"atoms":[{"chain":"A","resid":"1","atomName":"CA"},{"chain":"A","resid":"2","atomName":"CA"}],"draw":true,"label":"distance"});
    let measured = call(&mut state, &mut camera, "measure_geometry", args.clone());
    let r = Uid::new(measured["rendererId"].as_u64().unwrap());
    let object = state.scene_manager().object(Uid::new(id)).unwrap();
    let line = &object.renderer(r).unwrap().atom_intr_lines()[0];
    assert_eq!(line.aids(), ["A.1.CA", "A.2.CA"]);
    assert!(line.positions().is_empty());
    let mut coords = object
        .mol()
        .atoms()
        .iter()
        .map(|a| a.position)
        .collect::<Vec<_>>();
    coords[1].x += 4.;
    state
        .apply_coordinate_frame(
            Uid::new(id),
            &cuemol_mdtools::TrajectoryFrame {
                coordinates: coords
                    .iter()
                    .flat_map(|p| [p.x as f32, p.y as f32, p.z as f32])
                    .collect(),
                cell: None,
            },
        )
        .unwrap();
    let mut args = args;
    args["draw"] = json!(false);
    let updated = call(&mut state, &mut camera, "measure_geometry", args);
    assert!(updated["value"].as_f64().unwrap() > measured["value"].as_f64().unwrap() + 2.);
}

#[test]
fn animation_identity_camera_seek_and_saved_timeline() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    molecule(&mut state, &mut camera, "animated", 0.);
    let added = call(
        &mut state,
        &mut camera,
        "add_anim",
        json!({"type":"spin","name":"turn"}),
    );
    let id = added["elements"][0]["uid"].as_u64().unwrap();
    call(
        &mut state,
        &mut camera,
        "set_anim_time",
        json!({"element":format!("#{id}"),"startMs":0.,"endMs":2000.}),
    );
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":format!("#{id}"),"prop":"angle","value":"180"}),
    );
    let before = camera.rotation();
    call(
        &mut state,
        &mut camera,
        "animate",
        json!({"action":"seek","timeMs":1000.}),
    );
    assert_ne!(camera.rotation(), before);
    let listed = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(listed["elements"][0]["uid"], id);
    assert_eq!(listed["mgr"]["lengthMs"], 2000.);
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("animation.qsc", &bytes).unwrap();
    assert_eq!(
        call(&mut restored, &mut camera, "list_anims", json!({}))["elements"][0]["uid"],
        id
    );
}

#[test]
fn usalign_restores_known_rigid_translation() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let reference = molecule(&mut state, &mut camera, "reference", 0.);
    let moving = molecule(&mut state, &mut camera, "moving", 12.);
    call(
        &mut state,
        &mut camera,
        "superpose",
        json!({"refId":reference,"movId":moving}),
    );
    let a = state
        .scene_manager()
        .object(Uid::new(reference))
        .unwrap()
        .mol();
    let b = state
        .scene_manager()
        .object(Uid::new(moving))
        .unwrap()
        .mol();
    for (a, b) in a.atoms().iter().zip(b.atoms()) {
        assert!((a.position - b.position).length3d() < 1e-5);
    }
}

#[test]
fn stored_render_size_background_and_transparency_are_applied() {
    use base64::Engine;
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    call(&mut state, &mut camera, "create_scene", json!({}));
    call(
        &mut state,
        &mut camera,
        "set_prop",
        json!({"path":"bgcolor","value":"white"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"width=24; height=16; useGi=false; transparent=false"}),
    );
    let data = call(&mut state, &mut camera, "get_screenshot", json!({}));
    assert_eq!(data["width"], 24);
    assert_eq!(data["height"], 16);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data["_image"].as_str().unwrap())
        .unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; decoder.output_buffer_size()];
    decoder.next_frame(&mut bytes).unwrap();
    assert!(bytes.iter().all(|b| *b == 255));
}
