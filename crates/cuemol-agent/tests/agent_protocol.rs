use cuemol_agent::{secret, Event, History, Provider, Settings};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tiny_http::{Header, Response, Server};

fn sse(events: Vec<Value>) -> String {
    events
        .into_iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect()
}
fn openai(output: Vec<Value>, text: &str) -> String {
    sse(vec![
        json!({"type":"response.output_text.delta","delta":text}),
        json!({"type":"response.completed","response":{"status":"completed","output":output,"usage":{"input_tokens":12,"output_tokens":5,"input_tokens_details":{"cached_tokens":3}}}}),
    ])
}
fn events(provider: Provider, tools: bool) -> String {
    match provider {
        Provider::OpenAi if tools => openai(
            vec![
                json!({"type":"reasoning","id":"reason-1","encrypted_content":"opaque-openai-reasoning","summary":[]}),
                json!({"type":"function_call","id":"function-1","call_id":"call-1","name":"create_scene","arguments":"{\"name\":\"Agent scene\"}"}),
                json!({"type":"function_call","id":"function-2","call_id":"call-2","name":"capture_view","arguments":"{\"width\":32,\"height\":32}"}),
            ],
            "Preparing. ",
        ),
        Provider::OpenAi => openai(
            vec![
                json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done.","annotations":[]}]}),
            ],
            "Done.",
        ),
        Provider::Anthropic if tools => sse(vec![
            json!({"type":"message_start","message":{"usage":{"input_tokens":12,"cache_read_input_tokens":3}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private reasoning"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"opaque-anthropic-signature"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call-1","name":"create_scene","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"name\":\"Agent"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" scene\"}"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"call-2","name":"capture_view","input":{"width":32,"height":32}}}),
            json!({"type":"content_block_stop","index":2}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}),
            json!({"type":"message_stop"}),
        ]),
        Provider::Anthropic => sse(vec![
            json!({"type":"message_start","message":{"usage":{"input_tokens":12}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Done."}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}),
            json!({"type":"message_stop"}),
        ]),
        Provider::Google if tools => sse(vec![
            json!({"candidates":[{"content":{"role":"model","parts":[
            {"functionCall":{"name":"create_scene","args":{"name":"Agent scene"}},"thoughtSignature":"opaque-google-signature"},
            {"functionCall":{"name":"capture_view","args":{"width":32,"height":32}}}
        ]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":5}}),
        ]),
        Provider::Google => sse(vec![
            json!({"candidates":[{"content":{"role":"model","parts":[{"text":"Done."}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":5}}),
        ]),
    }
}
fn settings(provider: Provider, endpoint: String) -> Settings {
    Settings {
        enabled: true,
        model: format!("{}:test-model", provider.name()),
        reasoning: "default".into(),
        endpoint,
        ..Settings::default()
    }
}
fn next(job: &cuemol_agent::Job) -> Event {
    let start = Instant::now();
    loop {
        if let Some(event) = job.poll() {
            return event;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "agent did not respond"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn respond(mut request: tiny_http::Request, body: String) -> Value {
    let mut input = String::new();
    request.as_reader().read_to_string(&mut input).unwrap();
    request
        .respond(
            Response::from_string(body)
                .with_header(Header::from_bytes("Content-Type", "text/event-stream").unwrap()),
        )
        .unwrap();
    serde_json::from_str(&input).unwrap()
}

fn roundtrip(provider: Provider) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let header = match provider {
            Provider::OpenAi => "Authorization",
            Provider::Anthropic => "x-api-key",
            Provider::Google => "x-goog-api-key",
        };
        assert!(request
            .headers()
            .iter()
            .any(|h| h.field.equiv(header) && h.value.as_str().contains("test-key")));
        let first = respond(request, events(provider, true));
        assert!(first.to_string().contains("create_scene"));
        assert!(first.to_string().contains("<scene_state>"));
        let second = respond(
            server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            events(provider, false),
        );
        let wire = second.to_string();
        assert!(wire.contains("sceneId"));
        assert!(wire.contains("aW1hZ2U="));
        assert!(wire.contains(match provider {
            Provider::OpenAi => "opaque-openai-reasoning",
            Provider::Anthropic => "opaque-anthropic-signature",
            Provider::Google => "opaque-google-signature",
        }));
        if provider == Provider::OpenAi {
            assert_eq!(second["store"], false);
        }
    });
    let mut state = DesktopAppState::new();
    state.new_scene("Initial", true);
    let mut camera = Camera::new();
    let job = cuemol_agent::start(
        json!({}),
        settings(provider, endpoint),
        History::default(),
        secret("test-key".into()),
        "Make a scene and show it".into(),
        Box::new(|| {}),
    );
    let mut transcript = String::new();
    let mut images = 0;
    loop {
        match next(&job) {
            Event::Text(text) => transcript.push_str(&text),
            Event::Call {
                name,
                arguments,
                reply,
            } => {
                let value = if name == "capture_view" {
                    Ok(json!({"_image":"aW1hZ2U=","width":32,"height":32}))
                } else {
                    cuemol_command::call_tool(&mut state, &mut camera, &name, &arguments)
                        .map_err(|e| e.to_string())
                };
                reply.send_blocking(value).unwrap();
            }
            Event::ToolResult { image, .. } => {
                if image.is_some() {
                    images += 1;
                }
            }
            Event::Finished(result) => {
                let outcome = result.unwrap();
                assert_eq!(outcome.tool_calls, 2);
                assert_eq!(outcome.usage.input, 24);
                assert_eq!(outcome.usage.output, 10);
                assert!(!outcome.round_limit);
                break;
            }
            _ => {}
        }
    }
    assert!(transcript.ends_with("Done."));
    assert!(!transcript.contains("private reasoning"));
    assert_eq!(images, 1);
    assert_eq!(state.scene_summaries().len(), 2);
    worker.join().unwrap();
}
#[test]
fn openai_stream_executes_tools_and_replays_encrypted_reasoning() {
    roundtrip(Provider::OpenAi);
}
#[test]
fn anthropic_stream_reassembles_arguments_and_preserves_signatures() {
    roundtrip(Provider::Anthropic);
}
#[test]
fn google_stream_replays_thought_signatures_and_images() {
    roundtrip(Provider::Google);
}

#[test]
fn cancellation_invalidates_a_queued_tool_before_it_can_mutate() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        respond(server.recv().unwrap(), events(Provider::OpenAi, true));
    });
    let job = cuemol_agent::start(
        json!({}),
        settings(Provider::OpenAi, endpoint),
        History::default(),
        secret("test-key".into()),
        "Edit".into(),
        Box::new(|| {}),
    );
    loop {
        if let Event::Call { reply, .. } = next(&job) {
            job.cancel();
            loop {
                if let Event::Finished(result) = next(&job) {
                    assert_eq!(result.unwrap_err(), "Cancelled");
                    break;
                }
            }
            assert!(reply.is_closed());
            break;
        }
    }
    worker.join().unwrap();
}

#[test]
fn incomplete_stream_never_executes_a_partial_tool() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        respond(
            server.recv().unwrap(),
            sse(vec![
                json!({"type":"response.output_item.added","item":{"type":"function_call","name":"create_scene"}}),
            ]),
        );
    });
    let job = cuemol_agent::start(
        json!({}),
        settings(Provider::OpenAi, endpoint),
        History::default(),
        secret("test-key".into()),
        "Edit".into(),
        Box::new(|| {}),
    );
    match next(&job) {
        Event::Finished(result) => assert!(result.unwrap_err().contains("before completion")),
        _ => panic!("partial call was emitted"),
    }
    worker.join().unwrap();
}

#[test]
fn tool_errors_reach_model_and_rounds_are_bounded() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        for round in 0..16 {
            let body = respond(
                server
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .unwrap(),
                openai(
                    vec![
                        json!({"type":"function_call","call_id":format!("call-{round}"),"name":"missing_tool","arguments":"{}"}),
                    ],
                    "",
                ),
            );
            if round > 0 {
                assert!(body.to_string().contains("Unknown tool"));
            }
        }
    });
    let job = cuemol_agent::start(
        json!({}),
        settings(Provider::OpenAi, endpoint),
        History::default(),
        secret("test-key".into()),
        "Edit".into(),
        Box::new(|| {}),
    );
    loop {
        match next(&job) {
            Event::Call { reply, .. } => reply.send_blocking(Err("Unknown tool".into())).unwrap(),
            Event::Finished(result) => {
                let result = result.unwrap();
                assert_eq!(result.tool_calls, 16);
                assert!(result.round_limit);
                break;
            }
            _ => {}
        }
    }
    worker.join().unwrap();
}

#[test]
fn model_listing_follows_google_pages_and_filters_embedding_models() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let worker = std::thread::spawn(move || {
        let request = server.recv().unwrap();
        request.respond(Response::from_string(json!({"models":[{"name":"models/gemini-test","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"next-token"}).to_string())).unwrap();
        let request = server.recv().unwrap();
        assert!(request.url().contains("pageToken=next-token"));
        request.respond(Response::from_string(json!({"models":[{"name":"models/gemini-second","supportedGenerationMethods":["generateContent"]}]}).to_string())).unwrap();
    });
    let models = cuemol_agent::ModelJob::start(
        settings(Provider::Google, endpoint),
        secret("test-key".into()),
        Box::new(|| {}),
    )
    .wait()
    .unwrap();
    assert_eq!(models, ["google:gemini-second", "google:gemini-test"]);
    worker.join().unwrap();
}
