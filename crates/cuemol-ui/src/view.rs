use cuemol_core::Uid;
use cuemol_desktop_core::ViewTransform;
use cuemol_qsys::{Camera, CenterMark};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct ViewPanel {
    pub rects: BTreeMap<String, [f32; 4]>,
    context: Option<Uid>,
    rotation: [f64; 3],
    ids: BTreeMap<String, egui::Id>,
    focus: Option<String>,
    drag: Option<Drag>,
}

#[derive(Debug)]
struct Drag {
    field: String,
    raw: f64,
    sent: f64,
}

impl ViewPanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        camera: &mut Camera,
        view_id: Option<Uid>,
        viewport: [u32; 2],
    ) -> bool {
        self.rects.clear();
        if self.context != view_id {
            self.context = view_id;
            self.rotation = [0.; 3];
            self.ids.clear();
            self.focus = None;
            self.drag = None;
        }
        let mut changed = false;
        ui.push_id(("view-controls", view_id), |ui| {
            ui.add_enabled_ui(view_id.is_some(), |ui| {
                for section in ["Rotation", "Translation", "Zoom / Slab"] {
                    ui.label(egui::RichText::new(section).strong());
                    egui::Grid::new(section)
                        .num_columns(2)
                        .spacing([8., 4.])
                        .show(ui, |ui| {
                            for axis in 0..3 {
                                let center = camera.center();
                                let (label, value, decimals, unit) = match section {
                                    "Rotation" => (
                                        ["RotX", "RotY", "RotZ"][axis],
                                        self.rotation[axis],
                                        0,
                                        "°",
                                    ),
                                    "Translation" => (
                                        ["TraX", "TraY", "TraZ"][axis],
                                        [center.x, center.y, center.z][axis],
                                        2,
                                        "Å",
                                    ),
                                    _ => (
                                        ["Zoom", "Slab", "Dist"][axis],
                                        [camera.zoom(), camera.slab_depth(), camera.cam_dist()]
                                            [axis],
                                        0,
                                        "Å",
                                    ),
                                };
                                ui.label(label);
                                let edit = self.field(ui, label, value, decimals, unit);
                                let Some((value, relative)) = edit else {
                                    ui.end_row();
                                    continue;
                                };
                                let transform = match section {
                                    "Rotation" => {
                                        let delta = if relative {
                                            value
                                        } else {
                                            value - self.rotation[axis]
                                        };
                                        self.rotation[axis] += delta;
                                        let mut angles = [0.; 3];
                                        angles[axis] = delta;
                                        ViewTransform::Rotate(angles)
                                    }
                                    "Translation" if relative => {
                                        let mut delta = [0.; 3];
                                        delta[axis] = value;
                                        ViewTransform::Translate(delta)
                                    }
                                    "Translation" => {
                                        let mut center = [center.x, center.y, center.z];
                                        center[axis] = value;
                                        ViewTransform::Center(center)
                                    }
                                    _ => {
                                        let absolute = if relative {
                                            [camera.zoom(), camera.slab_depth(), camera.cam_dist()]
                                                [axis]
                                                + value
                                        } else {
                                            value
                                        };
                                        match axis {
                                            0 => ViewTransform::Zoom(absolute.max(0.01)),
                                            1 => ViewTransform::Slab(absolute.max(0.)),
                                            _ => ViewTransform::Distance(absolute.max(0.)),
                                        }
                                    }
                                };
                                changed |= transform.apply(camera, viewport).is_ok();
                                ui.end_row();
                            }
                        });
                    ui.add_space(6.);
                }
                ui.label(egui::RichText::new("Projection").strong());
                egui::Grid::new("projection")
                    .num_columns(2)
                    .spacing([8., 4.])
                    .show(ui, |ui| {
                        ui.label("Perspective");
                        let mut perspective = camera.is_perspective();
                        let response = ui.checkbox(&mut perspective, "");
                        self.record("Perspective", &response);
                        if response.changed() {
                            camera.set_perspective(perspective);
                            changed = true;
                        }
                        ui.end_row();
                        ui.label("Center mark");
                        let mut mark = camera.center_mark();
                        let response = egui::ComboBox::from_id_salt("center-mark")
                            .width((ui.available_width() - 8.).max(65.))
                            .selected_text(mark_label(mark))
                            .show_ui(ui, |ui| {
                                for choice in
                                    [CenterMark::None, CenterMark::Cross, CenterMark::Axis]
                                {
                                    let response =
                                        ui.selectable_value(&mut mark, choice, mark_label(choice));
                                    self.record(
                                        &format!("Center mark {}", mark_label(choice)),
                                        &response,
                                    );
                                }
                            });
                        self.record("Center mark", &response.response);
                        if mark != camera.center_mark() {
                            camera.set_center_mark(mark);
                            changed = true;
                        }
                        ui.end_row();
                    });
            });
        });
        if !ui.input(|input| input.pointer.primary_down()) {
            self.rotation = [0.; 3];
            self.drag = None;
        }
        if let Some(label) = self.focus.take() {
            if let Some(id) = self.ids.get(&label) {
                ui.memory_mut(|memory| memory.request_focus(*id));
                let mut text = egui::TextEdit::load_state(ui.ctx(), *id).unwrap_or_default();
                text.cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(usize::MAX),
                    )));
                text.store(ui.ctx(), *id);
            }
        }
        changed
    }

    fn field(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        value: f64,
        decimals: usize,
        unit: &str,
    ) -> Option<(f64, bool)> {
        let mut edited = value;
        let mut arrow = 0.;
        let mut enter = false;
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size.x = (ui.available_width() - 4.).max(65.);
                let id = ui.next_auto_id();
                if ui.memory(|memory| memory.has_focus(id)) {
                    ui.memory_mut(|memory| {
                        memory.set_focus_lock_filter(
                            id,
                            egui::EventFilter {
                                vertical_arrows: true,
                                ..Default::default()
                            },
                        )
                    });
                    ui.input(|input| {
                        arrow = f64::from(input.key_pressed(egui::Key::ArrowUp))
                            - f64::from(input.key_pressed(egui::Key::ArrowDown));
                        enter = input.key_pressed(egui::Key::Enter);
                    });
                    if arrow != 0. {
                        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                    }
                }
                ui.add(
                    egui::DragValue::new(&mut edited)
                        .speed(1.)
                        .fixed_decimals(decimals)
                        .suffix(format!(" {unit}"))
                        .update_while_editing(false),
                )
            })
            .inner;
        self.ids.insert(label.into(), response.id);
        self.record(label, &response);
        if enter && response.lost_focus() {
            self.focus = match label {
                "TraX" => Some("TraY".into()),
                "TraY" => Some("TraZ".into()),
                _ => None,
            };
        }
        if response.dragged() || arrow != 0. {
            let scalar = matches!(label, "Zoom" | "Slab" | "Dist");
            let next = if arrow != 0. {
                if scalar {
                    value + arrow
                } else {
                    arrow
                }
            } else {
                let modifiers = ui.input(|input| input.modifiers);
                let scale = if modifiers.shift { 0.1 } else { 1. };
                let snap = if modifiers.shift {
                    if label.starts_with("Tra") {
                        0.01
                    } else {
                        0.1
                    }
                } else if modifiers.ctrl || modifiers.command {
                    10.
                } else {
                    1.
                };
                let start = self.drag.as_ref().is_none_or(|drag| drag.field != label);
                if start {
                    self.drag = Some(Drag {
                        field: label.into(),
                        raw: value,
                        sent: value,
                    });
                }
                let pixels = if start {
                    ui.input(|input| {
                        input
                            .pointer
                            .interact_pos()
                            .zip(input.pointer.press_origin())
                            .map_or(0., |(pos, origin)| pos.x - origin.x)
                    })
                } else {
                    response.drag_delta().x
                };
                let drag = self.drag.as_mut().unwrap();
                drag.raw += f64::from(pixels) * scale;
                let minimum = match label {
                    "Zoom" => 0.01,
                    "Slab" | "Dist" => 0.,
                    _ => f64::NEG_INFINITY,
                };
                drag.raw = drag.raw.max(minimum);
                let snapped = ((drag.raw / snap + 0.5).floor() * snap).max(minimum);
                let delta = snapped - drag.sent;
                drag.sent = snapped;
                if scalar {
                    snapped
                } else {
                    delta
                }
            };
            ui.data_mut(|data| {
                data.remove::<String>(response.id);
                data.remove::<f64>(response.id);
            });
            return if scalar {
                (next != value).then_some((next, false))
            } else {
                (next != 0.).then_some((next, true))
            };
        }
        if response.changed() && edited.is_finite() && edited != value {
            ui.data_mut(|data| data.remove::<String>(response.id));
            return Some((edited, false));
        }
        None
    }

    fn record(&mut self, name: &str, response: &egui::Response) {
        let rect = response.rect;
        self.rects.insert(
            format!("View {name}"),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }
}

fn mark_label(mark: CenterMark) -> &'static str {
    match mark {
        CenterMark::None => "None",
        CenterMark::Cross => "Crosshair",
        CenterMark::Axis => "Axis",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        panel: ViewPanel,
        camera: Camera,
        ctx: egui::Context,
        view_id: Uid,
        modifiers: egui::Modifiers,
    }
    impl Harness {
        fn new() -> Self {
            Self {
                panel: Default::default(),
                camera: Camera::new(),
                ctx: Default::default(),
                view_id: Uid::new(1),
                modifiers: Default::default(),
            }
        }
        fn frame(&mut self, events: Vec<egui::Event>) {
            let ctx = self.ctx.clone();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 700.),
                    )),
                    events,
                    modifiers: self.modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("view-test")
                        .exact_width(260.)
                        .show(ctx, |ui| {
                            self.panel
                                .show(ui, &mut self.camera, Some(self.view_id), [800, 200]);
                        });
                },
            );
        }
        fn pos(&mut self, key: &str) -> egui::Pos2 {
            self.frame(Vec::new());
            self.frame(Vec::new());
            let [x, y, w, h] = self.panel.rects[key];
            egui::pos2(x + w / 2., y + h / 2.)
        }
        fn pointer(&mut self, pos: egui::Pos2, pressed: bool) {
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
        fn click(&mut self, key: &str) {
            let pos = self.pos(key);
            self.pointer(pos, true);
            self.pointer(pos, false);
            self.frame(Vec::new());
        }
        fn key(&mut self, key: egui::Key) {
            for pressed in [true, false] {
                self.frame(vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }]);
            }
        }
        fn text(&mut self, key: &str, value: &str) {
            self.click(key);
            self.frame(vec![egui::Event::Text(value.into())]);
            self.key(egui::Key::Enter);
            self.frame(Vec::new());
        }
        fn drag(&mut self, key: &str, delta: egui::Vec2) {
            let start = self.pos(key);
            self.pointer(start, true);
            self.frame(vec![egui::Event::PointerMoved(start + delta * 0.5)]);
            self.frame(vec![egui::Event::PointerMoved(start + delta)]);
            self.pointer(start + delta, false);
            self.frame(Vec::new());
        }
    }

    #[test]
    fn view_numeric_inputs_commit_once_and_chain_translation_fields() {
        let mut h = Harness::new();
        h.text("View RotZ", "90");
        assert!((h.camera.rotation().w - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
        assert_eq!(h.panel.rotation, [0.; 3]);
        for _ in 0..3 {
            h.frame(Vec::new());
        }
        assert!((h.camera.rotation().z - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
        h.text("View TraX", "12.25");
        assert_eq!(h.camera.center().x, 12.25);
        assert!(h.ctx.memory(|memory| memory.has_focus(h.panel.ids["TraY"])));
        h.frame(vec![egui::Event::Text("-8.5".into())]);
        h.key(egui::Key::Enter);
        assert_eq!(h.camera.center().y, -8.5);
        h.frame(vec![egui::Event::Text("3".into())]);
        h.key(egui::Key::Enter);
        assert_eq!(h.camera.center().z, 3.);
        h.camera.set_slab_depth(300.);
        h.text("View Dist", "10");
        assert_eq!(h.camera.cam_dist(), 10.);
        assert_eq!(h.camera.slab_depth(), 300.);
        h.text("View Slab", "500");
        assert_eq!(h.camera.slab_depth(), 20.);
        h.text("View Zoom", "-10");
        assert_eq!(h.camera.zoom(), 0.01);
    }

    #[test]
    fn view_drags_and_arrow_steps_pan_in_rotated_view_coordinates() {
        let mut h = Harness::new();
        ViewTransform::Rotate([30., 45., 90.])
            .apply(&mut h.camera, [800, 200])
            .unwrap();
        let mut expected = h.camera.clone();
        ViewTransform::Translate([20., 0., 0.])
            .apply(&mut expected, [800, 200])
            .unwrap();
        h.drag("View TraX", egui::vec2(20., 18.));
        let actual = h.camera.center();
        let wanted = expected.center();
        assert!(
            (actual.x - wanted.x).abs() < 1e-9
                && (actual.y - wanted.y).abs() < 1e-9
                && (actual.z - wanted.z).abs() < 1e-9,
            "{actual:?} != {wanted:?}"
        );
        h.click("View TraY");
        ViewTransform::Translate([0., 1., 0.])
            .apply(&mut expected, [800, 200])
            .unwrap();
        h.key(egui::Key::ArrowUp);
        assert_eq!(h.camera.center(), expected.center());
        h.key(egui::Key::Escape);
        h.frame(Vec::new());
        assert_eq!(h.camera.center(), expected.center());
        let before = h.camera.clone();
        h.drag("View RotX", egui::vec2(0., 30.));
        assert_eq!(h.camera, before);
        h.camera.set_zoom(10.);
        h.modifiers.shift = true;
        h.drag("View Zoom", egui::vec2(20., 0.));
        assert!((h.camera.zoom() - 12.).abs() < 1e-9);
        h.modifiers.shift = false;
        h.modifiers.ctrl = true;
        h.drag("View Zoom", egui::vec2(20., 0.));
        assert_eq!(h.camera.zoom(), 30.);
    }

    #[test]
    fn changing_view_discards_uncommitted_numeric_input() {
        let mut h = Harness::new();
        h.click("View TraX");
        h.frame(vec![egui::Event::Text("72".into())]);
        assert_eq!(h.camera.center().x, 0.);
        h.view_id = Uid::new(2);
        h.key(egui::Key::Enter);
        h.frame(Vec::new());
        assert_eq!(h.camera.center().x, 0.);
        h.click("View Perspective");
        assert!(h.camera.is_perspective());
        h.click("View Center mark");
        h.click("View Center mark Axis");
        assert_eq!(h.camera.center_mark(), CenterMark::Axis);
    }
}
