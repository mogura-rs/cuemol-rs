use crate::{inspector::numeric_change, numeric::NumericEdit, InspectorTarget};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::collections::BTreeMap;

type Changes = Vec<(&'static str, Value)>;

#[derive(Debug, Default)]
pub struct DensityPanel {
    pub rects: BTreeMap<String, [f32; 4]>,
    pub target: Option<(Uid, Uid)>,
    context: Option<(Uid, Uid, Uid)>,
    edits: BTreeMap<String, NumericEdit>,
    color: String,
    committed_color: String,
    color_picker: Option<egui::Color32>,
}

impl DensityPanel {
    pub fn cancel(&mut self) -> Changes {
        let mut changes = Vec::new();
        let Some((scene, _, renderer)) = self.context else {
            return changes;
        };
        let target = InspectorTarget {
            kind: "renderer".into(),
            id: Some(renderer.get()),
        };
        for (key, edit) in &mut self.edits {
            let Some(drag) = edit.drag.take() else {
                continue;
            };
            if drag.realtime {
                changes.push(numeric_change(
                    &target,
                    Some(scene.get()),
                    key,
                    "abort",
                    &drag,
                    &drag.original,
                ));
            }
            edit.value = drag.original.parse().ok();
            edit.dirty = false;
            edit.cancelled = true;
            edit.action = None;
        }
        changes
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        camera: &Camera,
        preferred: Option<(Uid, Uid)>,
    ) -> Changes {
        self.rects.clear();
        if !ui.input(|input| input.pointer.primary_down()) {
            for edit in self.edits.values_mut() {
                edit.cancelled = false;
            }
        }
        let mut changes =
            if ui.input(|input| input.key_pressed(egui::Key::Escape) || !input.focused) {
                self.cancel()
            } else {
                Vec::new()
            };
        let maps = state.active_density_map_summaries();
        let targets = maps
            .iter()
            .flat_map(|map| {
                map.renderers
                    .iter()
                    .filter(|renderer| {
                        matches!(
                            renderer.renderer_type.as_str(),
                            "contour" | "isosurf" | "gpu_mapmesh" | "gpu_mapvol"
                        )
                    })
                    .map(move |renderer| {
                        (
                            (map.uid, renderer.uid),
                            format!("{}/{}", map.name, renderer.name),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let valid = |target: &(Uid, Uid)| targets.iter().any(|(candidate, _)| candidate == target);
        let mut selected = self
            .target
            .filter(valid)
            .or(preferred.filter(valid))
            .or_else(|| targets.first().map(|(target, _)| *target));
        let selected_name = targets
            .iter()
            .find(|(target, _)| Some(*target) == selected)
            .map(|(_, name)| name.as_str())
            .unwrap_or("(none)");
        ui.add_enabled_ui(!targets.is_empty(), |ui| {
            let response = egui::ComboBox::from_id_salt("density-renderer")
                .width((ui.available_width() - 8.).max(40.))
                .selected_text(selected_name)
                .show_ui(ui, |ui| {
                    for (target, name) in &targets {
                        let response = ui.selectable_value(&mut selected, Some(*target), name);
                        record(
                            &mut self.rects,
                            &format!("Density target {name}"),
                            response.rect,
                        );
                    }
                })
                .response;
            record(&mut self.rects, "Density renderer", response.rect);
        });
        self.target = selected;
        let context = state
            .active_scene()
            .zip(selected)
            .map(|(scene, (map, renderer))| (scene, map, renderer));
        if self.context != context {
            changes.extend(self.cancel());
            self.edits.clear();
            self.color.clear();
            self.committed_color.clear();
            self.color_picker = None;
            self.context = context;
        }
        let Some((scene, owner, renderer)) = self.context else {
            ui.weak(if maps.is_empty() {
                "No density map loaded."
            } else {
                "No density renderer."
            });
            return changes;
        };
        let values = match state.auxiliary_renderer_properties(owner, renderer) {
            Ok(values) => values,
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                return changes;
            }
        };
        let explicit = state
            .auxiliary_renderer_explicit_properties(owner, renderer)
            .unwrap_or_default();
        let map = maps.iter().find(|map| map.uid == owner).unwrap();
        let target = InspectorTarget {
            kind: "renderer".into(),
            id: Some(renderer.get()),
        };
        let write = |prop: &str, value: String| {
            (
                "set_node_prop",
                json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":prop,"value":value}),
            )
        };
        let absolute = values
            .get("use_abslevel")
            .is_some_and(|value| value == "true");
        let percent = values
            .get("map_type_resolved")
            .is_some_and(|value| value == "em");
        let solid = values.get("colormode").is_none_or(|value| value == "solid");
        let menu = ui.menu_button("Options", |ui| {
            for (label, selected, prop, value) in [
                (
                    if percent {
                        "Use top-percent contour level"
                    } else {
                        "Use sigma contour level"
                    },
                    !absolute,
                    "use_abslevel",
                    "false",
                ),
                (
                    "Use absolute contour level",
                    absolute,
                    "use_abslevel",
                    "true",
                ),
                ("Solid color", solid, "colormode", "solid"),
            ] {
                let response = ui.selectable_label(selected, label);
                record(&mut self.rects, label, response.rect);
                if response.clicked() {
                    if !selected {
                        changes.push(write(prop, value.into()));
                    }
                    ui.close();
                }
            }
        });
        record(&mut self.rects, "Density options", menu.response.rect);
        ui.horizontal(|ui| {
            let width = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(35.);
            let redraw = ui.add_sized([width, 24.], egui::Button::new("Redraw"));
            record(&mut self.rects, "Density redraw", redraw.rect);
            if redraw.clicked() {
                changes.push(write("center", camera.center().script_string()));
            }
            let has_cell = map.renderers.iter().any(|renderer| {
                matches!(renderer.renderer_type.as_str(), "*unitcell" | "unitcell")
            });
            let cell = ui
                .add_enabled_ui(!has_cell, |ui| {
                    ui.add_sized([width, 24.], egui::Button::new("Cell"))
                })
                .inner;
            record(&mut self.rects, "Density cell", cell.rect);
            if cell.clicked() {
                changes.push((
                    "create_renderer",
                    json!({"objId":owner.get(),"rendererType":"*unitcell","name":"unitcell"}),
                ));
            }
        });
        let color = values.get("color").map(String::as_str).unwrap_or("#00ff00");
        if self.committed_color != color {
            if self.color == self.committed_color {
                self.color = color.into();
            }
            self.committed_color = color.into();
        }
        ui.add_enabled_ui(solid, |ui| {
            ui.horizontal(|ui| {
                if let Some(rgba) = state
                    .active_style_colors([color])
                    .first()
                    .copied()
                    .flatten()
                {
                    let rgba =
                        egui::Color32::from_rgba_unmultiplied(rgba.r, rgba.g, rgba.b, rgba.a);
                    let button = ui.add(
                        egui::Button::new(" ")
                            .fill(rgba)
                            .min_size(egui::vec2(24., 24.)),
                    );
                    record(&mut self.rects, "Density color picker", button.rect);
                    if button.clicked() {
                        self.color_picker = Some(rgba);
                    }
                }
                let text = ui.add_sized(
                    [ui.available_width().max(24.), 24.],
                    egui::TextEdit::singleline(&mut self.color),
                );
                record(&mut self.rects, "Density color", text.rect);
                if (text.lost_focus()
                    || text.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    && self.color != color
                {
                    changes.push(write("color", self.color.clone()));
                }
            });
        });
        let scalar = |key: &str, fallback: f64| {
            values
                .get(key)
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .unwrap_or(fallback)
        };
        let alpha = scalar("alpha", 1.);
        self.number(
            ui,
            &target,
            scene,
            "alpha",
            "Transp",
            "Density opacity",
            alpha,
            !explicit.contains("alpha"),
            json!({"min":0.,"max":1.,"step":0.1,"realtime":true}),
            true,
            &mut changes,
        );
        let level = scalar(
            if absolute {
                "level"
            } else {
                "siglevel_effective"
            },
            1.1,
        );
        let minimum = scalar(
            if absolute {
                "ui_abslevel_min"
            } else {
                "ui_level_min"
            },
            if percent { 0. } else { -10. },
        );
        let maximum = scalar(
            if absolute {
                "ui_abslevel_max"
            } else {
                "ui_level_max"
            },
            10.,
        );
        let enabled = maximum > minimum;
        let step = if absolute && enabled {
            10_f64.powf(((maximum - minimum) / 100.).log10().floor())
        } else if percent {
            0.01
        } else {
            0.1
        };
        self.number(ui, &target, scene, if absolute { "level" } else { "siglevel" }, "Level", "Density level", level, !explicit.contains("siglevel"), json!({"min":minimum,"max":maximum.max(minimum),"step":step,"unit":if absolute {""} else if percent {"%"} else {"σ"}}), enabled, &mut changes);
        ui.weak(if absolute {
            if percent {
                format!(
                    "= top {:.2} % of grid points",
                    scalar("siglevel_effective", 0.)
                )
            } else {
                format!("= {:.2} σ", scalar("siglevel_effective", 0.))
            }
        } else {
            format!("= {:.3} (absolute)", scalar("level", 0.))
        });
        let extent = scalar("extent", 15.);
        self.number(ui, &target, scene, "extent", "Extent", "Density extent", extent, !explicit.contains("extent"), json!({"min":0.,"max":scalar("maxExtent", map.radius.max(1.)).max(0.),"step":1.,"unit":"Å"}), values.get("region_mode_resolved").is_none_or(|value| value != "full"), &mut changes);
        if let Some(rgba) = &mut self.color_picker {
            let mut open = true;
            let mut apply = false;
            let mut cancel = false;
            egui::Window::new("Density color")
                .id(egui::Id::new("density-color-picker"))
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.color_edit_button_srgba(rgba);
                    ui.horizontal(|ui| {
                        apply = ui.button("Apply").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if apply {
                let [r, g, b, a] = rgba.to_srgba_unmultiplied();
                changes.push(write(
                    "color",
                    format!("rgba({r},{g},{b},{})", f32::from(a) / 255.),
                ));
            }
            if !open || apply || cancel {
                self.color_picker = None;
            }
        }
        changes
    }

    #[allow(clippy::too_many_arguments)]
    fn number(
        &mut self,
        ui: &mut egui::Ui,
        target: &InspectorTarget,
        scene: Uid,
        key: &str,
        label: &str,
        rect_name: &str,
        value: f64,
        is_default: bool,
        schema: Value,
        enabled: bool,
        changes: &mut Changes,
    ) {
        let original = value.to_string();
        let edit = self.edits.entry(key.into()).or_default();
        let mut displayed = if edit.drag.is_some() || edit.cancelled {
            edit.value.unwrap_or(value).to_string()
        } else {
            original.clone()
        };
        ui.horizontal(|ui| {
            ui.add_sized([52., 24.], egui::Label::new(label));
            let (response, changed) = ui
                .add_enabled_ui(enabled, |ui| {
                    crate::numeric::field(
                        ui,
                        key,
                        "real",
                        &mut displayed,
                        Some(&schema),
                        &original,
                        is_default,
                        edit,
                        &mut self.rects,
                    )
                })
                .inner;
            record(&mut self.rects, rect_name, response.rect);
            if let Some((mode, drag, value)) = edit.action.take() {
                changes.push(numeric_change(
                    target,
                    Some(scene.get()),
                    key,
                    mode,
                    &drag,
                    &value,
                ));
            }
            if changed && displayed.parse::<f64>().ok() != Some(value) {
                changes.push((
                    "set_node_prop",
                    json!({"nodeType":target.kind,"nodeId":target.id,"prop":key,"value":displayed}),
                ));
            }
        });
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, rect: egui::Rect) {
    rects.insert(
        name.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}
