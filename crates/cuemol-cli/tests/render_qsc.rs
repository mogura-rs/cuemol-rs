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

#[test]
fn render_qsc_renders_saved_density_map_renderers() {
    let mut map = cuemol_xtal::DensityMap::new();
    map.set_map_byte_array(
        &[0, 32, 64, 96, 128, 160, 192, 255],
        [2, 2, 2],
        -1.0,
        1.0,
        0.0,
        0.5,
    );
    map.set_map_params([0, 0, 0], [2, 2, 2]);
    map.set_xtal_params(10.0, 10.0, 10.0, 90.0, 90.0, 90.0);
    let qdf = cuemol_xtal::write_qdf_denmap_bytes(&map);
    let encoded =
        String::from_utf8(cuemol_qsys::encode_qdf0_bytes(&qdf, 1, "13").unwrap()).unwrap();
    let qsc = format!(
        "<scene>\n\
         <object type=\"DensityMap\" name=\"map\" src=\"datachunk:00000\" srctype=\"qdfmap\">\n\
         <renderer type=\"contour\" name=\"wire\" siglevel=\"0\"/>\n\
         <renderer type=\"isosurf\" name=\"solid\" siglevel=\"0\" color=\"#0000ff\" binning=\"2\" max_grids=\"77\" use_pbc=\"false\" colormode=\"multigrad\" color_mapname=\"map\">\n\
         <multi_grad><gradnode par=\"-1\"><col value=\"#ff0000\"/></gradnode><gradnode par=\"1\"><col value=\"#00ff00\"/></gradnode></multi_grad>\n\
         </renderer>\n\
         <renderer type=\"isosurf\" name=\"lines\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"line\"/>\n\
         <renderer type=\"isosurf\" name=\"points\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"point\"/>\n\
         </object>\n\
         </scene>\n\
         ========== End of XML ==========\n\
         ==========START datachunk:00000==========\n\
         {encoded}\
         ==========END datachunk:00000==========\n"
    );
    let qsc_path = std::env::temp_dir().join("cuemol_render_qsc_map_test.qsc");
    let out = std::env::temp_dir().join("cuemol_render_qsc_map_test.obj");
    std::fs::write(&qsc_path, qsc).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run cuemol render-qsc for map");
    assert!(status.success());
    let obj = std::fs::read_to_string(&out).unwrap();
    assert!(obj.lines().any(|line| line.starts_with("v ")));
    assert!(obj.lines().any(|line| line.starts_with("f ")));
    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(out).ok();
}
