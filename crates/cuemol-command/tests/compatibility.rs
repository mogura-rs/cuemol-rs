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
fn later_paint_overrides_earlier_regions_and_explicit_insert_preserves_selection() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "paint", 0.);
    let renderer = call(
        &mut state,
        &mut camera,
        "create_renderer",
        json!({"objId":id,"rendererType":"ballstick","name":"atoms"}),
    )["rendererId"]
        .as_u64()
        .unwrap();
    for (selection, color) in [("*", "red"), ("resid 1", "blue")] {
        call(
            &mut state,
            &mut camera,
            "add_paint",
            json!({"nodeId":renderer,"nodeType":"renderer","selection":selection,"color":color}),
        );
    }
    let entries = call(
        &mut state,
        &mut camera,
        "list_paint",
        json!({"nodeId":renderer,"nodeType":"renderer"}),
    );
    assert_eq!(entries["entries"][0]["color"], "blue");
    assert_eq!(entries["entries"][1]["color"], "red");
    assert_eq!(
        state.molecule_selection_text(Uid::new(id)).unwrap(),
        "resid 1"
    );
    call(
        &mut state,
        &mut camera,
        "add_paint_at",
        json!({"nodeId":renderer,"nodeType":"renderer","selection":"resid 2","color":"green","at":1}),
    );
    assert_eq!(
        state.molecule_selection_text(Uid::new(id)).unwrap(),
        "resid 1"
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
fn command_labels_use_live_validated_formats() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "labels", 0.0);
    let result = call(
        &mut state,
        &mut camera,
        "set_atom_labels",
        json!({"objId":id,"selection":"aid 1","format":"{molname}:{bfac:.2f}:{aid:03d}"}),
    );
    let renderer = Uid::new(result["rendererId"].as_u64().unwrap());
    let text = |state: &DesktopAppState| {
        let object = state.scene_manager().object(Uid::new(id)).unwrap();
        object.name_label_entries(object.renderer(renderer).unwrap())[0]
            .1
            .clone()
    };
    assert_eq!(text(&state), "labels.pdb:10.00:001");
    state
        .scene_manager_mut()
        .object_mut(Uid::new(id))
        .unwrap()
        .mol_mut()
        .atoms_mut()[0]
        .b_factor = 23.75;
    assert_eq!(text(&state), "labels.pdb:23.75:001");
    let before = state.active_scene_bytes().unwrap();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_atom_labels",
        &json!({"objId":id,"selection":"aid 1","format":"{missing}"})
    )
    .is_err());
    assert_eq!(state.active_scene_bytes().unwrap(), before);
    let mut restored = DesktopAppState::new();
    restored.open_bytes("labels.qsc", &before).unwrap();
    let object = restored
        .scene_manager()
        .scene(restored.active_scene().unwrap())
        .unwrap()
        .objects()
        .first()
        .unwrap();
    let renderer = object
        .renderers()
        .iter()
        .find(|renderer| renderer.renderer_type() == "*namelabel")
        .unwrap();
    assert_eq!(
        object.name_label_entries(renderer)[0].1,
        "labels.pdb:23.75:001"
    );
}

#[test]
fn symmetry_copy_applies_row_major_matrix_coordinates() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "original", 0.0);
    let result = call(
        &mut state,
        &mut camera,
        "create_symmetry_copy",
        json!({"objId":id,"name":"translated","matrix":"1 0 0 7 0 1 0 -3 0 0 1 2 0 0 0 1","rendererType":"cpk"}),
    );
    let original = state.scene_manager().object(Uid::new(id)).unwrap();
    let copy = state
        .scene_manager()
        .object(Uid::new(result["objectId"].as_u64().unwrap()))
        .unwrap();
    for (a, b) in original.mol().atoms().iter().zip(copy.mol().atoms()) {
        assert!(
            (b.position - a.position - cuemol_core::Vector4::new3(7.0, -3.0, 2.0)).length3d()
                < 1e-9
        );
    }
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
    let before = state.clone();
    let preview = call(
        &mut state,
        &mut camera,
        "superpose",
        json!({"refId":reference,"movId":moving,"transform":false}),
    );
    assert_eq!(preview["transformed"], false);
    assert!(preview["rmsd"].as_f64().unwrap() < 1e-5);
    assert_eq!(preview["alignedCount"], 8);
    assert!(before.same_undo_state(&state));
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
fn render_commands_share_typed_scene_data_and_migrate_legacy_aliases() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let sid = state.new_scene("settings", true).result_scene;
    state
        .scene_manager_mut()
        .scene_mut(sid)
        .unwrap()
        .set_scene_attribute(
            "render_settings",
            r##"{"width":320,"transparent":true,"useGi":false,"umbreon.giBounces":3}"##,
        );
    let migrated = cuemol_command::render_settings(&state).unwrap();
    assert_eq!(migrated["width"], 320);
    assert_eq!(migrated["transparentBg"], true);
    assert_eq!(migrated["umbreon.useGI"], false);
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"width=640; povray.shadow=true"}),
    );
    let scene = state.scene_manager().scene(sid).unwrap();
    let data = scene.app_data().get("render").unwrap();
    let uid = data.uid();
    assert_eq!(data.get("width"), Some("640"));
    assert_eq!(data.get("povray.shadow"), Some("true"));
    let extensions: Value =
        serde_json::from_str(scene.scene_attribute("render_settings").unwrap()).unwrap();
    assert_eq!(extensions, json!({"umbreon.giBounces":3}));
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_render_settings",
        &json!({"settings":"width=1200; umbrella=1"})
    )
    .is_err());
    assert_eq!(
        cuemol_command::render_settings(&state).unwrap()["width"],
        640
    );
    call(
        &mut state,
        &mut camera,
        "set_render_settings",
        json!({"settings":"width=1200"}),
    );
    let scene = state.scene_manager_mut().scene_mut(sid).unwrap();
    assert_eq!(scene.app_data().get("render").unwrap().uid(), uid);
    scene.set_scene_attribute("render_settings", r#"{"width":320,"umbreon.giBounces":3}"#);
    assert_eq!(
        cuemol_command::render_settings(&state).unwrap()["width"],
        1200
    );
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("settings.qsc", &bytes).unwrap();
    assert_eq!(
        cuemol_command::render_settings(&state).unwrap(),
        cuemol_command::render_settings(&restored).unwrap()
    );
    assert!(restored
        .scene_manager()
        .scene(restored.active_scene().unwrap())
        .unwrap()
        .write_qsc_string()
        .contains("appdata"));
}

#[test]
fn lighting_settings_roundtrip_and_ground_color_validation_are_atomic() {
    let defaults = cuemol_command::rendering_params(&serde_json::Map::new(), "umbreon").unwrap();
    assert_eq!(defaults.gi_bounces, 1);
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
    assert_eq!(params.ao_res_div, 0);
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

#[test]
fn centering_uses_selection_centroids_and_fitting_uses_transformed_view_bounds() {
    use cuemol_core::{Matrix4, Vector4};
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "center", 0.0);
    let object = state.scene_manager_mut().object_mut(Uid::new(id)).unwrap();
    object
        .mol_mut()
        .set_selection(cuemol_molstr::parse_selector("aid 1:3").unwrap());
    object.set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(10., 20., 30.)));
    let expected = object.mol().get_center_pos(true).unwrap() + Vector4::new3(10., 20., 30.);
    call(
        &mut state,
        &mut camera,
        "center_view",
        json!({"objId":id,"zoom":false}),
    );
    assert!((camera.center() - expected).length() < 1e-10);
    camera.set_cam_dist(111.);
    call(
        &mut state,
        &mut camera,
        "center_view",
        json!({"objId":id,"selection":"aid 1","zoom":true}),
    );
    assert_eq!(camera.center(), Vector4::new3(10., 20., 32.));
    assert_eq!(
        (camera.zoom(), camera.slab_depth(), camera.cam_dist()),
        (4., 4., 111.)
    );
}

#[test]
fn property_paths_match_dotted_names_object_precedence_and_uid_references() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "dotted", 0.);
    call(
        &mut state,
        &mut camera,
        "rename_node",
        json!({"nodeType":"object","nodeId":id,"name":"1crn.pdb"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_prop",
        json!({"path":"1crn.pdb.visible","value":"false"}),
    );
    assert!(!state
        .scene_manager()
        .object(Uid::new(id))
        .unwrap()
        .is_visible());
    call(
        &mut state,
        &mut camera,
        "set_prop",
        json!({"path":format!("#{id}.visible"),"value":"true"}),
    );
    assert!(state
        .scene_manager()
        .object(Uid::new(id))
        .unwrap()
        .is_visible());
    call(
        &mut state,
        &mut camera,
        "rename_node",
        json!({"nodeType":"object","nodeId":id,"name":"view"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_prop",
        json!({"path":"view.visible","value":"false"}),
    );
    assert!(!state
        .scene_manager()
        .object(Uid::new(id))
        .unwrap()
        .is_visible());
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"view","prop":"zoom","value":"25"}),
    );
    assert_eq!(camera.zoom(), 25.);
}

fn property_row(state: &mut DesktopAppState, camera: &mut Camera, id: u64, key: &str) -> Value {
    node_property_row(state, camera, "renderer", id, key)
}

fn node_property_row(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    kind: &str,
    id: u64,
    key: &str,
) -> Value {
    call(
        state,
        camera,
        "list_node_props",
        json!({"nodeType":kind,"nodeId":id}),
    )["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["key"] == key)
        .unwrap()
        .clone()
}

#[test]
fn inspector_object_default_flags_survive_save_and_query_scripts() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "object-defaults", 0.);
    let values = [
        ("visible", "true"),
        ("locked", "false"),
        ("ui_collapsed", "false"),
        ("xformMat", "(identity)"),
    ];
    for (key, value) in values {
        assert_eq!(
            node_property_row(&mut state, &mut camera, "object", id, key)["isdefault"],
            true,
            "{key}"
        );
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"object","nodeId":id,"prop":key,"value":value}),
        );
        assert_eq!(
            node_property_row(&mut state, &mut camera, "object", id, key)["isdefault"],
            false,
            "{key}"
        );
    }
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("defaults.qsc", &bytes).unwrap();
    let restored_id = restored.active_object_summaries()[0].uid.get();
    for (key, _) in values {
        assert_eq!(
            node_property_row(&mut restored, &mut camera, "object", restored_id, key)["isdefault"],
            false,
            "{key}"
        );
    }
    call(
        &mut restored,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"object","nodeId":restored_id}),
    );
    #[cfg(feature = "scripting")]
    {
        let before = restored.clone();
        restored
            .run_script_js("print(cuemol.getObject('object-defaults.pdb').visible);")
            .unwrap();
        assert!(before.same_undo_state(&restored));
        restored
            .run_script_js(
                "var o=cuemol.getObject('object-defaults.pdb'); o.visible=true; o.locked=false;",
            )
            .unwrap();
        for key in ["visible", "locked"] {
            assert_eq!(
                node_property_row(&mut restored, &mut camera, "object", restored_id, key)
                    ["isdefault"],
                false,
                "{key}"
            );
        }
        restored.run_script_js("var o=cuemol.getObject('object-defaults.pdb'); cuemol.resetProp(o,'visible'); cuemol.resetProp(o,'locked');").unwrap();
    }
    for (key, _) in values {
        assert_eq!(
            node_property_row(&mut restored, &mut camera, "object", restored_id, key)["isdefault"],
            true,
            "{key}"
        );
    }
    let bytes = restored.active_scene_bytes().unwrap();
    let xml = String::from_utf8_lossy(&bytes);
    let tag = xml.lines().find(|line| line.contains("<object ")).unwrap();
    for (key, _) in values {
        assert!(!tag.contains(&format!(" {key}=\"")), "{tag}");
    }
}

#[test]
fn inspector_object_default_color_changes_render_colors_and_roundtrips() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let id = molecule(&mut state, &mut camera, "object-color", 0.);
    state
        .set_object_coloring(
            Uid::new(id),
            Some("SolidColoring".into()),
            Default::default(),
        )
        .unwrap();
    let renderer = state
        .scene_manager_mut()
        .object_mut(Uid::new(id))
        .unwrap()
        .add_renderer("cpk", "color");
    state
        .set_renderer_coloring_details(
            Uid::new(id),
            renderer,
            Some("SolidColoring".into()),
            Default::default(),
            Vec::new(),
            Some("$molcol".into()),
        )
        .unwrap();
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":"defaultcolor","value":"$molcol"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"object","nodeId":id,"prop":"defaultcolor","value":"#2468ac"}),
    );
    let assert_color = |state: &DesktopAppState, expected: [u8; 3], stage: &str| {
        let snapshot = state.active_render_snapshot("").unwrap();
        assert_eq!(snapshot.atoms.len(), 8);
        for atom in snapshot.atoms {
            let rgba = atom.color.unwrap().rgba8().unwrap();
            assert_eq!(
                [rgba.r, rgba.g, rgba.b],
                expected,
                "{stage}: {:?}",
                state.active_object_summaries()
            );
        }
    };
    assert_color(&state, [0x24, 0x68, 0xac], "set");
    let mut restored = DesktopAppState::new();
    restored
        .open_bytes("color.qsc", &state.active_scene_bytes().unwrap())
        .unwrap();
    assert_color(&restored, [0x24, 0x68, 0xac], "reload");
    let id = restored.active_object_summaries()[0].uid.get();
    call(
        &mut restored,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"object","nodeId":id,"props":["defaultcolor"]}),
    );
    assert_color(&restored, [255, 255, 255], "reset");
    #[cfg(feature = "scripting")]
    {
        restored
            .run_script_js(
                "var o=cuemol.getObject('object-color.pdb'); o.defaultcolor=cuemol.makeColor('blue');",
            )
            .unwrap();
        assert_color(&restored, [0, 0, 255], "script");
        restored
            .run_script_js(
                "var o=cuemol.getObject('object-color.pdb'); cuemol.resetProp(o,'defaultcolor');",
            )
            .unwrap();
        assert_color(&restored, [255, 255, 255], "reset");
        assert_eq!(
            node_property_row(&mut restored, &mut camera, "object", id, "defaultcolor")
                ["isdefault"],
            true
        );
    }
}

#[test]
fn inspector_nested_defaults_match_reference_and_survive_scene_reload() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let object = Uid::new(molecule(&mut state, &mut camera, "properties", 0.));
    let id = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ribbon", "plain");
    let initial = property_row(&mut state, &mut camera, id.get(), "helixhead.type");
    assert_eq!(initial["value"], "arrow");
    assert_eq!(initial["defaultValue"], "smooth");
    assert_eq!(initial["isdefault"], true);
    assert_eq!(initial["allowed"], json!(["arrow", "flat", "smooth"]));
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helix.detail")["type"],
        "integer"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "style")["readonly"],
        true
    );
    for (key, value) in [
        ("helixhead.type", "flat"),
        ("helix.width", "2.5"),
        ("alpha", "1"),
    ] {
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":id.get(),"prop":key,"value":value}),
        );
        assert_eq!(
            property_row(&mut state, &mut camera, id.get(), key)["isdefault"],
            false
        );
    }
    for (key, value) in [
        ("helixhead.type", "unknown"),
        ("helix.detail", "1.2"),
        ("helix.width", "NaN"),
    ] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_node_prop",
            &json!({"nodeType":"renderer","nodeId":id.get(),"prop":key,"value":value})
        )
        .is_err());
    }
    assert!(call_tool(
        &mut state,
        &mut camera,
        "reset_node_props",
        &json!({"nodeType":"renderer","nodeId":id.get(),"props":["helix.width","name"]})
    )
    .is_err());
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helix.width")["value"],
        "2.5"
    );
    let result = call(
        &mut state,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"renderer","nodeId":id.get()}),
    );
    assert_eq!(result["reset"].as_array().unwrap().len(), 3);
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["value"],
        "smooth"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["isdefault"],
        true
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "name")["value"],
        "plain"
    );
    assert_eq!(
        call(
            &mut state,
            &mut camera,
            "reset_node_props",
            json!({"nodeType":"renderer","nodeId":id.get()})
        )["reset"],
        json!([])
    );
    let bytes = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("properties.qsc", &bytes).unwrap();
    let loaded_object = restored.active_object_summaries()[0].uid;
    let loaded_renderer = restored
        .scene_manager()
        .object(loaded_object)
        .unwrap()
        .renderer_by_name("plain")
        .unwrap()
        .uid();
    assert_eq!(
        property_row(
            &mut restored,
            &mut camera,
            loaded_renderer.get(),
            "helixhead.type"
        )["value"],
        "smooth"
    );
    call(
        &mut restored,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":loaded_renderer.get(),"prop":"helix.width","value":"3"}),
    );
    let bytes = restored.active_scene_bytes().unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("width=\"3\""));
}

#[test]
fn inspector_reset_preserves_style_inheritance_and_selection() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let object = Uid::new(molecule(&mut state, &mut camera, "styled", 0.));
    let scene = state.active_scene().unwrap();
    let index = state.create_scene_style(scene, "custom").unwrap();
    state.update_scene_style(scene, index, "custom", false, r#"<styles id="custom"><style id="ribbon-custom"><helix width="1.75"/><helixhead type="flat"/></style></styles>"#).unwrap();
    let id = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ribbon", "styled");
    state
        .set_renderer_styles(object, id, "ribbon-custom")
        .unwrap();
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":id.get(),"prop":"sel","value":"resid 1:3"}),
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helix.width")["value"],
        "1.75"
    );
    for (key, value) in [("helix.width", "8"), ("helixhead.type", "arrow")] {
        call(
            &mut state,
            &mut camera,
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":id.get(),"prop":key,"value":value}),
        );
    }
    call(
        &mut state,
        &mut camera,
        "reset_node_props",
        json!({"nodeType":"renderer","nodeId":id.get()}),
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helix.width")["value"],
        "1.75"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["value"],
        "flat"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "sel")["value"],
        "resid 1:3"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "style")["value"],
        "ribbon-custom"
    );
    #[cfg(feature = "scripting")]
    assert_eq!(state.run_script_js("var r=cuemol.getObject('styled.pdb').getRendererByName('styled'); print(r.helixhead.type); print(r.helix.width);").unwrap().0, "flat\n1.75\n");
}

#[test]
fn inspector_group_rename_and_structured_visibility_keep_members_together() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let object = Uid::new(molecule(&mut state, &mut camera, "grouped", 0.));
    let group = call(
        &mut state,
        &mut camera,
        "create_group",
        json!({"objId":object.get(),"name":"group"}),
    )["rendererId"]
        .as_u64()
        .unwrap();
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("cpk", "member");
    state
        .set_workspace_renderer_group_name(object, renderer, Some("group"))
        .unwrap();
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":group,"prop":"name","value":"renamed"}),
    );
    assert_eq!(
        property_row(&mut state, &mut camera, renderer.get(), "group")["value"],
        "renamed"
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":group,"prop":"visible","value":"false"}),
    );
    assert_eq!(
        property_row(&mut state, &mut camera, renderer.get(), "visible")["value"],
        "true"
    );
    call(
        &mut state,
        &mut camera,
        "set_node_prop",
        json!({"nodeType":"renderer","nodeId":group,"prop":"visible","value":"false","cascadeGroupVisibility":true}),
    );
    assert_eq!(
        property_row(&mut state, &mut camera, renderer.get(), "visible")["value"],
        "false"
    );
    let second = Uid::new(molecule(&mut state, &mut camera, "other", 3.));
    state
        .scene_manager_mut()
        .object_mut(second)
        .unwrap()
        .add_renderer("cpk", "occupied");
    for name in ["member", "occupied"] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_node_prop",
            &json!({"nodeType":"renderer","nodeId":group,"prop":"name","value":name})
        )
        .is_err());
    }
    assert_eq!(
        property_row(&mut state, &mut camera, group, "name")["value"],
        "renamed"
    );
}

#[cfg(feature = "scripting")]
#[test]
fn inspector_script_child_reset_uses_class_default_and_later_writes_are_explicit() {
    let mut state = DesktopAppState::new();
    let mut camera = Camera::new();
    let object = Uid::new(molecule(&mut state, &mut camera, "scripted", 0.));
    let id = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ribbon", "plain");
    let script = "var r=cuemol.getObject('scripted.pdb').getRendererByName('plain'); var j=cuemol.createObj('JctTable'); print(j.type); cuemol.resetProp(j,'type'); print(j.type); r.helixhead.type='flat'; cuemol.resetProp(r.helixhead,'type'); print(r.helixhead.type);";
    assert_eq!(
        state.run_script_js(script).unwrap().0,
        "arrow\nsmooth\nsmooth\n"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["isdefault"],
        true
    );
    state.run_script_js("var r=cuemol.getObject('scripted.pdb').getRendererByName('plain'); r.helixhead.type='arrow'; cuemol.resetProp(r.helix,'width'); r.helix.width=2.75;").unwrap();
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["value"],
        "arrow"
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helixhead.type")["isdefault"],
        false
    );
    assert_eq!(
        property_row(&mut state, &mut camera, id.get(), "helix.width")["isdefault"],
        false
    );
    assert!(
        String::from_utf8_lossy(&state.active_scene_bytes().unwrap()).contains("width=\"2.75\"")
    );
}
