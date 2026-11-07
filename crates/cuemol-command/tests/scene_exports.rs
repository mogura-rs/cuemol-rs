use cuemol_command::{
    call_tool,
    scene_export::{prepare_scene_export, write_scene_export, SceneExportFormat},
};
use cuemol_core::Vector4;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn fixture() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.open_bytes("probe.pdb", b"ATOM      1  CA  ALA A   1       2.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       8.000   4.000  10.000  1.00 10.00           C\nEND\n").unwrap();
    let id = state.active_object_summaries()[0].uid;
    state
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .add_renderer("cpk", "atoms");
    (state, Camera::new())
}

fn stl_vertices(bytes: &[u8]) -> Vec<[f32; 3]> {
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert!(count > 0);
    assert_eq!(bytes.len(), 84 + count * 50);
    bytes[84..]
        .chunks_exact(50)
        .flat_map(|face| {
            face[12..48].chunks_exact(12).map(|point| {
                std::array::from_fn(|i| {
                    f32::from_le_bytes(point[4 * i..4 * i + 4].try_into().unwrap())
                })
            })
        })
        .collect()
}

#[test]
fn scene_geometry_keeps_state_and_uses_the_live_camera() {
    let (state, mut camera) = fixture();
    let before = state.clone();
    let original = prepare_scene_export(&state, &camera, "", SceneExportFormat::Stl).unwrap();
    let vertices = stl_vertices(&original.bytes);
    camera.set_center(Vector4::new3(3., -5., 7.));
    let moved = prepare_scene_export(&state, &camera, "", SceneExportFormat::Stl).unwrap();
    for (a, b) in vertices.iter().zip(stl_vertices(&moved.bytes)) {
        for (axis, shift) in [3., -5., 7.].into_iter().enumerate() {
            assert!((a[axis] - shift - b[axis]).abs() < 1e-4);
        }
    }
    let mqo = prepare_scene_export(&state, &camera, "", SceneExportFormat::Mqo).unwrap();
    let text = String::from_utf8(mqo.bytes).unwrap();
    assert!(text.starts_with("Metasequoia Document"));
    assert!(text.contains("vertex "));
    assert!(text.contains("face "));
    assert!(state.same_undo_state(&before));
    assert_eq!(state.modified_scene_uids(), before.modified_scene_uids());
}

#[test]
fn portable_pov_keeps_background_and_matches_native_geometry_without_sidecars() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "set_node_prop",
        &json!({"nodeType":"scene", "nodeId":scene.get(), "prop":"bgcolor", "value":"#123456"}),
    )
    .unwrap();
    let before = state.clone();
    let portable = prepare_scene_export(&state, &camera, "", SceneExportFormat::Pov).unwrap();
    let source = String::from_utf8(portable.bytes).unwrap();
    assert!(source.contains("CueMol POV-Ray scene"));
    assert!(source.contains("sphere {"));
    assert!(source.contains("<0.070588,0.203922,0.337255>"));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("scene with spaces.pov");
    write_scene_export(&state, &camera, "", SceneExportFormat::Pov, &path).unwrap();
    let main = std::fs::read_to_string(&path).unwrap();
    let include = std::fs::read_to_string(path.with_extension("inc")).unwrap();
    assert!(main.contains("scene with spaces.inc"));
    assert!(!source.contains("scene with spaces.inc"));
    for line in source
        .lines()
        .filter(|line| line.contains("sphere {") || line.contains("background {"))
    {
        assert!(main.contains(line) || include.contains(line), "{line}");
    }
    assert!(state.same_undo_state(&before));
}

#[test]
fn geometry_commands_write_real_formats_and_fail_without_editing_the_scene() {
    let (mut state, mut camera) = fixture();
    let before = state.clone();
    let directory = tempfile::tempdir().unwrap();
    for format in ["stl", "mqo"] {
        let path = directory.path().join(format!("output.{format}"));
        call_tool(
            &mut state,
            &mut camera,
            "export_scene",
            &json!({"path":path, "format":format}),
        )
        .unwrap();
        let expected = prepare_scene_export(
            &state,
            &camera,
            "",
            SceneExportFormat::parse(format).unwrap(),
        )
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), expected.bytes);
    }
    assert!(write_scene_export(
        &state,
        &camera,
        "",
        SceneExportFormat::Stl,
        directory.path()
    )
    .is_err());
    assert!(SceneExportFormat::parse("unknown").is_err());
    assert!(state.same_undo_state(&before));
}
