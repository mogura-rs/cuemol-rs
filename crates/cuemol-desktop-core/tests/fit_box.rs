#![cfg(feature = "scripting")]

use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::read_pdb_str;

#[test]
fn fitting_resolves_named_selections_and_keeps_other_views_unchanged() {
    let mut state = DesktopAppState::new();
    let created = state.new_scene("fit-box", true);
    let molecule = read_pdb_str(
        "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\n\
         ATOM      2  CA  ALA B   2      10.000  20.000  30.000  1.00 10.00           C\nEND\n",
    )
    .unwrap();
    state
        .scene_manager_mut()
        .scene_mut(created.result_scene)
        .unwrap()
        .add_molecule("probe", molecule);
    let (output, _) = state
        .run_script_js(
            r#"
        var view = scene.getView(scene.activeViewID);
        var mol = cuemol.getObject('probe');
        var matrix = cuemol.createObj('Matrix');
        matrix.setTranslate(cuemol.Vector('(4,5,6)')); mol.xformMat = matrix;
        var styles = cuemol.getService('StyleManager');
        var set = styles.createStyleSet('fit-selections', scene.uid);
        styles.setStrData('sel', 'focus', 'chain B', scene.uid, set);
        var target = scene.createView(); target.sizeChanged(100,200);
        view.zoom = 77;
        var selection = cuemol.makeSel('focus', scene.uid);
        var before = target.zoom;
        print(mol.getFitBoxJSON(target, selection));
        print(target.zoom === before);
        mol.fitView2(target, selection);
        print([target.center.x,target.center.y,target.center.z,target.zoom,target.slab].join(','));
        print(view.zoom);
        print(mol.getFitBoxJSON(target, cuemol.makeSel('none')) === '');
        target.fitToBox(cuemol.Vector('(1,1,1)'), cuemol.Vector('(0,0,0)'));
        print([target.center.x,target.center.y,target.center.z,target.zoom,target.slab].join(','));
        mol.sel = selection; target.center = cuemol.Vector('(0,0,0)');
        mol.fitView(target, true);
        print([target.center.x,target.center.y,target.center.z].join(','));
    "#,
        )
        .unwrap();
    let mut lines = output.lines();
    let bounds: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(
        bounds,
        serde_json::json!({"min":[14.,25.,36.], "max":[14.,25.,36.]})
    );
    assert_eq!(
        lines.collect::<Vec<_>>(),
        [
            "true",
            "14,25,36,8,4",
            "77",
            "true",
            "14,25,36,8,4",
            "14,25,36"
        ]
    );
}
