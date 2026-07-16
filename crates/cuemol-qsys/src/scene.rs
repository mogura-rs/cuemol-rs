use crate::{Camera, SceneEvent, SceneObject, ScrEventManagerHandle, UndoManagerHandle, View};
use cuemol_core::{
    parse_ldom2_xml, write_ldom2_xml, EventCaster, LDom2Node, SharedEventListener, Uid,
    UidGenerator,
};
use cuemol_gfx::SolidColor;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AAMethod {
    None,
    #[default]
    Fxaa,
    Smaa,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene<T> {
    uid: Uid,
    name: String,
    source: String,
    source_type: String,
    base_path: String,
    qsc_options: Option<LDom2Node>,
    bg_color: SolidColor,
    ao_enabled: bool,
    ao_radius: f64,
    ao_intensity: f64,
    ao_slices: i32,
    ao_steps: i32,
    ao_half_res: bool,
    aa_method: AAMethod,
    aa_jitter_level: i32,
    onload: String,
    use_colproof: bool,
    icc_filename: String,
    icc_intent: i32,
    event_caster: Arc<EventCaster<SceneEvent>>,
    event_manager: Option<ScrEventManagerHandle>,
    undo_manager: UndoManagerHandle,
    modified: bool,
    just_created: bool,
    objects: BTreeMap<Uid, SceneObject<T>>,
    views: BTreeMap<Uid, View>,
    cameras: BTreeMap<String, Camera>,
    uid_generator: UidGenerator,
    active_object_uid: Option<Uid>,
    active_renderer_uid: Option<Uid>,
    active_view_uid: Option<Uid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimMgrHandle {
    scene_uid: Uid,
}

impl AnimMgrHandle {
    pub fn new(scene_uid: Uid) -> Self {
        Self { scene_uid }
    }

    pub fn scene_uid(&self) -> Uid {
        self.scene_uid
    }
}

impl<T> Scene<T> {
    pub fn new(uid: Uid, name: impl Into<String>) -> Self {
        Self::with_uid_generator(uid, name, UidGenerator::starting_after(uid.get()))
    }

    fn with_uid_generator(uid: Uid, name: impl Into<String>, uid_generator: UidGenerator) -> Self {
        Self {
            uid,
            name: name.into(),
            source: String::new(),
            source_type: String::new(),
            base_path: String::new(),
            qsc_options: None,
            bg_color: SolidColor::from_rgb(0.0, 0.0, 0.0),
            ao_enabled: false,
            ao_radius: 4.0,
            ao_intensity: 2.2,
            ao_slices: 9,
            ao_steps: 3,
            ao_half_res: false,
            aa_method: AAMethod::Fxaa,
            aa_jitter_level: 0,
            onload: String::new(),
            use_colproof: false,
            icc_filename: String::new(),
            icc_intent: 0,
            event_caster: Arc::new(EventCaster::new()),
            event_manager: None,
            undo_manager: UndoManagerHandle::new(),
            modified: false,
            just_created: true,
            objects: BTreeMap::new(),
            views: BTreeMap::new(),
            cameras: BTreeMap::new(),
            uid_generator,
            active_object_uid: None,
            active_renderer_uid: None,
            active_view_uid: None,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn attach_event_manager(&mut self, manager: ScrEventManagerHandle) {
        self.event_manager = Some(manager.clone());
        for object in self.objects.values_mut() {
            object.attach_event_manager(manager.clone());
        }
        for view in self.views.values_mut() {
            view.attach_event_manager(manager.clone());
        }
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.modified = true;
        self.fire_property_changed("name");
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

    pub fn set_source(&mut self, value: impl Into<String>) {
        self.source = value.into();
        self.modified = true;
        self.fire_property_changed("src");
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn get_source_type(&self) -> &str {
        self.source_type()
    }

    pub fn srctype(&self) -> &str {
        self.source_type()
    }

    pub fn set_source_type(&mut self, value: impl Into<String>) {
        self.source_type = value.into();
        self.modified = true;
        self.fire_property_changed("srctype");
    }

    pub fn base_path(&self) -> &str {
        &self.base_path
    }

    pub fn get_base_path(&self) -> &str {
        self.base_path()
    }

    pub fn base_path_qif(&self) -> &str {
        self.base_path()
    }

    pub fn set_base_path(&mut self, value: impl Into<String>) {
        self.base_path = value.into();
        self.fire_property_changed("base_path");
    }

    pub fn qsc_options(&self) -> Option<&LDom2Node> {
        self.qsc_options.as_ref()
    }

    pub fn set_qsc_options(&mut self, value: Option<LDom2Node>) {
        self.qsc_options = value;
    }

    pub fn bg_color(&self) -> &SolidColor {
        &self.bg_color
    }

    pub fn get_bg_color(&self) -> &SolidColor {
        self.bg_color()
    }

    pub fn bgcolor(&self) -> &SolidColor {
        self.bg_color()
    }

    pub fn set_bg_color(&mut self, value: SolidColor) {
        self.bg_color = value;
        self.modified = true;
        self.fire_property_changed("bgcolor");
    }

    pub fn is_ao_enabled(&self) -> bool {
        self.ao_enabled
    }

    pub fn set_ao_enabled(&mut self, value: bool) {
        self.ao_enabled = value;
        self.mark_render_option_changed("aoEnabled");
    }

    pub fn ao_radius(&self) -> f64 {
        self.ao_radius
    }

    pub fn get_ao_radius(&self) -> f64 {
        self.ao_radius()
    }

    pub fn set_ao_radius(&mut self, value: f64) {
        self.ao_radius = value;
        self.mark_render_option_changed("aoRadius");
    }

    pub fn ao_intensity(&self) -> f64 {
        self.ao_intensity
    }

    pub fn get_ao_intensity(&self) -> f64 {
        self.ao_intensity()
    }

    pub fn set_ao_intensity(&mut self, value: f64) {
        self.ao_intensity = value;
        self.mark_render_option_changed("aoIntensity");
    }

    pub fn ao_slices(&self) -> i32 {
        self.ao_slices
    }

    pub fn get_ao_slices(&self) -> i32 {
        self.ao_slices()
    }

    pub fn set_ao_slices(&mut self, value: i32) {
        self.ao_slices = value;
        self.mark_render_option_changed("aoSlices");
    }

    pub fn ao_steps(&self) -> i32 {
        self.ao_steps
    }

    pub fn get_ao_steps(&self) -> i32 {
        self.ao_steps()
    }

    pub fn set_ao_steps(&mut self, value: i32) {
        self.ao_steps = value;
        self.mark_render_option_changed("aoSteps");
    }

    pub fn is_ao_half_res(&self) -> bool {
        self.ao_half_res
    }

    pub fn set_ao_half_res(&mut self, value: bool) {
        self.ao_half_res = value;
        self.mark_render_option_changed("aoHalfRes");
    }

    pub fn aa_method(&self) -> AAMethod {
        self.aa_method
    }

    pub fn get_aa_method(&self) -> AAMethod {
        self.aa_method()
    }

    pub fn set_aa_method(&mut self, value: AAMethod) {
        self.aa_method = value;
        self.mark_render_option_changed("aa_method");
    }

    pub fn aa_jitter_level(&self) -> i32 {
        self.aa_jitter_level
    }

    pub fn get_aa_jitter_level(&self) -> i32 {
        self.aa_jitter_level()
    }

    pub fn set_aa_jitter_level(&mut self, value: i32) {
        self.aa_jitter_level = value;
        self.mark_render_option_changed("aaJitterLevel");
    }

    fn mark_render_option_changed(&mut self, property_name: &str) {
        self.modified = true;
        for view in self.views.values_mut() {
            view.invalidate();
        }
        self.fire_property_changed(property_name);
    }

    pub fn onload(&self) -> &str {
        &self.onload
    }

    pub fn set_onload(&mut self, value: impl Into<String>) {
        self.onload = value.into();
        self.fire_property_changed("onload");
    }

    pub fn use_colproof(&self) -> bool {
        self.use_colproof
    }

    pub fn is_use_colproof(&self) -> bool {
        self.use_colproof()
    }

    pub fn is_use_col_proof(&self) -> bool {
        self.use_colproof()
    }

    pub fn set_use_colproof(&mut self, value: bool) {
        self.use_colproof = value;
        self.fire_property_changed("use_colproof");
    }

    pub fn set_use_col_proof(&mut self, value: bool) {
        self.set_use_colproof(value);
    }

    pub fn icc_filename(&self) -> &str {
        &self.icc_filename
    }

    pub fn get_icc_file_name(&self) -> &str {
        self.icc_filename()
    }

    pub fn set_icc_filename(&mut self, value: impl Into<String>) {
        self.icc_filename = value.into();
        self.fire_property_changed("icc_filename");
    }

    pub fn set_icc_file_name(&mut self, value: impl Into<String>) {
        self.set_icc_filename(value);
    }

    pub fn icc_intent(&self) -> i32 {
        self.icc_intent
    }

    pub fn get_icc_intent(&self) -> i32 {
        self.icc_intent()
    }

    pub fn set_icc_intent(&mut self, value: i32) {
        self.icc_intent = value;
        self.fire_property_changed("icc_intent");
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, value: bool) {
        self.modified = value;
    }

    pub fn is_just_created(&self) -> bool {
        self.just_created
    }

    pub fn set_just_created(&mut self, value: bool) {
        self.just_created = value;
    }

    pub fn add_object(&mut self, name: impl Into<String>, data: T) -> Uid {
        let uid = self.uid_generator.next_uid();
        let mut object = SceneObject::new(uid, data);
        object.attach_uid_generator(self.uid_generator.clone());
        object.set_name(name);
        object.set_scene_uid(Some(self.uid));
        object.attach_scene_event_caster(Arc::clone(&self.event_caster));
        if let Some(manager) = self.event_manager.as_ref() {
            object.attach_event_manager(manager.clone());
        }
        self.objects.insert(uid, object);
        self.active_object_uid = Some(uid);
        self.modified = true;
        self.fire_scene_event_type(SceneEvent::SCE_OBJ_ADDED, uid, "");
        uid
    }

    pub fn insert_object(&mut self, mut object: SceneObject<T>) -> bool {
        let uid = object.uid();
        if self.objects.contains_key(&uid) {
            return false;
        }
        self.uid_generator.ensure_after(uid);
        object.attach_uid_generator(self.uid_generator.clone());
        object.set_scene_uid(Some(self.uid));
        object.attach_scene_event_caster(Arc::clone(&self.event_caster));
        if let Some(manager) = self.event_manager.as_ref() {
            object.attach_event_manager(manager.clone());
        }
        self.objects.insert(uid, object);
        self.active_object_uid = Some(uid);
        self.modified = true;
        self.fire_scene_event_type(SceneEvent::SCE_OBJ_ADDED, uid, "");
        true
    }

    pub fn object(&self, uid: Uid) -> Option<&SceneObject<T>> {
        self.objects.get(&uid)
    }

    pub fn get_object(&self, uid: Uid) -> Option<&SceneObject<T>> {
        self.object(uid)
    }

    pub fn object_mut(&mut self, uid: Uid) -> Option<&mut SceneObject<T>> {
        self.objects.get_mut(&uid)
    }

    pub fn object_by_name(&self, name: &str) -> Option<&SceneObject<T>> {
        self.objects.values().find(|object| object.name() == name)
    }

    pub fn get_object_by_name(&self, name: &str) -> Option<&SceneObject<T>> {
        self.object_by_name(name)
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn object_uids(&self) -> Vec<Uid> {
        let mut objects = self.objects.values().collect::<Vec<_>>();
        objects.sort_by_key(|object| (object.ui_order(), object.uid()));
        objects.into_iter().map(SceneObject::uid).collect()
    }

    pub fn get_obj_array(&self) -> Vec<&SceneObject<T>> {
        let mut objects = self.objects.values().collect::<Vec<_>>();
        objects.sort_by_key(|object| (object.ui_order(), object.uid()));
        objects
    }

    pub fn object_uid_list(&self) -> String {
        self.object_uids()
            .into_iter()
            .map(|uid| uid.get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn obj_uids(&self) -> String {
        self.object_uid_list()
    }

    pub fn get_obj_uid_list(&self) -> String {
        self.object_uid_list()
    }

    pub fn destroy_object(&mut self, uid: Uid) -> bool {
        if !self.objects.contains_key(&uid) {
            return false;
        }
        self.fire_scene_event_type(SceneEvent::SCE_OBJ_REMOVING, uid, "");
        let removed = self.objects.remove(&uid).is_some();
        if removed {
            if self.active_object_uid == Some(uid) {
                self.active_object_uid = None;
            }
            self.modified = true;
        }
        removed
    }

    pub fn destroy_all_objects(&mut self) {
        for uid in self.object_uids() {
            self.destroy_object(uid);
        }
    }

    pub fn clear_all_data(&mut self) {
        self.undo_manager
            .with_mut(|manager| manager.clear_all_info());
        self.destroy_all_objects();
        self.views.clear();
        self.cameras.clear();
        self.active_view_uid = None;
        self.active_renderer_uid = None;
    }

    pub fn clear_all_data_script(&mut self) {
        self.clear_all_data();
        self.fire_scene_event_type(SceneEvent::SCE_SCENE_CLEARALL, self.uid, "");
    }

    pub fn clear_all_data_scr(&mut self) {
        self.clear_all_data_script();
    }

    pub fn unloading(&self) {
        self.fire_scene_event_type(SceneEvent::SCE_SCENE_REMOVING, self.uid, "");
    }

    pub fn notify_loaded(&mut self) {
        self.just_created = false;
        self.fire_scene_event_type(SceneEvent::SCE_SCENE_ONLOADED, self.uid, "");
    }

    pub fn active_object_uid(&self) -> Option<Uid> {
        self.active_object_uid
    }

    pub fn active_obj_id(&self) -> Option<Uid> {
        self.active_object_uid()
    }

    pub fn get_active_obj_id(&self) -> Option<Uid> {
        self.active_object_uid()
    }

    pub fn set_active_obj_id(&mut self, uid: Option<Uid>) -> bool {
        self.set_active_object_uid(uid)
    }

    pub fn set_active_object_uid(&mut self, uid: Option<Uid>) -> bool {
        if uid.is_some_and(|uid| !self.objects.contains_key(&uid)) {
            return false;
        }
        self.active_object_uid = uid;
        true
    }

    pub fn active_renderer_uid(&self) -> Option<Uid> {
        self.active_renderer_uid
    }

    pub fn active_rend_id(&self) -> Option<Uid> {
        self.active_renderer_uid()
    }

    pub fn get_active_rend_id(&self) -> Option<Uid> {
        self.active_renderer_uid()
    }

    pub fn set_active_renderer_uid(&mut self, uid: Option<Uid>) {
        self.active_renderer_uid = uid;
    }

    pub fn set_active_rend_id(&mut self, uid: Option<Uid>) {
        self.set_active_renderer_uid(uid);
    }

    pub fn get_renderer(&self, uid: Uid) -> Option<&crate::Renderer> {
        self.objects
            .values()
            .find_map(|object| object.renderer(uid))
    }

    pub fn get_rend_by_name(&self, name: &str) -> Option<&crate::Renderer> {
        self.objects
            .values()
            .find_map(|object| object.renderer_by_name(name))
    }

    pub fn create_view(&mut self) -> Uid {
        let uid = self.uid_generator.next_uid();
        let mut view = View::new(uid, self.uid);
        if let Some(manager) = self.event_manager.as_ref() {
            view.attach_event_manager(manager.clone());
        }
        if self.views.is_empty() {
            view.set_active(true);
            self.active_view_uid = Some(uid);
        }
        self.views.insert(uid, view);
        self.fire_scene_event_type(SceneEvent::SCE_VIEW_ADDED, uid, "");
        uid
    }

    pub fn insert_view(&mut self, mut view: View) -> bool {
        let uid = view.uid();
        if self.views.contains_key(&uid) || view.scene_uid() != self.uid {
            return false;
        }
        self.uid_generator.ensure_after(uid);
        if self.views.is_empty() {
            view.set_active(true);
            self.active_view_uid = Some(uid);
        }
        if let Some(manager) = self.event_manager.as_ref() {
            view.attach_event_manager(manager.clone());
        }
        self.views.insert(uid, view);
        self.fire_scene_event_type(SceneEvent::SCE_VIEW_ADDED, uid, "");
        true
    }

    pub fn view(&self, uid: Uid) -> Option<&View> {
        self.views.get(&uid)
    }

    pub fn get_view(&self, uid: Uid) -> Option<&View> {
        self.view(uid)
    }

    pub fn view_mut(&mut self, uid: Uid) -> Option<&mut View> {
        self.views.get_mut(&uid)
    }

    pub fn view_by_name(&self, name: &str) -> Option<&View> {
        self.views.values().find(|view| view.name() == name)
    }

    pub fn get_view_by_name(&self, name: &str) -> Option<&View> {
        self.view_by_name(name)
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn get_view_count(&self) -> usize {
        self.view_count()
    }

    pub fn view_uids(&self) -> Vec<Uid> {
        self.views.keys().copied().collect()
    }

    pub fn get_view_array(&self) -> Vec<&View> {
        self.views.values().collect()
    }

    pub fn view_uid_list(&self) -> String {
        self.view_uids()
            .into_iter()
            .map(|uid| uid.get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn view_uids_str(&self) -> String {
        self.view_uid_list()
    }

    pub fn get_view_uid_list(&self) -> String {
        self.view_uid_list()
    }

    pub fn destroy_view(&mut self, uid: Uid) -> bool {
        if !self.views.contains_key(&uid) {
            return false;
        }
        self.fire_scene_event_type(SceneEvent::SCE_VIEW_REMOVING, uid, "");
        let removed = self.views.remove(&uid).is_some();
        if removed && self.active_view_uid == Some(uid) {
            self.active_view_uid = self.views.keys().next().copied();
            for view in self.views.values_mut() {
                view.set_active(Some(view.uid()) == self.active_view_uid);
            }
        }
        removed
    }

    pub fn active_view_uid(&self) -> Option<Uid> {
        self.active_view_uid
    }

    pub fn active_view_id(&self) -> Option<Uid> {
        self.active_view_uid()
    }

    pub fn get_active_view_id(&self) -> Option<Uid> {
        self.active_view_uid()
    }

    pub fn get_active_view(&self) -> Option<&View> {
        self.active_view_uid.and_then(|uid| self.views.get(&uid))
    }

    pub fn set_active_view_id(&mut self, uid: Option<Uid>) -> bool {
        self.set_active_view_uid(uid)
    }

    pub fn set_active_view_uid(&mut self, uid: Option<Uid>) -> bool {
        if uid.is_some_and(|uid| !self.views.contains_key(&uid)) {
            return false;
        }
        self.active_view_uid = uid;
        for view in self.views.values_mut() {
            view.set_active(Some(view.uid()) == uid);
        }
        true
    }

    pub fn set_camera(&mut self, name: impl Into<String>, mut camera: Camera) {
        let name = name.into();
        camera.set_name(name.clone());
        self.cameras.insert(name, camera);
    }

    pub fn has_camera(&self, name: &str) -> bool {
        self.cameras.contains_key(name)
    }

    pub fn camera(&self, name: &str) -> Option<&Camera> {
        self.cameras.get(name)
    }

    pub fn get_camera(&self, name: &str) -> Option<Camera> {
        self.camera(name).cloned()
    }

    pub fn get_camera_ref(&self, name: &str) -> Option<&Camera> {
        self.camera(name)
    }

    pub fn camera_count(&self) -> usize {
        self.cameras.len()
    }

    pub fn cameras(&self) -> &BTreeMap<String, Camera> {
        &self.cameras
    }

    pub fn destroy_camera(&mut self, name: &str) -> bool {
        self.cameras.remove(name).is_some()
    }

    pub fn save_view_to_camera(&mut self, view_uid: Uid, name: impl Into<String>) -> bool {
        let name = name.into();
        let Some(view) = self.view(view_uid) else {
            return false;
        };
        let source = self
            .cameras
            .get(&name)
            .map(|camera| camera.source().to_string())
            .unwrap_or_default();
        let mut camera = view.camera();
        camera.set_name(name.clone());
        camera.set_source(source);
        self.cameras.insert(name, camera);
        true
    }

    pub fn save_view_to_cam(&mut self, view_uid: Uid, name: impl Into<String>) -> bool {
        self.save_view_to_camera(view_uid, name)
    }

    pub fn load_view_from_camera(&mut self, view_uid: Uid, name: &str) -> bool {
        let Some(camera) = self.cameras.get(name).cloned() else {
            return false;
        };
        let Some(view) = self.view_mut(view_uid) else {
            return false;
        };
        view.set_camera(&camera);
        true
    }

    pub fn load_view_from_cam(&mut self, view_uid: Uid, name: &str) -> bool {
        self.load_view_from_camera(view_uid, name)
    }

    pub fn load_view_from_cam_anim(&mut self, view_uid: Uid, name: &str) -> bool {
        self.load_view_from_camera(view_uid, name)
    }

    pub fn save_camera_to(
        &mut self,
        camera_name: &str,
        filename: impl AsRef<std::path::Path>,
    ) -> bool {
        let filename = filename.as_ref();
        let Some(camera) = self.cameras.get(camera_name) else {
            return false;
        };
        let Some(mut tree) = parse_ldom2_xml(&camera.to_xml_string()).ok() else {
            return false;
        };
        if camera.visibility_size() != 0 {
            let visibilities = tree.top_mut().append_child("visibilities");
            for (uid, visible, is_object) in camera.visibility_settings() {
                if is_object {
                    let Some(object) = self.objects.get(&Uid::new(uid)) else {
                        continue;
                    };
                    let node = visibilities.append_child("object");
                    node.set_value(visible.to_string());
                    node.set_str_attr("target", object.name());
                } else {
                    let Some((object, renderer)) = self.objects.values().find_map(|object| {
                        object
                            .renderer(Uid::new(uid))
                            .map(|renderer| (object, renderer))
                    }) else {
                        continue;
                    };
                    let node = visibilities.append_child("renderer");
                    node.set_value(visible.to_string());
                    node.set_str_attr("target", renderer.name());
                    node.set_str_attr("type", renderer.type_name());
                    node.set_str_attr("client", object.name());
                }
            }
        }
        if std::fs::write(filename, write_ldom2_xml(&tree)).is_err() {
            return false;
        }
        if camera_name != "__current" {
            let source = filename.to_string_lossy().into_owned();
            if let Some(camera) = self.cameras.get_mut(camera_name) {
                camera.set_source(source);
            }
        }
        true
    }

    pub fn load_camera(&self, filename: impl AsRef<std::path::Path>) -> Option<Camera> {
        let filename = filename.as_ref();
        let xml = std::fs::read_to_string(filename).ok()?;
        let mut camera = Camera::from_xml_string(&xml).ok()?;
        let tree = parse_ldom2_xml(&xml).ok()?;
        if let Some(visibilities) = tree.top().find_child("visibilities") {
            for node in visibilities
                .children()
                .iter()
                .filter(|node| !node.is_attr())
            {
                let visible = node.value().eq_ignore_ascii_case("true");
                if node.tag_name() == "object" {
                    let target = node.str_attr("target").to_string();
                    if let Some(object) = self.object_by_name(&target) {
                        camera.vis_append(object.uid().get(), visible, true);
                    }
                } else if node.tag_name() == "renderer" {
                    let client = node.str_attr("client").to_string();
                    let target = node.str_attr("target").to_string();
                    let type_name = node.str_attr("type").to_string();
                    let renderer = self
                        .object_by_name(&client)
                        .and_then(|object| object.renderer_by_name_type(&target, &type_name));
                    if let Some(renderer) = renderer {
                        camera.vis_append(renderer.uid().get(), visible, false);
                    }
                }
            }
        }
        camera.set_source(filename.to_string_lossy().into_owned());
        Some(camera)
    }

    pub fn get_anim_mgr(&self) -> AnimMgrHandle {
        AnimMgrHandle::new(self.uid)
    }

    pub fn undo_manager(&self) -> UndoManagerHandle {
        self.undo_manager.clone()
    }

    pub fn is_undoable(&self) -> bool {
        self.undo_manager.with(crate::UndoManager::is_undoable)
    }

    pub fn is_redoable(&self) -> bool {
        self.undo_manager.with(crate::UndoManager::is_redoable)
    }

    pub fn undo_size(&self) -> usize {
        self.undo_manager.with(crate::UndoManager::undo_size)
    }

    pub fn get_undo_size(&self) -> usize {
        self.undo_size()
    }

    pub fn redo_size(&self) -> usize {
        self.undo_manager.with(crate::UndoManager::redo_size)
    }

    pub fn get_redo_size(&self) -> usize {
        self.redo_size()
    }

    pub fn undo_desc(&self, index: usize) -> Option<String> {
        self.undo_manager
            .with(|manager| manager.undo_desc(index).map(str::to_string))
    }

    pub fn get_undo_desc(&self, index: usize) -> Option<String> {
        self.undo_desc(index)
    }

    pub fn redo_desc(&self, index: usize) -> Option<String> {
        self.undo_manager
            .with(|manager| manager.get_redo_desc(index).map(str::to_string))
    }

    pub fn get_redo_desc(&self, index: usize) -> Option<String> {
        self.redo_desc(index)
    }

    pub fn start_undo_txn(&self, description: impl Into<String>) {
        self.undo_manager
            .with_mut(|manager| manager.start_txn(description));
    }

    pub fn rollback_undo_txn(&self) -> bool {
        self.undo_manager.with_mut(crate::UndoManager::rollback_txn)
    }

    pub fn commit_undo_txn(&self) -> bool {
        let result = self.undo_manager.with_mut(crate::UndoManager::commit_txn);
        if result {
            self.fire_scene_event_type(SceneEvent::SCE_SCENE_UNDOINFO, self.uid, "");
        }
        result
    }

    pub fn undo(&self, index: usize) -> bool {
        self.undo_manager.with_mut(|manager| manager.undo_n(index))
    }

    pub fn redo(&self, index: usize) -> bool {
        self.undo_manager.with_mut(|manager| manager.redo_n(index))
    }

    pub fn clear_undo_data(&self) {
        let changed = self
            .undo_manager
            .with(|manager| manager.undo_size() != 0 || manager.redo_size() != 0);
        if !changed {
            return;
        }
        self.undo_manager
            .with_mut(|manager| manager.clear_all_info());
        self.fire_scene_event_type(SceneEvent::SCE_SCENE_UNDOINFO, self.uid, "");
    }

    pub fn clear_undo_data_scr(&self) {
        self.clear_undo_data();
    }

    pub fn add_listener(&self, listener: SharedEventListener<SceneEvent>) -> i32 {
        self.event_caster.add(listener).map_or(-1, |id| id as i32)
    }

    pub fn remove_listener(&self, id: i32) -> bool {
        id >= 0 && self.event_caster.remove(id as u64).is_some()
    }

    pub fn listener_count(&self) -> usize {
        self.event_caster.listener_count()
    }

    pub fn fire_scene_event(&self, event: &mut SceneEvent) {
        event.set_source(self.uid.get());
        self.event_caster.replica_fire(event);
        if let Some(manager) = self.event_manager.as_ref() {
            let _ = manager.with(|manager| manager.fire_scene_event(event));
        }
    }

    fn fire_scene_event_type(&self, event_type: i32, target: Uid, descr: &str) {
        let mut event = SceneEvent::new();
        event.set_event_type(event_type);
        event.set_target(target.get());
        event.set_descr(descr);
        self.fire_scene_event(&mut event);
    }

    fn fire_property_changed(&self, property_name: &str) {
        self.fire_scene_event_type(SceneEvent::SCE_SCENE_PROPCHG, self.uid, property_name);
    }

    pub fn exec_js_file(&mut self, path: impl AsRef<std::path::Path>) -> bool {
        let Ok(script) = std::fs::read_to_string(path) else {
            return false;
        };
        self.onload = script;
        self.modified = true;
        true
    }

    pub fn exec_jsfile(&mut self, path: impl AsRef<std::path::Path>) -> bool {
        self.exec_js_file(path)
    }

    pub fn dump(&self) {
        eprintln!("{}", self.to_string());
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        format!("Scene({}, {})", self.uid.get(), self.name)
    }

    pub fn get_camera_info_json(&self) -> String {
        let entries = self
            .cameras
            .iter()
            .map(|(name, camera)| {
                format!(
                    "{{\"name\":\"{}\",\"vis_size\":{},\"src\":\"{}\"}}",
                    escape_json(name),
                    camera.vis_size(),
                    escape_json(camera.source())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_object_tree_json(&self) -> String {
        self.scene_tree_json(false)
    }

    pub fn get_scene_data_json(&self) -> String {
        self.scene_tree_json(true)
    }

    fn scene_tree_json(&self, grouped: bool) -> String {
        let root = format!(
            "{{\"name\":\"{}\",\"parent\":-1,\"open\":\"true\",\"type\":\"\",\"rends\":[],\"ID\":{}}}",
            escape_json(&self.name),
            self.uid.get()
        );
        let mut objects = self.objects.values().collect::<Vec<_>>();
        objects.sort_by_key(|object| (object.ui_order(), object.uid()));
        let entries = objects
            .into_iter()
            .map(|object| {
                let renderers = if grouped {
                    object.grouped_renderer_list_json()
                } else {
                    object.flat_renderer_list_json()
                };
                format!(
                    "{{\"name\":\"{}\",\"parent\":-1,\"ui_collapsed\":{},\"ui_order\":{},\"visible\":{},\"locked\":{},\"type\":\"{}\",\"ID\":{},\"rends\":{renderers}}}",
                    escape_json(object.name()),
                    object.is_ui_collapsed(),
                    object.ui_order(),
                    object.is_visible(),
                    object.is_ui_locked(),
                    escape_json(object.class_name()),
                    object.uid().get()
                )
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            format!("[{root}]")
        } else {
            format!("[{root},{}]", entries.join(","))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneManager<T> {
    scenes: BTreeMap<Uid, Scene<T>>,
    uid_generator: UidGenerator,
    active_scene_uid: Option<Uid>,
    perf_meas_samples: Option<i32>,
    event_manager: ScrEventManagerHandle,
}

impl<T> Default for SceneManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SceneManager<T> {
    pub fn new() -> Self {
        Self {
            scenes: BTreeMap::new(),
            uid_generator: UidGenerator::new(),
            active_scene_uid: None,
            perf_meas_samples: None,
            event_manager: ScrEventManagerHandle::new(),
        }
    }

    pub fn create_scene(&mut self, name: impl Into<String>) -> Uid {
        let uid = self.uid_generator.next_uid();
        let mut scene = Scene::with_uid_generator(uid, name, self.uid_generator.clone());
        scene.attach_event_manager(self.event_manager.clone());
        self.scenes.insert(uid, scene);
        if self.active_scene_uid.is_none() {
            self.active_scene_uid = Some(uid);
        }
        uid
    }

    pub fn scene(&self, uid: Uid) -> Option<&Scene<T>> {
        self.scenes.get(&uid)
    }

    pub fn event_manager(&self) -> ScrEventManagerHandle {
        self.event_manager.clone()
    }

    pub fn get_scene(&self, uid: Uid) -> Option<&Scene<T>> {
        self.scene(uid)
    }

    pub fn scene_mut(&mut self, uid: Uid) -> Option<&mut Scene<T>> {
        self.scenes.get_mut(&uid)
    }

    pub fn destroy_scene(&mut self, uid: Uid) -> bool {
        let Some(scene) = self.scenes.get_mut(&uid) else {
            return false;
        };
        scene.unloading();
        scene.destroy_all_objects();
        let removed = self.scenes.remove(&uid).is_some();
        if removed && self.active_scene_uid == Some(uid) {
            self.active_scene_uid = self.scenes.keys().next().copied();
        }
        removed
    }

    pub fn destroy_all_scenes(&mut self) {
        for uid in self.scene_uids() {
            self.destroy_scene(uid);
        }
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    pub fn scene_by_name(&self, name: &str) -> Option<&Scene<T>> {
        self.scenes.values().find(|scene| scene.name() == name)
    }

    pub fn get_scene_by_name(&self, name: &str) -> Option<&Scene<T>> {
        self.scene_by_name(name)
    }

    pub fn scene_by_name_mut(&mut self, name: &str) -> Option<&mut Scene<T>> {
        self.scenes.values_mut().find(|scene| scene.name() == name)
    }

    pub fn get_object(&self, uid: Uid) -> Option<&SceneObject<T>> {
        self.scenes.values().find_map(|scene| scene.object(uid))
    }

    pub fn get_view(&self, uid: Uid) -> Option<&View> {
        self.scenes.values().find_map(|scene| scene.view(uid))
    }

    pub fn get_renderer(&self, uid: Uid) -> Option<&crate::Renderer> {
        self.scenes
            .values()
            .find_map(|scene| scene.get_renderer(uid))
    }

    pub fn scene_uids(&self) -> Vec<Uid> {
        self.scenes.keys().copied().collect()
    }

    pub fn get_scene_uid_list(&self) -> String {
        self.scene_uids()
            .into_iter()
            .map(|uid| uid.get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn get_scene_uidlist(&self) -> String {
        self.get_scene_uid_list()
    }

    pub fn active_scene_uid(&self) -> Option<Uid> {
        self.active_scene_uid
    }

    pub fn active_scene_id(&self) -> Option<Uid> {
        self.active_scene_uid()
    }

    pub fn get_active_scene_id(&self) -> Option<Uid> {
        self.active_scene_uid()
    }

    pub fn set_active_scene_id(&mut self, uid: Option<Uid>) -> bool {
        self.set_active_scene_uid(uid)
    }

    pub fn set_active_scene_uid(&mut self, uid: Option<Uid>) -> bool {
        if uid.is_some_and(|uid| !self.scenes.contains_key(&uid)) {
            return false;
        }
        self.active_scene_uid = uid;
        true
    }

    pub fn check_and_update_scenes(&mut self) {
        for scene in self.scenes.values_mut() {
            for view in scene.views.values_mut() {
                view.check_and_update();
            }
            scene.just_created = false;
        }
    }

    pub fn enable_perf_meas(&mut self, naver: i32) {
        self.perf_meas_samples = Some(naver);
    }

    pub fn disable_perf_meas(&mut self) {
        self.perf_meas_samples = None;
    }

    pub fn dump(&self) {
        eprintln!("SceneManager(count={})", self.scenes.len());
    }

    pub fn get_uid_obj(&self, uid: Uid) -> Option<Uid> {
        if self.scenes.contains_key(&uid) {
            return Some(uid);
        }
        self.scenes
            .values()
            .any(|scene| {
                scene.object(uid).is_some()
                    || scene.view(uid).is_some()
                    || scene.get_renderer(uid).is_some()
            })
            .then_some(uid)
    }

    pub fn get_uidobj(&self, uid: Uid) -> Option<Uid> {
        self.get_uid_obj(uid)
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn get_version(&self) -> String {
        self.version()
    }

    pub fn build(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn get_build_id(&self) -> String {
        self.build()
    }

    pub fn ver_arch(&self) -> String {
        std::env::consts::ARCH.to_string()
    }

    pub fn get_ver_arch_name(&self) -> String {
        self.ver_arch()
    }
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::{shared_event_listener, LScrCallBack, LScrObject, LVarArgs, Result};
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeSet;
    use std::rc::Rc;
    use std::sync::Mutex;

    type ScriptCall = (String, i32, i32, i32);

    struct ScriptRecorder {
        calls: Rc<RefCell<Vec<ScriptCall>>>,
    }

    impl LScrObject for ScriptRecorder {}

    impl LScrCallBack for ScriptRecorder {
        fn invoke(&self, args: &mut LVarArgs) -> Result<bool> {
            self.calls.borrow_mut().push((
                args.get(1).get_string_value()?.as_str().to_string(),
                args.get(2).get_int_value()?,
                args.get(3).get_int_value()?,
                args.get(4).get_int_value()?,
            ));
            Ok(true)
        }
    }

    struct SceneEdit {
        value: Rc<Cell<i32>>,
    }

    impl crate::EditInfo for SceneEdit {
        fn undo(&mut self) -> bool {
            self.value.set(self.value.get() - 1);
            true
        }

        fn redo(&mut self) -> bool {
            self.value.set(self.value.get() + 1);
            true
        }
    }

    #[test]
    fn scene_render_options_match_reference_defaults_and_invalidate_views() {
        let mut scene = Scene::<()>::new(Uid::new(1), "scene");
        assert!(!scene.is_ao_enabled());
        assert_eq!(scene.ao_radius(), 4.0);
        assert_eq!(scene.ao_intensity(), 2.2);
        assert_eq!(scene.ao_slices(), 9);
        assert_eq!(scene.ao_steps(), 3);
        assert!(!scene.is_ao_half_res());
        assert_eq!(scene.aa_method(), AAMethod::Fxaa);
        assert_eq!(scene.aa_jitter_level(), 0);

        let view_uid = scene.create_view();
        scene.view_mut(view_uid).unwrap().reset_projection_changed();
        scene.set_ao_enabled(true);
        scene.set_ao_radius(7.5);
        scene.set_ao_intensity(1.4);
        scene.set_ao_slices(12);
        scene.set_ao_steps(5);
        scene.set_ao_half_res(true);
        scene.set_aa_method(AAMethod::Smaa);
        scene.set_aa_jitter_level(4);

        assert!(scene.is_ao_enabled());
        assert_eq!(scene.get_ao_radius(), 7.5);
        assert_eq!(scene.get_ao_intensity(), 1.4);
        assert_eq!(scene.get_ao_slices(), 12);
        assert_eq!(scene.get_ao_steps(), 5);
        assert!(scene.is_ao_half_res());
        assert_eq!(scene.get_aa_method(), AAMethod::Smaa);
        assert_eq!(scene.get_aa_jitter_level(), 4);
        assert!(scene.view(view_uid).unwrap().is_projection_changed());
        assert!(scene.is_modified());
    }

    #[test]
    fn scene_tree_json_matches_reference_hierarchy() {
        let mut scene = Scene::<()>::new(Uid::new(1), "scene\"");
        let later_uid = scene.add_object("later", ());
        let first_uid = scene.add_object("first", ());
        {
            let later = scene.object_mut(later_uid).unwrap();
            later.set_ui_order(20);
            later.set_class_name("MolCoord");
        }
        {
            let first = scene.object_mut(first_uid).unwrap();
            first.set_ui_order(10);
            first.set_class_name("DensityMap");
            let renderer_uid = first.create_renderer("mapmesh");
            let renderer = first.renderer_mut(renderer_uid).unwrap();
            renderer.set_name("mesh");
        }

        let flat = scene.get_object_tree_json();
        assert!(flat.starts_with("[{\"name\":\"scene\\\"\",\"parent\":-1"));
        assert!(flat.find("first").unwrap() < flat.find("later").unwrap());
        assert!(flat.contains("\"type\":\"DensityMap\""));
        assert!(flat.contains("\"rends\":[{\"name\":\"mesh\""));
        assert_eq!(scene.object_uids(), vec![first_uid, later_uid]);
        let grouped = scene.get_scene_data_json();
        assert!(grouped.contains("\"ID\":1"));
    }

    #[test]
    fn scene_manages_objects_and_cameras() {
        let mut manager = SceneManager::new();
        let scene_uid = manager.create_scene("test scene");
        assert_eq!(manager.active_scene_uid(), Some(scene_uid));
        let scene = manager.scene_mut(scene_uid).unwrap();
        assert!(scene.is_just_created());
        assert_eq!(scene.source(), "");
        assert_eq!(scene.bg_color().rgba8().r, 0);
        let object_uid = scene.add_object("myObj", ());
        assert_eq!(scene.object_count(), 1);
        assert_eq!(scene.active_object_uid(), Some(object_uid));
        assert_eq!(
            scene.object(object_uid).unwrap().scene_uid(),
            Some(scene_uid)
        );
        assert_eq!(scene.object_by_name("myObj").unwrap().uid(), object_uid);
        assert_eq!(scene.object_uid_list(), object_uid.get().to_string());

        let view_uid = scene.create_view();
        scene.view_mut(view_uid).unwrap().set_name("0");
        assert_eq!(scene.view_count(), 1);
        assert_eq!(scene.active_view_uid(), Some(view_uid));
        assert!(scene.view(view_uid).unwrap().is_active());
        assert_eq!(scene.view_by_name("0").unwrap().uid(), view_uid);
        assert_eq!(scene.view_uid_list(), view_uid.get().to_string());

        let mut camera = Camera::default();
        camera.set_zoom(75.0);
        camera.set_source("linked.cam");
        scene.set_camera("cam1", camera);
        assert!(scene.has_camera("cam1"));
        assert_eq!(scene.camera("cam1").unwrap().zoom(), 75.0);
        assert_eq!(scene.camera_count(), 1);
        assert!(scene.load_view_from_camera(view_uid, "cam1"));
        assert_eq!(scene.view(view_uid).unwrap().zoom(), 75.0);
        scene.view_mut(view_uid).unwrap().set_zoom(80.0);
        assert!(scene.save_view_to_camera(view_uid, "cam1"));
        assert_eq!(scene.camera("cam1").unwrap().zoom(), 80.0);
        assert_eq!(scene.camera("cam1").unwrap().source(), "linked.cam");
        assert!(scene.save_view_to_camera(view_uid, "cam2"));
        assert_eq!(scene.camera_count(), 2);
        assert_eq!(
            scene.get_camera_info_json(),
            "[{\"name\":\"cam1\",\"vis_size\":0,\"src\":\"linked.cam\"},{\"name\":\"cam2\",\"vis_size\":0,\"src\":\"\"}]"
        );
        assert!(scene.destroy_camera("cam1"));
        assert_eq!(scene.camera_count(), 1);
        assert_eq!(scene.get_anim_mgr().scene_uid(), scene_uid);
        assert!(scene.destroy_view(view_uid));
        assert_eq!(scene.view_count(), 0);
    }

    #[test]
    fn scene_saves_and_loads_camera_file() {
        let mut scene = Scene::<()>::new(Uid::new(1), "test scene");
        let object_uid = scene.add_object("target", ());
        let mut camera = Camera::new();
        camera.set_name("cam\tone");
        camera.set_zoom(88.0);
        camera.set_center(cuemol_core::Vector4::new3(1.0, 2.0, 3.0));
        camera.vis_append(object_uid.get(), false, true);
        scene.set_camera("cam1", camera);
        let path = std::env::temp_dir().join(format!(
            "cuemol2-rs-camera-{}-{}.xml",
            std::process::id(),
            scene.uid().get()
        ));

        assert!(scene.save_camera_to("cam1", &path));
        let saved_xml = std::fs::read_to_string(&path).unwrap();
        assert!(saved_xml.contains("<camera"));
        assert!(saved_xml.contains("<visibilities>"));
        assert!(saved_xml.contains("target=\"target\""));
        assert_eq!(
            scene.camera("cam1").unwrap().source(),
            path.to_string_lossy()
        );
        let loaded = scene.load_camera(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.name(), "cam1");
        assert_eq!(loaded.zoom(), 88.0);
        assert_eq!(loaded.center(), cuemol_core::Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(
            loaded.visibility_json(),
            format!(
                "{{\"{}\":{{\"uid\":{},\"type\":\"object\",\"include\":true,\"visible\":false}}}}",
                object_uid.get(),
                object_uid.get()
            )
        );
        assert_eq!(loaded.source(), path.to_string_lossy());
    }

    #[test]
    fn scene_manager_check_and_update_resets_view_flags() {
        let mut manager = SceneManager::<()>::new();
        let scene_uid = manager.create_scene("scene");
        let view_uid = manager.scene_mut(scene_uid).unwrap().create_view();
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .view_mut(view_uid)
            .unwrap()
            .set_view_size(100, 100);
        assert!(manager
            .scene(scene_uid)
            .unwrap()
            .view(view_uid)
            .unwrap()
            .is_projection_changed());

        manager.check_and_update_scenes();

        assert!(!manager.scene(scene_uid).unwrap().is_just_created());
        assert!(!manager
            .scene(scene_uid)
            .unwrap()
            .view(view_uid)
            .unwrap()
            .is_projection_changed());
    }

    #[test]
    fn scene_exec_js_file_loads_script_into_onload() {
        let mut scene = Scene::<()>::new(Uid::new(1), "scene");
        let path = std::env::temp_dir().join(format!(
            "cuemol2-rs-script-{}-{}.js",
            std::process::id(),
            scene.uid().get()
        ));
        std::fs::write(&path, "scene.name = 'x';").unwrap();

        assert!(scene.exec_js_file(&path));
        let _ = std::fs::remove_file(path);

        assert_eq!(scene.onload(), "scene.name = 'x';");
        assert!(scene.is_modified());
    }

    #[test]
    fn scene_fires_reference_lifecycle_events_with_scene_source() {
        let scene_uid = Uid::new(10);
        let mut scene = Scene::<()>::new(scene_uid, "scene");
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        let listener = shared_event_listener(move |event: &mut SceneEvent| {
            listener_events.lock().unwrap().push(event.clone());
        });
        let listener_id = scene.add_listener(listener);
        assert_eq!(listener_id, 0);

        scene.set_name("renamed");
        let object_uid = scene.add_object("object", ());
        assert!(scene.destroy_object(object_uid));
        let view_uid = scene.create_view();
        assert!(scene.destroy_view(view_uid));
        scene.clear_all_data_script();
        scene.notify_loaded();
        scene.unloading();

        let recorded = events.lock().unwrap();
        let types = recorded
            .iter()
            .map(SceneEvent::event_type)
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            vec![
                SceneEvent::SCE_SCENE_PROPCHG,
                SceneEvent::SCE_OBJ_ADDED,
                SceneEvent::SCE_OBJ_REMOVING,
                SceneEvent::SCE_VIEW_ADDED,
                SceneEvent::SCE_VIEW_REMOVING,
                SceneEvent::SCE_SCENE_CLEARALL,
                SceneEvent::SCE_SCENE_ONLOADED,
                SceneEvent::SCE_SCENE_REMOVING,
            ]
        );
        assert!(recorded
            .iter()
            .all(|event| event.source() == scene_uid.get()));
        assert_eq!(recorded[0].target(), scene_uid.get());
        assert_eq!(recorded[0].descr(), "name");
        assert_eq!(recorded[1].target(), object_uid.get());
        assert_eq!(recorded[2].target(), object_uid.get());
        assert_eq!(recorded[3].target(), view_uid.get());
        assert_eq!(recorded[4].target(), view_uid.get());
        drop(recorded);

        assert!(scene.remove_listener(listener_id));
        assert_eq!(scene.listener_count(), 0);
        scene.set_name("silent");
        assert_eq!(events.lock().unwrap().len(), 8);
    }

    #[test]
    fn scene_manager_notifies_unloading_before_removing_objects() {
        let mut manager = SceneManager::new();
        let scene_uid = manager.create_scene("scene");
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        let listener = shared_event_listener(move |event: &mut SceneEvent| {
            listener_events.lock().unwrap().push(event.event_type());
        });
        let scene = manager.scene_mut(scene_uid).unwrap();
        scene.add_listener(listener);
        scene.add_object("object", ());
        events.lock().unwrap().clear();

        assert!(manager.destroy_scene(scene_uid));
        assert_eq!(manager.scene_count(), 0);
        assert_eq!(
            events.lock().unwrap().as_slice(),
            &[SceneEvent::SCE_SCENE_REMOVING, SceneEvent::SCE_OBJ_REMOVING]
        );
    }

    #[test]
    fn object_renderer_lifecycle_is_forwarded_to_scene_listeners() {
        let mut scene = Scene::new(Uid::new(5), "scene");
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        scene.add_listener(shared_event_listener(move |event: &mut SceneEvent| {
            if matches!(
                event.event_type(),
                SceneEvent::SCE_REND_ADDED | SceneEvent::SCE_REND_REMOVING
            ) {
                listener_events.lock().unwrap().push(event.clone());
            }
        }));
        let object_uid = scene.add_object("object", ());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        assert!(scene
            .object_mut(object_uid)
            .unwrap()
            .destroy_renderer(renderer_uid));

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), SceneEvent::SCE_REND_ADDED);
        assert_eq!(events[1].event_type(), SceneEvent::SCE_REND_REMOVING);
        assert!(events.iter().all(|event| event.source() == 5));
        assert!(events
            .iter()
            .all(|event| event.target() == renderer_uid.get()));
    }

    #[test]
    fn scene_and_view_events_reach_the_shared_script_event_manager() {
        let mut manager = SceneManager::new();
        let scene_uid = manager.create_scene("scene");
        let handle = manager.event_manager();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let callback = Rc::new(ScriptRecorder {
            calls: Rc::clone(&calls),
        });
        handle.with_mut(|events| {
            events.add_listener(callback);
            events.append(
                "objectAdded",
                crate::ScrEventManager::SEM_OBJECT,
                crate::ScrEventManager::SEM_ADDED,
                scene_uid.get() as i64,
            );
            events.append(
                "viewPropChanged",
                crate::ScrEventManager::SEM_VIEW,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i64,
            );
            events.append(
                "visible",
                crate::ScrEventManager::SEM_OBJECT,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i64,
            );
            events.append(
                "name",
                crate::ScrEventManager::SEM_RENDERER,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i64,
            );
        });

        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_object("object", ());
        let object = scene.object_mut(object_uid).unwrap();
        object.set_visible(false);
        let renderer_uid = object.create_renderer("simple");
        object
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_name("renderer");
        let view_uid = scene.create_view();
        scene.view_mut(view_uid).unwrap().set_zoom(80.0);

        let calls = calls.borrow();
        assert_eq!(calls.len(), 4);
        assert_eq!(
            calls[0],
            (
                "objectAdded".to_string(),
                crate::ScrEventManager::SEM_OBJECT,
                crate::ScrEventManager::SEM_ADDED,
                scene_uid.get() as i32,
            )
        );
        assert_eq!(
            calls[1],
            (
                "visible".to_string(),
                crate::ScrEventManager::SEM_OBJECT,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i32,
            )
        );
        assert_eq!(
            calls[2],
            (
                "name".to_string(),
                crate::ScrEventManager::SEM_RENDERER,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i32,
            )
        );
        assert_eq!(
            calls[3],
            (
                "viewPropChanged".to_string(),
                crate::ScrEventManager::SEM_VIEW,
                crate::ScrEventManager::SEM_PROPCHG,
                scene_uid.get() as i32,
            )
        );
    }

    #[test]
    fn manager_allocates_unique_uids_across_all_qsys_object_kinds() {
        let mut manager = SceneManager::new();
        let first_scene = manager.create_scene("first");
        let second_scene = manager.create_scene("second");

        let first = manager.scene_mut(first_scene).unwrap();
        let first_object = first.add_object("object1", ());
        let first_renderer = first
            .object_mut(first_object)
            .unwrap()
            .create_renderer("simple");
        let first_view = first.create_view();

        let second = manager.scene_mut(second_scene).unwrap();
        let second_object = second.add_object("object2", ());
        let second_renderer = second
            .object_mut(second_object)
            .unwrap()
            .create_renderer("simple");
        let second_view = second.create_view();

        let ids = BTreeSet::from([
            first_scene,
            second_scene,
            first_object,
            first_renderer,
            first_view,
            second_object,
            second_renderer,
            second_view,
        ]);
        assert_eq!(ids.len(), 8);
        assert!(ids
            .iter()
            .all(|uid| manager.get_uid_obj(*uid) == Some(*uid)));
        assert_eq!(manager.get_object(first_object).unwrap().name(), "object1");
        assert_eq!(manager.get_view(second_view).unwrap().uid(), second_view);
        assert_eq!(
            manager.get_renderer(first_renderer).unwrap().uid(),
            first_renderer
        );

        let explicit = Uid::new(1000);
        assert!(manager
            .scene_mut(first_scene)
            .unwrap()
            .insert_object(SceneObject::new(explicit, ())));
        let after_explicit = manager.scene_mut(first_scene).unwrap().create_view();
        assert!(after_explicit.get() > explicit.get());
    }

    #[test]
    fn scene_exposes_undo_api_and_fires_undo_info_events() {
        let scene = Scene::<()>::new(Uid::new(1), "scene");
        let event_types = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&event_types);
        scene.add_listener(shared_event_listener(move |event: &mut SceneEvent| {
            if event.event_type() == SceneEvent::SCE_SCENE_UNDOINFO {
                listener_events.lock().unwrap().push(event.event_type());
            }
        }));
        let value = Rc::new(Cell::new(1));
        scene.start_undo_txn("change");
        scene.undo_manager().with_mut(|manager| {
            manager.add_edit_info(Box::new(SceneEdit {
                value: Rc::clone(&value),
            }));
        });
        assert!(scene.commit_undo_txn());
        assert!(scene.is_undoable());
        assert_eq!(scene.undo_size(), 1);
        assert_eq!(scene.undo_desc(0).as_deref(), Some("change"));

        assert!(scene.undo(0));
        assert_eq!(value.get(), 0);
        assert!(scene.is_redoable());
        assert_eq!(scene.redo_desc(0).as_deref(), Some("change"));
        assert!(scene.redo(0));
        assert_eq!(value.get(), 1);
        scene.clear_undo_data();
        assert_eq!(scene.undo_size(), 0);
        assert_eq!(scene.redo_size(), 0);
        assert_eq!(event_types.lock().unwrap().len(), 2);
    }
}
