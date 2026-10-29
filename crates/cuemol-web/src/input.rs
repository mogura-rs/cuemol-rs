use super::*;
use cuemol_qsys::view_input::{self as input, PointerRelease};

impl App {
    pub(super) fn pointer_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
    ) -> Vec<(egui::Pos2, bool, i32, i32)> {
        let rect = response.rect;
        let before = (
            self.camera.center(),
            self.camera.rotation(),
            self.camera.zoom(),
            self.camera.slab_depth(),
        );
        let size = [rect.width() as f64, rect.height() as f64];
        self.state
            .put_runtime_value("viewport:width", size[0].to_string());
        self.state
            .put_runtime_value("viewport:height", size[1].to_string());
        let now = js_sys::global().unchecked_into::<web_sys::Window>();
        let time = js_sys::Reflect::get(&now, &"performance".into())
            .ok()
            .and_then(|performance| {
                js_sys::Reflect::get(&performance, &"now".into())
                    .ok()
                    .map(|now| (performance, now))
            })
            .and_then(|(performance, now)| {
                now.dyn_into::<js_sys::Function>()
                    .ok()
                    .and_then(|f| f.call0(&performance).ok())
            })
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as i64;
        let momentum = self.state.config_value("cuemol2.ui.mouse-momentum-scroll") == "true";
        let options = self.state.active_view_options();
        self.input.translate_momentum = momentum || options.trans_mms;
        self.input.rotate_momentum = momentum || options.rot_mms;
        self.input.tick(&mut self.camera, size[1], time);
        let mut clicks = Vec::new();
        for event in std::mem::take(&mut self.input_events) {
            let number = |name: &str| event[name].as_f64().unwrap_or(0.0);
            let point = egui::pos2(number("x") as f32, number("y") as f32);
            let local = [
                (point.x - rect.left()) as f64,
                (point.y - rect.top()) as f64,
            ];
            let time = number("time") as i64;
            let mut modifiers = number("modifiers") as i32;
            let emulate = self.state.config_value("cuemol2.ui.mouse-emulate-rbutton") == "true";
            let mut button = number("button") as i32;
            if emulate && modifiers & input::CTRL != 0 {
                if modifiers & input::LEFT != 0 {
                    modifiers = (modifiers & !(input::CTRL | input::LEFT)) | input::RIGHT;
                }
                if button == input::LEFT {
                    button = input::RIGHT;
                }
            }
            let config = self.state.resolved_view_input(self.input_device.trackpad);
            let over_viewport =
                rect.contains(point) && ui.ctx().layer_id_at(point) == Some(response.layer_id);
            match event["kind"].as_str().unwrap_or("") {
                "down" if over_viewport => {
                    self.viewport_menu = None;
                    if self.navigation.tool.is_selection() && button == input::LEFT {
                        self.rectangle_start = Some(point);
                        self.lasso_points = vec![point];
                    }
                    self.input.pointer_down(time, local);
                }
                "move" if self.input.is_pressed() => {
                    if self.rectangle_start.is_some() {
                        if !over_viewport {
                            self.rectangle_start = None;
                            self.lasso_points.clear();
                            self.input.cancel();
                        } else if self
                            .lasso_points
                            .last()
                            .is_none_or(|p| p.distance(point) >= 2.)
                        {
                            self.lasso_points.push(point);
                        }
                    } else {
                        self.input.pointer_move(
                            &mut self.camera,
                            &config,
                            size,
                            time,
                            local,
                            modifiers,
                        );
                    }
                }
                "up" => {
                    if button == input::LEFT {
                        if let Some(start) = self.rectangle_start.take() {
                            let dragged = start.distance(point) >= 3.
                                || (self.navigation.tool == ViewportTool::LassoSelect
                                    && self.lasso_points.iter().any(|p| start.distance(*p) >= 3.));
                            if dragged {
                                if over_viewport {
                                    self.select_screen_region(
                                        rect,
                                        start,
                                        point,
                                        modifiers & input::SHIFT != 0,
                                    );
                                }
                                self.lasso_points.clear();
                                self.input.cancel();
                                continue;
                            }
                            self.lasso_points.clear();
                        }
                    }
                    let release = self
                        .input
                        .pointer_up(&self.camera, &config, size, time, local);
                    if over_viewport {
                        match release {
                            PointerRelease::Click => {
                                clicks.push((point, number("clicks") >= 2., button, modifiers))
                            }
                            PointerRelease::DoubleClick => {
                                clicks.push((point, true, button, modifiers))
                            }
                            _ => {}
                        }
                    }
                }
                "wheel" if over_viewport => {
                    let delta = [number("dx"), number("dy")];
                    self.input
                        .wheel(&mut self.camera, &config, size[1], delta, modifiers);
                    self.input_device.wheel(
                        time,
                        delta,
                        number("mode") as u32,
                        event["isMac"].as_bool().unwrap_or(false),
                    );
                }
                "gesture" if over_viewport => {
                    self.input.gesture(
                        &mut self.camera,
                        &config,
                        size[1],
                        number("axis") as i32,
                        number("delta"),
                        modifiers,
                    );
                    self.input_device.gesture(time);
                }
                "cancel" => {
                    self.input.cancel();
                    self.rectangle_start = None;
                    self.lasso_points.clear();
                    self.hover_hit = None;
                }
                _ => {}
            }
        }
        if self.input.is_animating() {
            ui.ctx().request_repaint();
        }
        if before
            != (
                self.camera.center(),
                self.camera.rotation(),
                self.camera.zoom(),
                self.camera.slab_depth(),
            )
        {
            self.hover_sample = None;
        }
        clicks
    }
}
