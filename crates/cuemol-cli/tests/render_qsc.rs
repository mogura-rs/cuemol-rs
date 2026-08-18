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
    // The scene's trace renderer (a main-chain line) plus the two atomintr
    // renderers' thin measurement-line cylinders produce real
    // geometry. (The atomintr renderers draw one line each between their atom
    // pairs — NOT a sphere per atom in the whole molecule; the sharp correctness
    // oracle for that lives in cuemol-desktop-core's snapshot test.)
    let verts = obj.lines().filter(|line| line.starts_with("v ")).count();
    let faces = obj.lines().filter(|line| line.starts_with("f ")).count();
    assert!(verts > 400, "expected trace geometry, got {verts} vertices");
    assert!(verts < 1000, "trace must not become a ribbon mesh: {verts}");
    assert!(faces >= 400, "expected real faces, got {faces}");
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
         <renderer type=\"isosurf\" name=\"solid\" siglevel=\"0\" color=\"#0000ff\" binning=\"2\" max_grids=\"77\" use_pbc=\"false\" colormode=\"multigrad\" color_mapname=\"map\" egtype=\"edges\" eglinew=\"0.05\" egcolor=\"#040506\" cullface=\"true\">\n\
         <multi_grad><gradnode par=\"-1\"><col value=\"#ff0000\"/></gradnode><gradnode par=\"1\"><col value=\"#00ff00\"/></gradnode></multi_grad>\n\
         </renderer>\n\
         <renderer type=\"isosurf\" name=\"lines\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"line\"/>\n\
         <renderer type=\"isosurf\" name=\"points\" siglevel=\"0\" use_pbc=\"false\" drawmode=\"point\"/>\n\
         <renderer type=\"gpu_mapvol\" name=\"volume\" siglevel=\"0\" use_pbc=\"false\"/>\n\
         <renderer type=\"*unitcell\" name=\"cell\" width=\"1.5\" color=\"#abcdef\"/>\n\
         </object>\n\
         <object type=\"MolCoord\" name=\"direct-mol\" src=\"datachunk:00002\" srctype=\"pdb\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,40,0,0,1)\">\n\
         <renderer type=\"dsurf2\" name=\"direct-potential\" sel=\"*\" surftype=\"vdw\" detail=\"4\" vdwr_C=\"1.0\" colormode=\"potential\" elepot=\"map\" lowpar=\"-1\" midpar=\"0\" highpar=\"1\" lowcol=\"#fedcba\" midcol=\"#fedcba\" highcol=\"#fedcba\" egtype=\"silhouette\" eglinew=\"0.1\" egcolor=\"#010203\" cullface=\"true\"/>\n\
         </object>\n\
         <object type=\"MolSurf\" name=\"colored-surface\" src=\"datachunk:00001\" srctype=\"obj\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,30,0,0,1)\">\n\
         <renderer type=\"molsurf\" name=\"surface-potential\" colormode=\"potential\" elepot=\"map\" lowpar=\"-1\" midpar=\"0\" highpar=\"1\" lowcol=\"#123456\" midcol=\"#123456\" highcol=\"#123456\"/>\n\
         <renderer type=\"molsurf\" name=\"surface-gradient\" colormode=\"multigrad\" color_mapname=\"map\">\n\
         <multi_grad><gradnode par=\"-1\"><col value=\"#654321\"/></gradnode><gradnode par=\"1\"><col value=\"#654321\"/></gradnode></multi_grad>\n\
         </renderer>\n\
         </object>\n\
         </scene>\n\
         ========== End of XML ==========\n\
         ==========START datachunk:00000==========\n\
         {encoded}\
         ==========END datachunk:00000==========\n\
         ==========START datachunk:00001==========\n\
         v 0 0 0\n\
         v 1 0 0\n\
         v 0 1 0\n\
         f 1 2 3\n\
         ==========END datachunk:00001==========\n\
         ==========START datachunk:00002==========\n\
ATOM      1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\n\
END\n\
         ==========END datachunk:00002==========\n"
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
    assert!(mqo.contains("Object \"surface_potential\""));
    assert!(mqo.contains("Object \"surface_gradient\""));
    assert!(mqo.contains("Object \"direct_potential\""));
    assert!(mqo.contains("col(0.070588 0.203922 0.337255 1.000000)"));
    assert!(mqo.contains("col(0.396078 0.262745 0.129412 1.000000)"));
    assert!(mqo.contains("col(0.996078 0.862745 0.729412 1.000000)"));
    assert!(mqo.contains("col(0.003922 0.007843 0.011765 1.000000)"));
    assert!(mqo.contains("col(0.015686 0.019608 0.023529 1.000000)"));
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
               <renderer type=\"molsurf\" name=\"surface-fill\" defaultcolor=\"#336699\" alpha=\"0.5\" egtype=\"silhouette\" eglinew=\"0.25\" egcolor=\"#ff0000\" cullface=\"true\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,0,0,1,1)\"/>\n\
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
    assert_eq!(obj.lines().filter(|line| line.starts_with("f ")).count(), 2);

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
    assert!(mqo.contains("col(1.000000 0.000000 0.000000 0.501961)"));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_colors_and_filters_saved_surfaces_by_molecule() {
    let stem = format!("cuemol_render_qsc_surface_molecule_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    let qsc = "<scene>\n\
               <object type=\"MolCoord\" name=\"mol\" src=\"datachunk:00000\" srctype=\"pdb\" visible=\"false\">\n\
               <coloring type=\"PaintColoring\"><paint sel=\"elem C\" color=\"#00ff00\"/><paint sel=\"elem O\" color=\"#ff0000\"/></coloring>\n\
               </object>\n\
               <object type=\"MolSurf\" name=\"surface\" src=\"datachunk:00001\" srctype=\"obj\" orig_mol=\"mol\">\n\
               <renderer type=\"molsurf\" name=\"molecule-color\" colormode=\"molecule\" target=\"mol\" sel=\"*\" showsel=\"elem C\"/>\n\
               </object>\n\
               </scene>\n\
               ========== End of XML ==========\n\
               ==========START datachunk:00000==========\n\
ATOM      1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\n\
ATOM      2  O1  LIG A   1      10.000   0.000   0.000  1.00 20.00           O\n\
END\n\
               ==========END datachunk:00000==========\n\
               ==========START datachunk:00001==========\n\
v 0 0 0\n\
v 1 0 0\n\
v 0 1 0\n\
v 10 0 0\n\
v 11 0 0\n\
v 10 1 0\n\
f 1 2 3\n\
f 4 5 6\n\
               ==========END datachunk:00001==========\n";
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export molecule-colored QSC surface to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert_eq!(obj.lines().filter(|line| line.starts_with("f ")).count(), 1);
    assert!(!obj.contains("v 10.000000"));

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export molecule-colored QSC surface to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"molecule_color\""));
    assert!(mqo.contains("col(0.000000 1.000000 0.000000 1.000000)"));
    assert!(!mqo.contains("col(1.000000 0.000000 0.000000 1.000000)"));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_exports_molecule_edge_styles() {
    let stem = format!("cuemol_render_qsc_molecule_edge_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    let qsc = "<scene>\n\
               <object type=\"MolCoord\" name=\"mol\" src=\"datachunk:00000\" srctype=\"pdb\">\n\
               <renderer type=\"ballstick\" name=\"edged-ball\" color=\"#abcdef\" egtype=\"silhouette\" eglinew=\"0.2\" egcolor=\"#010203\"/>\n\
               </object>\n\
               </scene>\n\
               ========== End of XML ==========\n\
               ==========START datachunk:00000==========\n\
ATOM      1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\n\
END\n\
               ==========END datachunk:00000==========\n";
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export edge-styled molecule to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert!(obj.lines().filter(|line| line.starts_with("f ")).count() > 100);

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export edge-styled molecule to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"edged_ball\""));
    assert!(mqo.contains("col(0.003922 0.007843 0.011765 1.000000)"));

    std::fs::remove_file(qsc_path).ok();
    std::fs::remove_file(obj_path).ok();
    std::fs::remove_file(mqo_path).ok();
}

#[test]
fn render_qsc_uses_default_and_special_molecule_renderers() {
    let stem = format!("cuemol_render_qsc_special_renderers_{}", std::process::id());
    let qsc_path = std::env::temp_dir().join(format!("{stem}.qsc"));
    let obj_path = std::env::temp_dir().join(format!("{stem}.obj"));
    let mqo_path = std::env::temp_dir().join(format!("{stem}.mqo"));
    let qsc = "<scene>\n\
               <object type=\"MolCoord\" name=\"default-mol\" src=\"datachunk:00000\" srctype=\"pdb\"/>\n\
               <object type=\"MolCoord\" name=\"special-mol\" src=\"datachunk:00001\" srctype=\"pdb\">\n\
               <renderer type=\"selection\" name=\"selection-overlay\"/>\n\
               <renderer type=\"test\" name=\"debug-cube\" color1=\"#112233\"/>\n\
               <renderer type=\"ms2test\" name=\"ms2-surface\"/>\n\
               </object>\n\
               </scene>\n\
               ========== End of XML ==========\n\
               ==========START datachunk:00000==========\n\
ATOM      1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\n\
ATOM      2  O1  LIG A   1       1.500   0.000   0.000  1.00 20.00           O\n\
CONECT    1    2\n\
END\n\
               ==========END datachunk:00000==========\n\
               ==========START datachunk:00001==========\n\
ATOM      1  C1  LIG A   1      20.000   0.000   0.000  1.00 20.00           C\n\
END\n\
               ==========END datachunk:00001==========\n";
    std::fs::write(&qsc_path, qsc).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("export default and special renderers to OBJ");
    assert!(status.success());
    let obj = std::fs::read_to_string(&obj_path).unwrap();
    assert!(obj.lines().filter(|line| line.starts_with("f ")).count() > 12);

    let status = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args([
            "render-qsc",
            qsc_path.to_str().unwrap(),
            mqo_path.to_str().unwrap(),
        ])
        .status()
        .expect("export default and special renderers to MQO");
    assert!(status.success());
    let mqo = std::fs::read_to_string(&mqo_path).unwrap();
    assert!(mqo.contains("Object \"default\""));
    assert!(mqo.contains("Object \"debug_cube\""));
    assert!(mqo.contains("Object \"ms2_surface\""));
    assert!(!mqo.contains("Object \"selection_overlay\""));

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
         <renderer type=\"lwrend\" name=\"lw-mesh\" data_id=\"7\" alpha=\"0.5\" egtype=\"edges\" eglinew=\"0.15\" egcolor=\"#070809\" xformMat=\"(1,0,0,0,0,1,0,0,0,0,1,0,0,0,1,1)\"/>\n\
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
    assert_eq!(obj.lines().filter(|line| line.starts_with("f ")).count(), 2);

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
    assert!(mqo.contains("col(0.027451 0.031373 0.035294 0.501961)"));

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
