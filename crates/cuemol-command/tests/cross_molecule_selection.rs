use cuemol_command::selection;
use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;

const PDB: &str = "HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00  0.00           C\nHETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O\nHETATM    3  N1  LIG A   1       1.000   1.500   0.000  1.00  0.00           N\nEND\n";

fn fixture() -> (DesktopAppState, Uid, Uid) {
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("cross selection", true).result_scene;
    let molecule = cuemol_molstr::read_pdb_str(PDB).unwrap();
    let scene = state.scene_manager_mut().scene_mut(scene).unwrap();
    let probe = scene.add_molecule("Probe", molecule.clone());
    let target = scene.add_molecule("Target", molecule);
    (state, probe, target)
}

fn selected(state: &DesktopAppState, object: Uid, text: &str) -> Vec<usize> {
    let selector = state.parse_object_selector(object, text).unwrap();
    state
        .scene_manager()
        .object(object)
        .unwrap()
        .mol()
        .select_indices(&selector)
}

#[test]
fn cross_molecule_distance_matches_published_reference() {
    let (mut state, probe, _) = fixture();
    for (text, expected) in [
        ("* around 0.1", vec![]),
        ("* around 0.1 [Target]", vec![0, 1, 2]),
        ("* around 0.1 [Probe]", vec![0, 1, 2]),
        ("name C1 around 2 [Target]", vec![0]),
        ("name C1 around 2.1 [Target]", vec![0, 1]),
        ("name C1 around 2.1 [Missing]", vec![]),
        ("name C1 around 2.1 [\"\"]", vec![1]),
        ("byres (name C1 around 2.1 [Target])", vec![0, 1, 2]),
        ("not (name C1 around 2.1 [Target])", vec![2]),
        (
            "(name O1 around 2.1 [Probe]) around 1.1 [Target]",
            vec![0, 1, 2],
        ),
    ] {
        assert_eq!(selected(&state, probe, text), expected, "{text}");
        assert_eq!(
            selection::evaluate(&state, Some(probe), text).count,
            Some(expected.len()),
            "{text}"
        );
        let count = cuemol_command::call_tool(
            &mut state,
            &mut cuemol_qsys::Camera::new(),
            "count_selection",
            &serde_json::json!({"objId":probe.get(), "selection":text}),
        )
        .unwrap();
        assert_eq!(count["count"], expected.len(), "{text}");
    }
}

#[test]
fn missing_renderer_selection_does_not_disable_other_molecules() {
    for kind in ["cpk", "dsurface"] {
        let (mut state, probe, target) = fixture();
        let object = state.scene_manager_mut().object_mut(probe).unwrap();
        let renderer = object.add_renderer(kind, "missing");
        object
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr("sel", "removed_name");
        state
            .scene_manager_mut()
            .object_mut(target)
            .unwrap()
            .add_renderer("cpk", "valid");
        let picks = state.active_pick_atoms("").unwrap();
        assert_eq!(picks.len(), 3);
        assert!(picks
            .iter()
            .all(|atom| atom.object_uid == Some(target.get())));
        let snapshot = state
            .render_snapshot(state.active_scene().unwrap(), "")
            .unwrap();
        assert_eq!(snapshot.atoms.len(), 3);
        assert!(snapshot
            .atoms
            .iter()
            .all(|atom| atom.object_uid == Some(target.get())));
        assert!(snapshot.meshes.iter().all(|mesh| mesh.vertices.is_empty()));
    }
}

#[test]
fn bound_selections_restrict_contacts_within_and_between_molecules() {
    let (mut state, probe, target) = fixture();
    for id in [probe, target] {
        for (index, atom) in state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .mol_mut()
            .atoms_mut()
            .iter_mut()
            .enumerate()
        {
            atom.res_id = index as i32 + 1;
        }
    }
    let expression = "name C1 around 2.1 [Target]";
    let pairs: serde_json::Value = serde_json::from_str(
        &state
            .calc_atom_contacts_in_object(probe, expression, 0., 4., false, 10)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(pairs, serde_json::json!([[1, 2], [1, 3], [2, 3]]));
    assert_eq!(
        state
            .define_atom_contacts(
                probe,
                expression,
                Some(target),
                Some("*"),
                0.,
                2.1,
                false,
                10,
                "contacts"
            )
            .unwrap()
            .count,
        5
    );
}

#[test]
fn molecular_surface_objects_regenerate_with_live_scene_selections() {
    let (mut state, probe, target) = fixture();
    let expression = "name C1 around 2.1 [Target]";
    let selected_surface = state
        .create_molecular_surface_object_with_backend(probe, expression, "cross", 1., 1.4, "ball")
        .unwrap();
    state
        .create_molecular_surface_object_with_backend(
            probe,
            "name C1,O1",
            "explicit",
            1.,
            1.4,
            "ball",
        )
        .unwrap();
    let meshes = state.active_scene_surface_meshes();
    assert_eq!(meshes.len(), 2);
    assert!(!meshes[0].vertices.is_empty());
    assert_eq!(meshes[0].vertices.len(), meshes[1].vertices.len());
    for (first, second) in meshes[0].vertices.iter().zip(&meshes[1].vertices) {
        assert_eq!(first.position, second.position);
        assert_eq!(first.normal, second.normal);
        assert_eq!(first.color, second.color);
    }
    assert_eq!(meshes[0].indices, meshes[1].indices);
    let mut transform = Matrix4::IDENTITY;
    transform.set_trans_part(Vector4::new3(2., 0., 0.));
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_xform_matrix(transform);
    state.regenerate_surface_object(selected_surface).unwrap();
    state
        .create_molecular_surface_object_with_backend(probe, "*", "all", 1., 1.4, "ball")
        .unwrap();
    let meshes = state.active_scene_surface_meshes();
    assert_eq!(meshes.len(), 3);
    assert_ne!(meshes[0].vertices, meshes[1].vertices);
    assert_eq!(meshes[0].vertices, meshes[2].vertices);
    assert_eq!(meshes[0].indices, meshes[2].indices);
}

#[test]
fn decimal_distance_cutoffs_keep_strict_double_precision_boundaries() {
    let (mut state, probe, target) = fixture();
    for id in [probe, target] {
        let atoms = state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .mol_mut()
            .atoms_mut();
        atoms[0].position = Vector4::ZERO;
        atoms[1].position = Vector4::new3(1.1, 0., 0.);
        atoms[2].position = Vector4::new3(5., 0., 0.);
    }
    assert!(selected(&state, probe, "name C1 around 1.1").is_empty());
    assert_eq!(selected(&state, probe, "name C1 around 1.1 [Target]"), [0]);
    for expression in [
        "name C1 around 1.1000000000001",
        "name C1 around 1.1000000000001 [Target]",
    ] {
        let expected = if expression.ends_with(']') {
            vec![0, 1]
        } else {
            vec![1]
        };
        assert_eq!(selected(&state, probe, expression), expected);
        let serialized = cuemol_molstr::parse_selector(expression)
            .unwrap()
            .to_string();
        assert_eq!(selected(&state, probe, &serialized), expected);
    }
}

#[test]
fn cross_molecule_distance_tracks_world_coordinates_and_own_scene() {
    let (mut state, probe, target) = fixture();
    let text = "name C1 around 2.1 [Target]";
    assert_eq!(selected(&state, probe, text), [0, 1]);
    let mut transform = Matrix4::IDENTITY;
    transform.set_trans_part(Vector4::new3(10., 0., 0.));
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_xform_matrix(transform);
    assert!(selected(&state, probe, text).is_empty());
    assert_eq!(selected(&state, probe, "* around 0.1 [Probe]"), [0, 1, 2]);
    state
        .scene_manager_mut()
        .object_mut(probe)
        .unwrap()
        .set_xform_matrix(transform);
    assert_eq!(selected(&state, probe, text), [0, 1]);
    state.new_scene("other", true);
    let scene = state.active_scene().unwrap();
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("Target", cuemol_molstr::read_pdb_str(PDB).unwrap());
    assert_eq!(selected(&state, probe, text), [0, 1]);
    assert_eq!(state.delete_atoms_in_object(probe, text).unwrap(), 2);
    assert_eq!(
        state.scene_manager().object(probe).unwrap().mol().atoms()[0]
            .name
            .trim(),
        "N1"
    );
}

#[test]
fn cross_molecule_selection_drives_geometry_picking_and_paint() {
    let (mut state, probe, target) = fixture();
    let expression = "name C1 around 2.1 [Target]";
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_visible(false);
    let object = state.scene_manager_mut().object_mut(probe).unwrap();
    object.set_coloring(Some("PaintColoring".into()), Default::default());
    object.set_paint_coloring(vec![
        (expression.into(), "#ff0000".into()),
        ("*".into(), "#0000ff".into()),
    ]);
    let renderer = object.add_renderer("cpk", "atoms");
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("defaultcolor", "$molcol");
    let all = state.active_render_snapshot("").unwrap();
    assert_eq!(all.atoms.len(), 3);
    let colors: Vec<_> = all
        .atoms
        .iter()
        .map(|atom| atom.color.as_ref().unwrap().rgba8().unwrap())
        .collect();
    assert_eq!(
        colors
            .iter()
            .map(|color| (color.r, color.b))
            .collect::<Vec<_>>(),
        [(255, 0), (255, 0), (0, 255)]
    );
    state
        .set_renderer_selection_text(probe, renderer, expression)
        .unwrap();
    assert_eq!(state.active_render_snapshot("").unwrap().atoms.len(), 2);
    assert_eq!(state.active_pick_atoms("").unwrap().len(), 2);
    state
        .set_molecule_selection_text(probe, expression)
        .unwrap();
    assert!(state
        .active_pick_atoms("")
        .unwrap()
        .iter()
        .all(|atom| atom.selected));
    let mut transform = Matrix4::IDENTITY;
    transform.set_trans_part(Vector4::new3(10., 0., 0.));
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_xform_matrix(transform);
    assert!(state.active_render_snapshot("").unwrap().atoms.is_empty());
    assert!(state.active_pick_atoms("").unwrap().is_empty());
    state
        .scene_manager_mut()
        .object_mut(probe)
        .unwrap()
        .set_xform_matrix(transform);
    let rendered = state.active_render_snapshot("").unwrap();
    assert_eq!(rendered.atoms.len(), 2);
    assert_eq!(rendered.atoms[0].center.x, 8.);
}

#[test]
fn bound_cross_selection_preserves_duplicate_ids_and_transformed_exports() {
    let (mut state, probe, target) = fixture();
    let atom = state.scene_manager().object(probe).unwrap().mol().atoms()[0].clone();
    {
        let object = state.scene_manager_mut().object_mut(probe).unwrap();
        let mut duplicate = atom.clone();
        duplicate.position = Vector4::new3(50., 0., 0.);
        duplicate.b_factor = 50.;
        object.mol_mut().push_atom(duplicate);
    }
    let expression = "name C1 around 2.1 [Target]";
    assert_eq!(selected(&state, probe, expression), [0, 1]);
    let mut transform = Matrix4::IDENTITY;
    transform.set_trans_part(Vector4::new3(10., 0., 0.));
    for id in [probe, target] {
        state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .set_xform_matrix(transform);
    }
    let exported = state
        .molecule_export_bytes(probe, expression, "pdb")
        .unwrap();
    let molecule = cuemol_molstr::read_pdb_str(std::str::from_utf8(&exported).unwrap()).unwrap();
    assert_eq!(molecule.atom_count(), 2);
    assert_eq!(molecule.atoms()[0].position.x, 8.);
    let selected_mesh = state
        .build_surface_mesh(probe, expression, 1., 1.4)
        .unwrap();
    let explicit_mesh = state
        .build_surface_mesh(probe, "name C1,O1 and bfac < 1", 1., 1.4)
        .unwrap();
    assert!(!selected_mesh.vertices.is_empty());
    assert_eq!(selected_mesh.vertices, explicit_mesh.vertices);
    assert_eq!(selected_mesh.indices, explicit_mesh.indices);
    assert_eq!(state.delete_atoms_in_object(probe, expression).unwrap(), 2);
    assert_eq!(
        state
            .scene_manager()
            .object(probe)
            .unwrap()
            .mol()
            .atom_count(),
        2
    );
    assert!(state
        .scene_manager()
        .object(probe)
        .unwrap()
        .mol()
        .atoms()
        .iter()
        .any(|atom| atom.position.x == 50.));
}

#[cfg(feature = "scripting")]
#[test]
fn scripts_rebind_cross_selections_after_live_transform_changes() {
    let (mut state, probe, target) = fixture();
    let scene = state.active_scene().unwrap().get();
    let source = format!(
        r#"
        var probe = cuemol.getObject({probe});
        var target = cuemol.getObject({target});
        var selection = cuemol.createObj('SelCommand');
        selection.compile('name C1 around 2.1 [Target]', {scene});
        var indices = probe.getSelArray(selection);
        print('indices=' + indices.getAt(0) + ',' + indices.getAt(1));
        print('count=' + probe.getAtomSize(selection));
        print('atoms=' + selection.isSelected(probe.getAtomByID(1)) + ',' + selection.isSelected(probe.getAtomByID(3)));
        probe.sel = selection;
        print('center=' + probe.getCenterPos(true).x);
        var shift = cuemol.createObj('Vector'); shift.x = 10;
        var transform = cuemol.createObj('Matrix'); transform.setTranslate(shift);
        target.xformMat = transform;
        print('target-moved=' + probe.getAtomSize(selection));
        probe.xformMat = transform;
        print('both-moved=' + probe.getAtomSize(selection));
        probe.deleteAtoms(selection);
        print('remaining=' + probe.getAtomSize());
    "#,
        probe = probe.get(),
        target = target.get()
    );
    let (output, _) = state.run_script_js(&source).unwrap();
    for expected in [
        "indices=0,1",
        "count=2",
        "atoms=true,false",
        "center=-1",
        "target-moved=0",
        "both-moved=2",
        "remaining=1",
    ] {
        assert!(output.contains(expected), "missing {expected}: {output}");
    }
}
