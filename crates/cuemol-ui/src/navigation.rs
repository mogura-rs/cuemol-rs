use crate::shell::{self, Icon};
use cuemol_command::navigation::{self, AtomHit, Command, Navigation, Tool};
use cuemol_desktop_core::DesktopAppState;
use egui::{Color32, Pos2, Rect, Stroke, Ui};
use serde_json::json;

pub enum ContextAction {
    Center,
    Command(Command),
}

pub fn context_menu(
    ui: &mut Ui,
    state: &DesktopAppState,
    hit: AtomHit,
) -> (Option<ContextAction>, Vec<(String, Rect)>) {
    let mut action = None;
    let mut rects = Vec::new();
    if let Ok(atom) = hit.atom(state) {
        ui.label(atom.format_msg());
    }
    if let Some(object) = state.scene_manager().object(hit.object) {
        if let Some(r) = object
            .renderers()
            .iter()
            .find(|r| r.is_visible() && !r.renderer_type().starts_with('*'))
        {
            ui.label(format!(
                "{}: {} ({})",
                object.name(),
                r.name(),
                r.renderer_type()
            ));
        }
    }
    ui.separator();
    let mut button = |ui: &mut Ui, label: &str| {
        let response = ui.button(label);
        rects.push((label.to_string(), response.rect));
        response.clicked()
    };
    if button(ui, "Center at this atom") {
        action = Some(ContextAction::Center);
    }
    ui.separator();
    let current = state.molecule_selection_text(hit.object).unwrap_or("");
    for add in [false, true] {
        for level in ["Atom", "Residue", "Chain", "Molecule"] {
            if add && level == "Molecule" {
                continue;
            }
            let label = format!("{}Select {level}", if add { "Add " } else { "" });
            if button(ui, &label) {
                if let Ok(mut text) = navigation::selection_for_hit(state, hit, level) {
                    if add && !current.is_empty() {
                        text = format!("({current}) or ({text})");
                    }
                    action = Some(ContextAction::Command(navigation::selection_command(
                        hit.object, text,
                    )));
                }
            }
        }
        ui.separator();
    }
    if button(ui, "Unselect") {
        action = Some(ContextAction::Command(navigation::selection_command(
            hit.object,
            String::new(),
        )));
    }
    if button(ui, "Invert Selection") {
        let text = if current.is_empty() {
            "*".into()
        } else if let Some(inner) = current.strip_prefix("!(").and_then(|s| s.strip_suffix(')')) {
            inner.into()
        } else {
            format!("!({current})")
        };
        action = Some(ContextAction::Command(navigation::selection_command(
            hit.object, text,
        )));
    }
    if button(ui, "Toggle Sidechain") && !current.is_empty() {
        let text = current
            .strip_prefix("bysidech ")
            .map(str::to_string)
            .unwrap_or_else(|| format!("bysidech {current}"));
        action = Some(ContextAction::Command(navigation::selection_command(
            hit.object, text,
        )));
    }
    for (label, byres) in [("Around Byres", true), ("Around", false)] {
        ui.menu_button(label, |ui| {
            for radius in [3, 5, 7, 10] {
                if button(ui, &format!("{radius} Å")) && !current.is_empty() {
                    let mut base = current;
                    if let Some((candidate, distance)) = base.rsplit_once(" around ") {
                        if distance.parse::<f64>().is_ok() {
                            base = candidate.strip_prefix("byres ").unwrap_or(candidate);
                            base = base
                                .strip_prefix('(')
                                .and_then(|s| s.strip_suffix(')'))
                                .unwrap_or(base);
                        }
                    }
                    let text = format!(
                        "{}({base}) around {radius}",
                        if byres { "byres " } else { "" }
                    );
                    action = Some(ContextAction::Command(navigation::selection_command(
                        hit.object, text,
                    )));
                    ui.close();
                }
            }
        });
    }
    (action, rects)
}

pub fn palette_menu(ui: &mut Ui, state: &mut DesktopAppState) -> bool {
    let mut visible = state.config_value("layout:tool-palette-collapsed") != "true";
    let changed = ui.checkbox(&mut visible, "Tool palette").changed();
    if changed {
        state.put_config_value("layout:tool-palette-collapsed", (!visible).to_string());
    }
    changed
}

pub fn cursor(ctx: &egui::Context, tool: Tool, pressed: bool) {
    ctx.set_cursor_icon(if tool != Tool::Navigate {
        egui::CursorIcon::Crosshair
    } else if pressed {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    });
}

pub fn tool_shortcut(ctx: &egui::Context) -> Option<Tool> {
    if ctx.wants_keyboard_input() || !ctx.input(|i| i.modifiers.is_none()) {
        return None;
    }
    [
        (egui::Key::N, Tool::Navigate),
        (egui::Key::B, Tool::RectSelect),
        (egui::Key::L, Tool::LassoSelect),
        (egui::Key::D, Tool::Distance),
        (egui::Key::A, Tool::Angle),
        (egui::Key::T, Tool::Torsion),
        (egui::Key::E, Tool::Bond),
    ]
    .into_iter()
    .find_map(|(key, tool)| ctx.input(|i| i.key_pressed(key)).then_some(tool))
}

pub fn palette(
    ui: &mut Ui,
    navigation: &mut Navigation,
    state: &mut DesktopAppState,
) -> (Vec<(&'static str, Rect)>, Option<Command>, bool) {
    let mut rects = Vec::new();
    let mut command = None;
    let mut collapsed = state.config_value("layout:tool-palette-collapsed") == "true";
    let label = if collapsed {
        "Show tool palette"
    } else {
        "Hide tool palette"
    };
    let response = ui
        .add_sized([30., 16.], egui::Button::new("").frame(false))
        .on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let center = response.rect.center();
    let offsets = if collapsed {
        [[-2., -4.], [2., 0.], [-2., 4.]]
    } else {
        [[-4., -2.], [0., 2.], [4., -2.]]
    };
    ui.painter().add(egui::Shape::line(
        offsets.map(|[x, y]| center + egui::vec2(x, y)).to_vec(),
        Stroke::new(1.3, ui.visuals().text_color()),
    ));
    rects.push((label, response.rect));
    let changed = response.clicked();
    if changed {
        collapsed = !collapsed;
        state.put_config_value("layout:tool-palette-collapsed", collapsed.to_string());
    }
    if collapsed {
        return (rects, command, changed);
    }
    ui.separator();
    for tool in Tool::ALL {
        if matches!(tool, Tool::RectSelect | Tool::Distance | Tool::Bond) {
            ui.separator();
        }
        if matches!(tool, Tool::Distance | Tool::Bond) {
            let (selected, rect) = options(ui, navigation, state, tool);
            command = selected.or(command);
            if let Some(rect) = rect {
                rects.push((
                    if tool == Tool::Bond {
                        "Bond editor options"
                    } else {
                        "Measure options"
                    },
                    rect,
                ));
            }
        }
        let icon = match tool {
            Tool::Navigate => Icon::Rotate,
            Tool::RectSelect => Icon::Selection,
            Tool::LassoSelect => Icon::Lasso,
            Tool::Distance => Icon::Measure,
            Tool::Angle => Icon::Angle,
            Tool::Torsion => Icon::Torsion,
            Tool::Bond => Icon::Bond,
        };
        let response = shell::icon_button(
            ui,
            icon,
            tool.label(),
            navigation.tool == tool,
            egui::vec2(30., 30.),
        );
        rects.push((tool.label(), response.rect));
        if response.clicked() {
            navigation.set_tool(tool);
        }
    }
    (rects, command, changed)
}

pub fn options(
    ui: &mut Ui,
    navigation: &mut Navigation,
    state: &DesktopAppState,
    tool: Tool,
) -> (Option<Command>, Option<Rect>) {
    let mut command = None;
    let mut rect = None;
    match tool {
        Tool::Distance | Tool::Angle | Tool::Torsion => {
            let response = ui
                .menu_button("    ", |ui| {
                    ui.set_min_width(180.);
                    ui.label("Target label set");
                    ui.add(
                        egui::TextEdit::singleline(&mut navigation.measure_target)
                            .hint_text("measure")
                            .desired_width(140.),
                    );
                    let mut names = std::collections::BTreeSet::new();
                    for o in state.active_object_summaries() {
                        for r in state
                            .renderer_summaries(o.uid)
                            .into_iter()
                            .filter(|r| r.renderer_type == "atomintr")
                        {
                            names.insert(r.name);
                        }
                    }
                    for name in names {
                        if ui
                            .selectable_label(navigation.measure_target == name, &name)
                            .clicked()
                        {
                            navigation.measure_target = name;
                            ui.close();
                        }
                    }
                })
                .response;
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    if tool == Tool::Bond {
                        "Bond editor options"
                    } else {
                        "Measure options"
                    },
                )
            });
            let center = response.rect.center();
            for dx in [-4., 0., 4.] {
                ui.painter().circle_filled(
                    center + egui::vec2(dx, 0.),
                    1.,
                    ui.visuals().text_color(),
                );
            }
            rect = Some(response.rect);
        }
        Tool::Bond => {
            let response=ui.menu_button("    ",|ui| {
                ui.set_min_width(220.);
                ui.label("Non-standard bonds");
                egui::ScrollArea::vertical().max_height(300.).show(ui,|ui| {
                    for object in state.active_object_summaries() {
                        ui.strong(&object.name);
                        let bonds = state.nonstandard_bond_summaries(object.uid).unwrap_or_default();
                        if bonds.is_empty() { ui.label("No non-standard bonds."); }
                        for bond in bonds {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} {}{}:{} — {} {}{}:{}",bond.atom1.chain_id,bond.atom1.residue_name,bond.atom1.residue_id,bond.atom1.atom_name,
                                    bond.atom2.chain_id,bond.atom2.residue_name,bond.atom2.residue_id,bond.atom2.atom_name));
                                if ui.button("Remove").clicked() {
                                    let spec = |a: &cuemol_desktop_core::GuiBondAtomSummary| {
                                        let alt = state.scene_manager().object(object.uid)
                                            .and_then(|o| o.mol().get_atom_by_id(a.serial))
                                            .and_then(|a| a.alt_loc).map(|c| c.to_string());
                                        json!({"chain":a.chain_id,"resid":format!("{}{}",a.residue_id,a.insertion_code.map(|c|c.to_string()).unwrap_or_default()),"atomName":a.atom_name,"altConf":alt})
                                    };
                                    command = Some(Command { name:"remove_bond",args:json!({"objId":object.uid.get(),"atoms":[spec(&bond.atom1),spec(&bond.atom2)]}) });
                                    ui.close();
                                }
                            });
                        }
                    }
                });
            }).response;
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    if tool == Tool::Bond {
                        "Bond editor options"
                    } else {
                        "Measure options"
                    },
                )
            });
            let center = response.rect.center();
            for dx in [-4., 0., 4.] {
                ui.painter().circle_filled(
                    center + egui::vec2(dx, 0.),
                    1.,
                    ui.visuals().text_color(),
                );
            }
            rect = Some(response.rect);
        }
        _ => {}
    }
    (command, rect)
}

pub fn feedback(
    ui: &Ui,
    state: &DesktopAppState,
    navigation: &Navigation,
    hover: Option<AtomHit>,
    atoms: &[cuemol_render::RenderAtom],
    mvp: [[f32; 4]; 4],
    rect: Rect,
) {
    let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
    let screen = |hit: AtomHit| {
        atoms
            .iter()
            .filter(|atom| {
                atom.object_uid == Some(hit.object.get()) && atom.atom_index == Some(hit.index)
            })
            .filter_map(|atom| {
                navigation::project(
                    mvp,
                    atom.center,
                    [rect.width() as f64, rect.height() as f64],
                )
            })
            .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
            .min_by(|a, b| a.distance_sq(pointer).total_cmp(&b.distance_sq(pointer)))
    };
    let painter = ui.painter();
    for hit in &navigation.picks {
        if let Some(p) = screen(*hit) {
            for d in [egui::vec2(7., 0.), egui::vec2(0., 7.)] {
                painter.line_segment([p - d, p + d], Stroke::new(1.5, Color32::YELLOW));
            }
        }
    }
    if state.config_value("view_input:hover-info") == "false" {
        return;
    }
    if let Some(hit) = hover {
        if let (Some(p), Ok(text)) = (screen(hit), hit.description(state)) {
            if state.config_value("view_input:hover-highlight") != "false" {
                painter.circle_stroke(p, 7., Stroke::new(2., Color32::from_rgb(255, 210, 70)));
            }
            let font = egui::FontId::proportional(12.);
            let galley = painter.layout_no_wrap(text, font, ui.visuals().text_color());
            let pos = Pos2::new(rect.left() + 8., rect.bottom() - galley.size().y - 26.);
            painter.rect_filled(
                Rect::from_min_size(pos - egui::vec2(4., 3.), galley.size() + egui::vec2(8., 6.)),
                3.,
                ui.visuals().window_fill(),
            );
            painter.galley(pos, galley, ui.visuals().text_color());
        }
    }
}

pub fn annotations(ui: &Ui, state: &DesktopAppState, mvp: [[f32; 4]; 4], rect: Rect) {
    let painter = ui.painter();
    let color = |text: &str| {
        let value = text.trim_start_matches('#');
        if value.len() == 6 {
            if let Ok(v) = u32::from_str_radix(value, 16) {
                return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
            }
        }
        Color32::YELLOW
    };
    for line in state.active_atom_intr_lines(mvp, rect.width(), rect.height(), 10000) {
        let a = rect.min + egui::vec2(line.x1, line.y1);
        let b = rect.min + egui::vec2(line.x2, line.y2);
        let stroke = Stroke::new(line.width, color(&line.color));
        if line.stipple {
            let length = a.distance(b);
            let direction = (b - a).normalized();
            for i in (0..length.ceil() as usize).step_by(8) {
                painter.line_segment(
                    [
                        a + direction * i as f32,
                        a + direction * (i as f32 + 4.).min(length),
                    ],
                    stroke,
                );
            }
        } else {
            painter.line_segment([a, b], stroke);
        }
    }
    for label in state
        .active_name_labels(mvp, rect.width(), rect.height(), 1000)
        .into_iter()
        .chain(state.active_atom_intr_labels(mvp, rect.width(), rect.height(), 1000))
    {
        painter.text(
            rect.min + egui::vec2(label.x, label.y),
            egui::Align2::LEFT_BOTTOM,
            label.text,
            egui::FontId::proportional(label.font_size.max(1.) as f32),
            color(&label.color),
        );
    }
}
