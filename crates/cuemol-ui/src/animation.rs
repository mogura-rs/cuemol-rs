mod geometry;
mod inspector;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke};
use geometry::*;
pub use geometry::{clock, parse_clock};
pub use inspector::ElementInspector;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type Command = (&'static str, Value);

#[derive(Default)]
pub struct Output {
    pub commands: Vec<Command>,
    pub inspect: Option<u64>,
}

#[derive(Clone, Debug)]
struct Drag {
    uid: u64,
    mode: EditMode,
    origin: egui::Pos2,
    original: Span,
    value: Span,
    reference: String,
    moved: bool,
}

#[derive(Debug)]
pub struct TimelinePanel {
    pub selected: Option<u64>,
    pub rects: BTreeMap<String, [f32; 4]>,
    scene: Option<u64>,
    scale: f64,
    drag: Option<Drag>,
    scrub: Option<f64>,
    time: Option<f64>,
    cancelled: bool,
    cached: Value,
    cached_revision: Option<(u64, u64)>,
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            selected: None,
            rects: BTreeMap::new(),
            scene: None,
            scale: DEFAULT_SCALE,
            drag: None,
            scrub: None,
            time: None,
            cancelled: false,
            cached: Value::Null,
            cached_revision: None,
        }
    }
}

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().filter(|n| n.is_finite()).unwrap_or(0.)
}
fn span(element: &Value) -> Span {
    Span {
        start: number(element, "startMs"),
        end: number(element, "endMs"),
        abs_start: number(element, "absStartMs"),
        abs_end: number(element, "absEndMs"),
        relative: element["timeRefName"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
    }
}
fn uid(element: &Value) -> u64 {
    element["uid"].as_u64().unwrap_or(0)
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
fn element_arg(uid: u64) -> String {
    format!("#{uid}")
}
fn seek(ms: f64) -> Command {
    ("animate", json!({"action":"seek","timeMs":ms}))
}
fn record(rects: &mut BTreeMap<String, [f32; 4]>, key: impl Into<String>, rect: Rect) {
    rects.insert(
        key.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}
fn color(kind: &str) -> Color32 {
    let [r, g, b] = match kind {
        "SimpleSpin" => [47, 125, 138],
        "CamMotion" => [58, 99, 176],
        "ShowHideAnim" => [176, 125, 42],
        "SlideInOutAnim" => [63, 138, 85],
        "MolAnim" => [115, 85, 168],
        "RealPropAnim" => [168, 79, 104],
        "RendXformAnim" => [90, 104, 176],
        _ => [74, 80, 96],
    };
    Color32::from_rgb(r, g, b)
}
fn icon(kind: &str) -> &'static str {
    use egui_phosphor::regular::*;
    match kind {
        "SimpleSpin" | "CamMotion" => CAMERA,
        "ShowHideAnim" => EYE,
        "SlideInOutAnim" | "RendXformAnim" => ARROWS_HORIZONTAL,
        "RealPropAnim" => PALETTE,
        _ => DIAMOND,
    }
}

impl TimelinePanel {
    pub fn cancel(&mut self) {
        self.drag = None;
        self.scrub = None;
        self.time = None;
        self.cancelled = true;
    }

    pub fn show_state(
        &mut self,
        ui: &mut egui::Ui,
        state: &cuemol_desktop_core::DesktopAppState,
        revision: u64,
    ) -> Output {
        let key = state.active_scene().map(|scene| (scene.get(), revision));
        if key != self.cached_revision {
            self.cached = cuemol_command::animation_timeline(state)
                .unwrap_or_else(|error| json!({"error":error.to_string()}));
            self.cached_revision = key;
        }
        let mut data = std::mem::take(&mut self.cached);
        if let Some(scene) = state
            .active_scene()
            .filter(|scene| state.has_scene_animation_player(*scene))
        {
            data["mgr"]["elapsedMs"] = json!(state.scene_animation_elapsed_ms(scene));
            if state.is_scene_animation_playing(scene) {
                data["mgr"]["playState"] = json!("playing");
            } else if data["mgr"]["playState"] == "playing" {
                data["mgr"]["playState"] = json!("stopped");
            }
        }
        let output = self.show(ui, &data, state.active_view_summary().is_some());
        self.cached = data;
        output
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Value, has_view: bool) -> Output {
        let mut output = Output::default();
        self.rects.clear();
        let scene = data["sceneId"].as_u64();
        if self.scene != scene {
            self.scene = scene;
            self.selected = None;
            self.cancel();
        }
        let Some(elements) = data["elements"].as_array() else {
            if let Some(error) = data["error"].as_str() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else {
                ui.weak("No active scene");
            }
            return output;
        };
        if self
            .selected
            .is_some_and(|id| !elements.iter().any(|e| uid(e) == id))
        {
            self.selected = None;
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape) || !input.focused) {
            self.cancel();
        }
        if !ui.input(|input| input.pointer.primary_down() || input.pointer.primary_released()) {
            self.cancelled = false;
            self.drag = None;
            self.scrub = None;
            self.time = None;
        }
        let length = number(&data["mgr"], "lengthMs");
        let content = elements
            .iter()
            .fold(length.max(1000.), |end, e| end.max(number(e, "absEndMs")));
        let can_seek = has_view && data["resolveError"].is_null();
        self.transport(ui, data, elements, has_view, can_seek, content, &mut output);
        if let Some(error) = data["resolveError"].as_str() {
            ui.colored_label(ui.visuals().warn_fg_color, error);
        }
        let preview = self.update_drag(ui, elements, &mut output);
        let height = (ui.available_height() - 31.).max(60.);
        egui::ScrollArea::both()
            .id_salt("animation_strips")
            .auto_shrink([false, false])
            .max_height(height)
            .min_scrolled_height(0.)
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.cursor().min;
                let visible = Rect::from_min_size(origin + viewport.min.to_vec2(), viewport.size());
                let size = vec2(
                    LABEL_WIDTH + timeline_width(content, self.scale) as f32,
                    RULER_HEIGHT + (elements.len().max(1) as f32 * LANE_HEIGHT),
                );
                ui.allocate_space(size);
                let clip = visible.intersect(ui.clip_rect());
                let timeline_x = origin.x + LABEL_WIDTH;
                let time_clip = clip.intersect(Rect::from_min_max(
                    pos2(visible.left() + LABEL_WIDTH, visible.top() + RULER_HEIGHT),
                    visible.max,
                ));
                let labels_clip = clip.intersect(Rect::from_min_max(
                    pos2(visible.left(), visible.top() + RULER_HEIGHT),
                    pos2(visible.left() + LABEL_WIDTH, visible.bottom()),
                ));
                let painter = ui.painter().with_clip_rect(time_clip);
                let labels = ui.painter().with_clip_rect(labels_clip);
                let step = tick_step(self.scale);
                let step_px = (step * self.scale) as f32;
                let first_tick = ((time_clip.left() - timeline_x) / step_px).floor().max(0.) as u64;
                let last_tick = ((time_clip.right() - timeline_x) / step_px).ceil().max(0.) as u64;
                for tick in first_tick..=last_tick {
                    let x = timeline_x + tick as f32 * step_px;
                    painter.line_segment(
                        [pos2(x, time_clip.top()), pos2(x, time_clip.bottom())],
                        Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
                    );
                }
                for (index, element) in elements.iter().enumerate() {
                    let id = uid(element);
                    let y = origin.y + RULER_HEIGHT + index as f32 * LANE_HEIGHT;
                    if y + LANE_HEIGHT < time_clip.top() || y > time_clip.bottom() {
                        continue;
                    }
                    let selected = self.selected == Some(id);
                    let lane = Rect::from_min_max(
                        pos2(time_clip.left(), y),
                        pos2(time_clip.right(), y + LANE_HEIGHT),
                    );
                    if selected {
                        painter.rect_filled(
                            lane,
                            0.,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.25),
                        );
                    }
                    painter.line_segment(
                        [lane.left_bottom(), lane.right_bottom()],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                    let label = Rect::from_min_size(
                        pos2(visible.left(), y),
                        vec2(LABEL_WIDTH, LANE_HEIGHT),
                    );
                    if selected {
                        labels.rect_filled(label, 0., ui.visuals().selection.bg_fill);
                    }
                    let label_color = if element["resolveError"].is_string() {
                        ui.visuals().warn_fg_color
                    } else {
                        ui.visuals().text_color()
                    };
                    labels.text(
                        pos2(label.left() + 7., label.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon(text(element, "type")),
                        egui::FontId::proportional(14.),
                        label_color,
                    );
                    labels.text(
                        pos2(label.left() + 26., label.center().y),
                        egui::Align2::LEFT_CENTER,
                        text(element, "name"),
                        egui::FontId::proportional(12.),
                        label_color,
                    );
                    let label_response = ui.interact(
                        label.intersect(labels_clip),
                        ui.id().with(("anim_label", id)),
                        Sense::click(),
                    );
                    record(
                        &mut self.rects,
                        format!("Animation label {}", text(element, "name")),
                        label.intersect(labels_clip),
                    );
                    if label_response.clicked() {
                        self.selected = if selected { None } else { Some(id) };
                    }
                    if label_response.double_clicked() {
                        self.selected = Some(id);
                        output.inspect = Some(id);
                    }
                    let s = preview
                        .filter(|(preview_id, _)| *preview_id == id)
                        .map(|(_, span)| span)
                        .unwrap_or_else(|| span(element));
                    let start = s.abs_start.max(0.);
                    let end = s.abs_end.max(start);
                    let bar = Rect::from_min_size(
                        pos2(timeline_x + (start * self.scale) as f32, y + 4.),
                        vec2(
                            ((end - start) * self.scale).max(3.) as f32,
                            LANE_HEIGHT - 8.,
                        ),
                    );
                    let empty = ui.interact(
                        lane.intersect(time_clip),
                        ui.id().with(("anim_lane", id)),
                        Sense::click(),
                    );
                    if empty.clicked()
                        && !ui.input(|input| {
                            input
                                .pointer
                                .interact_pos()
                                .is_some_and(|p| bar.contains(p))
                        })
                    {
                        self.selected = None;
                    }
                    let visible_bar = bar.intersect(time_clip);
                    if visible_bar.is_positive() {
                        let shade = color(text(element, "type"));
                        let shade = if element["disabled"] == true {
                            shade.gamma_multiply(0.45)
                        } else {
                            shade
                        };
                        painter.rect_filled(bar, 2., shade);
                        let bar_painter = painter.with_clip_rect(visible_bar);
                        let warning = element["resolveError"].is_string()
                            || s.abs_start < 0.
                            || s.abs_end < s.abs_start;
                        if warning {
                            bar_painter.rect_stroke(
                                bar,
                                2.,
                                Stroke::new(1., ui.visuals().warn_fg_color),
                                egui::StrokeKind::Inside,
                            );
                            if element["resolveError"].is_string() {
                                let mut x = visible_bar.left() - bar.height();
                                while x < visible_bar.right() {
                                    bar_painter.line_segment(
                                        [pos2(x, bar.bottom()), pos2(x + bar.height(), bar.top())],
                                        Stroke::new(3., Color32::from_white_alpha(40)),
                                    );
                                    x += 8.;
                                }
                            }
                        }
                        if selected {
                            bar_painter.rect_stroke(
                                bar,
                                2.,
                                Stroke::new(1., ui.visuals().selection.stroke.color),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if bar.width() > 30. {
                            bar_painter.text(
                                pos2(bar.left() + 8., bar.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!(
                                    "{} {}",
                                    icon(text(element, "type")),
                                    text(element, "name")
                                ),
                                egui::FontId::proportional(11.),
                                Color32::from_rgb(244, 246, 250),
                            );
                        }
                        let response = ui.interact(
                            visible_bar,
                            ui.id().with(("anim_strip", id)),
                            Sense::click_and_drag(),
                        );
                        record(
                            &mut self.rects,
                            format!("Animation strip {id}"),
                            visible_bar,
                        );
                        let pointer = ui.input(|input| input.pointer.interact_pos());
                        let mode = pointer
                            .map(|p| {
                                if p.x <= bar.left() + 6. {
                                    EditMode::Left
                                } else if p.x >= bar.right() - 6. {
                                    EditMode::Right
                                } else {
                                    EditMode::Move
                                }
                            })
                            .unwrap_or(EditMode::Move);
                        if response.hovered() || selected {
                            for x in [bar.left(), bar.right() - 6.] {
                                bar_painter.rect_filled(
                                    Rect::from_min_size(pos2(x, bar.top()), vec2(6., bar.height())),
                                    0.,
                                    Color32::from_white_alpha(35),
                                );
                            }
                        }
                        if !self.cancelled
                            && response.is_pointer_button_down_on()
                            && ui.input(|input| input.pointer.primary_pressed())
                        {
                            if let Some(origin) = pointer {
                                self.selected = Some(id);
                                self.drag = Some(Drag {
                                    uid: id,
                                    mode,
                                    origin,
                                    original: span(element),
                                    value: span(element),
                                    reference: text(element, "timeRefName").into(),
                                    moved: false,
                                });
                            }
                        }
                        if response.double_clicked() {
                            output.inspect = Some(id);
                        }
                        response
                            .on_hover_cursor(if mode == EditMode::Move {
                                egui::CursorIcon::Grab
                            } else {
                                egui::CursorIcon::ResizeHorizontal
                            })
                            .on_hover_text(format!(
                                "{} ({})\n{} – {}{}",
                                text(element, "name"),
                                text(element, "type"),
                                clock(s.abs_start),
                                clock(s.abs_end),
                                element["resolveError"]
                                    .as_str()
                                    .map(|s| format!("\n{s}"))
                                    .unwrap_or_default()
                            ));
                    }
                }
                if elements.is_empty() {
                    painter.text(
                        pos2(time_clip.left() + 10., time_clip.top() + 14.),
                        egui::Align2::LEFT_CENTER,
                        "No animation elements",
                        egui::FontId::proportional(12.),
                        ui.visuals().weak_text_color(),
                    );
                }
                let ruler_rect = Rect::from_min_max(
                    pos2(visible.left() + LABEL_WIDTH, visible.top()),
                    pos2(visible.right(), visible.top() + RULER_HEIGHT),
                )
                .intersect(clip);
                let ruler = ui.painter().with_clip_rect(ruler_rect);
                ruler.rect_filled(ruler_rect, 0., ui.visuals().panel_fill);
                for tick in first_tick..=last_tick {
                    let x = timeline_x + tick as f32 * step_px;
                    ruler.text(
                        pos2(x + 4., ruler_rect.center().y - 2.),
                        egui::Align2::LEFT_CENTER,
                        tick_label(tick as f64 * step, step),
                        egui::FontId::proportional(11.),
                        ui.visuals().text_color(),
                    );
                    ruler.line_segment(
                        [
                            pos2(x, ruler_rect.bottom() - 5.),
                            pos2(x, ruler_rect.bottom()),
                        ],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                }
                let response = ui.interact(
                    ruler_rect,
                    ui.id().with("anim_ruler"),
                    if can_seek {
                        Sense::click_and_drag()
                    } else {
                        Sense::hover()
                    },
                );
                record(&mut self.rects, "Animation ruler", ruler_rect);
                if can_seek
                    && !self.cancelled
                    && response.is_pointer_button_down_on()
                    && ui.input(|input| input.pointer.primary_pressed())
                {
                    self.scrub = Some(number(&data["mgr"], "elapsedMs"));
                }
                if self.scrub.is_some() {
                    if !can_seek {
                        self.scrub = None;
                    } else {
                        if let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                            self.scrub =
                                Some(((pos.x - timeline_x) as f64 / self.scale).clamp(0., content));
                        }
                        if ui.input(|input| input.pointer.primary_released()) {
                            output.commands.push(seek(self.scrub.take().unwrap()));
                        }
                    }
                }
                let now = self
                    .scrub
                    .or(self.time)
                    .unwrap_or_else(|| number(&data["mgr"], "elapsedMs"));
                let x = timeline_x + (now * self.scale) as f32;
                let head = ui
                    .painter()
                    .with_clip_rect(Rect::from_min_max(ruler_rect.min, time_clip.max));
                head.line_segment(
                    [pos2(x, ruler_rect.top()), pos2(x, time_clip.bottom())],
                    Stroke::new(2., ui.visuals().selection.stroke.color),
                );
                head.add(egui::Shape::convex_polygon(
                    vec![
                        pos2(x - 5., ruler_rect.top()),
                        pos2(x + 5., ruler_rect.top()),
                        pos2(x, ruler_rect.top() + 7.),
                    ],
                    ui.visuals().selection.stroke.color,
                    Stroke::NONE,
                ));
                let corner = Rect::from_min_size(visible.min, vec2(LABEL_WIDTH, RULER_HEIGHT));
                ui.painter()
                    .with_clip_rect(clip)
                    .rect_filled(corner, 0., ui.visuals().panel_fill);
                ui.painter().with_clip_rect(clip).text(
                    corner.left_center() + vec2(7., 0.),
                    egui::Align2::LEFT_CENTER,
                    "Elements",
                    egui::FontId::proportional(12.),
                    ui.visuals().weak_text_color(),
                );
            });
        self.edit_toolbar(ui, elements, &mut output);
        output
    }

    fn update_drag(
        &mut self,
        ui: &egui::Ui,
        elements: &[Value],
        output: &mut Output,
    ) -> Option<(u64, Span)> {
        let drag = self.drag.as_mut()?;
        let unchanged = elements
            .iter()
            .find(|e| uid(e) == drag.uid)
            .is_some_and(|e| span(e) == drag.original && text(e, "timeRefName") == drag.reference);
        if !unchanged {
            self.drag = None;
            return None;
        }
        if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
            let delta = pointer.x - drag.origin.x;
            if delta.abs() >= 3. {
                drag.moved = true;
            }
            if drag.moved {
                drag.value = drag.original.shifted(drag.mode, delta as f64 / self.scale);
            }
        }
        let preview = (drag.uid, drag.value);
        if ui.input(|input| input.pointer.primary_released()) {
            let drag = self.drag.take().unwrap();
            let start = (drag.value.start + 0.5).floor();
            let end = (drag.value.end + 0.5).floor();
            if drag.moved && (start != drag.original.start || end != drag.original.end) {
                output.commands.push((
                    "set_anim_time",
                    json!({"element":element_arg(drag.uid),"startMs":start,"endMs":end}),
                ));
            }
        }
        Some(preview)
    }

    fn transport(
        &mut self,
        ui: &mut egui::Ui,
        data: &Value,
        elements: &[Value],
        has_view: bool,
        can_seek: bool,
        content: f64,
        output: &mut Output,
    ) {
        let playing = data["mgr"]["playState"] == "playing";
        ui.horizontal_wrapped(|ui| {
            for (name, label, enabled) in [
                ("Start", "⏮", can_seek),
                (
                    if playing { "Pause" } else { "Play" },
                    if playing { "⏸" } else { "▶" },
                    can_seek && number(&data["mgr"], "lengthMs") > 0.,
                ),
                ("Stop", "■", has_view),
                ("End", "⏭", can_seek),
            ] {
                let response = ui
                    .add_enabled(enabled, egui::Button::new(label))
                    .on_hover_text(name);
                record(&mut self.rects, format!("Animation {name}"), response.rect);
                if response.clicked() {
                    output.commands.push(match name {
                        "Start" => seek(0.),
                        "End" => seek(number(&data["mgr"], "lengthMs")),
                        _ => ("animate", json!({"action":name.to_lowercase()})),
                    });
                }
            }
            let mut value = self
                .time
                .or(self.scrub)
                .unwrap_or_else(|| number(&data["mgr"], "elapsedMs"));
            let response = ui.add_enabled(
                can_seek,
                egui::DragValue::new(&mut value)
                    .speed(10.)
                    .range(0.0..=content)
                    .custom_formatter(|value, _| clock(value))
                    .custom_parser(parse_clock)
                    .update_while_editing(false),
            );
            record(&mut self.rects, "Animation current time", response.rect);
            if !self.cancelled {
                if response.dragged() {
                    self.time = Some(value);
                } else if response.changed() || response.drag_stopped() {
                    self.time = None;
                    output.commands.push(seek(value));
                }
            }
            ui.weak(format!("/ {}", clock(number(&data["mgr"], "lengthMs"))));
            ui.label("Start camera");
            let mut start = text(&data["mgr"], "startcam").to_string();
            let combo = egui::ComboBox::from_id_salt("animation_start_camera")
                .width(120.)
                .selected_text(if start.is_empty() { "(none)" } else { &start })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut start, String::new(), "(none)");
                    for camera in data["cameras"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        ui.selectable_value(&mut start, camera.to_string(), camera);
                    }
                });
            record(
                &mut self.rects,
                "Animation start camera",
                combo.response.rect,
            );
            if start != text(&data["mgr"], "startcam") {
                output
                    .commands
                    .push(("set_anim_options", json!({"startCamera":start})));
            }
            let mut looping = data["mgr"]["loop"] == true;
            let response = ui.checkbox(&mut looping, "Loop");
            record(&mut self.rects, "Animation Loop", response.rect);
            if response.changed() {
                output
                    .commands
                    .push(("set_anim_options", json!({"loop":looping})));
            }
            ui.weak(format!(
                "{} elements · {} FPS",
                elements.len(),
                data["fps"].as_u64().unwrap_or(30)
            ));
            let fit_width = (ui.max_rect().width() - LABEL_WIDTH).max(1.) as f64;
            ui.allocate_ui_with_layout(
                vec2(88., ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    for label in ["Fit", "−", "+"] {
                        let response = ui.button(label);
                        record(
                            &mut self.rects,
                            format!("Animation zoom {label}"),
                            response.rect,
                        );
                        if response.clicked() {
                            self.scale = match label {
                                "Fit" => fit_scale(content, fit_width),
                                "−" => clamp_scale(self.scale / 1.4),
                                _ => clamp_scale(self.scale * 1.4),
                            };
                            self.drag = None;
                        }
                    }
                },
            );
        });
    }

    fn edit_toolbar(&mut self, ui: &mut egui::Ui, elements: &[Value], output: &mut Output) {
        let selected = self
            .selected
            .and_then(|id| elements.iter().position(|e| uid(e) == id));
        ui.horizontal(|ui| {
            let add = ui
                .menu_button("+", |ui| {
                    for (kind, label) in [
                        ("spin", "Simple spin"),
                        ("camera", "Camera motion"),
                        ("show", "Show"),
                        ("hide", "Hide"),
                        ("slidein", "Slide in"),
                        ("slideout", "Slide out"),
                        ("mol", "Molecule morphing"),
                        ("wait", "No operation"),
                    ] {
                        let response = ui.button(label);
                        record(
                            &mut self.rects,
                            format!("Animation add {kind}"),
                            response.rect,
                        );
                        if response.clicked() {
                            let mut args = json!({"type":kind});
                            if let Some(next) = selected.and_then(|i| elements.get(i + 1)) {
                                args["before"] = json!(element_arg(uid(next)));
                            }
                            output.commands.push(("add_anim", args));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Add element");
            record(&mut self.rects, "Animation Add", add.rect);
            for (label, enabled) in [
                ("Delete", selected.is_some()),
                ("↑", selected.is_some_and(|i| i > 0)),
                ("↓", selected.is_some_and(|i| i + 1 < elements.len())),
                ("Properties…", selected.is_some()),
            ] {
                let glyph = match label {
                    "↑" => egui_phosphor::regular::ARROW_UP,
                    "↓" => egui_phosphor::regular::ARROW_DOWN,
                    _ => label,
                };
                let response = ui.add_enabled(enabled, egui::Button::new(glyph));
                record(&mut self.rects, format!("Animation {label}"), response.rect);
                if response.clicked() {
                    let index = selected.unwrap();
                    let id = uid(&elements[index]);
                    match label {
                        "Delete" => {
                            output
                                .commands
                                .push(("remove_anim", json!({"element":element_arg(id)})));
                            self.selected = elements
                                .get(index + 1)
                                .or_else(|| index.checked_sub(1).and_then(|i| elements.get(i)))
                                .map(uid);
                        }
                        "Properties…" => output.inspect = Some(id),
                        _ => output.commands.push((
                            "move_anim",
                            json!({"element":element_arg(id),"by":if label=="↑" {-1}else{1}}),
                        )),
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests;
