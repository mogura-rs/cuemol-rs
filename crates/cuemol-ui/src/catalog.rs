use cuemol_desktop_core::DesktopAppState;
use egui::{Response, Ui};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Catalog {
    pub rects: BTreeMap<String, [f32; 4]>,
    examples: Examples,
    rows: Properties,
    mock: Properties,
    generic: bool,
    selection: crate::selection::Builder,
    property_selection: crate::selection::Builder,
}

#[derive(Debug)]
struct Examples {
    text: String,
    kind: String,
    filter: String,
    secret: String,
    multiline: String,
    combo: String,
    number: f64,
    number2: f64,
    drag: f64,
    time: f64,
    vector: [f64; 3],
    vector_text: String,
    enabled: bool,
    gated: bool,
    location: [f64; 3],
    selection: String,
    opacity: f64,
    angle: f64,
    color: egui::Color32,
    list: usize,
    tree: bool,
    segment: usize,
    radio: usize,
}
impl Default for Examples {
    fn default() -> Self {
        Self {
            text: "aname CA".into(),
            kind: "ribbon".into(),
            filter: String::new(),
            secret: String::new(),
            multiline: String::new(),
            combo: "gpt-5.6".into(),
            number: 50.,
            number2: 8.,
            drag: 1.,
            time: 1500.,
            vector: [1., 2., 3.],
            vector_text: "(1,2,3)".into(),
            enabled: true,
            gated: true,
            location: [-0.52, -0.7, 0.54612],
            selection: "*".into(),
            opacity: 80.,
            angle: 90.,
            color: egui::Color32::from_rgb(0x3b, 0x82, 0xf6),
            list: 1,
            tree: true,
            segment: 0,
            radio: 0,
        }
    }
}

#[derive(Debug)]
struct Property<T> {
    value: T,
    default: T,
    modified: bool,
}
impl<T: Clone> Property<T> {
    fn new(value: T, default: T, modified: bool) -> Self {
        Self {
            value,
            default,
            modified,
        }
    }
    fn reset(&mut self) {
        self.value = self.default.clone();
        self.modified = false;
    }
}
#[derive(Debug)]
struct Properties {
    label: Property<String>,
    visible: Property<bool>,
    opacity: Property<f64>,
    width: Property<f64>,
    selection: Property<String>,
}
impl Default for Properties {
    fn default() -> Self {
        Self {
            label: Property::new("backbone".into(), "chain A".into(), true),
            visible: Property::new(false, true, true),
            opacity: Property::new(0.4, 1., true),
            width: Property::new(1., 1., false),
            selection: Property::new("resn ALA".into(), "*".into(), true),
        }
    }
}
impl Default for Catalog {
    fn default() -> Self {
        let mut mock = Properties::default();
        mock.width.value = 2.5;
        mock.width.modified = true;
        Self {
            rects: BTreeMap::new(),
            examples: Examples::default(),
            rows: Properties::default(),
            mock,
            generic: false,
            selection: crate::selection::Builder::default(),
            property_selection: crate::selection::Builder::default(),
        }
    }
}
impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn show(&mut self, ui: &mut Ui, state: &DesktopAppState, revision: u64) {
        self.rects.clear();
        let height = ((ui.available_height() - 90.) / 3.).clamp(90., 280.);
        for index in 0..3 {
            let response = egui::CollapsingHeader::new(format!("Catalog {}", index + 1))
                .id_salt(("catalog-pane", index))
                .default_open(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("catalog-scroll", index))
                        .auto_shrink([false, false])
                        .max_height(height)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.spacing_mut().text_edit_width = ui.available_width();
                            match index {
                                0 => forms(ui, &mut self.examples, &mut self.rects),
                                1 => lists(
                                    ui,
                                    &mut self.examples,
                                    &mut self.selection,
                                    state,
                                    revision,
                                    &mut self.rects,
                                ),
                                _ => properties(
                                    ui,
                                    &mut self.rows,
                                    &mut self.mock,
                                    &mut self.generic,
                                    &mut self.property_selection,
                                    state,
                                    revision,
                                    &mut self.rects,
                                ),
                            }
                        });
                });
            record(
                ui,
                &mut self.rects,
                &format!("Catalog {}", index + 1),
                &response.header_response,
            );
            if let Some(body) = response.body_response {
                record(
                    ui,
                    &mut self.rects,
                    &format!("Catalog {} content", index + 1),
                    &body,
                );
            }
        }
    }
}

fn record(ui: &Ui, rects: &mut BTreeMap<String, [f32; 4]>, key: &str, response: &Response) {
    let r = response.rect.intersect(ui.clip_rect());
    if r.is_positive() {
        rects.insert(key.into(), [r.left(), r.top(), r.width(), r.height()]);
    }
}
fn group(ui: &mut Ui, title: &str, draw: impl FnOnce(&mut Ui)) {
    ui.add_space(6.);
    ui.strong(title);
    ui.separator();
    draw(ui);
    ui.add_space(10.);
}
fn text(ui: &mut Ui, label: &str, value: &mut String, rects: &mut BTreeMap<String, [f32; 4]>) {
    ui.label(label);
    let r = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    record(ui, rects, &format!("Catalog {label}"), &r);
}
fn choice(ui: &mut Ui, label: &str, value: &mut String, options: &[(&str, &str)]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label)
        .width(ui.available_width())
        .selected_text(
            options
                .iter()
                .find(|(v, _)| *v == value)
                .map_or(value.as_str(), |(_, label)| *label),
        )
        .show_ui(ui, |ui| {
            for (key, label) in options {
                ui.selectable_value(value, (*key).into(), *label);
            }
        });
}
fn number(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    unit: &str,
    rects: &mut BTreeMap<String, [f32; 4]>,
) -> bool {
    ui.label(label);
    numeric_input(ui, label, value, range, step, unit, rects)
}
fn numeric_input(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    unit: &str,
    rects: &mut BTreeMap<String, [f32; 4]>,
) -> bool {
    let speed = if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
        step * 10.
    } else if ui.input(|i| i.modifiers.shift) {
        step / 10.
    } else {
        step
    };
    let r = ui.add_sized(
        [ui.available_width(), 20.],
        egui::DragValue::new(value)
            .range(range)
            .speed(speed)
            .suffix(unit)
            .update_while_editing(false),
    );
    record(ui, rects, &format!("Catalog {label}"), &r);
    r.changed()
}
fn forms(ui: &mut Ui, e: &mut Examples, rects: &mut BTreeMap<String, [f32; 4]>) {
    group(ui, "Label hierarchy (FieldSection vs Field)", |ui| {
        choice(
            ui,
            "Molecule",
            &mut e.kind,
            &[("ribbon", "1CRN"), ("cpk", "3J3Q")],
        );
        ui.strong("Term");
        text(ui, "Dist", &mut e.text, rects);
    });
    group(ui, "Text & Select", |ui| {
        text(ui, "TextField", &mut e.text, rects);
        ui.label("TextField (invalid)");
        let mut invalid = "bogus(".to_string();
        egui::Frame::NONE
            .stroke(egui::Stroke::new(1., ui.visuals().error_fg_color))
            .show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut invalid).desired_width(f32::INFINITY));
            });
        ui.add_enabled_ui(false, |ui| {
            text(ui, "TextField (disabled)", &mut "locked".to_string(), rects);
        });
        ui.label("TextField (filter / leftIcon)");
        ui.horizontal(|ui| {
            ui.label("⌕");
            ui.add(
                egui::TextEdit::singleline(&mut e.filter)
                    .hint_text("Filter…")
                    .desired_width(f32::INFINITY),
            );
        });
        ui.label("TextField (password)");
        ui.add(
            egui::TextEdit::singleline(&mut e.secret)
                .password(true)
                .hint_text("API key")
                .desired_width(f32::INFINITY),
        );
        text(
            ui,
            "ComboBoxField (labelled suggestions)",
            &mut e.combo,
            rects,
        );
        egui::ComboBox::from_id_salt("catalog-suggestions")
            .selected_text("Suggestions")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut e.combo, "gpt-6-astra".into(), "Most capable");
                ui.selectable_value(&mut e.combo, "gpt-5.6".into(), "Flagship");
            });
        ui.label("TextAreaField");
        let id = ui.make_persistent_id("catalog-multiline");
        if ui.memory(|memory| memory.has_focus(id)) {
            ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter);
                input.consume_key(egui::Modifiers::CTRL, egui::Key::Enter);
            });
        }
        let rows = e.multiline.lines().count().clamp(1, 6);
        egui::ScrollArea::vertical()
            .id_salt("catalog-multiline-scroll")
            .max_height(ui.text_style_height(&egui::TextStyle::Body) * 6. + 8.)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut e.multiline)
                        .id(id)
                        .desired_rows(rows)
                        .desired_width(f32::INFINITY)
                        .hint_text("Cmd/Ctrl+Enter submits"),
                );
            });
        choice(
            ui,
            "SelectField",
            &mut e.kind,
            &[("ribbon", "Ribbon"), ("cpk", "CPK"), ("stick", "Stick")],
        );
    });
    group(ui, "Numeric & Switch", |ui| {
        ui.label("NumericField (slider + unit)");
        ui.add(egui::Slider::new(&mut e.number, 0. ..=100.).suffix(" Å"));
        number(
            ui,
            "NumericField (no slider)",
            &mut e.number2,
            0. ..=100.,
            1.,
            "",
            rects,
        );
        number(
            ui,
            "DragNumericField (drag-snap / click-edit)",
            &mut e.drag,
            0. ..=10.,
            0.1,
            " Å",
            rects,
        );
        ui.label("TimeField (segments / drag / Ctrl+wheel / Enter to type)");
        ui.horizontal(|ui| {
            for (part, scale) in [
                ("hours", 3_600_000.),
                ("minutes", 60_000.),
                ("seconds", 1000.),
                ("milliseconds", 1.),
            ] {
                let mut value = (e.time / scale).floor()
                    % if part == "hours" {
                        100.
                    } else if part == "milliseconds" {
                        1000.
                    } else {
                        60.
                    };
                let before = value;
                let r = ui.add(
                    egui::DragValue::new(&mut value)
                        .speed(1.)
                        .range(
                            0. ..=if part == "hours" {
                                99.
                            } else if part == "milliseconds" {
                                999.
                            } else {
                                59.
                            },
                        )
                        .max_decimals(0)
                        .min_decimals(0),
                );
                if r.changed() {
                    e.time += (value - before) * scale;
                }
                if r.hovered() && ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                    let dy = ui.input(|i| i.raw_scroll_delta.y);
                    if dy != 0. {
                        e.time = (e.time + f64::from(dy.signum()) * scale).max(0.);
                    }
                }
                record(ui, rects, &format!("Catalog time {part}"), &r);
            }
        });
        let r = ui.add_sized(
            [ui.available_width(), 20.],
            egui::DragValue::new(&mut e.time)
                .range(0. ..=f64::MAX)
                .custom_formatter(|v, _| crate::animation::clock(v))
                .custom_parser(crate::animation::parse_clock)
                .speed(10.),
        );
        record(ui, rects, "Catalog time text", &r);
        ui.label("VectorField (x/y/z cells; qlib::Vector4D text)");
        let before = e.vector;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 16.) / 3.;
            for (i, value) in e.vector.iter_mut().enumerate() {
                let r = ui.add_sized([width, 20.], egui::DragValue::new(value).speed(0.1));
                record(ui, rects, &format!("Catalog vector {i}"), &r);
            }
        });
        if e.vector != before {
            e.vector_text = format!("({},{},{})", e.vector[0], e.vector[1], e.vector[2]);
        }
        let r = ui.add(egui::TextEdit::singleline(&mut e.vector_text).desired_width(f32::INFINITY));
        record(ui, rects, "Catalog Vector text", &r);
        if r.lost_focus() {
            if let Ok(v) = cuemol_core::Vector4::from_string(&e.vector_text) {
                if [v.x(), v.y(), v.z()].iter().all(|n| n.is_finite()) {
                    e.vector = [v.x(), v.y(), v.z()];
                }
            }
        }
        let r = ui.checkbox(&mut e.enabled, "SwitchField");
        record(ui, rects, "Catalog SwitchField", &r);
        ui.checkbox(&mut e.enabled, "CheckboxField (inline controlFirst)");
        let r = ui.checkbox(
            &mut e.gated,
            "GatedControl (checkbox + control, one property)",
        );
        record(ui, rects, "Catalog gate", &r);
        ui.add_enabled_ui(e.gated, |ui| {
            number(ui, "Gated value", &mut e.drag, 0. ..=10., 0.1, " Å", rects);
        });
    });
}
fn selection(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    builder: &mut crate::selection::Builder,
    state: &DesktopAppState,
    revision: u64,
    rects: &mut BTreeMap<String, [f32; 4]>,
) -> bool {
    let previous = value.clone();
    text(ui, label, value, rects);
    egui::CollapsingHeader::new("Selection picker")
        .id_salt(label)
        .show(ui, |ui| {
            if let Some(applied) = builder.show(ui, state, None, revision, value) {
                *value = applied.expression;
            }
        });
    *value != previous
}
fn lists(
    ui: &mut Ui,
    e: &mut Examples,
    builder: &mut crate::selection::Builder,
    state: &DesktopAppState,
    revision: u64,
    rects: &mut BTreeMap<String, [f32; 4]>,
) {
    group(ui, "FieldGrid (aligned label column)", |ui| {
        egui::Grid::new("catalog-location")
            .num_columns(2)
            .show(ui, |ui| {
                for (label, value) in ["Location X", "Y", "Z"].into_iter().zip(&mut e.location) {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1).suffix(" m"));
                    ui.end_row();
                }
            });
    });
    group(ui, "Shared widgets", |ui| {
        selection(
            ui,
            "MolSelList (selection picker)",
            &mut e.selection,
            builder,
            state,
            revision,
            rects,
        );
        ui.horizontal(|ui| {
            ui.label("ColorField");
            ui.color_edit_button_srgba(&mut e.color);
        });
        ui.label("Opacity");
        ui.add(egui::Slider::new(&mut e.opacity, 0. ..=100.).suffix("%"));
        number(
            ui,
            "Stride (slider=false)",
            &mut e.opacity,
            1. ..=9999.,
            1.,
            "",
            rects,
        );
        number(
            ui,
            "Angle (hideLabel, in a Field)",
            &mut e.angle,
            0. ..=360.,
            1.,
            "°",
            rects,
        );
    });
    group(ui, "Listbox (list / tree row)", |ui| {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            for (i, label) in ["1CRN", "3J3Q", "Water", "Ligand"].into_iter().enumerate() {
                let r = ui.selectable_value(&mut e.list, i, label);
                record(ui, rects, &format!("Catalog list {label}"), &r);
            }
        });
    });
    group(ui, "Tree (ListboxTree) / disclosure caret", |ui| {
        let _ = crate::shell::tree_row(
            ui,
            "1ggg (MolCoord)",
            crate::shell::Icon::Atom,
            false,
            Some(&mut e.tree),
            None,
        );
        if e.tree {
            ui.indent("catalog-renderers", |ui| {
                ui.label("ribbon1 (ribbon)");
                ui.label("simple1 (simple)");
            });
        }
        ui.label("▾  ▸  ·  expanded / collapsed / leaf");
    });
    group(ui, "Segmented control", |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, label) in ["All", "Backbone", "Sidechain"].into_iter().enumerate() {
                ui.selectable_value(&mut e.segment, i, label);
            }
        });
    });
    group(ui, "Radio group", |ui| {
        ui.label("Location");
        ui.horizontal(|ui| {
            ui.radio_value(&mut e.radio, 0, "Temporary");
            ui.radio_value(&mut e.radio, 1, "Custom");
        });
    });
    group(ui, "Buttons", |ui| {
        ui.horizontal_wrapped(|ui| {
            let _ = ui.button("Default");
            let _ = ui.add(egui::Button::new("Primary").fill(ui.visuals().selection.bg_fill));
            let _ = ui.button("✓ Icon");
            let _ = ui.add(egui::Button::new("Minimal").frame(false));
            ui.add_enabled(false, egui::Button::new("Disabled"));
        });
    });
}
fn property<T: Clone>(
    ui: &mut Ui,
    name: &str,
    key: &str,
    p: &mut Property<T>,
    default_label: &str,
    rects: &mut BTreeMap<String, [f32; 4]>,
    editor: impl FnOnce(&mut Ui, &mut T, &mut BTreeMap<String, [f32; 4]>) -> bool,
) {
    let start = ui.cursor().min;
    ui.horizontal(|ui| {
        ui.label(name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui
                .add_enabled_ui(p.modified, |ui| {
                    crate::shell::icon_button(
                        ui,
                        crate::shell::Icon::Undo,
                        &format!("Reset {key}"),
                        false,
                        egui::vec2(20., 20.),
                    )
                })
                .inner
                .on_hover_text(format!("Reset to default: {default_label}"));
            record(ui, rects, &format!("Catalog reset {key}"), &r);
            if r.clicked() {
                p.reset();
            }
        });
    });
    if editor(ui, &mut p.value, rects) {
        p.modified = true;
    }
    if p.modified {
        let end = ui.cursor().min;
        ui.painter().line_segment(
            [start, end],
            egui::Stroke::new(2., ui.visuals().selection.bg_fill),
        );
    }
    ui.add_space(6.);
}
#[allow(clippy::too_many_arguments)]
fn properties(
    ui: &mut Ui,
    rows: &mut Properties,
    mock: &mut Properties,
    generic: &mut bool,
    builder: &mut crate::selection::Builder,
    state: &DesktopAppState,
    revision: u64,
    rects: &mut BTreeMap<String, [f32; 4]>,
) {
    group(ui, "Property rows (PropertyField)", |ui| {
        property(
            ui,
            "Label",
            "Label",
            &mut rows.label,
            "chain A",
            rects,
            |ui, v, rects| {
                let r = ui.text_edit_singleline(v);
                record(ui, rects, "Catalog property Label", &r);
                r.changed()
            },
        );
        property(
            ui,
            "Visible",
            "Visible",
            &mut rows.visible,
            "on",
            rects,
            |ui, v, rects| {
                let r = ui.checkbox(v, "");
                record(ui, rects, "Catalog property Visible", &r);
                r.changed()
            },
        );
        property(
            ui,
            "Opacity",
            "Opacity",
            &mut rows.opacity,
            "1.00",
            rects,
            |ui, v, rects| numeric_input(ui, "property Opacity", v, 0. ..=1., 0.1, "", rects),
        );
        property(
            ui,
            "Line width",
            "Line width",
            &mut rows.width,
            "1.00",
            rects,
            |ui, v, rects| {
                numeric_input(ui, "property Line width", v, 0. ..=10., 0.2, " px", rects)
            },
        );
        property(
            ui,
            "Selection",
            "Selection",
            &mut rows.selection,
            "*",
            rects,
            |ui, v, rects| selection(ui, "property Selection", v, builder, state, revision, rects),
        );
    });
    group(ui, "Inspector property pane (mockup)", |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(generic, false, "Properties");
            ui.selectable_value(generic, true, "Generic");
        });
        let modified = mock.visible.modified || mock.opacity.modified || mock.width.modified;
        let r = ui.add_enabled(modified, egui::Button::new("Reset all to default"));
        record(ui, rects, "Catalog mock reset all", &r);
        if r.clicked() {
            mock.visible.reset();
            mock.opacity.reset();
            mock.width.reset();
        }
        egui::CollapsingHeader::new("Basic settings")
            .default_open(true)
            .show(ui, |ui| {
                property(
                    ui,
                    "Visible",
                    "mock Visible",
                    &mut mock.visible,
                    "on",
                    rects,
                    |ui, v, rects| {
                        let r = ui.checkbox(v, "");
                        record(ui, rects, "Catalog mock Visible", &r);
                        r.changed()
                    },
                );
                property(
                    ui,
                    "Opacity",
                    "mock Opacity",
                    &mut mock.opacity,
                    "1.00",
                    rects,
                    |ui, v, rects| numeric_input(ui, "mock Opacity", v, 0. ..=1., 0.1, "", rects),
                );
            });
        egui::CollapsingHeader::new("Edge lines")
            .default_open(true)
            .show(ui, |ui| {
                property(
                    ui,
                    "Width",
                    "mock Width",
                    &mut mock.width,
                    "1.00",
                    rects,
                    |ui, v, rects| numeric_input(ui, "mock Width", v, 0. ..=10., 0.2, " px", rects),
                );
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        catalog: &mut Catalog,
        state: &DesktopAppState,
        events: Vec<egui::Event>,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320., 900.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    catalog.show(ui, state, 0);
                });
            },
        );
    }
    fn click(ctx: &egui::Context, catalog: &mut Catalog, state: &DesktopAppState, key: &str) {
        let [x, y, w, h] = catalog.rects[key];
        let position = egui::pos2(x + w / 2., y + h / 2.);
        frame(
            ctx,
            catalog,
            state,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            ctx,
            catalog,
            state,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }
    #[test]
    fn property_flags_are_explicit_and_sample_editing_does_not_change_the_scene() {
        let ctx = egui::Context::default();
        let state = DesktopAppState::new();
        let config = state.config_text();
        let mut catalog = Catalog::new();
        for _ in 0..3 {
            frame(&ctx, &mut catalog, &state, vec![]);
        }
        for name in [
            "Catalog 1",
            "Catalog 2",
            "Catalog 3",
            "Catalog property Visible",
        ] {
            assert!(catalog.rects.contains_key(name), "{name}");
        }
        click(&ctx, &mut catalog, &state, "Catalog property Visible");
        assert!(catalog.rows.visible.value);
        assert!(catalog.rows.visible.modified);
        assert!(!catalog.mock.visible.value);
        click(&ctx, &mut catalog, &state, "Catalog reset Visible");
        assert!(catalog.rows.visible.value);
        assert!(!catalog.rows.visible.modified);
        assert!(catalog.mock.visible.modified);
        assert_eq!(catalog.mock.width.value, 2.5);
        assert_eq!(state.config_text(), config);
    }
}
