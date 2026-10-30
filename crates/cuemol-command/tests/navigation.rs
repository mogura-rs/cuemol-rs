use cuemol_command::{call_tool, navigation::*};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::Value;

fn fixture() -> (DesktopAppState, Uid) {
    let mut state = DesktopAppState::new();
    let mut pdb = String::new();
    for (i, (res, [x, y, z])) in [
        (1, [0., 0., 0.]),
        (1, [1., 0., 0.]),
        (2, [1., 1., 0.]),
        (3, [1., 1., 1.]),
        (4, [2., 1., 1.]),
        (4, [3., 1., 1.]),
    ]
    .into_iter()
    .enumerate()
    {
        pdb.push_str(&format!(
            "HETATM{:5}  C{:<2} LIG A{:4}    {:8.3}{:8.3}{:8.3}  1.00 10.00           C  \n",
            i + 1,
            i + 1,
            res,
            x,
            y,
            z
        ));
    }
    state.open_bytes("picks.pdb", pdb.as_bytes()).unwrap();
    let id = state.active_object_summaries()[0].uid;
    (state, id)
}
fn execute(state: &mut DesktopAppState, command: Command) -> Value {
    call_tool(state, &mut Camera::new(), command.name, &command.args).unwrap()
}
fn click(
    nav: &mut Navigation,
    state: &mut DesktopAppState,
    id: Uid,
    index: usize,
    double: bool,
    shift: bool,
) -> Option<Value> {
    nav.click(state, AtomHit { object: id, index }, double, shift)
        .unwrap()
        .map(|c| execute(state, c))
}
fn selected(state: &DesktopAppState, id: Uid) -> Vec<usize> {
    let mol = state.scene_manager().object(id).unwrap().mol();
    if mol.selection_text().is_empty() {
        Vec::new()
    } else {
        mol.select_indices(mol.selection())
    }
}

#[test]
fn navigate_toggles_labels_and_residues_and_extends_from_the_anchor() {
    let (mut state, id) = fixture();
    let mut nav = Navigation::default();
    click(&mut nav, &mut state, id, 0, false, false).unwrap();
    let object = state.scene_manager().object(id).unwrap();
    let labels = object
        .renderers()
        .iter()
        .find(|r| r.renderer_type() == "*namelabel")
        .unwrap();
    assert_eq!(labels.name(), "");
    assert_eq!(object.name_label_entries(labels).len(), 1);
    assert!(selected(&state, id).is_empty());
    click(&mut nav, &mut state, id, 0, false, false).unwrap();
    let object = state.scene_manager().object(id).unwrap();
    assert!(object
        .name_label_entries(
            object
                .renderers()
                .iter()
                .find(|r| r.renderer_type() == "*namelabel")
                .unwrap()
        )
        .is_empty());
    click(&mut nav, &mut state, id, 0, true, false).unwrap();
    assert_eq!(selected(&state, id), [0, 1]);
    click(&mut nav, &mut state, id, 3, true, true).unwrap();
    assert_eq!(selected(&state, id), [0, 1, 2, 3]);
    click(&mut nav, &mut state, id, 0, true, false).unwrap();
    assert_eq!(selected(&state, id), [2, 3]);
}

#[test]
fn measurement_clicks_create_live_labels_with_values_and_reset_picks() {
    for (tool, count, expected) in [
        (Tool::Distance, 2, 1.),
        (Tool::Angle, 3, 90.),
        (Tool::Torsion, 4, 90.),
    ] {
        let (mut state, id) = fixture();
        let mut nav = Navigation::default();
        nav.set_tool(tool);
        for i in 0..count - 1 {
            assert!(click(&mut nav, &mut state, id, i, false, false).is_none());
        }
        assert_eq!(nav.picks.len(), count - 1);
        let result = click(&mut nav, &mut state, id, count - 1, false, false).unwrap();
        assert!((result["value"].as_f64().unwrap().abs() - expected).abs() < 1e-9);
        assert!(nav.picks.is_empty());
        let object = state.scene_manager().object(id).unwrap();
        let renderer = object
            .renderers()
            .iter()
            .find(|r| r.renderer_type() == "atomintr")
            .unwrap();
        assert_eq!(object.atom_intr_labels(renderer).len(), 1);
        assert_eq!(
            renderer.qsc_attrs().get("showlabel").map(String::as_str),
            Some("true")
        );
    }
}

#[test]
fn bond_tool_rejects_invalid_pairs_and_preserves_the_first_pick() {
    let (mut state, id) = fixture();
    let mut nav = Navigation::default();
    nav.set_tool(Tool::Bond);
    assert!(click(&mut nav, &mut state, id, 0, false, false).is_none());
    assert!(click(&mut nav, &mut state, id, 0, false, false).is_none());
    assert_eq!(nav.picks.len(), 1);
    let result = click(&mut nav, &mut state, id, 5, false, false).unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(state.nonstandard_bond_summaries(id).unwrap().len(), 1);
    assert!(nav.picks.is_empty());
    click(&mut nav, &mut state, id, 1, false, false);
    nav.set_tool(Tool::Distance);
    assert!(nav.picks.is_empty());
    click(&mut nav, &mut state, id, 1, false, false);
    nav.cancel();
    assert!(nav.picks.is_empty());
}

#[test]
fn polygon_selection_handles_concave_edges_and_shift_addition() {
    let polygon = [[0., 0.], [4., 0.], [4., 1.], [1., 1.], [1., 4.], [0., 4.]];
    assert!(polygon_contains(&polygon, [0.5, 3.]));
    assert!(polygon_contains(&polygon, [1., 2.]));
    assert!(!polygon_contains(&polygon, [2., 2.]));
    let (mut state, id) = fixture();
    for c in selection_commands(
        &state,
        &[AtomHit {
            object: id,
            index: 0,
        }],
        false,
    )
    .unwrap()
    {
        execute(&mut state, c);
    }
    for c in selection_commands(
        &state,
        &[AtomHit {
            object: id,
            index: 3,
        }],
        true,
    )
    .unwrap()
    {
        execute(&mut state, c);
    }
    assert_eq!(selected(&state, id), [0, 3]);
    assert!(selection_commands(&state, &[], false).unwrap().is_empty());
    assert_eq!(selected(&state, id), [0, 3]);
}

#[test]
fn labels_and_context_selection_resolve_insertions_and_alternate_locations() {
    let (mut state, id) = fixture();
    let mol = state.scene_manager_mut().object_mut(id).unwrap().mol_mut();
    for a in mol.atoms_mut().iter_mut().take(2) {
        a.chain_id = "A B".into();
        a.ins_code = Some('A');
        a.name = "CA".into();
    }
    mol.atoms_mut()[0].alt_loc = Some('A');
    mol.atoms_mut()[1].alt_loc = Some('B');
    let mut nav = Navigation::default();
    click(&mut nav, &mut state, id, 1, false, false).unwrap();
    let object = state.scene_manager().object(id).unwrap();
    let renderer = object
        .renderers()
        .iter()
        .find(|r| r.renderer_type() == "*namelabel")
        .unwrap();
    assert_eq!(
        object.name_label_entries(renderer)[0].0,
        object.mol().atoms()[1].position
    );
    let text = selection_for_hit(
        &state,
        AtomHit {
            object: id,
            index: 0,
        },
        "Residue",
    )
    .unwrap();
    execute(&mut state, selection_command(id, text));
    assert_eq!(selected(&state, id), [0, 1]);
}

#[test]
fn viewport_picking_excludes_atoms_hidden_by_renderer_selection() {
    let (mut state, id) = fixture();
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    let renderer = object.renderers()[0].uid();
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_selection(cuemol_molstr::parse_selector("aid 2").unwrap());
    let atoms = state.active_pick_atoms("").unwrap();
    assert_eq!(atoms.len(), 1);
    assert_eq!(atoms[0].atom_index, Some(1));
    state
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_visible(false);
    assert!(state.active_pick_atoms("").unwrap().is_empty());
}

fn duplicate_molecule(state: &mut DesktopAppState, id: Uid) -> Uid {
    let molecule = state.scene_manager().object(id).unwrap().mol().clone();
    let scene = state.active_scene().unwrap();
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("other", molecule)
}
fn measurement_value(state: &DesktopAppState, id: Uid, name: &str) -> Option<f64> {
    let scene = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .unwrap();
    let object = scene.object(id).unwrap();
    let renderer = scene.resolve_atom_intr_renderer(object, object.renderer_by_name(name).unwrap());
    object.atom_intr_values(&renderer)[0]
}

#[test]
fn cross_object_measurements_follow_atoms_transforms_renames_and_scene_reload() {
    use cuemol_core::{Matrix4, Vector4};
    let (mut state, id) = fixture();
    let other = duplicate_molecule(&mut state, id);
    state
        .scene_manager_mut()
        .object_mut(other)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(3., 0., 0.)));
    let mut nav = Navigation::default();
    nav.set_tool(Tool::Distance);
    click(&mut nav, &mut state, id, 0, false, false);
    let value = click(&mut nav, &mut state, other, 1, false, false).unwrap();
    assert_eq!(value["value"], 4.);
    let renderer = state
        .scene_manager()
        .object(id)
        .unwrap()
        .renderer_by_name("measure")
        .unwrap();
    assert!(renderer.atom_intr_lines()[0].positions().is_empty());
    assert_eq!(measurement_value(&state, id, "measure"), Some(4.));
    state
        .scene_manager_mut()
        .object_mut(other)
        .unwrap()
        .mol_mut()
        .atoms_mut()[1]
        .position
        .x = 3.;
    state
        .scene_manager_mut()
        .object_mut(id)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(1., 0., 0.)));
    state
        .scene_manager_mut()
        .object_mut(other)
        .unwrap()
        .set_name("renamed");
    assert_eq!(measurement_value(&state, id, "measure"), Some(5.));
    let mvp = [
        [0.1, 0., 0., 0.],
        [0., 0.1, 0., 0.],
        [0., 0., 0.1, 0.],
        [0., 0., 0., 1.],
    ];
    let labels = state.active_atom_intr_labels(mvp, 100., 100., 10);
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "5.00");
    assert!((labels[0].x - 67.5).abs() < 1e-5);
    let scene = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .unwrap();
    let owner_name = scene.object(id).unwrap().name().to_string();
    let qsc = scene.write_qsc_string();
    assert!(qsc.contains("mol2=\"renamed\""));
    let mut loaded = cuemol_molstr::MolSceneManager::new();
    let sid = loaded.new_scene("reload");
    let scene = loaded.scene_mut(sid).unwrap();
    scene.load_qsc_str(&qsc).unwrap();
    let other = scene.object_by_name("renamed").unwrap().uid();
    let owner = scene.object_by_name(&owner_name).unwrap().uid();
    scene.object_mut(other).unwrap().set_name("again");
    scene.object_mut(other).unwrap().mol_mut().atoms_mut()[1]
        .position
        .x = 4.;
    let object = scene.object(owner).unwrap();
    let renderer =
        scene.resolve_atom_intr_renderer(object, object.renderer_by_name("measure").unwrap());
    assert_eq!(object.atom_intr_values(&renderer), vec![Some(6.)]);
    assert!(scene.write_qsc_string().contains("mol2=\"again\""));
    scene.destroy_object(other);
    let object = scene.object(owner).unwrap();
    let renderer =
        scene.resolve_atom_intr_renderer(object, object.renderer_by_name("measure").unwrap());
    assert!(object.atom_intr_segments(&renderer).is_empty());
}

#[cfg(feature = "scripting")]
#[test]
fn scripted_and_contact_measurements_keep_live_cross_object_references() {
    let (mut state, id) = fixture();
    let other = duplicate_molecule(&mut state, id);
    let source = format!("var r = cuemol.getObject({}).createRenderer('atomintr', 'script'); r.appendById(1, {}, 2, false);", id.get(), other.get());
    call_tool(
        &mut state,
        &mut Camera::new(),
        "execute_script",
        &serde_json::json!({"source": source}),
    )
    .unwrap();
    let contacts = state
        .define_atom_contacts(
            id,
            "aid 1",
            Some(other),
            Some("aid 2"),
            0.,
            2.,
            false,
            10,
            "contacts",
        )
        .unwrap();
    assert_eq!(contacts.count, 1);
    assert_eq!(measurement_value(&state, id, "script"), Some(1.));
    assert_eq!(measurement_value(&state, id, "contacts"), Some(1.));
    state
        .scene_manager_mut()
        .object_mut(other)
        .unwrap()
        .mol_mut()
        .atoms_mut()[1]
        .position
        .x = 7.;
    assert_eq!(measurement_value(&state, id, "script"), Some(7.));
    assert_eq!(measurement_value(&state, id, "contacts"), Some(7.));
}

#[test]
fn measurement_labels_and_geometry_apply_object_transforms_once() {
    use cuemol_core::{Matrix4, Vector4};
    let (mut state, id) = fixture();
    let mut nav = Navigation::default();
    nav.set_tool(Tool::Distance);
    click(&mut nav, &mut state, id, 0, false, false);
    click(&mut nav, &mut state, id, 1, false, false);
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    let hidden = object.renderers()[0].uid();
    object.renderer_mut(hidden).unwrap().set_visible(false);
    let renderer = object.renderer_by_name("measure").unwrap().uid();
    for (key, value) in [("mode", "simple"), ("stipple0", "-1"), ("stipple1", "-1")] {
        object
            .renderer_mut(renderer)
            .unwrap()
            .set_qsc_attr(key, value);
    }
    object.set_xform_matrix(
        Matrix4::make_trans_mat(Vector4::new3(1., 0., 0.))
            .mul_mat(Matrix4::make_scale_mat(Vector4::new3(2., 2., 2.))),
    );
    assert_eq!(measurement_value(&state, id, "measure"), Some(2.));
    let snapshot = state.active_render_snapshot("").unwrap();
    assert!(!snapshot.bonds.is_empty());
    assert_eq!(snapshot.bonds[0].from.x, 1.);
    assert_eq!(snapshot.bonds[0].to.x, 3.);
    let mvp = [
        [0.1, 0., 0., 0.],
        [0., 0.1, 0., 0.],
        [0., 0., 0.1, 0.],
        [0., 0., 0., 1.],
    ];
    let labels = state.active_atom_intr_labels(mvp, 100., 100., 10);
    assert_eq!(labels[0].text, "2.00");
    assert!((labels[0].x - 60.).abs() < 1e-5);
    let lines = state.active_atom_intr_lines(mvp, 100., 100., 10);
    assert!((lines[0].x1 - 55.).abs() < 1e-5);
    assert!((lines[0].x2 - 65.).abs() < 1e-5);
}

#[test]
fn backbone_picking_uses_visible_residue_pivots_and_renderer_transforms() {
    use cuemol_core::{Matrix4, Vector4};
    let (mut state, id) = fixture();
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    for (atom, name) in object
        .mol_mut()
        .atoms_mut()
        .iter_mut()
        .zip(["CA", "CB", "CA", "CA", "CA", "CB"])
    {
        atom.name = name.into();
        atom.res_name = "ALA".into();
    }
    object.mol_mut().apply_topology();
    for renderer in object
        .renderers()
        .iter()
        .map(|r| r.uid())
        .collect::<Vec<_>>()
    {
        object.renderer_mut(renderer).unwrap().set_visible(false);
    }
    let renderer = object.add_renderer("ribbon", "backbone");
    object.set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(1., 0., 0.)));
    object.renderer_mut(renderer).unwrap().set_qsc_attr(
        "xformMat",
        &Matrix4::make_trans_mat(Vector4::new3(2., 0., 0.)).to_script_string(),
    );
    let atoms = state.active_pick_atoms("").unwrap();
    assert_eq!(
        atoms
            .iter()
            .filter_map(|a| a.atom_index)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4]
    );
    assert_eq!(atoms[0].center.x, 3.);
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_selection(cuemol_molstr::parse_selector("name CB").unwrap());
    assert_eq!(
        state
            .active_pick_atoms("")
            .unwrap()
            .iter()
            .filter_map(|a| a.atom_index)
            .collect::<Vec<_>>(),
        [0, 4]
    );
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_selection(cuemol_molstr::parse_selector("*").unwrap());
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("pivotatom", "CB");
    assert_eq!(
        state
            .active_pick_atoms("")
            .unwrap()
            .iter()
            .filter_map(|a| a.atom_index)
            .collect::<Vec<_>>(),
        [1, 2, 3, 5]
    );
}
