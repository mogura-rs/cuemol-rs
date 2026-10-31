use super::*;
use cuemol_desktop_core::GuiStyleTarget;
use cuemol_ui::{
    shell,
    style_editor::{Action, StyleEditor},
};

impl App {
    pub(super) fn open_style_editor(&mut self, target: Option<GuiStyleTarget>) {
        if let Some(scene) = self.state.active_scene() {
            self.style_editor = Some(StyleEditor::new(scene, target));
        }
    }

    pub(super) fn style_tree(&mut self, ui: &mut egui::Ui) {
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        ui.separator();
        let response = shell::tree_row(ui, "Styles", shell::Icon::Open, false, None, None);
        self.record_ui_rect("Styles", &response);
        response.context_menu(|ui| {
            let response = ui.button("New Style...");
            self.record_ui_rect("New style", &response);
            if response.clicked() {
                self.open_style_editor(None);
                ui.close();
            }
            let response = ui.button("Load style...");
            self.record_ui_rect("Load style", &response);
            if response.clicked() {
                if let Some(input) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("styles"))
                    .and_then(|node| node.dyn_into::<web_sys::HtmlInputElement>().ok())
                {
                    input.click();
                }
                ui.close();
            }
        });
        ui.indent("style-sets", |ui| {
            for summary in self.state.style_summaries(scene) {
                ui.push_id(summary.target, |ui| {
                let scope = if summary.local_index.is_none() {
                    "global"
                } else {
                    "scene"
                };
                let label = format!(
                    "{}{}{}",
                    summary.name,
                    if scope == "global" { " (global)" } else { "" },
                    if summary.modified { " *" } else { "" }
                );
                let response = shell::tree_row(ui, &label, shell::Icon::Open, false, None, None);
                self.record_ui_rect(&format!("Style {scope} {}", summary.name), &response);
                self.record_ui_rect(
                    &format!("Style {scope} #{}", summary.target.runtime_id()),
                    &response,
                );
                if response.double_clicked() {
                    self.open_style_editor(Some(summary.target));
                }
                response.context_menu(|ui| {
                    let response = ui.button("Edit...");
                    self.record_ui_rect("Edit style", &response);
                    if response.clicked() {
                        self.open_style_editor(Some(summary.target));
                        ui.close();
                    }
                    let mut readonly = summary.read_only;
                    let response = ui.add_enabled(
                        summary.read_only || !summary.modified,
                        egui::Checkbox::new(&mut readonly, "Read-only"),
                    );
                    self.record_ui_rect("Style context readonly", &response);
                    if response.changed() {
                        self.call(
                            "set_style_readonly",
                            json!({"styleSet":summary.name,"styleId":summary.target.runtime_id(),"scope":scope,"readonly":readonly}),
                        );
                        ui.close();
                    }
                    if ui.button("Save style...").clicked() {
                        self.save_style_download(summary.target, &format!("{}.xml", summary.name));
                        ui.close();
                    }
                    let response = ui.button("Delete");
                    self.record_ui_rect("Style context delete", &response);
                    if response.clicked() {
                        self.call(
                            "delete_style_set",
                            json!({"styleSet":summary.name,"styleId":summary.target.runtime_id(),"scope":scope}),
                        );
                        ui.close();
                    }
                });
                });
            }
        });
    }

    pub(super) fn style_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.style_editor.take() else {
            return;
        };
        let action = editor.show(ctx, &self.state, self.revision);
        self.ui_rects.extend(
            editor
                .rects
                .iter()
                .map(|(key, value)| (key.clone(), *value)),
        );
        if let Some(action) = action {
            let success = match action {
                Action::Command(name, args) => self.call_shared(name, args),
                Action::Save(target, name) => self.save_style_download(target, &name),
            };
            editor.completed(&self.state, (!success).then(|| self.message.clone()));
        }
        if editor.open {
            self.style_editor = Some(editor);
        }
    }

    fn save_style_download(&mut self, target: GuiStyleTarget, name: &str) -> bool {
        let result = self
            .state
            .style_xml(target)
            .map_err(|error| error.to_string())
            .and_then(|xml| download(name, xml.as_bytes(), "application/xml").map_err(js_error));
        match result {
            Ok(()) => {
                self.message = format!("Downloaded {name}");
                true
            }
            Err(error) => {
                self.message = error;
                false
            }
        }
    }
}
