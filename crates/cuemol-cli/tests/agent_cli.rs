use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn agent_cli_uses_stdin_key_edits_and_saves_scene() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.pdb");
    let output = temp.path().join("result.qsc");
    std::fs::write(
        &input,
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
    )
    .unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        for round in 0..2 {
            let mut request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            assert!(request.headers().iter().any(
                |h| h.field.equiv("Authorization") && h.value.as_str() == "Bearer mock-cli-key"
            ));
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).unwrap();
            let body: Value = serde_json::from_str(&text).unwrap();
            let output = if round == 0 {
                let context = body["input"][0]["content"].as_str().unwrap();
                let scene: Value = serde_json::from_str(
                    context
                        .strip_prefix("<scene_state>")
                        .unwrap()
                        .split("</scene_state>")
                        .next()
                        .unwrap(),
                )
                .unwrap();
                json!([{"type":"function_call","call_id":"rename","name":"set_node_prop","arguments":json!({"nodeType":"object","nodeId":scene["objects"][0]["id"],"prop":"name","value":"Agent result"}).to_string()}])
            } else {
                json!([])
            };
            let delta = json!({"type":"response.output_text.delta","delta":if round==1 {"Renamed the molecule."}else{""}});
            let event = json!({"type":"response.completed","response":{"status":"completed","output":output}});
            request
                .respond(
                    tiny_http::Response::from_string(format!("data: {delta}\n\ndata: {event}\n\n"))
                        .with_header(
                            tiny_http::Header::from_bytes("Content-Type", "text/event-stream")
                                .unwrap(),
                        ),
                )
                .unwrap();
        }
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args(["agent", "run"])
        .arg(&input)
        .args([
            "--prompt",
            "Rename the molecule",
            "--model",
            "openai:test-model",
            "--reasoning",
            "default",
            "--endpoint",
            &endpoint,
            "--api-key-stdin",
            "--save",
        ])
        .arg(&output)
        .env("CUEMOL_CONFIG_DIR", temp.path().join("config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"mock-cli-key\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stderr}");
    assert!(String::from_utf8_lossy(&result.stdout).contains("Renamed the molecule."));
    assert!(!stderr.contains("mock-cli-key"));
    assert!(std::fs::read_to_string(output)
        .unwrap()
        .contains("Agent result"));
    assert!(!temp.path().join("config/agent.json").exists());
    worker.join().unwrap();
}
