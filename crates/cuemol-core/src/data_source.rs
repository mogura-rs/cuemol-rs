use crate::{CueMolError, FileInStream, InStream, LString, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSourceSelection {
    pub path: LString,
    pub read_from_alt_src: bool,
}

/// Selects `src` first and falls back to `alt_src` like `LDataSrcContainer`.
pub fn select_src_alt_src(
    src: impl AsRef<str>,
    alt_src: impl AsRef<str>,
    base_path: impl AsRef<str>,
) -> Result<DataSourceSelection> {
    let src = src.as_ref();
    let alt_src = alt_src.as_ref();
    let mut base_path = base_path.as_ref().to_string();
    if base_path.is_empty() && alt_src.is_empty() {
        base_path = ".".to_string();
    }

    let mut path = resolve_candidate(src, &base_path)?;
    if !is_readable_file(&path) {
        if alt_src.is_empty() {
            return Err(cannot_open(&path));
        }
        path = resolve_candidate(alt_src, &base_path)?;
        if !is_readable_file(&path) {
            return Err(cannot_open(&path));
        }
        return Ok(DataSourceSelection {
            path: path_to_lstring(path),
            read_from_alt_src: true,
        });
    }

    Ok(DataSourceSelection {
        path: path_to_lstring(path),
        read_from_alt_src: false,
    })
}

/// Data source container interface corresponding to `qlib::LDataSrcContainer`.
pub trait LDataSrcContainer {
    fn is_data_src_writable(&self) -> bool {
        false
    }

    fn data_chunk_reader_name(&self, _qdf_version: i32) -> LString {
        LString::new()
    }

    fn read_from_stream(&mut self, _input: &mut dyn InStream) -> Result<()> {
        Ok(())
    }

    fn write_data_chunk(&self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn update_src_path(&mut self, path: &LString);

    fn read_from_path(&mut self, path: impl AsRef<Path>) -> Result<()>
    where
        Self: Sized,
    {
        let mut input = FileInStream::new();
        input.open(path)?;
        let read_result = self.read_from_stream(&mut input);
        let close_result = input.close();
        read_result?;
        close_result
    }

    fn read_from_src_alt_src(
        &mut self,
        src: impl AsRef<str>,
        alt_src: impl AsRef<str>,
        base_path: impl AsRef<str>,
    ) -> Result<DataSourceSelection>
    where
        Self: Sized,
    {
        let selection = select_src_alt_src(src, alt_src, base_path)?;
        self.read_from_path(selection.path.as_str())?;
        Ok(selection)
    }
}

fn resolve_candidate(candidate: &str, base_path: &str) -> Result<PathBuf> {
    let candidate = Path::new(candidate);
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    if base_path.is_empty() {
        return Ok(PathBuf::new());
    }
    std::path::absolute(Path::new(base_path).join(candidate))
        .map_err(|error| CueMolError::Io(error.to_string()))
}

fn is_readable_file(path: &Path) -> bool {
    path.is_file()
}

fn path_to_lstring(path: PathBuf) -> LString {
    LString::from(path.to_string_lossy().into_owned())
}

fn cannot_open(path: &Path) -> CueMolError {
    CueMolError::Io(format!(
        "fatal error, cannot open file: \"{}\"",
        path.to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct ByteCollector {
        data: Vec<u8>,
        updated_path: LString,
    }

    impl LDataSrcContainer for ByteCollector {
        fn read_from_stream(&mut self, input: &mut dyn InStream) -> Result<()> {
            let mut buffer = [0_u8; 16];
            while input.ready()? {
                let size = buffer.len();
                let count = input.read(&mut buffer, 0, size)?;
                if count == 0 {
                    break;
                }
                self.data.extend_from_slice(&buffer[..count]);
            }
            Ok(())
        }

        fn update_src_path(&mut self, path: &LString) {
            self.updated_path = path.clone();
        }
    }

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cuemol-data-source-{}-{nonce}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn source_selection_prefers_primary_then_falls_back_to_alternate() {
        let dir = temp_dir();
        fs::write(dir.join("primary.dat"), b"primary").unwrap();
        fs::write(dir.join("alternate.dat"), b"alternate").unwrap();

        let primary =
            select_src_alt_src("primary.dat", "alternate.dat", dir.to_str().unwrap()).unwrap();
        assert!(!primary.read_from_alt_src);
        assert!(primary.path.as_str().ends_with("primary.dat"));

        let alternate =
            select_src_alt_src("missing.dat", "alternate.dat", dir.to_str().unwrap()).unwrap();
        assert!(alternate.read_from_alt_src);
        assert!(alternate.path.as_str().ends_with("alternate.dat"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn source_selection_reports_missing_candidates_and_requires_base_for_relative_alt() {
        let dir = temp_dir();
        assert!(select_src_alt_src("missing.dat", "", dir.to_str().unwrap()).is_err());
        assert!(select_src_alt_src("missing.dat", "alternate.dat", "").is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn data_source_container_reads_the_selected_file() {
        let dir = temp_dir();
        fs::write(dir.join("alternate.dat"), b"payload").unwrap();
        let mut collector = ByteCollector::default();

        let selection = collector
            .read_from_src_alt_src("missing.dat", "alternate.dat", dir.to_str().unwrap())
            .unwrap();
        assert!(selection.read_from_alt_src);
        assert_eq!(collector.data, b"payload");
        assert!(collector.updated_path.is_empty());

        fs::remove_dir_all(dir).unwrap();
    }
}
