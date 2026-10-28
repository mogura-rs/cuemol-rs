use cuemol_render::{HatchLayer, HatchLayerKind, HatchLook, HatchRenderParams, HATCH_STYLES};
use egui::{ComboBox, DragValue, Ui};

fn real(ui: &mut Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(DragValue::new(value).speed(0.05).range(range))
            .changed()
    })
    .inner
}
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
    let mut look = HatchLook::style(&params.style).unwrap_or_default();
    if let Err(error) = look
        .apply_spec(&params.layers_spec, 1)
        .and_then(|_| look.apply_spec(&params.tone_spec, 6))
    {
        ui.colored_label(egui::Color32::RED, error.to_string());
    }
    let mut layer_changed = false;
    let mut tone_changed = false;
    ui.collapsing("Layers", |ui| {
        let mut remove = None;
        let mut duplicate = None;
        for (i, layer) in look.layers.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.collapsing(format!("Layer {} · {:?}", i + 1, layer.kind), |ui| {
                    ui.horizontal(|ui| {
                        ComboBox::from_id_salt("kind")
                            .selected_text(format!("{:?}", layer.kind))
                            .show_ui(ui, |ui| {
                                for kind in [
                                    HatchLayerKind::Line,
                                    HatchLayerKind::Dot,
                                    HatchLayerKind::Stipple,
                                ] {
                                    layer_changed |= ui
                                        .selectable_value(
                                            &mut layer.kind,
                                            kind,
                                            format!("{kind:?}"),
                                        )
                                        .changed();
                                }
                            });
                        if ui.small_button("Duplicate").clicked() {
                            duplicate = Some(i);
                        }
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    layer_changed |= real(ui, "Angle", &mut layer.angle, -180. ..=180.);
                    layer_changed |= real(ui, "Spacing", &mut layer.spacing, 0.1..=100.);
                    if layer.kind == HatchLayerKind::Line {
                        layer_changed |= real(ui, "Width", &mut layer.width, 0.1..=8.);
                    } else {
                        layer_changed |= real(ui, "Dot scale", &mut layer.dotscale, 0.1..=8.);
                    }
                    layer_changed |= real(ui, "Light tone", &mut layer.tonehi, 0. ..=1.);
                    layer_changed |= real(ui, "Dark tone", &mut layer.tonelo, 0. ..=1.);
                    layer_changed |= real(ui, "Fade", &mut layer.fade, 0. ..=100.);
                    layer_changed |= real(ui, "Opacity", &mut layer.opacity, 0. ..=1.);
                    layer_changed |= real(ui, "Ink darkness", &mut layer.inkscale, 0. ..=1.);
                    ui.collapsing("Randomness and geometry", |ui| {
                        layer_changed |= real(ui, "Softness", &mut layer.soft, 0.5..=2.);
                        layer_changed |= real(ui, "Jitter", &mut layer.jitter, 0. ..=0.5);
                        layer_changed |= real(ui, "Paper tooth", &mut layer.tooth, 0. ..=1.);
                        layer_changed |= real(ui, "Tooth scale", &mut layer.toothscale, 0.5..=100.);
                        ui.horizontal(|ui| {
                            ui.label("Seed");
                            layer_changed |= ui.add(DragValue::new(&mut layer.seed)).changed();
                        });
                        if layer.kind != HatchLayerKind::Stipple {
                            ui.horizontal(|ui| {
                                ui.label("Subdivisions");
                                layer_changed |= ui
                                    .add(DragValue::new(&mut layer.subdiv).range(0..=10))
                                    .changed();
                            });
                        }
                        if layer.kind == HatchLayerKind::Line {
                            layer_changed |= real(ui, "Wobble", &mut layer.wobble, 0. ..=20.);
                            layer_changed |= real(ui, "Wavelength", &mut layer.wobwave, 2. ..=200.);
                            layer_changed |= real(ui, "Width jitter", &mut layer.wjitter, 0. ..=1.);
                            layer_changed |= real(ui, "Stroke length", &mut layer.slen, 0. ..=500.);
                            ui.add_enabled_ui(layer.slen > 0., |ui| {
                                layer_changed |=
                                    real(ui, "Stroke gap", &mut layer.sgap, 0. ..=100.);
                                layer_changed |= real(ui, "Taper", &mut layer.taper, 0. ..=0.9);
                                layer_changed |=
                                    real(ui, "Angle jitter", &mut layer.anglejitter, 0. ..=15.);
                                layer_changed |=
                                    real(ui, "Length jitter", &mut layer.lenjitter, 0. ..=0.9);
                            });
                        } else {
                            layer_changed |=
                                real(ui, "Shape exponent", &mut layer.shape, 1. ..=64.);
                            layer_changed |= real(ui, "Aspect", &mut layer.aspect, 0.5..=2.);
                            layer_changed |=
                                real(ui, "Dot angle", &mut layer.dotangle, -180. ..=180.);
                            if layer.kind == HatchLayerKind::Dot {
                                layer_changed |=
                                    ui.checkbox(&mut layer.invert, "Merge dark dots").changed();
                            }
                        }
                    });
                });
            });
        }
        if let Some(i) = duplicate {
            look.layers.insert(i + 1, look.layers[i].clone());
            layer_changed = true;
        }
        if let Some(i) = remove {
            if look.layers.len() > 1 {
                look.layers.remove(i);
                layer_changed = true;
            }
        }
        ui.horizontal(|ui| {
            for (label, kind) in [
                ("Add line", HatchLayerKind::Line),
                ("Add dot", HatchLayerKind::Dot),
                ("Add stipple", HatchLayerKind::Stipple),
            ] {
                if ui.button(label).clicked() {
                    look.layers.push(HatchLayer {
                        kind,
                        ..Default::default()
                    });
                    layer_changed = true;
                }
            }
        });
    });
    ui.collapsing("Advanced shading", |ui| {
        let t = &mut look.tone;
        for (label, value) in [
            ("Diffuse", &mut t.diffuse),
            ("Ambient", &mut t.ambient),
            ("Wrap", &mut t.wrap),
            ("Rim darkening", &mut t.rim),
            ("Rim power", &mut t.rimpow),
            ("Rim light bias", &mut t.rimbias),
            ("Contact AO", &mut t.contact),
            ("Shape AO", &mut t.shape),
            ("Black point", &mut t.black),
            ("White point", &mut t.white),
            ("Highlight", &mut t.hl),
            ("Highlight softness", &mut t.hlsoft),
            ("Gamma", &mut t.gamma),
            ("Specular cut", &mut t.speccut),
        ] {
            tone_changed |= real(ui, label, value, 0. ..=8.);
        }
        tone_changed |= real(ui, "Ink shade", &mut look.inkshade, 0. ..=1.);
        tone_changed |= real(ui, "Minimum contrast", &mut look.mincontrast, 0. ..=1.);
        tone_changed |= ui
            .checkbox(&mut look.over, "Overlay shaded color")
            .changed();
        tone_changed |= ui.checkbox(&mut look.tonefog, "Fade tone in fog").changed();
        ui.horizontal(|ui| {
            ui.label("Tone levels");
            tone_changed |= ui
                .add(DragValue::new(&mut look.levels).range(0..=64))
                .changed();
            ui.label("Fill levels");
            tone_changed |= ui
                .add(DragValue::new(&mut look.albedoquant).range(0..=64))
                .changed();
        });
    });
    if layer_changed {
        params.layers_spec = look
            .spec()
            .lines()
            .filter(|line| line.starts_with("layer:"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if tone_changed {
        params.tone_spec = look
            .spec()
            .lines()
            .filter(|line| !line.starts_with("layer:"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    ui.collapsing("Specification text", |ui| {
        ui.label("Layers");
        ui.text_edit_multiline(&mut params.layers_spec);
        ui.label("Tone and ink");
        ui.text_edit_multiline(&mut params.tone_spec);
    });
}
