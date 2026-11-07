use super::*;
use crate::image_job::PreparedImageExport;
use cuemol_command::image_export::{ImageExportKind, ImageExportTarget};
use serde_json::{json, Value};

#[derive(Default)]
pub(super) struct ImageExports {
    next_id: u64,
    session: Option<Session>,
}

struct Session {
    id: String,
    target: ImageExportTarget,
    selection: String,
    cancel: Arc<AtomicBool>,
    receiver: Option<mpsc::Receiver<Result<(), String>>>,
    result: Option<Result<(), String>>,
    path: String,
    released: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl ImageExports {
    pub(super) fn pending(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.receiver.is_some())
    }

    pub(super) fn poll(&mut self) -> Option<String> {
        let session = self.session.as_mut()?;
        let result = match session.receiver.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("Image export worker stopped".into()),
        };
        let status = match &result {
            Ok(()) => format!("Exported image: {}", session.path),
            Err(_) if session.cancel.load(Ordering::Relaxed) => "Image export cancelled".into(),
            Err(error) => error.clone(),
        };
        session.receiver = None;
        session.result = Some(result);
        if session.released {
            self.session = None;
        }
        Some(status)
    }

    fn request(
        &mut self,
        model: &AppModel,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        if operation == "begin" {
            if self.pending() {
                return Err("An image export is already running".into());
            }
            let target = ImageExportTarget::capture(&model.desktop, &model.camera, &json!({}))
                .map_err(|e| e.to_string())?;
            self.next_id += 1;
            let id = self.next_id.to_string();
            self.session = Some(Session {
                id: id.clone(),
                target,
                selection: model.selection.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
                receiver: None,
                result: None,
                path: String::new(),
                released: false,
            });
            return Ok(json!({"id": id, "width": model.view_width, "height": model.view_height}));
        }
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .ok_or("Image export ID is required")?;
        if operation == "release" && !self.session.as_ref().is_some_and(|s| s.id == id) {
            return Ok(json!({}));
        }
        let session = self
            .session
            .as_mut()
            .filter(|s| s.id == id)
            .ok_or("Image export session has expired")?;
        match operation {
            "start" => {
                if session.receiver.is_some() || session.result.is_some() || session.released {
                    return Err("Image export has already started".into());
                }
                if session.cancel.load(Ordering::Relaxed) {
                    return Err("Image export cancelled".into());
                }
                let kind = match args.get("kind").and_then(Value::as_str) {
                    Some("png") => ImageExportKind::Png,
                    Some("umbreon") => ImageExportKind::Umbreon,
                    _ => return Err("Unknown image export format".into()),
                };
                let dimension = |name| -> Result<u32, String> {
                    args.get(name)
                        .and_then(Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok())
                        .ok_or_else(|| format!("Invalid image {name}"))
                };
                let options = crate::GuiPngExportOptions {
                    width: dimension("width")?,
                    height: dimension("height")?,
                    resolution_dpi: args
                        .get("resolutionDpi")
                        .and_then(Value::as_f64)
                        .ok_or("Image resolution is required")?,
                    transparent: args
                        .get("transparent")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                let path = args
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .ok_or("An image output path is required")?
                    .to_string();
                let prepared = PreparedImageExport::capture(
                    &model.desktop,
                    &session.target,
                    kind,
                    options,
                    &session.selection,
                )
                .map_err(|e| e.to_string())?;
                let (sender, receiver) = mpsc::channel();
                let cancel = session.cancel.clone();
                let output = PathBuf::from(&path);
                std::thread::Builder::new()
                    .name("cuemol-image-export".into())
                    .spawn(move || {
                        let result = prepared.write(&output, &cancel).map_err(|e| e.to_string());
                        let _ = sender.send(result);
                    })
                    .map_err(|e| e.to_string())?;
                session.receiver = Some(receiver);
                session.path = path;
            }
            "cancel" => session.cancel.store(true, Ordering::Relaxed),
            "release" => {
                session.cancel.store(true, Ordering::Relaxed);
                if session.receiver.is_none() {
                    self.session = None;
                } else {
                    session.released = true;
                }
                return Ok(json!({}));
            }
            "poll" => {}
            _ => return Err(format!("Unknown image export operation: {operation}")),
        }
        let error = session
            .result
            .as_ref()
            .and_then(|result| result.as_ref().err());
        let cancelled = error.is_some() && session.cancel.load(Ordering::Relaxed);
        Ok(json!({"id":id, "running":session.receiver.is_some(),
            "completed":session.result.is_some(), "cancelled":cancelled,
            "error":if cancelled { None } else { error }}))
    }
}

impl AppModel {
    pub(super) fn poll_image_export(&mut self) {
        if let Some(status) = self.image_exports.poll() {
            self.status = status;
        }
    }

    pub(super) fn image_export_request(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        self.poll_image_export();
        let mut exports = std::mem::take(&mut self.image_exports);
        let result = exports.request(self, operation, args);
        self.image_exports = exports;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(model: &AppModel) -> Value {
        json!({"scene":model.desktop.active_scene().unwrap().get(),
            "camera":model.camera.to_xml_string(), "stored":model.desktop.active_view_camera().unwrap().to_xml_string(),
            "modified":model.desktop.modified_scene_uids().into_iter().map(|id| id.get()).collect::<Vec<_>>(),
            "undo":model.undo_stack.len(), "redo":model.redo_stack.len()})
    }

    #[test]
    fn image_export_sessions_preserve_state_and_reject_stale_or_deleted_targets() {
        let mut model = super::super::tests::model_without_gpu();
        model.camera.set_zoom(73.);
        let before = state(&model);
        let first = model.image_export_request("begin", &json!({})).unwrap();
        assert_eq!(state(&model), before);
        model.image_export_request("release", &first).unwrap();
        assert_eq!(state(&model), before);
        let second = model.image_export_request("begin", &json!({})).unwrap();
        model.image_export_request("release", &first).unwrap();
        assert!(model.image_exports.session.is_some());
        assert!(model.image_export_request("poll", &first).is_err());
        model
            .desktop
            .destroy_scene(model.desktop.active_scene().unwrap());
        let result = model.image_export_request("start", &json!({"id":second["id"], "kind":"umbreon", "path":"unused.png", "width":32,"height":24,"resolutionDpi":150.}));
        assert!(result.unwrap_err().contains("scene or view"));
        model.image_export_request("release", &second).unwrap();
    }

    #[test]
    fn image_export_worker_keeps_captured_camera_and_live_edits() {
        let mut model = super::super::tests::model_without_gpu();
        model.desktop.open_bytes("atom.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        cuemol_command::call_tool(&mut model.desktop, &mut model.camera, "set_render_settings", &json!({"settings":"backend=povray; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1"})).unwrap();
        model.camera.set_zoom(4.);
        let session = model.image_export_request("begin", &json!({})).unwrap();
        let options = crate::GuiPngExportOptions {
            width: 32,
            height: 24,
            resolution_dpi: 150.,
            transparent: true,
        };
        let expected = model
            .image_exports
            .session
            .as_ref()
            .unwrap()
            .target
            .prepare_umbreon(&model.desktop, options)
            .unwrap()
            .run_png_with_cancel(&AtomicBool::new(false))
            .unwrap();
        model.desktop.new_scene("Other", true);
        cuemol_command::call_tool(&mut model.desktop, &mut model.camera, "set_render_settings", &json!({"settings":"umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1"})).unwrap();
        model.camera.set_zoom(121.);
        let before = state(&model);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image with spaces.png");
        let start = model.image_export_request("start", &json!({"id":session["id"],"kind":"umbreon","path":path,"width":32,"height":24,"resolutionDpi":150.,"transparent":true})).unwrap();
        assert_eq!(start["running"], true);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while model.image_exports.pending() && std::time::Instant::now() < until {
            model.poll_image_export();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = model.image_export_request("poll", &session).unwrap();
        assert_eq!(result["completed"], true);
        assert!(result["error"].is_null(), "{result}");
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert_eq!(state(&model), before);
        model.image_export_request("release", &session).unwrap();
        assert!(!model.image_exports.pending());
        let failed = model.image_export_request("begin", &json!({})).unwrap();
        let missing = dir.path().join("missing/image.png");
        model.image_export_request("start", &json!({"id":failed["id"],"kind":"umbreon","path":missing,"width":32,"height":24,"resolutionDpi":150.,"transparent":true})).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while model.image_exports.pending() && std::time::Instant::now() < until {
            model.poll_image_export();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = model.image_export_request("poll", &failed).unwrap();
        assert_eq!(result["completed"], true);
        assert!(result["error"].as_str().is_some(), "{result}");
        assert!(!missing.exists());
        assert_eq!(state(&model), before);
        model.image_export_request("release", &failed).unwrap();
    }

    #[test]
    fn image_export_cancel_releases_worker_and_preserves_existing_file() {
        let mut model = super::super::tests::model_without_gpu();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.png");
        fs::write(&path, b"existing image").unwrap();
        let before = state(&model);
        let session = model.image_export_request("begin", &json!({})).unwrap();
        model.image_export_request("start", &json!({"id":session["id"],"kind":"umbreon","path":path,"width":1024,"height":768,"resolutionDpi":150.,"transparent":true})).unwrap();
        model.image_export_request("cancel", &session).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while model.image_exports.pending() && std::time::Instant::now() < until {
            model.poll_image_export();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = model.image_export_request("poll", &session).unwrap();
        assert_eq!(result["completed"], true);
        assert_eq!(result["cancelled"], true);
        assert!(result["error"].is_null());
        model.image_export_request("release", &session).unwrap();
        assert!(model.image_exports.session.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"existing image");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(state(&model), before);
    }
}
