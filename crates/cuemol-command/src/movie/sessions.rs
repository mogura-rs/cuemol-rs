use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, SystemTime},
};

const META: &str = ".cuemol-session.json";
const FORMAT: &str = "cuemol-movie-session-1";
const FRAME_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MOVIE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const SESSION_LIMIT: usize = 10;

/// The lock protects this run; dropping it deliberately leaves the output on disk.
pub struct MovieSession {
    directory: PathBuf,
    _lock: File,
}

impl Drop for MovieSession {
    fn drop(&mut self) {
        let _ = self._lock.unlock();
    }
}

impl MovieSession {
    pub fn create(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let directory = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(root)
            .map_err(|e| e.to_string())?;
        let mut lock = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(directory.path().join(META))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|e| e.to_string())?;
        let metadata = serde_json::json!({"format":FORMAT,"pid":std::process::id(),
            "startedAt":SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis()});
        lock.write_all(metadata.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            directory: directory.keep(),
            _lock: lock,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

pub fn session_directory() -> Result<&'static Path, String> {
    static SESSION: OnceLock<Result<MovieSession, String>> = OnceLock::new();
    SESSION
        .get_or_init(|| {
            let root = std::env::temp_dir().join("cuemol-movies");
            sweep(&root, SystemTime::now());
            MovieSession::create(&root)
        })
        .as_ref()
        .map(MovieSession::directory)
        .map_err(Clone::clone)
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct SweepResult {
    pub removed_frames: usize,
    pub removed_directories: usize,
}

struct PastSession {
    directory: PathBuf,
    lock: File,
    files: Vec<(PathBuf, SystemTime)>,
    newest_movie: Option<SystemTime>,
}

impl PastSession {
    fn open(directory: PathBuf) -> Option<Self> {
        if !fs::symlink_metadata(&directory).ok()?.is_dir() {
            return None;
        }
        let marker = directory.join(META);
        if !fs::symlink_metadata(&marker).ok()?.is_file() {
            return None;
        }
        let mut lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(marker)
            .ok()?;
        lock.try_lock().ok()?;
        let mut text = String::new();
        (&mut lock).take(4096).read_to_string(&mut text).ok()?;
        if serde_json::from_str::<serde_json::Value>(&text).ok()?["format"] != FORMAT {
            return None;
        }
        let mut files = Vec::new();
        let mut newest_movie = None;
        for entry in fs::read_dir(&directory).ok()? {
            let entry = entry.ok()?;
            let meta = fs::symlink_metadata(entry.path()).ok()?;
            let name = entry.file_name();
            let name = name.to_str()?;
            // Unknown files, nested directories, and symlinks are never swept.
            if !meta.is_file()
                || !(name == META
                    || super::files::is_frame(name)
                    || super::files::is_movie(name)
                    || super::files::is_pending(name)
                    || super::files::is_output_lock(name))
            {
                return None;
            }
            let modified = meta.modified().ok()?;
            if super::files::is_movie(name) && !super::files::is_pending(name) {
                newest_movie = Some(
                    newest_movie.map_or(modified, |previous: SystemTime| previous.max(modified)),
                );
            }
            files.push((entry.path(), modified));
        }
        Some(Self {
            directory,
            lock,
            files,
            newest_movie,
        })
    }

    fn remove(self) -> bool {
        for (path, _) in &self.files {
            if path.file_name().is_some_and(|name| name == META) {
                continue;
            }
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return false;
                }
            }
        }
        // Windows cannot remove the marker while its lock handle remains open.
        drop(self.lock);
        if fs::remove_file(self.directory.join(META)).is_err() {
            return false;
        }
        fs::remove_dir(self.directory).is_ok()
    }
}

pub fn sweep(root: &Path, now: SystemTime) -> SweepResult {
    let mut result = SweepResult::default();
    let Ok(entries) = fs::read_dir(root) else {
        return result;
    };
    let old = |modified, limit| now.duration_since(modified).is_ok_and(|age| age > limit);
    let mut with_movie = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("session-") {
            continue;
        }
        let Some(session) = PastSession::open(entry.path()) else {
            continue;
        };
        for (path, modified) in &session.files {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| super::files::is_frame(name) || super::files::is_pending(name))
                && old(*modified, FRAME_TTL)
                && fs::remove_file(path).is_ok()
            {
                result.removed_frames += 1;
            }
        }
        let expired = match session.newest_movie {
            Some(time) => old(time, MOVIE_TTL),
            None => session
                .files
                .iter()
                .map(|(_, time)| *time)
                .max()
                .is_some_and(|time| old(time, FRAME_TTL)),
        };
        if expired {
            result.removed_directories += usize::from(session.remove());
        } else if session.newest_movie.is_some() {
            with_movie.push(session);
        }
    }
    with_movie.sort_by_key(|session| std::cmp::Reverse(session.newest_movie));
    for session in with_movie.into_iter().skip(SESSION_LIMIT) {
        result.removed_directories += usize::from(session.remove());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamp(path: &Path, time: SystemTime) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(time))
            .unwrap();
    }

    fn session(root: &Path, now: SystemTime, days: u64, movie: bool) -> MovieSession {
        let session = MovieSession::create(root).unwrap();
        fs::write(session.directory.join("movie_frm_0000.png"), b"frame").unwrap();
        if movie {
            fs::write(session.directory.join("movie.mp4"), b"movie").unwrap();
        }
        for entry in fs::read_dir(&session.directory).unwrap() {
            timestamp(
                &entry.unwrap().path(),
                now - Duration::from_secs(days * 24 * 3600),
            );
        }
        session
    }

    #[test]
    fn active_sessions_and_unknown_content_are_kept_while_expired_frames_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let active = session(root.path(), now, 40, true);
        let past = session(root.path(), now, 2, true);
        let past_path = past.directory.clone();
        let pending = past_path.join(".cuemol-movie-pending-crash.png");
        fs::write(&pending, b"unfinished frame").unwrap();
        timestamp(&pending, now - Duration::from_secs(2 * 24 * 3600));
        fs::write(past_path.join(".movie.cuemol-movie-lock"), b"").unwrap();
        drop(past);
        let unknown = session(root.path(), now, 40, true);
        let unknown_path = unknown.directory.clone();
        fs::write(unknown_path.join("keep.txt"), b"keep").unwrap();
        drop(unknown);
        let unmarked = root.path().join("session-not-ours");
        fs::create_dir(&unmarked).unwrap();
        fs::write(unmarked.join("movie.mp4"), b"keep").unwrap();
        let swept = sweep(root.path(), now);
        assert_eq!(
            swept,
            SweepResult {
                removed_frames: 2,
                removed_directories: 0
            }
        );
        assert!(active.directory.join("movie_frm_0000.png").exists());
        assert!(past_path.join("movie.mp4").exists());
        assert!(!past_path.join("movie_frm_0000.png").exists());
        assert!(unknown_path.join("movie_frm_0000.png").exists());
        assert!(unmarked.join("movie.mp4").exists());
        let active_path = active.directory.clone();
        drop(active);
        assert!(active_path.exists());
        assert_eq!(sweep(root.path(), now).removed_directories, 1);
        assert!(!active_path.exists());
    }

    #[test]
    fn retention_uses_movie_age_and_limits_only_inactive_sessions() {
        let root = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let no_movie = session(root.path(), now, 2, false);
        let no_movie_path = no_movie.directory.clone();
        drop(no_movie);
        let active = session(root.path(), now, 20, true);
        let mut paths = Vec::new();
        for day in 0..12 {
            let past = session(root.path(), now, day, true);
            paths.push(past.directory.clone());
        }
        let result = sweep(root.path(), now);
        assert_eq!(result.removed_directories, 3);
        assert!(!no_movie_path.exists());
        assert!(active.directory.exists());
        assert!(paths[..10].iter().all(|path| path.exists()));
        assert!(paths[10..].iter().all(|path| !path.exists()));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_sessions_and_contents_are_never_followed() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let past = session(outside.path(), now, 40, true);
        let path = past.directory.clone();
        drop(past);
        symlink(&path, root.path().join("session-link")).unwrap();
        assert_eq!(sweep(root.path(), now), SweepResult::default());
        assert!(path.join("movie.mp4").exists());
        let nested = session(root.path(), now, 40, false);
        let nested_path = nested.directory.clone();
        symlink(path.join("movie.mp4"), nested_path.join("movie.mp4")).unwrap();
        drop(nested);
        assert_eq!(sweep(root.path(), now), SweepResult::default());
        assert!(nested_path.join("movie_frm_0000.png").exists());
    }
}
