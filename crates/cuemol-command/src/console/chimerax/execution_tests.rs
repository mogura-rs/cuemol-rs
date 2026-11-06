use super::*;
use crate::console::{Action, Dialect, Session, Submission};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;

fn state() -> DesktopAppState {
    let mut state = DesktopAppState::new();
    state.open_bytes("first.pdb",b"ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00           C\nATOM      2  N   ALA A   1       2.000   3.000   4.000  1.00 10.00           N\nATOM      3  CB  ALA A   1       3.000   2.000   1.000  1.00 10.00           C\nEND\n").unwrap();
    state
}
fn run(
    session: &mut Session,
    state: &mut DesktopAppState,
    camera: &mut Camera,
    source: &str,
) -> Result<Vec<String>, String> {
    let mut submission = Submission::new(source, Dialect::ChimeraX)?;
    let mut out = Vec::new();
    while let Some(action) = submission.next(session, state, camera)? {
        match action {
            Action::Tool { name, args } => {
                crate::call_tool(state, camera, &name, &args)
                    .map_err(|e| format!("{name}: {e}"))?;
            }
            Action::Output(text) => out.push(text),
            Action::Echo(_) => {}
            other => return Err(format!("Unexpected action: {other:?}")),
        }
    }
    Ok(out)
}
fn selected(state: &DesktopAppState, object: Uid, text: &str) -> Vec<String> {
    let selector = state.parse_object_selector(object, text).unwrap();
    let mol = state.scene_manager().object(object).unwrap().mol();
    mol.select_indices(&selector)
        .into_iter()
        .map(|i| mol.atoms()[i].name.clone())
        .collect()
}
fn displayed(state: &DesktopAppState, object: Uid, name: &str) -> Vec<String> {
    let renderer = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderers()
        .iter()
        .find(|r| r.name() == name)
        .unwrap();
    selected(state, object, renderer.qsc_attr("sel").unwrap_or("*"))
}
#[test]
fn live_selections_display_and_aliases_share_scene_state() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    let original = state.renderer_summaries(object).len();
    run(
        &mut session,
        &mut state,
        &mut camera,
        "alias mark select $1; show sel\nmark #1@CA\nstyle sel sphere",
    )
    .unwrap();
    assert_eq!(
        selected(
            &state,
            object,
            state.molecule_selection_text(object).unwrap()
        ),
        vec!["CA"]
    );
    assert_eq!(displayed(&state, object, "chx:sphere"), vec!["CA"]);
    assert!(displayed(&state, object, "chx:stick").is_empty());
    assert_eq!(state.renderer_summaries(object).len(), original + 5);
    run(
        &mut session,
        &mut state,
        &mut camera,
        "hide sel; style sel ball; show sel",
    )
    .unwrap();
    assert_eq!(displayed(&state, object, "chx:stick"), vec!["CA"]);
    assert!(displayed(&state, object, "chx:ball").is_empty());
    let sphere = state
        .renderer_summaries(object)
        .into_iter()
        .find(|r| r.name == "chx:sphere")
        .unwrap()
        .uid;
    state
        .set_renderer_selection_text(object, sphere, "name N")
        .unwrap();
    run(
        &mut session,
        &mut state,
        &mut camera,
        "show #1; style #1@N ball",
    )
    .unwrap();
    assert_eq!(displayed(&state, object, "chx:ball"), vec!["N"]);
    assert!(displayed(&state, object, "chx:sphere").is_empty());
    run(
        &mut session,
        &mut state,
        &mut camera,
        "name alpha @CA\nselect alpha\nselect subtract @CA",
    )
    .unwrap();
    assert!(selected(
        &state,
        object,
        state.molecule_selection_text(object).unwrap()
    )
    .is_empty());
    let output = run(
        &mut session,
        &mut state,
        &mut camera,
        "info atoms @CA attribute name",
    )
    .unwrap();
    assert!(output.iter().any(|line| line.ends_with("@CA name CA")));
}
#[test]
fn alias_refuses_history_operations_before_applying_edits() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    state.set_molecule_selection_text(object, "name N").unwrap();
    run(
        &mut session,
        &mut state,
        &mut camera,
        "alias unsafe select all; undo",
    )
    .unwrap();
    assert!(run(&mut session, &mut state, &mut camera, "unsafe")
        .unwrap_err()
        .contains("cannot run inside an alias"));
    assert_eq!(state.molecule_selection_text(object).unwrap(), "name N");
    run(
        &mut session,
        &mut state,
        &mut camera,
        "alias select select $*\nselect @CA",
    )
    .unwrap();
    assert_eq!(
        selected(
            &state,
            object,
            state.molecule_selection_text(object).unwrap()
        ),
        vec!["CA"]
    );
}
#[test]
fn model_numbers_survive_closes_undo_and_scene_switches() {
    let mut state = state();
    let mut session = Session::default();
    let first = session.chimerax.models(&state).unwrap()[0].clone();
    let snapshot = state.clone();
    let scene = state.active_scene().unwrap();
    crate::call_tool(
        &mut state,
        &mut Camera::new(),
        "delete_node",
        &serde_json::json!({"nodeType":"object","nodeId":first.id.get()}),
    )
    .unwrap();
    let second = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("second", cuemol_molstr::MolCoord::new());
    assert_eq!(
        session
            .chimerax
            .models(&state)
            .unwrap()
            .iter()
            .find(|m| m.id == second)
            .unwrap()
            .number,
        2
    );
    state = snapshot;
    assert_eq!(session.chimerax.models(&state).unwrap()[0].number, 1);
    state.new_scene("other", true);
    let scene = state.active_scene().unwrap();
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("third", cuemol_molstr::MolCoord::new());
    assert_eq!(session.chimerax.models(&state).unwrap()[0].number, 1);
}

#[test]
fn camera_commands_preserve_selection_and_use_reference_screen_axes() {
    use cuemol_core::{Matrix4, Vector4};
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    state.set_molecule_selection_text(object, "name N").unwrap();
    camera.set_center(Vector4::new3(0., 0., 0.));
    run(
        &mut session,
        &mut state,
        &mut camera,
        "turn y 3 30; move z 4",
    )
    .unwrap();
    assert!((camera.center() - Vector4::new3(4., 0., 0.)).length3d() < 1e-10);
    run(
        &mut session,
        &mut state,
        &mut camera,
        "view name start; view #1@CA; view start",
    )
    .unwrap();
    assert!((camera.center() - Vector4::new3(4., 0., 0.)).length3d() < 1e-10);
    let output = run(&mut session, &mut state, &mut camera, "view matrix").unwrap();
    assert!(output[0].starts_with("view matrix camera "));
    let bad = camera.clone();
    assert!(run(&mut session, &mut state, &mut camera, "turn z 3 forever").is_err());
    assert!(crate::console::same_camera_view(&camera, &bad));
    camera.set_rotation(cuemol_core::Quat::IDENTITY);
    let mut transform = Matrix4::IDENTITY;
    transform.values[0][3] = 10.;
    state
        .set_workspace_object_transform(object, &transform.to_script_string())
        .unwrap();
    run(&mut session, &mut state, &mut camera, "view @CA").unwrap();
    assert_eq!(camera.center(), Vector4::new3(11., 2., 3.));
    assert_eq!(state.molecule_selection_text(object).unwrap(), "name N");
    run(
        &mut session,
        &mut state,
        &mut camera,
        "view matrix camera 1,0,0,0,0,1,0,0,0,0,1,50",
    )
    .unwrap();
    assert!(camera.is_perspective());
    assert!((camera.cam_dist() - 47.5).abs() < 1e-10);
    assert!((camera.center().z - 2.5).abs() < 1e-10);
}

#[test]
fn paint_survives_style_changes_and_keeps_prior_entries() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    run(
        &mut session,
        &mut state,
        &mut camera,
        "show; color red target a; color @N blue; style sphere",
    )
    .unwrap();
    for name in ["chx:stick", "chx:ball", "chx:sphere"] {
        let renderer = state
            .renderer_summaries(object)
            .into_iter()
            .find(|r| r.name == name)
            .unwrap()
            .uid;
        let (_, _, entries) = state.renderer_coloring_state(object, renderer).unwrap();
        assert_eq!(entries.len(), 6);
        assert_eq!(entries[0].1, "#0000ff");
        assert_eq!(entries[1].1, "#ff0000");
    }
    run(
        &mut session,
        &mut state,
        &mut camera,
        "color bychain; color byhetero; rainbow chains palette red:blue",
    )
    .unwrap();
    let output = run(
        &mut session,
        &mut state,
        &mut camera,
        "color rgba(1.0,0,0,0.5)",
    )
    .unwrap();
    assert!(output.iter().any(|line| line.contains("drawn opaque")));
}

#[test]
fn surfaces_keep_calculation_atoms_when_hiding_patches() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    run(
        &mut session,
        &mut state,
        &mut camera,
        "surface #1@CA color red transparency 20; show #1@N surfaces; surface hidePatches #1@CA",
    )
    .unwrap();
    let renderer = state
        .renderer_summaries(object)
        .into_iter()
        .find(|r| r.name == "chx:surface:A")
        .unwrap()
        .uid;
    let r = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap();
    assert_eq!(
        selected(&state, object, r.qsc_attr("sel").unwrap()),
        vec!["CA", "N", "CB"]
    );
    assert_eq!(
        selected(&state, object, r.qsc_attr("showsel").unwrap()),
        vec!["N"]
    );
    assert!((r.qsc_attr("alpha").unwrap().parse::<f64>().unwrap() - 0.8).abs() < 1e-6);
    run(
        &mut session,
        &mut state,
        &mut camera,
        "surface enclose #1@CB; ~surface #1@CB",
    )
    .unwrap();
    assert!(run(
        &mut session,
        &mut state,
        &mut camera,
        "transparency @N 50 atoms"
    )
    .unwrap_err()
    .contains("whole models"));
    run(
        &mut session,
        &mut state,
        &mut camera,
        "transparency #1 50 atoms; surface close #1@N",
    )
    .unwrap();
    assert!(!state
        .renderer_summaries(object)
        .iter()
        .any(|r| r.name.starts_with("chx:surface:")));
}

#[test]
fn residue_labels_anchor_ca_when_side_chain_is_selected() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    run(
        &mut session,
        &mut state,
        &mut camera,
        "label @CB color blue size 16 onTop true; label @N atoms attribute bfactor",
    )
    .unwrap();
    let r = state
        .renderer_summaries(object)
        .into_iter()
        .find(|r| r.name == "chx:label:residues")
        .unwrap()
        .uid;
    let props =
        crate::properties::node_property_values(&state, &camera, "renderer", Some(r)).unwrap();
    assert_eq!(props.get("font_size").map(String::as_str), Some("16"));
    let object_data = state.scene_manager().object(object).unwrap();
    let labels = object_data
        .name_label_entries_with_format(object_data.renderer(r).unwrap(), "{resn} {resi}");
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].0, object_data.mol().atoms()[0].position);
    run(
        &mut session,
        &mut state,
        &mut camera,
        "label delete @CB; ~label @N atoms",
    )
    .unwrap();
    let object_data = state.scene_manager().object(object).unwrap();
    assert!(object_data
        .name_label_entries_with_format(object_data.renderer(r).unwrap(), "{resn} {resi}")
        .is_empty());
}

#[test]
fn distances_delete_and_alignment_report_after_transform() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let object = state.active_object_summaries()[0].uid;
    let output = run(
        &mut session,
        &mut state,
        &mut camera,
        "distance @CA,N decimalPlaces 2",
    )
    .unwrap();
    assert!(output.iter().any(|s| s.ends_with("1.73Å")));
    run(
        &mut session,
        &mut state,
        &mut camera,
        "distance delete @CA,N",
    )
    .unwrap();
    let r = state
        .renderer_summaries(object)
        .into_iter()
        .find(|r| r.name == "chx:distance")
        .unwrap()
        .uid;
    assert!(state.atom_intr_def_summaries(object, r).unwrap().is_empty());
    let scene = state.active_scene().unwrap();
    let mol = state.scene_manager().object(object).unwrap().mol().clone();
    let second = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("second", mol);
    let mut transform = cuemol_core::Matrix4::IDENTITY;
    transform.values[0][3] = 20.;
    state
        .set_workspace_object_transform(second, &transform.to_script_string())
        .unwrap();
    let output = run(&mut session, &mut state, &mut camera, "align #2 toAtoms #1").unwrap();
    assert!(
        output.iter().any(|s| s.ends_with("0.000 angstroms")),
        "{output:?}"
    );
    let output = run(&mut session, &mut state, &mut camera, "distance #1@CA|#2@N").unwrap();
    assert!(output
        .iter()
        .any(|s| s.contains("#1/A") && s.contains("#2/A")));
    run(&mut session, &mut state, &mut camera, "~distance").unwrap();
    assert!(!state
        .renderer_summaries(object)
        .iter()
        .any(|r| r.name == "chx:distance"));
}

#[test]
fn open_save_delete_and_script_use_shared_file_operations() {
    let mut state = state();
    let mut session = Session::default();
    let mut camera = Camera::new();
    let dir = std::env::temp_dir().join(format!(
        "cuemol-chx-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let pdb = dir.join("model.pdb");
    let object = state.active_object_summaries()[0].uid;
    crate::call_tool(
        &mut state,
        &mut camera,
        "save_object",
        &serde_json::json!({"objId":object.get(),"fileName":pdb.to_string_lossy(),"link":false}),
    )
    .unwrap();
    run(
        &mut session,
        &mut state,
        &mut camera,
        &format!("open \"{}\" name opened", pdb.display()),
    )
    .unwrap();
    let new = state
        .active_object_summaries()
        .into_iter()
        .find(|o| o.name == "opened")
        .unwrap()
        .uid;
    assert_eq!(displayed(&state, new, "chx:stick"), vec!["CA", "N", "CB"]);
    assert!(displayed(&state, new, "chx:cartoon").is_empty());
    let r = state
        .renderer_summaries(new)
        .into_iter()
        .find(|r| r.name == "chx:stick")
        .unwrap()
        .uid;
    let (_, _, entries) = state.renderer_coloring_state(new, r).unwrap();
    assert!(
        entries.iter().any(|entry| entry.0 == "*"),
        "model paint was lost: {entries:?}"
    );
    let saved = dir.join("export.dat");
    run(
        &mut session,
        &mut state,
        &mut camera,
        &format!("save \"{}\" format pdb models #2", saved.display()),
    )
    .unwrap();
    assert!(std::fs::read_to_string(&saved).unwrap().contains("ATOM"));
    let script = dir.join("select.cxc");
    std::fs::write(&script, "select #2@CA\n").unwrap();
    run(
        &mut session,
        &mut state,
        &mut camera,
        &format!("open \"{}\"", script.display()),
    )
    .unwrap();
    assert_eq!(
        selected(&state, new, state.molecule_selection_text(new).unwrap()),
        vec!["CA"]
    );
    run(
        &mut session,
        &mut state,
        &mut camera,
        "delete #2@CB; close #1",
    )
    .unwrap();
    assert_eq!(
        state
            .scene_manager()
            .object(new)
            .unwrap()
            .mol()
            .atoms()
            .len(),
        2
    );
    assert!(state.scene_manager().object(object).is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_uses_keyword_value_types_and_quotes_paths() {
    let state = state();
    let session = Session::default();
    let complete = |text| {
        super::complete(&session, &state, text)
            .into_iter()
            .map(|c| c.replacement)
            .collect::<Vec<_>>()
    };
    assert_eq!(complete("sty"), vec!["style "]);
    assert_eq!(
        complete("style atomStyle sph"),
        vec!["style atomStyle sphere "]
    );
    assert_eq!(complete("set silhouettes t"), vec!["set silhouettes true "]);
    assert!(complete("color col red").contains(&"color col red ".into()));
    assert!(!complete("label onTop r")
        .iter()
        .any(|s| s.ends_with("red ")));
    let dir = std::env::temp_dir().join(format!("cuemol-chx-complete-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("white space.pdb"), "").unwrap();
    assert!(complete(&format!("open {}/white", dir.display()))
        .contains(&format!("open \"{}/white space.pdb\" ", dir.display())));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn first_model_numbers_follow_workspace_order_then_remain_stable() {
    let mut state = state();
    let mut camera = Camera::new();
    let mut session = Session::default();
    let first = state.active_object_summaries()[0].uid;
    let scene = state.active_scene().unwrap();
    let second = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("second", cuemol_molstr::MolCoord::new());
    crate::call_tool(&mut state,&mut camera,"set_node_prop",&serde_json::json!({"nodeType":"object","nodeId":first.get(),"prop":"ui_order","value":"20"})).unwrap();
    crate::call_tool(&mut state,&mut camera,"set_node_prop",&serde_json::json!({"nodeType":"object","nodeId":second.get(),"prop":"ui_order","value":"10"})).unwrap();
    state.open_bytes("density.dx",b"object 1 class gridpositions counts 1 1 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 1 1 2\nobject 3 class array type double rank 0 items 2 data follows\n-1 1\n").unwrap();
    let models = session.chimerax.models(&state).unwrap();
    assert_eq!(models.len(), 3);
    assert!(!models[2].molecule);
    assert_eq!(models[0].id, second);
    assert_eq!(models[1].id, first);
    crate::call_tool(&mut state,&mut camera,"set_node_prop",&serde_json::json!({"nodeType":"object","nodeId":first.get(),"prop":"ui_order","value":"5"})).unwrap();
    assert_eq!(session.chimerax.models(&state).unwrap()[0].id, second);
}
