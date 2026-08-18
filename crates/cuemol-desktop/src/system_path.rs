use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemPathCommand {
    program: &'static str,
    args: Vec<OsString>,
}

fn system_path_command(path: &Path, reveal: bool) -> Result<SystemPathCommand, String> {
    if !path.is_file() {
        return Err(format!("file does not exist: {}", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        let mut args = Vec::with_capacity(2);
        if reveal {
            args.push(OsString::from("-R"));
        }
        args.push(path.as_os_str().to_owned());
        Ok(SystemPathCommand {
            program: "open",
            args,
        })
    }
    #[cfg(target_os = "windows")]
    {
        if reveal {
            return Ok(SystemPathCommand {
                program: "explorer",
                args: vec![OsString::from("/select,"), path.as_os_str().to_owned()],
            });
        }
        Ok(SystemPathCommand {
            program: "cmd",
            args: vec![
                OsString::from("/C"),
                OsString::from("start"),
                OsString::new(),
                path.as_os_str().to_owned(),
            ],
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if reveal {
            path.parent()
                .ok_or_else(|| "movie output has no parent directory".to_string())?
        } else {
            path
        };
        Ok(SystemPathCommand {
            program: "xdg-open",
            args: vec![target.as_os_str().to_owned()],
        })
    }
}

fn launch_system_path(path: &Path, reveal: bool) -> Result<(), String> {
    let command = system_path_command(path, reveal)?;
    Command::new(command.program)
        .args(command.args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to open {}: {error}", path.display()))
}

pub(crate) fn open_system_path(path: &Path) -> Result<(), String> {
    launch_system_path(path, false)
}

pub(crate) fn reveal_system_path(path: &Path) -> Result<(), String> {
    launch_system_path(path, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn system_path_commands_pass_paths_without_shell_expansion() {
        let path =
            std::env::temp_dir().join(format!("cuemol movie output {}.mp4", std::process::id()));
        fs::write(&path, b"movie").unwrap();

        let open = system_path_command(&path, false).unwrap();
        let reveal = system_path_command(&path, true).unwrap();
        assert!(open
            .args
            .iter()
            .any(|argument| argument == path.as_os_str()));
        #[cfg(target_os = "macos")]
        {
            assert_eq!(open.program, "open");
            assert_eq!(reveal.program, "open");
            assert_eq!(reveal.args[0], "-R");
            assert_eq!(reveal.args[1], path.as_os_str());
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(open.program, "cmd");
            assert_eq!(reveal.program, "explorer");
            assert_eq!(reveal.args[0], "/select,");
            assert_eq!(reveal.args[1], path.as_os_str());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert_eq!(open.program, "xdg-open");
            assert_eq!(reveal.program, "xdg-open");
            assert_eq!(reveal.args[0], path.parent().unwrap().as_os_str());
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn system_path_commands_reject_missing_files() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-missing-movie-output-{}.mp4",
            std::process::id()
        ));
        assert!(system_path_command(&path, false).is_err());
        assert!(system_path_command(&path, true).is_err());
    }
}
