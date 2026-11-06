use cuemol_command::about::{self, Rows};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct AboutDialog {
    open: bool,
    rows: Rows,
    pub rects: BTreeMap<String, egui::Rect>,
}

impl AboutDialog {
    pub fn open(&mut self, rows: Rows) {
        self.rows = rows;
        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.rects.clear();
        if !self.open {
            return;
        }
        let mut close = false;
        egui::Window::new("About cuemol-rs")
            .open(&mut self.open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 64.).clamp(160., 520.));
                crate::shell::logo(ui, 96.);
                ui.heading(about::PRODUCT);
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 240.).max(80.))
                    .show(ui, |ui| {
                        egui::Grid::new("about-rows")
                            .num_columns(2)
                            .max_col_width((ui.available_width() - 100.).max(60.))
                            .show(ui, |ui| {
                                for (label, value) in &self.rows {
                                    ui.strong(format!("{label}:"));
                                    ui.add(egui::Label::new(value).wrap().selectable(true));
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let copy = ui.button("Copy");
                    self.rects.insert("Copy About".into(), copy.rect);
                    if copy.clicked() {
                        ctx.copy_text(about::text(&self.rows));
                    }
                    let button = ui.button("Close");
                    self.rects.insert("Close About".into(), button.rect);
                    close = button.clicked();
                });
            });
        self.open &= !close && !ctx.input(|input| input.key_pressed(egui::Key::Escape));
    }
}
