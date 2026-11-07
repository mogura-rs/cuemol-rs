//! Build and runtime details for the native and browser About dialogs.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const PRODUCT: &str = "CueMol3";
pub type Rows = Vec<(String, String)>;

#[derive(Clone, Copy)]
pub enum Frontend {
    Egui,
    Tauri,
    Browser,
}

#[derive(Deserialize)]
struct BuildInfo {
    version: String,
    commit: String,
    date: String,
    compiler: String,
    target: String,
    dependencies: BTreeMap<String, Vec<String>>,
}

pub fn rows(frontend: Frontend, runtime: &str) -> Rows {
    static BUILD: OnceLock<BuildInfo> = OnceLock::new();
    let build = BUILD.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/about-build.json")))
            .expect("valid build metadata")
    });
    let mut rows = vec![
        ("Version".into(), build.version.clone()),
        ("Commit".into(), build.commit.clone()),
        ("Date".into(), build.date.clone()),
        ("Target".into(), build.target.clone()),
        ("Rust".into(), build.compiler.clone()),
        (
            "Frontend".into(),
            match frontend {
                Frontend::Egui => "egui",
                Frontend::Tauri => "Tauri",
                Frontend::Browser => "Browser (WASM)",
            }
            .into(),
        ),
    ];
    let mut dependencies = vec![("wgpu", "wgpu")];
    dependencies.push(match frontend {
        Frontend::Tauri => ("Tauri", "tauri"),
        _ => ("egui", "egui"),
    });
    if cfg!(feature = "scripting") {
        dependencies.push(("Boa JavaScript", "boa_engine"));
    }
    for (label, name) in dependencies {
        if let Some(versions) = build.dependencies.get(name) {
            rows.push((label.into(), versions.join(", ")));
        }
    }
    rows.push((
        if matches!(frontend, Frontend::Browser) {
            "Browser / OS"
        } else {
            "OS"
        }
        .into(),
        runtime.into(),
    ));
    rows
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_rows(frontend: Frontend) -> Rows {
    static OS: OnceLock<String> = OnceLock::new();
    let os = OS.get_or_init(|| {
        #[cfg(unix)]
        let output = std::process::Command::new("uname")
            .args(["-s", "-r", "-m"])
            .output();
        #[cfg(windows)]
        let output = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output();
        #[cfg(not(any(unix, windows)))]
        let output: std::io::Result<std::process::Output> =
            Err(std::io::ErrorKind::Unsupported.into());
        output
            .ok()
            .filter(|value| value.status.success())
            .map(|value| String::from_utf8_lossy(&value.stdout).trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{} {}", std::env::consts::OS, std::env::consts::ARCH))
    });
    rows(frontend, os)
}

pub fn text(rows: &[(String, String)]) -> String {
    std::iter::once(PRODUCT.to_owned())
        .chain(
            rows.iter()
                .map(|(label, value)| format!("{label}: {value}")),
        )
        .collect::<Vec<_>>()
        .join("\n")
}
