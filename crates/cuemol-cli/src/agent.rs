use cuemol_agent::{credentials, secret, Event, History, ModelJob, Provider, Settings};
use cuemol_command::{call_tool, config};
use cuemol_desktop_core::DesktopAppState;
use serde_json::json;
use std::io::{Read, Write};

pub fn run(args: Vec<String>) -> Result<(), String> {
    let action = args
        .first()
        .map(String::as_str)
        .unwrap_or("help")
        .to_string();
    if matches!(action.as_str(), "help" | "--help" | "-h") {
        println!("cuemol agent run [FILE] --prompt TEXT [--model PROVIDER:MODEL] [--reasoning default|low|medium|high] [--save SCENE.qsc]\ncuemol agent models [--model PROVIDER:MODEL]\ncuemol agent key set|delete PROVIDER\n\nKeys are read from the OS keychain or provider environment variable. The key set command reads the key from standard input. Optional --endpoint URL selects a compatible API server. Use --api-key-stdin to supply a session-only key without accessing the OS keychain.");
        return Ok(());
    }
    if action == "key" {
        let command = args.get(1).ok_or("Missing key command")?;
        let provider: Provider = args.get(2).ok_or("Missing provider")?.parse()?;
        if args.len() != 3 {
            return Err("Usage: cuemol agent key set|delete PROVIDER".into());
        }
        return match command.as_str() {
            "set" => {
                let mut key = secret(String::new());
                std::io::stdin()
                    .take(16 * 1024)
                    .read_to_string(&mut key)
                    .map_err(|e| e.to_string())?;
                credentials::save(provider, key.trim())
            }
            "delete" => credentials::delete(provider),
            _ => Err("Use key set or key delete.".into()),
        };
    }
    if !matches!(action.as_str(), "run" | "models") {
        return Err(format!("Unknown agent command: {action}"));
    }
    let mut settings: Settings = std::fs::read_to_string(config::directory().join("agent.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut args = args.into_iter().skip(1);
    let mut file = None;
    let mut save = None;
    let mut prompt = None;
    let mut key_stdin = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--api-key-stdin" => key_stdin = true,
            "--model" => settings.model = args.next().ok_or("Missing model")?,
            "--reasoning" => settings.reasoning = args.next().ok_or("Missing reasoning effort")?,
            "--endpoint" => settings.endpoint = args.next().ok_or("Missing endpoint")?,
            "--prompt" => prompt = Some(args.next().ok_or("Missing prompt")?),
            "--save" => save = Some(args.next().ok_or("Missing output scene")?),
            _ if !arg.starts_with('-') && file.is_none() => file = Some(arg),
            _ => return Err(format!("Unexpected argument: {arg}")),
        }
    }
    let (provider, _) = settings.model_spec()?;
    if action == "run" && prompt.as_ref().is_none_or(|s| s.trim().is_empty()) {
        return Err("Use --prompt TEXT to run an agent turn.".into());
    }
    let key = if key_stdin {
        let mut key = secret(String::new());
        std::io::stdin()
            .take(16 * 1024 + 1)
            .read_to_string(&mut key)
            .map_err(|e| e.to_string())?;
        if key.len() > 16 * 1024 {
            return Err("API key is too long".into());
        }
        secret(key.trim().to_string())
    } else {
        credentials::load(provider)?
    };
    if action == "models" {
        for model in ModelJob::start(settings, key, Box::new(|| {})).wait()? {
            println!("{model}");
        }
        return Ok(());
    }
    let prompt = prompt
        .filter(|s| !s.trim().is_empty())
        .ok_or("Use --prompt TEXT to run an agent turn.")?;
    let mut state = DesktopAppState::new();
    state.new_scene("Untitled", true);
    config::load(&mut state).map_err(|e| e.to_string())?;
    let mut camera = state.active_view_camera().unwrap_or_default();
    if let Some(file) = file {
        call_tool(&mut state, &mut camera, "load_file", &json!({"path":file}))
            .map_err(|e| e.to_string())?;
    }
    let scene = call_tool(&mut state, &mut camera, "get_scene_state", &json!({}))
        .map_err(|e| e.to_string())?;
    settings.enabled = true;
    let job = cuemol_agent::start(
        scene,
        settings,
        History::default(),
        key,
        prompt,
        Box::new(|| {}),
    );
    let result = loop {
        match job.receive()? {
            Event::Text(text) => {
                print!("{text}");
                std::io::stdout().flush().map_err(|e| e.to_string())?;
            }
            Event::ToolStarted { name, .. } => eprintln!("Running {name}"),
            Event::ToolResult {
                name, summary, ok, ..
            } => {
                if !ok {
                    eprintln!("{name}: {summary}");
                }
            }
            Event::Call {
                name,
                arguments,
                reply,
            } => {
                let result = call_tool(&mut state, &mut camera, &name, &arguments)
                    .map_err(|e| e.to_string());
                let _ = reply.send_blocking(result);
            }
            Event::Finished(result) => break result,
        }
    };
    println!();
    if let Some(save) = save {
        state
            .set_active_view_camera(&camera)
            .map_err(|e| e.to_string())?;
        state
            .save_active_scene_qsc(save)
            .map_err(|e| e.to_string())?;
    }
    let outcome = result?;
    eprintln!(
        "{} tool calls, {} input / {} output tokens",
        outcome.tool_calls, outcome.usage.input, outcome.usage.output
    );
    if outcome.round_limit {
        return Err("Round limit reached; completed edits were kept.".into());
    }
    Ok(())
}
