#![cfg(feature = "scripting")]

use cuemol_desktop_core::DesktopAppState;

const TARGET: &[u8] = b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N\nATOM      2  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      3  C   ALA A   1       2.000   0.000   0.000  1.00 10.00           C\nATOM      4  CA AALA A   1       1.000   0.100   0.000  0.50 10.00           C\nATOM      5  CA BALA A   1       1.000  -0.100   0.000  0.50 10.00           C\nEND\n";
const SOURCE: &[u8] = b"ATOM      1  N   ALA B   1       0.000   2.000   0.000  1.00 10.00           N\nATOM      2  CA  ALA B   1       2.000   2.000   0.000  1.00 10.00           C\nEND\n";

fn fixture() -> DesktopAppState {
    let mut app = DesktopAppState::new();
    let scene = app.new_scene("atoms", true).result_scene;
    for (name, bytes) in [("target", TARGET), ("source", SOURCE)] {
        let molecule = cuemol_molstr::read_pdb_str(std::str::from_utf8(bytes).unwrap()).unwrap();
        app.scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule(name, molecule);
    }
    app
}

#[test]
fn scripted_atom_history_retains_ids_alt_conformations_and_coordinates() {
    let mut app = fixture();
    let (log, _) = app.run_script_js(r#"
        var m=cuemol.getObject('target'), source=cuemol.getObject('source');
        scene.startUndoTxn('merge');m.copyAtoms(source,cuemol.makeSel('*'));scene.commitUndoTxn();
        print('merge='+m.getAtomSize()+':'+m.getAtomByID(6).chainName+':'+m.getAtomByID(7).name);
        scene.startUndoTxn('move');m.getAtomByID(7).pos=cuemol.Vector('(9,2,0)');scene.commitUndoTxn();
        scene.startUndoTxn('delete');m.deleteAtoms(cuemol.makeSel('aid 2'));scene.commitUndoTxn();
        print('delete='+m.getAtomSize()+':'+(m.getAtomByID(2)===null)+':'+(m.getAtomByID(4)===null)+':'+(m.getAtomByID(5)===null));
        scene.undo(0);print('restore='+m.getAtomSize()+':'+m.getAtomByID(7).pos.x);
        scene.undo(0);print('moveUndo='+m.getAtomByID(7).pos.x);
        scene.undo(0);print('mergeUndo='+m.getAtomSize());
        scene.redo(0);print('mergeRedo='+m.getAtomByID(6).chainName+':'+m.getAtomByID(7).pos.x);
        scene.redo(0);print('moveRedo='+m.getAtomByID(7).pos.x);
        scene.redo(0);print('deleteRedo='+m.getAtomSize());
        scene.undo(0);scene.undo(0);scene.undo(0);
        print('fresh='+m.appendAtom1(source.getAtomByID(1),'C',1,'ALA'));
    "#).unwrap();
    assert_eq!(log, "merge=7:B:CA\ndelete=4:true:true:true\nrestore=7:9\nmoveUndo=2\nmergeUndo=5\nmergeRedo=B:2\nmoveRedo=9\ndeleteRedo=4\nfresh=8\n");
}

#[test]
fn application_snapshots_do_not_reuse_atom_ids_after_undo() {
    let mut app = fixture();
    let before = app.clone();
    app.run_script_js(
        "cuemol.getObject('target').copyAtoms(cuemol.getObject('source'),cuemol.makeSel('*'))",
    )
    .unwrap();
    let after = app.replace_snapshot(before);
    let undone = app.replace_snapshot(after);
    app.restore_snapshot(undone);
    let (log, _) = app.run_script_js("print(cuemol.getObject('target').appendAtom1(cuemol.getObject('source').getAtomByID(1),'C',1,'ALA'))").unwrap();
    assert_eq!(log, "8\n");
}
