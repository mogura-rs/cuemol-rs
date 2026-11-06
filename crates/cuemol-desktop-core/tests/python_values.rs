#![cfg(all(feature = "scripting", not(target_arch = "wasm32")))]

use cuemol_desktop_core::DesktopAppState;

fn run(app: &mut DesktopAppState, source: &str) -> String {
    app.run_script_js(&format!(
        "getService('PythonBridge').runString({});",
        serde_json::to_string(source).unwrap()
    ))
    .unwrap()
    .0
}

#[test]
fn python_command_results_are_dictionaries_with_live_scene_handles() {
    let mut app = DesktopAppState::new();
    app.ensure_active_scene();
    run(
        &mut app,
        "import cuemol as cm\n\
         result = cm.svc('CmdMgr').runCmdArgs('new_scene',\n\
             {'scene_name': 'python-dictionary', 'create_view': False, 'activate': False})\n\
         assert isinstance(result, dict)\n\
         assert sorted(result) == ['result_scene', 'result_view']\n\
         assert result['result_view'] is None\n\
         saved = result['result_scene']\n\
         assert cm.isscene(saved)\n\
         saved.setName('renamed')\n",
    );
    assert_eq!(run(&mut app, "print(saved.name)"), "renamed\n");
    assert!(app.scene_manager().scene_uids().iter().any(|uid| app
        .scene_manager()
        .scene(*uid)
        .is_some_and(|scene| scene.name() == "renamed")));
}

#[test]
fn python_nested_values_preserve_wrappers_and_reject_cycles() {
    let mut app = DesktopAppState::new();
    app.ensure_active_scene();
    let source = r#"
        cuemol.pythonValue = function() {
            var value = {type:'entry', entries:[{vector:cuemol.createObj('Vector','(1,2,3)')}, null], count:2};
            Object.defineProperty(value, 'hidden', {value:'private', enumerable:false});
            return value;
        };
        cuemol.pythonCycle = function() { var cycle = {}; cycle.self = cycle; return cycle; };
        getService('PythonBridge').runString($PYTHON);
    "#;
    let python = r#"
import cuemol as cm
value = cm.pythonValue()
assert isinstance(value, dict) and value['type'] == 'entry'
assert 'hidden' not in value and value['count'] == 2
assert isinstance(value['entries'], list) and value['entries'][1] is None
vector = value['entries'][0]['vector']
assert cm.iswrapper(vector) and vector.x == 1
vector.x = 7
assert vector.x == 7
try:
    cm.pythonCycle()
except RuntimeError as error:
    assert 'cyclic' in str(error)
else:
    raise AssertionError('cycle accepted')
assert cm.pythonValue()['count'] == 2
"#;
    app.run_script_js(&source.replace("$PYTHON", &serde_json::to_string(python).unwrap()))
        .unwrap();
}
