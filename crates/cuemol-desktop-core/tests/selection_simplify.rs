#![cfg(feature = "scripting")]

use cuemol_desktop_core::DesktopAppState;

#[test]
fn selection_rewrites_are_callable_without_changing_the_source() {
    let mut state = DesktopAppState::new();
    state.ensure_active_scene();
    let (output, _) = state
        .run_script_js(
            r#"
        var s = cuemol.makeSel('not chain A or not chain B');
        print(s.invertSimplified());
        print(s.toString());
        s.compile('aid 1 or aid 2 or aid 3', 0);
        print(s.simplify());
        s.compile('chain A or bysidech chain B', 0);
        print(s.toggleSidechain());
        s.compile('byres resn ALA around 3', 0);
        print(s.rewriteAround(5, false));
        var empty = cuemol.SelCommand();
        print(empty.invertSimplified());
        var styles = cuemol.getService('StyleManager');
        var style = styles.createStyleSet('selection-probe', scene.uid);
        styles.setStrData('sel', 'focus', 'chain A', scene.uid, style);
        s.compile('focus or none', scene.uid);
        print(s.simplify());
        print(s.toString());
    "#,
        )
        .unwrap();
    assert_eq!(output.trim(), "not (not chain A or not chain B)\nnot chain A or not chain B\naid 1:3\nbysidech (chain A or (bysidech chain B))\n(resn ALA) around 5.0\n*\nfocus\nfocus or none");
}
