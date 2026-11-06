use cuemol_command::{call_tool, load_fetched, prepare_fetch, view_json};
use cuemol_core::{Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::{Camera, ScalarObject};
use cuemol_xtal::Ccp4MapReader;
use serde_json::json;

fn map_bytes(em: bool) -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    let mut word = |index: usize, value: u32| {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    };
    for axis in 0..3 {
        word(axis, 4);
        word(axis + 7, 4);
        word(axis + 10, 8_f32.to_bits());
        word(axis + 13, 90_f32.to_bits());
        word(axis + 16, axis as u32 + 1);
    }
    word(3, 2);
    word(22, if em { 0 } else { 1 });
    word(19, 0_f32.to_bits());
    word(20, 1000_f32.to_bits());
    word(21, 50_f32.to_bits());
    word(54, 100_f32.to_bits());
    bytes[208..212].copy_from_slice(b"MAP ");
    for index in 0..64 {
        let value = if index % 2 == 0 { index as f32 } else { 1000. };
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn initial_state() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("maps", true);
    let mut camera = Camera::new();
    camera.set_center(Vector4::new3(30., 40., 50.));
    (state, camera)
}

#[test]
fn symmetry_unit_cell_supports_maps_and_reuses_the_existing_renderer() {
    let (mut state, mut camera) = initial_state();
    let before = camera.clone();
    let plan = prepare_fetch("fetch_url", &json!({"url":"https://example.test/map", "reader":"ccp4map", "mapCenterPolicy":"setMapCenter"})).unwrap();
    load_fetched(&mut state, &mut camera, &plan, &map_bytes(false)).unwrap();
    let map = state.active_density_map_summaries()[0].uid;
    let first = call_tool(
        &mut state,
        &mut camera,
        "show_unit_cell",
        &json!({"objId":map.get()}),
    )
    .unwrap();
    let saved = state.clone();
    let second = call_tool(
        &mut state,
        &mut camera,
        "show_unit_cell",
        &json!({"objId":map.get()}),
    )
    .unwrap();
    assert_eq!(first, second);
    assert!(saved.same_undo_state(&state));
    assert_eq!(camera, before);
    assert_eq!(
        state
            .density_map_summary(map)
            .unwrap()
            .renderers
            .iter()
            .filter(|renderer| renderer.renderer_type == "*unitcell")
            .count(),
        1
    );
}

#[test]
fn symmetry_rejects_zero_cell_angles_without_replacing_existing_crystal_data() {
    let (mut state, mut camera) = initial_state();
    let scene = state.active_scene().unwrap();
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("cell", Default::default());
    let args = json!({"objId":object.get(),"a":20.,"b":30.,"c":40.,"alpha":90.,"beta":90.,"gamma":90.,"spaceGroup":19});
    call_tool(&mut state, &mut camera, "set_symmetry", &args).unwrap();
    for key in ["alpha", "beta", "gamma"] {
        let before = state.clone();
        let mut invalid = args.clone();
        invalid[key] = json!(0.);
        assert!(
            call_tool(&mut state, &mut camera, "set_symmetry", &invalid).is_err(),
            "zero {key} was accepted"
        );
        assert!(before.same_undo_state(&state));
    }
}

#[test]
fn local_map_center_policy_preserves_view_and_centers_renderer() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("density.map");
    std::fs::write(&path, map_bytes(false)).unwrap();
    let (mut state, mut camera) = initial_state();
    let before = view_json(&camera);
    call_tool(
        &mut state,
        &mut camera,
        "load_ccp4",
        &json!({
            "path": path, "mapCenterPolicy": "setMapCenter"
        }),
    )
    .unwrap();
    assert_eq!(view_json(&camera), before);
    let summary = &state.active_density_map_summaries()[0];
    assert_eq!(
        summary.renderers[0].specific["center"],
        Vector4::new3(30., 40., 50.).to_string()
    );
}

#[test]
fn downloaded_map_center_policy_preserves_view() {
    let (mut state, mut camera) = initial_state();
    let before = view_json(&camera);
    let plan = prepare_fetch(
        "fetch_url",
        &json!({
            "url": "https://example.test/density", "reader": "ccp4map",
            "mapCenterPolicy": "setMapCenter"
        }),
    )
    .unwrap();
    load_fetched(&mut state, &mut camera, &plan, &map_bytes(false)).unwrap();
    assert_eq!(view_json(&camera), before);
}

#[test]
fn automatic_map_center_policy_uses_map_kind_independently_of_molecule_centering() {
    for (em, policy, center_view, moves_view) in [
        (false, "auto", true, false),
        (true, "auto", true, true),
        (true, "auto", false, true),
        (true, "setMapCenter", true, false),
        (false, "moveViewCenter", true, true),
        (false, "moveViewCenter", false, true),
    ] {
        let (mut state, mut camera) = initial_state();
        let before = view_json(&camera);
        let plan = prepare_fetch(
            "fetch_url",
            &json!({
                "url": "https://example.test/density", "reader": "ccp4map",
                "mapCenterPolicy": policy, "centerView": center_view
            }),
        )
        .unwrap();
        load_fetched(&mut state, &mut camera, &plan, &map_bytes(em)).unwrap();
        assert_eq!(
            view_json(&camera) != before,
            moves_view,
            "em={em}, policy={policy}, centerView={center_view}"
        );
    }
}

#[test]
fn invalid_local_map_policy_and_stride_preserve_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("density.map");
    std::fs::write(&path, map_bytes(false)).unwrap();
    for (tool, args) in [
        ("load_ccp4", json!({"path": path, "subsample": 0})),
        (
            "load_file",
            json!({"path": path, "mapCenterPolicy": "unknown"}),
        ),
    ] {
        let (mut state, mut camera) = initial_state();
        let previous = state.clone();
        let view = view_json(&camera);
        assert!(call_tool(&mut state, &mut camera, tool, &args).is_err());
        assert!(state.same_undo_state(&previous));
        assert_eq!(view_json(&camera), view);
    }
}

#[test]
fn command_subsampling_matches_reader_before_quantization() {
    let bytes = map_bytes(false);
    let mut reader = Ccp4MapReader::new();
    reader.set_subsample(2);
    let expected = reader.read_density_map_bytes(&bytes).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("density.map");
    std::fs::write(&path, &bytes).unwrap();
    let (mut state, mut camera) = initial_state();
    let result = call_tool(
        &mut state,
        &mut camera,
        "load_ccp4",
        &json!({
            "path": path, "subsample": 2, "centerView": false
        }),
    )
    .unwrap();
    let id = Uid::new(result["objectId"].as_u64().unwrap());
    let summary = state.density_map_summary(id).unwrap();
    assert_eq!(summary.counts, [2, 2, 2]);
    assert_eq!(summary.min_density, expected.den_min());
    assert_eq!(summary.max_density, expected.den_max());
    assert_eq!(state.density_quantization_step(id), expected.quant_step());
    let saved = state.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored.open_bytes("sampled.qsc", &saved).unwrap();
    let reloaded = &restored.active_density_map_summaries()[0];
    assert_eq!(reloaded.counts, summary.counts);
    assert_eq!(reloaded.min_density, summary.min_density);
    assert_eq!(reloaded.max_density, summary.max_density);
    assert_eq!(reloaded.center, summary.center);
}

#[test]
fn creating_map_renderer_applies_policy_only_to_the_new_renderer() {
    let (mut state, mut camera) = initial_state();
    state.open_bytes("crystal.map", &map_bytes(false)).unwrap();
    let map = state.active_density_map_summaries()[0].clone();
    let old = map.renderers[0].clone();
    let result = call_tool(
        &mut state,
        &mut camera,
        "create_renderer",
        &json!({"objId":map.uid.get(),"rendererType":"contour","mapCenterPolicy":"setMapCenter"}),
    )
    .unwrap();
    let summary = state.density_map_summary(map.uid).unwrap();
    assert_eq!(
        summary
            .renderers
            .iter()
            .find(|r| r.uid == old.uid)
            .unwrap()
            .specific["center"],
        old.specific["center"]
    );
    assert_eq!(
        summary
            .renderers
            .iter()
            .find(|r| Some(r.uid.get()) == result["rendererId"].as_u64())
            .unwrap()
            .specific["center"],
        camera.center().to_string()
    );
    call_tool(&mut state,&mut camera,"create_renderer",&json!({"objId":map.uid.get(),"rendererType":"isosurf","mapCenterPolicy":"moveViewCenter","centerView":false})).unwrap();
    assert_ne!(camera.center(), Vector4::new3(30., 40., 50.));
}

#[test]
fn emdb_download_uses_absolute_or_default_contour_and_fits_the_map() {
    for level in [Some(10.), None] {
        let (mut state, mut camera) = initial_state();
        let before = camera.clone();
        let mut plan = prepare_fetch("fetch_emdb", &json!({"id":"11638","level":level})).unwrap();
        if plan.is_metadata() {
            plan = cuemol_command::advance_fetch(&plan, 503, &[])
                .unwrap()
                .unwrap();
        }
        plan.filename = "test.map".into();
        let result = load_fetched(&mut state, &mut camera, &plan, &map_bytes(false)).unwrap();
        let id = Uid::new(result["objectId"].as_u64().unwrap());
        assert_eq!(result["emdbId"], "EMD-11638");
        assert_eq!(state.density_map_kind(id).unwrap().1, "em");
        let map = state.density_map_summary(id).unwrap();
        assert_eq!(map.renderers.len(), 1);
        assert_eq!(map.renderers[0].renderer_type, "isosurf");
        assert_ne!(camera, before);
        if let Some(level) = level {
            let prop = call_tool(
                &mut state,
                &mut camera,
                "get_renderer_property",
                &json!({"rendId":map.renderers[0].uid.get(),"prop":"level"}),
            )
            .unwrap();
            assert!(
                (prop["value"].as_str().unwrap().parse::<f64>().unwrap() - level).abs()
                    <= state.density_quantization_step(id),
                "{prop}"
            );
        }
    }
}
