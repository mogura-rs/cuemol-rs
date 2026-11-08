use cuemol_command::{
    call_tool,
    console::{Action, Session},
    prepare_save_object,
};
use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn fixture() -> (DesktopAppState, Uid) {
    let mut state = DesktopAppState::new();
    state.open_bytes("original.pdb", b"ATOM      1  CA  ALA A   1       2.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       8.000   4.000  10.000  1.00 10.00           C\nEND\n").unwrap();
    let id = state.active_object_summaries()[0].uid;
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    object.set_name("probe");
    object.set_source("original.pdb");
    object.set_modified(true);
    object.set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(100., 0., 0.)));
    (state, id)
}

#[test]
fn partial_exports_keep_source_and_raw_coordinates_across_formats() {
    let (mut state, id) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let mut camera = Camera::new();
    for format in ["pdb", "sdf", "pqr", "xyzr", "qdfmol"] {
        let path = directory.path().join(format!("selected-{format}.data"));
        let before = state.clone();
        let result = call_tool(&mut state, &mut camera, "save_object", &json!({"objId":id.get(),"fileName":path,"format":format,"selection":"resid 2","link":true})).unwrap();
        assert_eq!(result["format"], format);
        assert!(before.same_undo_state(&state));
        let bytes = std::fs::read(path).unwrap();
        let molecule = match format {
            "pdb" => cuemol_molstr::read_pdb_str(std::str::from_utf8(&bytes).unwrap()).unwrap(),
            "sdf" => cuemol_molstr::read_sdf_str(std::str::from_utf8(&bytes).unwrap()).unwrap(),
            "qdfmol" => cuemol_molstr::read_qdf_mol_bytes(&bytes).unwrap(),
            "pqr" => {
                let text = std::str::from_utf8(&bytes).unwrap();
                let rows = text
                    .lines()
                    .filter(|line| line.starts_with("ATOM "))
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), 1);
                let fields = rows[0].split_whitespace().collect::<Vec<_>>();
                assert_eq!(fields[3], "GLY");
                assert_eq!(fields[5], "2");
                let xyz = fields[6..9]
                    .iter()
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(xyz, vec![8., 4., 10.]);
                continue;
            }
            "xyzr" => {
                let coordinates = std::str::from_utf8(&bytes)
                    .unwrap()
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(&coordinates[..3], &[8., 4., 10.]);
                assert_eq!(coordinates.len(), 4);
                continue;
            }
            _ => unreachable!(),
        };
        assert_eq!(molecule.atom_count(), 1);
        assert_eq!(molecule.atoms()[0].position, Vector4::new3(8., 4., 10.));
    }
    assert_eq!(
        state.scene_manager().object(id).unwrap().source(),
        "original.pdb"
    );
}

#[test]
fn whole_exports_link_only_after_success_and_explicit_false_preserves_state() {
    let (mut state, id) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let mut camera = Camera::new();
    let path = directory.path().join("whole.PDB");
    let before = state.clone();
    call_tool(
        &mut state,
        &mut camera,
        "save_object",
        &json!({"objId":id.get(),"fileName":path,"link":false}),
    )
    .unwrap();
    assert!(before.same_undo_state(&state));
    for args in [
        json!({"objId":id.get(),"fileName":directory.path().join("missing/failed.pdb")}),
        json!({"objId":id.get(),"fileName":directory.path().join("bad.cif")}),
        json!({"objId":id.get(),"fileName":path,"format":"unknown"}),
        json!({"objId":id.get(),"fileName":path,"selection":"resid"}),
    ] {
        assert!(call_tool(&mut state, &mut camera, "save_object", &args).is_err());
        assert!(before.same_undo_state(&state));
    }
    for selection in ["*", "", "   "] {
        call_tool(
            &mut state,
            &mut camera,
            "save_object",
            &json!({"objId":id.get(),"fileName":path,"selection":selection}),
        )
        .unwrap();
        let object = state.scene_manager().object(id).unwrap();
        assert_eq!(object.source(), path.to_string_lossy());
        assert_eq!(object.source_type(), "pdb");
        assert!(!object.modified());
    }
    let parsed = cuemol_molstr::read_pdb_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(parsed.atom_count(), 2);
    assert_eq!(parsed.atoms()[0].position.x, 2.);
}

#[test]
fn map_export_options_preserve_or_update_source_and_reject_atom_selections() {
    let (mut state, _) = fixture();
    let mut map = cuemol_xtal::DensityMap::new();
    map.set_map_byte_array(&[0, 255], [2, 1, 1], 0., 1., 0.5, 0.5);
    map.set_map_params([0; 3], [2, 1, 1]);
    map.set_xtal_params(20., 10., 10., 90., 90., 90.);
    let id = state.add_density_map(state.active_scene().unwrap(), "density", map);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("density.data");
    let before = state.clone();
    let prepared = prepare_save_object(
        &state,
        &json!({"objId":id.get(),"fileName":path,"format":"qdfmap","link":false}),
    )
    .unwrap();
    cuemol_xtal::read_qdf_denmap_bytes(&prepared.bytes).unwrap();
    prepared.write(&mut state).unwrap();
    assert!(before.same_undo_state(&state));
    assert!(prepare_save_object(
        &state,
        &json!({"objId":id.get(),"fileName":path,"format":"qdfmap","selection":"resid 2"})
    )
    .is_err());
    prepare_save_object(
        &state,
        &json!({"objId":id.get(),"fileName":path,"format":"qdfmap"}),
    )
    .unwrap()
    .write(&mut state)
    .unwrap();
    assert_eq!(
        state.density_map_summary(id).unwrap().source,
        path.to_string_lossy()
    );
}

#[test]
fn native_write_binds_export_options_and_preparation_keeps_live_state() {
    let (state, id) = fixture();
    let actions = Session::default()
        .plan(&state, "write probe, selected.data, resid 2, pdb, false")
        .unwrap();
    let args = actions
        .iter()
        .find_map(|action| match action {
            Action::Tool { name, args } if name == "save_object" => Some(args),
            _ => None,
        })
        .unwrap();
    assert_eq!(args["objId"], id.get());
    assert_eq!(args["selection"], "resid 2");
    assert_eq!(args["format"], "pdb");
    assert_eq!(args["link"], false);
    let prepared = prepare_save_object(&state, args).unwrap();
    assert_eq!(
        state.scene_manager().object(id).unwrap().source(),
        "original.pdb"
    );
    assert_eq!(
        cuemol_molstr::read_pdb_str(std::str::from_utf8(&prepared.bytes).unwrap())
            .unwrap()
            .atom_count(),
        1
    );
}

#[test]
fn surface_and_lightweight_exports_roundtrip_and_link_without_losing_geometry() {
    use cuemol_molstr::{LoadedLWObject, LoadedSurface};
    let (mut state, _) = fixture();
    let scene = state.active_scene().unwrap();
    let data = "v 0 0 0\nv 2 0 0\nv 0 2 0\nf 1 2 3\n";
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_loaded_surface(LoadedSurface {
            uid: Uid::new(0),
            name: "surface".into(),
            object_type: "MolSurfObj".into(),
            srctype: "obj".into(),
            data: data.into(),
            raw_data: data.as_bytes().to_vec().into(),
            qsc_node: None,
        });
    let surface = state.surface_summaries(scene)[0].uid;
    let lw = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_loaded_lw_object(LoadedLWObject {
            uid: Uid::new(0),
            name: "lightweight".into(),
            srctype: "qdflwobj".into(),
            raw_data: cuemol_lwview::write_qdf_lwobj_bytes(&cuemol_lwview::LWObject::new()).into(),
            visible: true,
            xform: Matrix4::IDENTITY,
            renderers: vec![],
            qsc_node: None,
        });
    let directory = tempfile::tempdir().unwrap();
    for (id, format) in [(surface, "qdfsurf"), (lw, "qdflwobj")] {
        let path = directory.path().join(format!("{format}.data"));
        let before = state.clone();
        let prepared = prepare_save_object(
            &state,
            &json!({"objId":id.get(),"fileName":path,"format":format,"link":false}),
        )
        .unwrap();
        let bytes = prepared.bytes.clone();
        prepared.write(&mut state).unwrap();
        assert!(before.same_undo_state(&state));
        if format == "qdfsurf" {
            let mut roundtrip = DesktopAppState::new();
            roundtrip.open_bytes("surface.qdf", &bytes).unwrap();
            assert_eq!(roundtrip.active_surface_summaries().len(), 1);
            assert!(!roundtrip.active_scene_surface_meshes().is_empty());
        } else {
            cuemol_lwview::read_qdf_lwobj_bytes(&bytes).unwrap();
        }
        prepare_save_object(
            &state,
            &json!({"objId":id.get(),"fileName":path,"format":format}),
        )
        .unwrap()
        .write(&mut state)
        .unwrap();
        assert_eq!(
            prepare_save_object(
                &state,
                &json!({"objId":id.get(),"fileName":path,"format":format,"link":false})
            )
            .unwrap()
            .bytes,
            bytes
        );
    }
}
