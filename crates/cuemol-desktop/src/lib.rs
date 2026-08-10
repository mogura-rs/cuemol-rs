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
use std::collections::HashMap;
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
use std::path::{Path, PathBuf};
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
use std::sync::{Arc, Condvar, Mutex, OnceLock};

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static NEXT_DOWNLOAD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
type DownloadResult = std::result::Result<PathBuf, String>;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
struct DownloadGate {
    result: Mutex<Option<DownloadResult>>,
    ready: Condvar,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
type DownloadGates = Mutex<HashMap<PathBuf, Arc<DownloadGate>>>;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static DOWNLOAD_GATES: OnceLock<DownloadGates> = OnceLock::new();

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

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn download_to_shared_cache(url: &str, path: &Path) -> DownloadResult {
    if nonempty_file(path) {
        return Ok(path.to_path_buf());
    }
    with_download_gate(path, || download_uncached(url, path))
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn with_download_gate(path: &Path, operation: impl FnOnce() -> DownloadResult) -> DownloadResult {
    let gates = DOWNLOAD_GATES.get_or_init(Default::default);
    let (gate, leader) = {
        let mut entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(gate) = entries.get(path) {
            (gate.clone(), false)
        } else {
            let gate = Arc::new(DownloadGate {
                result: Mutex::new(None),
                ready: Condvar::new(),
            });
            entries.insert(path.to_path_buf(), gate.clone());
            (gate, true)
        }
    };

    if leader {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
            .unwrap_or_else(|_| Err("download task panicked".to_string()));
        *gate
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(result.clone());
        gate.ready.notify_all();
        let mut entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
        if entries
            .get(path)
            .is_some_and(|current| Arc::ptr_eq(current, &gate))
        {
            entries.remove(path);
        }
        return result;
    }

    let mut result = gate
        .result
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    loop {
        if let Some(result) = result.as_ref() {
            return result.clone();
        }
        result = gate
            .ready
            .wait(result)
            .unwrap_or_else(|poison| poison.into_inner());
    }
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn download_uncached(url: &str, path: &Path) -> DownloadResult {
    if nonempty_file(path) {
        return Ok(path.to_path_buf());
    }
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create download cache: {error}"))?;
    }
    let download_id = NEXT_DOWNLOAD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let partial_path = path.with_file_name(format!(
        "{filename}.{}.{download_id}.partial",
        std::process::id(),
    ));
    let output = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--compressed",
            "--connect-timeout",
            "5",
            "--max-time",
            "30",
        ])
        .arg("--output")
        .arg(&partial_path)
        .arg(url)
        .output()
        .map_err(|error| format!("failed to start curl: {error}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&partial_path);
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("curl failed with status {}", output.status)
        } else {
            format!("curl failed: {detail}")
        });
    }
    if !nonempty_file(&partial_path) {
        let _ = std::fs::remove_file(&partial_path);
        return Err("download returned an empty file".to_string());
    }
    if let Err(error) = std::fs::rename(&partial_path, path) {
        if nonempty_file(path) {
            let _ = std::fs::remove_file(&partial_path);
        } else {
            let _ = std::fs::remove_file(&partial_path);
            return Err(format!("failed to cache download: {error}"));
        }
    }
    Ok(path.to_path_buf())
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn nonempty_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0)
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

#[cfg(all(test, any(feature = "egui-gui", feature = "tauri-gui")))]
mod download_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn concurrent_cache_requests_share_one_operation() {
        let path =
            std::env::temp_dir().join(format!("cuemol-download-gate-test-{}", std::process::id()));
        let starts = Arc::new(AtomicUsize::new(0));
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let leader_path = path.clone();
        let leader_starts = starts.clone();
        let leader = std::thread::spawn(move || {
            with_download_gate(&leader_path, || {
                leader_starts.fetch_add(1, Ordering::Relaxed);
                started_sender.send(()).unwrap();
                release_receiver.recv().unwrap();
                Ok(leader_path.clone())
            })
        });
        started_receiver.recv().unwrap();

        let follower_path = path.clone();
        let follower_starts = starts.clone();
        let follower = std::thread::spawn(move || {
            with_download_gate(&follower_path, || {
                follower_starts.fetch_add(1, Ordering::Relaxed);
                Ok(follower_path.clone())
            })
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            let follower_is_waiting = DOWNLOAD_GATES
                .get()
                .and_then(|gates| {
                    let entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
                    entries.get(&path).map(Arc::strong_count)
                })
                .is_some_and(|strong_count| strong_count >= 3);
            if follower_is_waiting {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        release_sender.send(()).unwrap();

        assert_eq!(leader.join().unwrap().unwrap(), path);
        assert_eq!(follower.join().unwrap().unwrap(), path);
        assert_eq!(starts.load(Ordering::Relaxed), 1);
    }
}
