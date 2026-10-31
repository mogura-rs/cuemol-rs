use cuemol_core::{parse_ldom2_xml, Uid};
use cuemol_desktop_core::{DesktopAppState, GuiStyleTarget};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Action {
    Command(&'static str, Value),
    Save(GuiStyleTarget, String),
}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    value: String,
}

#[derive(Debug)]
pub struct StyleEditor {
    pub rects: BTreeMap<String, [f32; 4]>,
    pub open: bool,
    scene: Uid,
    target: Option<GuiStyleTarget>,
    revision: Option<u64>,
    name: String,
    stored_name: String,
    source: String,
    read_only: bool,
    modified: bool,
    tab: usize,
    entries: [Vec<Entry>; 3],
    edits: BTreeMap<(usize, String), String>,
    new_name: String,
    new_value: String,
    error: Option<String>,
}

fn record(
    rects: &mut BTreeMap<String, [f32; 4]>,
    key: impl Into<String>,
    response: &egui::Response,
) {
    let rect = response.rect;
    rects.insert(
        key.into(),
        [rect.left(), rect.top(), rect.width(), rect.height()],
    );
}

impl StyleEditor {
    pub fn new(scene: Uid, target: Option<GuiStyleTarget>) -> Self {
        Self {
            rects: BTreeMap::new(),
            open: true,
            scene,
            target,
            revision: None,
            name: String::new(),
            stored_name: String::new(),
            source: String::new(),
            read_only: false,
            modified: false,
            tab: 0,
            entries: Default::default(),
            edits: BTreeMap::new(),
            new_name: String::new(),
            new_value: String::new(),
            error: None,
        }
    }

    pub fn completed(&mut self, state: &DesktopAppState, error: Option<String>) {
        if error.is_none() {
            if self.target.is_none() {
                self.target = state.find_style_target(self.scene, self.name.trim());
            }
            self.revision = None;
            self.new_name.clear();
            self.new_value.clear();
        }
        self.error = error;
    }

    fn refresh(&mut self, state: &DesktopAppState, revision: u64) -> Result<(), String> {
        if self.revision == Some(revision) || self.target.is_none() {
            return Ok(());
        }
        let target = self.target.unwrap();
        let summary = state
            .style_summary(target)
            .map_err(|error| error.to_string())?;
        let xml = state.style_xml(target).map_err(|error| error.to_string())?;
        let tree = parse_ldom2_xml(&xml).map_err(|error| error.to_string())?;
        let name = tree.top().str_attr("id").to_string();
        if self.name == self.stored_name {
            self.name = name.clone();
        }
        self.stored_name = name;
        self.source = summary.source;
        self.read_only = summary.read_only;
        self.modified = summary.modified;
        let mut dirty = BTreeMap::new();
        for (tab, entries) in self.entries.iter().enumerate() {
            for entry in entries {
                let key = (tab, entry.name.clone());
                if let Some(value) = self.edits.get(&key).filter(|value| **value != entry.value) {
                    dirty.insert(key, value.clone());
                }
            }
        }
        self.entries = Default::default();
        self.edits.clear();
        for node in tree.top().children().iter().filter(|node| !node.is_attr()) {
            let tab = match node.tag_name() {
                "color" => 0,
                "sel" => 1,
                "style" => 2,
                _ => continue,
            };
            let name = node.str_attr("id").to_string();
            if name.is_empty() {
                continue;
            }
            let value = if tab == 2 {
                node.type_name().to_string()
            } else {
                node.value().to_string()
            };
            let key = (tab, name.clone());
            let edit = dirty.remove(&key).unwrap_or_else(|| value.clone());
            self.edits.insert(key, edit);
            self.entries[tab].push(Entry { name, value });
        }
        self.revision = Some(revision);
        Ok(())
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        state: &DesktopAppState,
        revision: u64,
    ) -> Option<Action> {
        self.rects.clear();
        if !self.open {
            return None;
        }
        let valid = match self.refresh(state, revision) {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        };
        let scope = if self.target.is_some_and(|target| target.scope().get() == 0) {
            "global"
        } else {
            "scene"
        };
        let common = json!({"sceneId":self.scene.get(),"styleSet":self.stored_name,"scope":scope,"styleId":self.target.map(GuiStyleTarget::runtime_id)});
        let command = |name, mut args: Value| {
            for (key, value) in common.as_object().unwrap() {
                args[key] = value.clone();
            }
            Action::Command(name, args)
        };
        let mut action = None;
        let mut open = self.open;
        let title = if self.target.is_none() {
            "New Style".into()
        } else {
            format!("Style editor: {}", self.stored_name)
        };
        egui::Window::new(title).id(egui::Id::new("style-editor")).open(&mut open)
            .default_width(520.).min_width(420.).collapsible(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let response = ui.text_edit_singleline(&mut self.name);
                    record(&mut self.rects, "Style name", &response);
                    let enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    let label = if self.target.is_none() { "Create" } else { "Rename" };
                    let enabled = valid && !self.name.trim().is_empty()
                        && (self.target.is_none() || (self.name != self.stored_name && self.name.trim() != self.stored_name));
                    let response = ui.add_enabled(enabled, egui::Button::new(label));
                    record(&mut self.rects, format!("Style {label}"), &response);
                    if enabled && (response.clicked() || enter) {
                        action = Some(if self.target.is_none() {
                            Action::Command("create_style_set", json!({"sceneId":self.scene.get(),"name":self.name.trim()}))
                        } else { command("rename_style_set", json!({"name":self.name.trim()})) });
                    }
                });
                if self.target.is_some() && valid {
                    ui.horizontal(|ui| {
                        ui.label(if scope == "global" { "Global style set" } else { "Scene style set" });
                        let mut read_only = self.read_only;
                        let response = ui.add_enabled(self.read_only || !self.modified, egui::Checkbox::new(&mut read_only, "Read-only"));
                        record(&mut self.rects, "Style readonly", &response);
                        if response.changed() { action = Some(command("set_style_readonly", json!({"readonly":read_only}))); }
                        if self.modified { ui.weak("Modified"); }
                    });
                    if !self.source.is_empty() { ui.add(egui::Label::new(&self.source).truncate()).on_hover_text(&self.source); }
                    ui.horizontal(|ui| {
                        for (tab, label) in ["Color", "Selection", "Styles"].into_iter().enumerate() {
                            let response = ui.selectable_value(&mut self.tab, tab, label);
                            record(&mut self.rects, format!("Style tab {label}"), &response);
                        }
                    });
                    ui.separator();
                    let tab = self.tab;
                    let writable = !self.read_only;
                    if self.entries[tab].is_empty() { ui.weak("No entries"); }
                    egui::ScrollArea::vertical().id_salt("style-entries").max_height(300.).show_rows(ui, ui.spacing().interact_size.y, self.entries[tab].len(), |ui, range| {
                        for entry in &self.entries[tab][range] {
                            ui.push_id((tab, &entry.name), |ui| ui.horizontal(|ui| {
                                ui.add_sized([110., 20.], egui::Label::new(&entry.name));
                                if tab < 2 {
                                    let value = self.edits.get_mut(&(tab, entry.name.clone())).unwrap();
                                    let response = ui.add_enabled(writable, egui::TextEdit::singleline(value).desired_width(220.));
                                    record(&mut self.rects, format!("Style value {tab} {}", entry.name), &response);
                                    let enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                    if tab == 0 {
                                        if let Some(rgba) = cuemol_gfx::parse_color(value).ok().and_then(|color| color.rgba8()) {
                                            let mut color = egui::Color32::from_rgba_unmultiplied(rgba.r,rgba.g,rgba.b,rgba.a);
                                            if ui.add_enabled_ui(writable, |ui| ui.color_edit_button_srgba(&mut color)).inner.changed() {
                                                let [r,g,b,a] = color.to_srgba_unmultiplied();
                                                *value=cuemol_gfx::SolidColor::from_rgba_u8(r,g,b,a).script_string();
                                                action=Some(command("define_color",json!({"name":entry.name,"color":value})));
                                            }
                                        }
                                    }
                                    let response = ui.add_enabled(writable, egui::Button::new("Apply"));
                                    record(&mut self.rects, format!("Style apply {tab} {}", entry.name), &response);
                                    if response.clicked() || enter {
                                        let mut args = json!({"name":entry.name});
                                        args[if tab == 0 { "color" } else { "selection" }] = json!(value);
                                        action = Some(command(if tab == 0 {"define_color"} else {"define_selection"}, args));
                                    }
                                } else { ui.weak(&entry.value); }
                                let response = ui.add_enabled(writable, egui::Button::new("Delete"));
                                record(&mut self.rects, format!("Style delete {tab} {}", entry.name), &response);
                                if response.clicked() { action = Some(command(["remove_color","remove_selection","remove_style"][tab],json!({"name":entry.name}))); }
                            }));
                        }
                    });
                    if tab < 2 {
                        ui.separator();
                        ui.horizontal(|ui| {
                            let response = ui.add_enabled(writable, egui::TextEdit::singleline(&mut self.new_name).hint_text("Name").desired_width(110.));
                            record(&mut self.rects, "Style new name", &response);
                            let response = ui.add_enabled(writable, egui::TextEdit::singleline(&mut self.new_value).hint_text(if tab == 0 { "Color" } else { "Selection expression" }).desired_width(220.));
                            record(&mut self.rects, "Style new value", &response);
                            let response = ui.add_enabled(writable && !self.new_name.trim().is_empty() && !self.new_value.trim().is_empty(), egui::Button::new("Add"));
                            record(&mut self.rects, "Style add entry", &response);
                            if response.clicked() {
                                let mut args=json!({"name":self.new_name.trim()});
                                args[if tab == 0 { "color" } else { "selection" }] = json!(self.new_value.trim());
                                action=Some(command(if tab == 0 {"define_color"} else {"define_selection"},args));
                            }
                        });
                    }
                    ui.separator();
                    let response = ui.button("Save style...");
                    record(&mut self.rects, "Style save", &response);
                    if response.clicked() { action=Some(Action::Save(self.target.unwrap(),format!("{}.xml",self.stored_name))); }
                }
                if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                let response = ui.button("Close");
                record(&mut self.rects, "Style close", &response);
                if response.clicked() { self.open=false; }
            });
        self.open &= open;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_preserves_dirty_fields_and_updates_untouched_entries() {
        let mut state = DesktopAppState::new();
        let scene = state.new_scene("drafts", true).result_scene;
        state.import_scene_style(scene,
            r##"<styles id="palette"><color id="accent" value="#123456"/><sel id="focus" value="name C1"/></styles>"##,
            false).unwrap();
        let target = state.find_style_target(scene, "palette").unwrap();
        let mut editor = StyleEditor::new(scene, Some(target));
        editor.refresh(&state, 1).unwrap();
        editor.name = "unfinished-name".into();
        editor.edits.insert((1, "focus".into()), "name O1".into());
        state.edit_style(target,
            r##"<styles id="palette"><color id="accent" value="#abcdef"/><sel id="focus" value="name C1"/></styles>"##).unwrap();
        editor.refresh(&state, 2).unwrap();
        assert_eq!(editor.name, "unfinished-name");
        assert_eq!(editor.edits[&(1, "focus".into())], "name O1");
        assert_eq!(editor.edits[&(0, "accent".into())], "#abcdef");
        assert_eq!(editor.stored_name, "palette");
        state
            .edit_style(target, "<styles id=\"palette\"/>")
            .unwrap();
        editor.refresh(&state, 3).unwrap();
        assert!(editor.edits.is_empty());
    }
}
