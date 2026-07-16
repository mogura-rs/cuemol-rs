use crate::{ObjectEvent, Renderer, SceneEvent, ScrEventManagerHandle, StyleManager};
use cuemol_core::{CueMolError, EventCaster, Result, SharedEventListener, Uid, UidGenerator};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjExtData {
    class_name: String,
    values: BTreeMap<String, String>,
}

impl ObjExtData {
    pub fn new(class_name: impl Into<String>) -> Self {
        Self {
            class_name: class_name.into(),
            values: BTreeMap::new(),
        }
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn set_value(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values.insert(name.into(), value.into());
    }

    pub fn write_qdf_data(&self) -> BTreeMap<String, String> {
        self.values.clone()
    }

    pub fn read_qdf_data(&mut self, values: &BTreeMap<String, String>) {
        self.values.clone_from(values);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject<T> {
    uid: Uid,
    scene_uid: Option<Uid>,
    class_name: String,
    name: String,
    visible: bool,
    ui_locked: bool,
    modified: bool,
    ui_collapsed: bool,
    source: String,
    source_type: String,
    alt_source: String,
    xform_matrix: cuemol_core::Matrix4,
    ui_order: i32,
    renderers: BTreeMap<Uid, Renderer>,
    ext_data: BTreeMap<String, ObjExtData>,
    uid_generator: UidGenerator,
    event_caster: Arc<EventCaster<ObjectEvent>>,
    scene_event_caster: Option<Arc<EventCaster<SceneEvent>>>,
    event_manager: Option<ScrEventManagerHandle>,
    data: T,
}

impl<T> SceneObject<T> {
    pub fn new(uid: Uid, data: T) -> Self {
        Self {
            uid,
            scene_uid: None,
            class_name: "Object".to_string(),
            name: String::new(),
            visible: true,
            ui_locked: false,
            modified: false,
            ui_collapsed: false,
            source: String::new(),
            source_type: String::new(),
            alt_source: String::new(),
            xform_matrix: cuemol_core::Matrix4::IDENTITY,
            ui_order: uid.get() as i32,
            renderers: BTreeMap::new(),
            ext_data: BTreeMap::new(),
            uid_generator: UidGenerator::starting_after(uid.get()),
            event_caster: Arc::new(EventCaster::new()),
            scene_event_caster: None,
            event_manager: None,
            data,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn scene_uid(&self) -> Option<Uid> {
        self.scene_uid
    }

    pub fn scene_id(&self) -> Option<Uid> {
        self.scene_uid()
    }

    pub fn get_scene_id(&self) -> Option<Uid> {
        self.scene_uid()
    }

    pub fn get_scene(&self) -> Option<Uid> {
        self.scene_uid()
    }

    pub fn set_scene_uid(&mut self, scene_uid: Option<Uid>) {
        self.scene_uid = scene_uid;
        for renderer in self.renderers.values_mut() {
            renderer.set_scene_uid(scene_uid.unwrap_or(Uid::new(0)));
        }
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn set_class_name(&mut self, class_name: impl Into<String>) {
        self.class_name = class_name.into();
    }

    pub(crate) fn attach_scene_event_caster(&mut self, caster: Arc<EventCaster<SceneEvent>>) {
        self.scene_event_caster = Some(caster);
    }

    pub(crate) fn attach_event_manager(&mut self, manager: ScrEventManagerHandle) {
        self.event_manager = Some(manager.clone());
        for renderer in self.renderers.values_mut() {
            renderer.attach_event_manager(manager.clone());
        }
    }

    pub(crate) fn attach_uid_generator(&mut self, generator: UidGenerator) {
        generator.ensure_after(self.uid);
        for uid in self.renderers.keys() {
            generator.ensure_after(*uid);
        }
        self.uid_generator = generator;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.fire_property_changed("name");
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn visible(&self) -> bool {
        self.is_visible()
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.fire_property_changed("visible");
    }

    pub fn is_ui_locked(&self) -> bool {
        self.ui_locked
    }

    pub fn locked(&self) -> bool {
        self.is_ui_locked()
    }

    pub fn set_ui_locked(&mut self, locked: bool) {
        self.ui_locked = locked;
        self.fire_property_changed("locked");
    }

    pub fn is_ui_collapsed(&self) -> bool {
        self.ui_collapsed
    }

    pub fn ui_collapsed(&self) -> bool {
        self.is_ui_collapsed()
    }

    pub fn set_ui_collapsed(&mut self, collapsed: bool) {
        self.ui_collapsed = collapsed;
        self.fire_property_changed("ui_collapsed");
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn get_modified_flag(&self) -> bool {
        self.modified()
    }

    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn get_source(&self) -> &str {
        self.source()
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
        self.fire_property_changed("src");
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn get_source_type(&self) -> &str {
        self.source_type()
    }

    pub fn set_source_type(&mut self, source_type: impl Into<String>) {
        self.source_type = source_type.into();
        self.fire_property_changed("srctype");
    }

    pub fn alt_source(&self) -> &str {
        &self.alt_source
    }

    pub fn get_alt_source(&self) -> &str {
        self.alt_source()
    }

    pub fn alt_src(&self) -> &str {
        self.alt_source()
    }

    pub fn set_alt_source(&mut self, value: impl Into<String>) {
        self.alt_source = value.into();
        self.fire_property_changed("alt_src");
    }

    pub fn set_alt_src(&mut self, value: impl Into<String>) {
        self.set_alt_source(value);
    }

    pub fn xform_matrix(&self) -> cuemol_core::Matrix4 {
        self.xform_matrix
    }

    pub fn get_xform_matrix(&self) -> cuemol_core::Matrix4 {
        self.xform_matrix()
    }

    pub fn xform_mat(&self) -> cuemol_core::Matrix4 {
        self.xform_matrix()
    }

    pub fn set_xform_matrix(&mut self, value: cuemol_core::Matrix4) {
        self.xform_matrix = value;
        self.fire_property_changed("xformMat");
    }

    pub fn set_xform_mat(&mut self, value: cuemol_core::Matrix4) {
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

    pub fn create_renderer(&mut self, type_name: impl Into<String>) -> Uid {
        let uid = self.uid_generator.next_uid();
        let mut renderer = Renderer::with_uid(type_name, uid);
        renderer.set_client_obj_uid(self.uid);
        renderer.set_scene_uid(self.scene_uid.unwrap_or(Uid::new(0)));
        if let Some(caster) = self.scene_event_caster.as_ref() {
            renderer.attach_scene_event_caster(Arc::clone(caster));
        }
        if let Some(manager) = self.event_manager.as_ref() {
            renderer.attach_event_manager(manager.clone());
        }
        self.renderers.insert(uid, renderer);
        self.fire_renderer_scene_event(SceneEvent::SCE_REND_ADDED, uid);
        uid
    }

    pub fn create_preset_renderer(
        &mut self,
        preset_name: impl Into<String>,
        grp_name: impl Into<String>,
        name_prefix: impl Into<String>,
    ) -> Result<Uid> {
        let mut styles = StyleManager::new();
        styles.load_builtin_default_styles(0);
        self.create_preset_renderer_with_style(&styles, 0, preset_name, grp_name, name_prefix)
    }

    pub fn create_preset_renderer_with_style(
        &mut self,
        styles: &StyleManager,
        scope_id: u64,
        preset_name: impl Into<String>,
        grp_name: impl Into<String>,
        name_prefix: impl Into<String>,
    ) -> Result<Uid> {
        let preset_name = preset_name.into();
        let group_name = grp_name.into();
        let name_prefix = name_prefix.into();
        let node = styles
            .style_node(scope_id, &preset_name)
            .ok_or_else(|| CueMolError::NotFound(format!("renderer preset {preset_name}")))?;
        if !node.type_name().ends_with("-rendpreset") {
            return Err(CueMolError::invalid_argument(format!(
                "{preset_name} is not a renderer preset ({})",
                node.type_name()
            )));
        }

        let group_uid = self.uid_generator.next_uid();
        let mut group = Renderer::with_uid("*group", group_uid);
        group.set_name(group_name.clone());
        for child in node
            .children()
            .iter()
            .filter(|child| child.tag_name() == "renderer")
        {
            let type_name = child.type_name();
            if type_name.is_empty() {
                continue;
            }
            let uid = self.create_renderer(type_name);
            if let Some(renderer) = self.renderers.get_mut(&uid) {
                renderer.set_name(format!("{name_prefix}{type_name}"));
                renderer.set_group_name(group_name.clone());
                renderer.read_properties_from_node(child);
            }
        }
        if !self.attach_renderer(group) {
            return Err(CueMolError::invalid_argument(
                "renderer preset group UID collision",
            ));
        }
        Ok(group_uid)
    }

    pub fn attach_renderer(&mut self, mut renderer: Renderer) -> bool {
        let uid = renderer.uid();
        if self.renderers.contains_key(&uid) {
            return false;
        }
        self.uid_generator.ensure_after(uid);
        renderer.set_client_obj_uid(self.uid);
        renderer.set_scene_uid(self.scene_uid.unwrap_or(Uid::new(0)));
        if let Some(caster) = self.scene_event_caster.as_ref() {
            renderer.attach_scene_event_caster(Arc::clone(caster));
        }
        if let Some(manager) = self.event_manager.as_ref() {
            renderer.attach_event_manager(manager.clone());
        }
        self.renderers.insert(uid, renderer);
        self.fire_renderer_scene_event(SceneEvent::SCE_REND_ADDED, uid);
        true
    }

    pub fn renderer(&self, uid: Uid) -> Option<&Renderer> {
        self.renderers.get(&uid)
    }

    pub fn renderer_mut(&mut self, uid: Uid) -> Option<&mut Renderer> {
        self.renderers.get_mut(&uid)
    }

    pub fn get_renderer(&self, uid: Uid) -> Option<&Renderer> {
        self.renderer(uid)
    }

    pub fn renderer_by_type(&self, type_name: &str) -> Option<&Renderer> {
        self.renderers
            .values()
            .find(|renderer| renderer.type_name() == type_name)
    }

    pub fn get_renderer_by_type(&self, type_name: &str) -> Option<&Renderer> {
        self.renderer_by_type(type_name)
    }

    pub fn renderer_by_name(&self, name: &str) -> Option<&Renderer> {
        self.renderers
            .values()
            .find(|renderer| renderer.name() == name)
    }

    pub fn get_renderer_by_name(&self, name: &str) -> Option<&Renderer> {
        self.renderer_by_name(name)
    }

    pub fn get_rend_by_name(&self, name: &str) -> Option<&Renderer> {
        self.renderer_by_name(name)
    }

    pub fn renderer_by_name_type(&self, name: &str, type_name: &str) -> Option<&Renderer> {
        self.renderers
            .values()
            .find(|renderer| renderer.name() == name && renderer.type_name() == type_name)
    }

    pub fn get_renderer_by_name_type(&self, name: &str, type_name: &str) -> Option<&Renderer> {
        self.renderer_by_name_type(name, type_name)
    }

    pub fn destroy_renderer(&mut self, uid: Uid) -> bool {
        if !self.renderers.contains_key(&uid) {
            return false;
        }
        self.fire_renderer_scene_event(SceneEvent::SCE_REND_REMOVING, uid);
        self.renderers.remove(&uid).is_some()
    }

    pub fn rend_count(&self) -> usize {
        self.renderers.len()
    }

    pub fn get_rend_count(&self) -> usize {
        self.rend_count()
    }

    pub fn renderer_by_index(&self, index: usize) -> Option<&Renderer> {
        self.sorted_renderers().get(index).copied()
    }

    pub fn get_renderer_by_index(&self, index: usize) -> Option<&Renderer> {
        self.renderer_by_index(index)
    }

    pub fn renderer_uids(&self) -> Vec<Uid> {
        let mut renderers = self.renderers.values().collect::<Vec<_>>();
        renderers.sort_by_key(|renderer| (renderer.ui_order(), renderer.uid()));
        renderers.into_iter().map(crate::Renderer::uid).collect()
    }

    pub fn get_rend_array(&self) -> Vec<&Renderer> {
        self.sorted_renderers()
    }

    pub fn rend_uid_list(&self) -> String {
        self.renderer_uids()
            .into_iter()
            .map(|uid| uid.get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn rend_uids(&self) -> String {
        self.rend_uid_list()
    }

    pub fn get_rend_uid_list(&self) -> String {
        self.rend_uid_list()
    }

    pub fn flat_renderer_list_json(&self) -> String {
        let entries = self
            .sorted_renderers()
            .into_iter()
            .filter(|renderer| renderer.type_name() != "*group")
            .map(renderer_data_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_flat_rend_list_json(&self) -> String {
        self.flat_renderer_list_json()
    }

    pub fn grouped_renderer_list_json(&self) -> String {
        let entries = self
            .sorted_renderers()
            .into_iter()
            .filter(|renderer| renderer.group_name().is_empty())
            .map(|renderer| {
                let mut json = renderer_data_json(renderer);
                if renderer.type_name() == "*group" {
                    let children = self
                        .sorted_renderers()
                        .into_iter()
                        .filter(|child| child.group_name() == renderer.name())
                        .map(renderer_data_json)
                        .collect::<Vec<_>>()
                        .join(",");
                    json.pop();
                    json.push_str(&format!(",\"childNodes\":[{children}]}}"));
                }
                json
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_grouped_rend_list_json(&self) -> String {
        self.grouped_renderer_list_json()
    }

    pub fn filtered_renderer_list_json(&self, group_filter: &str) -> String {
        let entries = self
            .sorted_renderers()
            .into_iter()
            .filter(|renderer| renderer.group_name() == group_filter)
            .map(renderer_data_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_filtered_rend_list_json(&self, group_filter: &str) -> String {
        self.filtered_renderer_list_json(group_filter)
    }

    fn sorted_renderers(&self) -> Vec<&Renderer> {
        let mut renderers = self.renderers.values().collect::<Vec<_>>();
        renderers.sort_by_key(|renderer| (renderer.ui_order(), renderer.uid()));
        renderers
    }

    pub fn search_compatible_renderer_names(&self) -> String {
        "simple,cpk,ballstick,ribbon,tube".to_string()
    }

    pub fn ext_data_names(&self) -> String {
        self.ext_data.keys().cloned().collect::<Vec<_>>().join(",")
    }

    pub fn get_ext_data_names(&self) -> String {
        self.ext_data_names()
    }

    pub fn ext_data(&self, name: &str) -> Option<&ObjExtData> {
        self.ext_data.get(name)
    }

    pub fn ext_data_mut(&mut self, name: &str) -> Option<&mut ObjExtData> {
        self.ext_data.get_mut(name)
    }

    pub fn get_ext_data(&self, name: &str) -> Option<&ObjExtData> {
        self.ext_data(name)
    }

    pub fn get_create_ext_data(&mut self, name: impl Into<String>) -> &mut ObjExtData {
        let name = name.into();
        self.ext_data
            .entry(name.clone())
            .or_insert_with(|| ObjExtData::new(name))
    }

    pub fn set_ext_data(&mut self, data: ObjExtData) {
        self.ext_data.insert(data.class_name.clone(), data);
    }

    pub fn remove_ext_data(&mut self, name: &str) -> bool {
        self.ext_data.remove(name).is_some()
    }

    pub fn force_embed(&mut self) {
        if !self.source.starts_with("datachunk:") {
            self.source = "datachunk:".to_string();
            self.alt_source.clear();
        }
        self.source_type.clear();
        self.fire_property_changed("src");
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    pub fn add_listener(&self, listener: SharedEventListener<ObjectEvent>) -> i32 {
        self.event_caster.add(listener).map_or(-1, |id| id as i32)
    }

    pub fn remove_listener(&self, id: i32) -> bool {
        id >= 0 && self.event_caster.remove(id as u64).is_some()
    }

    pub fn fire_object_event(&self, event: &mut ObjectEvent) {
        event.set_source(self.scene_uid.map_or(0, Uid::get));
        self.event_caster.replica_fire(event);
        if let Some(manager) = self.event_manager.as_ref() {
            let _ = manager.with(|manager| manager.fire_object_event(event));
        }
    }

    fn fire_property_changed(&self, property_name: &str) {
        let mut event = ObjectEvent::new();
        event.set_event_type(ObjectEvent::OBE_PROPCHG);
        event.set_target(self.uid.get());
        event.set_descr(property_name);
        self.fire_object_event(&mut event);
    }

    fn fire_renderer_scene_event(&self, event_type: i32, renderer_uid: Uid) {
        let Some(caster) = self.scene_event_caster.as_ref() else {
            return;
        };
        let mut event = SceneEvent::new();
        event.set_event_type(event_type);
        event.set_source(self.scene_uid.map_or(0, Uid::get));
        event.set_target(renderer_uid.get());
        caster.replica_fire(&mut event);
    }
}

impl<T> fmt::Display for SceneObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object(name={}, UID={})", self.name, self.uid.get())
    }
}

fn renderer_data_json(renderer: &Renderer) -> String {
    let prefix = format!(
        "{{\"name\":\"{}\",\"type\":\"{}\",\"ui_order\":{},\"visible\":{},\"locked\":{}",
        escape_json(renderer.name()),
        escape_json(renderer.type_name()),
        renderer.ui_order(),
        renderer.is_visible(),
        renderer.is_ui_locked()
    );
    if renderer.type_name() == "*group" {
        format!(
            "{prefix},\"ui_collapsed\":false,\"ID\":{}}}",
            renderer.uid().get()
        )
    } else {
        format!("{prefix},\"ID\":{}}}", renderer.uid().get())
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

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::shared_event_listener;
    use std::sync::Mutex;

    #[test]
    fn object_defaults_match_qsys_reference() {
        let object = SceneObject::new(Uid::new(1), ());
        assert_eq!(object.uid(), Uid::new(1));
        assert_eq!(object.scene_uid(), None);
        assert!(object.is_visible());
        assert!(!object.is_ui_locked());
        assert!(!object.modified());
        assert!(!object.is_ui_collapsed());
        assert_eq!(object.class_name(), "Object");
        assert_eq!(object.name(), "");
        assert_eq!(object.source(), "");
    }

    #[test]
    fn force_embed_sets_datachunk_source() {
        let mut object = SceneObject::new(Uid::new(1), ());
        object.set_source("/path/to/file.pdb");
        object.set_source_type("pdb");
        object.set_alt_source("fallback.pdb");
        object.force_embed();
        assert_eq!(object.source(), "datachunk:");
        assert_eq!(object.alt_source(), "");
        assert_eq!(object.source_type(), "");
    }

    #[test]
    fn object_property_events_include_object_target_and_scene_source() {
        let mut object = SceneObject::new(Uid::new(7), ());
        object.set_scene_uid(Some(Uid::new(3)));
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        let id = object.add_listener(shared_event_listener(move |event: &mut ObjectEvent| {
            listener_events.lock().unwrap().push(event.clone());
        }));

        object.set_name("object");
        object.set_visible(false);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| event.event_type() == ObjectEvent::OBE_PROPCHG));
        assert!(events.iter().all(|event| event.target() == 7));
        assert!(events.iter().all(|event| event.source() == 3));
        assert_eq!(events[0].descr(), "name");
        assert_eq!(events[1].descr(), "visible");
        drop(events);
        assert!(object.remove_listener(id));
    }

    #[test]
    fn renderer_list_json_supports_flat_and_grouped_reference_shapes() {
        let mut object = SceneObject::new(Uid::new(1), ());
        let mut group = Renderer::with_uid("*group", Uid::new(2));
        group.set_name("grp");
        group.set_ui_order(10);
        assert!(object.attach_renderer(group));

        let mut child = Renderer::with_uid("simple", Uid::new(3));
        child.set_name("child");
        child.set_group_name("grp");
        child.set_ui_order(20);
        assert!(object.attach_renderer(child));

        let mut standalone = Renderer::with_uid("cpk", Uid::new(4));
        standalone.set_name("solo");
        standalone.set_ui_order(5);
        assert!(object.attach_renderer(standalone));

        let flat = object.get_flat_rend_list_json();
        assert!(!flat.contains("*group"));
        assert!(flat.find("solo").unwrap() < flat.find("child").unwrap());
        let grouped = object.get_grouped_rend_list_json();
        assert!(grouped.contains("\"type\":\"*group\""));
        assert!(grouped.contains("\"childNodes\":[{\"name\":\"child\""));
        let filtered = object.get_filtered_rend_list_json("grp");
        assert!(filtered.contains("\"name\":\"child\""));
        assert!(!filtered.contains("solo"));
        assert_eq!(object.rend_uids(), "4,2,3");
        assert_eq!(object.get_renderer_by_index(0).unwrap().name(), "solo");
    }

    #[test]
    fn object_extension_data_round_trips_named_values() {
        let mut object = SceneObject::new(Uid::new(1), ());
        let crystal = object.get_create_ext_data("CrystalInfo");
        crystal.set_value("a", "40.96");
        crystal.set_value("nsg", "4");
        assert_eq!(object.get_ext_data_names(), "CrystalInfo");
        assert_eq!(
            object.get_ext_data("CrystalInfo").unwrap().value("a"),
            Some("40.96")
        );
        assert!(object.remove_ext_data("CrystalInfo"));
        assert!(object.get_ext_data("CrystalInfo").is_none());
    }

    #[test]
    fn preset_renderer_builds_group_and_all_children_from_builtin_style() {
        let mut object = SceneObject::new(Uid::new(1), ());
        let group_uid = object
            .create_preset_renderer("Default1RendPreset", "default", "preset_")
            .unwrap();
        let group = object.renderer(group_uid).unwrap();
        assert_eq!(group.type_name(), "*group");
        assert_eq!(group.name(), "default");

        let children = object
            .get_rend_array()
            .into_iter()
            .filter(|renderer| renderer.group_name() == "default")
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].type_name(), "ribbon");
        assert_eq!(children[0].name(), "preset_ribbon");
        assert_eq!(children[0].style_names(), "DefaultRibbon,DefaultHSCPaint");
        assert_eq!(children[0].property_str("sel"), Some("protein"));
        assert_eq!(children[1].type_name(), "nucl");
        assert_eq!(children[2].type_name(), "ballstick");
    }
}
