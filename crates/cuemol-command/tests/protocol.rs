use cuemol_command::Protocol;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

#[test]
#[cfg(feature = "scripting")]
fn query_scripts_preserve_redo_and_do_not_consume_undo_entries() {
    let mut state = DesktopAppState::new();
    state.open_bytes("probe.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let mut camera = state.active_view_camera().unwrap();
    let mut protocol = Protocol::new();
    request(
        &mut protocol,
        &mut state,
        &mut camera,
        "initialize",
        json!({}),
    );
    for source in ["print(1)", "print(scene.name)"] {
        assert_eq!(
            call(
                &mut protocol,
                &mut state,
                &mut camera,
                "execute_script",
                json!({"source":source})
            )["isError"],
            false
        );
    }
    assert_eq!(
        call(&mut protocol, &mut state, &mut camera, "undo", json!({}))["isError"],
        true
    );
    assert_eq!(
        call(
            &mut protocol,
            &mut state,
            &mut camera,
            "execute_script",
            json!({"source":"scene.setName('edited');"})
        )["isError"],
        false
    );
    for _ in 0..25 {
        assert_eq!(
            call(
                &mut protocol,
                &mut state,
                &mut camera,
                "execute_script",
                json!({"source":"print(scene.name);"})
            )["isError"],
            false
        );
    }
    assert_eq!(
        call(&mut protocol, &mut state, &mut camera, "undo", json!({}))["isError"],
        false
    );
    assert_ne!(state.active_scene_summary().unwrap().name, "edited");
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "execute_script",
        json!({"source":"print(scene.name);"}),
    );
    assert_eq!(
        call(&mut protocol, &mut state, &mut camera, "redo", json!({}))["isError"],
        false
    );
    assert_eq!(state.active_scene_summary().unwrap().name, "edited");
    assert_eq!(
        call(
            &mut protocol,
            &mut state,
            &mut camera,
            "execute_script",
            json!({"source":"scene.setName('failed'); throw new Error('fail');"})
        )["isError"],
        true
    );
    assert_eq!(state.active_scene_summary().unwrap().name, "edited");
}

#[test]
#[cfg(feature = "scripting")]
fn script_playback_pauses_and_restores_renderer_transform_through_protocol() {
    let mut state = DesktopAppState::new();
    state.open_bytes("animation.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
    let scene = state.active_scene().unwrap();
    let object = state.active_object_summaries()[0].uid;
    let renderer = state
        .new_renderer(scene, object, "cpk", "affine")
        .unwrap()
        .result_renderer;
    let mut camera = Camera::new();
    let mut protocol = Protocol::without_history();
    request(
        &mut protocol,
        &mut state,
        &mut camera,
        "initialize",
        json!({}),
    );
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "execute_script",
        json!({"source":format!(
        "function time(ms) {{var t=cuemol.TimeValue();t.millisec=ms;return t;}} \
         cuemol.getObject({}).name='animated'; \
         var manager=scene.getAnimMgr();manager.clear();manager.length=time(10000);manager.loop=true; \
         var motion=cuemol.RendXformAnim();motion.rend='animated/affine'; \
         motion.end_tran=cuemol.Vector('(6,0,0,0)');motion.end=time(10000);manager.append(motion); \
         manager.start(cuemol.getView(scene.activeViewID));",object.get())}),
    );
    state
        .advance_scene_animation_by(scene, std::time::Duration::from_millis(1500))
        .unwrap();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "execute_script",
        json!({"source":"scene.getAnimMgr().pause();"}),
    );
    let paused = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap()
        .xform_matrix();
    assert_ne!(paused, cuemol_core::Matrix4::IDENTITY);
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "animate",
        json!({"action":"stop"}),
    );
    assert_eq!(
        state
            .scene_manager()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .unwrap()
            .xform_matrix(),
        cuemol_core::Matrix4::IDENTITY
    );
}

fn request(
    protocol: &mut Protocol,
    state: &mut DesktopAppState,
    camera: &mut Camera,
    method: &str,
    params: Value,
) -> Value {
    protocol
        .handle(
            json!({"jsonrpc":"2.0","id":7,"method":method,"params":params}),
            state,
            camera,
        )
        .unwrap()
}
fn call(
    protocol: &mut Protocol,
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: Value,
) -> Value {
    let reply = request(
        protocol,
        state,
        camera,
        "tools/call",
        json!({"name":name,"arguments":args}),
    );
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}
fn decoded(reply: &Value) -> Value {
    serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn handshake_mutation_undo_and_failed_command_are_consistent() {
    let mut p = Protocol::new();
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    assert_eq!(
        request(&mut p, &mut state, &mut camera, "tools/list", json!({}))["error"]["code"],
        -32002
    );
    let hello = request(
        &mut p,
        &mut state,
        &mut camera,
        "initialize",
        json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    );
    assert_eq!(hello["result"]["protocolVersion"], "2025-06-18");
    assert!(p
        .handle(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            &mut state,
            &mut camera
        )
        .is_none());
    let tools = request(&mut p, &mut state, &mut camera, "tools/list", json!({}));
    assert!(tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "load_file"));
    let create = call(
        &mut p,
        &mut state,
        &mut camera,
        "create_scene",
        json!({"name":"test scene"}),
    );
    assert_eq!(create["isError"], false);
    assert_eq!(state.scene_summaries().len(), 1);
    let before = camera.zoom();
    let bad = call(
        &mut p,
        &mut state,
        &mut camera,
        "set_view",
        json!({"zoom":30,"slab":-2}),
    );
    assert_eq!(bad["isError"], true);
    assert_eq!(camera.zoom(), before);
    assert_eq!(
        call(
            &mut p,
            &mut state,
            &mut camera,
            "set_view",
            json!({"zoom":30,"slab":12})
        )["isError"],
        false
    );
    assert_eq!(camera.zoom(), 30.);
    assert_eq!(
        call(&mut p, &mut state, &mut camera, "undo", json!({}))["isError"],
        false
    );
    assert_eq!(camera.zoom(), before);
    assert_eq!(
        call(&mut p, &mut state, &mut camera, "redo", json!({}))["isError"],
        false
    );
    assert_eq!(camera.zoom(), 30.);
    assert_eq!(
        call(
            &mut p,
            &mut state,
            &mut camera,
            "set_view",
            json!({"center":[1,2]})
        )["isError"],
        true
    );
    assert_eq!(
        call(
            &mut p,
            &mut state,
            &mut camera,
            "get_view",
            json!({"unexpected":true})
        )["isError"],
        true
    );
    assert!(decoded(&create)["result"]["sceneId"].as_u64().is_some());
}

#[test]
fn loads_selects_and_saves_a_real_scene_without_node() {
    let temp = tempfile::tempdir().unwrap();
    let pdb = temp.path().join("ligand.pdb");
    std::fs::write(&pdb,"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\nHETATM    2  O1  LIG A   1       1.200   0.000   0.000  1.00 20.00           O\nEND\n").unwrap();
    let mut p = Protocol::new();
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    request(
        &mut p,
        &mut state,
        &mut camera,
        "initialize",
        json!({"protocolVersion":"2025-11-25"}),
    );
    let loaded = call(
        &mut p,
        &mut state,
        &mut camera,
        "load_file",
        json!({"path":pdb}),
    );
    assert_eq!(loaded["isError"], false, "{loaded}");
    let id = decoded(&loaded)["result"]["objectId"].as_u64().unwrap();
    let selected = call(
        &mut p,
        &mut state,
        &mut camera,
        "count_selection",
        json!({"objId":id,"selection":"elem O"}),
    );
    assert_eq!(decoded(&selected)["result"]["count"], 1);
    let state_response = decoded(&call(
        &mut p,
        &mut state,
        &mut camera,
        "get_scene_state",
        json!({}),
    ));
    assert!(state_response["result"]["objects"][0]["renderers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["type"] == "ballstick"));
    let output = temp.path().join("session.qsc");
    let saved = call(
        &mut p,
        &mut state,
        &mut camera,
        "save_scene",
        json!({"path":output}),
    );
    assert_eq!(saved["isError"], false, "{saved}");
    std::fs::remove_file(pdb).unwrap();
    let mut reloaded = DesktopAppState::new();
    reloaded.open_path(output).unwrap();
    assert_eq!(
        reloaded.object_summaries(reloaded.active_scene().unwrap())[0].atom_count,
        2
    );
    let rendered = call(
        &mut p,
        &mut state,
        &mut camera,
        "get_screenshot",
        json!({"width":32,"height":32}),
    );
    assert_eq!(rendered["isError"], false, "{rendered}");
    assert_eq!(rendered["content"][1]["type"], "image");
}
