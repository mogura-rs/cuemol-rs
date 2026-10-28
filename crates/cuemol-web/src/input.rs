use super::*;
use cuemol_qsys::view_input::{self as input, PointerRelease};

impl App {
    pub(super) fn pointer_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
    ) -> Vec<(egui::Pos2, bool)> {
        let rect = response.rect;
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
        self.input.translate_momentum = momentum;
        self.input.rotate_momentum = momentum;
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
            let modifiers = number("modifiers") as i32;
            let config = self.state.resolved_view_input(self.input_device.trackpad);
            let over_viewport =
                rect.contains(point) && ui.ctx().layer_id_at(point) == Some(response.layer_id);
            match event["kind"].as_str().unwrap_or("") {
                "down"
                    if over_viewport
                        && (!self.rectangle_select || modifiers & input::LEFT == 0) =>
                {
                    self.input.pointer_down(time, local)
                }
                "move" if self.input.is_pressed() => {
                    self.input.pointer_move(
                        &mut self.camera,
                        &config,
                        size,
                        time,
                        local,
                        modifiers,
                    );
                }
                "up" => {
                    let release = self
                        .input
                        .pointer_up(&self.camera, &config, size, time, local);
                    if number("button") as i32 == input::LEFT && over_viewport {
                        match release {
                            PointerRelease::Click => clicks.push((point, false)),
                            PointerRelease::DoubleClick => clicks.push((point, true)),
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
                "cancel" => self.input.cancel(),
                _ => {}
            }
        }
        if self.input.is_animating() {
            ui.ctx().request_repaint();
        }
        clicks
    }

    pub(super) fn center_screen_atom(&mut self, rect: egui::Rect, point: egui::Pos2) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let mvp = glam::Mat4::from_cols_array_2d(&cuemol_desktop_core::offscreen::camera_mvp(
            &self.camera,
            rect.aspect_ratio() as f64,
        ));
        let hit_precision = self
            .state
            .resolved_view_input(self.input_device.trackpad)
            .hitprec() as f32;
        let nearest = snapshot
            .atoms
            .iter()
            .filter_map(|atom| {
                let p = mvp
                    * glam::Vec4::new(
                        atom.center.x as f32,
                        atom.center.y as f32,
                        atom.center.z as f32,
                        1.0,
                    );
                if p.w <= 0.0 {
                    return None;
                }
                let ndc = p.truncate() / p.w;
                if !(0.0..=1.0).contains(&ndc.z) {
                    return None;
                }
                let screen = egui::pos2(
                    rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
                    rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
                );
                let distance = screen.distance(point);
                (distance < hit_precision).then_some((distance, atom.center))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, center)) = nearest {
            self.camera.set_center(center);
        }
    }
}
