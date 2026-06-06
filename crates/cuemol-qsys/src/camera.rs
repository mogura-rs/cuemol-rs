use cuemol_core::{Quat, Vector4};
use std::collections::BTreeMap;

const F_EPS4: f64 = 1.0e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StereoMode {
    None,
    Parallel,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterMark {
    None,
    Cross,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    name: String,
    zoom: f64,
    slab_depth: f64,
    cam_dist: f64,
    stereo_distance: f64,
    stereo_mode: StereoMode,
    perspective: bool,
    center_mark: CenterMark,
    center: Vector4,
    rotation: Quat,
    visibility: BTreeMap<String, bool>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            name: String::new(),
            zoom: 50.0,
            slab_depth: 50.0,
            cam_dist: 200.0,
            stereo_distance: 1.0,
            stereo_mode: StereoMode::None,
            perspective: false,
            center_mark: CenterMark::Cross,
            center: Vector4::ZERO,
            rotation: Quat::new(1.0, 0.0, 0.0, 0.0),
            visibility: BTreeMap::new(),
        }
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.max(F_EPS4);
    }

    pub fn slab_depth(&self) -> f64 {
        self.slab_depth
    }

    pub fn set_slab_depth(&mut self, slab_depth: f64) {
        self.slab_depth = slab_depth.clamp(0.1, 10000.0);
    }

    pub fn cam_dist(&self) -> f64 {
        self.cam_dist
    }

    pub fn set_cam_dist(&mut self, cam_dist: f64) {
        self.cam_dist = cam_dist.clamp(0.1, 10000.0);
    }

    pub fn stereo_distance(&self) -> f64 {
        self.stereo_distance
    }

    pub fn set_stereo_distance(&mut self, stereo_distance: f64) {
        self.stereo_distance = stereo_distance;
    }

    pub fn stereo_mode(&self) -> StereoMode {
        self.stereo_mode
    }

    pub fn set_stereo_mode(&mut self, stereo_mode: StereoMode) {
        self.stereo_mode = stereo_mode;
    }

    pub fn is_perspective(&self) -> bool {
        self.perspective
    }

    pub fn set_perspective(&mut self, perspective: bool) {
        self.perspective = perspective;
    }

    pub fn center_mark(&self) -> CenterMark {
        self.center_mark
    }

    pub fn set_center_mark(&mut self, center_mark: CenterMark) {
        self.center_mark = center_mark;
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn set_center(&mut self, center: Vector4) {
        self.center = center;
    }

    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
    }

    pub fn visibility_size(&self) -> usize {
        self.visibility.len()
    }

    pub fn vis_size(&self) -> usize {
        self.visibility_size()
    }

    pub fn visibility_json(&self) -> String {
        if self.visibility.is_empty() {
            return "{}".to_string();
        }
        let entries = self
            .visibility
            .iter()
            .map(|(name, visible)| format!("\"{}\":{}", escape_json(name), visible))
            .collect::<Vec<_>>();
        format!("{{{}}}", entries.join(","))
    }

    pub fn get_vis_set_json(&self) -> String {
        self.visibility_json()
    }

    pub fn clear_visibility_settings(&mut self) {
        self.visibility.clear();
    }

    pub fn save_vis_settings<T>(&mut self, scene: &crate::Scene<T>) {
        self.visibility.clear();
        for object_uid in scene.object_uids() {
            let Some(object) = scene.object(object_uid) else {
                continue;
            };
            self.vis_append(object_uid.get(), object.is_visible(), true);
            for renderer_uid in object.renderer_uids() {
                let Some(renderer) = object.renderer(renderer_uid) else {
                    continue;
                };
                self.vis_append(renderer_uid.get(), renderer.is_visible(), false);
            }
        }
    }

    pub fn load_vis_settings<T>(&self, scene: &mut crate::Scene<T>) {
        for (key, visible) in &self.visibility {
            let Some((is_object, uid)) = parse_visibility_key(key) else {
                continue;
            };
            if is_object {
                if let Some(object) = scene.object_mut(uid) {
                    object.set_visible(*visible);
                }
            } else if let Some(object_uid) = scene.object_uids().into_iter().find(|object_uid| {
                scene
                    .object(*object_uid)
                    .is_some_and(|object| object.renderer(uid).is_some())
            }) {
                if let Some(renderer) = scene
                    .object_mut(object_uid)
                    .and_then(|object| object.renderer_mut(uid))
                {
                    renderer.set_visible(*visible);
                }
            }
        }
    }

    pub fn notify_loaded<T>(&self, scene: &mut crate::Scene<T>) {
        self.load_vis_settings(scene);
    }

    pub fn clear_vis_settings(&mut self) {
        self.clear_visibility_settings();
    }

    pub fn vis_append(&mut self, target_id: u64, visible: bool, is_object: bool) {
        let prefix = if is_object { "obj" } else { "rend" };
        self.visibility
            .insert(format!("{prefix}:{target_id}"), visible);
    }

    pub fn vis_remove(&mut self, target_id: u64) -> bool {
        let obj = self
            .visibility
            .remove(&format!("obj:{target_id}"))
            .is_some();
        let rend = self
            .visibility
            .remove(&format!("rend:{target_id}"))
            .is_some();
        obj || rend
    }

    pub fn to_camera_data(&self) -> String {
        let mut output = String::from("cuemol2-rs-camera\t1\n");
        push_field(&mut output, "name", &escape_field(&self.name));
        push_field(&mut output, "zoom", &self.zoom.to_string());
        push_field(&mut output, "slab_depth", &self.slab_depth.to_string());
        push_field(&mut output, "cam_dist", &self.cam_dist.to_string());
        push_field(
            &mut output,
            "stereo_distance",
            &self.stereo_distance.to_string(),
        );
        push_field(
            &mut output,
            "stereo_mode",
            stereo_mode_name(self.stereo_mode),
        );
        push_field(&mut output, "perspective", &self.perspective.to_string());
        push_field(
            &mut output,
            "center_mark",
            center_mark_name(self.center_mark),
        );
        push_field(&mut output, "center", &format_vector(self.center));
        push_field(&mut output, "rotation", &format_quat(self.rotation));
        for (target, visible) in &self.visibility {
            push_field(
                &mut output,
                "vis",
                &format!("{},{}", escape_field(target), visible),
            );
        }
        output
    }

    pub fn from_camera_data(input: &str) -> Option<Self> {
        let mut lines = input.lines();
        if lines.next()? != "cuemol2-rs-camera\t1" {
            return None;
        }
        let mut camera = Self::new();
        for line in lines {
            let Some((key, value)) = line.split_once('\t') else {
                continue;
            };
            match key {
                "name" => camera.name = unescape_field(value)?,
                "zoom" => camera.set_zoom(value.parse().ok()?),
                "slab_depth" => camera.set_slab_depth(value.parse().ok()?),
                "cam_dist" => camera.set_cam_dist(value.parse().ok()?),
                "stereo_distance" => camera.set_stereo_distance(value.parse().ok()?),
                "stereo_mode" => camera.set_stereo_mode(parse_stereo_mode(value)?),
                "perspective" => camera.set_perspective(value.parse().ok()?),
                "center_mark" => camera.set_center_mark(parse_center_mark(value)?),
                "center" => camera.set_center(parse_vector(value)?),
                "rotation" => camera.set_rotation(parse_quat(value)?),
                "vis" => {
                    let (target, visible) = value.rsplit_once(',')?;
                    camera
                        .visibility
                        .insert(unescape_field(target)?, visible.parse().ok()?);
                }
                _ => {}
            }
        }
        Some(camera)
    }
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('\t');
    output.push_str(value);
    output.push('\n');
}

fn stereo_mode_name(mode: StereoMode) -> &'static str {
    match mode {
        StereoMode::None => "none",
        StereoMode::Parallel => "parallel",
        StereoMode::Cross => "cross",
    }
}

fn parse_stereo_mode(value: &str) -> Option<StereoMode> {
    match value {
        "none" => Some(StereoMode::None),
        "parallel" => Some(StereoMode::Parallel),
        "cross" => Some(StereoMode::Cross),
        _ => None,
    }
}

fn center_mark_name(mark: CenterMark) -> &'static str {
    match mark {
        CenterMark::None => "none",
        CenterMark::Cross => "cross",
    }
}

fn parse_center_mark(value: &str) -> Option<CenterMark> {
    match value {
        "none" => Some(CenterMark::None),
        "cross" => Some(CenterMark::Cross),
        _ => None,
    }
}

fn format_vector(value: Vector4) -> String {
    format!("{},{},{},{}", value.x, value.y, value.z, value.w)
}

fn parse_vector(value: &str) -> Option<Vector4> {
    let mut parts = value.split(',');
    Some(Vector4::new4(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn format_quat(value: Quat) -> String {
    format!("{},{},{},{}", value.x, value.y, value.z, value.w)
}

fn parse_quat(value: &str) -> Option<Quat> {
    let mut parts = value.split(',');
    Some(Quat::new(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn escape_field(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_field(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next()? {
            '\\' => output.push('\\'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            _ => return None,
        }
    }
    Some(output)
}

fn parse_visibility_key(key: &str) -> Option<(bool, cuemol_core::Uid)> {
    let (prefix, id) = key.split_once(':')?;
    let uid = cuemol_core::Uid::new(id.parse().ok()?);
    match prefix {
        "obj" => Some((true, uid)),
        "rend" => Some((false, uid)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_defaults_match_qsys_reference() {
        let camera = Camera::default();
        assert_eq!(camera.zoom(), 50.0);
        assert_eq!(camera.slab_depth(), 50.0);
        assert_eq!(camera.cam_dist(), 200.0);
        assert_eq!(camera.stereo_distance(), 1.0);
        assert_eq!(camera.stereo_mode(), StereoMode::None);
        assert!(!camera.is_perspective());
        assert_eq!(camera.center_mark(), CenterMark::Cross);
        assert_eq!(camera.center(), Vector4::ZERO);
        assert_eq!(camera.rotation(), Quat::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(camera.visibility_size(), 0);
        assert_eq!(camera.visibility_json(), "{}");
    }

    #[test]
    fn camera_clamps_reference_ranges() {
        let mut camera = Camera::default();
        camera.set_cam_dist(0.05);
        assert_eq!(camera.cam_dist(), 0.1);
        camera.set_cam_dist(20000.0);
        assert_eq!(camera.cam_dist(), 10000.0);
        camera.set_zoom(0.0);
        assert_eq!(camera.zoom(), F_EPS4);
        camera.set_slab_depth(0.05);
        assert_eq!(camera.slab_depth(), 0.1);
        camera.set_slab_depth(20000.0);
        assert_eq!(camera.slab_depth(), 10000.0);
    }

    #[test]
    fn camera_saves_and_loads_scene_visibility_settings() {
        let mut scene = crate::Scene::new(cuemol_core::Uid::new(1), "scene");
        let object_uid = scene.add_object("obj", ());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        scene.object_mut(object_uid).unwrap().set_visible(false);
        scene
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_visible(false);

        let mut camera = Camera::new();
        camera.save_vis_settings(&scene);
        scene.object_mut(object_uid).unwrap().set_visible(true);
        scene
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_visible(true);

        camera.load_vis_settings(&mut scene);

        assert!(!scene.object(object_uid).unwrap().is_visible());
        assert!(!scene
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap()
            .is_visible());
    }
}
