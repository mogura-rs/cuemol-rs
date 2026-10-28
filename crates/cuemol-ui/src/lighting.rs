use cuemol_render::UmbreonRenderParams;

pub fn lighting_controls(ui: &mut egui::Ui, p: &mut UmbreonRenderParams, npr: bool) {
    ui.collapsing("Lighting and sampling", |ui| {
        ui.horizontal(|ui| {
            ui.label("Antialiasing");
            ui.add(egui::DragValue::new(&mut p.supersample).range(1..=8));
        });
        if !npr {
            ui.checkbox(&mut p.use_gi, "Global illumination");
        }
        let mut intensity = if p.light_intensity < 0. {
            1.3
        } else {
            p.light_intensity
        };
        if ui
            .add(egui::Slider::new(&mut intensity, 0.0..=5.0).text("Light intensity"))
            .changed()
        {
            p.light_intensity = intensity;
        }
        ui.add(egui::Slider::new(&mut p.flash_fraction, 0.0..=1.0).text("Headlight fraction"));
        ui.add(egui::Slider::new(&mut p.ambient_fraction, 0.0..=1.0).text("Ambient fraction"));
        ui.checkbox(&mut p.shadows, "Cast shadows");
        if p.shadows {
            ui.horizontal(|ui| {
                ui.label("Shadow samples");
                ui.add(egui::DragValue::new(&mut p.shadow_samples).range(1..=256));
            });
            ui.add(
                egui::Slider::new(&mut p.light_radius, 0.0..=45.0).text("Light radius (degrees)"),
            );
        }
        if p.use_gi && !npr {
            ui.horizontal(|ui| {
                ui.label("GI samples");
                ui.add(egui::DragValue::new(&mut p.gi_samples).range(1..=256));
                ui.label("Bounces");
                ui.add(egui::DragValue::new(&mut p.gi_bounces).range(1..=8));
            });
            ui.add(egui::Slider::new(&mut p.gi_intensity, 0.0..=3.0).text("Indirect intensity"));
            ui.add(
                egui::Slider::new(&mut p.gi_env_intensity, 0.0..=3.0).text("Environment intensity"),
            );
            ui.checkbox(&mut p.gi_sky_gradient, "Sky and ground gradient");
            if p.gi_sky_gradient {
                ui.horizontal(|ui| {
                    ui.label("Ground color");
                    let mut color = p.gi_ground_color.map(|v| v as f32);
                    if ui.color_edit_button_rgb(&mut color).changed() {
                        p.gi_ground_color = color.map(f64::from);
                    }
                });
            }
        }
        let mut ao = p.ao_samples > 0;
        if ui.checkbox(&mut ao, "Ambient occlusion").changed() {
            p.ao_samples = if ao { 64 } else { 0 };
        }
        if ao {
            ui.horizontal(|ui| {
                ui.label("AO samples");
                ui.add(egui::DragValue::new(&mut p.ao_samples).range(1..=256));
            });
            ui.horizontal(|ui| {
                ui.label("AO distance (0 = automatic)");
                ui.add(
                    egui::DragValue::new(&mut p.ao_distance)
                        .range(0.0..=1e6)
                        .speed(0.1),
                );
            });
            ui.add(egui::Slider::new(&mut p.ao_intensity, 0.0..=1.0).text("AO intensity"));
            ui.add(
                egui::Slider::new(&mut p.ao_diffuse_factor, 0.0..=1.0)
                    .text("Direct diffuse occlusion"),
            );
            ui.add(egui::Slider::new(&mut p.ao_falloff_power, 0.0..=4.0).text("Distance falloff"));
            ui.checkbox(&mut p.ao_multi_scale, "Contact and shape scales");
            ui.checkbox(&mut p.ao_bent_normal, "Directional ambient");
            ui.checkbox(&mut p.ao_low_discrepancy, "Low discrepancy sampling");
            ui.checkbox(&mut p.ao_per_pixel, "Gather once per output pixel");
        }
        let mut denoise = if p.denoiser > 0 {
            "A-trous"
        } else if p.gi_denoise {
            "Bilateral"
        } else {
            "None"
        };
        egui::ComboBox::from_label("Denoising")
            .selected_text(denoise)
            .show_ui(ui, |ui| {
                for value in ["Bilateral", "A-trous", "None"] {
                    ui.selectable_value(&mut denoise, value, value);
                }
            });
        p.gi_denoise = denoise == "Bilateral";
        p.denoiser = i32::from(denoise == "A-trous");
    });
    ui.collapsing("Contours and transparency", |ui| {
        ui.checkbox(&mut p.show_edge_lines, "Draw edge lines");
        if p.show_edge_lines {
            ui.checkbox(&mut p.contact_edges, "Contact contours between groups");
            let mut crease = p.crease_limit > 0.;
            if ui.checkbox(&mut crease, "Crease lines").changed() {
                p.crease_limit = if crease { 45. } else { -1. };
            }
            if crease {
                ui.add(egui::Slider::new(&mut p.crease_limit, 1.0..=179.0).text("Crease angle"));
            }
            ui.add(
                egui::Slider::new(&mut p.outline_far_depth, 0.0..=1.0).text("Far silhouette depth"),
            );
        }
        ui.checkbox(
            &mut p.per_pixel_blend,
            "Blend overlapping transparent sections per pixel",
        );
    });
}
