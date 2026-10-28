use super::*;
use cuemol_command::navigation::{self as shared, Command};

impl GuiApplication {
    pub(super) fn navigation_click(
        &mut self,
        camera: &CameraController,
        position: egui::Pos2,
        double: bool,
    ) {
        let Some(PickHit::Atom {
            object_uid,
            atom_index,
        }) = self.pick_hit(camera, position)
        else {
            return;
        };
        let hit = AtomHit::from((object_uid, atom_index));
        match self
            .navigation
            .click(&self.state, hit, double, self.shift_down)
        {
            Ok(Some(command)) => {
                self.apply_navigation_commands(camera, vec![command]);
            }
            Ok(None) => self.set_status(self.navigation.message.clone()),
            Err(error) => self.set_error(error),
        }
    }

    pub(super) fn apply_navigation_commands(
        &mut self,
        camera: &CameraController,
        commands: Vec<Command>,
    ) -> bool {
        if commands.is_empty() {
            return false;
        }
        let undo = self.capture_undo();
        let mut view = camera.to_qsys_camera("__current");
        let mut status = self.navigation.message.clone();
        for command in commands {
            match cuemol_command::call_tool(&mut self.state, &mut view, command.name, &command.args)
            {
                Ok(value) => {
                    if let Some((id, selection)) = command.selection() {
                        self.selected_object = Some(id);
                        self.molstruct_object = Some(id);
                        self.selected_renderer = None;
                        self.workspace_aux_selection = None;
                        self.selection_text = selection;
                        self.push_selection_history();
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
                    self.restore_gui_undo_state(undo);
                    self.set_error(error);
                    return false;
                }
            }
        }
        self.push_undo_snapshot(undo);
        self.rebuild_snapshot();
        self.set_status(status);
        true
    }

    pub(super) fn select_navigation_region(
        &mut self,
        camera: &CameraController,
        start: egui::Pos2,
        end: egui::Pos2,
    ) {
        let Some(viewport) = self.viewport_rect else {
            return;
        };
        let mvp = camera
            .view_projection(viewport.aspect_ratio())
            .to_cols_array_2d();
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
        let hits = shared::polygon_hits(
            &self.state,
            mvp,
            [viewport.width() as f64, viewport.height() as f64],
            &points,
        );
        match shared::selection_commands(&self.state, &hits, self.shift_down) {
            Ok(commands) => {
                if self.apply_navigation_commands(camera, commands) {
                    self.set_status(format!("Selected {} atoms", hits.len()));
                } else {
                    self.set_status("No atoms in selection region");
                }
            }
            Err(error) => self.set_error(error),
        }
    }

    pub(super) fn navigation_feedback(
        &mut self,
        ui: &egui::Ui,
        camera: &CameraController,
        rect: egui::Rect,
    ) {
        self.navigation.sync_scene(self.state.active_scene());
        let mvp = camera
            .view_projection(rect.aspect_ratio())
            .to_cols_array_2d();
        if self
            .last_cursor
            .is_some_and(|p| self.pointer_in_viewport(Some(p)))
        {
            cuemol_ui::navigation::cursor(ui.ctx(), self.navigation.tool, self.input.is_pressed());
        }
        let hover = self
            .last_cursor
            .filter(|p| self.pointer_in_viewport(Some(*p)))
            .filter(|_| !self.input.is_pressed() && self.rect_select_start.is_none())
            .filter(|_| self.state.config_value("view_input:hover-info") != "false");
        if let Some(point) = hover {
            if self.hover_sample != Some((point, self.snapshot_revision, mvp)) {
                self.hover_hit = match self.pick_hit(camera, point) {
                    Some(PickHit::Atom {
                        object_uid,
                        atom_index,
                    }) => Some(AtomHit::from((object_uid, atom_index))),
                    _ => None,
                };
                self.hover_sample = Some((point, self.snapshot_revision, mvp));
            }
        } else {
            self.hover_hit = None;
            self.hover_sample = None;
        }

        cuemol_ui::navigation::feedback(
            ui,
            &self.state,
            &self.navigation,
            self.hover_hit,
            mvp,
            rect,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::{dpi::PhysicalPosition, event::DeviceId};

    fn app() -> (
        GuiApplication,
        CameraController,
        cuemol_core::Uid,
        Vec<egui::Pos2>,
    ) {
        let mut app = GuiApplication::new();
        app.state.open_bytes("picks.pdb",b"HETATM    1  C1  LIG A   1      -4.000   0.000   0.000  1.00 10.00           C\nHETATM    2  C2  LIG A   1      -1.000   0.000   0.000  1.00 10.00           C\nHETATM    3  C3  LIG A   2       1.000   2.000   0.000  1.00 10.00           C\nHETATM    4  C4  LIG A   3       4.000   1.000   1.000  1.00 10.00           C\nEND\n").unwrap();
        let id = app.state.active_object_summaries()[0].uid;
        app.rebuild_snapshot();
        let viewport = egui::Rect::from_min_size(egui::pos2(100., 50.), egui::vec2(800., 600.));
        app.viewport_rect = Some(viewport);
        let mut camera = CameraController::default();
        camera.fit_snapshot(&app.snapshot);
        let points = app
            .state
            .active_pick_atoms("")
            .unwrap()
            .iter()
            .map(|a| {
                camera
                    .project_to_viewport(vec3_from_vector(a.center), viewport)
                    .unwrap()
                    .0
            })
            .collect();
        (app, camera, id, points)
    }
    fn move_to(app: &mut GuiApplication, camera: &mut CameraController, p: egui::Pos2) {
        app.handle_window_event(
            &WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(p.x as f64, p.y as f64),
            },
            camera,
        );
    }
    fn button(app: &mut GuiApplication, camera: &mut CameraController, pressed: bool, time: u64) {
        app.input_origin = Instant::now() - Duration::from_millis(time);
        app.handle_window_event(
            &WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: if pressed {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
                button: MouseButton::Left,
            },
            camera,
        );
    }
    fn click(app: &mut GuiApplication, camera: &mut CameraController, p: egui::Pos2, time: u64) {
        move_to(app, camera, p);
        button(app, camera, true, time);
        button(app, camera, false, time + 10);
    }
    #[test]
    fn native_click_events_toggle_labels_select_residues_and_undo() {
        let (mut app, mut camera, id, points) = app();
        let center = camera.target;
        click(&mut app, &mut camera, points[0], 1000);
        let o = app.state.scene_manager().object(id).unwrap();
        assert_eq!(
            o.name_label_entries(
                o.renderers()
                    .iter()
                    .find(|r| r.renderer_type() == "*namelabel")
                    .unwrap()
            )
            .len(),
            1
        );
        assert!(app.selection_text.is_empty());
        click(&mut app, &mut camera, points[0], 1100);
        let o = app.state.scene_manager().object(id).unwrap();
        assert_eq!(o.mol().select_indices(o.mol().selection()), [0, 1]);
        assert_eq!(camera.target, center);
        camera.ortho_scale *= 1.3;
        let view = camera.to_qsys_camera("undo-view");
        app.undo(&mut camera);
        assert_eq!(camera.to_qsys_camera("undo-view"), view);
        assert!(app.state.molecule_selection_text(id).unwrap().is_empty());
        app.redo(&mut camera);
        assert_eq!(camera.to_qsys_camera("undo-view"), view);
        assert!(!app.state.molecule_selection_text(id).unwrap().is_empty());
    }
    #[test]
    fn native_measurement_picks_share_one_undo_and_selection_drags_do_not_rotate() {
        let (mut app, mut camera, id, points) = app();
        app.navigation.set_tool(ViewportTool::Angle);
        for (i, p) in points.iter().take(3).enumerate() {
            click(&mut app, &mut camera, *p, 1000 + i as u64 * 600);
        }
        assert!(app.navigation.picks.is_empty());
        assert_eq!(
            app.state
                .renderer_summaries(id)
                .iter()
                .filter(|r| r.renderer_type == "atomintr")
                .count(),
            1
        );
        let ctx = egui::Context::default();
        let viewport = app.viewport_rect.unwrap();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(viewport),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| app.draw_name_labels(ui, &camera, viewport));
            },
        );
        fn has_value(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text() == "135.00",
                egui::Shape::Vec(shapes) => shapes.iter().any(has_value),
                _ => false,
            }
        }
        assert!(
            output.shapes.iter().any(|shape| has_value(&shape.shape)),
            "measurement value must be painted in the viewport"
        );
        assert_eq!(app.undo_stack.len(), 1);
        app.undo(&mut camera);
        assert!(!app
            .state
            .renderer_summaries(id)
            .iter()
            .any(|r| r.renderer_type == "atomintr"));
        app.navigation.set_tool(ViewportTool::LassoSelect);
        let orientation = camera.orientation;
        let p = points[0];
        let polygon = [
            p - egui::vec2(10., 10.),
            p + egui::vec2(10., -10.),
            p + egui::vec2(10., 10.),
            p + egui::vec2(-10., 10.),
            p - egui::vec2(10., 10.),
        ];
        move_to(&mut app, &mut camera, polygon[0]);
        button(&mut app, &mut camera, true, 4000);
        for p in &polygon[1..] {
            move_to(&mut app, &mut camera, *p);
        }
        button(&mut app, &mut camera, false, 4200);
        assert_eq!(app.selection_text, "aid 1");
        assert_eq!(camera.orientation, orientation);
    }
}
