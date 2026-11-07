use egui::Ui;

#[derive(Clone)]
struct Examples {
    text: String,
    enabled: bool,
    number: f64,
    choice: usize,
    selected: bool,
    expanded: bool,
    visible: bool,
    color: egui::Color32,
}

impl Default for Examples {
    fn default() -> Self {
        Self {
            text: "Example".into(),
            enabled: true,
            number: 12.,
            choice: 0,
            selected: false,
            expanded: true,
            visible: true,
            color: egui::Color32::LIGHT_BLUE,
        }
    }
}

pub fn show(ui: &mut Ui) {
    let id = ui.make_persistent_id("component-catalog");
    let mut values = ui
        .ctx()
        .data_mut(|data| data.get_temp::<Examples>(id).unwrap_or_default());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (heading, draw) in [("Forms", 0), ("Lists", 1), ("Properties", 2)] {
            egui::CollapsingHeader::new(heading)
                .default_open(true)
                .show(ui, |ui| match draw {
                    0 => {
                        ui.label("Text");
                        ui.text_edit_singleline(&mut values.text);
                        ui.checkbox(&mut values.enabled, "Enabled");
                        ui.add(egui::DragValue::new(&mut values.number).range(0.0..=100.0));
                        egui::ComboBox::from_label("Choice")
                            .selected_text(["First", "Second"][values.choice])
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut values.choice, 0, "First");
                                ui.selectable_value(&mut values.choice, 1, "Second");
                            });
                        ui.horizontal(|ui| {
                            if ui.button("Reset").clicked() {
                                values = Examples::default();
                            }
                            ui.add_enabled(false, egui::Button::new("Disabled"));
                        });
                    }
                    1 => {
                        let response = crate::shell::tree_row(
                            ui,
                            "Molecule",
                            crate::shell::Icon::Atom,
                            values.selected,
                            Some(&mut values.expanded),
                            Some(&mut values.visible),
                        );
                        if response.clicked() {
                            values.selected = !values.selected;
                        }
                        if values.expanded {
                            ui.indent("catalog-child", |ui| {
                                ui.label("Renderer");
                            });
                        }
                        ui.horizontal(|ui| {
                            for label in ["Console", "Output", "Sequence"] {
                                let _ = crate::shell::tab(ui, label, label == "Console", false);
                            }
                        });
                    }
                    _ => {
                        egui::Grid::new("catalog-properties")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label("Name");
                                ui.text_edit_singleline(&mut values.text);
                                ui.end_row();
                                ui.label("Color");
                                ui.color_edit_button_srgba(&mut values.color);
                                ui.end_row();
                                ui.label("Opacity");
                                ui.add(egui::Slider::new(&mut values.number, 0.0..=100.0));
                                ui.end_row();
                                ui.label("Read only");
                                ui.label("Molecule");
                                ui.end_row();
                            });
                    }
                });
        }
    });
    ui.ctx().data_mut(|data| data.insert_temp(id, values));
}
