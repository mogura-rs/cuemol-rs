use cuemol_core::{
    parse_ldom2_xml, write_ldom2_xml, CueMolError, LDom2Tree, Quat, Result, Vector4,
};
use std::collections::BTreeMap;

const F_EPS4: f64 = 1.0e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibilitySetting {
    visible: bool,
    is_object: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StereoMode {
    None,
    Parallel,
    Cross,
    Hardware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterMark {
    None,
    Cross,
    Axis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    name: String,
    source: String,
    alt_source: String,
    zoom: f64,
    slab_depth: f64,
    cam_dist: f64,
    stereo_distance: f64,
    stereo_mode: StereoMode,
    perspective: bool,
    center_mark: CenterMark,
    center: Vector4,
    rotation: Quat,
    visibility: BTreeMap<u64, VisibilitySetting>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            name: String::new(),
            source: String::new(),
            alt_source: String::new(),
            zoom: 50.0,
            slab_depth: 50.0,
            cam_dist: 200.0,
            stereo_distance: 1.0,
            stereo_mode: StereoMode::None,
            perspective: false,
            center_mark: CenterMark::Cross,
            center: Vector4::ZERO,
            rotation: Quat::IDENTITY,
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

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn get_source(&self) -> &str {
        self.source()
    }

    pub fn src(&self) -> &str {
        self.source()
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    pub fn alt_source(&self) -> &str {
        &self.alt_source
    }

    pub fn set_alt_source(&mut self, source: impl Into<String>) {
        self.alt_source = source.into();
    }

    pub fn update_source_path(&mut self, source: impl Into<String>) {
        self.set_source(source);
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

    pub fn slab_depth(&self) -> f64 {
        self.slab_depth
    }

    pub fn get_slab_depth(&self) -> f64 {
        self.slab_depth()
    }

    pub fn set_slab_depth(&mut self, slab_depth: f64) {
        self.slab_depth = slab_depth.clamp(0.1, 10000.0);
    }

    pub fn slab(&self) -> f64 {
        self.slab_depth()
    }

    pub fn set_slab(&mut self, slab_depth: f64) {
        self.set_slab_depth(slab_depth);
    }

    pub fn cam_dist(&self) -> f64 {
        self.cam_dist
    }

    pub fn get_cam_dist(&self) -> f64 {
        self.cam_dist()
    }

    pub fn set_cam_dist(&mut self, cam_dist: f64) {
        self.cam_dist = cam_dist.clamp(0.1, 10000.0);
    }

    pub fn distance(&self) -> f64 {
        self.cam_dist()
    }

    pub fn set_distance(&mut self, cam_dist: f64) {
        self.set_cam_dist(cam_dist);
    }

    pub fn stereo_distance(&self) -> f64 {
        self.stereo_distance
    }

    pub fn set_stereo_distance(&mut self, stereo_distance: f64) {
        self.stereo_distance = stereo_distance;
    }

    pub fn stereo_dist(&self) -> f64 {
        self.stereo_distance()
    }

    pub fn set_stereo_dist(&mut self, stereo_distance: f64) {
        self.set_stereo_distance(stereo_distance);
    }

    pub fn stereo_mode(&self) -> StereoMode {
        self.stereo_mode
    }

    pub fn get_stereo_mode(&self) -> StereoMode {
        self.stereo_mode()
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

    pub fn perspec(&self) -> bool {
        self.is_perspective()
    }

    pub fn set_perspec(&mut self, perspective: bool) {
        self.set_perspective(perspective);
    }

    pub fn center_mark(&self) -> CenterMark {
        self.center_mark
    }

    pub fn get_center_mark(&self) -> CenterMark {
        self.center_mark()
    }

    pub fn set_center_mark(&mut self, center_mark: CenterMark) {
        self.center_mark = center_mark;
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn get_center(&self) -> Vector4 {
        self.center()
    }

    pub fn set_center(&mut self, center: Vector4) {
        self.center = center;
    }

    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    pub fn get_rot_quat(&self) -> Quat {
        self.rotation()
    }

    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
    }

    pub fn set_rot_quat(&mut self, rotation: Quat) {
        self.set_rotation(rotation);
    }

    pub fn visibility_size(&self) -> usize {
        self.visibility.len()
    }

    pub fn vis_size(&self) -> usize {
        self.visibility_size()
    }

    pub fn get_vis_size(&self) -> usize {
        self.visibility_size()
    }

    pub fn visibility_json(&self) -> String {
        if self.visibility.is_empty() {
            return "{}".to_string();
        }
        let entries = self
            .visibility
            .iter()
            .map(|(uid, setting)| {
                let kind = if setting.is_object {
                    "object"
                } else {
                    "renderer"
                };
                format!(
                    "\"{uid}\":{{\"uid\":{uid},\"type\":\"{kind}\",\"include\":true,\"visible\":{}}}",
                    setting.visible
                )
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", entries.join(","))
    }

    pub fn get_vis_set_json(&self) -> String {
        self.visibility_json()
    }

    pub fn visibility_settings(&self) -> impl Iterator<Item = (u64, bool, bool)> + '_ {
        self.visibility
            .iter()
            .map(|(uid, setting)| (*uid, setting.visible, setting.is_object))
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
        for (uid, setting) in &self.visibility {
            let uid = cuemol_core::Uid::new(*uid);
            if setting.is_object {
                if let Some(object) = scene.object_mut(uid) {
                    object.set_visible(setting.visible);
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
                    renderer.set_visible(setting.visible);
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
        self.visibility
            .entry(target_id)
            .or_insert(VisibilitySetting { visible, is_object });
    }

    pub fn vis_remove(&mut self, target_id: u64) -> bool {
        self.visibility.remove(&target_id).is_some()
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
        for (uid, setting) in &self.visibility {
            let prefix = if setting.is_object { "obj" } else { "rend" };
            push_field(
                &mut output,
                "vis",
                &format!("{prefix}:{uid},{}", setting.visible),
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
                    let target = unescape_field(target)?;
                    let (is_object, uid) = parse_visibility_key(&target)?;
                    camera.vis_append(uid.get(), visible.parse().ok()?, is_object);
                }
                _ => {}
            }
        }
        Some(camera)
    }

    /// Serialize this camera in the LDOM2 XML shape used by SceneXMLWriter.
    pub fn to_xml_string(&self) -> String {
        let mut tree = LDom2Tree::new("camera");
        let node = tree.top_mut();
        node.set_str_attr("name", &self.name);
        if !self.source.is_empty() {
            node.set_str_attr("src", &self.source);
        }
        node.set_str_attr("zoom", self.zoom.to_string());
        node.set_str_attr("slab", self.slab_depth.to_string());
        node.set_str_attr("center", format_vector3(self.center));
        node.set_str_attr("rotation", format_quat_tuple(self.rotation));
        node.set_str_attr("distance", self.cam_dist.to_string());
        node.set_str_attr("stereoDist", self.stereo_distance.to_string());
        node.set_str_attr("stereoMode", stereo_mode_name(self.stereo_mode));
        node.set_str_attr("perspec", self.perspective.to_string());
        node.set_str_attr("centerMark", center_mark_name(self.center_mark));
        write_ldom2_xml(&tree)
    }

    /// Deserialize a standalone camera document produced by SceneXMLWriter.
    pub fn from_xml_string(input: &str) -> Result<Self> {
        let tree = parse_ldom2_xml(input)?;
        let node = tree.top();
        if node.tag_name() != "camera" {
            return Err(CueMolError::parse(format!(
                "expected camera XML, found {}",
                node.tag_name()
            )));
        }

        let mut camera = Self::new();
        if let Some(value) = xml_attr(node, "name") {
            camera.set_name(value);
        }
        if let Some(value) = xml_attr(node, "src") {
            camera.set_source(value);
        }
        if let Some(value) = xml_attr(node, "zoom") {
            camera.set_zoom(parse_xml_number(value, "zoom")?);
        }
        if let Some(value) = xml_attr(node, "slab") {
            camera.set_slab_depth(parse_xml_number(value, "slab")?);
        }
        if let Some(value) = xml_attr(node, "center") {
            camera.set_center(parse_vector3_tuple(value)?);
        }
        if let Some(value) = xml_attr(node, "rotation") {
            camera.set_rotation(parse_quat_tuple(value)?);
        }
        if let Some(value) = xml_attr(node, "distance") {
            camera.set_cam_dist(parse_xml_number(value, "distance")?);
        }
        if let Some(value) = xml_attr(node, "stereoDist") {
            camera.set_stereo_distance(parse_xml_number(value, "stereoDist")?);
        }
        if let Some(value) = xml_attr(node, "stereoMode") {
            camera.set_stereo_mode(parse_stereo_mode(value).ok_or_else(|| {
                CueMolError::parse(format!("invalid camera stereoMode: {value}"))
            })?);
        }
        if let Some(value) = xml_attr(node, "perspec") {
            camera.set_perspective(parse_xml_bool(value, "perspec")?);
        }
        if let Some(value) = xml_attr(node, "centerMark") {
            camera.set_center_mark(parse_center_mark(value).ok_or_else(|| {
                CueMolError::parse(format!("invalid camera centerMark: {value}"))
            })?);
        }
        Ok(camera)
    }
}

fn xml_attr<'a>(node: &'a cuemol_core::LDom2Node, name: &str) -> Option<&'a str> {
    node.find_child(name)
        .filter(|child| child.is_attr())
        .map(cuemol_core::LDom2Node::value)
}

fn parse_xml_number(value: &str, name: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| CueMolError::parse(format!("invalid camera {name}: {value}")))
}

fn parse_xml_bool(value: &str, name: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(CueMolError::parse(format!(
            "invalid camera {name}: {value}"
        ))),
    }
}

fn parse_tuple(value: &str, expected: usize, name: &str) -> Result<Vec<f64>> {
    let value = value
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(value.trim());
    let values = value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f64>()
                .map_err(|_| CueMolError::parse(format!("invalid camera {name}: {value}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if values.len() != expected {
        return Err(CueMolError::parse(format!(
            "invalid camera {name}: expected {expected} components"
        )));
    }
    Ok(values)
}

fn format_vector3(value: Vector4) -> String {
    format!("({},{},{})", value.x, value.y, value.z)
}

fn parse_vector3_tuple(value: &str) -> Result<Vector4> {
    let values = parse_tuple(value, 3, "center")?;
    Ok(Vector4::new3(values[0], values[1], values[2]))
}

fn format_quat_tuple(value: Quat) -> String {
    format!("({},{},{},{})", value.x, value.y, value.z, value.w)
}

fn parse_quat_tuple(value: &str) -> Result<Quat> {
    let values = parse_tuple(value, 4, "rotation")?;
    Ok(Quat::new(values[0], values[1], values[2], values[3]))
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
        StereoMode::Parallel => "para",
        StereoMode::Cross => "cross",
        StereoMode::Hardware => "hardware",
    }
}

fn parse_stereo_mode(value: &str) -> Option<StereoMode> {
    match value {
        "none" => Some(StereoMode::None),
        "para" | "parallel" => Some(StereoMode::Parallel),
        "cross" => Some(StereoMode::Cross),
        "hardware" => Some(StereoMode::Hardware),
        _ => None,
    }
}

fn center_mark_name(mark: CenterMark) -> &'static str {
    match mark {
        CenterMark::None => "none",
        CenterMark::Cross => "crosshair",
        CenterMark::Axis => "axis",
    }
}

fn parse_center_mark(value: &str) -> Option<CenterMark> {
    match value {
        "none" => Some(CenterMark::None),
        "cross" | "crosshair" => Some(CenterMark::Cross),
        "axis" => Some(CenterMark::Axis),
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
        assert_eq!(camera.source(), "");
        assert_eq!(camera.src(), "");
        assert_eq!(camera.alt_source(), "");
        assert_eq!(camera.slab_depth(), 50.0);
        assert_eq!(camera.slab(), 50.0);
        assert_eq!(camera.distance(), 200.0);
        assert_eq!(camera.stereo_dist(), 1.0);
        assert_eq!(camera.cam_dist(), 200.0);
        assert_eq!(camera.stereo_distance(), 1.0);
        assert_eq!(camera.stereo_mode(), StereoMode::None);
        assert!(!camera.is_perspective());
        assert!(!camera.perspec());
        assert_eq!(camera.center_mark(), CenterMark::Cross);
        assert_eq!(camera.center(), Vector4::ZERO);
        assert_eq!(camera.rotation(), Quat::IDENTITY);
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

    #[test]
    fn camera_roundtrips_all_scriptable_display_modes() {
        for stereo_mode in [
            StereoMode::None,
            StereoMode::Parallel,
            StereoMode::Cross,
            StereoMode::Hardware,
        ] {
            for center_mark in [CenterMark::None, CenterMark::Cross, CenterMark::Axis] {
                let mut camera = Camera::new();
                camera.set_stereo_mode(stereo_mode);
                camera.set_center_mark(center_mark);
                let restored = Camera::from_camera_data(&camera.to_camera_data()).unwrap();
                assert_eq!(restored.stereo_mode(), stereo_mode);
                assert_eq!(restored.center_mark(), center_mark);
            }
        }
    }

    #[test]
    fn camera_tracks_primary_and_alternative_sources() {
        let mut camera = Camera::new();
        camera.set_source("primary.cam");
        camera.set_alt_source("fallback.cam");
        assert_eq!(camera.source(), "primary.cam");
        assert_eq!(camera.alt_source(), "fallback.cam");
        camera.update_source_path("resolved.cam");
        assert_eq!(camera.source(), "resolved.cam");
    }

    #[test]
    fn camera_visibility_json_matches_reference_shape() {
        let mut camera = Camera::new();
        camera.vis_append(10, false, true);
        camera.vis_append(20, true, false);
        camera.vis_append(10, true, false);
        assert_eq!(camera.visibility_size(), 2);
        assert_eq!(
            camera.get_vis_set_json(),
            "{\"10\":{\"uid\":10,\"type\":\"object\",\"include\":true,\"visible\":false},\"20\":{\"uid\":20,\"type\":\"renderer\",\"include\":true,\"visible\":true}}"
        );
        assert!(camera.vis_remove(10));
        assert!(!camera.vis_remove(10));
    }

    #[test]
    fn camera_xml_round_trip_matches_scene_xml_shape() {
        let mut camera = Camera::new();
        camera.set_name("cam & one");
        camera.set_source("source.cam");
        camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
        camera.set_rotation(Quat::new(0.1, 0.2, 0.3, 0.9));
        camera.set_zoom(80.0);
        camera.set_slab_depth(35.0);
        camera.set_cam_dist(125.0);
        camera.set_stereo_distance(1.25);
        camera.set_stereo_mode(StereoMode::Parallel);
        camera.set_perspective(true);
        camera.set_center_mark(CenterMark::Axis);

        let xml = camera.to_xml_string();
        assert!(xml.contains("<camera"));
        assert!(xml.contains("name=\"cam &amp; one\""));
        assert!(xml.contains("center=\"(1,2,3)\""));
        assert!(xml.contains("rotation=\"(0.1,0.2,0.3,0.9)\""));

        let restored = Camera::from_xml_string(&xml).unwrap();
        assert_eq!(restored, camera);
    }
}
