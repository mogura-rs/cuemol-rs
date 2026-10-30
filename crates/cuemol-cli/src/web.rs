use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, String>;
#[cfg(feature = "bundled-web")]
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
#[cfg(not(feature = "bundled-web"))]
static BUNDLED_WEB: &[(&str, &str, &[u8])] = &[];

pub fn run(args: Vec<String>) -> Result<()> {
    match args.first().map(String::as_str).unwrap_or("serve") {
        "build" => build(&args[1..]),
        "serve" => serve(args.get(1..).unwrap_or_default()),
        _ => Err("Usage: cuemol web build [--dev] [--out-dir DIR] | serve [--port PORT] [--bind IP] [--dir DIR] [--no-open]".into()),
    }
}
fn run_command(command: &mut Command) -> Result<()> {
    let status = command.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Command failed: {status}"))
    }
}
fn source_root() -> Result<PathBuf> {
    let mut root = std::env::current_dir().map_err(|e| e.to_string())?;
    loop {
        if root.join("crates/cuemol-web/Cargo.toml").is_file() && root.join("Cargo.lock").is_file()
        {
            return Ok(root);
        }
        if !root.pop() {
            return Err("Run cuemol web build from the source checkout.".into());
        }
    }
}
fn build(args: &[String]) -> Result<()> {
    let root = source_root()?;
    let mut profile = "web";
    let mut out = root.join("web");
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dev" => profile = "dev",
            "--out-dir" => out = PathBuf::from(rest.next().ok_or("missing output directory")?),
            _ => return Err(format!("Unknown build option: {arg}")),
        }
    }
    if !out.is_absolute() {
        out = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(out);
    }
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    let version = lock
        .split("[[package]]")
        .find(|section| {
            section
                .lines()
                .any(|line| line == "name = \"wasm-bindgen\"")
        })
        .and_then(|section| {
            section.lines().find_map(|line| {
                line.strip_prefix("version = \"")
                    .and_then(|v| v.strip_suffix('"'))
            })
        })
        .ok_or("wasm-bindgen version missing from Cargo.lock")?;
    let tools = root.join(".cache/web-tools");
    let executable = if cfg!(windows) {
        "wasm-bindgen.exe"
    } else {
        "wasm-bindgen"
    };
    let mut bindgen = std::env::var_os("CUEMOL_WASM_BINDGEN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(executable));
    let matches_version = |path: &Path| {
        Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .is_some_and(|out| {
                out.status.success()
                    && String::from_utf8_lossy(&out.stdout)
                        .split_whitespace()
                        .nth(1)
                        == Some(version)
            })
    };
    if !matches_version(&bindgen) {
        bindgen = tools.join("bin").join(executable);
        if !matches_version(&bindgen) {
            eprintln!("Installing wasm-bindgen-cli {version} into the checkout cache…");
            run_command(
                Command::new("cargo")
                    .args(["install", "--locked", "--version", version, "--root"])
                    .arg(&tools)
                    .arg("wasm-bindgen-cli"),
            )?;
        }
    }
    run_command(Command::new("rustup").args(["target", "add", "wasm32-unknown-unknown"]))?;
    run_command(Command::new("cargo").current_dir(&root).args([
        "build",
        "--locked",
        "-p",
        "cuemol-web",
        "-p",
        "cuemol-web-loader",
        "--target",
        "wasm32-unknown-unknown",
        "--profile",
        profile,
    ]))?;
    let metadata = Command::new("cargo")
        .current_dir(&root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|e| e.to_string())?;
    if !metadata.status.success() {
        return Err("cargo metadata failed".into());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).map_err(|e| e.to_string())?;
    let target = Path::new(
        metadata["target_directory"]
            .as_str()
            .ok_or("missing Cargo target directory")?,
    )
    .join("wasm32-unknown-unknown")
    .join(if profile == "dev" { "debug" } else { profile });
    std::fs::create_dir_all(out.join("pkg")).map_err(|e| e.to_string())?;
    for name in ["cuemol_web", "cuemol_web_loader"] {
        run_command(
            Command::new(&bindgen)
                .arg(target.join(format!("{name}.wasm")))
                .args(["--target", "web", "--no-typescript", "--out-dir"])
                .arg(out.join("pkg")),
        )?;
    }
    run_command(
        Command::new("uv")
            .current_dir(&root)
            .args([
                "run",
                "--no-project",
                "python",
                "scripts/build-web-kernels.py",
                "--out-dir",
            ])
            .arg(out.join("pkg/kernels")),
    )?;
    run_command(
        Command::new("uv")
            .current_dir(&root)
            .args([
                "run",
                "--no-project",
                "python",
                "scripts/bundle-oidn.py",
                "--web",
                "--out",
            ])
            .arg(out.join("pkg/oidn")),
    )?;
    run_command(
        Command::new("uv")
            .current_dir(&root)
            .args([
                "run",
                "--no-project",
                "scripts/bundle-ui-fonts.py",
                "--out-dir",
            ])
            .arg(out.join("pkg/fonts")),
    )?;
    if out != root.join("web") {
        for name in [
            "index.html",
            "kernels.js",
            "oidn.js",
            "input.js",
            "loader.js",
            "loader-worker.js",
        ] {
            std::fs::copy(root.join("web").join(name), out.join(name))
                .map_err(|e| e.to_string())?;
        }
    }
    std::fs::copy(root.join("cuemol-rs.png"), out.join("pkg/logo.png"))
        .map_err(|e| e.to_string())?;
    let licenses = out.join("pkg/licenses");
    std::fs::create_dir_all(&licenses).map_err(|e| e.to_string())?;
    for name in ["smaa.txt", "molstar.txt", "xegtao.txt", "phosphor.txt"] {
        std::fs::copy(root.join("licenses").join(name), licenses.join(name))
            .map_err(|e| e.to_string())?;
    }
    println!("Built {}", out.display());
    Ok(())
}
fn mime(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}
fn serve(args: &[String]) -> Result<()> {
    let mut port = 8080_u16;
    let mut address = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut directory = None;
    let mut open = true;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--port" => {
                port = rest
                    .next()
                    .ok_or("missing port")?
                    .parse()
                    .map_err(|_| "invalid port")?
            }
            "--bind" => {
                address = rest
                    .next()
                    .ok_or("missing IP")?
                    .parse()
                    .map_err(|_| "invalid IP")?
            }
            "--dir" => {
                directory = Some(
                    std::fs::canonicalize(rest.next().ok_or("missing directory")?)
                        .map_err(|e| e.to_string())?,
                )
            }
            "--no-open" => open = false,
            _ => return Err(format!("Unknown server option: {arg}")),
        }
    }
    if directory.is_none() && BUNDLED_WEB.is_empty() {
        let local = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join("web");
        if local.join("pkg/cuemol_web_bg.wasm").is_file() {
            directory = Some(local);
        } else {
            return Err("This binary has no bundled web assets. Run cuemol web build, then cuemol web serve --dir web.".into());
        }
    }
    let server = tiny_http::Server::http((address, port)).map_err(|e| e.to_string())?;
    let url = format!("http://{}/", server.server_addr());
    println!("cuemol-rs: {url}");
    if open {
        let mut command = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", ""]);
            c
        } else {
            Command::new("xdg-open")
        };
        let _ = command
            .arg(&url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let server = &server;
            let directory = directory.as_deref();
            scope.spawn(move || {
                for request in server.incoming_requests() {
                    serve_request(request, directory);
                }
            });
        }
    });
    Ok(())
}

fn serve_request(request: tiny_http::Request, directory: Option<&Path>) {
    if !matches!(
        request.method(),
        tiny_http::Method::Get | tiny_http::Method::Head
    ) {
        let _ = request.respond(tiny_http::Response::empty(405));
        return;
    }
    let route = request
        .url()
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/');
    let name = if route.is_empty() {
        "index.html"
    } else {
        route
    };
    if !Path::new(name)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
        || name.contains('%')
        || name.contains('\\')
    {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    }
    let wants_gzip = request.headers().iter().any(|h| {
        h.field.equiv("Accept-Encoding") && h.value.as_str().split(',').any(|v| v.trim() == "gzip")
    });
    let asset = if let Some(dir) = directory {
        let path = dir.join(name);
        std::fs::canonicalize(path)
            .ok()
            .filter(|p| p.starts_with(dir) && p.is_file())
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| (mime(name), bytes, false))
    } else {
        BUNDLED_WEB
            .iter()
            .find(|(path, _, _)| *path == name)
            .and_then(|(_, mime, compressed)| {
                if wants_gzip {
                    Some((*mime, compressed.to_vec(), true))
                } else {
                    let mut bytes = Vec::new();
                    flate2::read::GzDecoder::new(*compressed)
                        .read_to_end(&mut bytes)
                        .ok()
                        .map(|_| (*mime, bytes, false))
                }
            })
    };
    let Some((mime, bytes, gzip)) = asset else {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    };
    let mut response = tiny_http::Response::from_data(bytes);
    for (key, value) in [
        ("Content-Type", mime),
        ("Cache-Control", "no-cache"),
        ("X-Content-Type-Options", "nosniff"),
        ("Vary", "Accept-Encoding"),
    ] {
        response.add_header(tiny_http::Header::from_bytes(key, value).unwrap());
    }
    if gzip {
        response.add_header(tiny_http::Header::from_bytes("Content-Encoding", "gzip").unwrap());
    }
    let _ = request.respond(response);
}
