use crate::{Camera, SceneObject, View};
use cuemol_core::Uid;
use cuemol_gfx::SolidColor;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Scene<T> {
    uid: Uid,
    name: String,
    source: String,
    source_type: String,
    base_path: String,
    bg_color: SolidColor,
    onload: String,
    use_colproof: bool,
    icc_filename: String,
    icc_intent: i32,
    next_listener_id: i32,
    listeners: Vec<i32>,
    modified: bool,
    just_created: bool,
    objects: BTreeMap<Uid, SceneObject<T>>,
    views: BTreeMap<Uid, View>,
    cameras: BTreeMap<String, Camera>,
    next_object_uid: u64,
    next_view_uid: u64,
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
        Self {
            uid,
            name: name.into(),
            source: String::new(),
            source_type: String::new(),
            base_path: String::new(),
            bg_color: SolidColor::from_rgb(0.0, 0.0, 0.0),
            onload: String::new(),
            use_colproof: false,
            icc_filename: String::new(),
            icc_intent: 0,
            next_listener_id: 1,
            listeners: Vec::new(),
            modified: false,
            just_created: true,
            objects: BTreeMap::new(),
            views: BTreeMap::new(),
            cameras: BTreeMap::new(),
            next_object_uid: 1,
            next_view_uid: 1,
            active_object_uid: None,
            active_renderer_uid: None,
            active_view_uid: None,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.modified = true;
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn src(&self) -> &str {
        self.source()
    }

    pub fn set_source(&mut self, value: impl Into<String>) {
        self.source = value.into();
        self.modified = true;
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn srctype(&self) -> &str {
        self.source_type()
    }

    pub fn set_source_type(&mut self, value: impl Into<String>) {
        self.source_type = value.into();
        self.modified = true;
    }

    pub fn base_path(&self) -> &str {
        &self.base_path
    }

    pub fn base_path_qif(&self) -> &str {
        self.base_path()
    }

    pub fn set_base_path(&mut self, value: impl Into<String>) {
        self.base_path = value.into();
    }

    pub fn bg_color(&self) -> &SolidColor {
        &self.bg_color
    }

    pub fn bgcolor(&self) -> &SolidColor {
        self.bg_color()
    }

    pub fn set_bg_color(&mut self, value: SolidColor) {
        self.bg_color = value;
        self.modified = true;
    }

    pub fn onload(&self) -> &str {
        &self.onload
    }

    pub fn set_onload(&mut self, value: impl Into<String>) {
        self.onload = value.into();
    }

    pub fn use_colproof(&self) -> bool {
        self.use_colproof
    }

    pub fn is_use_colproof(&self) -> bool {
        self.use_colproof()
    }

    pub fn set_use_colproof(&mut self, value: bool) {
        self.use_colproof = value;
    }

    pub fn icc_filename(&self) -> &str {
        &self.icc_filename
    }

    pub fn set_icc_filename(&mut self, value: impl Into<String>) {
        self.icc_filename = value.into();
    }

    pub fn icc_intent(&self) -> i32 {
        self.icc_intent
    }

    pub fn set_icc_intent(&mut self, value: i32) {
        self.icc_intent = value;
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
        let uid = Uid::new(self.next_object_uid);
        self.next_object_uid += 1;
        let mut object = SceneObject::new(uid, data);
        object.set_name(name);
        object.set_scene_uid(Some(self.uid));
        self.objects.insert(uid, object);
        self.active_object_uid = Some(uid);
        self.modified = true;
        uid
    }

    pub fn insert_object(&mut self, mut object: SceneObject<T>) -> bool {
        let uid = object.uid();
        if self.objects.contains_key(&uid) {
            return false;
        }
        object.set_scene_uid(Some(self.uid));
        self.objects.insert(uid, object);
        self.active_object_uid = Some(uid);
        self.modified = true;
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
        self.objects.keys().copied().collect()
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

    pub fn destroy_object(&mut self, uid: Uid) -> bool {
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
        self.objects.clear();
        self.active_object_uid = None;
        self.modified = true;
    }

    pub fn clear_all_data(&mut self) {
        self.destroy_all_objects();
        self.views.clear();
        self.cameras.clear();
        self.active_view_uid = None;
        self.active_renderer_uid = None;
    }

    pub fn active_object_uid(&self) -> Option<Uid> {
        self.active_object_uid
    }

    pub fn active_obj_id(&self) -> Option<Uid> {
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
        let uid = Uid::new(self.next_view_uid);
        self.next_view_uid += 1;
        let mut view = View::new(uid, self.uid);
        if self.views.is_empty() {
            view.set_active(true);
            self.active_view_uid = Some(uid);
        }
        self.views.insert(uid, view);
        uid
    }

    pub fn insert_view(&mut self, mut view: View) -> bool {
        let uid = view.uid();
        if self.views.contains_key(&uid) || view.scene_uid() != self.uid {
            return false;
        }
        if self.views.is_empty() {
            view.set_active(true);
            self.active_view_uid = Some(uid);
        }
        self.views.insert(uid, view);
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

    pub fn destroy_view(&mut self, uid: Uid) -> bool {
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

    pub fn set_camera(&mut self, name: impl Into<String>, camera: Camera) {
        self.cameras.insert(name.into(), camera);
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

    pub fn destroy_camera(&mut self, name: &str) -> bool {
        self.cameras.remove(name).is_some()
    }

    pub fn save_view_to_camera(&mut self, view_uid: Uid, name: impl Into<String>) -> bool {
        let Some(view) = self.view(view_uid) else {
            return false;
        };
        self.cameras.insert(name.into(), view.camera());
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

    pub fn save_camera_to(&self, camera_name: &str, filename: impl AsRef<std::path::Path>) -> bool {
        let Some(camera) = self.cameras.get(camera_name) else {
            return false;
        };
        std::fs::write(filename, camera.to_camera_data()).is_ok()
    }

    pub fn load_camera(&self, filename: impl AsRef<std::path::Path>) -> Option<Camera> {
        Camera::from_camera_data(&std::fs::read_to_string(filename).ok()?)
    }

    pub fn get_anim_mgr(&self) -> AnimMgrHandle {
        AnimMgrHandle::new(self.uid)
    }

    pub fn add_listener<C: cuemol_core::LScrCallBack + ?Sized>(&mut self, _callback: &C) -> i32 {
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.push(id);
        id
    }

    pub fn remove_listener(&mut self, id: i32) -> bool {
        let before = self.listeners.len();
        self.listeners.retain(|value| *value != id);
        before != self.listeners.len()
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
                    "{{\"name\":\"{}\",\"zoom\":{}}}",
                    escape_json(name),
                    camera.zoom()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_object_tree_json(&self) -> String {
        let objects = self
            .objects
            .values()
            .map(|object| {
                format!(
                    "{{\"uid\":{},\"name\":\"{}\",\"visible\":{}}}",
                    object.uid().get(),
                    escape_json(object.name()),
                    object.is_visible()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{objects}]")
    }

    pub fn get_scene_data_json(&self) -> String {
        format!(
            "{{\"uid\":{},\"name\":\"{}\",\"objects\":{},\"views\":{}}}",
            self.uid.get(),
            escape_json(&self.name),
            self.object_count(),
            self.view_count()
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneManager<T> {
    scenes: BTreeMap<Uid, Scene<T>>,
    next_scene_uid: u64,
    active_scene_uid: Option<Uid>,
    perf_meas_samples: Option<i32>,
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
            next_scene_uid: 1,
            active_scene_uid: None,
            perf_meas_samples: None,
        }
    }

    pub fn create_scene(&mut self, name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_scene_uid);
        self.next_scene_uid += 1;
        self.scenes.insert(uid, Scene::new(uid, name));
        if self.active_scene_uid.is_none() {
            self.active_scene_uid = Some(uid);
        }
        uid
    }

    pub fn scene(&self, uid: Uid) -> Option<&Scene<T>> {
        self.scenes.get(&uid)
    }

    pub fn get_scene(&self, uid: Uid) -> Option<&Scene<T>> {
        self.scene(uid)
    }

    pub fn scene_mut(&mut self, uid: Uid) -> Option<&mut Scene<T>> {
        self.scenes.get_mut(&uid)
    }

    pub fn destroy_scene(&mut self, uid: Uid) -> bool {
        let removed = self.scenes.remove(&uid).is_some();
        if removed && self.active_scene_uid == Some(uid) {
            self.active_scene_uid = self.scenes.keys().next().copied();
        }
        removed
    }

    pub fn destroy_all_scenes(&mut self) {
        self.scenes.clear();
        self.active_scene_uid = None;
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

    pub fn build(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn ver_arch(&self) -> String {
        std::env::consts::ARCH.to_string()
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
        scene.set_camera("cam1", camera);
        assert!(scene.has_camera("cam1"));
        assert_eq!(scene.camera("cam1").unwrap().zoom(), 75.0);
        assert_eq!(scene.camera_count(), 1);
        assert!(scene.load_view_from_camera(view_uid, "cam1"));
        assert_eq!(scene.view(view_uid).unwrap().zoom(), 75.0);
        assert!(scene.save_view_to_camera(view_uid, "cam2"));
        assert_eq!(scene.camera_count(), 2);
        assert!(scene.destroy_camera("cam1"));
        assert_eq!(scene.camera_count(), 1);
        assert_eq!(scene.get_anim_mgr().scene_uid(), scene_uid);
        assert!(scene.destroy_view(view_uid));
        assert_eq!(scene.view_count(), 0);
    }

    #[test]
    fn scene_saves_and_loads_camera_file() {
        let mut scene = Scene::<()>::new(Uid::new(1), "test scene");
        let mut camera = Camera::new();
        camera.set_name("cam\tone");
        camera.set_zoom(88.0);
        camera.set_center(cuemol_core::Vector4::new3(1.0, 2.0, 3.0));
        camera.vis_append(10, false, true);
        scene.set_camera("cam1", camera);
        let path = std::env::temp_dir().join(format!(
            "cuemol2-rs-camera-{}-{}.txt",
            std::process::id(),
            scene.uid().get()
        ));

        assert!(scene.save_camera_to("cam1", &path));
        let loaded = scene.load_camera(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(loaded.name(), "cam\tone");
        assert_eq!(loaded.zoom(), 88.0);
        assert_eq!(loaded.center(), cuemol_core::Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(loaded.visibility_json(), "{\"obj:10\":false}");
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
}
