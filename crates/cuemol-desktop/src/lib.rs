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
use std::collections::{BTreeMap, HashMap};
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
use std::path::{Path, PathBuf};
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
use std::sync::{Arc, Condvar, Mutex, OnceLock};
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
use std::time::{Duration, Instant};

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static NEXT_DOWNLOAD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
const MAX_PDB_HISTORY: usize = 10;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RendererSetupPreferences {
    pub renderer_type: Option<String>,
    pub recenter: bool,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
impl Default for RendererSetupPreferences {
    fn default() -> Self {
        Self {
            renderer_type: None,
            recenter: true,
        }
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
struct PdbDownloadGroup {
    url: String,
    path: PathBuf,
    entries: Vec<(usize, GuiPdbFileKind)>,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PdbProbeSpec {
    kinds: Vec<GuiPdbFileKind>,
    url: String,
    path: PathBuf,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
struct DownloadGate {
    result: Mutex<Option<DownloadResult>>,
    ready: Condvar,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
type AvailabilityResult = std::result::Result<bool, String>;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
struct AvailabilityGate {
    result: Mutex<Option<AvailabilityResult>>,
    ready: Condvar,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
#[derive(Debug, Clone, Copy)]
struct AvailabilityCacheEntry {
    available: bool,
    checked_at: Instant,
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
type DownloadGates = Mutex<HashMap<PathBuf, Arc<DownloadGate>>>;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
type AvailabilityGates = Mutex<HashMap<String, Arc<AvailabilityGate>>>;

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static DOWNLOAD_GATES: OnceLock<DownloadGates> = OnceLock::new();

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static PDB_AVAILABILITY_GATES: OnceLock<AvailabilityGates> = OnceLock::new();

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
static PDB_AVAILABILITY_CACHE: OnceLock<Mutex<HashMap<String, AvailabilityCacheEntry>>> =
    OnceLock::new();

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
const PDB_UNAVAILABLE_CACHE_TTL: Duration = Duration::from_secs(15);

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
fn configuration_path(filename: &str) -> PathBuf {
    let root = std::env::var_os("CUEMOL2_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(target_os = "windows") {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("CueMol2"))
            } else if cfg!(target_os = "macos") {
                std::env::var_os("HOME").map(PathBuf::from).map(|path| {
                    path.join("Library")
                        .join("Application Support")
                        .join("CueMol2")
                })
            } else {
                std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join("cuemol2"))
                    .or_else(|| {
                        std::env::var_os("HOME")
                            .map(PathBuf::from)
                            .map(|path| path.join(".config").join("cuemol2"))
                    })
            }
        })
        .unwrap_or_else(|| std::env::temp_dir().join("cuemol2"));
    root.join(filename)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn pdb_history_path() -> PathBuf {
    configuration_path("open-pdb-history.txt")
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn load_pdb_history_from(path: &Path) -> std::result::Result<Vec<String>, String> {
    let input = match std::fs::read_to_string(path) {
        Ok(input) => input,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("failed to read PDB history: {error}")),
    };
    let mut history = Vec::new();
    for value in input.lines() {
        let Ok(pdb_id) = normalize_pdb_id(value) else {
            continue;
        };
        let pdb_id = pdb_id.to_ascii_lowercase();
        if !history.contains(&pdb_id) {
            history.push(pdb_id);
        }
        if history.len() == MAX_PDB_HISTORY {
            break;
        }
    }
    Ok(history)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn load_pdb_history() -> Vec<String> {
    load_pdb_history_from(&pdb_history_path()).unwrap_or_default()
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn remember_pdb_id_at(path: &Path, value: &str) -> std::result::Result<Vec<String>, String> {
    let pdb_id = normalize_pdb_id(value)?.to_ascii_lowercase();
    let mut history = load_pdb_history_from(path)?;
    history.retain(|entry| entry != &pdb_id);
    history.insert(0, pdb_id);
    history.truncate(MAX_PDB_HISTORY);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create PDB history directory: {error}"))?;
    }
    let mut output = history.join("\n");
    output.push('\n');
    std::fs::write(path, output).map_err(|error| format!("failed to save PDB history: {error}"))?;
    Ok(history)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn remember_pdb_id(value: &str) -> std::result::Result<Vec<String>, String> {
    remember_pdb_id_at(&pdb_history_path(), value)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn load_renderer_setup_preferences_from(
    path: &Path,
) -> std::result::Result<RendererSetupPreferences, String> {
    let input = match std::fs::read_to_string(path) {
        Ok(input) => input,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RendererSetupPreferences::default());
        }
        Err(error) => {
            return Err(format!(
                "failed to read renderer setup preferences: {error}"
            ))
        }
    };
    let mut preferences = RendererSetupPreferences::default();
    for line in input.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        match key {
            "renderer_type" if !value.trim().is_empty() => {
                preferences.renderer_type = Some(value.trim().to_string());
            }
            "recenter" => preferences.recenter = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    Ok(preferences)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn load_renderer_setup_preferences() -> RendererSetupPreferences {
    let path = configuration_path("renderer-setup-preferences.txt");
    if path.exists() {
        return load_renderer_setup_preferences_from(&path).unwrap_or_default();
    }
    load_renderer_setup_preferences_from(&configuration_path("pdb-renderer-preferences.txt"))
        .unwrap_or_default()
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn save_renderer_setup_preferences_at(
    path: &Path,
    renderer_type: &str,
    recenter: bool,
) -> std::result::Result<(), String> {
    let renderer_type = renderer_type.trim();
    if renderer_type.is_empty()
        || renderer_type
            .chars()
            .any(|character| matches!(character, '\t' | '\r' | '\n'))
    {
        return Err("invalid renderer type preference".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create renderer preference directory: {error}"))?;
    }
    std::fs::write(
        path,
        format!("renderer_type\t{renderer_type}\nrecenter\t{recenter}\n"),
    )
    .map_err(|error| format!("failed to save renderer setup preferences: {error}"))
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn save_renderer_setup_preferences(
    renderer_type: &str,
    recenter: bool,
) -> std::result::Result<(), String> {
    save_renderer_setup_preferences_at(
        &configuration_path("renderer-setup-preferences.txt"),
        renderer_type,
        recenter,
    )
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
fn pdb_probe_specs(options: &PdbFetchOptions) -> std::result::Result<Vec<PdbProbeSpec>, String> {
    let mut probes: Vec<PdbProbeSpec> = Vec::new();
    for spec in pdb_download_specs(options)? {
        if let Some(probe) = probes.iter_mut().find(|probe| probe.url == spec.url) {
            probe.kinds.push(spec.kind);
        } else {
            probes.push(PdbProbeSpec {
                kinds: vec![spec.kind],
                url: spec.url,
                path: spec.path,
            });
        }
    }
    Ok(probes)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn probe_pdb_files(
    options: PdbFetchOptions,
) -> std::result::Result<Vec<GuiPdbFileKind>, String> {
    let mut probes = pdb_probe_specs(&options)?;
    if probes.len() == 1 {
        return probe_pdb_spec(probes.pop().expect("one PDB probe"));
    }
    let handles = probes
        .into_iter()
        .map(|probe| {
            std::thread::Builder::new()
                .name("cuemol-pdb-probe".to_string())
                .spawn(move || probe_pdb_spec(probe))
                .map_err(|error| format!("failed to start PDB availability check: {error}"))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut unavailable = Vec::new();
    for handle in handles {
        unavailable.extend(
            handle
                .join()
                .map_err(|_| "PDB availability check panicked".to_string())??,
        );
    }
    Ok(unavailable)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn probe_pdb_spec(probe: PdbProbeSpec) -> std::result::Result<Vec<GuiPdbFileKind>, String> {
    if nonempty_file(&probe.path) || probe_url_available(&probe.url)? {
        Ok(Vec::new())
    } else {
        Ok(probe.kinds)
    }
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn probe_url_available(url: &str) -> std::result::Result<bool, String> {
    probe_url_available_with(url, |url| {
        let status = std::process::Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--head",
                "--connect-timeout",
                "3",
                "--max-time",
                "8",
            ])
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|error| format!("failed to start PDB availability check: {error}"))?;
        Ok(status.success())
    })
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn probe_url_available_with(
    url: &str,
    operation: impl FnOnce(&str) -> AvailabilityResult,
) -> AvailabilityResult {
    if let Some(available) = cached_pdb_availability(url) {
        return Ok(available);
    }

    let gates = PDB_AVAILABILITY_GATES.get_or_init(Default::default);
    let (gate, leader) = {
        let mut entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(gate) = entries.get(url) {
            (Arc::clone(gate), false)
        } else {
            let gate = Arc::new(AvailabilityGate {
                result: Mutex::new(None),
                ready: Condvar::new(),
            });
            entries.insert(url.to_string(), Arc::clone(&gate));
            (gate, true)
        }
    };
    if !leader {
        let mut result = gate
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while result.is_none() {
            result = gate
                .ready
                .wait(result)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        return result
            .as_ref()
            .expect("availability gate result was checked")
            .clone();
    }

    let result = cached_pdb_availability(url).map_or_else(|| operation(url), Ok);
    if let Ok(available) = &result {
        PDB_AVAILABILITY_CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                url.to_string(),
                AvailabilityCacheEntry {
                    available: *available,
                    checked_at: Instant::now(),
                },
            );
    }
    {
        let mut stored = gate
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *stored = Some(result.clone());
        gate.ready.notify_all();
    }
    let mut entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
    if entries
        .get(url)
        .is_some_and(|current| Arc::ptr_eq(current, &gate))
    {
        entries.remove(url);
    }
    result
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn cached_pdb_availability(url: &str) -> Option<bool> {
    let entry = PDB_AVAILABILITY_CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(url)
        .copied()?;
    (entry.available || entry.checked_at.elapsed() <= PDB_UNAVAILABLE_CACHE_TTL)
        .then_some(entry.available)
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub(crate) fn download_pdb_files(
    options: PdbFetchOptions,
) -> std::result::Result<Vec<PdbDownloadedFile>, String> {
    let specs = pdb_download_specs(&options)?;
    let file_count = specs.len();
    let groups = group_pdb_download_specs(specs);
    let mut downloaded = if groups.len() == 1 {
        download_pdb_group(groups.into_iter().next().expect("one download group"))?
    } else {
        let handles = groups
            .into_iter()
            .enumerate()
            .map(|(index, group)| {
                std::thread::Builder::new()
                    .name(format!("cuemol-pdb-download-{index}"))
                    .spawn(move || download_pdb_group(group))
                    .map_err(|error| format!("failed to start PDB download: {error}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut downloaded = Vec::with_capacity(file_count);
        for handle in handles {
            downloaded.extend(
                handle
                    .join()
                    .map_err(|_| "PDB download task panicked".to_string())??,
            );
        }
        downloaded
    };
    downloaded.sort_unstable_by_key(|(index, _)| *index);
    Ok(downloaded
        .into_iter()
        .map(|(_, file)| file)
        .collect::<Vec<_>>())
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn group_pdb_download_specs(specs: Vec<PdbDownloadSpec>) -> Vec<PdbDownloadGroup> {
    let mut groups = BTreeMap::<(String, PathBuf), Vec<(usize, GuiPdbFileKind)>>::new();
    for (index, spec) in specs.into_iter().enumerate() {
        groups
            .entry((spec.url, spec.path))
            .or_default()
            .push((index, spec.kind));
    }
    groups
        .into_iter()
        .map(|((url, path), entries)| PdbDownloadGroup { url, path, entries })
        .collect()
}

#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
fn download_pdb_group(
    group: PdbDownloadGroup,
) -> std::result::Result<Vec<(usize, PdbDownloadedFile)>, String> {
    let path = download_to_shared_cache(&group.url, &group.path)?;
    Ok(group
        .entries
        .into_iter()
        .map(|(index, kind)| {
            (
                index,
                PdbDownloadedFile {
                    kind,
                    path: path.clone(),
                },
            )
        })
        .collect())
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
    fn pdb_history_matches_reference_mru_order_and_limit() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-pdb-history-test-{}-{}.txt",
            std::process::id(),
            NEXT_DOWNLOAD_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, "invalid\n1CRN\n1crn\n2xyz\n").unwrap();
        assert_eq!(load_pdb_history_from(&path).unwrap(), ["1crn", "2xyz"]);

        for index in 0..12 {
            let pdb_id = format!("{index:04}");
            remember_pdb_id_at(&path, &pdb_id).unwrap();
        }
        let history = remember_pdb_id_at(&path, "0005").unwrap();
        assert_eq!(history.len(), MAX_PDB_HISTORY);
        assert_eq!(history[0], "0005");
        assert_eq!(history[1], "0011");
        assert_eq!(load_pdb_history_from(&path).unwrap(), history);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn renderer_setup_preferences_match_reference_persistence() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-pdb-renderer-preferences-test-{}-{}.txt",
            std::process::id(),
            NEXT_DOWNLOAD_ID.fetch_add(1, Ordering::Relaxed)
        ));
        assert_eq!(
            load_renderer_setup_preferences_from(&path).unwrap(),
            RendererSetupPreferences::default()
        );
        save_renderer_setup_preferences_at(&path, "Default2RendPreset", false).unwrap();
        assert_eq!(
            load_renderer_setup_preferences_from(&path).unwrap(),
            RendererSetupPreferences {
                renderer_type: Some("Default2RendPreset".to_string()),
                recenter: false,
            }
        );
        std::fs::remove_file(path).unwrap();
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
    fn pdb_probe_specs_coalesce_the_shared_ebi_map() {
        let options = PdbFetchOptions {
            pdb_id: "1CRN".to_string(),
            fetch_coordinates: true,
            coordinate_source: "RCSB_CIF".to_string(),
            fetch_two_fofc: true,
            fetch_fofc: true,
            map_source: "EBI_MTZ".to_string(),
        };
        let probes = pdb_probe_specs(&options).unwrap();
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].kinds, [GuiPdbFileKind::Coordinates]);
        assert_eq!(
            probes[1].kinds,
            [GuiPdbFileKind::TwoFoFc, GuiPdbFileKind::FoFc]
        );
        assert!(probes[1].url.ends_with("/1crn_map.mtz"));
    }

    #[test]
    fn pdb_download_groups_coalesce_the_shared_ebi_map() {
        let options = PdbFetchOptions {
            pdb_id: "1CRN".to_string(),
            fetch_coordinates: true,
            coordinate_source: "RCSB_CIF".to_string(),
            fetch_two_fofc: true,
            fetch_fofc: true,
            map_source: "EBI_MTZ".to_string(),
        };
        let groups = group_pdb_download_specs(pdb_download_specs(&options).unwrap());
        assert_eq!(groups.len(), 2);
        let map = groups
            .iter()
            .find(|group| group.path.extension().is_some_and(|ext| ext == "mtz"))
            .unwrap();
        assert_eq!(map.entries.len(), 2);
        assert_eq!(map.entries[0].1, GuiPdbFileKind::TwoFoFc);
        assert_eq!(map.entries[1].1, GuiPdbFileKind::FoFc);
    }

    #[test]
    fn pdb_availability_checks_share_inflight_and_recent_negative_results() {
        let url = format!(
            "test://pdb-availability-{}-{}",
            std::process::id(),
            NEXT_DOWNLOAD_ID.fetch_add(1, Ordering::Relaxed)
        );
        let starts = Arc::new(AtomicUsize::new(0));
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let leader_url = url.clone();
        let leader_starts = starts.clone();
        let leader = std::thread::spawn(move || {
            probe_url_available_with(&leader_url, |_| {
                leader_starts.fetch_add(1, Ordering::Relaxed);
                started_sender.send(()).unwrap();
                release_receiver.recv().unwrap();
                Ok(false)
            })
        });
        started_receiver.recv().unwrap();

        let follower_url = url.clone();
        let follower_starts = starts.clone();
        let follower = std::thread::spawn(move || {
            probe_url_available_with(&follower_url, |_| {
                follower_starts.fetch_add(1, Ordering::Relaxed);
                Ok(true)
            })
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let follower_is_waiting = PDB_AVAILABILITY_GATES
                .get()
                .and_then(|gates| {
                    let entries = gates.lock().unwrap_or_else(|poison| poison.into_inner());
                    entries.get(&url).map(Arc::strong_count)
                })
                .is_some_and(|strong_count| strong_count >= 3);
            if follower_is_waiting {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        release_sender.send(()).unwrap();

        assert!(!leader.join().unwrap().unwrap());
        assert!(!follower.join().unwrap().unwrap());
        assert!(!probe_url_available_with(&url, |_| {
            starts.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        })
        .unwrap());
        assert_eq!(starts.load(Ordering::Relaxed), 1);
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
