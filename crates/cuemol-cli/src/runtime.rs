use cuemol_command::{call_tool, config as configuration, transport};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, String>;

pub(crate) fn state() -> Result<(DesktopAppState, Camera)> {
    let mut state = DesktopAppState::new();
    state.new_scene("Untitled", true);
    configuration::load(&mut state).map_err(|e| e.to_string())?;
    let camera = state.active_view_camera().unwrap_or_default();
    Ok((state, camera))
}

pub fn mcp(args: Vec<String>) -> Result<()> {
    let action = args.first().map(String::as_str).unwrap_or("connect");
    match action {
        "serve" | "stdio" => {
            if args.len() == 1 {
                return transport::serve_stdio().map_err(|e| e.to_string());
            }
            if args.len() == 3 && args[1] == "--http" {
                let address = args[2]
                    .parse()
                    .map_err(|_| "expected a loopback IP:port".to_string())?;
                return transport::serve_http(address).map_err(|e| e.to_string());
            }
            Err("Usage: cuemol mcp serve [--http 127.0.0.1:PORT]".into())
        }
        "connect" => {
            let path = match &args[1..] {
                [] => std::env::var_os("CUEMOL_LOCAL_API_INFO")
                    .map(PathBuf::from)
                    .unwrap_or_else(transport::info_path),
                [flag, path] if flag == "--info" => PathBuf::from(path),
                [flag] if flag == "--upstream" => std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
                    .ok_or("home directory unavailable")?
                    .join(".cuemol/local-api.json"),
                _ => return Err("Usage: cuemol mcp connect [--info FILE | --upstream]".into()),
            };
            transport::connect_stdio(&path).map_err(|e| e.to_string())
        }
        "config" => {
            let target = if args.get(1).is_some_and(|arg| arg == "--headless") {
                "serve"
            } else {
                "connect"
            };
            println!(
                "{}",
                json!({"mcpServers":{"cuemol":{"command":"cuemol","args":["mcp",target]}}})
            );
            Ok(())
        }
        "status" => {
            let info = transport::read_info(&transport::info_path()).map_err(|e| e.to_string())?;
            let answer = transport::post(&info, &json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
                .map_err(|e| e.to_string())?;
            if answer
                .as_ref()
                .is_none_or(|answer| answer.get("result").is_none())
            {
                return Err("MCP endpoint did not answer ping".into());
            }
            println!(
                "{}",
                json!({"port":info.port,"pid":info.pid,"endpoints":info.endpoints,"connected":true})
            );
            Ok(())
        }
        "tools" => {
            println!("{}", json!({"tools":cuemol_command::tools_list()}));
            Ok(())
        }
        "call" if args.len() == 3 => {
            let input: Value = serde_json::from_str(&args[2]).map_err(|e| e.to_string())?;
            let info = transport::read_info(&transport::info_path()).map_err(|e| e.to_string())?;
            transport::post(&info,&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"cuemol-cli","version":env!("CARGO_PKG_VERSION")}}})).map_err(|e|e.to_string())?;
            let response=transport::post(&info,&json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":args[1],"arguments":input}})).map_err(|e|e.to_string())?.ok_or("missing MCP response")?;
            println!("{response}");
            if response["result"]["isError"] == true || !response["error"].is_null() {
                Err("MCP tool failed".into())
            } else {
                Ok(())
            }
        }
        "--help" | "-h" | "help" => {
            println!("cuemol mcp connect [--upstream | --info FILE]\ncuemol mcp serve [--http 127.0.0.1:PORT]\ncuemol mcp config [--headless]\ncuemol mcp status\ncuemol mcp tools\ncuemol mcp call TOOL JSON");
            Ok(())
        }
        _ => Err("Unknown MCP command; use cuemol mcp --help".into()),
    }
}

pub fn config(args: Vec<String>) -> Result<()> {
    match args.as_slice() {
        [] | [_] if args.is_empty() || args[0] == "path" => {
            println!("{}", configuration::directory().display());
            Ok(())
        }
        [cmd, key] if cmd == "get" => {
            let mut state = DesktopAppState::new();
            configuration::load_preferences(&mut state).map_err(|e| e.to_string())?;
            println!("{}", state.config_value(key));
            Ok(())
        }
        [cmd, key, value] if cmd == "set" => {
            let mut state = DesktopAppState::new();
            configuration::load_preferences(&mut state).map_err(|e| e.to_string())?;
            state.put_config_value(key, value);
            let path = configuration::file_path();
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            state.save_config_file(path).map_err(|e| e.to_string())
        }
        _ => Err("Usage: cuemol config path | get KEY | set KEY VALUE".into()),
    }
}

pub fn script(command: &str, args: Vec<String>) -> Result<()> {
    if args.len() != 1 {
        return Err("Usage: cuemol run SCRIPT.js | cuemol -c JAVASCRIPT".into());
    }
    let source = if command == "run" {
        std::fs::read_to_string(&args[0]).map_err(|e| e.to_string())?
    } else {
        args[0].clone()
    };
    let (mut state, mut camera) = state()?;
    let result = call_tool(
        &mut state,
        &mut camera,
        "execute_script",
        &json!({"source":source}),
    )
    .map_err(|e| e.to_string())?;
    if let Some(log) = result["log"].as_str() {
        print!("{log}");
    }
    Ok(())
}

pub fn execute(args: Vec<String>) -> Result<()> {
    if args.is_empty() || args.len() > 2 {
        return Err("Usage: cuemol exec TOOL [JSON]".into());
    }
    let input = serde_json::from_str(args.get(1).map(String::as_str).unwrap_or("{}"))
        .map_err(|e| e.to_string())?;
    let (mut state, mut camera) = state()?;
    let result = call_tool(&mut state, &mut camera, &args[0], &input).map_err(|e| e.to_string())?;
    println!("{result}");
    Ok(())
}

pub fn render(args: Vec<String>) -> Result<()> {
    if args.len() < 2 {
        return Err(
            "Usage: cuemol render INPUT OUTPUT [--width PX --height PX --style gi|npr]".into(),
        );
    }
    let mut options = json!({"fileName":args[1]});
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--width" => {
                options["width"] = json!(value.parse::<u32>().map_err(|_| "invalid width")?)
            }
            "--height" => {
                options["height"] = json!(value.parse::<u32>().map_err(|_| "invalid height")?)
            }
            "--style" => options["style"] = json!(value),
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    let (mut state, mut camera) = state()?;
    call_tool(
        &mut state,
        &mut camera,
        "load_file",
        &json!({"path":args[0]}),
    )
    .map_err(|e| e.to_string())?;
    let result =
        call_tool(&mut state, &mut camera, "render_image", &options).map_err(|e| e.to_string())?;
    println!("{result}");
    Ok(())
}

pub fn trajectory_info(args: Vec<String>) -> Result<()> {
    let usage = "Usage: cuemol trajectory-info FILE [STRIDE] [--atoms N] [--box | --no-box]";
    let path = args.first().ok_or(usage)?;
    let mut stride = 1;
    let mut atoms = None;
    let mut boxed = None;
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--atoms" => {
                atoms = Some(
                    args.next()
                        .ok_or(usage)?
                        .parse()
                        .map_err(|_| "invalid atom count")?,
                )
            }
            "--box" => boxed = Some(true),
            "--no-box" => boxed = Some(false),
            _ => stride = arg.parse().map_err(|_| usage)?,
        }
    }
    let file = cuemol_mdtools::IndexedTrajectory::open_with_amber_box(path, stride, atoms, boxed)
        .map_err(|e| e.to_string())?;
    let first = file.read_frame(0).map_err(|e| e.to_string())?;
    println!(
        "{}",
        json!({"frames":file.frame_count(),"atoms":file.atom_count(),"indexBytes":file.index_bytes(),"cell":first.cell,"first":first.coordinates.get(..3)})
    );
    Ok(())
}
