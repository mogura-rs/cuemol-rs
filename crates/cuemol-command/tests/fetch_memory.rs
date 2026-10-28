use cuemol_command::{load_fetched, prepare_fetch};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;
const PDB:&[u8]=b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nHETATM    2  O1  LIG A   1       3.000   0.000   0.000  1.00 10.00           O  \nEND\n";
#[test]
fn memory_download_applies_renderer_options_and_rolls_back_invalid_setup() {
    let mut state = DesktopAppState::new();
    state.new_scene("test", true);
    let mut camera = Camera::new();
    let plan=prepare_fetch("fetch_url",&json!({"url":"https://example.test/molecule","reader":"pdb","name":"Downloaded","rendererType":"cpk","rendererName":"carbon","selection":"elem C"})).unwrap();
    let result = load_fetched(&mut state, &mut camera, &plan, PDB).unwrap();
    let id = cuemol_core::Uid::new(result["objectId"].as_u64().unwrap());
    assert_eq!(
        state.scene_manager().object(id).unwrap().name(),
        "Downloaded"
    );
    let renderers: Vec<_> = state
        .renderer_summaries(id)
        .into_iter()
        .filter(|r| r.renderer_type != "*selection")
        .collect();
    assert_eq!(renderers.len(), 1);
    assert_eq!(renderers[0].renderer_type, "cpk");
    assert_eq!(renderers[0].name, "carbon");
    let before = cuemol_command::view_json(&camera);
    let mut invalid = plan;
    invalid.arguments["rendererType"] = json!("no-such-renderer");
    assert!(load_fetched(&mut state, &mut camera, &invalid, PDB).is_err());
    assert_eq!(
        state.object_summaries(state.active_scene().unwrap()).len(),
        1
    );
    assert_eq!(cuemol_command::view_json(&camera), before);
}
#[test]
fn memory_map_download_honors_reader_alias_and_contour_name() {
    let mut state = DesktopAppState::new();
    state.new_scene("test", true);
    let mut camera = Camera::new();
    let plan=prepare_fetch("fetch_url",&json!({"url":"https://example.test/potential","reader":"apbs","name":"Potential","rendererType":"contour","rendererName":"level"})).unwrap();
    let dx=b"object 1 class gridpositions counts 1 1 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 1 1 2\nobject 3 class array type double rank 0 items 2 data follows\n-1 1\n";
    load_fetched(&mut state, &mut camera, &plan, dx).unwrap();
    let maps = state.active_density_map_summaries();
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].name, "Potential");
    assert_eq!(maps[0].renderers.len(), 1);
    assert_eq!(maps[0].renderers[0].name, "level");
}

#[test]
fn cancelled_native_render_does_not_create_an_output_file() {
    let mut state = DesktopAppState::new();
    state.new_scene("test", true);
    let camera = Camera::new();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cancelled.png");
    let render = cuemol_command::prepare_render(
        &state,
        &camera,
        "render_image",
        &json!({"fileName":path.to_string_lossy(),"width":32,"height":32}),
    )
    .unwrap();
    assert!(render
        .run_with_cancel(&std::sync::atomic::AtomicBool::new(true))
        .is_err());
    assert!(!path.exists());
}
