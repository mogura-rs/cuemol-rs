//! Desktop front ends (egui and Tauri) over the shared, GUI-agnostic
//! [`cuemol_desktop_core`] logic.
//!
//! The headless `DesktopAppState` facade, the `Gui*Summary` view-models, and the
//! offscreen wgpu renderer live in `cuemol-desktop-core` and are shared by both
//! front ends.
//!
//! Everything from the core crate is re-exported at this crate's root, so the
//! existing GUI modules keep referring to `crate::DesktopAppState`,
//! `crate::GuiObjectSummary`, `crate::offscreen::…` unchanged.

pub use cuemol_desktop_core::*;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn download_cache_path(filename: &str) -> std::path::PathBuf {
    let root = std::env::var_os("CUEMOL2_CACHE_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            if cfg!(target_os = "windows") {
                std::env::var_os("LOCALAPPDATA")
                    .map(std::path::PathBuf::from)
                    .map(|path| path.join("CueMol2").join("Cache"))
            } else if cfg!(target_os = "macos") {
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .map(|path| path.join("Library").join("Caches").join("CueMol2"))
            } else {
                std::env::var_os("XDG_CACHE_HOME")
                    .map(std::path::PathBuf::from)
                    .map(|path| path.join("cuemol2"))
                    .or_else(|| {
                        std::env::var_os("HOME")
                            .map(std::path::PathBuf::from)
                            .map(|path| path.join(".cache").join("cuemol2"))
                    })
            }
        })
        .unwrap_or_else(std::env::temp_dir);
    root.join("downloads").join(filename)
}

// The offscreen renderer is compiled into core only under its `offscreen`
// feature, which both GUIs turn on; re-export it so `crate::offscreen::…` keeps
// resolving in the GUI modules.
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub use cuemol_desktop_core::offscreen;

#[cfg(feature = "egui-gui")]
pub mod gui;

#[cfg(feature = "tauri-gui")]
pub mod tauri_app;
