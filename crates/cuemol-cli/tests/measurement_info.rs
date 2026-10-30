use cuemol_core::{Matrix4, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::{AtomIntrKind, AtomIntrLine};
use std::process::Command;

#[test]
fn qsc_info_resolves_named_measurements_against_current_object_transforms() {
    let mut state = DesktopAppState::new();
    let scene_uid = state.new_scene("measurements", true).result_scene;
    let index = state.create_scene_style(scene_uid, "measurements").unwrap();
    state.update_scene_style(scene_uid, index, "measurements", false, "<styles id=\"measurements\"><sel id=\"focus\" value=\"name C1 around 0.2 [Target]\"/></styles>").unwrap();
    let pdb = "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  C2  LIG A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let scene = state.scene_manager_mut().scene_mut(scene_uid).unwrap();
    let probe = scene.add_molecule("Probe", cuemol_molstr::read_pdb_str(pdb).unwrap());
    let target = scene.add_molecule("Target", cuemol_molstr::read_pdb_str(pdb).unwrap());
    let object = scene.object_mut(probe).unwrap();
    let renderer = object.add_renderer("atomintr", "measure");
    object
        .renderer_mut(renderer)
        .unwrap()
        .append_atom_intr_line(AtomIntrLine::from_selections(
            AtomIntrKind::Distance,
            vec!["focus".into(), "name C1".into()],
        ))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("measurements.qsc");
    for (offset, expected) in [(1., Some(1.)), (20., None)] {
        state
            .scene_manager_mut()
            .object_mut(target)
            .unwrap()
            .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(offset, 0., 0.)));
        state.save_active_scene_qsc(&path).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .env("CUEMOL_CONFIG_DIR", dir.path().join("config"))
            .args(["qsc-info", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let values = stdout
            .lines()
            .find_map(|line| line.strip_prefix("measurements="))
            .expect("measurement diagnostics");
        let values: serde_json::Value = serde_json::from_str(values).unwrap();
        assert_eq!(values[0]["distance"].as_f64(), expected);
    }
}
