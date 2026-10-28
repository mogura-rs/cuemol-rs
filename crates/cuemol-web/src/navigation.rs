use super::*;
use cuemol_command::navigation::{self as shared, Command};

impl App {
    fn gpu_pick_image(&self) -> Option<std::sync::Arc<cuemol_desktop_core::picking::PickImage>> {
        if !self
            .state
            .resolved_view_input(self.input_device.trackpad)
            .gpu_pick()
            || self.camera.stereo_mode() != cuemol_qsys::StereoMode::None
        {
            return None;
        }
        self.picking
            .image(cuemol_desktop_core::picking::GpuPicking::stamp(
                self.revision,
                &self.camera,
                self.target.as_ref()?.size,
            ))
    }
    fn pick_record(
        &self,
        viewport: egui::Rect,
        point: egui::Pos2,
    ) -> Option<cuemol_desktop_core::picking::PickRecord> {
        let image = self.gpu_pick_image();
        let point = [
            (point.x - viewport.left()) as f64,
            (point.y - viewport.top()) as f64,
        ];
        let size = [viewport.width() as f64, viewport.height() as f64];
        let precision = self
            .state
            .resolved_view_input(self.input_device.trackpad)
            .hitprec();
        image
            .as_ref()
            .and_then(|image| image.hit_at(point, size, precision))
            .or_else(|| {
                cuemol_desktop_core::picking::cpu_hit(
                    self.snapshot.as_ref()?,
                    (if image.is_some() {
                        cuemol_desktop_core::offscreen::camera_pick_mvp
                    } else {
                        cuemol_desktop_core::offscreen::camera_mvp
                    })(&self.camera, viewport.aspect_ratio() as f64),
                    point,
                    size,
                    precision,
                    image.is_some(),
                )
            })
    }
    pub(super) fn screen_hit(&self, viewport: egui::Rect, point: egui::Pos2) -> Option<AtomHit> {
        self.pick_record(viewport, point)
            .map(|record| AtomHit::from((record.object_id, record.atom_index)))
    }
    pub(super) fn navigation_click(
        &mut self,
        viewport: egui::Rect,
        point: egui::Pos2,
        double: bool,
        shift: bool,
    ) {
        let Some(hit) = self.screen_hit(viewport, point) else {
            return;
        };
        match self.navigation.click(&self.state, hit, double, shift) {
            Ok(Some(command)) => {
                self.apply_navigation_commands(vec![command]);
            }
            Ok(None) => self.message = self.navigation.message.clone(),
            Err(error) => self.message = error.to_string(),
        }
    }
    pub(super) fn apply_navigation_commands(&mut self, commands: Vec<Command>) -> bool {
        if commands.is_empty() {
            return false;
        }
        let previous = self.capture_history();
        let mut status = self.navigation.message.clone();
        for command in commands {
            match cuemol_command::call_tool(
                &mut self.state,
                &mut self.camera,
                command.name,
                &command.args,
            ) {
                Ok(value) => {
                    if let Some((id, selection)) = command.selection() {
                        self.selected = Some(id);
                        self.selection = selection;
                        status = "Selection updated".into();
                    }
                    if command.name == "measure_geometry" {
                        status = format!(
                            "{}: {:.3} {}",
                            self.navigation.tool.label(),
                            value["value"].as_f64().unwrap_or(0.),
                            value["unit"].as_str().unwrap_or("")
                        );
                    } else if matches!(command.name, "add_bond" | "remove_bond") {
                        status = if value["changed"] == true {
                            "Bond updated"
                        } else {
                            "Bond unchanged"
                        }
                        .into();
                    }
                }
                Err(error) => {
                    self.restore_history(previous);
                    self.message = error.to_string();
                    return false;
                }
            }
        }
        self.history.push(previous);
        if self.history.len() > 12 {
            self.history.remove(0);
        }
        self.future.clear();
        self.revision = self.revision.wrapping_add(1);
        self.navigation.cancel();
        self.dirty = true;
        self.message = status;
        true
    }
    pub(super) fn select_screen_region(
        &mut self,
        viewport: egui::Rect,
        start: egui::Pos2,
        end: egui::Pos2,
        add: bool,
    ) {
        let points = if self.navigation.tool == ViewportTool::LassoSelect {
            self.lasso_points.clone()
        } else {
            vec![
                start,
                egui::pos2(end.x, start.y),
                end,
                egui::pos2(start.x, end.y),
            ]
        };
        let points = points
            .into_iter()
            .map(|p| {
                [
                    (p.x - viewport.left()) as f64,
                    (p.y - viewport.top()) as f64,
                ]
            })
            .collect::<Vec<_>>();
        let mvp = cuemol_desktop_core::offscreen::camera_pick_mvp(
            &self.camera,
            viewport.aspect_ratio() as f64,
        );
        let hits = shared::polygon_hits(
            &self.state,
            mvp,
            [viewport.width() as f64, viewport.height() as f64],
            &points,
        );
        match shared::selection_commands(&self.state, &hits, add) {
            Ok(commands) => {
                if self.apply_navigation_commands(commands) {
                    self.message = format!("Selected {} atoms", hits.len());
                } else {
                    self.message = "No atoms in selection region".into();
                }
            }
            Err(error) => self.message = error.to_string(),
        }
    }
    pub(super) fn navigation_feedback(&mut self, ui: &egui::Ui, response: &egui::Response) {
        self.navigation.sync_scene(self.state.active_scene());
        let rect = response.rect;
        let mvp =
            cuemol_desktop_core::offscreen::camera_mvp(&self.camera, rect.aspect_ratio() as f64);
        if ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| rect.contains(p) && ui.ctx().layer_id_at(p) == Some(response.layer_id))
        {
            cuemol_ui::navigation::cursor(ui.ctx(), self.navigation.tool, self.input.is_pressed());
        }
        let gpu_image = self.gpu_pick_image();
        self.state.sync_viewport_pick_image(
            &self.camera,
            [rect.width(), rect.height()],
            gpu_image.clone(),
        );
        let hover = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p) && ui.ctx().layer_id_at(*p) == Some(response.layer_id))
            .filter(|_| !self.input.is_pressed() && self.rectangle_start.is_none())
            .filter(|_| self.state.config_value("view_input:hover-info") != "false");
        if let Some(point) = hover {
            if self.hover_sample != Some((point, self.revision, mvp, gpu_image.is_some())) {
                let record = self.pick_record(rect, point);
                self.state
                    .set_active_hover_hit(record.map(|record| record.hit));
                self.hover_hit = self.screen_hit(rect, point);
                self.hover_sample = Some((point, self.revision, mvp, gpu_image.is_some()));
            }
        } else {
            if self.hover_sample.is_some() {
                self.state.set_active_hover_hit(None);
            }
            self.hover_hit = None;
            self.hover_sample = None;
        }

        cuemol_ui::navigation::annotations(ui, &self.state, mvp, rect);
        cuemol_ui::navigation::feedback(
            ui,
            &self.state,
            &self.navigation,
            self.hover_hit,
            &self.pick_atoms,
            mvp,
            rect,
        );
    }
    pub(super) fn viewport_context_menu(&mut self, ctx: &egui::Context) {
        let Some((pos, hit)) = self.viewport_menu else {
            return;
        };
        let mut action = None;
        egui::Area::new(egui::Id::new("viewport-context-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    let (selected, rects) =
                        cuemol_ui::navigation::context_menu(ui, &self.state, hit);
                    action = selected;
                    for (label, rect) in rects {
                        self.ui_rects
                            .insert(label, [rect.min.x, rect.min.y, rect.width(), rect.height()]);
                    }
                });
            });
        if let Some(action) = action {
            match action {
                cuemol_ui::navigation::ContextAction::Center => {
                    if let Some(center) = hit.position(&self.state) {
                        self.camera.set_center(center);
                        self.input.cancel();
                    }
                }
                cuemol_ui::navigation::ContextAction::Command(command) => {
                    self.apply_navigation_commands(vec![command]);
                }
            }
            self.viewport_menu = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.viewport_menu = None;
        }
    }
}
