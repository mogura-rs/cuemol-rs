use super::*;

pub(super) struct Import {
    pub generation: u64,
    pub scene: Option<Uid>,
    pub object: Uid,
    pub revision: u64,
    pub insert_before: Option<usize>,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name=cuemolChooseMorphFiles)]
    fn choose_morph_files(generation: f64);
}

impl App {
    pub(super) fn open_morph(&mut self) {
        self.cancel_morph_import();
        self.morph_editor.name_width = self
            .state
            .config_value("morph_editor:name_width")
            .parse::<f32>()
            .ok()
            .filter(|width| width.is_finite())
            .unwrap_or(cuemol_ui::morph::NAME_COLUMN_DEFAULT_WIDTH)
            .clamp(cuemol_ui::morph::COLUMN_MIN_WIDTH, 600.);
        self.morph_editor.opened(&self.state);
        self.show_morph = true;
    }

    fn close_morph(&mut self) {
        self.cancel_morph_import();
        self.show_morph = false;
        self.morph_editor.generation = self.morph_editor.generation.wrapping_add(1);
    }

    fn cancel_morph_import(&mut self) {
        self.source_queue
            .retain(|source| !matches!(source, loading::Source::Morph(_, _)));
        if self.source_morph.is_some() {
            let remaining = std::mem::take(&mut self.source_queue);
            self.cancel_load();
            for source in remaining {
                self.queue_request(source);
            }
        }
    }

    pub(super) fn queue_morph_files(&mut self, generation: u64, files: Vec<web_sys::File>) {
        if !self.show_morph || self.morph_editor.generation != generation {
            return;
        }
        let object = match self.morph_editor.target(&self.state) {
            Ok(object) if self.state.morph_frame_value(object).is_some() => object,
            _ => return,
        };
        self.cancel_console("Morph frames added; remaining console commands cancelled.");
        let batch = Rc::new(RefCell::new(Import {
            generation,
            scene: self.state.active_scene(),
            object,
            revision: self.revision,
            insert_before: self.morph_editor.insert_before(),
        }));
        for file in files {
            self.queue_request(loading::Source::Morph(file, batch.clone()));
        }
    }

    pub(super) fn current_morph_import(&self, import: &Import) -> bool {
        self.show_morph
            && self.morph_editor.generation == import.generation
            && self.state.active_scene() == import.scene
            && self.revision == import.revision
            && self.morph_editor.object == Some(import.object)
            && self.morph_editor.target(&self.state).is_ok()
            && self.state.morph_frame_value(import.object).is_some()
    }

    pub(super) fn morph_import_error(&mut self, error: String) {
        self.source_queue
            .retain(|source| !matches!(source, loading::Source::Morph(_, _)));
        self.morph_editor.error.clone_from(&error);
        self.message = error;
    }

    pub(super) fn apply_morph_prepared(&mut self, file: cuemol_desktop_core::PreparedFile) {
        let Some(batch) = self.source_morph.clone() else {
            return;
        };
        let mut import = batch.borrow_mut();
        if !self.current_morph_import(&import) {
            return;
        }
        let name = file.name().to_string();
        let mut before = self.capture_history();
        before.record = cuemol_command::console::actions::Record::operation(
            &self.state,
            "add_morph_frame",
            &json!({"objId":import.object.get(),"file":name,"before":import.insert_before.map(|i|i+1)}),
        );
        match self
            .state
            .insert_morph_frame_from_prepared(import.object, file, import.insert_before)
        {
            Ok(summary) => {
                self.morph_editor.selected = import
                    .insert_before
                    .map(|i| i.min(summary.frame_count.saturating_sub(1)));
                import.insert_before = self.morph_editor.insert_before();
                self.morph_editor.error.clear();
                self.finish_morph_edit(before);
                import.revision = self.revision;
                self.message = format!("Added morph frame {name}");
            }
            Err(error) => {
                self.state.restore_snapshot(before.state);
                self.morph_import_error(error.to_string());
            }
        }
    }

    fn finish_morph_edit(&mut self, before: HistoryEntry) {
        self.state.mark_scene_changes_since(&before.state);
        self.history.push(before);
        if self.history.len() > 12 {
            self.history.remove(0);
        }
        self.future.clear();
        self.navigation.cancel();
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
    }

    pub(super) fn morph_ui(&mut self, ctx: &egui::Context) {
        if !self.show_morph {
            return;
        }
        if self.morph_editor.scene != self.state.active_scene() {
            self.close_morph();
            self.message = "The active scene changed. Reopen Morph animation tool.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let width = self.morph_editor.name_width;
        let generation = self.morph_editor.generation;
        let window = egui::Window::new("Morph animation tool")
            .open(&mut open)
            .collapsible(false)
            .default_width(480.)
            .show(ctx, |ui| {
                action = self.morph_editor.show(
                    ui,
                    &self.state,
                    self.busy || !self.source_queue.is_empty(),
                );
            });
        self.ui_rects.extend(self.morph_editor.rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Morph window", &window.response);
        }
        if width != self.morph_editor.name_width {
            self.state.put_config_value(
                "morph_editor:name_width",
                self.morph_editor.name_width.to_string(),
            );
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item("cuemol.config", &self.state.config_text());
            }
        }
        if generation != self.morph_editor.generation {
            self.cancel_morph_import();
        }
        use cuemol_ui::morph::Action;
        if !open || action == Some(Action::Close) {
            self.close_morph();
        } else if action == Some(Action::AddPdb) {
            choose_morph_files(self.morph_editor.generation as f64);
        } else if let Some(action) = action {
            self.cancel_console("Morph edited; remaining console commands cancelled.");
            let mut before = self.capture_history();
            let object = self.morph_editor.object.map(Uid::get);
            let (tool, args) = match action {
                Action::Convert => ("convert_to_morph", json!({"objId":object})),
                Action::AddSource(source) => (
                    "add_morph_frame",
                    json!({"objId":object,"fromObjId":source.get(),"before":self.morph_editor.insert_before().map(|i|i+1)}),
                ),
                Action::Remove(index) => (
                    "remove_morph_frame",
                    json!({"objId":object,"frame":index+1}),
                ),
                _ => return,
            };
            before.record =
                cuemol_command::console::actions::Record::operation(&self.state, tool, &args);
            match self.morph_editor.apply(&mut self.state, action) {
                Ok(true) => self.finish_morph_edit(before),
                Ok(false) => {}
                Err(error) => {
                    self.state.restore_snapshot(before.state);
                    self.morph_editor.error = error.to_string();
                }
            }
        }
    }
}
