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
        let mut settings = crate::render_settings(state)?;
        let backend = settings
            .get("backend")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or("umbreon")
            .to_string();
        settings.retain(|key, _| !key.contains('.') || key.starts_with(&format!("{backend}.")));
        let scene_name = state
            .active_scene()
            .and_then(|uid| state.scene_manager().scene(uid))
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
}

impl Entry {
    fn read(&self) -> Result<Arc<[u8]>, String> {
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
            },
            image,
            bytes,
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
}
