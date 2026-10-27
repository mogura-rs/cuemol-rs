#[cfg(not(feature = "scripting"))]
use cuemol_core::CueMolError;
use cuemol_core::Result;
use cuemol_desktop_core::DesktopAppState;
use std::path::PathBuf;

pub fn directory() -> PathBuf {
    std::env::var_os("CUEMOL_CONFIG_DIR")
        .or_else(|| std::env::var_os("CUEMOL2_CONFIG_DIR"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .map(|path| path.join("cuemol"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .map(|path| path.join(".config").join("cuemol"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("cuemol"))
}

pub fn cache_directory() -> PathBuf {
    std::env::var_os("CUEMOL_CACHE_DIR")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CACHE_HOME")
                .filter(|s| !s.is_empty())
                .map(|s| PathBuf::from(s).join("cuemol"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|s| PathBuf::from(s).join(".cache").join("cuemol"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("cuemol-cache"))
}

pub fn load(state: &mut DesktopAppState) -> Result<()> {
    let path = std::env::var_os("CUEMOL_CONFIG_FILE")
        .or_else(|| std::env::var_os("CUEMOL2_CONFIG_FILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| directory().join("config.tsv"));
    if path.is_file() {
        state.load_config_file(path)?;
    }
    let path = directory().join("startup.js");
    if path.is_file() {
        let source = std::fs::read_to_string(&path)?;
        #[cfg(feature = "scripting")]
        {
            let (log, _) = state.run_script_js(&source)?;
            if !log.is_empty() {
                eprintln!("{log}");
            }
        }
        #[cfg(not(feature = "scripting"))]
        if !source.trim().is_empty() {
            return Err(CueMolError::invalid_argument(
                "startup.js requires the scripting feature",
            ));
        }
    }
    Ok(())
}
