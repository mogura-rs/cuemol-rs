use super::{files, MovieOptions, MovieResult, MovieScene};
use crate::{render_history::RenderSource, PreparedRender};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

pub struct MovieOutcome {
    pub movie: MovieResult,
    pub source: RenderSource,
    pub size: [u32; 2],
    pub elapsed_seconds: f64,
    pub error: Option<String>,
}

pub enum MovieEvent {
    Frame { index: u32, path: PathBuf },
    Finished(MovieOutcome),
}

enum Worker {
    Frame(mpsc::Receiver<Result<PathBuf, String>>),
    Encode(mpsc::Receiver<Result<PathBuf, String>>),
}

struct OutputLease {
    _file: File,
}

impl Drop for OutputLease {
    fn drop(&mut self) {
        let _ = self._file.unlock();
    }
}

impl OutputLease {
    fn acquire(directory: &Path, base: &str) -> Result<Arc<Self>, String> {
        let path = directory.join(format!(".{base}.cuemol-movie-lock"));
        if fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_file()) {
            return Err("Movie output lock is not a regular file".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock().map_err(|_| {
            "Another movie job is using this output folder and base name".to_string()
        })?;
        Ok(Arc::new(Self { _file: file }))
    }
}

pub struct MovieJob {
    scene: Option<MovieScene>,
    source: RenderSource,
    size: [u32; 2],
    options: MovieOptions,
    directory: PathBuf,
    base: String,
    total: u32,
    completed: u32,
    worker: Option<Worker>,
    cancel: Arc<AtomicBool>,
    lease: Arc<OutputLease>,
    ffmpeg: Option<PathBuf>,
    started: Instant,
    done: bool,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl MovieJob {
    pub fn start(
        state: &DesktopAppState,
        camera: &Camera,
        target: &Value,
        options: MovieOptions,
        size: [u32; 2],
        overwrite: bool,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        options.validate()?;
        let mut scene = MovieScene::capture(state, camera, target).map_err(|e| e.to_string())?;
        let total = super::frame_count(scene.duration(), options.fps, options.dup_last_frame)?;
        let mut source = scene.source().map_err(|e| e.to_string())?;
        source
            .settings
            .insert("width".into(), serde_json::json!(size[0]));
        source
            .settings
            .insert("height".into(), serde_json::json!(size[1]));
        source
            .settings
            .insert("unit".into(), serde_json::json!("px"));
        let frame = scene
            .prepare_frame(0, options.fps, size)
            .map_err(|e| e.to_string())?;
        let ffmpeg = if options.make_movie {
            Some(resolve_encoder(state)?)
        } else {
            None
        };
        let (directory, base, lease) = Self::output(&options)?;
        let mut options = options;
        options.output_dir = directory.to_string_lossy().into_owned();
        if files::has_outputs(&directory, &base)? && !overwrite {
            return Err("Existing movie outputs require confirmation before replacement".into());
        }
        files::cleanup_animation_outputs(&directory, &base)?;
        let mut job = Self {
            scene: Some(scene),
            source,
            size,
            options,
            directory,
            base,
            total,
            completed: 0,
            worker: None,
            cancel: Arc::new(AtomicBool::new(false)),
            lease,
            ffmpeg,
            started: Instant::now(),
            done: false,
            wake,
        };
        job.spawn_frame(frame)?;
        Ok(job)
    }

    pub fn reencode(
        state: &DesktopAppState,
        options: MovieOptions,
        source: RenderSource,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        options.validate()?;
        if !options.make_movie {
            return Err("Movie encoding is disabled".into());
        }
        let ffmpeg = Some(resolve_encoder(state)?);
        let (directory, base, lease) = Self::output(&options)?;
        let mut options = options;
        options.output_dir = directory.to_string_lossy().into_owned();
        let total = files::animation_frame_count(&directory, &base)?;
        if total == 0 {
            return Err("No complete frame sequence found in the output folder".into());
        }
        let size = image::image_dimensions(files::frame_path(&directory, &base, 0)?)
            .map_err(|e| e.to_string())?;
        let mut job = Self {
            scene: None,
            source,
            size: [size.0, size.1],
            options,
            directory,
            base,
            total,
            completed: total,
            worker: None,
            cancel: Arc::new(AtomicBool::new(false)),
            lease,
            ffmpeg,
            started: Instant::now(),
            done: false,
            wake,
        };
        job.spawn_encode()?;
        Ok(job)
    }

    fn output(options: &MovieOptions) -> Result<(PathBuf, String, Arc<OutputLease>), String> {
        let directory = if options.use_temp_dir {
            super::sessions::session_directory()?.to_path_buf()
        } else {
            if options.output_dir.trim().is_empty() {
                return Err("Choose a movie output folder".into());
            }
            PathBuf::from(options.output_dir.trim())
        };
        if !directory.is_dir() {
            return Err("Movie output folder does not exist".into());
        }
        let directory = directory.canonicalize().map_err(|e| e.to_string())?;
        let base = super::base_name(&options.base_name)?.to_string();
        let lease = OutputLease::acquire(&directory, &base)?;
        Ok((directory, base, lease))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn completed(&self) -> u32 {
        self.completed
    }
    pub fn total(&self) -> u32 {
        self.total
    }
    pub fn elapsed_seconds(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
    pub fn cleanup(options: &MovieOptions) -> Result<usize, String> {
        let (directory, base, _lease) = Self::output(options)?;
        files::cleanup_animation_outputs(&directory, &base)
    }
    pub fn message(&self) -> String {
        if self.cancel.load(Ordering::Relaxed) {
            "Stopping movie render…".into()
        } else if matches!(self.worker, Some(Worker::Encode(_))) {
            format!("Encoding {} frames…", self.total)
        } else {
            format!(
                "Rendering frame {} / {}",
                (self.completed + 1).min(self.total),
                self.total
            )
        }
    }

    fn spawn_frame(&mut self, frame: PreparedRender) -> Result<(), String> {
        let temporary = tempfile::Builder::new()
            .prefix(".cuemol-movie-pending-")
            .suffix(".png")
            .tempfile_in(&self.directory)
            .map_err(|e| e.to_string())?;
        let path = files::frame_path(&self.directory, &self.base, self.completed)?;
        let cancel = self.cancel.clone();
        let lease = self.lease.clone();
        let wake = self.wake.clone();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _lease = lease;
            let result = frame
                .to_file(temporary.path())
                .run_with_cancel(&cancel)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Movie render cancelled".into());
                    }
                    temporary.persist(&path).map_err(|e| e.to_string())?;
                    Ok(path)
                });
            let _ = send.send(result);
            wake();
        });
        self.worker = Some(Worker::Frame(receive));
        Ok(())
    }

    fn spawn_encode(&mut self) -> Result<(), String> {
        let executable = self
            .ffmpeg
            .clone()
            .ok_or("No FFmpeg executable is configured")?;
        let path = files::animation_movie_output_path(
            &self.directory,
            &self.base,
            &self.options.movie_format,
        )?;
        let args = files::animation_ffmpeg_args(
            &self.base,
            self.options.fps,
            self.total,
            &self.options.movie_format,
            self.options.bitrate_kbps,
        )?;
        let directory = self.directory.clone();
        let cancel = self.cancel.clone();
        let lease = self.lease.clone();
        let wake = self.wake.clone();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _lease = lease;
            let result = encode(&executable, &directory, &path, args, &cancel);
            let _ = send.send(result);
            wake();
        });
        self.worker = Some(Worker::Encode(receive));
        Ok(())
    }

    pub fn poll(&mut self) -> Option<MovieEvent> {
        if self.done {
            return None;
        }
        let Some(worker) = &self.worker else {
            return Some(self.finish(None, None));
        };
        let (receiver, encoding) = match worker {
            Worker::Frame(receiver) => (receiver, false),
            Worker::Encode(receiver) => (receiver, true),
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("Movie worker stopped".into()),
        };
        self.worker = None;
        if self.cancel.load(Ordering::Relaxed) {
            return Some(self.finish(None, Some("Movie render cancelled".into())));
        }
        let path = match result {
            Ok(path) => path,
            Err(error) => return Some(self.finish(None, Some(error))),
        };
        if encoding {
            return Some(self.finish(Some(path), None));
        }
        let index = self.completed;
        self.completed += 1;
        let next = if self.completed < self.total {
            self.scene
                .as_mut()
                .unwrap()
                .prepare_frame(self.completed, self.options.fps, self.size)
                .map_err(|e| e.to_string())
                .and_then(|frame| self.spawn_frame(frame))
        } else if self.options.make_movie {
            self.spawn_encode()
        } else {
            Ok(())
        };
        if let Err(error) = next {
            return Some(self.finish(None, Some(error)));
        }
        Some(MovieEvent::Frame { index, path })
    }

    fn finish(&mut self, movie: Option<PathBuf>, error: Option<String>) -> MovieEvent {
        self.done = true;
        MovieEvent::Finished(MovieOutcome {
            movie: MovieResult {
                output_dir: self.directory.to_string_lossy().into_owned(),
                base_name: self.base.clone(),
                frame_count: self.completed,
                fps: self.options.fps,
                movie_path: movie.map(|path| path.to_string_lossy().into_owned()),
                options: self.options.clone(),
            },
            source: self.source.clone(),
            size: self.size,
            elapsed_seconds: self.elapsed_seconds(),
            error,
        })
    }
}

impl Drop for MovieJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn resolve_encoder(state: &DesktopAppState) -> Result<PathBuf, String> {
    let path = crate::external::resolve(state, "ffmpeg").map_err(|e| e.to_string())?;
    if !path.is_file() {
        return Err("Configured FFmpeg executable does not exist".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("Configured FFmpeg file is not executable".into());
        }
    }
    Ok(path)
}

fn encode(
    executable: &Path,
    directory: &Path,
    output: &Path,
    mut args: Vec<String>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let suffix = format!(
        ".{}",
        output
            .extension()
            .and_then(|value| value.to_str())
            .ok_or("Invalid movie output name")?
    );
    let temporary = tempfile::Builder::new()
        .prefix(".cuemol-movie-pending-")
        .suffix(&suffix)
        .tempfile_in(directory)
        .map_err(|e| e.to_string())?;
    *args.last_mut().unwrap() = temporary.path().to_string_lossy().into_owned();
    args.splice(
        0..0,
        ["-nostdin".into(), "-loglevel".into(), "error".into()],
    );
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::external::configure_command(&mut command, executable);
    let mut child = crate::external::spawn_command(&mut command)
        .map_err(|e| format!("Cannot start FFmpeg: {e}"))?;
    let mut stderr = child.stderr.take().unwrap();
    let log = std::thread::spawn(move || {
        let mut result = Vec::new();
        let mut block = [0; 8192];
        while let Ok(count) = stderr.read(&mut block) {
            if count == 0 {
                break;
            }
            let keep = count.min((64 * 1024usize).saturating_sub(result.len()));
            result.extend_from_slice(&block[..keep]);
        }
        String::from_utf8_lossy(&result).into_owned()
    });
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Movie encoding cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!("FFmpeg exited with {status}"))
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.to_string());
            }
        }
    };
    let log = log.join().unwrap_or_default();
    result.map_err(|error| {
        if log.is_empty() {
            error
        } else {
            format!("{error}\n{log}")
        }
    })?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Movie encoding cancelled".into());
    }
    if !fs::metadata(temporary.path()).is_ok_and(|meta| meta.is_file() && meta.len() > 0) {
        return Err("FFmpeg produced no movie".into());
    }
    temporary.persist(output).map_err(|e| e.to_string())?;
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(directory: &Path) {
        for index in 0..3 {
            image::RgbaImage::from_pixel(32, 24, image::Rgba([index * 60, 40, 200, 255]))
                .save(files::frame_path(directory, "movie", index as u32).unwrap())
                .unwrap();
        }
    }

    fn options(directory: &Path) -> MovieOptions {
        let mut options = MovieOptions::default();
        options.set_custom(directory.to_string_lossy().into_owned());
        options
    }

    fn finish(job: &mut MovieJob) -> MovieOutcome {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match job.poll() {
                Some(MovieEvent::Finished(result)) => return result,
                _ => assert!(Instant::now() < deadline, "movie job timed out"),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn script(directory: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn failed_and_cancelled_encoding_keep_previous_movie_and_release_output_lock() {
        let directory = tempfile::tempdir().unwrap();
        frames(directory.path());
        let output = directory.path().join("movie.mp4");
        fs::write(&output, b"previous movie").unwrap();
        let fail = script(directory.path(), "fail-encoder", "echo failure >&2; exit 7");
        let mut state = DesktopAppState::new();
        state.put_config_value("cuemol2.ui.ffmpeg-path", fail.to_string_lossy());
        let options = options(directory.path());
        let mut job = MovieJob::reencode(
            &state,
            options.clone(),
            RenderSource::default(),
            Arc::new(|| {}),
        )
        .unwrap();
        assert!(MovieJob::cleanup(&options).is_err());
        assert!(MovieJob::reencode(
            &state,
            options.clone(),
            RenderSource::default(),
            Arc::new(|| {})
        )
        .is_err());
        let outcome = finish(&mut job);
        assert!(outcome.error.as_ref().unwrap().contains("failure"));
        assert_eq!(outcome.movie.frame_count, 3);
        assert!(outcome.movie.movie_path.is_none());
        assert_eq!(fs::read(&output).unwrap(), b"previous movie");
        drop(job);
        let slow = script(
            directory.path(),
            "slow-encoder",
            "echo $$ > encoder.pid; exec sleep 30",
        );
        state.put_config_value("cuemol2.ui.ffmpeg-path", slow.to_string_lossy());
        let mut job = MovieJob::reencode(
            &state,
            options.clone(),
            RenderSource::default(),
            Arc::new(|| {}),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let pid_path = directory.path().join("encoder.pid");
        while !pid_path.exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        job.cancel();
        assert!(finish(&mut job).error.unwrap().contains("cancelled"));
        let pid = fs::read_to_string(pid_path).unwrap();
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!alive.success(), "cancelled encoder process is still alive");
        assert_eq!(fs::read(&output).unwrap(), b"previous movie");
        drop(job);
        assert_eq!(MovieJob::cleanup(&options).unwrap(), 4);
        assert!(fs::read_dir(directory.path())
            .unwrap()
            .all(|entry| !files::is_pending(&entry.unwrap().file_name().to_string_lossy())));
    }

    #[test]
    fn missing_encoder_and_invalid_scene_leave_existing_output_intact() {
        let directory = tempfile::tempdir().unwrap();
        frames(directory.path());
        let before = fs::read(directory.path().join("movie_frm_0000.png")).unwrap();
        let mut state = DesktopAppState::new();
        state.put_config_value(
            "cuemol2.ui.ffmpeg-path",
            directory.path().join("missing").to_string_lossy(),
        );
        let options = options(directory.path());
        assert!(MovieJob::reencode(
            &state,
            options.clone(),
            RenderSource::default(),
            Arc::new(|| {})
        )
        .is_err());
        assert!(MovieJob::start(
            &state,
            &Camera::new(),
            &serde_json::json!({}),
            options,
            [32, 24],
            true,
            Arc::new(|| {})
        )
        .is_err());
        assert_eq!(
            fs::read(directory.path().join("movie_frm_0000.png")).unwrap(),
            before
        );
    }

    #[test]
    #[ignore = "Requires FFmpeg with all supported movie codecs and ffprobe"]
    fn ffmpeg_reencodes_all_seven_formats_from_the_existing_frames() {
        let directory = tempfile::tempdir().unwrap();
        frames(directory.path());
        let state = DesktopAppState::new();
        for (format, _) in super::super::FORMATS {
            let mut options = options(directory.path());
            options.movie_format = format.into();
            options.fps = 12.5;
            let mut job =
                MovieJob::reencode(&state, options, RenderSource::default(), Arc::new(|| {}))
                    .unwrap();
            let result = finish(&mut job);
            assert!(result.error.is_none(), "{format}: {:?}", result.error);
            let movie = result.movie.movie_path.unwrap();
            let probe = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-count_frames",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=nb_read_frames,width,height",
                    "-of",
                    "json",
                    &movie,
                ])
                .output()
                .unwrap();
            assert!(probe.status.success(), "{format}");
            let metadata: Value = serde_json::from_slice(&probe.stdout).unwrap();
            assert_eq!(metadata["streams"][0]["nb_read_frames"], "3", "{format}");
            assert_eq!(metadata["streams"][0]["width"], 32);
            assert_eq!(metadata["streams"][0]["height"], 24);
        }
    }
}
