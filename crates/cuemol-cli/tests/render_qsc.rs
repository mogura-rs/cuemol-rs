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
         <object type=\"DensityMap\" name=\"map\" src=\"datachunk:00000\" srctype=\"qdfmap\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,20,0,0,1)\">\n\
         <renderer type=\"contour\" name=\"wire\" siglevel=\"0\"/>\n\
         <renderer type=\"isosurf\" name=\"solid\" siglevel=\"0\" color=\"#0000ff\" binning=\"2\" max_grids=\"77\" use_pbc=\"false\" colormode=\"multigrad\" color_mapname=\"map\">\n\
         <multi_grad><gradnode par=\"-1\"><col value=\"#ff0000\"/></gradnode><gradnode par=\"1\"><col value=\"#00ff00\"/></gradnode></multi_grad>\n\
         </renderer>\n\
         <renderer type=\"isosurf\" name=\"lines\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"line\"/>\n\
         <renderer type=\"isosurf\" name=\"points\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"point\"/>\n\
         <renderer type=\"gpu_mapvol\" name=\"volume\" siglevel=\"0\" use_pbc=\"false\"/>\n\
         <renderer type=\"*unitcell\" name=\"cell\" width=\"1.5\" color=\"#abcdef\"/>\n\
         </object>\n\
         </scene>\n\
         ========== End of XML ==========\n\
         ==========START datachunk:00000==========\n\
         {encoded}\
         ==========END datachunk:00000==========\n"
    );
    let qsc_path = std::env::temp_dir().join("cuemol_render_qsc_map_test.qsc");
    let out = std::env::temp_dir().join("cuemol_render_qsc_map_test.obj");
    let mqo_path = std::env::temp_dir().join("cuemol_render_qsc_map_test.mqo");
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
    let min_x = obj
        .lines()
        .filter_map(|line| line.strip_prefix("v "))
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|value| value.parse::<f64>().ok())
        .fold(f64::INFINITY, f64::min);
    assert!(min_x > 4.0, "density transform was not applied: {min_x}");
    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC density renderers to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"volume\""));
    assert!(mqo.contains("Object \"cell\""));
    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(out).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_preserves_exporter_scene_context() {
    let fixture = reference_dir().join("tests/test_data/number_chain_atomintr1.qsc");
    if !fixture.exists() {
        eprintln!("skipping: reference fixture absent ({})", fixture.display());
        return;
    }
    let source = std::fs::read_to_string(&fixture).unwrap();
    let source = source
        .replacen("<scene>", "<scene bgcolor=\"#123456\">", 1)
        .replacen(
            "<renderer type=\"trace\" group=\"\" name=\"trace1\"",
            "<renderer type=\"trace\" group=\"\" name=\"trace1\" alpha=\"0.5\"",
            1,
        )
        .replacen(
            "<renderer type=\"atomintr\" group=\"\" name=\"measure\"",
            "<renderer type=\"atomintr\" group=\"\" name=\"measure\" material=\"metallic_chrome\"",
            1,
        );
    let stem = format!("cuemol_render_qsc_context_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let pov_path = std::env::temp_dir().join(format!("{stem}.pov"));
    let inc_path = std::env::temp_dir().join(format!("{stem}.inc"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    std::fs::write(&qsc_path, source).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            pov_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC scene to POV-Ray");
    assert!(status.success());
    let pov = std::fs::read_to_string(&pov_path).unwrap();
    let inc = std::fs::read_to_string(&inc_path).unwrap();
    assert!(pov.contains("#declare _bgcolor = <0.070588,0.203922,0.337255>;"));
    assert!(pov.contains("#declare _center = <7.278200,10.185700,6.874700>;"));
    assert!(pov.contains("#declare _distance = 200.000000;"));
    assert!(inc.contains("texture{ T_Chrome_4D }"));
    assert!(inc.contains("0.498039"));

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC scene to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"trace1\""));
    assert!(mqo.contains("Object \"measure\""));
    assert!(mqo.contains("Object \"measure2\""));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(pov_path).ok();
    std::fs::remove_file(inc_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_exports_saved_surface_renderers() {
    let stem = format!("cuemol_render_qsc_surface_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    let qsc = "<scene>\n\
               <object type=\"MolSurf\" name=\"surface\" src=\"datachunk:00000\" srctype=\"obj\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,2,3,4,1)\">\n\
               <renderer type=\"molsurf\" name=\"surface-fill\" defaultcolor=\"#336699\" alpha=\"0.5\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,0,0,1,1)\"/>\n\
               </object>\n\
               </scene>\n\
               ========== End of XML ==========\n\
               ==========START datachunk:00000==========\n\
               v 0 0 0\n\
               v 1 0 0\n\
               v 0 1 0\n\
               f 1 2 3\n\
               ==========END datachunk:00000==========\n";
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC surface to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert!(obj.contains("v 2.000000 3.000000 5.000000"));
    assert_eq!(obj.lines().filter(|line| line.starts_with("f ")).count(), 1);

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC surface to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"surface_fill\""));
    assert!(mqo.contains("col(0.200000 0.400000 0.600000 0.501961)"));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_exports_saved_lightweight_renderers() {
    let mut object = cuemol_lwview::LWObject::new();
    object.add_draw_element(cuemol_lwview::LWDrawElement::new(
        7,
        cuemol_gfx::AbstDrawElem::DRAW_TRIANGLES as i8,
        1.0,
        [32, 64, 128, 255],
        cuemol_lwview::LWDrawData::Vertices(vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ]),
    ));
    let qdf = cuemol_lwview::write_qdf_lwobj_bytes(&object);
    let encoded =
        String::from_utf8(cuemol_qsys::encode_qdf0_bytes(&qdf, 1, "13").unwrap()).unwrap();
    let qsc = format!(
        "<scene>\n\
         <object type=\"LWObject\" name=\"lightweight\" src=\"datachunk:00000\" srctype=\"qdflwobj\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,2,0,0,1)\">\n\
         <renderer type=\"lwrend\" name=\"lw-mesh\" data_id=\"7\" alpha=\"0.5\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,0,0,1,1)\"/>\n\
         </object>\n\
         </scene>\n\
         ========== End of XML ==========\n\
         ==========START datachunk:00000==========\n\
         {encoded}\
         ==========END datachunk:00000==========\n"
    );
    let stem = format!("cuemol_render_qsc_lw_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC lightweight object to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert!(obj.contains("v 2.000000 0.000000 1.000000"));
    assert_eq!(obj.lines().filter(|line| line.starts_with("f ")).count(), 1);

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export QSC lightweight object to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"lw_mesh\""));
    assert!(mqo.contains("col(0.125490 0.250980 0.501961 0.501961)"));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_exports_molecule_unit_cell_renderer() {
    let qsc = "<scene>\n\
               <object type=\"MolCoord\" name=\"crystal\" src=\"datachunk:00000\" srctype=\"pdb\">\n\
               <renderer type=\"ballstick\" name=\"atoms\"/>\n\
               <renderer type=\"symm\" name=\"mates\" unitcell=\"true\"/>\n\
               <renderer type=\"unitcell\" name=\"cell\" width=\"1.5\" color=\"#abcdef\"/>\n\
               </object>\n\
               </scene>\n\
               ========== End of XML ==========\n\
               ==========START datachunk:00000==========\n\
               CRYST1   10.000   10.000   10.000  90.00  90.00  90.00 P -1\n\
               ATOM      1  CA  ALA A   1       1.000   1.000   1.000  1.00  0.00           C\n\
               END\n\
               ==========END datachunk:00000==========\n";
    let stem = format!("cuemol_render_qsc_unitcell_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export molecule unit cell to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert!(
        obj.lines().filter(|line| line.starts_with("v ")).count() > 300,
        "unit-cell cylinders were not exported"
    );
    let max_x = obj
        .lines()
        .filter_map(|line| line.strip_prefix("v "))
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|value| value.parse::<f64>().ok())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(max_x > 8.0, "symmetry mate was not exported: {max_x}");

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export molecule unit cell to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"atoms\""));
    assert!(mqo.contains("Object \"cell\""));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}
