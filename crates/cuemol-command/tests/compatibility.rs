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
fn viewport_effect_properties_are_discoverable_validated_and_saved() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    molecule(&mut state, &mut camera, "effects", 0.0);
    let properties = call(
        &mut state,
        &mut camera,
        "list_node_props",
        json!({"nodeType":"scene"}),
    );
    for (key, expected) in [
        ("aoEnabled", "false"),
        ("aa_method", "fxaa"),
        ("aaJitterLevel", "0"),
        ("aaSmaaThreshold", "0.05"),
    ] {
        let row = properties["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == key)
            .unwrap();
        assert_eq!(row["value"], expected);
    }
    let changes = [
        ("aoEnabled", "true"),
        ("aoHalfRes", "true"),
        ("aoRadius", "6.2"),
        ("aa_method", "smaa"),
        ("aaJitterLevel", "3"),
        ("aaSmaaThreshold", "0.12"),
    ];
    for (key, value) in changes {
        call(
            &mut state,
            &mut camera,
            "set_prop",
            json!({"path":key,"value":value}),
        );
    }
    for (key, value) in [
        ("aoRadius", "NaN"),
        ("aaSmaaThreshold", "-1"),
        ("aa_method", "unknown"),
        ("aaJitterLevel", "6"),
    ] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_prop",
            &json!({"path":key,"value":value})
        )
        .is_err());
    }
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("effects.qsc", &bytes).unwrap();
    for (key, value) in changes {
        assert_eq!(
            call(&mut restored, &mut camera, "get_prop", json!({"path":key}))["value"],
            value
        );
    }
    call(
        &mut restored,
        &mut camera,
        "reset_prop",
        json!({"path":"aaSmaaThreshold"}),
    );
    assert_eq!(
        call(
            &mut restored,
            &mut camera,
            "get_prop",
            json!({"path":"aaSmaaThreshold"})
        )["value"],
        "0.05"
    );
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

#[test]
fn prefixed_npr_settings_change_hatching_and_keep_alpha() {
    use base64::Engine;
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let object = molecule(&mut state, &mut camera, "hatched", 0.);
    call(
        &mut state,
        &mut camera,
        "create_renderer",
        json!({"objId":object,"rendererType":"cpk"}),
    );
    call(
        &mut state,
        &mut camera,
        "center_view",
        json!({"objId":object,"zoom":true}),
    );
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"backend=umbreon_npr; width=96; height=96; transparentBg=true; umbreon_npr.hatchDensity=0.3; umbreon_npr.hatchStyle=ink-cross"}),
    );
    let first = call(&mut state, &mut camera, "get_screenshot", json!({}));
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"umbreon_npr.hatchDensity=3"}),
    );
    let second = call(&mut state, &mut camera, "get_screenshot", json!({}));
    assert_ne!(first["_image"], second["_image"]);
    let png = base64::engine::general_purpose::STANDARD
        .decode(second["_image"].as_str().unwrap())
        .unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; decoder.output_buffer_size()];
    let info = decoder.next_frame(&mut bytes).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert!(bytes.chunks_exact(4).any(|p| p[3] == 0));
    assert!(bytes.chunks_exact(4).any(|p| p[3] > 250 && p[0] < 128));
    assert!(bytes.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]));
}

#[test]
fn unsupported_render_engines_and_denoisers_do_not_change_settings() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    call(&mut state, &mut camera, "create_scene", json!({}));
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"width=24; denoise=Bilateral"}),
    );
    let before = cuemol_command::render_settings(&state).unwrap();
    for setting in [
        "width=96; backend=povray",
        "denoise=unknown",
        "umbreon.width=96",
    ] {
        assert!(cuemol_command::call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":setting})
        )
        .is_err());
        assert_eq!(cuemol_command::render_settings(&state).unwrap(), before);
    }
}

#[test]
fn analytic_surface_backends_regenerate_after_embedded_scene_roundtrip() {
    for backend in ["meshms", "ball", "rust"] {
        let mut state = DesktopAppState::new();
        let mut camera = Camera::new();
        let id = molecule(&mut state, &mut camera, "surface_source", 0.);
        let result = call(
            &mut state,
            &mut camera,
            "create_surface",
            json!({"objId":id,"name":"surface","backend":backend,"density":1.5}),
        );
        assert_eq!(result["backend"], backend);
        let surface = Uid::new(result["objectId"].as_u64().unwrap());
        assert_eq!(
            state.surface_summary(surface).unwrap().original_density,
            1.5
        );
        let result = call(
            &mut state,
            &mut camera,
            "recalc_surface",
            json!({"surfId":surface.get(),"density":3.}),
        );
        assert_eq!(result["backend"], backend);
        assert!(result["triangles"].as_u64().unwrap() > 0);
        assert_eq!(state.surface_summary(surface).unwrap().original_density, 3.);
        let bytes = state.active_scene_bytes().unwrap();
        let mut restored = DesktopAppState::new();
        restored.open_bytes("surface.qsc", &bytes).unwrap();
        let surface = restored.active_surface_summaries()[0].uid;
        let result = call(
            &mut restored,
            &mut camera,
            "recalc_surface",
            json!({"surfId":surface.get()}),
        );
        assert_eq!(result["backend"], backend);
        assert!(result["triangles"].as_u64().unwrap() > 0);
    }
}

#[test]
fn edited_hatch_specs_and_labels_roundtrip_and_invalid_edits_are_atomic() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    call(&mut state, &mut camera, "create_scene", json!({}));
    let style = call(
        &mut state,
        &mut camera,
        "get_hatch_style",
        json!({"style":"stipple"}),
    );
    assert!(style["spec"].as_str().unwrap().contains("kind=stipple"));
    let hatch = cuemol_render::HatchRenderParams {
        style: "manga".into(),
        coloring: "Colored ink on color fill".into(),
        layers_spec: "layer:kind=line,angle=33; layer:kind=dot,spacing=9".into(),
        tone_spec: "tone:strength=0.4,curve=0.7; ink:mincontrast=0.3".into(),
        ..Default::default()
    };
    let settings = cuemol_command::hatch_settings(&hatch);
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":settings}),
    );
    let before = cuemol_command::render_settings(&state).unwrap();
    let loaded = cuemol_command::hatch_params(&before, "umbreon_npr").unwrap();
    assert_eq!(loaded.layers_spec, hatch.layers_spec);
    assert_eq!(loaded.tone_spec, hatch.tone_spec);
    assert_eq!(loaded.coloring, hatch.coloring);
    for bad in [
        "umbreon_npr.hatchLayersSpec=\"layer:bad=3\"",
        "umbreon_npr.hatchStyle=unknown",
        "umbreon_npr.hatchToneSpec=\"tone:gamma=NaN\"",
    ] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":bad})
        )
        .is_err());
        assert_eq!(before, cuemol_command::render_settings(&state).unwrap());
    }
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("edited.qsc", &bytes).unwrap();
    assert_eq!(before, cuemol_command::render_settings(&restored).unwrap());
}

#[test]
fn lighting_settings_roundtrip_and_ground_color_validation_are_atomic() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    molecule(&mut state, &mut camera, "lighting", 0.);
    let settings="umbreon.lightIntensity=2; umbreon.flashFraction=0.3; umbreon.ambientFraction=0.4; umbreon.aoEnabled=true; umbreon.aoMultiScale=false; umbreon.aoGather=Per shading hit; umbreon.giSkyGradient=true; umbreon.giGroundColor=#aabbcc; umbreon.giBounces=3; umbreon.contactEdges=false; umbreon.perPixelBlend=false; umbreon.outlineFarDepth=0.7";
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":settings}),
    );
    let saved = cuemol_command::render_settings(&state).unwrap();
    let params = cuemol_command::rendering_params(&saved, "umbreon").unwrap();
    assert_eq!(params.light_intensity, 2.);
    assert_eq!(params.ao_samples, 64);
    assert!(!params.ao_per_pixel);
    assert!(!params.ao_multi_scale);
    assert_eq!(
        params.gi_ground_color,
        [170. / 255., 187. / 255., 204. / 255.]
    );
    assert_eq!(params.gi_bounces, 3);
    assert!(!params.per_pixel_blend);
    let encoded = cuemol_command::rendering_settings(&params, "umbreon");
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":encoded}),
    );
    assert_eq!(
        params,
        cuemol_command::rendering_params(
            &cuemol_command::render_settings(&state).unwrap(),
            "umbreon"
        )
        .unwrap()
    );
    let before = cuemol_command::render_settings(&state).unwrap();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_render_settings",
        &json!({"settings":"umbreon.giGroundColor=#bad; umbrella=1"})
    )
    .is_err());
    assert_eq!(before, cuemol_command::render_settings(&state).unwrap());
}

#[test]
fn camera_commands_use_screen_axes_and_current_viewport_height() {
    use cuemol_core::{Quat, Vector4};
    let mut state = DesktopAppState::new();
    state.new_scene("camera", true);
    state.put_runtime_value("viewport:width", "900");
    state.put_runtime_value("viewport:height", "450");
    let mut camera = Camera::new();
    let initial = Quat::IDENTITY
        .rotate_y(65.0)
        .unwrap()
        .rotate_z(-27.0)
        .unwrap();
    camera.set_rotation(initial);
    call(
        &mut state,
        &mut camera,
        "rotate_view",
        json!({"axis":"x","angle":30.0}),
    );
    let expected = initial.rotate_x(30.0).unwrap();
    let actual = camera.rotation();
    for (a, b) in [
        (actual.x, expected.x),
        (actual.y, expected.y),
        (actual.z, expected.z),
        (actual.w, expected.w),
    ] {
        assert!((a - b).abs() < 1.0e-12);
    }
    call(
        &mut state,
        &mut camera,
        "pan_view",
        json!({"dx":12.0,"dy":-7.0,"dz":4.0}),
    );
    let mut delta = Vector4::new3(12.0 * 50.0 / 450.0, 7.0 * 50.0 / 450.0, -1.0);
    expected.conj().to_matrix().xform3d(&mut delta);
    assert!((camera.center() + delta).length() < 1.0e-12);
    assert_eq!(camera.cam_dist(), 200.0);
}
