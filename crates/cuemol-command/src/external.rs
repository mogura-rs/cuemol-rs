use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::DesktopAppState;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
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

/// Resolve a default tool name while retaining explicit user paths.
pub fn resolve_default(state: &DesktopAppState, tool: &str, value: &str) -> String {
    if value.trim().is_empty() || value.trim() == tool || value.trim() == format!("{tool}.exe") {
        if let Ok(path) = resolve(state, tool) {
            return path.to_string_lossy().into_owned();
        }
    }
    value.to_string()
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
    let mut child = spawn_command(&mut command)
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

/// Preserve paths for POV-Ray's own command-line parser.
pub fn povray_path_arg(option: &str, path: &str) -> String {
    if path
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, ';' | '#' | '[' | ']' | '=' | '\'' | '"'))
    {
        let quote = if path.contains('"') { '\'' } else { '"' };
        format!("{option}{quote}{path}{quote}")
    } else {
        format!("{option}{path}")
    }
}

pub fn configure_command(command: &mut Command, executable: &Path) {
    if let Some(root) = executable.parent().and_then(Path::parent) {
        let ini = root.join("cuemol.ini");
        let include = root.join("include");
        if ini.is_file() && include.join("colors.inc").is_file() {
            command.env("POVINI", ini);
            command.arg(povray_path_arg("+L", &include.to_string_lossy()));
        }
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

/// Retry transient executable-write locks, keeping all other spawn errors unchanged.
pub fn spawn_command(command: &mut Command) -> std::io::Result<Child> {
    #[cfg(unix)]
    for delay in [5, 10, 20, 40, 80] {
        match command.spawn() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(Duration::from_millis(delay));
            }
            result => return result,
        }
    }
    command.spawn()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn executable_write_lock_can_clear_before_spawn() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("tool");
        let mut writer = std::fs::File::create(&executable).unwrap();
        writer.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        writer
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let mut command = Command::new(&executable);
        assert_eq!(command.spawn().unwrap_err().raw_os_error(), Some(26));
        let close = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            drop(writer);
        });
        let status = spawn_command(&mut command).unwrap().wait().unwrap();
        close.join().unwrap();
        assert!(status.success());
    }

    #[test]
    fn persistent_write_lock_and_missing_executable_remain_errors() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("tool");
        let mut writer = std::fs::File::create(&executable).unwrap();
        writer.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        writer
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .unwrap();
        assert_eq!(
            spawn_command(&mut Command::new(&executable))
                .unwrap_err()
                .raw_os_error(),
            Some(26)
        );
        drop(writer);
        assert_eq!(
            spawn_command(&mut Command::new(directory.path().join("missing")))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    #[test]
    fn default_tool_names_use_configuration_without_replacing_explicit_paths() {
        let mut state = DesktopAppState::new();
        state.put_config_value(
            "cuemol2.ui.render.pov-exe-path",
            "configured/povray".to_string(),
        );
        for value in ["", "povray", "povray.exe"] {
            assert_eq!(
                resolve_default(&state, "povray", value),
                "configured/povray"
            );
        }
        assert_eq!(
            resolve_default(&state, "povray", "selected/renderer"),
            "selected/renderer"
        );
    }

    #[test]
    fn bundled_povray_keeps_its_include_path_after_relocation() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("moved tools/povray");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("include")).unwrap();
        std::fs::write(root.join("cuemol.ini"), "").unwrap();
        std::fs::write(root.join("include/colors.inc"), "").unwrap();
        let executable = root.join("bin/povray");
        let mut command = Command::new(&executable);
        command.arg("+Iscene.pov");
        configure_command(&mut command, &executable);
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "+Iscene.pov".to_string(),
                format!("+L\"{}\"", root.join("include").display())
            ]
        );
        assert!(command
            .get_envs()
            .any(|(key, value)| key == "POVINI"
                && value == Some(root.join("cuemol.ini").as_os_str())));
        let mut ordinary = Command::new(directory.path().join("bin/tool"));
        configure_command(&mut ordinary, directory.path().join("bin/tool").as_path());
        assert_eq!(ordinary.get_args().count(), 0);
    }
}
