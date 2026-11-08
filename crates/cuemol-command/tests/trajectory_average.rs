use cuemol_command::call_tool;
use cuemol_core::Vector4;
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath};
use cuemol_qsys::Camera;
use serde_json::json;

const PDB: &[u8] =
    b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n";

#[test]
fn explicit_frame_average_clips_the_window_without_changing_playback_settings() {
    let mut state = DesktopAppState::new();
    let GuiLoadedPath::Object { object_uid, .. } = state.open_bytes("one.pdb", PDB).unwrap() else {
        panic!("molecule expected");
    };
    state.set_object_trajectory(
        object_uid,
        [-0., 1., 5.]
            .into_iter()
            .map(|x| vec![Vector4::new3(x, 0., 0.)])
            .collect(),
    );
    let mut camera = Camera::new();
    for (frame, radius, expected) in [(0, 1, 0.5_f64), (1, 1, 2.), (2, 8, 2.), (0, 0, -0.)] {
        call_tool(
            &mut state,
            &mut camera,
            "set_frame",
            &json!({"objId":object_uid.get(),"frame":frame,"averageRadius":radius}),
        )
        .unwrap();
        let x = state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .mol()
            .atoms()[0]
            .position
            .x;
        assert_eq!(x.to_bits(), expected.to_bits());
        assert_eq!(state.trajectory_frame_state(object_uid), Some((frame, 3)));
    }
    call_tool(
        &mut state,
        &mut camera,
        "set_frame",
        &json!({"objId":object_uid.get(),"frame":2}),
    )
    .unwrap();
    assert_eq!(
        state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .mol()
            .atoms()[0]
            .position
            .x,
        5.
    );
    let before = state
        .scene_manager()
        .object(object_uid)
        .unwrap()
        .mol()
        .clone();
    for args in [
        json!({"objId":object_uid.get(),"frame":1,"averageRadius":-1}),
        json!({"objId":object_uid.get(),"frame":3,"averageRadius":1}),
    ] {
        assert!(call_tool(&mut state, &mut camera, "set_frame", &args).is_err());
        assert_eq!(
            state.scene_manager().object(object_uid).unwrap().mol(),
            &before
        );
        assert_eq!(state.trajectory_frame_state(object_uid), Some((2, 3)));
    }
}

#[test]
fn a_bad_neighbor_cannot_partially_apply_an_average() {
    let mut state = DesktopAppState::new();
    let GuiLoadedPath::Object { object_uid, .. } = state.open_bytes("one.pdb", PDB).unwrap() else {
        panic!("molecule expected");
    };
    state.set_object_trajectory(
        object_uid,
        vec![vec![Vector4::new3(7., 0., 0.)], Vec::new()],
    );
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "set_frame",
        &json!({"objId":object_uid.get(),"frame":0}),
    )
    .unwrap();
    let before = state
        .scene_manager()
        .object(object_uid)
        .unwrap()
        .mol()
        .clone();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_frame",
        &json!({"objId":object_uid.get(),"frame":0,"averageRadius":1})
    )
    .is_err());
    assert_eq!(
        state.scene_manager().object(object_uid).unwrap().mol(),
        &before
    );
    assert_eq!(state.trajectory_frame_state(object_uid), Some((0, 2)));
}
