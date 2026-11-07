use cuemol_command::hatch_editor::{self, Environment, HatchEditor, Values};
use cuemol_render::{HatchRenderParams, HATCH_STYLES};
use egui::{ComboBox, DragValue, Ui};
use serde_json::json;

pub fn hatch_controls(ui: &mut Ui, params: &mut HatchRenderParams) {
    let before = params.style.clone();
    ui.horizontal(|ui| {
        ui.label("Style");
        ComboBox::from_id_salt("hatch-style")
            .selected_text(&params.style)
            .show_ui(ui, |ui| {
                for style in HATCH_STYLES {
                    ui.selectable_value(&mut params.style, style.into(), style);
                }
            });
        if ui.button("Reset to style").clicked() {
            params.layers_spec.clear();
            params.tone_spec.clear();
        }
    });
    if before != params.style {
        params.layers_spec.clear();
        params.tone_spec.clear();
    }
    ui.horizontal(|ui| {
        ui.label("Coloring");
        ComboBox::from_id_salt("hatch-coloring")
            .selected_text(&params.coloring)
            .show_ui(ui, |ui| {
                for choice in [
                    "Style default",
                    "Ink on paper",
                    "Colored ink on paper",
                    "Ink on color fill",
                    "Colored ink on color fill",
                ] {
                    ui.selectable_value(&mut params.coloring, choice.into(), choice);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Mark density");
        ui.add(
            DragValue::new(&mut params.density)
                .speed(0.05)
                .range(0.1..=10.),
        );
        ui.label("Mark width");
        ui.add(
            DragValue::new(&mut params.width_scale)
                .speed(0.05)
                .range(0.1..=8.),
        );
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut params.custom_ink, "Custom ink");
        ui.add_enabled_ui(params.custom_ink, |ui| {
            ui.color_edit_button_srgb(&mut params.ink)
        });
        ui.checkbox(&mut params.custom_paper, "Custom paper");
        ui.add_enabled_ui(params.custom_paper, |ui| {
            ui.color_edit_button_srgb(&mut params.paper)
        });
    });
    ui.checkbox(&mut params.default_edges, "Default contours");
    ui.horizontal(|ui| {
        ui.label("Shading strength");
        ui.add(
            DragValue::new(&mut params.tone_strength)
                .speed(0.05)
                .range(0.0..=8.),
        );
        ui.label("Curve");
        ui.add(
            DragValue::new(&mut params.tone_curve)
                .speed(0.05)
                .range(0.1..=10.),
        );
    });
    hatch_detail_controls(ui, params, true, 3., &mut |_, _| {});
}

fn fields(
    ui: &mut Ui,
    section: &str,
    values: &mut Values,
    env: Environment,
    place: &str,
    prefix: &str,
    record: &mut dyn FnMut(&str, &egui::Response),
) -> bool {
    let mut changed = false;
    for field in HatchEditor::fields(section, values, env) {
        if field["place"] != place {
            continue;
        }
        let key = field["key"].as_str().unwrap();
        let label = field["label"].as_str().unwrap();
        let enabled = field["enabled"].as_bool().unwrap();
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                if field["type"] == "bool" {
                    let mut value = values[key].as_bool().unwrap_or(false);
                    let response = ui.checkbox(&mut value, label);
                    if response.changed() {
                        values.insert(key.into(), json!(value));
                    }
                    response
                } else {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let mut value = values[key].as_f64().unwrap_or(0.);
                        let mut widget = DragValue::new(&mut value)
                            .clamp_existing_to_range(false)
                            .speed(field["step"].as_f64().unwrap_or(0.05))
                            .range(
                                field["min"].as_f64().unwrap_or(0.)
                                    ..=field["max"].as_f64().unwrap_or(1.),
                            );
                        if field["type"] == "int" {
                            widget = widget.max_decimals(0);
                        }
                        if let Some(unit) = field["unit"].as_str() {
                            widget = widget.suffix(format!(" {unit}"));
                        }
                        let response = ui.add(widget);
                        if response.changed() {
                            values.insert(key.into(), json!(value));
                        }
                        response
                    })
                    .inner
                }
            })
            .inner;
        record(&format!("NPR {prefix} {label}"), &response);
        changed |= response.changed();
        if let Some(hint) = field["hint"].as_str() {
            ui.weak(hint);
        }
    }
    changed
}

pub fn hatch_detail_controls(
    ui: &mut Ui,
    params: &mut HatchRenderParams,
    ao_enabled: bool,
    supersample: f64,
    record: &mut dyn FnMut(&str, &egui::Response),
) {
    ui.weak(format!("Style template: {}", params.style));
    if params.density != 1. || params.width_scale != 1. {
        ui.weak(format!(
            "Render tab multipliers: Mark density ×{}, Mark width ×{}",
            params.density, params.width_scale
        ));
    }
    let mut editor = match HatchEditor::load(params) {
        Ok(editor) => editor,
        Err(error) => {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Reset to style").clicked() {
                params.layers_spec.clear();
                params.tone_spec.clear();
            }
            specification(ui, params);
            return;
        }
    };
    let env = Environment::new(params, &editor, ao_enabled, supersample);
    let dirty = editor.dirty();
    let reset = ui
        .horizontal(|ui| {
            let response = ui.add_enabled(dirty, egui::Button::new("Reset to style"));
            if dirty {
                ui.weak("Edited");
            }
            response
        })
        .inner;
    record("NPR Reset to style", &reset);
    if reset.clicked() {
        params.layers_spec.clear();
        params.tone_spec.clear();
        return;
    }
    let mut layer_changed = false;
    let mut tone_changed = false;
    let layers = egui::CollapsingHeader::new("Layers")
        .default_open(true)
        .show(ui, |ui| {
            let mut remove = None;
            let mut duplicate = None;
            let count = editor.layers.len();
            for (i, layer) in editor.layers.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    let kind = layer["kind"].as_str().unwrap_or("line");
                    let response = egui::CollapsingHeader::new(format!("Layer {} · {kind}", i + 1))
                        .id_salt("layer")
                        .default_open(i == 0)
                        .show(ui, |ui| {
                            let response = ComboBox::from_id_salt("kind")
                                .selected_text(layer["kind"].as_str().unwrap_or("line"))
                                .show_ui(ui, |ui| {
                                    for kind in ["line", "dot", "stipple"] {
                                        let response = ui.selectable_value(
                                            layer.get_mut("kind").unwrap(),
                                            json!(kind),
                                            kind,
                                        );
                                        record(
                                            &format!("NPR layer {} kind {kind}", i + 1),
                                            &response,
                                        );
                                        layer_changed |= response.changed();
                                    }
                                })
                                .response;
                            record(&format!("NPR layer {} Kind", i + 1), &response);
                            ui.horizontal(|ui| {
                                let response =
                                    ui.add_enabled(count < 64, egui::Button::new("Duplicate"));
                                record(&format!("NPR layer {} Duplicate", i + 1), &response);
                                if response.clicked() {
                                    duplicate = Some(i);
                                }
                                let response =
                                    ui.add_enabled(count > 1, egui::Button::new("Remove"));
                                record(&format!("NPR layer {} Remove", i + 1), &response);
                                if response.clicked() {
                                    remove = Some(i);
                                }
                            });
                            let prefix = format!("layer {}", i + 1);
                            layer_changed |=
                                fields(ui, "layer", layer, env, "primary", &prefix, record);
                            let advanced = ui.collapsing("Randomness / Advanced", |ui| {
                                layer_changed |=
                                    fields(ui, "layer", layer, env, "advanced", &prefix, record);
                            });
                            record(
                                &format!("NPR layer {} Advanced", i + 1),
                                &advanced.header_response,
                            );
                        });
                    record(&format!("NPR layer {}", i + 1), &response.header_response);
                });
            }
            if let Some(i) = duplicate {
                editor.layers.insert(i + 1, editor.layers[i].clone());
                layer_changed = true;
            }
            if let Some(i) = remove {
                editor.layers.remove(i);
                layer_changed = true;
            }
            ui.horizontal_wrapped(|ui| {
                for kind in ["line", "dot", "stipple"] {
                    let response = ui.add_enabled(
                        editor.layers.len() < 64,
                        egui::Button::new(format!("Add {kind}")),
                    );
                    record(&format!("NPR Add {kind}"), &response);
                    if response.clicked() {
                        editor.layers.push(hatch_editor::new_layer(kind));
                        layer_changed = true;
                    }
                }
            });
        });
    record("NPR Layers", &layers.header_response);
    let shading = egui::CollapsingHeader::new("Shading")
        .default_open(true)
        .show(ui, |ui| {
            tone_changed |= fields(
                ui,
                "tone",
                &mut editor.tone,
                env,
                "primary",
                "shading",
                record,
            );
            let advanced = ui.collapsing("Advanced", |ui| {
                tone_changed |= fields(
                    ui,
                    "tone",
                    &mut editor.tone,
                    env,
                    "advanced",
                    "shading",
                    record,
                );
                tone_changed |= fields(ui, "ink", &mut editor.ink, env, "advanced", "ink", record);
            });
            record("NPR Shading Advanced", &advanced.header_response);
        });
    record("NPR Shading", &shading.header_response);
    if layer_changed || tone_changed {
        let (layers, tone) = editor.specs();
        if layer_changed {
            params.layers_spec = layers;
        }
        if tone_changed {
            params.tone_spec = tone;
        }
    }
    specification(ui, params);
}

fn specification(ui: &mut Ui, params: &mut HatchRenderParams) {
    ui.collapsing("Specification text", |ui| {
        ui.label("Layers");
        ui.text_edit_multiline(&mut params.layers_spec);
        ui.label("Tone and ink");
        ui.text_edit_multiline(&mut params.tone_spec);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showing_hatch_details_preserves_values_outside_control_ranges() {
        let ctx = egui::Context::default();
        let mut params = HatchRenderParams {
            style: "pencil".into(),
            layers_spec: "layer:spacing=128,width=10".into(),
            tone_spec: "tone:strength=4,curve=3".into(),
            ..Default::default()
        };
        let before = (params.layers_spec.clone(), params.tone_spec.clone());
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 1000.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    hatch_detail_controls(ui, &mut params, false, 1., &mut |_, _| {})
                });
            },
        );
        assert_eq!((params.layers_spec, params.tone_spec), before);
    }
}
