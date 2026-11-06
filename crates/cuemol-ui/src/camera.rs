use crate::shell::{self, Icon};
use cuemol_core::Uid;
use cuemol_desktop_core::{DesktopAppState, GuiCameraSummary};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Intent {
    Command(&'static str, Value),
    Clipboard(&'static str),
    Load,
    SaveFile(bool),
    Reload,
}

#[derive(Clone)]
struct CameraDrag {
    scene: Uid,
    name: String,
}

#[derive(Debug, Default)]
pub struct CameraPanel {
    pub selected: Option<String>,
    pub rects: BTreeMap<String, [f32; 4]>,
    scene: Option<Uid>,
    editing: Option<(String, String, bool)>,
    new_name: Option<(String, bool)>,
    properties: Option<(String, cuemol_qsys::Camera, cuemol_qsys::Camera)>,
    visibility: Option<(String, Vec<cuemol_desktop_core::GuiCameraVisibilityEntry>)>,
}

impl CameraPanel {
    fn record(&mut self, key: impl Into<String>, response: &egui::Response) {
        let rect = response.rect;
        self.rects.insert(
            key.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }

    fn reconcile(&mut self, state: &DesktopAppState) -> Vec<GuiCameraSummary> {
        if self.scene != state.active_scene() {
            self.scene = state.active_scene();
            self.selected = None;
            self.editing = None;
            self.new_name = None;
            self.properties = None;
            self.visibility = None;
        }
        let cameras = state.active_camera_summaries();
        if self
            .selected
            .as_ref()
            .is_some_and(|name| !cameras.iter().any(|camera| &camera.name == name))
        {
            self.selected = None;
        }
        if self
            .editing
            .as_ref()
            .is_some_and(|(name, _, _)| !cameras.iter().any(|camera| &camera.name == name))
        {
            self.editing = None;
        }
        cameras
    }

    fn begin_new(&mut self, state: &DesktopAppState) {
        if let Some(scene) = state.active_scene() {
            if let Ok(name) = state.suggest_camera_name(scene) {
                self.new_name = Some((name, true));
            }
        }
    }

    fn menu_button(&mut self, ui: &mut egui::Ui, label: &str) -> egui::Response {
        let response = ui.button(label);
        self.record(format!("Camera menu {label}"), &response);
        response
    }

    fn menu(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        camera: Option<&GuiCameraSummary>,
        actions: &mut Vec<Intent>,
    ) {
        if self.menu_button(ui, "New camera…").clicked() {
            self.begin_new(state);
            ui.close();
        }
        if let Some(camera) = camera {
            for (label, tool, args) in [
                (
                    "Save from view",
                    "save_camera",
                    json!({"name":camera.name,"withVisibility":false}),
                ),
                (
                    "Save from view with show/hide",
                    "save_camera",
                    json!({"name":camera.name,"withVisibility":true}),
                ),
                (
                    "Apply to view",
                    "apply_camera",
                    json!({"name":camera.name,"withVisibility":false}),
                ),
                (
                    "Apply to view with show/hide",
                    "apply_camera",
                    json!({"name":camera.name,"withVisibility":true}),
                ),
            ] {
                if self.menu_button(ui, label).clicked() {
                    actions.push(Intent::Command(tool, args));
                    ui.close();
                }
            }
            ui.separator();
            if self.menu_button(ui, "Rename…").clicked() {
                self.editing = Some((camera.name.clone(), camera.name.clone(), true));
                ui.close();
            }
            if self.menu_button(ui, "Delete").clicked() {
                actions.push(Intent::Command(
                    "delete_camera",
                    json!({"name":camera.name}),
                ));
                ui.close();
            }
            for (label, action) in [("Cut", "cut"), ("Copy", "copy")] {
                if self.menu_button(ui, label).clicked() {
                    actions.push(Intent::Clipboard(action));
                    ui.close();
                }
            }
        }
        if self.menu_button(ui, "Paste").clicked() {
            actions.push(Intent::Clipboard("paste"));
            ui.close();
        }
        ui.separator();
        let file_menu = ui.menu_button("Camera file", |ui| {
            if self.menu_button(ui, "Load…").clicked() {
                actions.push(Intent::Load);
                ui.close();
            }
            if let Some(camera) = camera {
                let reload = ui.add_enabled(!camera.source.is_empty(), egui::Button::new("Reload"));
                self.record("Camera menu Reload", &reload);
                if reload.clicked() {
                    actions.push(Intent::Reload);
                    ui.close();
                }
                for (label, save_as) in [("Save", false), ("Save as…", true)] {
                    if self.menu_button(ui, label).clicked() {
                        actions.push(Intent::SaveFile(save_as));
                        ui.close();
                    }
                }
            }
        });
        self.record("Camera menu Camera file", &file_menu.response);
        if let Some(camera) = camera {
            if self.menu_button(ui, "Edit show/hide…").clicked() {
                if let Ok(entries) = state.camera_visibility_entries(camera.scene_uid, &camera.name)
                {
                    self.visibility = Some((camera.name.clone(), entries));
                }
                ui.close();
            }
            if ui
                .add_enabled(
                    camera.visibility_count > 0,
                    egui::Button::new("Clear show/hide"),
                )
                .clicked()
            {
                actions.push(Intent::Command(
                    "clear_camera_visibility",
                    json!({"name":camera.name}),
                ));
                ui.close();
            }
            if self.menu_button(ui, "Properties…").clicked() {
                if let Some(saved) = state
                    .scene_manager()
                    .scene(camera.scene_uid)
                    .and_then(|scene| scene.camera_by_name(&camera.name))
                {
                    self.properties = Some((camera.name.clone(), saved.clone(), saved.clone()));
                }
                ui.close();
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &DesktopAppState) -> Vec<Intent> {
        let cameras = self.reconcile(state);
        let mut actions = Vec::new();
        let Some(scene) = self.scene else {
            ui.label("No active scene");
            return actions;
        };
        let selected = self.selected.clone();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            let new = shell::icon_button(
                ui,
                Icon::New,
                "New camera from view",
                false,
                egui::vec2(26., 24.),
            );
            self.record("Camera new", &new);
            if new.clicked() {
                self.begin_new(state);
            }
            for (key, icon, label, tool, visibility) in [
                (
                    "save",
                    Icon::CameraSave(false),
                    "Save from view",
                    "save_camera",
                    false,
                ),
                (
                    "save visibility",
                    Icon::CameraSave(true),
                    "Save from view with show/hide",
                    "save_camera",
                    true,
                ),
                (
                    "apply",
                    Icon::CameraApply(false),
                    "Apply to view",
                    "apply_camera",
                    false,
                ),
                (
                    "apply visibility",
                    Icon::CameraApply(true),
                    "Apply to view with show/hide",
                    "apply_camera",
                    true,
                ),
            ] {
                let button = ui
                    .add_enabled_ui(selected.is_some(), |ui| {
                        shell::icon_button(ui, icon, label, false, egui::vec2(26., 24.))
                    })
                    .inner;
                self.record(format!("Camera {key}"), &button);
                if button.clicked() {
                    actions.push(Intent::Command(
                        tool,
                        json!({"name":selected,"withVisibility":visibility}),
                    ));
                }
            }
            let delete = ui
                .add_enabled_ui(selected.is_some(), |ui| {
                    shell::icon_button(
                        ui,
                        Icon::Delete,
                        "Delete camera",
                        false,
                        egui::vec2(26., 24.),
                    )
                })
                .inner;
            self.record("Camera delete", &delete);
            if delete.clicked() {
                actions.push(Intent::Command("delete_camera", json!({"name":selected})));
            }
        });
        let list_id = ui.make_persistent_id("camera-list");
        ui.interact(
            ui.max_rect(),
            list_id,
            egui::Sense::focusable_noninteractive(),
        );
        let has_focus = ui.memory(|memory| memory.has_focus(list_id));
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                list_id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
        let mut moved_by_key = false;
        if has_focus && self.editing.is_none() && self.new_name.is_none() {
            let index = selected
                .as_ref()
                .and_then(|name| cameras.iter().position(|camera| &camera.name == name));
            let next = ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    Some(index.map_or(0, |i| (i + 1).min(cameras.len().saturating_sub(1))))
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    Some(index.map_or(0, |i| i.saturating_sub(1)))
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                    Some(0)
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                    Some(cameras.len().saturating_sub(1))
                } else {
                    None
                }
            });
            if let Some(camera) = next.and_then(|index| cameras.get(index)) {
                self.selected = Some(camera.name.clone());
                moved_by_key = true;
                ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            }
            if let Some(name) = self.selected.clone() {
                if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
                    self.editing = Some((name.clone(), name.clone(), true));
                }
                if ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                }) {
                    actions.push(Intent::Command("delete_camera", json!({"name":name})));
                }
            }
        }
        if cameras.is_empty() {
            ui.weak("(no cameras)");
        }
        for (index, camera) in cameras.iter().enumerate() {
            ui.push_id((&scene, &camera.name), |ui| {
                if self
                    .editing
                    .as_ref()
                    .is_some_and(|(name, _, _)| name == &camera.name)
                {
                    let (name, mut value, first) = self.editing.take().unwrap();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut value).desired_width(ui.available_width()),
                    );
                    self.record("Camera rename", &response);
                    if first {
                        response.request_focus();
                        if let Some(mut text) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                            text.cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(value.chars().count()),
                                )));
                            text.store(ui.ctx(), response.id);
                        }
                    }
                    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                        ui.memory_mut(|memory| memory.request_focus(list_id));
                    } else if !first
                        && (response.lost_focus()
                            || ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        let value = value.trim();
                        if !value.is_empty() && value != name {
                            actions.push(Intent::Command(
                                "rename_camera",
                                json!({"name":name,"newName":value}),
                            ));
                            self.selected = Some(value.into());
                        }
                        ui.memory_mut(|memory| memory.request_focus(list_id));
                    } else {
                        self.editing = Some((name, value, false));
                    }
                    return;
                }
                let response = ui.add_sized(
                    [ui.available_width(), 24.],
                    egui::Button::selectable(self.selected.as_ref() == Some(&camera.name), "")
                        .sense(egui::Sense::click_and_drag()),
                );
                self.record(format!("camera:{}", camera.name), &response);
                if response.clicked() || response.secondary_clicked() {
                    self.selected = Some(camera.name.clone());
                    ui.memory_mut(|memory| memory.request_focus(list_id));
                }
                if response.double_clicked() {
                    actions.push(Intent::Command(
                        "apply_camera",
                        json!({"name":camera.name,"withVisibility":true}),
                    ));
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        self.selected.as_ref() == Some(&camera.name),
                        &camera.name,
                    )
                });
                let color = ui.visuals().text_color();
                let icon_rect = egui::Rect::from_min_size(
                    response.rect.left_top() + egui::vec2(4., 4.),
                    egui::vec2(16., 16.),
                );
                shell::paint_icon(ui, Icon::SavedCamera, icon_rect, color);
                let mut right = response.rect.right() - 4.;
                for (enabled, icon, tooltip) in [
                    (
                        !camera.source.is_empty(),
                        Icon::Link,
                        camera.source.as_str(),
                    ),
                    (
                        camera.visibility_count > 0,
                        Icon::Eye,
                        "Has saved show/hide state",
                    ),
                ] {
                    if enabled {
                        let badge = egui::Rect::from_min_size(
                            egui::pos2(right - 16., response.rect.top() + 4.),
                            egui::vec2(16., 16.),
                        );
                        shell::paint_icon(ui, icon, badge, ui.visuals().weak_text_color());
                        if ui.rect_contains_pointer(badge) {
                            response.clone().on_hover_text(tooltip);
                        }
                        right -= 20.;
                    }
                }
                let text_rect = egui::Rect::from_min_max(
                    egui::pos2(icon_rect.right() + 5., response.rect.top()),
                    egui::pos2(right, response.rect.bottom()),
                );
                let mut text = egui::text::LayoutJob::simple_singleline(
                    camera.name.clone(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    color,
                );
                text.wrap.max_width = text_rect.width().max(0.);
                text.wrap.max_rows = 1;
                text.wrap.break_anywhere = true;
                let text = ui.painter().layout_job(text);
                ui.painter()
                    .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                    .galley(
                        egui::pos2(text_rect.left(), text_rect.center().y - text.size().y / 2.),
                        text,
                        color,
                    );
                if camera.name != "__current" && self.editing.is_none() {
                    response.dnd_set_drag_payload(CameraDrag {
                        scene,
                        name: camera.name.clone(),
                    });
                }
                let after = ui.input(|input| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|pos| pos.y > response.rect.center().y)
                });
                if response
                    .dnd_hover_payload::<CameraDrag>()
                    .is_some_and(|drag| drag.scene == scene && drag.name != camera.name)
                {
                    let y = if after {
                        response.rect.bottom()
                    } else {
                        response.rect.top()
                    };
                    ui.painter().hline(
                        response.rect.x_range(),
                        y,
                        (2., ui.visuals().selection.stroke.color),
                    );
                }
                if let Some(drag) = response
                    .dnd_release_payload::<CameraDrag>()
                    .filter(|drag| drag.scene == scene)
                {
                    if let Some(from) = cameras.iter().position(|camera| camera.name == drag.name) {
                        let insertion = index + usize::from(after);
                        let to = insertion
                            .saturating_sub(usize::from(from < insertion))
                            .max(usize::from(cameras[0].name == "__current"));
                        if from != to {
                            actions.push(Intent::Command(
                                "move_camera",
                                json!({"name":drag.name,"to":to+1}),
                            ));
                        }
                    }
                }
                response.context_menu(|ui| self.menu(ui, state, Some(camera), &mut actions));
                if moved_by_key && self.selected.as_ref() == Some(&camera.name) {
                    response.scroll_to_me(None);
                }
            });
        }
        let empty =
            ui.allocate_response(egui::vec2(ui.available_width(), 26.), egui::Sense::click());
        if empty.clicked() {
            ui.memory_mut(|memory| memory.request_focus(list_id));
        }
        if let Some(drag) = empty
            .dnd_release_payload::<CameraDrag>()
            .filter(|drag| drag.scene == scene)
        {
            actions.push(Intent::Command(
                "move_camera",
                json!({"name":drag.name,"end":"bottom"}),
            ));
        }
        empty.context_menu(|ui| {
            self.selected = None;
            self.menu(ui, state, None, &mut actions);
        });
        actions
    }

    fn script(&self, name: &str, edit: &str) -> Intent {
        let name = json!(name);
        let scene = self.scene.unwrap().get();
        Intent::Command(
            "execute_script",
            json!({"source":format!("(function() {{ var s=cuemol.getScene({scene}); var c=s.getCamera({name}); if (!c) throw new Error('Camera no longer exists'); {edit} }})();")}),
        )
    }

    fn visibility_dialog(&mut self, ctx: &egui::Context, state: &DesktopAppState) -> Vec<Intent> {
        let (name, mut entries) = self.visibility.take().unwrap();
        if !state
            .active_camera_summaries()
            .iter()
            .any(|camera| camera.name == name)
        {
            return Vec::new();
        }
        let mut submit = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("camera-visibility")).show(ctx, |ui| {
            ui.heading(format!("Show/hide: {name}"));
            ui.label("Only included objects and renderers change when this camera is applied.");
            egui::ScrollArea::vertical()
                .max_height(340.)
                .show(ui, |ui| {
                    for entry in &mut entries {
                        ui.push_id(entry.target_uid, |ui| {
                            ui.horizontal(|ui| {
                                if !entry.is_object {
                                    ui.add_space(16.);
                                }
                                let included = ui.checkbox(&mut entry.included, &entry.name);
                                self.record(
                                    format!("Camera include {}", entry.target_uid.get()),
                                    &included,
                                );
                                let visible = ui
                                    .add_enabled_ui(entry.included, |ui| {
                                        ui.checkbox(&mut entry.visible, "Visible")
                                    })
                                    .inner;
                                self.record(
                                    format!("Camera visible {}", entry.target_uid.get()),
                                    &visible,
                                );
                            })
                        });
                    }
                });
            ui.horizontal(|ui| {
                let ok = ui.button("OK");
                self.record("Camera visibility OK", &ok);
                submit = ok.clicked();
                let button = ui.button("Cancel");
                self.record("Camera visibility Cancel", &button);
                cancel = button.clicked();
            });
        });
        if submit {
            let value = entries
                .iter()
                .filter(|entry| entry.included)
                .map(|entry| (entry.target_uid.get(), entry.visible, entry.is_object))
                .collect::<Vec<_>>();
            let current = state
                .scene_manager()
                .scene(self.scene.unwrap())
                .unwrap()
                .camera_by_name(&name)
                .unwrap()
                .visibility_settings()
                .collect::<Vec<_>>();
            let mut sorted = value.clone();
            sorted.sort();
            let mut current = current;
            current.sort();
            if sorted == current {
                return Vec::new();
            }
            vec![self.script(&name, &format!("c.clearVisSettings(); for (var e of {}) c.visAppend(e[0],e[1],e[2]); s.setCamera({},c);",json!(value),json!(name)))]
        } else {
            if !cancel && !modal.should_close() {
                self.visibility = Some((name, entries));
            }
            Vec::new()
        }
    }

    fn properties_dialog(&mut self, ctx: &egui::Context, state: &DesktopAppState) -> Vec<Intent> {
        let (old_name, original, mut camera) = self.properties.take().unwrap();
        let cameras = state.active_camera_summaries();
        if !cameras.iter().any(|camera| camera.name == old_name) {
            return Vec::new();
        }
        let mut name = camera.name().to_string();
        let center = camera.center();
        let rotation = camera.rotation();
        let mut values = [
            camera.zoom(),
            camera.cam_dist(),
            camera.slab_depth(),
            center.x,
            center.y,
            center.z,
            rotation.x,
            rotation.y,
            rotation.z,
            rotation.w,
            camera.stereo_distance(),
        ];
        let mut perspective = camera.is_perspective();
        let mut stereo = camera.stereo_mode();
        let mut mark = camera.center_mark();
        let mut submit = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("saved-camera-properties")).show(ctx, |ui| {
            ui.heading("Camera properties");
            egui::Grid::new("camera-fields").show(ui, |ui| {
                ui.label("Name");
                let response = ui.text_edit_singleline(&mut name);
                self.record("Camera property name", &response);
                ui.end_row();
                for (index, label) in [
                    "Zoom",
                    "Distance",
                    "Slab",
                    "Center X",
                    "Center Y",
                    "Center Z",
                    "Rotation X",
                    "Rotation Y",
                    "Rotation Z",
                    "Rotation W",
                    "Stereo distance",
                ]
                .iter()
                .enumerate()
                {
                    ui.label(*label);
                    let response = ui.add(
                        egui::DragValue::new(&mut values[index])
                            .speed(if (6..10).contains(&index) { 0.01 } else { 0.1 }),
                    );
                    self.record(format!("Camera property {label}"), &response);
                    ui.end_row();
                }
                ui.label("Perspective");
                ui.checkbox(&mut perspective, "");
                ui.end_row();
                ui.label("Stereo");
                egui::ComboBox::from_id_salt("camera-stereo")
                    .selected_text(format!("{stereo:?}"))
                    .show_ui(ui, |ui| {
                        for value in [
                            cuemol_qsys::StereoMode::None,
                            cuemol_qsys::StereoMode::Parallel,
                            cuemol_qsys::StereoMode::Cross,
                            cuemol_qsys::StereoMode::Hardware,
                        ] {
                            ui.selectable_value(&mut stereo, value, format!("{value:?}"));
                        }
                    });
                ui.end_row();
                ui.label("Center mark");
                egui::ComboBox::from_id_salt("camera-center-mark")
                    .selected_text(format!("{mark:?}"))
                    .show_ui(ui, |ui| {
                        for value in [
                            cuemol_qsys::CenterMark::None,
                            cuemol_qsys::CenterMark::Cross,
                            cuemol_qsys::CenterMark::Axis,
                        ] {
                            ui.selectable_value(&mut mark, value, format!("{value:?}"));
                        }
                    });
                ui.end_row();
                ui.label("Source");
                ui.label(camera.source());
                ui.end_row();
            });
            let valid = !name.trim().is_empty()
                && !cameras
                    .iter()
                    .any(|camera| camera.name == name.trim() && camera.name != old_name)
                && values.iter().all(|value| value.is_finite())
                && values[..3].iter().all(|value| *value > 0.)
                && values[10] > 0.
                && values[6..10].iter().map(|value| value * value).sum::<f64>() > 1e-24;
            ui.horizontal(|ui| {
                let ok = ui.add_enabled(valid, egui::Button::new("OK"));
                self.record("Camera properties OK", &ok);
                submit = ok.clicked();
                let button = ui.button("Cancel");
                self.record("Camera properties Cancel", &button);
                cancel = button.clicked();
            });
        });
        camera.set_name(&name);
        camera.set_zoom(values[0]);
        camera.set_cam_dist(values[1]);
        camera.set_slab_depth(values[2]);
        camera.set_center(cuemol_core::Vector4::new4(
            values[3], values[4], values[5], center.w,
        ));
        camera.set_rotation(cuemol_core::Quat::new(
            values[6], values[7], values[8], values[9],
        ));
        camera.set_stereo_distance(values[10]);
        camera.set_perspective(perspective);
        camera.set_stereo_mode(stereo);
        camera.set_center_mark(mark);
        if submit {
            camera.set_name(name.trim());
            if camera == original {
                return Vec::new();
            }
            let values = camera_values(&camera);
            let old = camera_values(&original);
            let changed = values
                .as_object()
                .unwrap()
                .iter()
                .filter(|(key, value)| old[*key] != **value)
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<serde_json::Map<_, _>>();
            self.selected = Some(camera.name().to_string());
            vec![self.script(&old_name,&format!("var n={}; if (n!=={} && s.hasCamera(n)) throw new Error('Camera name already exists'); var p={}; for(var k of Object.keys(p)) c[k]=p[k]; if(n!=={}) s.destroyCamera({}); s.setCamera(n,c);",json!(camera.name()),json!(old_name),Value::Object(changed),json!(old_name),json!(old_name)))]
        } else {
            if !cancel && !modal.should_close() {
                self.properties = Some((old_name, original, camera));
            }
            Vec::new()
        }
    }

    pub fn dialog(&mut self, ctx: &egui::Context, state: &DesktopAppState) -> Vec<Intent> {
        self.reconcile(state);
        if self.properties.is_some() {
            return self.properties_dialog(ctx, state);
        }
        if self.visibility.is_some() {
            return self.visibility_dialog(ctx, state);
        }
        let Some((mut name, first)) = self.new_name.take() else {
            return Vec::new();
        };
        let mut submit = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("camera-new")).show(ctx, |ui| {
            ui.heading("New camera");
            ui.label("Name for new camera:");
            let response = ui.text_edit_singleline(&mut name);
            self.record("Camera new name", &response);
            if first {
                response.request_focus();
            }
            ui.horizontal(|ui| {
                let ok = ui.add_enabled(!name.trim().is_empty(), egui::Button::new("Create"));
                self.record("Camera create", &ok);
                submit = ok.clicked()
                    || (!name.trim().is_empty()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                let button = ui.button("Cancel");
                self.record("Camera create cancel", &button);
                cancel = button.clicked();
            });
        });
        if submit {
            let name = name.trim().to_string();
            self.selected = Some(name.clone());
            vec![Intent::Command(
                "save_camera",
                json!({"name":name,"withVisibility":false}),
            )]
        } else {
            if !cancel && !modal.should_close() {
                self.new_name = Some((name, false));
            }
            Vec::new()
        }
    }
}

fn camera_values(camera: &cuemol_qsys::Camera) -> Value {
    let center = camera.center();
    let rotation = camera.rotation();
    json!({"zoom":camera.zoom(),"distance":camera.cam_dist(),"slab":camera.slab_depth(),
        "center":{"x":center.x,"y":center.y,"z":center.z,"w":center.w},
        "rotation":{"x":rotation.x,"y":rotation.y,"z":rotation.z,"a":rotation.w},
        "perspec":camera.is_perspective(),"stereoDist":camera.stereo_distance(),
        "stereoMode":match camera.stereo_mode() { cuemol_qsys::StereoMode::None=>"none",cuemol_qsys::StereoMode::Parallel=>"para",cuemol_qsys::StereoMode::Cross=>"cross",cuemol_qsys::StereoMode::Hardware=>"hardware" },
        "centerMark":match camera.center_mark() {cuemol_qsys::CenterMark::None=>"none",cuemol_qsys::CenterMark::Cross=>"crosshair",cuemol_qsys::CenterMark::Axis=>"axis"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::Camera;

    struct Harness {
        panel: CameraPanel,
        state: DesktopAppState,
        view: Camera,
        ctx: egui::Context,
        commands: Vec<String>,
    }
    impl Harness {
        fn new() -> Self {
            let mut state = DesktopAppState::new();
            state.new_scene("cameras", true);
            Self {
                panel: Default::default(),
                state,
                view: Camera::new(),
                ctx: Default::default(),
                commands: Vec::new(),
            }
        }
        fn apply(&mut self, actions: Vec<Intent>) {
            for action in actions {
                if let Intent::Command(tool, args) = action {
                    self.commands.push(tool.into());
                    cuemol_command::call_tool(&mut self.state, &mut self.view, tool, &args)
                        .unwrap();
                }
            }
        }
        fn frame(&mut self, events: Vec<egui::Event>) {
            self.panel.rects.clear();
            let ctx = self.ctx.clone();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 700.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    let actions = egui::SidePanel::left("camera-test")
                        .exact_width(260.)
                        .show(ctx, |ui| self.panel.show(ui, &self.state))
                        .inner;
                    self.apply(actions);
                    let actions = self.panel.dialog(ctx, &self.state);
                    self.apply(actions);
                },
            );
        }
        fn click(&mut self, key: &str) {
            self.frame(Vec::new());
            self.frame(Vec::new());
            let [x, y, w, h] = self.panel.rects[key];
            let pos = egui::pos2(x + w / 2., y + h / 2.);
            for pressed in [true, false] {
                self.frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ]);
            }
        }
        fn key(&mut self, key: egui::Key) {
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
            self.frame(Vec::new());
        }
        fn save(&mut self, name: &str) {
            cuemol_command::call_tool(
                &mut self.state,
                &mut self.view,
                "save_camera",
                &json!({"name":name}),
            )
            .unwrap();
        }
    }

    #[test]
    fn camera_toolbar_distinguishes_view_and_visibility_and_keyboard_rename() {
        let mut h = Harness::new();
        let scene = h.state.active_scene().unwrap();
        let object = h
            .state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("mol", Default::default());
        h.save("front");
        h.save("back");
        h.click("camera:front");
        h.state.set_object_visible(object, false).unwrap();
        h.click("Camera save visibility");
        assert_eq!(h.state.active_camera_summaries()[0].visibility_count, 1);
        h.state.set_object_visible(object, true).unwrap();
        h.click("Camera apply");
        assert!(h.state.object_summaries(scene)[0].visible);
        h.click("Camera apply visibility");
        assert!(!h.state.object_summaries(scene)[0].visible);
        h.click("Camera save");
        assert_eq!(h.state.active_camera_summaries()[0].visibility_count, 0);
        h.click("camera:front");
        h.key(egui::Key::ArrowDown);
        assert_eq!(h.panel.selected.as_deref(), Some("back"));
        h.key(egui::Key::F2);
        h.frame(vec![egui::Event::Text("renamed".into())]);
        h.key(egui::Key::Enter);
        assert_eq!(h.state.active_camera_summaries()[1].name, "renamed");
        h.key(egui::Key::Delete);
        assert_eq!(h.state.active_camera_summaries().len(), 1);
    }

    #[test]
    fn camera_dialogs_preserve_metadata_and_cancel_on_scene_change() {
        let mut h = Harness::new();
        h.save("front");
        h.frame(Vec::new());
        let scene = h.state.active_scene().unwrap();
        let object = h
            .state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("mol", Default::default());
        let mut camera = h
            .state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("front")
            .unwrap()
            .clone();
        camera.set_source("front.cam");
        camera.vis_append(object.get(), false, true);
        h.state
            .replace_scene_camera(scene, "front", camera.clone())
            .unwrap();
        let mut changed = camera.clone();
        changed.set_zoom(81.);
        changed.set_name("renamed");
        h.panel.properties = Some(("front".into(), camera, changed));
        h.click("Camera properties OK");
        let saved = h
            .state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("renamed")
            .unwrap();
        assert_eq!(saved.zoom(), 81.);
        assert_eq!(saved.source(), "front.cam");
        assert_eq!(saved.visibility_size(), 1);
        let entries = h.state.camera_visibility_entries(scene, "renamed").unwrap();
        h.panel.visibility = Some(("renamed".into(), entries));
        h.click("Camera visibility OK");
        assert_eq!(h.commands.len(), 1);
        h.click("Camera new");
        h.click("Camera create cancel");
        assert_eq!(h.commands.len(), 1);
        h.click("Camera new");
        h.state.new_scene("other", true);
        h.frame(Vec::new());
        assert!(h.panel.new_name.is_none());
        assert!(h.state.active_camera_summaries().is_empty());
    }

    #[test]
    fn camera_drag_reorders_one_command_and_keeps_current_pinned() {
        let mut h = Harness::new();
        for name in ["front", "__current", "back"] {
            h.save(name);
        }
        h.frame(Vec::new());
        h.frame(Vec::new());
        let [x, y, w, height] = h.panel.rects["camera:back"];
        let from = egui::pos2(x + w / 2., y + height / 2.);
        let [x, y, w, _] = h.panel.rects["camera:__current"];
        let to = egui::pos2(x + w / 2., y + 1.);
        h.frame(vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.frame(Vec::new());
        h.frame(vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(h.commands, ["move_camera"]);
        assert_eq!(
            h.state
                .active_camera_summaries()
                .iter()
                .map(|camera| camera.name.as_str())
                .collect::<Vec<_>>(),
            ["__current", "back", "front"]
        );
    }
}
