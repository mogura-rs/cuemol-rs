#![cfg(feature = "scripting")]

use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::{AtomIntrKind, AtomIntrLine};

const PDB: &str = "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nHETATM    2  C2  LIG A   1       1.000   0.000   0.000  1.00 10.00           C\nHETATM    3  C3  LIG A   1       2.000   1.000   0.000  1.00 10.00           C\nHETATM    4  C4  LIG A   1       3.000   2.000   1.000  1.00 10.00           C\nHETATM    5  C5  LIG A   1      20.000  20.000  20.000  1.00 10.00           C\nHETATM    6  C6  LIG A   1      30.000  30.000  30.000  1.00 10.00           C\nEND\n";
const CROSS: &str = "name C1 around 1.5 [Target]";
const STAGES: [([f64; 3], [f64; 3]); 3] = [
    ([0., 0., 0.], [0.5, 135., 90.]),
    ([1., 0., 0.], [1. / 3., 45., 90.]),
    ([2., 1., 1.], [2.179449471770337, 13.262676008304892, 0.]),
];
const MVP: [[f32; 4]; 4] = [
    [0.1, 0., 0., 0.],
    [0., 0.1, 0., 0.],
    [0., 0., 0.1, 0.],
    [0., 0., 0., 1.],
];

fn fixture() -> (DesktopAppState, Uid, Uid) {
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("measurements", true).result_scene;
    let scene = state.scene_manager_mut().scene_mut(scene).unwrap();
    let probe = scene.add_molecule("Probe", cuemol_molstr::read_pdb_str(PDB).unwrap());
    let target = scene.add_molecule("Target", cuemol_molstr::read_pdb_str(PDB).unwrap());
    (state, probe, target)
}

fn shift(state: &mut DesktopAppState, object: Uid, xyz: [f64; 3]) {
    state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .set_xform_matrix(Matrix4::make_trans_mat(Vector4::new3(
            xyz[0], xyz[1], xyz[2],
        )));
}

fn install_definitions(state: &mut DesktopAppState, object: Uid) -> Uid {
    let object = state.scene_manager_mut().object_mut(object).unwrap();
    let renderer = object.add_renderer("atomintr", "measure");
    let output = object.renderer_mut(renderer).unwrap();
    output.set_qsc_attr("showlabel", "true");
    output.set_qsc_attr("mode", "simple");
    for (kind, tail) in [
        (AtomIntrKind::Distance, vec!["name C2"]),
        (AtomIntrKind::Angle, vec!["name C2", "name C3"]),
        (AtomIntrKind::Torsion, vec!["name C2", "name C3", "name C4"]),
    ] {
        let selectors = std::iter::once(CROSS)
            .chain(tail)
            .map(String::from)
            .collect();
        output
            .append_atom_intr_line(AtomIntrLine::from_selections(kind, selectors))
            .unwrap();
    }
    renderer
}

fn values(state: &DesktopAppState, object: Uid) -> Vec<Option<f64>> {
    let scene = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .unwrap();
    let object = scene.object(object).unwrap();
    let renderer =
        scene.resolve_atom_intr_renderer(object, object.renderer_by_name("measure").unwrap());
    object.atom_intr_values(&renderer)
}

fn assert_values(actual: Vec<Option<f64>>, expected: [f64; 3]) {
    assert_eq!(actual.len(), 3);
    for (actual, expected) in actual.into_iter().zip(expected) {
        let actual = actual.expect("measurement must resolve scene selections");
        assert!((actual - expected).abs() < 2e-6, "{actual} != {expected}");
    }
}

#[test]
fn scene_measurements_resolve_cross_selection_centroids_and_reload_live_definitions() {
    let (mut state, object, target) = fixture();
    install_definitions(&mut state, object);
    for (xyz, expected) in STAGES {
        shift(&mut state, target, xyz);
        assert_values(values(&state, object), expected);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("measurements.qsc");
    state.save_active_scene_qsc(&path).unwrap();
    let mut loaded = DesktopAppState::new();
    loaded.load_qsc_file(&path).unwrap();
    let scene = loaded
        .scene_manager()
        .scene(loaded.active_scene().unwrap())
        .unwrap();
    let object = scene.object_by_name("Probe").unwrap().uid();
    let target = scene.object_by_name("Target").unwrap().uid();
    assert_values(values(&loaded, object), STAGES[2].1);
    shift(&mut loaded, target, [20., 0., 0.]);
    assert_eq!(values(&loaded, object), [None, None, None]);
    shift(&mut loaded, target, [0., 0., 0.]);
    assert_values(values(&loaded, object), STAGES[0].1);
    let renderer = loaded
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer_by_name("measure")
        .unwrap();
    assert!(renderer
        .atom_intr_lines()
        .iter()
        .all(|line| line.positions().is_empty()));
    assert_eq!(renderer.atom_intr_lines()[0].selectors()[0], CROSS);
}

#[test]
fn script_measurement_append_and_values_use_the_scene_selection_context() {
    for (xyz, expected) in STAGES {
        let (mut state, object, target) = fixture();
        shift(&mut state, target, xyz);
        let source = format!("var r=cuemol.getObject({}).createRenderer('atomintr','measure'); print(JSON.stringify([r.append('{CROSS}','name C2'),r.appendAngle('{CROSS}','name C2','name C3'),r.appendTorsion('{CROSS}','name C2','name C3','name C4')]));",object.get());
        assert_eq!(state.run_script_js(&source).unwrap().0, "[0,1,2]\n");
        let source = format!("var r=cuemol.getObject({}).getRendererByName('measure'); print(JSON.stringify([r.getValue(0),r.getValue(1),r.getValue(2)]));",object.get());
        let result = state.run_script_js(&source).unwrap().0;
        assert_values(serde_json::from_str(&result).unwrap(), expected);
        shift(&mut state, target, STAGES[0].0);
        let result = state.run_script_js(&source).unwrap().0;
        assert_values(serde_json::from_str(&result).unwrap(), STAGES[0].1);
    }
    let (mut state, object, target) = fixture();
    shift(&mut state, target, [20., 0., 0.]);
    assert_eq!(state.run_script_js(&format!("var r=cuemol.getObject({}).createRenderer('atomintr','empty'); print(r.append('{CROSS}','name C2'));",object.get())).unwrap().0, "-1\n");
}

#[test]
fn measurement_screen_labels_and_lines_follow_target_selection_changes() {
    let (mut state, object, target) = fixture();
    install_definitions(&mut state, object);
    for (xyz, expected) in STAGES {
        shift(&mut state, target, xyz);
        let labels = state.active_atom_intr_labels(MVP, 200., 200., 10);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].text, format!("{:.2}", expected[0]));
        assert_eq!(state.active_atom_intr_lines(MVP, 200., 200., 20).len(), 6);
    }
    shift(&mut state, target, [20., 0., 0.]);
    assert!(state
        .active_atom_intr_labels(MVP, 200., 200., 10)
        .is_empty());
    assert!(state.active_atom_intr_lines(MVP, 200., 200., 20).is_empty());
}

#[test]
fn measurement_selection_centroids_apply_owner_and_target_transforms_once() {
    let (mut state, object, target) = fixture();
    install_definitions(&mut state, object);
    let offset = [10., -3., 5.];
    shift(&mut state, object, offset);
    for (xyz, expected) in STAGES {
        shift(
            &mut state,
            target,
            std::array::from_fn(|i| xyz[i] + offset[i]),
        );
        assert_values(values(&state, object), expected);
        let result = state.run_script_js(&format!("var r=cuemol.getObject({}).getRendererByName('measure'); print(JSON.stringify([r.getValue(0),r.getValue(1),r.getValue(2)]));",object.get())).unwrap().0;
        assert_values(serde_json::from_str(&result).unwrap(), expected);
    }
}

#[test]
fn named_measurements_keep_live_definitions_for_labels_geometry_and_scene_saving() {
    let (mut state, object, target) = fixture();
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_visible(false);
    state.run_script_js(&format!("var styles=cuemol.getService('StyleManager'); var sid=styles.createStyleSet('measurements',scene.uid); styles.setStrData('sel','focus','name C1',scene.uid,sid); var r=cuemol.getObject({}).createRenderer('atomintr','measure'); r.showlabel=true; r.mode='fancy'; print(r.append('focus','name C2'));", object.get())).map(|r| assert_eq!(r.0,"0\n")).unwrap();
    let renderer = state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer_by_name("measure")
        .unwrap();
    assert_eq!(
        renderer.atom_intr_lines()[0].selectors(),
        ["focus", "name C2"]
    );
    for (expression, expected) in [
        ("name C1", Some(1.)),
        ("name C3", Some(2_f64.sqrt())),
        ("name absent", None),
        ("name C1", Some(1.)),
    ] {
        state
            .run_script_js(&format!(
                "var styles=cuemol.getService('StyleManager'), sid=styles.hasStyleSet('measurements',scene.uid); styles.setStrData('sel','focus','{expression}',scene.uid,sid);"
            ))
            .unwrap();
        let labels = state.active_atom_intr_labels(MVP, 200., 200., 10);
        let snapshot = state.active_render_snapshot("").unwrap();
        if let Some(expected) = expected {
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].text, format!("{expected:.2}"));
            assert!(snapshot.mesh_triangle_count() > 0);
        } else {
            assert!(labels.is_empty());
            assert_eq!(snapshot.mesh_triangle_count(), 0);
        }
        let bytes = state.active_scene_bytes().unwrap();
        let xml = String::from_utf8_lossy(&bytes);
        assert!(xml.contains("sel1=\"focus\""), "{xml}");
        let mut loaded = DesktopAppState::new();
        loaded.open_bytes("named.qsc", &bytes).unwrap();
        assert_eq!(loaded.active_atom_intr_labels(MVP, 200., 200., 10), labels);
        assert_eq!(
            loaded
                .active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            snapshot.mesh_triangle_count()
        );
    }
    state
        .run_script_js("var styles=cuemol.getService('StyleManager'), sid=styles.hasStyleSet('measurements',scene.uid); styles.removeStrData('sel','focus',scene.uid,sid);")
        .unwrap();
    assert!(state
        .active_atom_intr_labels(MVP, 200., 200., 10)
        .is_empty());
    assert_eq!(
        state
            .active_render_snapshot("")
            .unwrap()
            .mesh_triangle_count(),
        0
    );
    state
        .run_script_js("var styles=cuemol.getService('StyleManager'), sid=styles.hasStyleSet('measurements',scene.uid); styles.setStrData('sel','focus','name C1',scene.uid,sid);")
        .unwrap();
    assert_eq!(
        state.active_atom_intr_labels(MVP, 200., 200., 10)[0].text,
        "1.00"
    );
}

#[test]
fn fancy_angle_and_torsion_caps_point_away_from_the_first_vertex() {
    let (mut state, object, target) = fixture();
    state
        .scene_manager_mut()
        .object_mut(target)
        .unwrap()
        .set_visible(false);
    let points = [
        Vector4::new3(-3., -2., 0.),
        Vector4::new3(0., 1., 0.),
        Vector4::new3(4., -2., 0.),
        Vector4::new3(5., -2., 2.),
    ];
    let source = state.scene_manager_mut().object_mut(object).unwrap();
    let renderer = source.add_renderer("atomintr", "arrows");
    let output = source.renderer_mut(renderer).unwrap();
    for (name, value) in [
        ("mode", "fancy"),
        ("captype_start", "flat"),
        ("captype_end", "arrow"),
        ("stipple0", "-1"),
        ("arrowheight", "0.8"),
        ("arrowwidth", "3"),
    ] {
        output.set_qsc_attr(name, value);
    }
    for kind in [
        AtomIntrKind::Distance,
        AtomIntrKind::Angle,
        AtomIntrKind::Torsion,
    ] {
        output
            .append_atom_intr_line(AtomIntrLine::from_positions(
                kind,
                points[..kind.atom_count()].to_vec(),
            ))
            .unwrap();
    }
    let source = state.scene_manager().object(object).unwrap();
    let output = source.renderer(renderer).unwrap();
    let fancy = [
        (points[0], points[1]),
        (points[1], points[0]),
        (points[1], points[2]),
        (points[1], points[0]),
        (points[1], points[2]),
        (points[2], points[3]),
    ];
    assert_eq!(source.atom_intr_fancy_segments(output), fancy);
    let options = cuemol_render::AtomIntrRenderOptions {
        start_cap: cuemol_render::AtomIntrCapType::Flat,
        end_cap: cuemol_render::AtomIntrCapType::Arrow,
        stipples: [-1.; 6],
        arrow_height: 0.8,
        arrow_width: 3.,
        ..Default::default()
    };
    let expected =
        cuemol_render::atom_intr_snapshot_with_options(&fancy, Some(object.get()), &options);
    assert_eq!(
        state.active_render_snapshot("").unwrap().meshes,
        expected.meshes
    );
    state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("mode", "simple");
    let simple = state.active_render_snapshot("").unwrap();
    let pairs: Vec<_> = simple
        .bonds
        .iter()
        .map(|bond| (bond.from, bond.to))
        .collect();
    assert_eq!(
        pairs,
        [
            (points[0], points[1]),
            (points[0], points[1]),
            (points[1], points[2]),
            (points[0], points[1]),
            (points[1], points[2]),
            (points[2], points[3])
        ]
    );
}
