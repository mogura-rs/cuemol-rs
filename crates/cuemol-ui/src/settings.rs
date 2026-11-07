use cuemol_desktop_core::preferences::{Preferences, FORCE_FIELDS};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    #[default]
    Theme,
    Labels,
    Rendering,
    Mouse,
    Keys,
    Files,
    Updates,
    Advanced,
    Apbs,
    Installed,
    Console,
    Mcp,
    Agent,
}
impl Category {
    pub const ALL: &[Self] = &[
        Self::Theme,
        Self::Labels,
        Self::Rendering,
        Self::Mouse,
        Self::Keys,
        Self::Files,
        Self::Updates,
        Self::Advanced,
        Self::Apbs,
        Self::Installed,
        Self::Console,
        Self::Mcp,
        Self::Agent,
    ];
    pub fn group(self) -> &'static str {
        match self {
            Self::Theme | Self::Labels | Self::Rendering => "Display",
            Self::Mouse | Self::Keys => "Input",
            Self::Files | Self::Updates | Self::Advanced => "General",
            Self::Apbs => "Tools",
            _ => "Plugins",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::Labels => "Atom Labels",
            Self::Rendering => "Rendering",
            Self::Mouse => "Mouse & Navigation",
            Self::Keys => "Keyboard",
            Self::Files => "Files",
            Self::Updates => "Updates",
            Self::Advanced => "Configuration & Startup",
            Self::Apbs => "APBS / PDB2PQR",
            Self::Installed => "Installed",
            Self::Console => "Console",
            Self::Mcp => "MCP Server",
            Self::Agent => "AI Agent",
        }
    }
    pub fn keywords(self) -> &'static str {
        match self {
            Self::Theme => "dark light theme hidpi retina language",
            Self::Labels => "atom label font size color bold italic",
            Self::Rendering => "rendering povray pov-ray include ffmpeg blendpng movie",
            Self::Mouse => "mouse navigation pointing device trackpad rotation sensitivity pick precision gpu hover momentum",
            Self::Keys => "key keyboard shortcut ctrl alt shift",
            Self::Files => "files drop drag file manager current new scene",
            Self::Updates => "updates startup version release install check",
            Self::Advanced => "advanced configuration startup javascript script",
            Self::Apbs => "tools apbs pdb2pqr force field executable electrostatic",
            Self::Installed => "plugins installed catalog agent md trajectory console pymol chimerax mcp",
            Self::Console => "console command line access terminal",
            Self::Mcp => "mcp server accept connections port",
            Self::Agent => "agent model reasoning api key provider enter",
        }
    }
    pub fn available(self, preferences: &Preferences, browser: bool) -> bool {
        match self {
            Self::Mcp => !browser && preferences.plugins.get("mcp") == Some(&true),
            Self::Console => !browser && preferences.plugins.get("console") == Some(&true),
            Self::Agent => preferences.plugins.get("agent") == Some(&true),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPath {
    Povray,
    PovrayInclude,
    Ffmpeg,
    Apbs,
    Pdb2pqr,
}
impl ToolPath {
    pub fn value_mut(self, preferences: &mut Preferences) -> &mut String {
        match self {
            Self::Povray => &mut preferences.tools.povray,
            Self::PovrayInclude => &mut preferences.tools.povray_include,
            Self::Ffmpeg => &mut preferences.tools.ffmpeg,
            Self::Apbs => &mut preferences.tools.apbs,
            Self::Pdb2pqr => &mut preferences.tools.pdb2pqr,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Povray => "POV-Ray Executable",
            Self::PovrayInclude => "POV-Ray Include Directory",
            Self::Ffmpeg => "ffmpeg Executable",
            Self::Apbs => "APBS Executable",
            Self::Pdb2pqr => "pdb2pqr Executable",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Browse(ToolPath),
    CheckUpdates,
}

#[derive(Debug, Default)]
pub struct SettingsPanel {
    pub category: Category,
    pub search: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    font_listener: bool,
}
impl SettingsPanel {
    fn record(&mut self, label: &str, response: &egui::Response) {
        let r = response.rect;
        self.rects
            .insert(label.into(), [r.left(), r.top(), r.width(), r.height()]);
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        preferences: &mut Preferences,
        browser: bool,
        mut extra: impl FnMut(Category, &mut egui::Ui),
    ) -> Vec<Action> {
        self.rects.clear();
        if !self.category.available(preferences, browser) {
            self.category = Category::Installed;
        }
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search settings")
                .desired_width(f32::INFINITY),
        );
        self.record("Search settings", &search);
        ui.separator();
        let query = self.search.trim().to_lowercase();
        let categories = Category::ALL
            .iter()
            .copied()
            .filter(|c| {
                c.available(preferences, browser)
                    && (query.is_empty()
                        || c.keywords().contains(&query)
                        || c.group().to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>();
        let height = ui.available_height().max(180.);
        let mut actions = Vec::new();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(180., height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(180.);
                    egui::ScrollArea::vertical()
                        .id_salt("settings-navigation")
                        .show(ui, |ui| {
                            for group in ["Display", "Input", "General", "Tools", "Plugins"] {
                                let children = categories
                                    .iter()
                                    .copied()
                                    .filter(|c| c.group() == group)
                                    .collect::<Vec<_>>();
                                if children.is_empty() {
                                    continue;
                                }
                                let response = egui::CollapsingHeader::new(group)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for category in children {
                                            let response = ui.selectable_label(
                                                self.category == category,
                                                category.label(),
                                            );
                                            self.record(
                                                &format!("Settings {}", category.label()),
                                                &response,
                                            );
                                            if response.clicked() {
                                                self.category = category;
                                                self.search.clear();
                                            }
                                        }
                                    });
                                self.record(
                                    &format!("Settings group {group}"),
                                    &response.header_response,
                                );
                            }
                        });
                },
            );
            ui.separator();
            let width = ui.available_width().max(180.);
            ui.allocate_ui_with_layout(
                egui::vec2(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    egui::ScrollArea::vertical()
                        .id_salt("settings-content")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if categories.is_empty() {
                                ui.label("No settings match this search.");
                            }
                            for category in categories
                                .into_iter()
                                .filter(|c| !query.is_empty() || *c == self.category)
                                .collect::<Vec<_>>()
                            {
                                ui.heading(category.label());
                                ui.add_space(6.);
                                self.section(ui, preferences, category, browser, &mut actions);
                                extra(category, ui);
                                ui.add_space(12.);
                            }
                        });
                },
            );
        });
        actions
    }
    fn checkbox(&mut self, ui: &mut egui::Ui, value: &mut bool, label: &str) {
        let response = ui.checkbox(value, label);
        self.record(label, &response);
    }
    fn text(&mut self, ui: &mut egui::Ui, value: &mut String, label: &str) {
        ui.label(label);
        let response = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
        self.record(label, &response);
    }
    fn number(
        &mut self,
        ui: &mut egui::Ui,
        value: &mut f64,
        label: &str,
        range: std::ops::RangeInclusive<f64>,
        speed: f64,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            let response = ui.add(egui::DragValue::new(value).range(range).speed(speed));
            self.record(label, &response);
        });
    }
    fn choice(
        &mut self,
        ui: &mut egui::Ui,
        value: &mut String,
        label: &str,
        choices: &[(&str, &str)],
    ) {
        let response = egui::ComboBox::from_label(label)
            .selected_text(
                choices
                    .iter()
                    .find(|(key, _)| *key == value.as_str())
                    .map_or(value.as_str(), |(_, name)| *name),
            )
            .show_ui(ui, |ui| {
                for (key, name) in choices {
                    let response = ui.selectable_value(value, (*key).into(), *name);
                    self.record(&format!("{label} {name}"), &response);
                }
            });
        self.record(label, &response.response);
    }
    fn paths(
        &mut self,
        ui: &mut egui::Ui,
        p: &mut Preferences,
        paths: &[ToolPath],
        actions: &mut Vec<Action>,
    ) {
        ui.weak("Leave a path empty to use the bundled tool or search PATH.");
        for path in paths {
            self.text(ui, path.value_mut(p), path.label());
            if ui.button(format!("Browse {}…", path.label())).clicked() {
                actions.push(Action::Browse(*path));
            }
            ui.add_space(6.);
        }
    }
    fn section(
        &mut self,
        ui: &mut egui::Ui,
        p: &mut Preferences,
        c: Category,
        browser: bool,
        actions: &mut Vec<Action>,
    ) {
        match c {
            Category::Theme => {
                self.checkbox(ui, &mut p.dark_mode, "Dark Mode");
                if !browser {
                    self.checkbox(ui, &mut p.use_hidpi, "Enable HiDPI (Retina) display");
                    self.choice(
                        ui,
                        &mut p.ui_language,
                        "UI language",
                        &[("en-US", "English (en-US)"), ("ja", "Japanese (ja)")],
                    );
                }
            }
            Category::Labels => {
                if !self.font_listener {
                    let ctx = ui.ctx().clone();
                    cuemol_desktop_core::font_catalog::on_change(move || ctx.request_repaint());
                    self.font_listener = true;
                }
                cuemol_desktop_core::font_catalog::request();
                let response = egui::ComboBox::from_label("Atom Label Font")
                    .selected_text(&p.atom_label_font)
                    .show_ui(ui, |ui| {
                        for name in cuemol_desktop_core::font_catalog::families() {
                            ui.selectable_value(&mut p.atom_label_font, name.clone(), name);
                        }
                    });
                self.record("Atom Label Font", &response.response);
                self.text(ui, &mut p.atom_label_font, "Custom font family");
                self.number(
                    ui,
                    &mut p.atom_label_size,
                    "Atom Label Size",
                    6.0..=72.0,
                    1.0,
                );
                self.text(ui, &mut p.atom_label_color, "Atom Label Color");
                if let Ok(color) = cuemol_gfx::parse_color(&p.atom_label_color) {
                    if let Some(color) = color.rgba8() {
                        let mut color = egui::Color32::from_rgba_unmultiplied(
                            color.r, color.g, color.b, color.a,
                        );
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            p.atom_label_color =
                                format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
                        }
                    }
                }
                self.checkbox(ui, &mut p.atom_label_bold, "Atom Label Bold");
                self.checkbox(ui, &mut p.atom_label_italic, "Atom Label Italic");
                let mut sample = egui::RichText::new("ABC abc 123")
                    .size(p.atom_label_size.clamp(6., 72.) as f32);
                if p.atom_label_bold {
                    sample = sample.strong();
                }
                if p.atom_label_italic {
                    sample = sample.italics();
                }
                if let Ok(color) = cuemol_gfx::parse_color(&p.atom_label_color) {
                    if let Some(color) = color.rgba8() {
                        sample = sample.color(egui::Color32::from_rgba_unmultiplied(
                            color.r, color.g, color.b, color.a,
                        ));
                    }
                }
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(sample);
                });
            }
            Category::Rendering => {
                if browser {
                    ui.label("Use the Render panel for browser GI and NPR rendering. External executables run in the native application.");
                } else {
                    self.paths(
                        ui,
                        p,
                        &[ToolPath::Povray, ToolPath::PovrayInclude, ToolPath::Ffmpeg],
                        actions,
                    );
                }
                ui.weak("Render layer composition is built in.");
            }
            Category::Mouse => {
                self.choice(
                    ui,
                    &mut p.input_device,
                    "Pointing device",
                    &[
                        ("auto", "Auto-detect"),
                        ("mouse", "Mouse"),
                        ("trackpad", "Mac trackpad"),
                    ],
                );
                p.multitouch_pad = p.input_device == "trackpad";
                self.choice(
                    ui,
                    &mut p.mouse_preset,
                    "View operation preset",
                    &[
                        ("DefaultViewInConf", "Default"),
                        ("CootViewInConf", "Coot"),
                        ("PymolViewInConf", "PyMOL"),
                        ("IphoneViewInConf", "iPhone"),
                        ("TrackpadViewInConf", "Trackpad"),
                    ],
                );
                self.number(
                    ui,
                    &mut p.trackball_radius,
                    "XY Rotation Sensitivity",
                    0.1..=5.,
                    0.1,
                );
                self.number(ui, &mut p.hit_precision, "Pick Precision", 1.0..=50., 1.0);
                self.checkbox(ui, &mut p.gpu_picking, "GPU picking");
                self.checkbox(ui, &mut p.hover_info, "Hover info");
                self.checkbox(ui, &mut p.hover_highlight, "Hover highlight");
                self.checkbox(ui, &mut p.momentum_scroll, "Momentum scroll");
                self.checkbox(
                    ui,
                    &mut p.emulate_right_button,
                    "Emulate right mouse button",
                );
            }
            Category::Keys => {
                egui::Grid::new("settings-shortcuts").show(ui, |ui| {
                    for heading in ["Name", "Key", "Ctrl", "Alt", "Shift"] {
                        ui.strong(heading);
                    }
                    ui.end_row();
                    for shortcut in &mut p.shortcuts {
                        ui.label(if shortcut.id == "view-trans-plusx" {
                            "Translate view +X"
                        } else {
                            "Translate view −X"
                        });
                        egui::ComboBox::from_id_salt(&shortcut.id)
                            .selected_text(&shortcut.key)
                            .show_ui(ui, |ui| {
                                for key in ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"] {
                                    ui.selectable_value(&mut shortcut.key, key.into(), key);
                                }
                            });
                        ui.checkbox(&mut shortcut.ctrl, "");
                        ui.checkbox(&mut shortcut.alt, "");
                        ui.checkbox(&mut shortcut.shift, "");
                        ui.end_row();
                    }
                });
            }
            Category::Files => {
                for (label, target) in [
                    ("Drag and drop opens into", &mut p.drop_target),
                    ("File manager opens into", &mut p.shell_target),
                ] {
                    if browser && label == "File manager opens into" {
                        continue;
                    }
                    let response = egui::ComboBox::from_label(label)
                        .selected_text(if *target == cuemol_desktop_core::FileOpenTarget::New {
                            "New scene"
                        } else {
                            "Current scene"
                        })
                        .show_ui(ui, |ui| {
                            for (value, name) in [
                                (cuemol_desktop_core::FileOpenTarget::Active, "Current scene"),
                                (cuemol_desktop_core::FileOpenTarget::New, "New scene"),
                            ] {
                                let response = ui.selectable_value(target, value, name);
                                self.record(&format!("{label} {name}"), &response);
                                if label == "Drag and drop opens into" {
                                    self.record(&format!("Drop target {name}"), &response);
                                }
                            }
                        });
                    self.record(label, &response.response);
                    ui.add_space(8.);
                }
                ui.weak("Files opened together share one destination. Scene files keep their own scene. File > Open uses the current scene.");
            }
            Category::Updates => {
                if !browser {
                    self.checkbox(ui, &mut p.update_check, "Check for Updates at Startup");
                }
                if ui
                    .button(if browser {
                        "Reload viewer"
                    } else {
                        "Check for Updates"
                    })
                    .clicked()
                {
                    actions.push(Action::CheckUpdates);
                }
            }
            Category::Advanced => {}
            Category::Apbs => {
                if browser {
                    ui.label("APBS and pdb2pqr run in the native application.");
                } else {
                    self.paths(ui, p, &[ToolPath::Apbs, ToolPath::Pdb2pqr], actions);
                }
                self.choice(
                    ui,
                    &mut p.tools.force_field,
                    "pdb2pqr Force Field",
                    &FORCE_FIELDS.iter().map(|s| (*s, *s)).collect::<Vec<_>>(),
                );
            }
            Category::Installed => {
                for plugin in cuemol_desktop_core::plugins::BUILTIN_PLUGINS
                    .iter()
                    .filter(|p| !p.always_enabled && (!browser || p.id != "mcp"))
                {
                    let value = p
                        .plugins
                        .entry(plugin.id.into())
                        .or_insert(plugin.default_enabled);
                    let response = ui.checkbox(value, plugin.name);
                    self.record(&format!("Plugin {}", plugin.id), &response);
                }
            }
            Category::Console => {
                self.checkbox(ui, &mut p.console_remote, "Command line access");
                ui.label("Allow cuemol commands to use this window's scene through its authenticated local server.");
                ui.code("cuemol console --connect");
            }
            Category::Mcp => {
                self.checkbox(ui, &mut p.mcp_accept_connections, "Accept connections");
                ui.horizontal(|ui| {
                    ui.label("Port (0 = automatic)");
                    ui.add(egui::DragValue::new(&mut p.mcp_port));
                });
                ui.code("cuemol mcp connect");
            }
            Category::Agent => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_keep_navigation_and_content_in_bounded_columns() {
        let ctx = egui::Context::default();
        let mut panel = SettingsPanel::default();
        let mut preferences = Preferences::default();
        let frame = |panel: &mut SettingsPanel, preferences: &mut Preferences| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 580.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        panel.show(ui, preferences, false, |_, _| {});
                    });
                },
            );
        };
        frame(&mut panel, &mut preferences);
        frame(&mut panel, &mut preferences);
        let theme = panel.rects["Settings Theme"];
        let installed = panel.rects["Settings Installed"];
        assert!((theme[0] - installed[0]).abs() < 1.);
        assert!(installed[1] > theme[1] + 100.);
        assert!(panel.rects["Dark Mode"][0] > theme[0] + 150.);
        for rect in panel.rects.values() {
            assert!(rect[0] + rect[2] <= 800., "{rect:?}");
        }
        assert!(!panel.rects.contains_key("Settings AI Agent"));
        preferences.plugins.insert("agent".into(), true);
        frame(&mut panel, &mut preferences);
        assert!(panel.rects.contains_key("Settings AI Agent"));
        panel.category = Category::Agent;
        preferences.plugins.insert("agent".into(), false);
        frame(&mut panel, &mut preferences);
        assert_eq!(panel.category, Category::Installed);
        panel.search = "pick".into();
        frame(&mut panel, &mut preferences);
        assert!(panel.rects.contains_key("GPU picking"));
        assert!(!panel.rects.contains_key("Dark Mode"));
    }
}
