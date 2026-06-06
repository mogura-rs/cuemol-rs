use crate::{Camera, CenterMark, DrawObj, StereoMode};
use cuemol_core::{Quat, Uid, Vector4};
use std::collections::BTreeMap;

const F_EPS4: f64 = 1.0e-4;

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    uid: Uid,
    scene_uid: Uid,
    name: String,
    active: bool,
    zoom: f64,
    view_dist: f64,
    slab_depth: f64,
    rot_quat: Quat,
    center: Vector4,
    perspective: bool,
    trans_mms: bool,
    rot_mms: bool,
    stereo_mode: StereoMode,
    stereo_distance: f64,
    swap_stereo_eyes: bool,
    center_mark: CenterMark,
    width: u32,
    height: u32,
    scale_factor: (f64, f64),
    projection_changed: bool,
    style_names: String,
    draw_objs: BTreeMap<String, DrawObj>,
}

impl View {
    pub fn new(uid: Uid, scene_uid: Uid) -> Self {
        Self {
            uid,
            scene_uid,
            name: String::new(),
            active: false,
            zoom: 50.0,
            view_dist: 200.0,
            slab_depth: 50.0,
            rot_quat: Quat::IDENTITY,
            center: Vector4::ZERO,
            perspective: false,
            trans_mms: false,
            rot_mms: false,
            stereo_mode: StereoMode::None,
            stereo_distance: 1.0,
            swap_stereo_eyes: false,
            center_mark: CenterMark::Cross,
            width: 0,
            height: 0,
            scale_factor: (1.0, 1.0),
            projection_changed: false,
            style_names: String::new(),
            draw_objs: BTreeMap::new(),
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn scene_uid(&self) -> Uid {
        self.scene_uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn active(&self) -> bool {
        self.is_active()
    }

    pub fn set_active(&mut self, value: bool) {
        self.active = value;
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn get_zoom(&self) -> f64 {
        self.zoom()
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.max(F_EPS4);
    }

    pub fn view_dist(&self) -> f64 {
        self.view_dist
    }

    pub fn set_view_dist(&mut self, view_dist: f64) {
        self.view_dist = view_dist.clamp(0.1, 10000.0);
        self.slab_depth = self.slab_depth.min(self.view_dist * 2.0).max(0.1);
    }

    pub fn slab_depth(&self) -> f64 {
        self.slab_depth
    }

    pub fn slab(&self) -> f64 {
        self.slab_depth()
    }

    pub fn get_slab_depth(&self) -> f64 {
        self.slab_depth()
    }

    pub fn set_slab_depth(&mut self, slab_depth: f64) {
        self.slab_depth = slab_depth.clamp(0.1, self.view_dist * 2.0);
    }

    pub fn rot_quat(&self) -> Quat {
        self.rot_quat
    }

    pub fn get_rot_quat(&self) -> Quat {
        self.rot_quat()
    }

    pub fn set_rot_quat(&mut self, rot_quat: Quat) {
        self.rot_quat = rot_quat.normalize().unwrap_or(rot_quat);
    }

    pub fn rotate_view(&mut self, x_deg: f64, y_deg: f64, z_deg: f64) {
        if x_deg == 0.0 && y_deg == 0.0 && z_deg == 0.0 {
            return;
        }
        let qx = axis_angle(Vector4::new3(1.0, 0.0, 0.0), x_deg);
        let qy = axis_angle(Vector4::new3(0.0, 1.0, 0.0), y_deg);
        let qz = axis_angle(Vector4::new3(0.0, 0.0, 1.0), z_deg);
        self.set_rot_quat(mul_quat(mul_quat(mul_quat(qz, qy), qx), self.rot_quat));
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn view_center(&self) -> Vector4 {
        self.center()
    }

    pub fn get_view_center(&self) -> Vector4 {
        self.center()
    }

    pub fn set_center(&mut self, center: Vector4) {
        self.center = center;
    }

    pub fn set_view_center(&mut self, center: Vector4) {
        self.set_center(center);
    }

    pub fn translate_view(&mut self, dx: f64, dy: f64, dz: f64) {
        let delta = self
            .right_vector()
            .scale(dx)
            .add_vec(self.up_vector().scale(dy))
            .add_vec(self.forward_vector().scale(dz));
        self.center = self.center.add_vec(delta);
    }

    pub fn is_perspective(&self) -> bool {
        self.perspective
    }

    pub fn is_perspec(&self) -> bool {
        self.is_perspective()
    }

    pub fn perspec(&self) -> bool {
        self.is_perspective()
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.perspective = value;
    }

    pub fn set_perspec(&mut self, value: bool) {
        self.set_perspective(value);
    }

    pub fn is_trans_mms(&self) -> bool {
        self.trans_mms
    }

    pub fn trans_mms(&self) -> bool {
        self.is_trans_mms()
    }

    pub fn set_trans_mms(&mut self, value: bool) {
        self.trans_mms = value;
    }

    pub fn is_rot_mms(&self) -> bool {
        self.rot_mms
    }

    pub fn rot_mms(&self) -> bool {
        self.is_rot_mms()
    }

    pub fn set_rot_mms(&mut self, value: bool) {
        self.rot_mms = value;
    }

    pub fn stereo_mode(&self) -> StereoMode {
        self.stereo_mode
    }

    pub fn set_stereo_mode(&mut self, value: StereoMode) {
        self.stereo_mode = value;
    }

    pub fn has_hw_stereo(&self) -> bool {
        false
    }

    pub fn has_hwstereo(&self) -> bool {
        self.has_hw_stereo()
    }

    pub fn stereo_distance(&self) -> f64 {
        self.stereo_distance
    }

    pub fn stereo_dist(&self) -> f64 {
        self.stereo_distance()
    }

    pub fn set_stereo_dist(&mut self, value: f64) {
        self.set_stereo_distance(value);
    }

    pub fn set_stereo_distance(&mut self, value: f64) {
        self.stereo_distance = value;
    }

    pub fn is_swap_stereo_eyes(&self) -> bool {
        self.swap_stereo_eyes
    }

    pub fn swap_stereo_eyes(&self) -> bool {
        self.is_swap_stereo_eyes()
    }

    pub fn set_swap_stereo_eyes(&mut self, value: bool) {
        self.swap_stereo_eyes = value;
    }

    pub fn center_mark(&self) -> CenterMark {
        self.center_mark
    }

    pub fn set_center_mark(&mut self, value: CenterMark) {
        self.center_mark = value;
    }

    pub fn camera(&self) -> Camera {
        let mut camera = Camera::new();
        camera.set_name(self.name.clone());
        camera.set_zoom(self.zoom);
        camera.set_cam_dist(self.view_dist);
        camera.set_slab_depth(self.slab_depth);
        camera.set_center(self.center);
        camera.set_rotation(self.rot_quat);
        camera.set_perspective(self.perspective);
        camera.set_stereo_mode(self.stereo_mode);
        camera.set_stereo_distance(self.stereo_distance);
        camera.set_center_mark(self.center_mark);
        camera
    }

    pub fn get_camera(&self) -> Camera {
        self.camera()
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.name = camera.name().to_string();
        self.zoom = camera.zoom();
        self.view_dist = camera.cam_dist();
        self.slab_depth = camera.slab_depth();
        self.center = camera.center();
        self.rot_quat = camera.rotation();
        self.perspective = camera.is_perspective();
        self.stereo_mode = camera.stereo_mode();
        self.stereo_distance = camera.stereo_distance();
        self.center_mark = camera.center_mark();
        self.set_projection_changed();
    }

    pub fn up_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(0.0, 1.0, 0.0))
    }

    pub fn up_vec(&self) -> Vector4 {
        self.up_vector()
    }

    pub fn right_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(1.0, 0.0, 0.0))
    }

    pub fn right_vec(&self) -> Vector4 {
        self.right_vector()
    }

    pub fn forward_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(0.0, 0.0, -1.0))
    }

    pub fn forward_vec(&self) -> Vector4 {
        self.forward_vector()
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn get_width(&self) -> u32 {
        self.width()
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn get_height(&self) -> u32 {
        self.height()
    }

    pub fn set_view_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.set_projection_changed();
    }

    pub fn size_changed(&mut self, width: u32, height: u32) {
        self.set_view_size(width, height);
    }

    pub fn set_scl_fac(&mut self, x: f64, y: f64) {
        self.scale_factor = (x.max(F_EPS4), y.max(F_EPS4));
        self.set_projection_changed();
    }

    pub fn scale_factor(&self) -> (f64, f64) {
        self.scale_factor
    }

    pub fn is_projection_changed(&self) -> bool {
        self.projection_changed
    }

    pub fn set_projection_changed(&mut self) {
        self.projection_changed = true;
    }

    pub fn invalidate(&mut self) {
        self.set_projection_changed();
    }

    pub fn redraw(&mut self) {
        self.set_projection_changed();
    }

    pub fn check_and_update(&mut self) {
        self.reset_projection_changed();
    }

    pub fn reset_projection_changed(&mut self) {
        self.projection_changed = false;
    }

    pub fn style_names(&self) -> &str {
        &self.style_names
    }

    pub fn get_style_names(&self) -> &str {
        self.style_names()
    }

    pub fn apply_styles(&mut self, names: impl Into<String>) {
        self.style_names = names.into();
    }

    pub fn push_style(&mut self, names: &str) -> bool {
        if names.trim().is_empty() {
            return false;
        }
        if self.style_names.is_empty() {
            self.style_names = names.to_string();
        } else {
            self.style_names.push(',');
            self.style_names.push_str(names);
        }
        true
    }

    pub fn remove_style_regex(&mut self, regex: &str) -> bool {
        let before = self.style_names.clone();
        self.style_names = self
            .style_names
            .split(',')
            .filter(|name| !name.contains(regex))
            .collect::<Vec<_>>()
            .join(",");
        before != self.style_names
    }

    pub fn translate_view_drag(&mut self, dx: f64, dy: f64, dz: f64) {
        self.translate_view(dx, dy, dz);
    }

    pub fn hit_test(&self, _x: i32, _y: i32) -> String {
        String::new()
    }

    pub fn hit_test_rect(&self, _x: i32, _y: i32, _w: i32, _h: i32, _nearest: bool) -> String {
        String::new()
    }

    pub fn get_draw_obj(&mut self, class_name: &str) -> Option<&mut DrawObj> {
        if class_name.trim().is_empty() {
            return None;
        }
        Some(self.draw_objs.entry(class_name.to_string()).or_default())
    }

    pub fn draw_obj_count(&self) -> usize {
        self.draw_objs.len()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_mouse_down(
        &mut self,
        _client_x: f64,
        _client_y: f64,
        _screen_x: f64,
        _screen_y: f64,
        _modifier: i32,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_mouse_up(
        &mut self,
        _client_x: f64,
        _client_y: f64,
        _screen_x: f64,
        _screen_y: f64,
        _modifier: i32,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_mouse_move(
        &mut self,
        _client_x: f64,
        _client_y: f64,
        _screen_x: f64,
        _screen_y: f64,
        _modifier: i32,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_wheel(
        &mut self,
        _client_x: f64,
        _client_y: f64,
        _screen_x: f64,
        _screen_y: f64,
        _modifier: i32,
        _delta_x: f64,
        _delta_y: f64,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_gesture(
        &mut self,
        _client_x: f64,
        _client_y: f64,
        _screen_x: f64,
        _screen_y: f64,
        _modifier: i32,
        _axis_id: i32,
        _delta: f64,
    ) {
    }
}

fn axis_angle(axis: Vector4, degrees: f64) -> Quat {
    let half = degrees.to_radians() * 0.5;
    let sin_half = half.sin();
    Quat::new(
        axis.x * sin_half,
        axis.y * sin_half,
        axis.z * sin_half,
        half.cos(),
    )
}

fn mul_quat(lhs: Quat, rhs: Quat) -> Quat {
    Quat::new(
        lhs.w * rhs.x + lhs.x * rhs.w + lhs.y * rhs.z - lhs.z * rhs.y,
        lhs.w * rhs.y - lhs.x * rhs.z + lhs.y * rhs.w + lhs.z * rhs.x,
        lhs.w * rhs.z + lhs.x * rhs.y - lhs.y * rhs.x + lhs.z * rhs.w,
        lhs.w * rhs.w - lhs.x * rhs.x - lhs.y * rhs.y - lhs.z * rhs.z,
    )
}

fn conjugate(value: Quat) -> Quat {
    Quat::new(-value.x, -value.y, -value.z, value.w)
}

fn rotate_vector(rotation: Quat, value: Vector4) -> Vector4 {
    let qv = Quat::new(value.x, value.y, value.z, 0.0);
    let rotated = mul_quat(mul_quat(rotation, qv), conjugate(rotation));
    Vector4::new3(rotated.x, rotated.y, rotated.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sqlen(quat: Quat) -> f64 {
        quat.x * quat.x + quat.y * quat.y + quat.z * quat.z + quat.w * quat.w
    }

    #[test]
    fn view_clamps_and_normalizes_reference_values() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_zoom(0.0);
        assert!(view.zoom() > 0.0);
        view.set_view_dist(0.0);
        assert!(view.view_dist() >= 0.1);
        view.set_view_dist(99999.0);
        assert!(view.view_dist() <= 10000.0);
        view.set_slab_depth(0.0);
        assert!(view.slab_depth() >= 0.1);

        view.set_rot_quat(Quat::new(2.0, 0.0, 0.0, 0.0));
        assert!((sqlen(view.rot_quat()) - 1.0).abs() < 1.0e-6);
        view.set_rot_quat(Quat::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(sqlen(view.rot_quat()), 0.0);
    }

    #[test]
    fn view_vectors_and_projection_flags_match_reference_shape() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        assert_eq!(view.name(), "");
        assert!(!view.is_active());
        assert!(!view.is_perspective());
        assert_eq!(view.stereo_mode(), StereoMode::None);
        assert_eq!(view.center_mark(), CenterMark::Cross);
        assert!((view.up_vector().length() - 1.0).abs() < 1.0e-6);
        assert!((view.right_vector().length() - 1.0).abs() < 1.0e-6);
        assert!((view.forward_vector().length() - 1.0).abs() < 1.0e-6);
        assert!(view.up_vector().dot(view.right_vector()).abs() < 1.0e-6);

        let before = view.rot_quat();
        view.rotate_view(45.0, 0.0, 0.0);
        assert_ne!(view.rot_quat(), before);
        assert!((sqlen(view.rot_quat()) - 1.0).abs() < 1.0e-6);

        view.reset_projection_changed();
        view.set_view_size(320, 240);
        assert!(view.is_projection_changed());
        assert_eq!(view.width(), 320);
        assert_eq!(view.height(), 240);
        view.reset_projection_changed();
        view.set_scl_fac(2.0, 3.0);
        assert_eq!(view.scale_factor(), (2.0, 3.0));
        assert!(view.is_projection_changed());

        view.set_name("0");
        view.set_active(true);
        view.push_style("default");
        assert_eq!(view.name(), "0");
        assert!(view.is_active());
        assert_eq!(view.style_names(), "default");
    }

    #[test]
    fn view_round_trips_camera_state() {
        let mut camera = Camera::new();
        camera.set_name("__current");
        camera.set_zoom(75.0);
        camera.set_cam_dist(300.0);
        camera.set_slab_depth(90.0);
        camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
        camera.set_perspective(true);
        camera.set_stereo_mode(StereoMode::Cross);

        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_camera(&camera);
        assert_eq!(view.name(), "__current");
        assert_eq!(view.zoom(), 75.0);
        assert_eq!(view.view_dist(), 300.0);
        assert_eq!(view.slab_depth(), 90.0);
        assert_eq!(view.center(), Vector4::new3(1.0, 2.0, 3.0));
        assert!(view.is_perspective());
        assert_eq!(view.stereo_mode(), StereoMode::Cross);

        let out = view.camera();
        assert_eq!(out.name(), "__current");
        assert_eq!(out.zoom(), 75.0);
        assert_eq!(out.cam_dist(), 300.0);
    }

    #[test]
    fn view_draw_object_lookup_is_stateful() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        assert_eq!(view.draw_obj_count(), 0);
        let draw = view.get_draw_obj("CenterMarkDrawObj").unwrap();
        assert!(!draw.is_enabled());
        draw.set_enabled(true);
        assert_eq!(view.draw_obj_count(), 1);
        assert!(view.get_draw_obj("").is_none());
        assert!(view.get_draw_obj("CenterMarkDrawObj").unwrap().is_enabled());
    }
}
