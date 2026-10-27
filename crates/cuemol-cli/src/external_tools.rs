use cuemol_command::{config, external};
use cuemol_desktop_core::DesktopAppState;
use serde_json::json;
use std::process::Command;

type Result<T> = std::result::Result<T, String>;
pub fn run(args: Vec<String>) -> Result<()> {
    let mut state = DesktopAppState::new();
    config::load_preferences(&mut state).map_err(|e| e.to_string())?;
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            let mut tools = vec![
                json!({"name":"umbreon","builtIn":true}),
                json!({"name":"usalign","builtIn":true}),
            ];
            for name in ["apbs", "pdb2pqr", "ffmpeg", "povray"] {
                let path = external::resolve(&state, name).ok();
                tools.push(
                    json!({"name":name,"builtIn":false,"available":path.is_some(),"path":path}),
                );
            }
            println!("{}", serde_json::to_string_pretty(&tools).unwrap());
            Ok(())
        }
        "path" if args.len() == 2 => {
            println!(
                "{}",
                external::resolve(&state, &args[1])
                    .map_err(|e| e.to_string())?
                    .display()
            );
            Ok(())
        }
        "run" if args.len() >= 2 => {
            let executable = external::resolve(&state, &args[1]).map_err(|e| e.to_string())?;
            let mut command = Command::new(&executable);
            command.args(&args[2..]);
            external::configure_command(&mut command, &executable);
            let status = command.status().map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("{} exited with {status}", args[1]))
            }
        }
        "bundle" => {
            let mut root = std::env::current_dir().map_err(|e| e.to_string())?;
            while !root.join("scripts/bundle-tools.py").is_file() {
                if !root.pop() {
                    return Err("Run cuemol tools bundle from the source checkout.".into());
                }
            }
            let status = Command::new("uv")
                .current_dir(root)
                .args(["run", "--no-project", "python", "scripts/bundle-tools.py"])
                .args(&args[1..])
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("Tools bundle failed: {status}"))
            }
        }
        _ => Err(
            "Usage: cuemol tools list | path NAME | run NAME [ARG ...] | bundle [--out DIR]".into(),
        ),
    }
}
