pub use cuemol_desktop_core::sequence_residue_number;
use cuemol_desktop_core::{DesktopAppState, GuiSeqChain, GuiSequenceCache, GuiSequenceLayout};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencePosition {
    pub object_uid: cuemol_core::Uid,
    pub chain_id: String,
    pub position: usize,
}

#[derive(Debug, Clone)]
pub enum SequenceAction {
    Range {
        object_uid: cuemol_core::Uid,
        anchor_selection: String,
        range_selection: String,
        toggle: bool,
        center_selection: Option<String>,
    },
    Selection {
        object_uid: cuemol_core::Uid,
        expression: String,
        status: &'static str,
    },
    Center {
        object_uid: cuemol_core::Uid,
        selection: String,
    },
    Copy(String),
}

const SEQUENCE_ROW_HEIGHT: f32 = 19.0;
const SEQUENCE_RULER_HEIGHT: f32 = 16.0;

struct SequenceColors {
    header: egui::Color32,
    row: egui::Color32,
    selection: egui::Color32,
    marker: egui::Color32,
    track: egui::Color32,
    ruler: egui::Color32,
}

impl SequenceColors {
    fn new(dark: bool) -> Self {
        use egui::Color32;
        if dark {
            Self {
                header: Color32::from_rgb(33, 35, 43),
                row: Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                selection: Color32::from_rgba_unmultiplied(95, 175, 215, 128),
                marker: Color32::from_rgb(224, 108, 117),
                track: Color32::from_rgb(135, 195, 138),
                ruler: Color32::from_rgb(147, 155, 166),
            }
        } else {
            Self {
                header: Color32::from_rgb(232, 233, 236),
                row: Color32::from_rgba_unmultiplied(0, 0, 0, 15),
                selection: Color32::from_rgba_unmultiplied(41, 121, 181, 89),
                marker: Color32::from_rgb(198, 40, 40),
                track: Color32::from_rgb(46, 125, 50),
                ruler: Color32::from_rgb(154, 157, 165),
            }
        }
    }
}

pub fn sequence_column_count(chains: &[GuiSeqChain], _cell_width: f32) -> usize {
    GuiSequenceLayout::new(chains).columns
}

fn sequence_positions_share_row(lhs: &SequencePosition, rhs: &SequencePosition) -> bool {
    lhs.object_uid == rhs.object_uid && lhs.chain_id == rhs.chain_id
}

pub fn sequence_position_in_drag_range(
    position: &SequencePosition,
    start: Option<&SequencePosition>,
    end: Option<&SequencePosition>,
) -> bool {
    let (Some(start), Some(end)) = (start, end) else {
        return false;
    };
    if !sequence_positions_share_row(start, end) || !sequence_positions_share_row(start, position) {
        return false;
    }
    let (first, last) = if start.position <= end.position {
        (start.position, end.position)
    } else {
        (end.position, start.position)
    };
    (first..=last).contains(&position.position)
}

pub fn sequence_range_action(
    chains: &[GuiSeqChain],
    start: &SequencePosition,
    end: &SequencePosition,
    toggle: bool,
    center: bool,
) -> Option<SequenceAction> {
    if !sequence_positions_share_row(start, end) {
        return None;
    }
    let chain = chains
        .iter()
        .find(|chain| chain.object_uid == start.object_uid && chain.chain_id == start.chain_id)?;
    let anchor_selection = chain.residues.get(start.position)?.selection.clone();
    let center_selection = if center {
        Some(chain.residues.get(end.position)?.selection.clone())
    } else {
        None
    };
    let (first, last) = if start.position <= end.position {
        (start.position, end.position)
    } else {
        (end.position, start.position)
    };
    let range_selection = chain.residues[first..=last]
        .iter()
        .map(|residue| residue.selection.as_str())
        .collect::<Vec<_>>()
        .join("|");
    Some(SequenceAction::Range {
        object_uid: start.object_uid,
        anchor_selection,
        range_selection,
        toggle,
        center_selection,
    })
}

#[derive(Debug)]
pub struct SequencePanel {
    cache: GuiSequenceCache,
    layout: GuiSequenceLayout,
    cell_width: f32,
    marker: Option<SequencePosition>,
    drag_start: Option<SequencePosition>,
    drag_end: Option<SequencePosition>,
    shift_at_down: bool,
    context_hit: Option<SequencePosition>,
    scroll_offset: egui::Vec2,
    pub name_width: f32,
    name_drag_start_width: Option<f32>,
    pub rects: BTreeMap<String, [f32; 4]>,
}

impl Default for SequencePanel {
    fn default() -> Self {
        Self {
            cache: GuiSequenceCache::default(),
            layout: GuiSequenceLayout::default(),
            cell_width: 0.,
            marker: None,
            drag_start: None,
            drag_end: None,
            shift_at_down: false,
            context_hit: None,
            scroll_offset: egui::Vec2::ZERO,
            name_width: 140.,
            name_drag_start_width: None,
            rects: BTreeMap::new(),
        }
    }
}

impl SequencePanel {
    pub fn new(name_width: f32) -> Self {
        Self {
            name_width: name_width.clamp(50., 400.),
            ..Self::default()
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut DesktopAppState,
        generation: u64,
    ) -> Option<SequenceAction> {
        self.rects.clear();
        let cell_width =
            ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(13.), 'M')) + 2.;
        if self.cache.refresh(state, "", generation) {
            self.layout = GuiSequenceLayout::new(self.cache.rows());
        }
        self.cell_width = cell_width;
        let chains = self.cache.rows();
        let layout = &self.layout;
        if chains.is_empty() {
            self.marker = None;
            self.drag_start = None;
            self.context_hit = None;
            ui.weak("No molecule loaded.");
            return None;
        }
        let mut action = None;
        let colors = SequenceColors::new(ui.visuals().dark_mode);
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, egui::Sense::hover());
        let name_width = self.name_width.min((full.width() - 30.).max(0.));
        let split_x = full.left() + name_width;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(full.min, egui::pos2(split_x, full.bottom())),
            0.,
            colors.header,
        );
        let splitter_rect = egui::Rect::from_min_max(
            egui::pos2(split_x, full.top()),
            egui::pos2(split_x + 5., full.bottom()),
        );
        let splitter = ui.interact(
            splitter_rect,
            ui.id().with("sequence_splitter"),
            egui::Sense::drag(),
        );
        ui.painter().line_segment(
            [splitter_rect.center_top(), splitter_rect.center_bottom()],
            egui::Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        if splitter.drag_started() {
            self.name_drag_start_width = Some(name_width);
        }
        if splitter.dragged() {
            if let Some(start) = self.name_drag_start_width {
                self.name_width = (start + splitter.drag_delta().x).clamp(50., 400.);
            }
        }
        if splitter.drag_stopped() {
            self.name_drag_start_width = None;
            state.put_config_value("sequence_panel:name_width", self.name_width.to_string());
        }
        let grid_rect = egui::Rect::from_min_max(
            egui::pos2(splitter_rect.right(), full.top() + SEQUENCE_RULER_HEIGHT),
            full.max,
        );
        let ruler = egui::Rect::from_min_max(
            egui::pos2(grid_rect.left(), full.top()),
            grid_rect.right_top(),
        );
        ui.painter().rect_filled(ruler, 0., colors.header);
        ui.painter().line_segment(
            [ruler.left_bottom(), ruler.right_bottom()],
            egui::Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        record_rect(&mut self.rects, "Sequence grid", grid_rect);
        record_rect(&mut self.rects, "Sequence splitter", splitter_rect);
        let content_width = (layout.columns + 1) as f32 * cell_width;
        let content_height = chains.len() as f32 * SEQUENCE_ROW_HEIGHT;
        let mut origin = grid_rect.min - self.scroll_offset;
        ui.scope_builder(egui::UiBuilder::new().max_rect(grid_rect), |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(grid_rect));
            let output = egui::ScrollArea::both()
                .id_salt("sequence_grid")
                .auto_shrink([false, false])
                .scroll_source(
                    egui::scroll_area::ScrollSource::SCROLL_BAR
                        | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
                )
                .scroll_offset(self.scroll_offset)
                .show_viewport(ui, |ui, viewport| {
                    let (canvas, response) = ui.allocate_exact_size(
                        egui::vec2(content_width, content_height),
                        egui::Sense::click_and_drag(),
                    );
                    origin = canvas.min;
                    let visible = ui.clip_rect().intersect(canvas);
                    let first_row = ((viewport.top() / SEQUENCE_ROW_HEIGHT).floor().max(0.)
                        as usize)
                        .min(chains.len());
                    let last_row = ((viewport.bottom() / SEQUENCE_ROW_HEIGHT).ceil().max(0.)
                        as usize)
                        .min(chains.len());
                    let first_column = (viewport.left() / cell_width).floor().max(0.) as usize;
                    let last_column = ((viewport.right() / cell_width).ceil().max(0.) as usize)
                        .min(layout.columns);
                    for (row_index, chain) in
                        chains.iter().enumerate().take(last_row).skip(first_row)
                    {
                        let y = origin.y + row_index as f32 * SEQUENCE_ROW_HEIGHT;
                        if row_index % 2 == 1 {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(origin.x, y),
                                    egui::vec2(content_width, SEQUENCE_ROW_HEIGHT),
                                ),
                                0.,
                                colors.row,
                            );
                        }
                        let first_residue = chain
                            .residues
                            .partition_point(|residue| layout.column(residue) < first_column);
                        for residue in chain.residues.iter().skip(first_residue) {
                            let column = layout.column(residue);
                            if column >= last_column {
                                break;
                            }
                            let cell = egui::Rect::from_min_size(
                                egui::pos2(origin.x + column as f32 * cell_width, y),
                                egui::vec2(cell_width, SEQUENCE_ROW_HEIGHT),
                            );
                            if residue.selected {
                                ui.painter().rect_filled(cell, 0., colors.selection);
                            }
                            ui.painter().text(
                                cell.center(),
                                egui::Align2::CENTER_CENTER,
                                if residue.letter == "X" {
                                    "*"
                                } else {
                                    &residue.letter
                                },
                                egui::FontId::monospace(13.),
                                ui.visuals().text_color(),
                            );
                        }
                    }
                    let pointer = ui.input(|input| input.pointer.hover_pos());
                    let hover =
                        pointer
                            .filter(|point| visible.contains(*point))
                            .and_then(|point| {
                                sequence_hit(chains, layout, origin, cell_width, point, None)
                            });
                    if let Some(hit) = &hover {
                        let chain = sequence_chain(chains, hit).unwrap();
                        let residue = &chain.residues[hit.position];
                        response.clone().on_hover_text(format!(
                            "{} {}",
                            residue.res_name.trim(),
                            residue.residue_index
                        ));
                    }
                    if response.hovered() && ui.input(|input| input.pointer.primary_pressed()) {
                        if let Some(hit) = hover.clone() {
                            self.shift_at_down = ui.input(|input| input.modifiers.shift);
                            self.drag_start = Some(hit.clone());
                            self.drag_end = Some(hit.clone());
                            if !self.shift_at_down {
                                self.marker = Some(hit);
                            }
                        }
                    }
                    if self.drag_start.is_some()
                        && ui.input(|input| {
                            input.pointer.primary_down() || input.pointer.primary_released()
                        })
                    {
                        let hit = pointer.and_then(|point| {
                            sequence_hit(
                                chains,
                                layout,
                                origin,
                                cell_width,
                                point,
                                self.drag_start.as_ref(),
                            )
                        });
                        if hit.is_some() || ui.input(|input| input.pointer.primary_released()) {
                            self.drag_end = hit;
                        }
                    }
                    if response.secondary_clicked() {
                        self.context_hit = hover;
                        self.marker = self.context_hit.clone();
                    }
                    if let Some(marker) = self.marker.as_ref().and_then(|marker| {
                        sequence_cell_rect(chains, layout, origin, cell_width, marker)
                    }) {
                        ui.painter().rect_stroke(
                            marker,
                            0.,
                            egui::Stroke::new(1., colors.marker),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if let (Some(start), Some(end)) = (
                        self.drag_start.as_ref().and_then(|hit| {
                            sequence_cell_rect(chains, layout, origin, cell_width, hit)
                        }),
                        self.drag_end.as_ref().and_then(|hit| {
                            sequence_cell_rect(chains, layout, origin, cell_width, hit)
                        }),
                    ) {
                        ui.painter().rect_stroke(
                            start.union(end),
                            0.,
                            egui::Stroke::new(1., colors.track),
                            egui::StrokeKind::Inside,
                        );
                    }
                    response.context_menu(|ui| {
                        let Some(hit) = &self.context_hit else {
                            return;
                        };
                        let Some(chain) = sequence_chain(chains, hit) else {
                            return;
                        };
                        let Some(residue) = chain.residues.get(hit.position) else {
                            return;
                        };
                        let current = state
                            .molecule_selection_text(chain.object_uid)
                            .ok()
                            .unwrap_or("");
                        ui.add_enabled(
                            false,
                            egui::Button::new(format!(
                                "{} {}{} {}",
                                chain.object_name,
                                if chain.chain_id.is_empty() {
                                    "_"
                                } else {
                                    &chain.chain_id
                                },
                                residue.residue_index,
                                residue.res_name.trim()
                            )),
                        );
                        ui.separator();
                        if menu_button(ui, &mut self.rects, "Center here") {
                            action = Some(SequenceAction::Center {
                                object_uid: chain.object_uid,
                                selection: residue.selection.clone(),
                            });
                        }
                        if menu_button(ui, &mut self.rects, "Toggle sel") {
                            action = sequence_range_action(chains, hit, hit, true, false);
                        }
                        for (label, by_residue) in [("Around Byresid", true), ("Around", false)] {
                            let response = ui.menu_button(label, |ui| {
                                for radius in [3., 5., 7., 10.] {
                                    if menu_button(ui, &mut self.rects, &format!("{radius:.0} Å"))
                                        && !current.is_empty()
                                    {
                                        action = Some(SequenceAction::Selection {
                                            object_uid: chain.object_uid,
                                            expression: around_selection_expression(
                                                current, radius, by_residue,
                                            ),
                                            status: "Around mol selection",
                                        });
                                    }
                                }
                            });
                            record_rect(&mut self.rects, label, response.response.rect);
                        }
                        if menu_button(ui, &mut self.rects, "Unselect all") {
                            action = Some(SequenceAction::Selection {
                                object_uid: chain.object_uid,
                                expression: String::new(),
                                status: "Unselect all",
                            });
                        }
                        if menu_button(ui, &mut self.rects, "Invert sel") {
                            action = Some(SequenceAction::Selection {
                                object_uid: chain.object_uid,
                                expression: invert_selection_expression(current),
                                status: "Invert sel",
                            });
                        }
                        ui.separator();
                        if menu_button(ui, &mut self.rects, "Copy sequence") {
                            action = Some(SequenceAction::Copy(
                                chain
                                    .residues
                                    .iter()
                                    .map(|residue| {
                                        if residue.letter == "X" {
                                            "*"
                                        } else {
                                            &residue.letter
                                        }
                                    })
                                    .collect(),
                            ));
                        }
                    });
                });
            self.scroll_offset = output.state.offset;
        });
        self.rects.insert(
            "Sequence origin".into(),
            [origin.x, origin.y, cell_width, SEQUENCE_ROW_HEIGHT],
        );
        let names = egui::Rect::from_min_max(
            full.min + egui::vec2(0., SEQUENCE_RULER_HEIGHT),
            egui::pos2(split_x, full.bottom()),
        );
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(names));
        let first_row = (self.scroll_offset.y / SEQUENCE_ROW_HEIGHT).floor().max(0.) as usize;
        let last_row = ((self.scroll_offset.y + names.height()) / SEQUENCE_ROW_HEIGHT)
            .ceil()
            .max(0.) as usize;
        for (row, chain) in chains.iter().enumerate().take(last_row).skip(first_row) {
            let y = origin.y + row as f32 * SEQUENCE_ROW_HEIGHT;
            let rect = egui::Rect::from_min_size(
                egui::pos2(full.left(), y),
                egui::vec2(name_width, SEQUENCE_ROW_HEIGHT),
            );
            if row % 2 == 1 {
                painter.rect_filled(rect, 0., colors.row);
            }
            painter.text(
                rect.left_center() + egui::vec2(8., 0.),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{}:{}",
                    if chain.chain_id.is_empty() {
                        "_"
                    } else {
                        &chain.chain_id
                    },
                    chain.object_name
                ),
                egui::FontId::proportional(13.),
                ui.visuals().text_color(),
            );
        }
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(ruler));
        let first_column = (self.scroll_offset.x / cell_width).floor().max(0.) as usize;
        let last_column = (((self.scroll_offset.x + ruler.width()) / cell_width)
            .ceil()
            .max(0.) as usize)
            .min(layout.columns);
        for column in first_column..last_column {
            let x = origin.x + (column as f32 + 0.5) * cell_width;
            painter.line_segment(
                [
                    egui::pos2(x, ruler.top() + 12.),
                    egui::pos2(x, ruler.bottom()),
                ],
                egui::Stroke::new(1., colors.ruler),
            );
            if let Some(number) = layout
                .ruler_number(column)
                .filter(|number| *number != 0 && number % 5 == 0)
            {
                painter.text(
                    egui::pos2(x, ruler.top()),
                    egui::Align2::CENTER_TOP,
                    number,
                    egui::FontId::monospace(10.),
                    colors.ruler,
                );
            }
        }
        if ui.input(|input| input.pointer.primary_released()) {
            let start = self.drag_start.take();
            let end = self.drag_end.take();
            if let (Some(start), Some(end)) = (start, end) {
                if start != end {
                    self.marker = Some(end.clone());
                    action = sequence_range_action(chains, &start, &end, true, false);
                } else if let Some(marker) = self.marker.as_ref().filter(|_| self.shift_at_down) {
                    action = sequence_range_action(chains, marker, &end, false, false);
                    if action.is_some() {
                        self.marker = Some(end);
                    }
                } else {
                    self.marker = Some(end.clone());
                    action = sequence_range_action(chains, &start, &end, true, true);
                }
            }
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape) || !input.focused) {
            self.drag_start = None;
            self.drag_end = None;
        }
        action
    }
}

fn sequence_chain<'a>(
    chains: &'a [GuiSeqChain],
    position: &SequencePosition,
) -> Option<&'a GuiSeqChain> {
    chains.iter().find(|chain| {
        chain.object_uid == position.object_uid && chain.chain_id == position.chain_id
    })
}

fn sequence_hit(
    chains: &[GuiSeqChain],
    layout: &GuiSequenceLayout,
    origin: egui::Pos2,
    cell_width: f32,
    point: egui::Pos2,
    pinned: Option<&SequencePosition>,
) -> Option<SequencePosition> {
    if point.x < origin.x {
        return None;
    }
    let row = if let Some(pinned) = pinned {
        chains.iter().position(|chain| {
            chain.object_uid == pinned.object_uid && chain.chain_id == pinned.chain_id
        })?
    } else {
        if point.y < origin.y {
            return None;
        }
        ((point.y - origin.y) / SEQUENCE_ROW_HEIGHT).floor() as usize
    };
    let chain = chains.get(row)?;
    let column = ((point.x - origin.x) / cell_width).floor() as usize;
    let position = chain
        .residues
        .partition_point(|residue| layout.column(residue) < column);
    (chain
        .residues
        .get(position)
        .map(|residue| layout.column(residue))
        == Some(column))
    .then(|| SequencePosition {
        object_uid: chain.object_uid,
        chain_id: chain.chain_id.clone(),
        position,
    })
}

fn sequence_cell_rect(
    chains: &[GuiSeqChain],
    layout: &GuiSequenceLayout,
    origin: egui::Pos2,
    cell_width: f32,
    hit: &SequencePosition,
) -> Option<egui::Rect> {
    let row = chains
        .iter()
        .position(|chain| chain.object_uid == hit.object_uid && chain.chain_id == hit.chain_id)?;
    let column = layout.column(chains[row].residues.get(hit.position)?);
    Some(egui::Rect::from_min_size(
        origin + egui::vec2(column as f32 * cell_width, row as f32 * SEQUENCE_ROW_HEIGHT),
        egui::vec2(cell_width, SEQUENCE_ROW_HEIGHT),
    ))
}

fn record_rect(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, rect: egui::Rect) {
    rects.insert(
        name.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}

fn menu_button(ui: &mut egui::Ui, rects: &mut BTreeMap<String, [f32; 4]>, label: &str) -> bool {
    let response = ui.button(label);
    record_rect(rects, label, response.rect);
    if response.clicked() {
        ui.close();
        true
    } else {
        false
    }
}

fn invert_selection_expression(current: &str) -> String {
    let current = current.trim();
    if current.is_empty() {
        return "*".to_string();
    }
    if let Some(inner) = current
        .strip_prefix("!(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        inner.trim().to_string()
    } else {
        format!("!({current})")
    }
}

fn around_selection_expression(current: &str, radius: f64, by_residue: bool) -> String {
    let current = current.trim();
    let mut base = current;
    if let Some((candidate, distance)) = current.rsplit_once(" around ") {
        if distance.trim().parse::<f64>().is_ok() {
            base = candidate.trim();
            if let Some(without_byres) = base.strip_prefix("byres ") {
                base = without_byres.trim();
            }
            if let Some(inner) = base
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
            {
                base = inner.trim();
            }
        }
    }
    let base = if by_residue {
        format!("byres ({base})")
    } else {
        format!("({base})")
    };
    format!("{base} around {radius}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_desktop_core::PreparedFile;

    fn state() -> DesktopAppState {
        let mut state = DesktopAppState::new();
        state.ensure_active_scene();
        let data = b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
                     ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C  \n\
                     ATOM      3  CA  SER A   3      20.000   0.000   0.000  1.00 10.00           C  \nEND\n";
        state
            .open_prepared(PreparedFile::parse("sequence.pdb", data).unwrap())
            .unwrap();
        state
    }

    fn frame(
        ctx: &egui::Context,
        panel: &mut SequencePanel,
        state: &mut DesktopAppState,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
        generation: u64,
    ) -> Option<SequenceAction> {
        let mut action = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 300.),
                )),
                events,
                modifiers,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = panel.show(ui, state, generation);
                });
            },
        );
        action
    }

    fn pointer(
        pos: egui::Pos2,
        button: egui::PointerButton,
        pressed: bool,
        modifiers: egui::Modifiers,
    ) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            },
        ]
    }

    fn cell(panel: &SequencePanel, column: usize) -> egui::Pos2 {
        let [x, y, width, height] = panel.rects["Sequence origin"];
        egui::pos2(x + (column as f32 + 0.5) * width, y + height * 0.5)
    }

    #[test]
    fn sequence_pointer_click_shift_drag_and_context_actions() {
        let ctx = egui::Context::default();
        let mut panel = SequencePanel::default();
        let mut state = state();
        let modifiers = egui::Modifiers::default();
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        let first = cell(&panel, 1);
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(first, egui::PointerButton::Primary, true, modifiers),
            modifiers,
            0,
        );
        let click = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(first, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        )
        .unwrap();
        assert!(
            matches!(click, SequenceAction::Range { toggle: true, center_selection: Some(ref text), .. } if text == "'A'.1.*")
        );
        let third = cell(&panel, 3);
        let shift = egui::Modifiers {
            shift: true,
            ..modifiers
        };
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(third, egui::PointerButton::Primary, true, shift),
            shift,
            0,
        );
        let range = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(third, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        )
        .unwrap();
        assert!(
            matches!(range, SequenceAction::Range { toggle: false, center_selection: None, ref range_selection, .. } if range_selection == "'A'.1.*|'A'.2.*|'A'.3.*")
        );
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(first, egui::PointerButton::Primary, true, modifiers),
            modifiers,
            0,
        );
        let outside_row = third + egui::vec2(0., 60.);
        frame(
            &ctx,
            &mut panel,
            &mut state,
            vec![egui::Event::PointerMoved(outside_row)],
            modifiers,
            0,
        );
        let drag = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(outside_row, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        )
        .unwrap();
        assert!(
            matches!(drag, SequenceAction::Range { toggle: true, center_selection: None, ref range_selection, .. } if range_selection == "'A'.1.*|'A'.2.*|'A'.3.*")
        );
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(third, egui::PointerButton::Secondary, true, modifiers),
            modifiers,
            0,
        );
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(third, egui::PointerButton::Secondary, false, modifiers),
            modifiers,
            0,
        );
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        let [x, y, w, h] = panel.rects["Unselect all"];
        let clear = egui::pos2(x + w / 2., y + h / 2.);
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(clear, egui::PointerButton::Primary, true, modifiers),
            modifiers,
            0,
        );
        let action = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(clear, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        )
        .unwrap();
        assert!(
            matches!(action, SequenceAction::Selection { ref expression, .. } if expression.is_empty())
        );
    }

    #[test]
    fn sequence_cells_reach_signed_inserted_and_distant_residues() {
        let mut state = DesktopAppState::new();
        state.ensure_active_scene();
        let mut text = String::new();
        for (serial, (number, insertion)) in [
            (-2, None),
            (-1, None),
            (0, None),
            (1, None),
            (1, Some('A')),
            (1, Some('B')),
            (2, None),
            (5000, None),
        ]
        .into_iter()
        .enumerate()
        {
            text.push_str(&format!("ATOM  {:5}  CA  ALA A{number:4}{}      0.000   0.000   0.000  1.00 10.00           C\n", serial + 1, insertion.unwrap_or(' ')));
        }
        state
            .open_prepared(PreparedFile::parse("indices.pdb", text.as_bytes()).unwrap())
            .unwrap();
        let ctx = egui::Context::default();
        let mut panel = SequencePanel::default();
        let modifiers = egui::Modifiers::default();
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        for (column, index) in [
            (0, "-2"),
            (1, "-1"),
            (2, "0"),
            (3, "1"),
            (4, "1A"),
            (5, "1B"),
            (6, "2"),
        ] {
            let point = cell(&panel, column);
            frame(
                &ctx,
                &mut panel,
                &mut state,
                pointer(point, egui::PointerButton::Primary, true, modifiers),
                modifiers,
                0,
            );
            let action = frame(
                &ctx,
                &mut panel,
                &mut state,
                pointer(point, egui::PointerButton::Primary, false, modifiers),
                modifiers,
                0,
            )
            .unwrap();
            assert!(
                matches!(action, SequenceAction::Range { center_selection: Some(ref selection), .. } if selection == &format!("'A'.{index}.*"))
            );
        }
        panel.scroll_offset.x = 5004. * panel.cell_width - 100.;
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        let point = cell(&panel, 5004);
        assert!((0.0..800.0).contains(&point.x));
        assert_eq!(
            panel.cache.rows()[0].residues.last().unwrap().residue_index,
            "5000"
        );
        assert_eq!(
            panel
                .layout
                .column(panel.cache.rows()[0].residues.last().unwrap()),
            5004
        );
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(point, egui::PointerButton::Primary, true, modifiers),
            modifiers,
            0,
        );
        assert!(
            panel.drag_start.is_some(),
            "point={point:?}, rects={:?}, offset={:?}",
            panel.rects,
            panel.scroll_offset
        );
        let action = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(point, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        )
        .expect("distant residue click");
        assert!(
            matches!(action, SequenceAction::Range { center_selection: Some(ref selection), .. } if selection == "'A'.5000.*")
        );
        assert_eq!(panel.layout.ruler_number(5004), Some(5000));
        assert_eq!(panel.layout.ruler_number(4), None);
    }

    #[test]
    fn sequence_initial_shift_click_toggles_and_centers() {
        let mut state = state();
        let ctx = egui::Context::default();
        let mut panel = SequencePanel::default();
        let modifiers = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        frame(&ctx, &mut panel, &mut state, Vec::new(), modifiers, 0);
        let point = cell(&panel, 1);
        frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(point, egui::PointerButton::Primary, true, modifiers),
            modifiers,
            0,
        );
        let action = frame(
            &ctx,
            &mut panel,
            &mut state,
            pointer(point, egui::PointerButton::Primary, false, modifiers),
            modifiers,
            0,
        );
        assert!(matches!(
            action,
            Some(SequenceAction::Range {
                toggle: true,
                center_selection: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn sequence_cache_refreshes_after_selection_edit_and_scene_replacement() {
        let mut state = state();
        let id = state.active_object_summaries()[0].uid;
        let mut cache = GuiSequenceCache::default();
        assert!(cache.refresh(&state, "", 0));
        let pointer = cache.rows().as_ptr();
        assert!(!cache.refresh(&state, "", 0));
        assert_eq!(cache.rows().as_ptr(), pointer);
        state.set_molecule_selection_text(id, "'A'.2.*").unwrap();
        assert!(cache.refresh(&state, "", 1));
        assert_eq!(
            cache.rows()[0]
                .residues
                .iter()
                .map(|r| r.selected)
                .collect::<Vec<_>>(),
            [false, true, false]
        );
        let before = state.clone();
        let object = state.scene_manager_mut().object_mut(id).unwrap();
        object.set_name("renamed");
        object.mol_mut().atoms_mut()[0].res_id = 7;
        assert!(cache.refresh(&state, "", 2));
        assert_eq!(cache.rows()[0].object_name, "renamed");
        assert_eq!(
            cache.rows()[0]
                .residues
                .iter()
                .map(|r| r.res_id)
                .collect::<Vec<_>>(),
            [2, 3, 7]
        );
        state = before;
        assert!(cache.refresh(&state, "", 3));
        assert_eq!(cache.rows()[0].residues[0].res_id, 1);
        state.new_scene("empty", true);
        assert!(cache.refresh(&state, "", 3));
        assert!(cache.rows().is_empty());
    }
}
