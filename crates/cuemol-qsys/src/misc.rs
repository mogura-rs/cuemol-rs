use crate::{RendererEvent, SceneEvent, ScrEventManagerHandle};
use cuemol_core::{
    EventCaster, LDom2Node, LPropEvent, LPropSupport, LString, LVariant, Matrix4,
    SharedEventListener, Uid, Vector4,
};
use cuemol_gfx::{RawHitData, Rgba8, SolidColor};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

pub const INVALID_UID: u64 = 0;

#[derive(Debug, Clone, PartialEq)]
struct GradientNode {
    value: f64,
    color: Rgba8,
    expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiGradient {
    nodes: Vec<GradientNode>,
}

impl Default for MultiGradient {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiGradient {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn create_default() -> Self {
        let mut gradient = Self::new();
        gradient.insert(
            0.0,
            Rgba8 {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
        );
        gradient
    }

    pub fn size(&self) -> usize {
        self.nodes.len()
    }
    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn insert(&mut self, value: f64, color: Rgba8) -> isize {
        self.insert_with_expression(value, color, None)
    }

    pub fn insert_with_expression(
        &mut self,
        value: f64,
        color: Rgba8,
        expression: Option<String>,
    ) -> isize {
        if !value.is_finite() || self.nodes.iter().any(|node| node.value == value) {
            return -1;
        }
        let index = self
            .nodes
            .iter()
            .position(|node| value < node.value)
            .unwrap_or(self.nodes.len());
        self.nodes.insert(
            index,
            GradientNode {
                value,
                color,
                expression,
            },
        );
        index as isize
    }

    pub fn value_at(&self, index: usize) -> Option<f64> {
        self.nodes.get(index).map(|node| node.value)
    }
    pub fn get_value_at(&self, index: usize) -> Option<f64> {
        self.value_at(index)
    }
    pub fn color_at(&self, index: usize) -> Option<Rgba8> {
        self.nodes.get(index).map(|node| node.color)
    }
    pub fn get_color_at(&self, index: usize) -> Option<Rgba8> {
        self.color_at(index)
    }
    pub fn get_color_2(&self, index: usize) -> Option<Rgba8> {
        self.color_at(index)
    }

    pub fn color_text_at(&self, index: usize) -> Option<String> {
        self.nodes.get(index).map(|node| {
            node.expression.clone().unwrap_or_else(|| {
                let color = node.color;
                SolidColor::from_rgba_u8(color.r, color.g, color.b, color.a).script_string()
            })
        })
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.nodes.len() {
            return false;
        }
        self.nodes.remove(index);
        true
    }

    pub fn change_at(&mut self, index: usize, value: f64, color: Rgba8) -> bool {
        if !value.is_finite()
            || index >= self.nodes.len()
            || self
                .nodes
                .iter()
                .enumerate()
                .any(|(i, node)| i != index && node.value == value)
        {
            return false;
        }
        self.nodes[index] = GradientNode {
            value,
            color,
            expression: None,
        };
        self.nodes
            .sort_by(|lhs, rhs| lhs.value.total_cmp(&rhs.value));
        true
    }

    pub fn copy_from(&mut self, source: &MultiGradient) {
        self.nodes.clone_from(&source.nodes);
    }
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    pub fn get_nodes_json(&self) -> String {
        serde_json::Value::Array(self.nodes.iter().enumerate().map(|(index, node)| {
            serde_json::json!({"value": node.value, "color": self.color_text_at(index).unwrap(),
                "r": node.color.r, "g": node.color.g, "b": node.color.b})
        }).collect()).to_string()
    }

    pub fn set_nodes_json(
        &mut self,
        json: &str,
        mut resolve_color: impl FnMut(&str) -> Option<Rgba8>,
    ) -> cuemol_core::Result<()> {
        let data: serde_json::Value = serde_json::from_str(json).map_err(|error| {
            cuemol_core::CueMolError::parse(format!("invalid gradient JSON: {error}"))
        })?;
        let nodes = data.as_array().ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("gradient nodes must be an array")
        })?;
        let mut scratch = Self::new();
        for node in nodes {
            let value = node["value"]
                .as_f64()
                .or_else(|| node["value"].as_str()?.parse().ok())
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    cuemol_core::CueMolError::invalid_argument(
                        "gradient node requires a finite value",
                    )
                })?;
            let text = node["color"].as_str().ok_or_else(|| {
                cuemol_core::CueMolError::invalid_argument("gradient node requires a color")
            })?;
            cuemol_gfx::parse_color(text)?;
            let color = resolve_color(text).ok_or_else(|| {
                cuemol_core::CueMolError::invalid_argument(format!(
                    "invalid gradient color: {text}"
                ))
            })?;
            scratch.insert_with_expression(value, color, Some(text.to_string()));
        }
        self.copy_from(&scratch);
        Ok(())
    }

    pub fn read_from_ldom2_node(
        &mut self,
        node: &LDom2Node,
        mut resolve_color: impl FnMut(&str) -> Option<Rgba8>,
    ) -> usize {
        self.clear();
        for node in node
            .children()
            .iter()
            .filter(|child| !child.is_attr() && child.tag_name() == "gradnode")
        {
            let Some(value) = node.str_attr("par").as_str().parse::<f64>().ok() else {
                continue;
            };
            let Some(color_node) = node.find_child("col") else {
                continue;
            };
            let text = if color_node.value().trim().is_empty() {
                color_node.contents().trim()
            } else {
                color_node.value().trim()
            };
            let Some(color) = resolve_color(text) else {
                continue;
            };
            self.insert_with_expression(value, color, Some(text.to_string()));
        }
        self.size()
    }

    pub fn color(&self, value: f64) -> Rgba8 {
        match self.nodes.as_slice() {
            [] => Rgba8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            [node] => node.color,
            nodes => {
                if value <= nodes[0].value {
                    return nodes[0].color;
                }
                if value >= nodes[nodes.len() - 1].value {
                    return nodes[nodes.len() - 1].color;
                }
                for pair in nodes.windows(2) {
                    let (left, right) = (&pair[0], &pair[1]);
                    if left.value <= value && value <= right.value {
                        let t = ((value - left.value) / (right.value - left.value)) as f32;
                        return lerp_color(left.color, right.color, t);
                    }
                }
                nodes[nodes.len() - 1].color
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Renderer {
    uid: Uid,
    scene_uid: Uid,
    client_obj_uid: Uid,
    type_name: String,
    center: Vector4,
    has_center: bool,
    name: String,
    visible: bool,
    ui_locked: bool,
    style_names: String,
    style_generation: u64,
    default_material: String,
    default_alpha: f64,
    xform_matrix: Matrix4,
    ui_order: i32,
    group_name: String,
    edge_line_type: i32,
    edge_line_width: f64,
    edge_line_color: SolidColor,
    properties: BTreeMap<String, String>,
    event_caster: Arc<EventCaster<RendererEvent>>,
    _scene_event_caster: Option<Arc<EventCaster<SceneEvent>>>,
    event_manager: Option<ScrEventManagerHandle>,
}

impl Renderer {
    pub const ELT_NONE: i32 = 0;
    pub const ELT_EDGES: i32 = 1;
    pub const ELT_SILHOUETTE: i32 = 2;

    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            uid: Uid::new(INVALID_UID),
            scene_uid: Uid::new(INVALID_UID),
            client_obj_uid: Uid::new(INVALID_UID),
            type_name: type_name.into(),
            center: Vector4::new3(0.0, 0.0, 0.0),
            has_center: false,
            name: String::new(),
            visible: true,
            ui_locked: false,
            style_names: String::new(),
            style_generation: 0,
            default_material: String::new(),
            default_alpha: 1.0,
            xform_matrix: Matrix4::IDENTITY,
            ui_order: 0,
            group_name: String::new(),
            edge_line_type: Self::ELT_NONE,
            edge_line_width: 0.01,
            edge_line_color: SolidColor::from_rgb(0.0, 0.0, 0.0),
            properties: BTreeMap::new(),
            event_caster: Arc::new(EventCaster::new()),
            _scene_event_caster: None,
            event_manager: None,
        }
    }

    pub fn with_uid(type_name: impl Into<String>, uid: Uid) -> Self {
        let mut renderer = Self::new(type_name);
        renderer.uid = uid;
        renderer.ui_order = uid.get() as i32;
        renderer
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

    pub fn scene_id(&self) -> Uid {
        self.scene_uid()
    }

    pub fn get_scene_id(&self) -> Uid {
        self.scene_uid()
    }

    pub fn get_scene(&self) -> Option<Uid> {
        (self.scene_uid.get() != INVALID_UID).then_some(self.scene_uid)
    }

    pub fn set_scene_uid(&mut self, value: Uid) {
        self.scene_uid = value;
    }

    pub(crate) fn attach_scene_event_caster(&mut self, caster: Arc<EventCaster<SceneEvent>>) {
        self._scene_event_caster = Some(caster);
    }

    pub(crate) fn attach_event_manager(&mut self, manager: ScrEventManagerHandle) {
        self.event_manager = Some(manager);
    }

    pub fn client_obj_uid(&self) -> Uid {
        self.client_obj_uid
    }

    pub fn client_obj_id(&self) -> Uid {
        self.client_obj_uid()
    }

    pub fn get_client_obj_id(&self) -> Uid {
        self.client_obj_uid()
    }

    pub fn get_client_obj(&self) -> Option<Uid> {
        (self.client_obj_uid.get() != INVALID_UID).then_some(self.client_obj_uid)
    }

    pub fn set_client_obj_uid(&mut self, value: Uid) {
        self.client_obj_uid = value;
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn get_type_name(&self) -> &str {
        self.type_name()
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn get_center_scr(&self) -> Vector4 {
        self.center()
    }

    pub fn get_center(&self) -> Vector4 {
        self.center()
    }

    pub fn set_center(&mut self, value: Vector4) {
        self.center = value;
        self.has_center = true;
        self.fire_property_changed("center");
    }

    pub fn clear_center(&mut self) {
        self.center = Vector4::new3(0.0, 0.0, 0.0);
        self.has_center = false;
        self.fire_property_changed("center");
    }

    pub fn has_center(&self) -> bool {
        self.has_center
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.fire_property_changed("name");
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn visible(&self) -> bool {
        self.is_visible()
    }

    pub fn set_visible(&mut self, value: bool) {
        self.visible = value;
        self.fire_property_changed("visible");
    }

    pub fn is_ui_locked(&self) -> bool {
        self.ui_locked
    }

    pub fn locked(&self) -> bool {
        self.is_ui_locked()
    }

    pub fn set_ui_locked(&mut self, value: bool) {
        self.ui_locked = value;
        self.fire_property_changed("locked");
    }

    pub fn set_locked(&mut self, value: bool) {
        self.set_ui_locked(value);
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

    pub fn set_style_names(&mut self, value: impl Into<String>) {
        self.style_names = value.into();
        self.fire_property_changed("styles");
    }

    pub fn default_material(&self) -> &str {
        &self.default_material
    }

    pub fn get_default_material(&self) -> &str {
        self.default_material()
    }

    pub fn material(&self) -> &str {
        self.default_material()
    }

    pub fn set_default_material(&mut self, value: impl Into<String>) {
        self.default_material = value.into();
        self.fire_property_changed("default_material");
    }

    pub fn set_material(&mut self, value: impl Into<String>) {
        self.set_default_material(value);
    }

    pub fn default_alpha(&self) -> f64 {
        self.default_alpha
    }

    pub fn get_default_alpha(&self) -> f64 {
        self.default_alpha()
    }

    pub fn alpha(&self) -> f64 {
        self.default_alpha()
    }

    pub fn set_default_alpha(&mut self, value: f64) {
        self.default_alpha = value;
        self.fire_property_changed("default_alpha");
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.set_default_alpha(value);
    }

    pub fn xform_matrix(&self) -> Matrix4 {
        self.xform_matrix
    }

    pub fn get_xform_matrix(&self) -> Matrix4 {
        self.xform_matrix()
    }

    pub fn xform_mat(&self) -> Matrix4 {
        self.xform_matrix()
    }

    pub fn set_xform_matrix(&mut self, value: Matrix4) {
        self.xform_matrix = value;
        self.fire_property_changed("xformMat");
    }

    pub fn set_xform_mat(&mut self, value: Matrix4) {
        self.set_xform_matrix(value);
    }

    pub fn ui_order(&self) -> i32 {
        self.ui_order
    }

    pub fn get_ui_order(&self) -> i32 {
        self.ui_order()
    }

    pub fn set_ui_order(&mut self, value: i32) {
        self.ui_order = value;
        self.fire_property_changed("ui_order");
    }

    pub fn group_name(&self) -> &str {
        &self.group_name
    }

    pub fn get_group_name(&self) -> &str {
        self.group_name()
    }

    pub fn group(&self) -> &str {
        self.group_name()
    }

    pub fn set_group_name(&mut self, value: impl Into<String>) {
        self.group_name = value.into();
        self.fire_property_changed("group");
    }

    pub fn set_group(&mut self, value: impl Into<String>) {
        self.set_group_name(value);
    }

    pub fn edge_line_type(&self) -> i32 {
        self.edge_line_type
    }

    pub fn edge_group_name(&self) -> &str {
        self.properties.get("egroup").map_or("", String::as_str)
    }

    pub fn get_edge_group_name(&self) -> &str {
        self.edge_group_name()
    }

    pub fn set_edge_group_name(&mut self, value: impl Into<String>) {
        self.properties.insert("egroup".into(), value.into());
        self.fire_property_changed("egroup");
    }

    pub fn get_edge_line_type(&self) -> i32 {
        self.edge_line_type()
    }

    pub fn egtype(&self) -> i32 {
        self.edge_line_type()
    }

    pub fn set_edge_line_type(&mut self, value: i32) {
        self.edge_line_type = value;
        self.fire_property_changed("egtype");
    }

    pub fn set_egtype(&mut self, value: i32) {
        self.set_edge_line_type(value);
    }

    pub fn edge_line_width(&self) -> f64 {
        self.edge_line_width
    }

    pub fn get_edge_line_width(&self) -> f64 {
        self.edge_line_width()
    }

    pub fn eglinew(&self) -> f64 {
        self.edge_line_width()
    }

    pub fn set_edge_line_width(&mut self, value: f64) {
        self.edge_line_width = value;
        self.fire_property_changed("eglinew");
    }

    pub fn set_eglinew(&mut self, value: f64) {
        self.set_edge_line_width(value);
    }

    pub fn edge_line_color(&self) -> &SolidColor {
        &self.edge_line_color
    }

    pub fn get_edge_line_color(&self) -> &SolidColor {
        self.edge_line_color()
    }

    pub fn egcolor(&self) -> &SolidColor {
        self.edge_line_color()
    }

    pub fn set_edge_line_color(&mut self, value: SolidColor) {
        self.edge_line_color = value;
        self.fire_property_changed("egcolor");
    }

    pub fn set_egcolor(&mut self, value: SolidColor) {
        self.set_edge_line_color(value);
    }

    pub fn property_str(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    pub fn set_property_str(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(name.into(), value.into());
    }

    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }

    pub fn to_style_node(&self) -> LDom2Node {
        let mut node = LDom2Node::new();
        node.set_tag_name("style");
        node.set_type_name("renderer");
        node.set_str_attr("visible", self.visible.to_string());
        node.set_str_attr("locked", self.ui_locked.to_string());
        node.set_str_attr("material", &self.default_material);
        node.set_str_attr("alpha", self.default_alpha.to_string());
        node.set_str_attr("xformMat", self.xform_matrix.to_script_string().to_string());
        node.set_str_attr("egtype", self.edge_line_type.to_string());
        node.set_str_attr("eglinew", self.edge_line_width.to_string());
        node.set_str_attr("egcolor", self.edge_line_color.script_string().to_string());
        for (name, value) in &self.properties {
            if !matches!(name.as_str(), "name" | "type" | "ui_order" | "group") {
                node.set_str_attr(name, value);
            }
        }
        node
    }

    pub fn read_properties_from_node(&mut self, node: &cuemol_core::LDom2Node) {
        for child in node.children().iter().filter(|child| child.is_attr()) {
            match child.tag_name() {
                "name" => self.set_name(child.value()),
                "group" => self.set_group_name(child.value()),
                "style" => self.apply_styles(child.value()),
                "visible" => {
                    self.set_visible(!matches!(child.value(), "false" | "0" | "no" | "off"))
                }
                "locked" => self.set_ui_locked(matches!(child.value(), "true" | "1" | "yes")),
                "default_material" => self.set_default_material(child.value()),
                "default_alpha" => {
                    if let Ok(value) = child.value().parse() {
                        self.set_default_alpha(value);
                    }
                }
                "xformMat" => {
                    if let Ok(value) = Matrix4::from_script_string(child.value()) {
                        self.set_xform_matrix(value);
                    }
                }
                "ui_order" => {
                    if let Ok(value) = child.value().parse() {
                        self.set_ui_order(value);
                    }
                }
                name => self.set_property_str(name, child.value()),
            }
        }
    }

    pub fn script_string(&self) -> LString {
        LString::from(format!(
            "Renderer(name={}, UID={})",
            self.name,
            self.uid.get()
        ))
    }

    pub fn apply_styles(&mut self, names: impl Into<String>) {
        self.style_names = names.into();
        self.reapply_style();
        self.fire_property_changed("styles");
    }

    pub fn reapply_style(&mut self) {
        self.style_generation = self.style_generation.saturating_add(1);
    }

    pub fn style_generation(&self) -> u64 {
        self.style_generation
    }

    pub fn remove_style_regex(&mut self, regex: &str) -> bool {
        let before = self.style_names.clone();
        self.style_names = self
            .style_names
            .split(',')
            .filter(|name| !name.contains(regex))
            .collect::<Vec<_>>()
            .join(",");
        let changed = before != self.style_names;
        if changed {
            self.fire_property_changed("styles");
        }
        changed
    }

    pub fn is_compatible_obj<T>(&self, _object: &crate::SceneObject<T>) -> bool {
        true
    }

    pub fn is_hit_test_supported(&self) -> bool {
        false
    }

    pub fn interp_hit(&self, _hit_data: &dyn RawHitData) -> String {
        String::new()
    }

    pub fn add_listener(&self, listener: SharedEventListener<RendererEvent>) -> i32 {
        self.event_caster.add(listener).map_or(-1, |id| id as i32)
    }

    pub fn remove_listener(&self, id: i32) -> bool {
        id >= 0 && self.event_caster.remove(id as u64).is_some()
    }

    pub fn fire_renderer_event(&self, event: &mut RendererEvent) {
        event.set_source(self.scene_uid.get());
        self.event_caster.replica_fire(event);
        if let Some(manager) = self.event_manager.as_ref() {
            let _ = manager.with(|manager| manager.fire_renderer_event(event));
        }
    }

    fn fire_property_changed(&self, property_name: &str) {
        let mut event = RendererEvent::new();
        event.set_event_type(RendererEvent::RNE_PROPCHG);
        event.set_target(self.uid.get());
        event.set_descr(property_name);
        self.fire_renderer_event(&mut event);
    }
}

impl fmt::Display for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Renderer(name={}, UID={})", self.name, self.uid.get())
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new("")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestRenderer {
    renderer: Renderer,
    color1: SolidColor,
}

impl TestRenderer {
    pub fn new() -> Self {
        Self {
            renderer: Renderer::new("test"),
            color1: SolidColor::from_rgb(0.0, 1.0, 1.0),
        }
    }

    pub fn with_uid(uid: Uid) -> Self {
        Self {
            renderer: Renderer::with_uid("test", uid),
            color1: SolidColor::from_rgb(0.0, 1.0, 1.0),
        }
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    pub fn type_name(&self) -> &str {
        self.renderer.type_name()
    }

    pub fn color1(&self) -> &SolidColor {
        &self.color1
    }

    pub fn color_1(&self) -> &SolidColor {
        self.color1()
    }

    pub fn set_color1(&mut self, value: SolidColor) {
        self.color1 = value;
    }

    pub fn set_color_1(&mut self, value: SolidColor) {
        self.set_color1(value);
    }

    pub fn is_hit_test_supported(&self) -> bool {
        true
    }

    pub fn interp_hit(&self, hit_data: &dyn RawHitData) -> String {
        let face_id = hit_data.data_at(self.renderer.uid(), 0, 0);
        match face_id {
            Some(face_id) => format!("\"faceid\":{face_id},\"objtype\":\"Test\","),
            None => "\"objtype\":\"Test\",".to_string(),
        }
    }
}

impl Default for TestRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendGroup {
    renderer: Renderer,
    ui_collapsed: bool,
}

impl RendGroup {
    pub fn new() -> Self {
        Self {
            renderer: Renderer::new("*group"),
            ..Self::default()
        }
    }

    pub fn type_name(&self) -> &'static str {
        "*group"
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    pub fn script_string(&self) -> LString {
        LString::from("Renderer group")
    }

    pub fn is_compatible_obj(&self) -> bool {
        true
    }

    pub fn is_ui_collapsed(&self) -> bool {
        self.ui_collapsed
    }

    pub fn ui_collapsed(&self) -> bool {
        self.is_ui_collapsed()
    }

    pub fn set_ui_collapsed(&mut self, value: bool) {
        self.ui_collapsed = value;
    }

    pub fn is_visible(&self) -> bool {
        self.renderer.is_visible()
    }

    pub fn set_visible(&mut self, value: bool) {
        self.renderer.set_visible(value);
    }

    pub fn is_ui_locked(&self) -> bool {
        self.renderer.is_ui_locked()
    }

    pub fn set_ui_locked(&mut self, value: bool) {
        self.renderer.set_ui_locked(value);
    }

    pub fn is_hit_test_supported(&self) -> bool {
        false
    }

    pub fn group_name(&self) -> &str {
        self.renderer.group_name()
    }
}

impl Default for RendGroup {
    fn default() -> Self {
        Self {
            renderer: Renderer::new("*group"),
            ui_collapsed: false,
        }
    }
}

pub struct PropEditInfo {
    target_uid: u64,
    prop_name: String,
    old_value: LVariant,
    new_value: LVariant,
    old_default: bool,
    new_default: bool,
    target: Option<Weak<RefCell<dyn LPropSupport>>>,
}

impl Default for PropEditInfo {
    fn default() -> Self {
        Self {
            target_uid: INVALID_UID,
            prop_name: String::new(),
            old_value: LVariant::Null,
            new_value: LVariant::Null,
            old_default: false,
            new_default: false,
            target: None,
        }
    }
}

impl fmt::Debug for PropEditInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PropEditInfo")
            .field("target_uid", &self.target_uid)
            .field("prop_name", &self.prop_name)
            .field("old_value", &self.old_value)
            .field("new_value", &self.new_value)
            .field("old_default", &self.old_default)
            .field("new_default", &self.new_default)
            .finish_non_exhaustive()
    }
}

impl PropEditInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn target_uid(&self) -> u64 {
        self.target_uid
    }

    pub fn set_target_uid(&mut self, value: u64) {
        self.target_uid = value;
    }

    pub fn prop_name(&self) -> &str {
        &self.prop_name
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.prop_name = value.into();
    }

    pub fn setup(
        &mut self,
        target_uid: u64,
        prop_name: impl Into<String>,
        old_value: impl Into<String>,
        new_value: impl Into<String>,
    ) {
        self.target_uid = target_uid;
        self.prop_name = prop_name.into();
        self.old_value = LVariant::String(old_value.into().into());
        self.new_value = LVariant::String(new_value.into().into());
        self.old_default = false;
        self.new_default = false;
        self.target = None;
    }

    pub fn setup_with_target(
        &mut self,
        target: Rc<RefCell<dyn LPropSupport>>,
        prop_name: impl Into<String>,
        old_value: LVariant,
        new_value: LVariant,
    ) {
        self.target_uid = target.borrow().root_uid();
        self.prop_name = prop_name.into();
        self.old_value = old_value;
        self.new_value = new_value;
        self.old_default = false;
        self.new_default = false;
        self.target = Some(Rc::downgrade(&target));
    }

    pub fn setup_from_event(&mut self, target: Rc<RefCell<dyn LPropSupport>>, event: &LPropEvent) {
        let prop_name = if event.parent_name().is_empty() {
            event.name().as_str().to_string()
        } else {
            format!("{}.{}", event.parent_name(), event.name())
        };
        self.target_uid = target.borrow().root_uid();
        self.prop_name = prop_name;
        self.old_value = event.old_value().clone();
        self.new_value = event.new_value().clone();
        self.old_default = event.is_old_default();
        self.new_default = event.is_new_default();
        self.target = Some(Rc::downgrade(&target));
    }

    pub fn is_undoable(&self) -> bool {
        self.target_exists()
    }

    pub fn is_redoable(&self) -> bool {
        self.target_exists()
    }

    pub fn undo(&self) -> bool {
        self.apply(&self.old_value, self.old_default)
    }

    pub fn redo(&self) -> bool {
        self.apply(&self.new_value, self.new_default)
    }

    fn target_exists(&self) -> bool {
        let Some(target) = self.target.as_ref().and_then(Weak::upgrade) else {
            return false;
        };
        let target = target.borrow();
        let name = LString::from(self.prop_name.as_str());
        if self.prop_name.contains('.') {
            target.has_nested_writable_property(&name)
        } else {
            target.has_writable_property(&name)
        }
    }

    fn apply(&self, value: &LVariant, default: bool) -> bool {
        let Some(target) = self.target.as_ref().and_then(Weak::upgrade) else {
            return false;
        };
        let mut target = target.borrow_mut();
        let name = LString::from(self.prop_name.as_str());
        match (self.prop_name.contains('.'), default) {
            (true, true) => target.reset_nested_property(&name),
            (true, false) => target.set_nested_property(&name, value),
            (false, true) => target.reset_property(&name),
            (false, false) => target.set_property(&name, value),
        }
    }
}

impl crate::EditInfo for PropEditInfo {
    fn undo(&mut self) -> bool {
        PropEditInfo::undo(self)
    }

    fn redo(&mut self) -> bool {
        PropEditInfo::redo(self)
    }

    fn is_undoable(&self) -> bool {
        PropEditInfo::is_undoable(self)
    }

    fn is_redoable(&self) -> bool {
        PropEditInfo::is_redoable(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewInputConfig {
    tb_rad: f64,
    hit_prec: f64,
    style_name: String,
    bindings: BTreeMap<i32, String>,
    explicit_bindings: Vec<(i32, String)>,
    explicit_tb_rad: bool,
    explicit_hit_prec: bool,
}

impl Default for ViewInputConfig {
    fn default() -> Self {
        let bindings = BTreeMap::from([
            (Self::VIEW_ROTX, "LBUTTON|MOUSE_X".to_string()),
            (Self::VIEW_ROTY, "LBUTTON|MOUSE_Y".to_string()),
            (Self::VIEW_ROTZ, "LBUTTON|RBUTTON|MOUSE_Y".to_string()),
            (Self::VIEW_TRAX, "RBUTTON|MOUSE_X".to_string()),
            (Self::VIEW_TRAY, "RBUTTON|MOUSE_Y".to_string()),
            (Self::VIEW_TRAZ, "LBUTTON|RBUTTON|MOUSE_X".to_string()),
            (
                Self::VIEW_ZOOM,
                "LBUTTON|SHIFT|MOUSE_Y,WHEEL1,WHEEL2".to_string(),
            ),
            (Self::VIEW_SLAB, "LBUTTON|SHIFT|MOUSE_X".to_string()),
        ]);
        Self {
            tb_rad: 0.8,
            hit_prec: 10.0,
            style_name: String::new(),
            bindings,
            explicit_bindings: Vec::new(),
            explicit_tb_rad: false,
            explicit_hit_prec: false,
        }
    }
}

impl ViewInputConfig {
    pub const VIEW_ROTX: i32 = 1;
    pub const VIEW_ROTY: i32 = 2;
    pub const VIEW_ROTZ: i32 = 3;
    pub const VIEW_ZOOM: i32 = 4;
    pub const VIEW_TRAX: i32 = 5;
    pub const VIEW_TRAY: i32 = 6;
    pub const VIEW_TRAZ: i32 = 7;
    pub const VIEW_SLAB: i32 = 8;
    pub const VIEW_DIST: i32 = 9;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn tb_rad(&self) -> f64 {
        self.tb_rad
    }

    pub fn get_tb_rad(&self) -> f64 {
        self.tb_rad()
    }

    pub fn tbrad(&self) -> f64 {
        self.tb_rad()
    }

    pub fn set_tb_rad(&mut self, value: f64) {
        self.tb_rad = value;
        self.explicit_tb_rad = true;
    }

    pub fn set_tbrad(&mut self, value: f64) {
        self.set_tb_rad(value);
    }

    pub fn hit_prec(&self) -> f64 {
        self.hit_prec
    }

    pub fn get_hit_prec(&self) -> f64 {
        self.hit_prec()
    }

    pub fn style_name(&self) -> &str {
        &self.style_name
    }

    pub fn get_style_name(&self) -> &str {
        self.style_name()
    }

    pub fn style(&self) -> &str {
        self.style_name()
    }

    pub fn apply_style(&mut self, value: impl Into<String>) {
        self.style_name = value.into();
    }

    pub fn set_style(&mut self, value: impl Into<String>) {
        self.apply_style(value);
    }

    pub fn hitprec(&self) -> f64 {
        self.hit_prec()
    }

    pub fn set_hit_prec(&mut self, value: f64) {
        self.hit_prec = value;
        self.explicit_hit_prec = true;
    }

    pub fn set_hitprec(&mut self, value: f64) {
        self.set_hit_prec(value);
    }

    pub fn set_binding(&mut self, id: i32, value: impl Into<String>) {
        let value = value.into();
        self.explicit_bindings
            .retain(|(previous, _)| *previous != id);
        self.explicit_bindings.push((id, value.clone()));
        self.assign_binding(id, value);
    }

    fn assign_binding(&mut self, id: i32, value: String) {
        let codes: Vec<_> = value
            .split(',')
            .map(crate::view_input::parse_binding)
            .filter(|&v| v != 0)
            .collect();
        for previous in self.bindings.values_mut() {
            *previous = previous
                .split(',')
                .filter(|v| !codes.contains(&crate::view_input::parse_binding(v)))
                .collect::<Vec<_>>()
                .join(",");
        }
        self.bindings.insert(id, value);
    }

    pub fn find_event(&self, axis: i32, modifiers: i32) -> i32 {
        let code = modifiers | (axis << 6);
        self.bindings
            .iter()
            .find_map(|(&id, value)| {
                value
                    .split(',')
                    .any(|v| !v.is_empty() && crate::view_input::parse_binding(v) == code)
                    .then_some(id)
            })
            .unwrap_or(-1)
    }

    pub fn resolved(&self, styles: &crate::StyleManager) -> Self {
        let mut resolved = Self::default();
        resolved.tb_rad = self.tb_rad;
        resolved.hit_prec = self.hit_prec;
        resolved.style_name = self.style_name.clone();
        for name in self.style_name.split(',').map(str::trim) {
            let Some(node) = styles.style_node(0, name) else {
                continue;
            };
            if !self.explicit_tb_rad {
                if let Ok(value) = node.str_attr("tbrad").as_str().parse() {
                    resolved.tb_rad = value;
                }
            }
            if !self.explicit_hit_prec {
                if let Ok(value) = node.str_attr("hitprec").as_str().parse() {
                    resolved.hit_prec = value;
                }
            }
            for (id, key) in [
                (1, "conf_rotx"),
                (2, "conf_roty"),
                (3, "conf_rotz"),
                (4, "conf_zoom"),
                (5, "conf_trax"),
                (6, "conf_tray"),
                (7, "conf_traz"),
                (8, "conf_slab"),
                (9, "conf_dist"),
            ] {
                if let Some(value) = node.find_child(key).map(|n| n.value()) {
                    resolved.assign_binding(id, value.to_string());
                }
            }
        }
        for (id, value) in &self.explicit_bindings {
            resolved.assign_binding(*id, value.clone());
        }
        resolved
    }

    pub fn binding(&self, id: i32) -> LString {
        self.bindings
            .get(&id)
            .cloned()
            .map(LString::from)
            .unwrap_or_default()
    }

    pub fn remove_bindings(&mut self, id: i32) {
        self.set_binding(id, "");
    }

    pub fn set_conf_rot_x(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_ROTX, value);
    }

    pub fn conf_rot_x(&self) -> LString {
        self.binding(Self::VIEW_ROTX)
    }

    pub fn get_conf_rot_x(&self) -> LString {
        self.conf_rot_x()
    }

    pub fn set_conf_rotx(&mut self, value: impl Into<String>) {
        self.set_conf_rot_x(value);
    }

    pub fn conf_rotx(&self) -> LString {
        self.conf_rot_x()
    }

    pub fn set_conf_roty(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_ROTY, value);
    }

    pub fn conf_roty(&self) -> LString {
        self.binding(Self::VIEW_ROTY)
    }

    pub fn get_conf_rot_y(&self) -> LString {
        self.conf_roty()
    }

    pub fn set_conf_rot_y(&mut self, value: impl Into<String>) {
        self.set_conf_roty(value);
    }

    pub fn set_conf_rotz(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_ROTZ, value);
    }

    pub fn conf_rotz(&self) -> LString {
        self.binding(Self::VIEW_ROTZ)
    }

    pub fn get_conf_rot_z(&self) -> LString {
        self.conf_rotz()
    }

    pub fn set_conf_rot_z(&mut self, value: impl Into<String>) {
        self.set_conf_rotz(value);
    }

    pub fn set_conf_trax(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_TRAX, value);
    }

    pub fn conf_trax(&self) -> LString {
        self.binding(Self::VIEW_TRAX)
    }

    pub fn get_conf_tra_x(&self) -> LString {
        self.conf_trax()
    }

    pub fn set_conf_tra_x(&mut self, value: impl Into<String>) {
        self.set_conf_trax(value);
    }

    pub fn set_conf_tray(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_TRAY, value);
    }

    pub fn conf_tray(&self) -> LString {
        self.binding(Self::VIEW_TRAY)
    }

    pub fn get_conf_tra_y(&self) -> LString {
        self.conf_tray()
    }

    pub fn set_conf_tra_y(&mut self, value: impl Into<String>) {
        self.set_conf_tray(value);
    }

    pub fn set_conf_traz(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_TRAZ, value);
    }

    pub fn conf_traz(&self) -> LString {
        self.binding(Self::VIEW_TRAZ)
    }

    pub fn get_conf_tra_z(&self) -> LString {
        self.conf_traz()
    }

    pub fn set_conf_tra_z(&mut self, value: impl Into<String>) {
        self.set_conf_traz(value);
    }

    pub fn set_conf_zoom(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_ZOOM, value);
    }

    pub fn conf_zoom(&self) -> LString {
        self.binding(Self::VIEW_ZOOM)
    }

    pub fn get_conf_zoom(&self) -> LString {
        self.conf_zoom()
    }

    pub fn set_conf_slab(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_SLAB, value);
    }

    pub fn conf_slab(&self) -> LString {
        self.binding(Self::VIEW_SLAB)
    }

    pub fn get_conf_slab(&self) -> LString {
        self.conf_slab()
    }

    pub fn set_conf_dist(&mut self, value: impl Into<String>) {
        self.set_binding(Self::VIEW_DIST, value);
    }

    pub fn conf_dist(&self) -> LString {
        self.binding(Self::VIEW_DIST)
    }

    pub fn get_conf_dist(&self) -> LString {
        self.conf_dist()
    }
}

fn lerp_color(lhs: Rgba8, rhs: Rgba8, t: f32) -> Rgba8 {
    let lerp = |a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * t).round() as u8 };
    Rgba8 {
        r: lerp(lhs.r, rhs.r),
        g: lerp(lhs.g, rhs.g),
        b: lerp(lhs.b, rhs.b),
        a: lerp(lhs.a, rhs.a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::{shared_event_listener, MapPropContainer, PropObject, PropSpec};
    use std::sync::Mutex;

    fn color(r: u8, g: u8, b: u8) -> Rgba8 {
        Rgba8 { r, g, b, a: 255 }
    }

    #[test]
    fn multi_gradient_matches_reference_node_and_color_cases() {
        let mut gradient = MultiGradient::new();
        assert_eq!(gradient.size(), 0);
        assert_eq!(gradient.insert(0.0, color(255, 0, 0)), 0);
        assert_eq!(gradient.insert(1.0, color(0, 255, 0)), 1);
        assert_eq!(gradient.insert(0.5, color(0, 0, 255)), 1);
        assert_eq!(gradient.insert(0.5, color(1, 1, 1)), -1);
        assert_eq!(gradient.value_at(0), Some(0.0));
        assert_eq!(gradient.color_at(1), Some(color(0, 0, 255)));
        assert!(gradient.remove_at(0));
        assert!(!gradient.remove_at(99));
        assert_eq!(gradient.color(-1.0), color(0, 0, 255));
        assert_eq!(gradient.color(9.0), color(0, 255, 0));
        gradient.clear();
        assert_eq!(gradient.color(0.5), color(0, 0, 0));
        assert_eq!(MultiGradient::create_default().size(), 1);
        assert_eq!(
            MultiGradient::create_default().color(0.),
            color(255, 255, 255)
        );
    }

    #[test]
    fn gradient_json_is_atomic_and_preserves_expressions_and_first_duplicate() {
        let resolve = |text: &str| cuemol_gfx::parse_color(text).ok()?.rgba8();
        let mut gradient = MultiGradient::new();
        gradient.set_nodes_json(r##"[{"value":2,"color":"#ff0000"},{"value":-1,"color":"#0000ff{alpha:0.5}"},{"value":2,"color":"#00ff00"}]"##, resolve).unwrap();
        assert_eq!(gradient.size(), 2);
        assert_eq!(gradient.color_at(1), Some(color(255, 0, 0)));
        assert_eq!(gradient.color_at(0).unwrap().a, 128);
        let before = gradient.clone();
        for invalid in [
            "{",
            "{}",
            r##"[{"value":0,"color":"#ffffff"},{"value":1}]"##,
            r##"[{"value":0,"color":"#oops"}]"##,
        ] {
            assert!(gradient.set_nodes_json(invalid, resolve).is_err());
            assert_eq!(gradient, before);
        }
        let json: serde_json::Value = serde_json::from_str(&gradient.get_nodes_json()).unwrap();
        assert_eq!(json[0]["color"], "#0000ff{alpha:0.5}");
        assert_eq!(json[0]["b"], 255);
        let mut restored = MultiGradient::new();
        restored
            .set_nodes_json(&gradient.get_nodes_json(), resolve)
            .unwrap();
        assert_eq!(gradient, restored);
    }

    #[test]
    fn multi_gradient_reads_ldom_gradnodes() {
        let mut node = LDom2Node::new();
        node.set_tag_name("multi_grad");
        let gradient_node = node.append_child("gradnode");
        gradient_node.set_str_attr("par", "2.5");
        gradient_node.append_child("col").set_value("#0a141e");

        let mut gradient = MultiGradient::new();
        assert_eq!(
            gradient.read_from_ldom2_node(&node, |value| {
                cuemol_gfx::parse_color(value).ok()?.rgba8()
            }),
            1
        );
        assert_eq!(gradient.value_at(0), Some(2.5));
        assert_eq!(gradient.color_at(0), Some(color(10, 20, 30)));
    }

    #[test]
    fn renderer_and_test_renderer_match_qif_defaults() {
        let mut renderer = Renderer::new("sample");
        assert_eq!(renderer.uid(), Uid::new(INVALID_UID));
        assert_eq!(renderer.scene_uid(), Uid::new(INVALID_UID));
        assert_eq!(renderer.client_obj_uid(), Uid::new(INVALID_UID));
        assert_eq!(renderer.type_name(), "sample");
        assert_eq!(renderer.name(), "");
        assert!(renderer.is_visible());
        assert!(!renderer.is_ui_locked());
        assert_eq!(renderer.default_material(), "");
        assert_eq!(renderer.default_alpha(), 1.0);
        assert_eq!(renderer.xform_matrix(), Matrix4::IDENTITY);
        assert_eq!(renderer.group_name(), "");
        assert_eq!(renderer.edge_line_type(), Renderer::ELT_NONE);
        assert_eq!(renderer.edge_line_width(), 0.01);
        assert_eq!(
            renderer.edge_line_color().rgba8(),
            SolidColor::from_rgb(0.0, 0.0, 0.0).rgba8()
        );
        assert!(!renderer.has_center());
        renderer.set_locked(true);
        renderer.set_material("matte");
        renderer.set_alpha(0.75);
        renderer.set_group("main");
        renderer.set_egtype(Renderer::ELT_EDGES);
        renderer.set_eglinew(0.02);
        renderer.set_egcolor(SolidColor::from_rgb(1.0, 0.0, 0.0));
        assert!(renderer.locked());
        assert_eq!(renderer.material(), "matte");
        assert_eq!(renderer.alpha(), 0.75);
        assert_eq!(renderer.group(), "main");
        assert_eq!(renderer.egtype(), Renderer::ELT_EDGES);
        assert_eq!(renderer.eglinew(), 0.02);
        assert_eq!(
            renderer.egcolor().rgba8(),
            SolidColor::from_rgb(1.0, 0.0, 0.0).rgba8()
        );
        renderer.set_property_str("bondw", "0.2");
        let style = renderer.to_style_node();
        assert_eq!(style.tag_name(), "style");
        assert_eq!(style.type_name(), "renderer");
        assert_eq!(style.str_attr("visible").to_string(), "true");
        assert_eq!(style.str_attr("locked").to_string(), "true");
        assert_eq!(style.str_attr("material").to_string(), "matte");
        assert_eq!(style.str_attr("alpha").to_string(), "0.75");
        assert_eq!(style.str_attr("egtype").to_string(), "1");
        assert_eq!(style.str_attr("bondw").to_string(), "0.2");
        assert!(style.str_attr("name").is_empty());
        assert!(style.str_attr("group").is_empty());
        renderer.set_center(Vector4::new3(1.0, 2.0, 3.0));
        assert!(renderer.has_center());
        assert_eq!(renderer.center(), Vector4::new3(1.0, 2.0, 3.0));
        renderer.clear_center();
        assert!(!renderer.has_center());
        assert_eq!(renderer.style_generation(), 0);
        renderer.apply_styles("default");
        assert_eq!(renderer.style(), "default");
        assert_eq!(renderer.style_generation(), 1);
        renderer.reapply_style();
        assert_eq!(renderer.style_generation(), 2);

        let test = TestRenderer::new();
        assert_eq!(test.type_name(), "test");
        assert_eq!(
            test.color1().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 1.0).rgba8()
        );
        assert!(!renderer.is_hit_test_supported());

        let test = TestRenderer::with_uid(Uid::new(9));
        let mut context = cuemol_gfx::HittestContext::new();
        context.add_hit(Uid::new(9), 0.5, vec![4]);
        let mut hit_data = cuemol_gfx::HitData::new();
        hit_data.create_nearest(&context);
        assert!(test.is_hit_test_supported());
        assert_eq!(
            test.interp_hit(&hit_data),
            "\"faceid\":4,\"objtype\":\"Test\","
        );
    }

    #[test]
    fn renderer_property_events_include_renderer_target_and_scene_source() {
        let mut renderer = Renderer::with_uid("simple", Uid::new(9));
        renderer.set_scene_uid(Uid::new(4));
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        let id = renderer.add_listener(shared_event_listener(move |event: &mut RendererEvent| {
            listener_events.lock().unwrap().push(event.clone());
        }));

        renderer.set_name("renderer");
        renderer.set_default_alpha(0.5);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| event.event_type() == RendererEvent::RNE_PROPCHG));
        assert!(events.iter().all(|event| event.target() == 9));
        assert!(events.iter().all(|event| event.source() == 4));
        assert_eq!(events[0].descr(), "name");
        assert_eq!(events[1].descr(), "default_alpha");
        drop(events);
        assert!(renderer.remove_listener(id));
    }

    #[test]
    fn rend_group_and_prop_edit_info_match_reference_defaults() {
        let mut group = RendGroup::new();
        assert_eq!(group.type_name(), "*group");
        assert_eq!(group.renderer().type_name(), "*group");
        assert!(group.script_string().equals("Renderer group"));
        assert!(group.is_compatible_obj());
        assert!(!group.is_ui_collapsed());
        group.set_ui_collapsed(true);
        assert!(group.ui_collapsed());
        assert!(group.is_visible());
        assert!(!group.is_ui_locked());
        assert!(!group.is_hit_test_supported());
        assert_eq!(group.group_name(), "");

        let mut edit = PropEditInfo::new();
        assert_eq!(edit.target_uid(), INVALID_UID);
        edit.set_target_uid(42);
        edit.set_prop_name("myProp");
        assert_eq!(edit.target_uid(), 42);
        assert_eq!(edit.prop_name(), "myProp");
        edit.setup(INVALID_UID, "someProp", "old", "new");
        assert!(!edit.undo());
        assert!(!edit.redo());
    }

    #[test]
    fn prop_edit_info_applies_values_defaults_and_nested_properties() {
        let mut properties = MapPropContainer::new();
        properties.set_root_uid(42);
        properties.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            7.into(),
        );
        assert!(properties.set_prop_int("count", 9));
        assert!(properties.reset_property(&"count".into()));
        let target: Rc<RefCell<dyn LPropSupport>> = Rc::new(RefCell::new(properties));

        let mut reset_event = LPropEvent::new("count");
        reset_event.set_old_value(&9.into());
        reset_event.set_new_default(true);
        let mut reset_edit = PropEditInfo::new();
        reset_edit.setup_from_event(target.clone(), &reset_event);
        assert!(reset_edit.undo());
        assert_eq!(
            target
                .borrow()
                .get_property(&"count".into())
                .unwrap()
                .get_int_value()
                .unwrap(),
            9
        );
        assert!(!target.borrow().is_prop_default(&"count".into()));
        assert!(reset_edit.redo());
        assert_eq!(
            target
                .borrow()
                .get_property(&"count".into())
                .unwrap()
                .get_int_value()
                .unwrap(),
            7
        );
        assert!(target.borrow().is_prop_default(&"count".into()));

        let mut child = MapPropContainer::new();
        child.add_property(
            "value",
            PropSpec::writable("real").with_default(),
            1.5.into(),
        );
        let mut root = MapPropContainer::new();
        root.set_root_uid(77);
        root.add_property(
            "child",
            PropSpec::writable("object"),
            LVariant::Object(Box::new(PropObject::new(child))),
        );
        assert!(root.set_nested_property(&"child.value".into(), &2.25.into()));
        let nested_target: Rc<RefCell<dyn LPropSupport>> = Rc::new(RefCell::new(root));
        let mut nested_edit = PropEditInfo::new();
        nested_edit.setup_with_target(
            nested_target.clone(),
            "child.value",
            1.5.into(),
            2.25.into(),
        );
        assert!(nested_edit.undo());
        assert_eq!(
            nested_target
                .borrow()
                .get_nested_property(&"child.value".into())
                .unwrap()
                .get_real_value()
                .unwrap(),
            1.5
        );
        assert!(nested_edit.redo());
        assert_eq!(
            nested_target
                .borrow()
                .get_nested_property(&"child.value".into())
                .unwrap()
                .get_real_value()
                .unwrap(),
            2.25
        );
    }

    #[test]
    fn view_input_config_roundtrips_bindings_like_reference() {
        let mut config = ViewInputConfig::new();
        assert_eq!(config.hit_prec(), 10.0);
        assert_eq!(config.get_tb_rad(), 0.8);
        assert_eq!(config.get_conf_rot_x().as_str(), "LBUTTON|MOUSE_X");
        assert_eq!(config.get_conf_rot_y().as_str(), "LBUTTON|MOUSE_Y");
        assert_eq!(config.get_conf_tra_x().as_str(), "RBUTTON|MOUSE_X");
        assert_eq!(
            config.get_conf_zoom().as_str(),
            "LBUTTON|SHIFT|MOUSE_Y,WHEEL1,WHEEL2"
        );
        assert_eq!(config.get_conf_slab().as_str(), "LBUTTON|SHIFT|MOUSE_X");
        assert!(config.get_conf_dist().is_empty());
        config.set_style("default");
        assert_eq!(config.style(), "default");
        config.set_tb_rad(1.5);
        config.set_hit_prec(0.05);
        assert_eq!(config.tb_rad(), 1.5);
        assert_eq!(config.hit_prec(), 0.05);

        config.set_binding(ViewInputConfig::VIEW_ROTX, "LBUTTON|MOUSE_X");
        assert!(
            config
                .binding(ViewInputConfig::VIEW_ROTX)
                .index_of("LBUTTON")
                >= 0
        );
        config.set_binding(ViewInputConfig::VIEW_ROTY, "RBUTTON|MOUSE_Y");
        config.remove_bindings(ViewInputConfig::VIEW_ROTY);
        assert!(config.binding(ViewInputConfig::VIEW_ROTY).is_empty());
        config.set_binding(ViewInputConfig::VIEW_ROTZ, "SHIFT|MOUSE_X");
        config.set_binding(ViewInputConfig::VIEW_ROTZ, "");
        assert!(config.binding(ViewInputConfig::VIEW_ROTZ).is_empty());

        config.set_conf_rot_x("LBUTTON|MOUSE_X");
        assert!(config.conf_rot_x().index_of("MOUSE_X") >= 0);
        config.set_conf_zoom("WHEEL1,CTRL|MOUSE_Y");
        assert!(config.conf_zoom().index_of("WHEEL1") >= 0);
    }
}
