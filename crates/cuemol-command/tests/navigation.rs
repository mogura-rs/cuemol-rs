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
