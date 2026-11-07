use super::*;

impl App {
    pub(super) fn save_current_view_download(&mut self) {
        let result = (|| -> Result<(), String> {
            let prepared = cuemol_command::prepare_current_view(&self.state, &self.camera)
                .map_err(|error| error.to_string())?;
            download("view.cam", prepared.xml.as_bytes(), "application/xml").map_err(js_error)?;
            prepared
                .finish(&mut self.state)
                .map_err(|error| error.to_string())
        })();
        self.message = match result {
            Ok(()) => "Saved current view to view.cam".into(),
            Err(error) => error,
        };
    }

    pub(super) fn prune_camera_files(&mut self) {
        if self.camera_files.is_empty() {
            return;
        }
        let mut retained = std::collections::BTreeSet::new();
        for scene in self.state.scene_summaries() {
            for camera in self.state.camera_summaries(scene.uid) {
                retained.insert((scene.uid, camera.source));
            }
        }
        for entry in self.history.iter().chain(&self.future) {
            retained.extend(entry.camera_sources.iter().cloned());
        }
        self.camera_files.retain(|key, _| retained.contains(key));
    }

    pub(super) fn save_camera_file(&mut self) {
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        let Some(name) = self.camera_panel.selected.as_deref() else {
            return;
        };
        match self.state.scene_camera_xml(scene, name) {
            Ok(xml) => {
                if let Err(error) =
                    download(&format!("{name}.cam"), xml.as_bytes(), "application/xml")
                {
                    self.message = js_error(error);
                }
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    pub(super) fn reload_camera_file(&mut self) {
        if self.busy || self.source_active {
            self.message = "Wait for the current file to finish loading.".into();
            return;
        }
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        let Some(name) = self.camera_panel.selected.clone() else {
            return;
        };
        let Some(source) = self
            .state
            .active_camera_summaries()
            .iter()
            .find(|camera| camera.name == name)
            .map(|camera| camera.source.clone())
        else {
            return;
        };
        let Some(file) = self.camera_files.get(&(scene, source)).cloned() else {
            self.message = "Open the camera file in this tab before reloading it.".into();
            return;
        };
        self.queue_request(loading::Source::Camera(file, name));
    }

    pub(super) fn apply_camera_file(
        &mut self,
        file: web_sys::File,
        xml: &str,
        replace: Option<&str>,
    ) {
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        let before = self.capture_history();
        let result = self
            .state
            .import_scene_camera_xml(scene, &file.name(), xml, replace);
        match result {
            Ok(name) => {
                if replace.is_none() {
                    if let Ok(camera) = self.state.load_scene_camera(scene, &name, true) {
                        self.camera = camera;
                    }
                }
                self.camera_files.insert((scene, file.name()), file);
                self.camera_panel.selected = Some(name.clone());
                self.clipboard_camera = Some(name.clone());
                if !before.state.same_scene_undo_state(&self.state) {
                    self.state.mark_scene_changes_since(&before.state);
                    self.push_history(before);
                    self.revision = self.revision.wrapping_add(1);
                }
                self.prune_camera_files();
                self.dirty = true;
                self.remember_source();
                self.message = format!("Loaded camera {name}");
            }
            Err(error) => self.message = error.to_string(),
        }
    }
}
