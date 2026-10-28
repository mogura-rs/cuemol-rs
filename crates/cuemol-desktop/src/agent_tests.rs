use super::*;
use serde_json::{json, Value};

fn tool(name: &str, args: Value, id: &str) -> Value {
    json!({"type":"function_call","call_id":id,"name":name,"arguments":args.to_string()})
}
fn mock(output: Value, finish: bool) -> (String, std::thread::JoinHandle<()>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        let outputs = if finish {
            vec![output, json!([])]
        } else {
            vec![output]
        };
        for output in outputs {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            let event = json!({"type":"response.completed","response":{"status":"completed","output":output}});
            request
                .respond(
                    tiny_http::Response::from_string(format!("data: {event}\n\n")).with_header(
                        tiny_http::Header::from_bytes("Content-Type", "text/event-stream").unwrap(),
                    ),
                )
                .unwrap();
        }
    });
    (endpoint, worker)
}
fn start(app: &mut GuiApplication, ctx: &egui::Context, endpoint: String) {
    app.agent.settings = cuemol_agent::Settings {
        enabled: true,
        model: "openai:test-model".into(),
        reasoning: "default".into(),
        endpoint,
        ..Default::default()
    };
    app.agent.set_session_key("mock-key".into());
    app.agent.start(
        &mut app.state,
        &mut CameraController::default().to_qsys_camera("__current"),
        "Update the molecule".into(),
        ctx,
        app.snapshot_revision,
    );
}
#[cfg(unix)]
fn apbs_app() -> (GuiApplication, tempfile::TempDir, cuemol_core::Uid) {
    use std::os::unix::fs::PermissionsExt;
    let mut app = GuiApplication::new();
    let scene = app.state.active_scene().unwrap();
    let molecule=cuemol_molstr::read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nATOM      2  CB  ALA A   1       2.000   3.000   4.000  1.00 10.00           C  \nEND\n").unwrap();
    let object = app
        .state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("source", molecule);
    cuemol_command::call_tool(
        &mut app.state,
        &mut cuemol_qsys::Camera::new(),
        "create_renderer",
        &json!({"objId":object.get(),"rendererType":"ballstick"}),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("apbs.sh");
    fs::write(&path,r#"#!/bin/sh
sleep 0.2
base=$(awk '/write pot dx/{print $4}' "$1")
printf 'object 1 class gridpositions counts 1 1 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 1 1 2\nobject 3 class array type double rank 0 items 2 data follows\n-1 1\n' > "${base}.dx"
"#).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    app.apbs_executable = path.to_string_lossy().into_owned();
    app.config_path = temp
        .path()
        .join("config.tsv")
        .to_string_lossy()
        .into_owned();
    app.rebuild_snapshot();
    (app, temp, object)
}
#[cfg(unix)]
#[test]
fn agent_apbs_runs_in_background_and_joins_the_turns_undo_transaction() {
    let (mut app, _temp, object) = apbs_app();
    let initial_renderers = app.state.renderer_summaries(object).len();
    let (endpoint, worker) = mock(
        json!([
            tool("enable_toolsets", json!({"toolsets":["molops"]}), "enable"),
            tool(
                "create_renderer",
                json!({"objId":object.get(),"rendererType":"cpk"}),
                "renderer"
            ),
            tool(
                "calc_elepot",
                json!({"objId":object.get(),"method":"internal","name":"agent potential"}),
                "apbs"
            )
        ]),
        true,
    );
    let ctx = egui::Context::default();
    let mut camera = CameraController::default();
    start(&mut app, &ctx, endpoint);
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut saw_background = false;
    while app.agent.running() || app.apbs_job.is_some() {
        assert!(Instant::now() < deadline, "{}", app.agent.status());
        app.agent_ui(&ctx, &mut camera);
        saw_background |= app.apbs_job.is_some();
        app.poll_apbs_calculation();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(saw_background);
    assert_eq!(
        app.state.active_density_map_summaries().len(),
        1,
        "{}",
        app.agent.status()
    );
    assert_eq!(
        app.state.renderer_summaries(object).len(),
        initial_renderers + 1
    );
    assert_eq!(app.undo_stack.len(), 1);
    app.undo(&mut camera);
    assert!(app.state.active_density_map_summaries().is_empty());
    assert_eq!(
        app.state.renderer_summaries(object).len(),
        initial_renderers
    );
    worker.join().unwrap();
}
#[cfg(unix)]
#[test]
fn external_edit_cancels_agent_apbs_without_importing_late_output() {
    let (mut app, _temp, object) = apbs_app();
    let (endpoint, worker) = mock(
        json!([
            tool("enable_toolsets", json!({"toolsets":["molops"]}), "enable"),
            tool(
                "calc_elepot",
                json!({"objId":object.get(),"method":"internal"}),
                "apbs"
            )
        ]),
        false,
    );
    let ctx = egui::Context::default();
    let mut camera = CameraController::default();
    start(&mut app, &ctx, endpoint);
    let deadline = Instant::now() + Duration::from_secs(6);
    while app.apbs_job.is_none() {
        assert!(app.agent.running(), "{}", app.agent.status());
        assert!(Instant::now() < deadline);
        app.agent_ui(&ctx, &mut camera);
        std::thread::sleep(Duration::from_millis(5));
    }
    app.state
        .set_object_common_properties(object, "Manual edit", true, false)
        .unwrap();
    app.rebuild_snapshot();
    app.agent_ui(&ctx, &mut camera);
    assert!(!app.agent.running());
    while app.apbs_job.is_some() {
        assert!(Instant::now() < deadline);
        app.poll_apbs_calculation();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.state.active_density_map_summaries().is_empty());
    assert_eq!(
        app.state.scene_manager().object(object).unwrap().name(),
        "Manual edit"
    );
    assert!(app.undo_stack.is_empty());
    worker.join().unwrap();
}
