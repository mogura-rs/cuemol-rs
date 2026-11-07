use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::VecDeque, sync::Arc};

pub const HISTORY_LIMIT: usize = 50;
pub const BROWSER_BYTE_LIMIT: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderSource {
    pub scene_name: String,
    pub settings: Map<String, Value>,
}

impl RenderSource {
    pub fn capture(state: &cuemol_desktop_core::DesktopAppState) -> cuemol_core::Result<Self> {
        let scene = state
            .active_scene()
            .ok_or_else(|| cuemol_core::CueMolError::not_found("render scene"))?;
        Self::capture_scene(state, scene)
    }

    pub fn capture_scene(
        state: &cuemol_desktop_core::DesktopAppState,
        scene: cuemol_core::Uid,
    ) -> cuemol_core::Result<Self> {
        let mut settings = crate::scene_render_settings(state, scene)?;
        let backend = settings
            .get("backend")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or("umbreon")
            .to_string();
        settings.retain(|key, _| !key.contains('.') || key.starts_with(&format!("{backend}.")));
        let scene_name = state
            .scene_manager()
            .scene(scene)
            .map(|scene| scene.name().to_string())
            .unwrap_or_default();
        Ok(Self {
            scene_name,
            settings,
        })
    }

    pub fn settings_argument(&self) -> String {
        self.settings
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderRecord {
    pub id: u64,
    pub source: RenderSource,
    pub size: [u32; 2],
    pub elapsed_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movie: Option<crate::movie::MovieResult>,
}

impl RenderRecord {
    pub fn filename(&self) -> String {
        let name = self
            .source
            .scene_name
            .chars()
            .map(|c| {
                if c.is_control() || "/\\:*?\"<>|".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect::<String>();
        let name = name.trim().trim_matches('.');
        format!(
            "{}-{}x{}.png",
            if name.is_empty() { "render" } else { name },
            self.size[0],
            self.size[1]
        )
    }
}

#[derive(Debug)]
enum ImageStorage {
    Memory(Arc<[u8]>),
    #[cfg(not(target_arch = "wasm32"))]
    File(std::path::PathBuf),
}

#[derive(Debug)]
struct Entry {
    record: RenderRecord,
    image: ImageStorage,
    bytes: usize,
    frame: Option<u32>,
    frame_cached: bool,
}

impl Entry {
    fn read(&self) -> Result<Arc<[u8]>, String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((movie, frame)) = self
            .record
            .movie
            .as_ref()
            .zip(self.frame)
            .filter(|_| !self.frame_cached)
        {
            let path = crate::movie::files::frame_path(
                std::path::Path::new(&movie.output_dir),
                &movie.base_name,
                frame,
            )?;
            return std::fs::read(path)
                .map(Arc::from)
                .map_err(|e| format!("Movie frame is no longer available: {e}"));
        }
        match &self.image {
            ImageStorage::Memory(bytes) => Ok(bytes.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            ImageStorage::File(path) => std::fs::read(path)
                .map(Arc::from)
                .map_err(|e| e.to_string()),
        }
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let ImageStorage::File(path) = &self.image {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Debug)]
pub struct RenderHistory {
    entries: VecDeque<Entry>,
    selected: Option<usize>,
    next_id: u64,
    bytes: usize,
    byte_limit: Option<usize>,
    #[cfg(not(target_arch = "wasm32"))]
    directory: Option<tempfile::TempDir>,
}

impl Default for RenderHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            selected: None,
            next_id: 1,
            bytes: 0,
            byte_limit: cfg!(target_arch = "wasm32").then_some(BROWSER_BYTE_LIMIT),
            #[cfg(not(target_arch = "wasm32"))]
            directory: None,
        }
    }
}

impl RenderHistory {
    pub fn in_memory(byte_limit: usize) -> Self {
        Self {
            byte_limit: Some(byte_limit),
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn current(&self) -> Option<&RenderRecord> {
        self.selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| &entry.record)
    }

    pub fn push(
        &mut self,
        png: Arc<[u8]>,
        source: RenderSource,
        size: [u32; 2],
        elapsed_seconds: Option<f64>,
    ) -> Result<(), String> {
        let bytes = png.len();
        #[cfg(not(target_arch = "wasm32"))]
        let image = if self.byte_limit.is_none() {
            if self.directory.is_none() {
                self.directory = Some(
                    tempfile::Builder::new()
                        .prefix("cuemol-render-history-")
                        .tempdir()
                        .map_err(|e| e.to_string())?,
                );
            }
            let path = self
                .directory
                .as_ref()
                .unwrap()
                .path()
                .join(format!("{}.png", self.next_id));
            std::fs::write(&path, &png).map_err(|e| e.to_string())?;
            ImageStorage::File(path)
        } else {
            ImageStorage::Memory(png)
        };
        #[cfg(target_arch = "wasm32")]
        let image = ImageStorage::Memory(png);
        self.entries.push_back(Entry {
            record: RenderRecord {
                id: self.next_id,
                source,
                size,
                elapsed_seconds,
                movie: None,
            },
            image,
            bytes,
            frame: None,
            frame_cached: false,
        });
        self.next_id = self.next_id.wrapping_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        while self.entries.len() > HISTORY_LIMIT
            || (self.entries.len() > 1 && self.byte_limit.is_some_and(|limit| self.bytes > limit))
        {
            if let Some(entry) = self.entries.pop_front() {
                self.bytes -= entry.bytes;
            }
        }
        self.selected = Some(self.entries.len() - 1);
        Ok(())
    }

    pub fn push_movie(
        &mut self,
        png: Arc<[u8]>,
        source: RenderSource,
        size: [u32; 2],
        elapsed_seconds: Option<f64>,
        movie: crate::movie::MovieResult,
    ) -> Result<(), String> {
        if movie.frame_count == 0 {
            return Err("Movie has no rendered frames".into());
        }
        self.push(png, source, size, elapsed_seconds)?;
        if movie.movie_path.is_some() {
            for entry in self.entries.iter_mut().rev().skip(1) {
                if let Some(previous) = &mut entry.record.movie {
                    if previous.movie_path == movie.movie_path {
                        previous.movie_path = None;
                    }
                }
            }
        }
        self.entries.back_mut().unwrap().record.movie = Some(movie);
        Ok(())
    }

    pub fn invalidate_movie_identity(&mut self, directory: &str, base: &str) {
        for entry in &mut self.entries {
            let Some(movie) = &mut entry.record.movie else {
                continue;
            };
            if movie.output_dir == directory && movie.base_name == base {
                movie.frames_available = false;
                movie.movie_path = None;
                if !entry.frame_cached {
                    entry.frame = None;
                }
            }
        }
    }

    pub fn select_frame_bytes_with<T>(
        &mut self,
        record: u64,
        index: u32,
        png: Arc<[u8]>,
        validate: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<T, String> {
        let selected = self.selected.ok_or("No movie is selected")?;
        let entry = self
            .entries
            .get_mut(selected)
            .ok_or("No movie is selected")?;
        let movie = entry
            .record
            .movie
            .as_ref()
            .ok_or("Selected render is not a movie")?;
        if entry.record.id != record || !movie.frames_available || index >= movie.frame_count {
            return Err("Movie frame request is no longer current".into());
        }
        if !matches!(&entry.image, ImageStorage::Memory(_)) {
            return Err("Byte-based frame selection requires memory history".into());
        }
        let decoded = validate(&png)?;
        self.bytes = self
            .bytes
            .saturating_sub(entry.bytes)
            .saturating_add(png.len());
        entry.bytes = png.len();
        entry.image = ImageStorage::Memory(png);
        entry.frame = Some(index);
        entry.frame_cached = true;
        while self.entries.len() > 1 && self.byte_limit.is_some_and(|limit| self.bytes > limit) {
            let selected = self.selected.unwrap();
            let remove = usize::from(selected == 0);
            let entry = self.entries.remove(remove).unwrap();
            self.bytes -= entry.bytes;
            if remove < selected {
                self.selected = Some(selected - 1);
            }
        }
        Ok(decoded)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn invalidate_movie_outputs(&mut self, directory: &std::path::Path, base: &str) {
        let directory = directory
            .canonicalize()
            .unwrap_or_else(|_| directory.to_path_buf());
        for entry in &mut self.entries {
            let Some(movie) = &mut entry.record.movie else {
                continue;
            };
            let path = std::path::Path::new(&movie.output_dir);
            if movie.base_name == base
                && path.canonicalize().unwrap_or_else(|_| path.to_path_buf()) == directory
            {
                movie.frames_available = false;
                movie.movie_path = None;
                entry.frame = None;
                entry.frame_cached = false;
            }
        }
    }

    pub fn frame(&self) -> Option<u32> {
        let entry = self.entries.get(self.selected?)?;
        entry
            .record
            .movie
            .as_ref()
            .map(|movie| entry.frame.unwrap_or(movie.frame_count - 1))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn select_frame_with<T>(
        &mut self,
        index: u32,
        validate: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<(Arc<[u8]>, T), String> {
        let entry = self
            .selected
            .and_then(|index| self.entries.get_mut(index))
            .ok_or("No movie is selected")?;
        let movie = entry
            .record
            .movie
            .as_ref()
            .ok_or("Selected render is not a movie")?;
        if !movie.frames_available {
            return Err(
                "Movie frames were replaced or removed; the saved preview is still available"
                    .into(),
            );
        }
        if index >= movie.frame_count {
            return Err("Movie frame is outside the rendered range".into());
        }
        let path = crate::movie::files::frame_path(
            std::path::Path::new(&movie.output_dir),
            &movie.base_name,
            index,
        )?;
        let png: Arc<[u8]> = std::fs::read(path)
            .map_err(|e| format!("Movie frame is no longer available: {e}"))?
            .into();
        let decoded = validate(&png)?;
        entry.frame = Some(index);
        entry.frame_cached = false;
        Ok((png, decoded))
    }

    pub fn image(&self, index: usize) -> Result<Arc<[u8]>, String> {
        self.entries
            .get(index)
            .ok_or("Render result is no longer available")?
            .read()
    }

    pub fn select(&mut self, index: usize) -> Result<Arc<[u8]>, String> {
        self.select_with(index, |_| Ok(())).map(|(png, ())| png)
    }

    pub fn select_with<T>(
        &mut self,
        index: usize,
        validate: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<(Arc<[u8]>, T), String> {
        let png = self.image(index)?;
        let decoded = validate(&png)?;
        self.selected = Some(index);
        Ok((png, decoded))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.selected = None;
        self.bytes = 0;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.directory = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(name: &str) -> RenderSource {
        RenderSource {
            scene_name: name.into(),
            settings: serde_json::json!({"dpi": 300, "hatchSpec": "a;b=\"c\""})
                .as_object()
                .unwrap()
                .clone(),
        }
    }

    #[test]
    fn asynchronous_movie_frames_reject_stale_results_and_keep_the_memory_budget() {
        let mut history = RenderHistory::in_memory(8);
        let movie = crate::movie::MovieResult {
            output_dir: "session-one".into(),
            base_name: "movie".into(),
            frame_count: 3,
            frames_available: true,
            fps: 30.,
            movie_path: None,
            options: Default::default(),
        };
        for name in ["first", "second"] {
            history
                .push_movie(
                    Arc::from(&b"last"[..]),
                    source(name),
                    [1, 1],
                    None,
                    movie.clone(),
                )
                .unwrap();
        }
        history.select(0).unwrap();
        let id = history.current().unwrap().id;
        assert!(history
            .select_frame_bytes_with(id + 1, 0, Arc::from(&b"wrong"[..]), |_| Ok(()))
            .is_err());
        assert!(history
            .select_frame_bytes_with(id, 3, Arc::from(&b"wrong"[..]), |_| Ok(()))
            .is_err());
        assert!(history
            .select_frame_bytes_with(id, 0, Arc::from(&b"bad"[..]), |_| Err::<(), _>(
                "bad image".into()
            ))
            .is_err());
        assert_eq!(history.frame(), Some(2));
        history
            .select_frame_bytes_with(id, 0, Arc::from(&b"frame-zero"[..]), |_| Ok(()))
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history.bytes, 10);
        assert_eq!(history.current().unwrap().id, id);
        assert_eq!(history.frame(), Some(0));
        assert_eq!(&*history.image(0).unwrap(), b"frame-zero");
        history.invalidate_movie_identity("session-one", "movie");
        assert!(
            !history
                .current()
                .unwrap()
                .movie
                .as_ref()
                .unwrap()
                .frames_available
        );
        assert!(history
            .select_frame_bytes_with(id, 1, Arc::from(&b"stale"[..]), |_| Ok(()))
            .is_err());
        assert_eq!(&*history.image(0).unwrap(), b"frame-zero");
        assert_eq!(history.frame(), Some(0));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn failed_movie_history_writes_preserve_the_previous_encoded_path() {
        let mut history = RenderHistory::default();
        let movie = crate::movie::MovieResult {
            output_dir: "outputs".into(),
            base_name: "movie".into(),
            frame_count: 2,
            frames_available: true,
            fps: 30.,
            movie_path: Some("outputs/movie.mp4".into()),
            options: Default::default(),
        };
        history
            .push_movie(
                Arc::from(&b"first"[..]),
                source("first"),
                [1, 1],
                None,
                movie.clone(),
            )
            .unwrap();
        std::fs::remove_dir_all(history.directory.as_ref().unwrap().path()).unwrap();
        assert!(history
            .push_movie(
                Arc::from(&b"second"[..]),
                source("second"),
                [1, 1],
                None,
                movie.clone()
            )
            .is_err());
        assert_eq!(history.len(), 1);
        assert_eq!(
            history
                .current()
                .unwrap()
                .movie
                .as_ref()
                .unwrap()
                .movie_path,
            movie.movie_path
        );
    }

    #[test]
    fn memory_budget_and_navigation_keep_the_latest_oversized_image() {
        let mut history = RenderHistory::in_memory(6);
        history
            .push(Arc::from(&b"first"[..]), source("one"), [2, 3], Some(1.))
            .unwrap();
        history
            .push(Arc::from(&b"two"[..]), source("two"), [3, 4], None)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(&*history.select(0).unwrap(), b"two");
        assert!(history.select(1).is_err());
        assert_eq!(history.selected(), Some(0));
        history
            .push(Arc::from(&b"oversized"[..]), source("large"), [4, 5], None)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(&*history.image(0).unwrap(), b"oversized");
        assert_eq!(history.current().unwrap().source.scene_name, "large");
    }

    #[test]
    fn count_limit_browsing_and_clear_preserve_snapshot_identity() {
        let mut history = RenderHistory::in_memory(usize::MAX);
        for n in 0..HISTORY_LIMIT + 3 {
            history
                .push(
                    Arc::from([n as u8]),
                    source(&format!("scene-{n}")),
                    [3, 4],
                    None,
                )
                .unwrap();
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(&*history.select(0).unwrap(), &[3]);
        let record = history.current().unwrap();
        assert_eq!(record.source.scene_name, "scene-3");
        assert_eq!(record.filename(), "scene-3-3x4.png");
        assert!(record
            .source
            .settings_argument()
            .contains("hatchSpec=\"a;b=\\\"c\\\"\""));
        history.clear();
        assert!(history.is_empty());
        assert!(history.current().is_none());
        assert!(history.image(0).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn disk_archive_is_owned_and_failed_reads_keep_selection() {
        let mut first = RenderHistory::default();
        let mut second = RenderHistory::default();
        for history in [&mut first, &mut second] {
            history
                .push(Arc::from(&b"a"[..]), source("a"), [1, 1], None)
                .unwrap();
            history
                .push(Arc::from(&b"b"[..]), source("b"), [1, 1], None)
                .unwrap();
        }
        let first_dir = first.directory.as_ref().unwrap().path().to_owned();
        std::fs::remove_file(first_dir.join("1.png")).unwrap();
        assert!(first.select(0).is_err());
        assert_eq!(first.selected(), Some(1));
        first.clear();
        assert!(!first_dir.exists());
        assert_eq!(&*second.select(0).unwrap(), b"a");
        let second_dir = second.directory.as_ref().unwrap().path().to_owned();
        drop(second);
        assert!(!second_dir.exists());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn movie_history_reads_one_frame_and_never_owns_the_movie_output_directory() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("movie_frm_0000.png");
        std::fs::write(&first, b"first frame").unwrap();
        std::fs::write(directory.path().join("movie_frm_0001.png"), b"last frame").unwrap();
        let mut options = crate::movie::MovieOptions::default();
        options.set_custom(directory.path().to_string_lossy().into_owned());
        let movie = crate::movie::MovieResult {
            output_dir: options.output_dir.clone(),
            base_name: "movie".into(),
            frame_count: 2,
            frames_available: true,
            fps: 30.,
            movie_path: None,
            options,
        };
        let mut history = RenderHistory::default();
        history
            .push_movie(
                Arc::from(&b"last frame"[..]),
                source("movie"),
                [32, 24],
                Some(1.),
                movie,
            )
            .unwrap();
        assert_eq!(history.frame(), Some(1));
        assert_eq!(&*history.image(0).unwrap(), b"last frame");
        let (png, ()) = history.select_frame_with(0, |_| Ok(())).unwrap();
        assert_eq!(&*png, b"first frame");
        assert_eq!(history.frame(), Some(0));
        assert!(history
            .select_frame_with(1, |_| Err::<(), _>("decode failed".into()))
            .is_err());
        assert_eq!(history.frame(), Some(0));
        assert!(history.select_frame_with(2, |_| Ok(())).is_err());
        assert_eq!(&*history.image(0).unwrap(), b"first frame");
        history.clear();
        assert_eq!(std::fs::read(first).unwrap(), b"first frame");
        assert!(directory.path().join("movie_frm_0001.png").exists());
    }
}
