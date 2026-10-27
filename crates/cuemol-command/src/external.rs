use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::DesktopAppState;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn resolve(state: &DesktopAppState, tool: &str) -> Result<PathBuf> {
    let key = match tool {
        "apbs" => "cuemol2.ui.apbs-exe-path",
        "pdb2pqr" => "cuemol2.ui.pdb2pqr-py-path",
        "povray" => "cuemol2.ui.render.pov-exe-path",
        "ffmpeg" => "cuemol2.ui.ffmpeg-path",
        _ => return Err(CueMolError::invalid_argument("unknown external tool")),
    };
    let configured = state.config_value(key);
    if !configured.trim().is_empty() {
        return Ok(PathBuf::from(configured));
    }
    let name = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.into()
    };
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("CUEMOL_TOOLS_DIR") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("cuemol-tools"));
            roots.push(dir.join("tools"));
            roots.push(dir.join("../Resources/cuemol-tools"));
            roots.push(dir.join("../Resources/tools"));
        }
    }
    roots.push(crate::config::cache_directory().join("tools"));
    for root in roots {
        for path in [
            root.join(&name),
            root.join(tool).join("bin").join(&name),
            root.join(tool).join(&name),
        ] {
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join(&name);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(CueMolError::not_found(format!(
        "{tool}; install the tools bundle or set {key} with cuemol config set"
    )))
}

pub fn run(
    executable: &Path,
    args: &[String],
    work: &Path,
    timeout_seconds: u64,
) -> Result<String> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(work)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_command(&mut command, executable);
    let mut child = command
        .spawn()
        .map_err(|e| CueMolError::Io(format!("cannot start {}: {e}", executable.display())))?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut stream = stream;
            let mut bytes = Vec::new();
            let mut block = [0u8; 8192];
            loop {
                match stream.read(&mut block) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let keep = n.min((1024 * 1024usize).saturating_sub(bytes.len()));
                        bytes.extend_from_slice(&block[..keep]);
                    }
                }
            }
            bytes
        })
    };
    let stdout = read(Box::new(child.stdout.take().unwrap()));
    let stderr = read(Box::new(child.stderr.take().unwrap()));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(timeout_seconds) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CueMolError::Io("external tool timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let mut output = stdout.join().unwrap_or_default();
    output.extend(stderr.join().unwrap_or_default());
    let text = String::from_utf8_lossy(&output).into_owned();
    if !status.success() {
        return Err(CueMolError::Io(format!(
            "{} exited with {status}: {text}",
            executable.display()
        )));
    }
    Ok(text)
}

pub fn configure_command(command: &mut Command, executable: &Path) {
    if let Some(root) = executable.parent().and_then(Path::parent) {
        let lib = root.join("lib");
        if lib.is_dir() {
            let key = if cfg!(target_os = "macos") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };
            let mut dirs = vec![lib];
            dirs.extend(std::env::split_paths(
                &std::env::var_os(key).unwrap_or_default(),
            ));
            if let Ok(paths) = std::env::join_paths(dirs) {
                command.env(key, paths);
            }
        }
    }
}
