use crate::{
    Camera, CenterMark, DragState, DrawObj, FrameRenderParams, InDevEvent, MouseEventHandler,
    ScrEventManagerHandle, StereoMode, ViewEvent,
};
use cuemol_core::{EventCaster, LRegExpr, Matrix4, Quat, SharedEventListener, Uid, Vector4};
use cuemol_gfx::{
    jitter_offset, jitter_sample_count, AoConstants, HitData, HittestContext, RawHitData,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

const F_EPS4: f64 = 1.0e-4;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewHitCandidate {
    pub x: f64,
    pub y: f64,
    pub depth: f64,
    pub renderer_id: Uid,
    pub renderer_type: String,
    pub renderer_name: String,
    pub object_id: Uid,
    pub object_name: String,
    pub names: Vec<i32>,
    pub hit_fragment: String,
}

impl ViewHitCandidate {
    pub fn with_hit_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.hit_fragment = fragment.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    uid: Uid,
    scene_uid: Uid,
    name: String,
    camera_template: Camera,
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
    event_caster: Arc<EventCaster<ViewEvent>>,
    input_caster: Arc<EventCaster<InDevEvent>>,
    mouse_handler: MouseEventHandler,
    event_manager: Option<ScrEventManagerHandle>,
    hit_precision: f64,
    hit_candidates: Vec<ViewHitCandidate>,
    jitter_sample_index: i32,
    jitter_more_samples: bool,
    jitter_reset_requested: bool,
    jitter_offset_pixels: (f64, f64),
    ao_half_pending: bool,
}

impl View {
    pub fn apply_undo_changes(&mut self, from: &Self, to: &Self) {
        use cuemol_core::snapshot::{apply_map_changes, apply_value_change};
        apply_value_change(&mut self.name, &from.name, &to.name);
        apply_value_change(&mut self.active, &from.active, &to.active);
        apply_value_change(&mut self.zoom, &from.zoom, &to.zoom);
        apply_value_change(&mut self.view_dist, &from.view_dist, &to.view_dist);
        apply_value_change(&mut self.slab_depth, &from.slab_depth, &to.slab_depth);
        apply_value_change(&mut self.rot_quat, &from.rot_quat, &to.rot_quat);
        apply_value_change(&mut self.center, &from.center, &to.center);
        apply_value_change(&mut self.perspective, &from.perspective, &to.perspective);
        apply_value_change(&mut self.trans_mms, &from.trans_mms, &to.trans_mms);
        apply_value_change(&mut self.rot_mms, &from.rot_mms, &to.rot_mms);
        apply_value_change(&mut self.stereo_mode, &from.stereo_mode, &to.stereo_mode);
        apply_value_change(
            &mut self.stereo_distance,
            &from.stereo_distance,
            &to.stereo_distance,
        );
        apply_value_change(
            &mut self.swap_stereo_eyes,
            &from.swap_stereo_eyes,
            &to.swap_stereo_eyes,
        );
        apply_value_change(&mut self.center_mark, &from.center_mark, &to.center_mark);
        apply_value_change(&mut self.width, &from.width, &to.width);
        apply_value_change(&mut self.height, &from.height, &to.height);
        apply_value_change(&mut self.scale_factor, &from.scale_factor, &to.scale_factor);
        apply_value_change(&mut self.style_names, &from.style_names, &to.style_names);
        apply_value_change(
            &mut self.hit_precision,
            &from.hit_precision,
            &to.hit_precision,
        );
        apply_map_changes(&mut self.draw_objs, &from.draw_objs, &to.draw_objs);
        self.camera_template
            .apply_undo_changes(&from.camera_template, &to.camera_template);
        self.projection_changed = true;
        self.jitter_reset_requested = true;
    }

    pub fn new(uid: Uid, scene_uid: Uid) -> Self {
        Self {
            uid,
            scene_uid,
            name: String::new(),
            camera_template: Camera::new(),
            active: true,
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
            width: 100,
            height: 100,
            scale_factor: (1.0, 1.0),
            projection_changed: true,
            style_names: String::new(),
            draw_objs: BTreeMap::new(),
            event_caster: Arc::new(EventCaster::new()),
            input_caster: Arc::new(EventCaster::new()),
            mouse_handler: MouseEventHandler::new(),
            event_manager: None,
            hit_precision: 10.0,
            hit_candidates: Vec::new(),
            jitter_sample_index: 0,
            jitter_more_samples: false,
            jitter_reset_requested: false,
            jitter_offset_pixels: (0.0, 0.0),
            ao_half_pending: false,
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

    pub fn get_scene(&self) -> Uid {
        self.scene_uid()
    }

    pub fn attach_event_manager(&mut self, manager: ScrEventManagerHandle) {
        self.event_manager = Some(manager);
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.fire_property_changed("name", false);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn active(&self) -> bool {
        self.is_active()
    }

    pub fn set_active(&mut self, value: bool) {
        if self.active == value {
            return;
        }
        self.active = value;
        let mut event = ViewEvent::new();
        event.set_event_type(ViewEvent::VWE_ACTIVATED);
        self.fire_view_event(&mut event);
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn get_zoom(&self) -> f64 {
        self.zoom()
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        let zoom = zoom.max(F_EPS4);
        if (self.zoom - zoom).abs() < F_EPS4 {
            return;
        }
        self.zoom = zoom;
        self.set_projection_changed();
        self.fire_property_changed("zoom", false);
    }

    pub fn view_dist(&self) -> f64 {
        self.view_dist
    }

    pub fn get_view_dist(&self) -> f64 {
        self.view_dist()
    }

    pub fn set_view_dist(&mut self, view_dist: f64) {
        let view_dist = view_dist.clamp(0.1, 10000.0);
        if (self.view_dist - view_dist).abs() < F_EPS4 {
            return;
        }
        self.view_dist = view_dist;
        self.slab_depth = self.slab_depth.min(self.view_dist * 2.0).max(0.1);
        self.set_projection_changed();
        self.fire_property_changed("distance", false);
    }

    pub fn distance(&self) -> f64 {
        self.view_dist()
    }

    pub fn set_distance(&mut self, view_dist: f64) {
        self.set_view_dist(view_dist);
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
        let slab_depth = slab_depth.clamp(0.1, self.view_dist * 2.0);
        if (self.slab_depth - slab_depth).abs() < F_EPS4 {
            return;
        }
        self.slab_depth = slab_depth;
        self.set_projection_changed();
        self.fire_property_changed("slab", false);
    }

    pub fn set_slab(&mut self, slab_depth: f64) {
        self.set_slab_depth(slab_depth);
    }

    pub fn rot_quat(&self) -> Quat {
        self.rot_quat
    }

    pub fn get_rot_quat(&self) -> Quat {
        self.rot_quat()
    }

    pub fn get_rot_quat_scr(&self) -> Quat {
        self.rot_quat()
    }

    pub fn set_rot_quat(&mut self, rot_quat: Quat) {
        self.rot_quat = rot_quat.normalize().unwrap_or(rot_quat);
        self.fire_property_changed("rotation", false);
    }

    pub fn rotation(&self) -> Quat {
        self.rot_quat()
    }

    pub fn set_rotation(&mut self, rot_quat: Quat) {
        self.set_rot_quat(rot_quat);
    }

    pub fn rotate_view(&mut self, x_deg: f64, y_deg: f64, z_deg: f64) {
        if x_deg == 0.0 && y_deg == 0.0 && z_deg == 0.0 {
            return;
        }
        let qx = axis_angle(Vector4::new3(1.0, 0.0, 0.0), x_deg);
        let qy = axis_angle(Vector4::new3(0.0, 1.0, 0.0), y_deg);
        let qz = axis_angle(Vector4::new3(0.0, 0.0, 1.0), z_deg);
        self.set_rot_quat(mul_quat(mul_quat(mul_quat(self.rot_quat, qx), qy), qz));
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

    pub fn get_view_center_scr(&self) -> Vector4 {
        self.center()
    }

    pub fn set_center(&mut self, center: Vector4) {
        self.center = center;
        self.fire_property_changed("center", false);
    }

    pub fn set_view_center(&mut self, center: Vector4) {
        self.set_center(center);
    }

    pub fn translate_view(&mut self, dx: f64, dy: f64, dz: f64) {
        self.set_center(self.translated_center(dx, dy, dz));
    }

    pub fn is_perspective(&self) -> bool {
        self.perspective
    }

    pub fn perspective(&self) -> bool {
        self.is_perspective()
    }

    pub fn is_perspec(&self) -> bool {
        self.is_perspective()
    }

    pub fn perspec(&self) -> bool {
        self.is_perspective()
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.perspective = value;
        self.set_projection_changed();
        self.fire_property_changed("perspec", false);
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
        self.fire_property_changed("trans_mms", false);
    }

    pub fn is_rot_mms(&self) -> bool {
        self.rot_mms
    }

    pub fn rot_mms(&self) -> bool {
        self.is_rot_mms()
    }

    pub fn set_rot_mms(&mut self, value: bool) {
        self.rot_mms = value;
        self.fire_property_changed("rot_mms", false);
    }

    pub fn stereo_mode(&self) -> StereoMode {
        self.stereo_mode
    }

    pub fn get_stereo_mode(&self) -> StereoMode {
        self.stereo_mode()
    }

    pub fn set_stereo_mode(&mut self, value: StereoMode) {
        self.stereo_mode = value;
        self.set_projection_changed();
        self.fire_property_changed("stereo_mode", false);
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

    pub fn get_stereo_dist(&self) -> f64 {
        self.stereo_distance()
    }

    pub fn set_stereo_dist(&mut self, value: f64) {
        self.set_stereo_distance(value);
    }

    pub fn set_stereo_distance(&mut self, value: f64) {
        self.stereo_distance = value;
        self.set_projection_changed();
        self.fire_property_changed("stereo_dist", false);
    }

    pub fn is_swap_stereo_eyes(&self) -> bool {
        self.swap_stereo_eyes
    }

    pub fn swap_stereo_eyes(&self) -> bool {
        self.is_swap_stereo_eyes()
    }

    pub fn set_swap_stereo_eyes(&mut self, value: bool) {
        self.swap_stereo_eyes = value;
        self.set_projection_changed();
        self.fire_property_changed("swap_stereo_eyes", false);
    }

    pub fn center_mark(&self) -> CenterMark {
        self.center_mark
    }

    pub fn get_center_mark(&self) -> CenterMark {
        self.center_mark()
    }

    pub fn set_center_mark(&mut self, value: CenterMark) {
        self.center_mark = value;
        self.fire_property_changed("center_mark", false);
    }

    pub fn camera(&self) -> Camera {
        let mut camera = self.camera_template.clone();
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

    pub fn fit_positions(&mut self, positions: impl IntoIterator<Item = Vector4>) -> bool {
        let mut camera = self.camera();
        let aspect = if self.width() > 0 && self.height() > 0 {
            f64::from(self.width()) / f64::from(self.height())
        } else {
            1.0
        };
        if !camera.fit_positions(positions, aspect) {
            return false;
        }
        self.set_view_center(camera.center());
        self.set_zoom(camera.zoom());
        self.set_slab_depth(camera.slab_depth());
        true
    }

    pub fn get_camera(&self) -> Camera {
        self.camera()
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.camera_template = camera.clone();
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
        self.fire_property_changed("setCamera", false);
    }

    pub fn up_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(0.0, 1.0, 0.0))
    }

    pub fn get_up_vector(&self) -> Vector4 {
        self.up_vector()
    }

    pub fn up_vec(&self) -> Vector4 {
        self.up_vector()
    }

    pub fn right_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(1.0, 0.0, 0.0))
    }

    pub fn get_right_vector(&self) -> Vector4 {
        self.right_vector()
    }

    pub fn right_vec(&self) -> Vector4 {
        self.right_vector()
    }

    pub fn forward_vector(&self) -> Vector4 {
        rotate_vector(self.rot_quat, Vector4::new3(0.0, 0.0, -1.0))
    }

    pub fn get_forward_vector(&self) -> Vector4 {
        self.forward_vector()
    }

    pub fn forward_vec(&self) -> Vector4 {
        self.forward_vector()
    }

    pub fn proj_to_screen(&self, world_position: Vector4) -> Vector4 {
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let projection = self.projection_matrix();
        let mut model_view = Matrix4::IDENTITY;
        model_view.translate(Vector4::new3(0.0, 0.0, -self.view_dist));
        model_view.rotate(self.rot_quat);
        model_view.translate(self.center.negate());

        let point = Vector4::new4(world_position.x, world_position.y, world_position.z, 1.0);
        let clip = projection.mul_vec(model_view.mul_vec(point));
        let clip_w = if clip.w.abs() < 1.0e-8 {
            if clip.w < 0.0 {
                -1.0e-8
            } else {
                1.0e-8
            }
        } else {
            clip.w
        };
        let ndc_x = clip.x / clip_w;
        let ndc_y = clip.y / clip_w;
        let ndc_z = clip.z / clip_w;
        Vector4::new3(
            (ndc_x * 0.5 + 0.5) * width,
            (0.5 - ndc_y * 0.5) * height,
            ndc_z,
        )
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
        let mut event = ViewEvent::new();
        event.set_event_type(ViewEvent::VWE_SIZECHG);
        self.fire_view_event(&mut event);
    }

    pub fn set_scl_fac(&mut self, x: f64, y: f64) {
        self.scale_factor = (x.max(F_EPS4), y.max(F_EPS4));
        self.set_projection_changed();
    }

    pub fn scale_factor(&self) -> (f64, f64) {
        self.scale_factor
    }

    pub fn backing_width(&self) -> u32 {
        backing_dimension(self.width, self.scale_factor.0)
    }

    pub fn backing_height(&self) -> u32 {
        backing_dimension(self.height, self.scale_factor.1)
    }

    pub fn projection_matrix(&self) -> Matrix4 {
        let slab_depth = self.slab_depth.max(0.1);
        let distance = self.view_dist;
        let slab_near = (distance - slab_depth / 2.0).max(0.1);
        let slab_far = distance + slab_depth;
        let view_width = self.zoom / 2.0;
        let height = f64::from(self.height);
        let mut aspect = if height > 0.0 {
            f64::from(self.width) / height
        } else {
            1.0
        };
        if matches!(self.stereo_mode, StereoMode::Parallel | StereoMode::Cross) {
            aspect /= 2.0;
        }
        let mut projection = if self.perspective {
            perspective_projection(view_width, aspect, slab_near, slab_far, distance)
        } else {
            orthographic_projection(view_width, aspect, slab_near, slab_far)
        };

        let backing_width = f64::from(self.backing_width());
        let backing_height = f64::from(self.backing_height());
        let jitter_x = if backing_width > 0.0 {
            2.0 * self.jitter_offset_pixels.0 / backing_width
        } else {
            0.0
        };
        let jitter_y = if backing_height > 0.0 {
            2.0 * self.jitter_offset_pixels.1 / backing_height
        } else {
            0.0
        };
        if self.perspective {
            projection.values[0][2] -= jitter_x;
            projection.values[1][2] -= jitter_y;
        } else {
            projection.values[0][3] += jitter_x;
            projection.values[1][3] += jitter_y;
        }
        projection
    }

    pub fn set_jitter_offset_pixels(&mut self, x: f64, y: f64) {
        self.jitter_offset_pixels = (x, y);
    }

    pub fn jitter_offset_pixels(&self) -> (f64, f64) {
        self.jitter_offset_pixels
    }

    pub fn compute_ao_constants(&self) -> AoConstants {
        let aspect = if self.height > 0 {
            f64::from(self.width) / f64::from(self.height)
        } else {
            1.0
        };
        AoConstants::from_camera(
            self.view_dist,
            self.zoom,
            self.slab_depth,
            aspect,
            self.backing_width() as i32,
            self.backing_height() as i32,
            self.perspective,
        )
    }

    pub fn prepare_frame_render(
        &mut self,
        jitter_level: i32,
        externally_updated: bool,
        ao_half_resolution: bool,
    ) -> FrameRenderParams {
        self.jitter_more_samples = false;
        self.jitter_offset_pixels = (0.0, 0.0);
        self.ao_half_pending = ao_half_resolution;

        let jitter_level = jitter_level.clamp(0, 5);
        let jitter_active = jitter_level > 0;
        let jitter_count = jitter_sample_count(jitter_level) as i32;
        if jitter_active {
            if externally_updated
                || self.jitter_reset_requested
                || self.jitter_sample_index >= jitter_count
            {
                self.jitter_sample_index = 0;
            }
            self.jitter_reset_requested = false;
            self.jitter_offset_pixels = jitter_offset(jitter_level, self.jitter_sample_index);
        }

        let ao_noise_offset = if jitter_active {
            let value = f64::from(self.jitter_sample_index) * 0.618_033_988_7;
            value.fract() as f32
        } else {
            0.0
        };
        FrameRenderParams {
            camera_ao_constants: self.compute_ao_constants(),
            ao_noise_offset,
            jitter_active,
            jitter_index: self.jitter_sample_index,
            jitter_count,
            blit_depth_to_default: true,
            ..FrameRenderParams::default()
        }
    }

    pub fn complete_frame_render(&mut self, params: &FrameRenderParams) {
        if !params.jitter_active {
            return;
        }
        if params.jitter_index + 1 < params.jitter_count {
            self.jitter_sample_index = params.jitter_index + 1;
            self.jitter_more_samples = true;
        } else {
            self.jitter_more_samples = false;
        }
    }

    pub fn needs_continuous_redraw(&self) -> bool {
        self.jitter_more_samples || self.ao_half_pending
    }

    pub fn force_redraw(&mut self) {
        self.jitter_reset_requested = true;
        self.set_projection_changed();
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

    pub fn style(&self) -> &str {
        self.style_names()
    }

    pub fn apply_styles(&mut self, names: impl Into<String>) {
        self.style_names = names
            .into()
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(",");
    }

    pub fn push_style(&mut self, name: &str) -> bool {
        let mut styles = if self.style_names.is_empty() {
            Vec::new()
        } else {
            self.style_names.split(',').collect::<Vec<_>>()
        };
        if styles.last().copied() == Some(name) {
            return false;
        }
        styles.retain(|style| *style != name);
        styles.push(name);
        self.style_names = styles.join(",");
        true
    }

    pub fn remove_style_regex(&mut self, regex: &str) -> bool {
        let before = self.style_names.clone();
        let mut expression = LRegExpr::with_pattern(regex);
        self.style_names = self
            .style_names
            .split(',')
            .filter(|name| !expression.match_text(*name).unwrap_or(false))
            .collect::<Vec<_>>()
            .join(",");
        before != self.style_names
    }

    pub fn translate_view_drag(&mut self, dx: f64, dy: f64, dz: f64) {
        self.center = self.translated_center(dx, dy, dz);
        self.fire_property_changed("center", true);
    }

    fn translated_center(&self, dx: f64, dy: f64, dz: f64) -> Vector4 {
        let height = f64::from(self.height).max(1.0);
        let pixel_scale = self.zoom / height;
        let xy_translation = self
            .right_vector()
            .scale(dx * pixel_scale)
            .add_vec(self.up_vector().scale(-dy * pixel_scale));
        let z_translation = self.forward_vector().scale(dz / 4.0);
        self.center - xy_translation - z_translation
    }

    pub fn hit_precision(&self) -> f64 {
        self.hit_precision
    }

    pub fn set_hit_precision(&mut self, value: f64) {
        self.hit_precision = value.max(0.0);
    }

    pub fn set_hit_candidates(&mut self, candidates: Vec<ViewHitCandidate>) {
        self.hit_candidates = candidates;
    }

    pub fn add_hit_candidate(&mut self, candidate: ViewHitCandidate) {
        self.hit_candidates.push(candidate);
    }

    pub fn clear_hit_candidates(&mut self) {
        self.hit_candidates.clear();
    }

    pub fn hit_candidate_count(&self) -> usize {
        self.hit_candidates.len()
    }

    pub fn hit_test(&self, x: i32, y: i32) -> String {
        let half = self.hit_precision * 0.5;
        let selected = self
            .hit_candidates
            .iter()
            .filter(|candidate| {
                (candidate.x - x as f64).abs() <= half
                    && (candidate.y - y as f64).abs() <= half
                    && candidate.depth.is_finite()
            })
            .collect::<Vec<_>>();
        let Some(candidate) = nearest_candidate(&selected) else {
            return String::new();
        };
        self.single_hit_json(candidate)
    }

    pub fn hit_test_rect(&self, x: i32, y: i32, width: i32, height: i32, nearest: bool) -> String {
        let x2 = x.saturating_add(width);
        let y2 = y.saturating_add(height);
        let min_x = x.min(x2) as f64;
        let max_x = x.max(x2) as f64;
        let min_y = y.min(y2) as f64;
        let max_y = y.max(y2) as f64;
        let selected = self
            .hit_candidates
            .iter()
            .filter(|candidate| {
                candidate.x >= min_x
                    && candidate.x <= max_x
                    && candidate.y >= min_y
                    && candidate.y <= max_y
                    && candidate.depth.is_finite()
            })
            .collect::<Vec<_>>();
        self.multiple_hit_json(selected, nearest)
    }

    pub fn hit_test_polygon(&self, points: &[(f64, f64)], nearest: bool) -> String {
        if points.len() < 3 {
            return String::new();
        }
        let selected = self
            .hit_candidates
            .iter()
            .filter(|candidate| {
                candidate.depth.is_finite() && point_in_polygon(candidate.x, candidate.y, points)
            })
            .collect::<Vec<_>>();
        self.multiple_hit_json(selected, nearest)
    }

    fn single_hit_json(&self, candidate: &ViewHitCandidate) -> String {
        format!(
            "{{{}\"data\":{},\"scene_id\":{},\"rend_id\":{},\"rendtype\":\"{}\",\"rend_name\":\"{}\",\"obj_id\":{},\"obj_name\":\"{}\"}}",
            normalized_hit_fragment(&candidate.hit_fragment),
            json_int_array(&candidate.names),
            self.scene_uid.get(),
            candidate.renderer_id.get(),
            escape_json(&candidate.renderer_type),
            escape_json(&candidate.renderer_name),
            candidate.object_id.get(),
            escape_json(&candidate.object_name),
        )
    }

    fn multiple_hit_json(&self, selected: Vec<&ViewHitCandidate>, nearest: bool) -> String {
        if selected.is_empty() {
            return String::new();
        }
        let selected = if nearest {
            nearest_candidate(&selected).into_iter().collect::<Vec<_>>()
        } else {
            selected
        };
        let mut context = HittestContext::new();
        for candidate in &selected {
            context.add_hit(
                candidate.renderer_id,
                candidate.depth,
                candidate.names.clone(),
            );
        }
        let mut data = HitData::new();
        data.create_all(&context);

        let mut seen_objects = BTreeSet::new();
        let mut output = Vec::new();
        for renderer_id in data.renderer_ids() {
            let Some(candidate) = selected
                .iter()
                .find(|candidate| candidate.renderer_id == renderer_id)
            else {
                continue;
            };
            if !seen_objects.insert(candidate.object_id) {
                continue;
            }
            let hits = (0..data.data_size(renderer_id))
                .map(|index| {
                    let mut names = Vec::new();
                    let mut sub_index = 0;
                    while let Some(value) = data.data_at(renderer_id, index, sub_index) {
                        names.push(value);
                        sub_index += 1;
                    }
                    json_int_array(&names)
                })
                .collect::<Vec<_>>()
                .join(",");
            output.push(format!(
                "{{\"rend_id\":{},{}\"data\":[{}],\"obj_id\":{}}}",
                renderer_id.get(),
                normalized_hit_fragment(&candidate.hit_fragment),
                hits,
                candidate.object_id.get()
            ));
        }
        if output.is_empty() {
            String::new()
        } else {
            format!("[{}]", output.join(","))
        }
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
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
    ) {
        self.mouse_handler
            .button_down(current_time_millis(), client_x.trunc(), client_y.trunc());
        let mut event = Self::input_event(client_x, client_y, screen_x, screen_y, modifier);
        event.set_event_type(InDevEvent::INDEV_MOUSE_DOWN);
        self.fire_input_event(&mut event);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_mouse_up(
        &mut self,
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
    ) {
        let (release_state, velocity) =
            self.mouse_handler
                .release(current_time_millis(), client_x.trunc(), client_y.trunc());
        let mut event = Self::input_event(client_x, client_y, screen_x, screen_y, modifier);
        match release_state {
            DragState::None => return,
            DragState::Down => {
                event.set_event_type(click_event_type(event.modifier()));
            }
            DragState::Drag => {
                event.set_event_type(InDevEvent::INDEV_DRAG_END);
                if let Some((x, y)) = velocity {
                    event.set_velo_x(x);
                    event.set_velo_y(y);
                }
            }
        }
        self.fire_input_event(&mut event);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_mouse_move(
        &mut self,
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
    ) {
        let previous_state = self.mouse_handler.state();
        if previous_state == DragState::None {
            return;
        }
        let delta =
            self.mouse_handler
                .move_to(current_time_millis(), client_x.trunc(), client_y.trunc());
        let state = self.mouse_handler.state();
        if state == DragState::Down {
            return;
        }
        let mut event = Self::input_event(client_x, client_y, screen_x, screen_y, modifier);
        event.set_event_type(if previous_state == DragState::Down {
            InDevEvent::INDEV_DRAG_START
        } else {
            InDevEvent::INDEV_DRAG_MOVE
        });
        if previous_state == DragState::Drag {
            let total = self
                .mouse_handler
                .displacement(client_x.trunc(), client_y.trunc());
            event.set_delta_x(delta.0 as i32);
            event.set_delta_y(delta.1 as i32);
            event.set_move_x(total.0 as i32);
            event.set_move_y(total.1 as i32);
        }
        self.fire_input_event(&mut event);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_wheel(
        &mut self,
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
        delta_x: f64,
        delta_y: f64,
    ) {
        let mut event = Self::input_event(client_x, client_y, screen_x, screen_y, modifier);
        event.set_event_type(InDevEvent::INDEV_WHEEL);
        event.set_delta_x(delta_x.round() as i32);
        event.set_delta_y(delta_y.round() as i32);
        self.fire_input_event(&mut event);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_gesture(
        &mut self,
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
        axis_id: i32,
        delta: f64,
    ) {
        let mut event = Self::input_event(client_x, client_y, screen_x, screen_y, modifier);
        event.set_event_type(InDevEvent::INDEV_GESTURE);
        event.set_gesture_axis(axis_id);
        event.set_delta_y(delta.round() as i32);
        self.fire_input_event(&mut event);
    }

    pub fn add_view_listener(&self, listener: SharedEventListener<ViewEvent>) -> i32 {
        self.event_caster.add(listener).map_or(-1, |id| id as i32)
    }

    pub fn remove_view_listener(&self, id: i32) -> bool {
        id >= 0 && self.event_caster.remove(id as u64).is_some()
    }

    pub fn add_input_listener(&self, listener: SharedEventListener<InDevEvent>) -> i32 {
        self.input_caster.add(listener).map_or(-1, |id| id as i32)
    }

    pub fn remove_input_listener(&self, id: i32) -> bool {
        id >= 0 && self.input_caster.remove(id as u64).is_some()
    }

    pub fn fire_view_event(&self, event: &mut ViewEvent) {
        event.set_target(self.uid.get());
        event.set_source(self.scene_uid.get());
        self.event_caster.replica_fire(event);
        if let Some(manager) = self.event_manager.as_ref() {
            let _ = manager.with_mut(|manager| manager.fire_view_event(event));
        }
    }

    pub fn fire_input_event(&self, event: &mut InDevEvent) {
        self.input_caster.fire(event);
    }

    fn fire_property_changed(&self, property_name: &str, dragging: bool) {
        let mut event = ViewEvent::new();
        event.set_event_type(if dragging {
            ViewEvent::VWE_PROPCHG_DRG
        } else {
            ViewEvent::VWE_PROPCHG
        });
        event.set_descr(property_name);
        self.fire_view_event(&mut event);
    }

    fn input_event(
        client_x: f64,
        client_y: f64,
        screen_x: f64,
        screen_y: f64,
        modifier: i32,
    ) -> InDevEvent {
        let mut event = InDevEvent::new();
        event.set_x(client_x as i32);
        event.set_y(client_y as i32);
        event.set_root_x(screen_x as i32);
        event.set_root_y(screen_y as i32);
        event.set_modifier(translate_browser_modifier(modifier));
        event
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "View(name={}, UID={})", self.name, self.uid.get())
    }
}

fn current_time_millis() -> i64 {
    cuemol_core::clock::SystemTime::now()
        .duration_since(cuemol_core::clock::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

fn backing_dimension(logical: u32, scale: f64) -> u32 {
    (f64::from(logical) * scale).clamp(0.0, f64::from(u32::MAX)) as u32
}

fn orthographic_projection(view_width: f64, aspect: f64, slab_near: f64, slab_far: f64) -> Matrix4 {
    let left = -view_width * aspect;
    let right = view_width * aspect;
    let bottom = -view_width;
    let top = view_width;
    let mut matrix = Matrix4::IDENTITY;
    matrix.values[0][0] = 2.0 / (right - left);
    matrix.values[1][1] = 2.0 / (top - bottom);
    matrix.values[2][2] = -2.0 / (slab_far - slab_near);
    matrix.values[0][3] = -(right + left) / (right - left);
    matrix.values[1][3] = -(top + bottom) / (top - bottom);
    matrix.values[2][3] = -(slab_far + slab_near) / (slab_far - slab_near);
    matrix
}

fn perspective_projection(
    view_width: f64,
    aspect: f64,
    slab_near: f64,
    slab_far: f64,
    distance: f64,
) -> Matrix4 {
    let scale = distance / view_width;
    let mut matrix = Matrix4::ZERO;
    matrix.values[0][0] = scale / aspect;
    matrix.values[1][1] = scale;
    matrix.values[2][2] = (slab_far + slab_near) / (slab_near - slab_far);
    matrix.values[2][3] = (2.0 * slab_far * slab_near) / (slab_near - slab_far);
    matrix.values[3][2] = -1.0;
    matrix
}

fn nearest_candidate<'a>(candidates: &[&'a ViewHitCandidate]) -> Option<&'a ViewHitCandidate> {
    candidates
        .iter()
        .copied()
        .min_by(|left, right| left.depth.total_cmp(&right.depth))
}

fn point_in_polygon(x: f64, y: f64, points: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for current in 0..points.len() {
        let (current_x, current_y) = points[current];
        let (previous_x, previous_y) = points[previous];
        if (current_y > y) != (previous_y > y)
            && x < (previous_x - current_x) * (y - current_y) / (previous_y - current_y) + current_x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn json_int_array(values: &[i32]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn normalized_hit_fragment(value: &str) -> String {
    let value = value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    if value.is_empty() {
        String::new()
    } else if value.ends_with(',') {
        value.to_string()
    } else {
        format!("{value},")
    }
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn translate_browser_modifier(value: i32) -> i32 {
    let mut result = 0;
    if value & 1 != 0 {
        result |= InDevEvent::INDEV_LBTN;
    }
    if value & 2 != 0 {
        result |= InDevEvent::INDEV_RBTN;
    }
    if value & 4 != 0 {
        result |= InDevEvent::INDEV_MBTN;
    }
    if value & 32 != 0 {
        result |= InDevEvent::INDEV_CTRL;
    }
    if value & 64 != 0 {
        result |= InDevEvent::INDEV_SHIFT;
    }
    if value & 128 != 0 {
        result |= InDevEvent::INDEV_ALT;
    }
    result
}

fn click_event_type(modifier: i32) -> i32 {
    if modifier & InDevEvent::INDEV_LBTN != 0 {
        InDevEvent::INDEV_LBTN_CLICK
    } else if modifier & InDevEvent::INDEV_RBTN != 0 {
        InDevEvent::INDEV_RBTN_CLICK
    } else {
        InDevEvent::INDEV_MBTN_CLICK
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
    use cuemol_core::shared_event_listener;
    use std::sync::Mutex;

    fn sqlen(quat: Quat) -> f64 {
        quat.x * quat.x + quat.y * quat.y + quat.z * quat.z + quat.w * quat.w
    }

    fn hit_candidate(
        x: f64,
        y: f64,
        depth: f64,
        renderer_id: u64,
        object_id: u64,
        names: &[i32],
    ) -> ViewHitCandidate {
        ViewHitCandidate {
            x,
            y,
            depth,
            renderer_id: Uid::new(renderer_id),
            renderer_type: format!("type{renderer_id}"),
            renderer_name: format!("renderer\"{renderer_id}"),
            object_id: Uid::new(object_id),
            object_name: format!("object{object_id}"),
            names: names.to_vec(),
            hit_fragment: String::new(),
        }
    }

    #[test]
    fn view_clamps_and_normalizes_reference_values() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_zoom(0.0);
        assert!(view.zoom() > 0.0);
        view.set_view_dist(0.0);
        assert!(view.view_dist() >= 0.1);
        view.set_distance(99999.0);
        assert!(view.distance() <= 10000.0);
        view.set_slab(0.0);
        assert!(view.slab_depth() >= 0.1);

        view.set_rotation(Quat::new(2.0, 0.0, 0.0, 0.0));
        assert!((sqlen(view.rotation()) - 1.0).abs() < 1.0e-6);
        view.set_rotation(Quat::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(sqlen(view.rotation()), 0.0);
        assert!(!view.perspective());
        assert_eq!(view.style(), "");
    }

    #[test]
    fn view_vectors_and_projection_flags_match_reference_shape() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        assert_eq!(view.name(), "");
        assert!(view.is_active());
        assert_eq!((view.width(), view.height()), (100, 100));
        assert!(view.is_projection_changed());
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
    fn view_translation_converts_pixels_to_world_coordinates() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(200, 100);
        view.translate_view(10.0, 20.0, 8.0);
        assert_eq!(view.center(), Vector4::new3(-5.0, 10.0, 2.0));

        view.set_center(Vector4::ZERO);
        view.translate_view_drag(10.0, 20.0, 8.0);
        assert_eq!(view.center(), Vector4::new3(-5.0, 10.0, 2.0));
    }

    #[test]
    fn view_style_stack_matches_reference_order_and_regex_removal() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.apply_styles(" base, ribbon ,labels ");
        assert_eq!(view.style_names(), "base,ribbon,labels");
        assert!(!view.push_style("labels"));
        assert!(view.push_style("base"));
        assert_eq!(view.style_names(), "ribbon,labels,base");
        assert!(view.remove_style_regex("^(ribbon|base)$"));
        assert_eq!(view.style_names(), "labels");
        assert!(!view.remove_style_regex("^missing$"));
    }

    #[test]
    fn view_projects_world_positions_to_logical_screen_pixels() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(800, 600);

        let center = view.proj_to_screen(Vector4::ZERO);
        assert!((center.x - 400.0).abs() < 1.0e-9);
        assert!((center.y - 300.0).abs() < 1.0e-9);
        assert!(center.z.is_finite());

        let right = view.proj_to_screen(Vector4::new3(100.0 / 3.0, 0.0, 0.0));
        let top = view.proj_to_screen(Vector4::new3(0.0, 25.0, 0.0));
        assert!((right.x - 800.0).abs() < 1.0e-9);
        assert!(top.y.abs() < 1.0e-9);

        view.set_center(Vector4::new3(10.0, 20.0, 30.0));
        view.set_perspective(true);
        let perspective_center = view.proj_to_screen(view.center());
        assert!((perspective_center.x - 400.0).abs() < 1.0e-9);
        assert!((perspective_center.y - 300.0).abs() < 1.0e-9);
        assert!(perspective_center.z.is_finite());
    }

    #[test]
    fn view_projection_jitter_and_ao_constants_use_backing_pixels() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(200, 100);
        view.set_scl_fac(2.0, 3.0);
        assert_eq!(view.backing_width(), 400);
        assert_eq!(view.backing_height(), 300);

        let plain_ortho = view.projection_matrix();
        view.set_jitter_offset_pixels(0.25, -0.5);
        let jittered_ortho = view.projection_matrix();
        assert!((jittered_ortho.values[0][3] - plain_ortho.values[0][3] - 0.00125).abs() < 1.0e-12);
        assert!(
            (jittered_ortho.values[1][3] - plain_ortho.values[1][3] + 1.0 / 300.0).abs() < 1.0e-12
        );

        view.set_perspective(true);
        view.set_jitter_offset_pixels(0.0, 0.0);
        let plain_perspective = view.projection_matrix();
        view.set_jitter_offset_pixels(0.25, -0.5);
        let jittered_perspective = view.projection_matrix();
        assert!(
            (jittered_perspective.values[0][2] - plain_perspective.values[0][2] + 0.00125).abs()
                < 1.0e-12
        );
        assert!(
            (jittered_perspective.values[1][2] - plain_perspective.values[1][2] - 1.0 / 300.0)
                .abs()
                < 1.0e-12
        );

        let constants = view.compute_ao_constants();
        assert_eq!(constants.is_ortho, 0);
        assert!((constants.viewport_pixel_size[0] - 1.0 / 400.0).abs() < 1.0e-7);
        assert!((constants.viewport_pixel_size[1] - 1.0 / 300.0).abs() < 1.0e-7);
    }

    #[test]
    fn view_frame_state_accumulates_and_restarts_temporal_jitter() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(320, 240);

        let first = view.prepare_frame_render(2, true, true);
        assert!(first.jitter_active);
        assert_eq!(first.jitter_index, 0);
        assert_eq!(first.jitter_count, 4);
        assert_eq!(view.jitter_offset_pixels(), (0.0, 0.0));
        assert!(view.needs_continuous_redraw());
        view.complete_frame_render(&first);
        assert!(view.needs_continuous_redraw());

        let second = view.prepare_frame_render(2, false, false);
        assert_eq!(second.jitter_index, 1);
        assert_eq!(view.jitter_offset_pixels(), (0.375, -0.125));
        assert!((second.ao_noise_offset - 0.618_034_f32).abs() < 1.0e-6);
        view.complete_frame_render(&second);

        let third = view.prepare_frame_render(2, false, false);
        view.complete_frame_render(&third);
        let fourth = view.prepare_frame_render(2, false, false);
        view.complete_frame_render(&fourth);
        assert!(!view.needs_continuous_redraw());

        view.force_redraw();
        assert!(view.is_projection_changed());
        let restarted = view.prepare_frame_render(2, false, false);
        assert_eq!(restarted.jitter_index, 0);

        let disabled = view.prepare_frame_render(0, false, false);
        assert!(!disabled.jitter_active);
        assert_eq!(disabled.jitter_count, 1);
        assert_eq!(view.jitter_offset_pixels(), (0.0, 0.0));
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
        camera.set_ui_order(7);
        camera.set_source("saved.cam");
        camera.vis_append(42, false, true);
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
        let copied = view.camera();
        assert_eq!(copied.ui_order(), 7);
        assert_eq!(copied.source(), "saved.cam");
        assert_eq!(
            copied.visibility_settings().collect::<Vec<_>>(),
            camera.visibility_settings().collect::<Vec<_>>()
        );
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

    #[test]
    fn view_property_and_size_events_match_reference_categories() {
        let mut view = View::new(Uid::new(7), Uid::new(3));
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        view.add_view_listener(shared_event_listener(move |event: &mut ViewEvent| {
            listener_events.lock().unwrap().push(event.clone());
        }));

        view.set_zoom(75.0);
        view.set_center(Vector4::new3(1.0, 2.0, 3.0));
        view.translate_view_drag(1.0, 0.0, 0.0);
        view.size_changed(640, 480);
        view.set_active(true);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event_type(), ViewEvent::VWE_PROPCHG);
        assert_eq!(events[0].descr(), "zoom");
        assert_eq!(events[1].descr(), "center");
        assert_eq!(events[2].event_type(), ViewEvent::VWE_PROPCHG_DRG);
        assert_eq!(events[2].descr(), "center");
        assert_eq!(events[3].event_type(), ViewEvent::VWE_SIZECHG);
        assert!(events.iter().all(|event| event.target() == 7));
        assert!(events.iter().all(|event| event.source() == 3));
    }

    #[test]
    fn raw_input_methods_emit_mouse_drag_wheel_gesture_and_click_events() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        view.add_input_listener(shared_event_listener(move |event: &mut InDevEvent| {
            listener_events.lock().unwrap().push(event.clone());
        }));

        view.on_mouse_down(10.0, 20.0, 100.0, 200.0, 1 | 32 | 64);
        view.on_mouse_move(20.0, 25.0, 110.0, 205.0, 1);
        view.on_mouse_move(24.0, 30.0, 114.0, 210.0, 1);
        view.on_mouse_up(24.0, 30.0, 114.0, 210.0, 0);
        view.on_wheel(4.0, 5.0, 40.0, 50.0, 0, 1.4, -2.6);
        view.on_gesture(6.0, 7.0, 60.0, 70.0, 128, 2, 3.6);
        view.on_mouse_down(1.0, 2.0, 3.0, 4.0, 1);
        view.on_mouse_up(1.0, 2.0, 3.0, 4.0, 1);

        let events = events.lock().unwrap();
        let types = events
            .iter()
            .map(InDevEvent::event_type)
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            vec![
                InDevEvent::INDEV_MOUSE_DOWN,
                InDevEvent::INDEV_DRAG_START,
                InDevEvent::INDEV_DRAG_MOVE,
                InDevEvent::INDEV_DRAG_END,
                InDevEvent::INDEV_WHEEL,
                InDevEvent::INDEV_GESTURE,
                InDevEvent::INDEV_MOUSE_DOWN,
                InDevEvent::INDEV_LBTN_CLICK,
            ]
        );
        assert!(events[0].is_lbutton_on());
        assert!(events[0].is_ctrl_on());
        assert!(events[0].is_shift_on());
        assert_eq!(events[1].move_x(), 0);
        assert_eq!(events[1].move_y(), 0);
        assert_eq!(events[2].delta_x(), 4);
        assert_eq!(events[2].delta_y(), 5);
        assert_eq!(events[2].move_x(), 14);
        assert_eq!(events[2].move_y(), 10);
        assert_eq!(events[4].delta_x(), 1);
        assert_eq!(events[4].delta_y(), -3);
        assert_eq!(events[5].gesture_axis(), 2);
        assert_eq!(events[5].delta_y(), 4);
        assert!(events[5].is_alt_on());
    }

    #[test]
    fn point_hit_test_returns_nearest_candidate_with_reference_metadata() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_hit_candidates(vec![
            hit_candidate(10.0, 10.0, 0.8, 8, 80, &[2]),
            hit_candidate(12.0, 11.0, 0.2, 3, 30, &[7, 9])
                .with_hit_fragment("\"atom_id\":7,\"sel\":\"aid 7\""),
        ]);
        assert_eq!(view.hit_precision(), 10.0);
        assert_eq!(view.hit_candidate_count(), 2);
        assert_eq!(
            view.hit_test(10, 10),
            "{\"atom_id\":7,\"sel\":\"aid 7\",\"data\":[7,9],\"scene_id\":2,\"rend_id\":3,\"rendtype\":\"type3\",\"rend_name\":\"renderer\\\"3\",\"obj_id\":30,\"obj_name\":\"object30\"}"
        );
        assert!(view.hit_test(100, 100).is_empty());
    }

    #[test]
    fn rectangle_and_polygon_hit_tests_group_renderers_and_deduplicate_objects() {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_hit_candidates(vec![
            hit_candidate(10.0, 10.0, 0.2, 1, 10, &[1]),
            hit_candidate(15.0, 15.0, 0.3, 1, 10, &[3]),
            hit_candidate(30.0, 30.0, 0.4, 2, 20, &[2]),
            hit_candidate(20.0, 20.0, 0.1, 3, 10, &[4]),
        ]);

        assert_eq!(
            view.hit_test_rect(0, 0, 40, 40, false),
            "[{\"rend_id\":1,\"data\":[[1],[3]],\"obj_id\":10},{\"rend_id\":2,\"data\":[[2]],\"obj_id\":20}]"
        );
        assert_eq!(
            view.hit_test_rect(0, 0, 40, 40, true),
            "[{\"rend_id\":3,\"data\":[[4]],\"obj_id\":10}]"
        );
        assert_eq!(
            view.hit_test_polygon(&[(5.0, 5.0), (25.0, 5.0), (25.0, 25.0), (5.0, 25.0)], false,),
            "[{\"rend_id\":1,\"data\":[[1],[3]],\"obj_id\":10}]"
        );
        assert!(view
            .hit_test_polygon(&[(0.0, 0.0), (1.0, 1.0)], false)
            .is_empty());
        view.clear_hit_candidates();
        assert_eq!(view.hit_candidate_count(), 0);
    }
}
