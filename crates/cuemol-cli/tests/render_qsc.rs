//! End-to-end test for `cuemol render-qsc`: load a real `.qsc` scene, resolve
//! each renderer's `style="..."` presets, and render it to an OBJ mesh.
//!
//! Self-skips when the C++ reference tree (fixture source) is absent, matching
//! the workspace `*_compat.rs` convention.

use std::path::PathBuf;
use std::process::Command;

fn reference_dir() -> PathBuf {
    option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"))
}

#[test]
fn render_qsc_renders_a_styled_scene_to_obj() {
    let fixture = reference_dir().join("tests/test_data/number_chain_atomintr1.qsc");
    if !fixture.exists() {
        eprintln!("skipping: reference fixture absent ({})", fixture.display());
        return;
    }
    let out = std::env::temp_dir().join("cuemol_render_qsc_test.obj");
    let _ = std::fs::remove_file(&out);

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            fixture.to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run cuemol render-qsc");
    assert!(status.success(), "render-qsc exited with failure");

    let obj = std::fs::read_to_string(&out).expect("read rendered OBJ");
    // The scene's trace renderer (a main-chain ribbon mesh) plus the two atomintr
    // renderers' thin measurement-line cylinders produce substantial real
    // geometry. (The atomintr renderers draw one line each between their atom
    // pairs — NOT a sphere per atom in the whole molecule; the sharp correctness
    // oracle for that lives in cuemol-desktop-core's snapshot test.)
    let verts = obj.lines().filter(|line| line.starts_with("v ")).count();
    let faces = obj.lines().filter(|line| line.starts_with("f ")).count();
    assert!(
        verts > 1000,
        "expected a substantial mesh, got {verts} vertices"
    );
    assert!(faces > 1000, "expected real faces, got {faces}");
    // Never emit a degenerate `f 0` (1-based OBJ indices).
    assert!(!obj.contains("f 0 "), "OBJ must use 1-based face indices");

    std::fs::remove_file(&out).ok();
}
