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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PdbFetchOptions {
    pub pdb_id: String,
    pub fetch_coordinates: bool,
    pub coordinate_source: String,
    pub fetch_two_fofc: bool,
    pub fetch_fofc: bool,
    pub map_source: String,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PdbDownloadedFile {
    pub kind: GuiPdbFileKind,
    pub path: PathBuf,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PdbDownloadSpec {
    kind: GuiPdbFileKind,
    url: String,
    path: PathBuf,
}

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
pub(crate) fn normalize_pdb_id(value: &str) -> std::result::Result<String, String> {
    let pdb_id = value.trim().to_ascii_uppercase();
    let mut characters = pdb_id.chars();
    if pdb_id.len() != 4
        || !characters
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        || !characters.all(|character| character.is_ascii_alphanumeric())
    {
        return Err(
            "PDB ID must start with a digit and contain four alphanumeric characters".to_string(),
        );
    }
    Ok(pdb_id)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn pdb_download_specs(
    options: &PdbFetchOptions,
) -> std::result::Result<Vec<PdbDownloadSpec>, String> {
    let pdb_id = normalize_pdb_id(&options.pdb_id)?;
    if !options.fetch_coordinates && !options.fetch_two_fofc && !options.fetch_fofc {
        return Err("select coordinates or at least one density map".to_string());
    }
    let lowercase_id = pdb_id.to_ascii_lowercase();
    let mut specs = Vec::new();
    if options.fetch_coordinates {
        let (extension, source) = match options.coordinate_source.as_str() {
            "RCSB_CIF" => ("cif.gz", "cif.gz"),
            "RCSB_PDB" => ("pdb.gz", "pdb.gz"),
            source => return Err(format!("unsupported PDB coordinate source: {source}")),
        };
        specs.push(PdbDownloadSpec {
            kind: GuiPdbFileKind::Coordinates,
            url: format!("https://files.rcsb.org/download/{lowercase_id}.{source}"),
            path: pdb_cached_path(
                &format!("cuemol-pdb-{pdb_id}.{extension}"),
                Some(&format!(
                    "cuemol-pdb-{pdb_id}.{}",
                    extension.trim_end_matches(".gz")
                )),
            ),
        });
    }
    if options.fetch_two_fofc || options.fetch_fofc {
        match options.map_source.as_str() {
            "RCSB_CIF" => {
                let middle = &lowercase_id[1..3];
                for (enabled, kind, map_type, suffix) in [
                    (
                        options.fetch_two_fofc,
                        GuiPdbFileKind::TwoFoFc,
                        "2fo-fc",
                        "2fofc",
                    ),
                    (options.fetch_fofc, GuiPdbFileKind::FoFc, "fo-fc", "fofc"),
                ] {
                    if !enabled {
                        continue;
                    }
                    specs.push(PdbDownloadSpec {
                        kind,
                        url: format!(
                            "https://files.rcsb.org/pub/pdb/validation_reports/{middle}/{lowercase_id}/{lowercase_id}_validation_{map_type}_map_coef.cif.gz"
                        ),
                        path: pdb_cached_path(
                            &format!("cuemol-pdb-{pdb_id}-{suffix}.cif.gz"),
                            None,
                        ),
                    });
                }
            }
            "EBI_MTZ" => {
                let url =
                    format!("https://www.ebi.ac.uk/pdbe/coordinates/files/{lowercase_id}_map.mtz");
                let path = pdb_cached_path(&format!("cuemol-pdb-{pdb_id}-map.mtz"), None);
                if options.fetch_two_fofc {
                    specs.push(PdbDownloadSpec {
                        kind: GuiPdbFileKind::TwoFoFc,
                        url: url.clone(),
                        path: path.clone(),
                    });
                }
                if options.fetch_fofc {
                    specs.push(PdbDownloadSpec {
                        kind: GuiPdbFileKind::FoFc,
                        url,
                        path,
                    });
                }
            }
            source => return Err(format!("unsupported PDB map source: {source}")),
        }
    }
    Ok(specs)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn pdb_cached_path(primary_filename: &str, legacy_filename: Option<&str>) -> PathBuf {
    let filenames = legacy_filename.map_or_else(
        || vec![primary_filename],
        |legacy| vec![primary_filename, legacy],
    );
    for filename in &filenames {
        let path = download_cache_path(filename);
        if nonempty_file(&path) {
            return path;
        }
    }
    for filename in &filenames {
        let path = std::env::temp_dir().join(filename);
        if nonempty_file(&path) {
            return path;
        }
    }
    let persistent = download_cache_path(primary_filename);
    if persistent
        .parent()
        .is_some_and(|parent| std::fs::create_dir_all(parent).is_ok())
    {
        persistent
    } else {
        std::env::temp_dir().join(primary_filename)
    }
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn download_pdb_files(
    options: PdbFetchOptions,
) -> std::result::Result<Vec<PdbDownloadedFile>, String> {
    let specs = pdb_download_specs(&options)?;
    let handles = specs
        .into_iter()
        .enumerate()
        .map(|(index, spec)| {
            std::thread::Builder::new()
                .name(format!("cuemol-pdb-download-{index}"))
                .spawn(move || {
                    download_to_shared_cache(&spec.url, &spec.path).map(|path| PdbDownloadedFile {
                        kind: spec.kind,
                        path,
                    })
                })
                .map_err(|error| format!("failed to start PDB download: {error}"))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|_| "PDB download task panicked".to_string())?
        })
        .collect()
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
    fn pdb_id_validation_matches_the_reference_dialog() {
        assert_eq!(normalize_pdb_id(" 1crn ").unwrap(), "1CRN");
        for invalid in ["", "1ABC2", "ABCD", "1AB_", "1AB-"] {
            assert!(normalize_pdb_id(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn pdb_download_specs_match_reference_coordinate_and_map_urls() {
        let options = PdbFetchOptions {
            pdb_id: "1CRN".to_string(),
            fetch_coordinates: true,
            coordinate_source: "RCSB_CIF".to_string(),
            fetch_two_fofc: true,
            fetch_fofc: true,
            map_source: "RCSB_CIF".to_string(),
        };
        let specs = pdb_download_specs(&options).unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].kind, GuiPdbFileKind::Coordinates);
        assert_eq!(specs[0].url, "https://files.rcsb.org/download/1crn.cif.gz");
        assert_eq!(specs[1].kind, GuiPdbFileKind::TwoFoFc);
        assert_eq!(
            specs[1].url,
            "https://files.rcsb.org/pub/pdb/validation_reports/cr/1crn/1crn_validation_2fo-fc_map_coef.cif.gz"
        );
        assert_eq!(specs[2].kind, GuiPdbFileKind::FoFc);
        assert_eq!(
            specs[2].url,
            "https://files.rcsb.org/pub/pdb/validation_reports/cr/1crn/1crn_validation_fo-fc_map_coef.cif.gz"
        );

        let mtz_specs = pdb_download_specs(&PdbFetchOptions {
            fetch_coordinates: false,
            coordinate_source: "RCSB_PDB".to_string(),
            map_source: "EBI_MTZ".to_string(),
            ..options
        })
        .unwrap();
        assert_eq!(mtz_specs.len(), 2);
        assert_eq!(mtz_specs[0].url, mtz_specs[1].url);
        assert_eq!(mtz_specs[0].path, mtz_specs[1].path);
        assert!(mtz_specs[0].url.ends_with("/1crn_map.mtz"));
    }

    #[test]
    fn pdb_download_specs_reject_empty_or_unknown_sources() {
        let mut options = PdbFetchOptions {
            pdb_id: "1CRN".to_string(),
            fetch_coordinates: false,
            coordinate_source: "RCSB_CIF".to_string(),
            fetch_two_fofc: false,
            fetch_fofc: false,
            map_source: "RCSB_CIF".to_string(),
        };
        assert!(pdb_download_specs(&options).is_err());
        options.fetch_coordinates = true;
        options.coordinate_source = "unknown".to_string();
        assert!(pdb_download_specs(&options).is_err());
        options.fetch_coordinates = false;
        options.fetch_two_fofc = true;
        options.map_source = "unknown".to_string();
        assert!(pdb_download_specs(&options).is_err());
    }

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
