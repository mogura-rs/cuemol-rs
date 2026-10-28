//! Camera input shared by the native and browser viewports.

use crate::{
    mouse::{DragState, MouseEventHandler},
    Camera, ViewInputConfig,
};
use cuemol_core::{Quat, Vector4};

pub const SHIFT: i32 = 1;
pub const CTRL: i32 = 2;
pub const ALT: i32 = 4;
pub const LEFT: i32 = 8;
pub const MIDDLE: i32 = 16;
pub const RIGHT: i32 = 32;

pub fn parse_binding(value: &str) -> i32 {
    let (mut modifiers, mut axis) = (0, 0);
    for token in value.split('|').map(|v| v.trim().to_ascii_uppercase()) {
        match token.as_str() {
            "SHIFT" => modifiers |= SHIFT,
            "CTRL" => modifiers |= CTRL,
            "ALT" => modifiers |= ALT,
            "LBUTTON" => modifiers |= LEFT,
            "MBUTTON" => modifiers |= MIDDLE,
            "RBUTTON" => modifiers |= RIGHT,
            "MOUSE_X" => axis = 0,
            "MOUSE_Y" => axis = 1,
            "WHEEL1" => axis = 2,
            "WHEEL2" => axis = 3,
            "GES_PANN_X" => axis = 4,
            "GES_PANN_Y" => axis = 5,
            "GES_PINCH" => axis = 6,
            "GES_ROTATE" => axis = 7,
            "GES_SWIPE_X" => axis = 8,
            "GES_SWIPE_Y" => axis = 9,
            _ => {}
        }
    }
    modifiers | (axis << 6)
}

pub fn trackball_rotation(current: [f64; 2], previous: [f64; 2], radius: f64) -> (Vector4, f64) {
    if !radius.is_finite() || radius <= 0.0 {
        return (Vector4::ZERO, 0.0);
    }
    let project = |p: [f64; 2]| {
        let (x, y) = (2.0 * p[0] - 1.0, 1.0 - 2.0 * p[1]);
        let d = x.hypot(y);
        let z = if d < radius * std::f64::consts::FRAC_1_SQRT_2 {
            (radius * radius - d * d).sqrt()
        } else {
            radius * radius / (2.0 * d)
        };
        Vector4::new3(x, y, z)
    };
    let (p1, p2) = (project(previous), project(current));
    let axis = p2.cross(p1);
    let length = axis.length();
    if !length.is_finite() || length < 1.0e-4 {
        return (Vector4::ZERO, 0.0);
    }
    let phi = 2.0
        * ((p1 - p2).length() / (2.0 * radius))
            .clamp(-1.0, 1.0)
            .asin();
    if phi < 1.0e-5 {
        return (Vector4::ZERO, 0.0);
    }
    (axis.scale(1.0 / length), phi)
}

fn trackball(camera: &mut Camera, current: [f64; 2], previous: [f64; 2], radius: f64) {
    let (axis, half_angle) = trackball_rotation(current, previous, radius);
    if let Ok(delta) = Quat::from_axis_half_angle(axis, half_angle) {
        camera.set_rotation(
            camera
                .rotation()
                .mul(delta)
                .normalize()
                .unwrap_or(Quat::IDENTITY),
        );
    }
}

fn translation(camera: &Camera, delta: [f64; 3], height: f64) -> Vector4 {
    let scale = if height > 0.0 {
        camera.zoom() / height
    } else {
        0.0
    };
    let mut vector = Vector4::new3(delta[0] * scale, -delta[1] * scale, -delta[2] / 4.0);
    camera.rotation().conj().to_matrix().xform3d(&mut vector);
    vector
}

pub fn apply_action(camera: &mut Camera, action: i32, delta: f64, height: f64) -> bool {
    if !delta.is_finite() {
        return false;
    }
    match action {
        1..=3 => {
            let rotation = match action {
                1 => camera.rotation().rotate_x(delta / 4.0),
                2 => camera.rotation().rotate_y(delta / 4.0),
                _ => camera.rotation().rotate_z(delta / 4.0),
            };
            camera.set_rotation(rotation.unwrap_or(Quat::IDENTITY));
        }
        4 => {
            let zoom = (camera.zoom() * (1.0 + delta / 200.0)).max(1.0e-4);
            if (zoom - camera.zoom()).abs() >= 1.0e-4 {
                camera.set_zoom(zoom);
            }
        }
        5..=7 => {
            let mut vector = [0.0; 3];
            vector[(action - 5) as usize] = delta;
            camera.set_center(camera.center() - translation(camera, vector, height));
        }
        8 => {
            let slab =
                (camera.slab_depth() * (1.0 + delta / 200.0)).clamp(0.1, camera.cam_dist() * 2.0);
            if (slab - camera.slab_depth()).abs() >= 1.0e-4 {
                camera.set_slab_depth(slab);
            }
        }
        _ => return false,
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerRelease {
    None,
    Click,
    DoubleClick,
    Drag,
}

#[derive(Debug, Clone)]
enum Momentum {
    Pan {
        start: Vector4,
        velocity: [f64; 2],
    },
    Rotate {
        start: Quat,
        axis: Vector4,
        spin: f64,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ViewInputController {
    mouse: MouseEventHandler,
    locked_axis: Option<usize>,
    last_click: Option<i64>,
    momentum_kind: i32,
    momentum: Option<(Momentum, i64, f64)>,
    pub translate_momentum: bool,
    pub rotate_momentum: bool,
}

impl ViewInputController {
    pub fn is_dragging(&self) -> bool {
        self.mouse.state() == DragState::Drag
    }
    pub fn is_pressed(&self) -> bool {
        self.mouse.state() != DragState::None
    }
    pub fn is_animating(&self) -> bool {
        self.momentum.is_some()
    }
    pub fn cancel(&mut self) {
        self.mouse.button_up();
        self.momentum = None;
        self.momentum_kind = 0;
        self.locked_axis = None;
    }
    pub fn pointer_down(&mut self, time_ms: i64, point: [f64; 2]) {
        self.momentum = None;
        self.momentum_kind = 0;
        self.mouse
            .button_down(time_ms, point[0].trunc(), point[1].trunc());
    }
    pub fn pointer_move(
        &mut self,
        camera: &mut Camera,
        config: &ViewInputConfig,
        size: [f64; 2],
        time_ms: i64,
        point: [f64; 2],
        modifiers: i32,
    ) -> bool {
        let before = self.mouse.state();
        let point = point.map(f64::trunc);
        let delta = self.mouse.move_to(time_ms, point[0], point[1]);
        if before != DragState::Drag {
            if self.is_dragging() {
                self.locked_axis = None;
            }
            return false;
        }
        self.drag(camera, config, size, point, [delta.0, delta.1], modifiers)
    }
    pub fn drag(
        &mut self,
        camera: &mut Camera,
        config: &ViewInputConfig,
        size: [f64; 2],
        point: [f64; 2],
        delta: [f64; 2],
        modifiers: i32,
    ) -> bool {
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return false;
        }
        let (x, y) = (
            config.find_event(0, modifiers),
            config.find_event(1, modifiers),
        );
        match (x, y) {
            (1, 2) | (2, 1) => {
                let (i, j) = if x == 1 { (0, 1) } else { (1, 0) };
                trackball(
                    camera,
                    [point[i] / size[i], point[j] / size[j]],
                    [
                        (point[i] - delta[i]) / size[i],
                        (point[j] - delta[j]) / size[j],
                    ],
                    config.tbrad(),
                );
                if x == 1 && self.rotate_momentum {
                    self.momentum_kind = 1;
                }
                true
            }
            (5, 6) | (6, 5) => {
                let d = if x == 5 {
                    [delta[0], delta[1], 0.0]
                } else {
                    [delta[1], delta[0], 0.0]
                };
                camera.set_center(camera.center() - translation(camera, d, size[1]));
                if x == 5 && self.translate_momentum {
                    self.momentum_kind = 2;
                }
                true
            }
            (-1, -1) => false,
            _ => {
                let axis = *self
                    .locked_axis
                    .get_or_insert(if delta[0].abs() > delta[1].abs() {
                        0
                    } else {
                        1
                    });
                apply_action(camera, if axis == 0 { x } else { y }, delta[axis], size[1])
            }
        }
    }
    pub fn pointer_up(
        &mut self,
        camera: &Camera,
        config: &ViewInputConfig,
        size: [f64; 2],
        time_ms: i64,
        point: [f64; 2],
    ) -> PointerRelease {
        let (state, velocity) = self
            .mouse
            .release(time_ms, point[0].trunc(), point[1].trunc());
        match state {
            DragState::None => PointerRelease::None,
            DragState::Down => {
                let double = self.last_click.is_some_and(|last| time_ms - last < 500);
                self.last_click = Some(time_ms);
                if double {
                    PointerRelease::DoubleClick
                } else {
                    PointerRelease::Click
                }
            }
            DragState::Drag => {
                if let Some((x, y)) = velocity {
                    self.start_momentum(camera, config, size, time_ms, point, [x, y]);
                }
                PointerRelease::Drag
            }
        }
    }
    pub fn start_momentum(
        &mut self,
        camera: &Camera,
        config: &ViewInputConfig,
        size: [f64; 2],
        time_ms: i64,
        point: [f64; 2],
        velocity: [f64; 2],
    ) {
        let speed = velocity[0].hypot(velocity[1]);
        if !speed.is_finite() || speed < 500.0 || size[0] <= 0.0 || size[1] <= 0.0 {
            return;
        }
        let velocity = velocity.map(|v| v * (10000.0 / speed).min(1.0));
        let (momentum, duration) = match self.momentum_kind {
            1 => {
                let previous = [point[0] / size[0], point[1] / size[1]];
                let current = [
                    previous[0] + velocity[0] / size[0] / 100.0,
                    previous[1] + velocity[1] / size[1] / 100.0,
                ];
                let (axis, half_angle) = trackball_rotation(current, previous, config.tbrad());
                (
                    Momentum::Rotate {
                        start: camera.rotation(),
                        axis,
                        spin: half_angle * 50.0,
                    },
                    speed.min(2000.0),
                )
            }
            2 => (
                Momentum::Pan {
                    start: camera.center(),
                    velocity,
                },
                500.0,
            ),
            _ => return,
        };
        self.momentum = Some((momentum, time_ms, duration));
    }
    pub fn tick(&mut self, camera: &mut Camera, height: f64, time_ms: i64) -> bool {
        let Some((momentum, start_ms, duration)) = &self.momentum else {
            return false;
        };
        let rho = ((time_ms - start_ms) as f64 / duration).clamp(0.0, 1.0);
        let ease = rho * (2.0 - rho);
        match momentum {
            Momentum::Pan { start, velocity } => camera.set_center(
                *start
                    - translation(
                        camera,
                        [velocity[0] * ease * 0.25, velocity[1] * ease * 0.25, 0.0],
                        height,
                    ),
            ),
            Momentum::Rotate { start, axis, spin } => {
                if let Ok(delta) = Quat::from_axis_half_angle(*axis, ease * spin * 0.5) {
                    camera.set_rotation(start.mul(delta));
                }
            }
        }
        if rho >= 1.0 {
            self.momentum = None;
        }
        true
    }
    pub fn wheel(
        &mut self,
        camera: &mut Camera,
        config: &ViewInputConfig,
        height: f64,
        delta: [f64; 2],
        modifiers: i32,
    ) -> bool {
        self.momentum = None;
        apply_action(
            camera,
            config.find_event(2, modifiers),
            delta[0].round() / 2.5,
            height,
        ) | apply_action(
            camera,
            config.find_event(3, modifiers),
            delta[1].round() / 2.5,
            height,
        )
    }
    pub fn gesture(
        &mut self,
        camera: &mut Camera,
        config: &ViewInputConfig,
        height: f64,
        axis: i32,
        delta: f64,
        modifiers: i32,
    ) -> bool {
        self.momentum = None;
        let scale = match axis {
            6 => 3.2,
            7 => -16.0,
            _ => 1.0,
        };
        apply_action(
            camera,
            config.find_event(axis, modifiers),
            (delta * scale).round(),
            height,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputDeviceDetector {
    pub trackpad: bool,
    mouse_streak: usize,
    last_trackpad: Option<i64>,
}
impl InputDeviceDetector {
    pub fn gesture(&mut self, time_ms: i64) {
        self.trackpad = true;
        self.mouse_streak = 0;
        self.last_trackpad = Some(time_ms);
    }
    pub fn wheel(&mut self, time_ms: i64, delta: [f64; 2], mode: u32, is_mac: bool) {
        let trackpad = mode == 0
            && (delta[0] != 0.0 || (is_mac && delta[1] != 0.0 && delta[1].fract() == 0.0));
        if trackpad {
            self.gesture(time_ms);
        } else if mode != 0 || delta[1] != 0.0 {
            self.mouse_streak += 1;
            if self.mouse_streak >= 3 && self.last_trackpad.is_none_or(|t| time_ms - t > 1500) {
                self.trackpad = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ViewInputConfig {
        let mut styles = crate::StyleManager::new();
        styles.load_builtin_default_styles(0);
        let mut config = ViewInputConfig::new();
        config.set_style("DefaultViewInConf");
        config.resolved(&styles)
    }
    #[test]
    fn pointer_threshold_and_second_button_preserve_the_drag_origin() {
        let mut input = ViewInputController::default();
        let config = config();
        let mut camera = Camera::new();
        let original = camera.clone();
        input.pointer_down(0, [200.9, 200.9]);
        input.pointer_move(
            &mut camera,
            &config,
            [800.0, 600.0],
            10,
            [201.9, 201.9],
            LEFT,
        );
        assert!(!input.is_dragging());
        assert_eq!(camera, original);
        input.pointer_move(
            &mut camera,
            &config,
            [800.0, 600.0],
            20,
            [202.1, 201.9],
            LEFT,
        );
        assert!(input.is_dragging());
        assert_eq!(camera, original);
        input.pointer_down(25, [202.1, 201.9]);
        input.pointer_move(
            &mut camera,
            &config,
            [800.0, 600.0],
            30,
            [210.1, 202.9],
            LEFT | RIGHT,
        );
        assert!((camera.center().z - 2.0).abs() < 1.0e-12);
        assert_eq!(camera.rotation(), original.rotation());
        assert_eq!(
            input.pointer_up(&camera, &config, [800.0, 600.0], 40, [210.1, 202.9]),
            PointerRelease::Drag
        );
        input.pointer_move(
            &mut camera,
            &config,
            [800.0, 600.0],
            50,
            [300.0, 300.0],
            LEFT,
        );
        assert!((camera.center().z - 2.0).abs() < 1.0e-12);
    }
    #[test]
    fn shift_drag_locks_the_first_motion_axis() {
        let mut input = ViewInputController::default();
        let config = config();
        let mut camera = Camera::new();
        input.drag(
            &mut camera,
            &config,
            [800.0, 600.0],
            [210.0, 201.0],
            [10.0, 1.0],
            LEFT | SHIFT,
        );
        input.drag(
            &mut camera,
            &config,
            [800.0, 600.0],
            [211.0, 301.0],
            [1.0, 100.0],
            LEFT | SHIFT,
        );
        assert_eq!(camera.zoom(), 50.0);
        assert!((camera.slab_depth() - 50.0 * 1.05 * 1.005).abs() < 1.0e-12);
        input.cancel();
        input.drag(
            &mut camera,
            &config,
            [800.0, 600.0],
            [211.0, 321.0],
            [0.0, 20.0],
            LEFT | SHIFT,
        );
        assert!((camera.zoom() - 55.0).abs() < 1.0e-12);
        assert_eq!(camera.cam_dist(), 200.0);
    }
    #[test]
    fn exact_modifiers_and_custom_bindings_replace_previous_assignments() {
        let mut config = config();
        assert_eq!(config.find_event(2, 0), 4);
        assert_eq!(config.find_event(2, SHIFT), -1);
        config.set_conf_slab("WHEEL1");
        assert_eq!(config.find_event(2, 0), 8);
        assert_eq!(config.find_event(3, 0), 4);
        config.set_conf_slab("");
        assert_eq!(config.find_event(2, 0), -1);
        config.set_conf_rotx(" alt | lbutton | mouse_y ");
        assert_eq!(config.find_event(1, ALT | LEFT), 1);
    }
    #[test]
    fn release_velocity_drives_momentum_and_new_input_cancels_it() {
        let mut input = ViewInputController {
            translate_momentum: true,
            ..Default::default()
        };
        let config = config();
        let mut camera = Camera::new();
        input.pointer_down(0, [100.0, 100.0]);
        for time in [10, 20, 30, 40] {
            input.pointer_move(
                &mut camera,
                &config,
                [800.0, 600.0],
                time,
                [100.0 + time as f64, 100.0],
                RIGHT,
            );
        }
        input.pointer_up(&camera, &config, [800.0, 600.0], 50, [150.0, 100.0]);
        assert!(input.is_animating());
        let center = camera.center();
        input.tick(&mut camera, 600.0, 550);
        assert!((camera.center().x - center.x + 1000.0 * 0.25 * 50.0 / 600.0).abs() < 1.0e-10);
        assert!(!input.is_animating());
        input.start_momentum(
            &camera,
            &config,
            [800.0, 600.0],
            600,
            [150.0, 100.0],
            [1000.0, 0.0],
        );
        assert!(input.is_animating());
        input.wheel(&mut camera, &config, 600.0, [0.0, -120.0], 0);
        assert!(!input.is_animating());
        assert_eq!(camera.cam_dist(), 200.0);
    }
    #[test]
    fn click_timing_and_trackpad_detection_follow_reference_hysteresis() {
        let mut input = ViewInputController::default();
        let camera = Camera::new();
        let config = config();
        for (time, expected) in [
            (1000, PointerRelease::Click),
            (1400, PointerRelease::DoubleClick),
            (1900, PointerRelease::Click),
        ] {
            input.pointer_down(time, [10.0, 10.0]);
            assert_eq!(
                input.pointer_up(&camera, &config, [800.0, 600.0], time + 1, [10.0, 10.0]),
                expected
            );
        }
        let mut device = InputDeviceDetector::default();
        device.wheel(0, [0.0, 4.0], 0, true);
        assert!(device.trackpad);
        for time in [10, 20, 30] {
            device.wheel(time, [0.0, 40.5], 0, true);
        }
        assert!(device.trackpad);
        device.wheel(1600, [0.0, 40.5], 0, true);
        assert!(!device.trackpad);
        device.wheel(1700, [1.0, 20.0], 0, false);
        assert!(device.trackpad);
        for time in [3400, 3410, 3420] {
            device.wheel(time, [0.0, 3.0], 1, false);
        }
        assert!(!device.trackpad);
        device.gesture(3430);
        assert!(device.trackpad);
    }
}
