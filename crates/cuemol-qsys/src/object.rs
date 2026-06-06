use crate::Renderer;
use cuemol_core::Uid;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject<T> {
    uid: Uid,
    scene_uid: Option<Uid>,
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
    next_renderer_uid: u64,
    data: T,
}

impl<T> SceneObject<T> {
    pub fn new(uid: Uid, data: T) -> Self {
        Self {
            uid,
            scene_uid: None,
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
            next_renderer_uid: 1,
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

    pub fn set_scene_uid(&mut self, scene_uid: Option<Uid>) {
        self.scene_uid = scene_uid;
        for renderer in self.renderers.values_mut() {
            renderer.set_scene_uid(scene_uid.unwrap_or(Uid::new(0)));
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn visible(&self) -> bool {
        self.is_visible()
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_ui_locked(&self) -> bool {
        self.ui_locked
    }

    pub fn locked(&self) -> bool {
        self.is_ui_locked()
    }

    pub fn set_ui_locked(&mut self, locked: bool) {
        self.ui_locked = locked;
    }

    pub fn is_ui_collapsed(&self) -> bool {
        self.ui_collapsed
    }

    pub fn ui_collapsed(&self) -> bool {
        self.is_ui_collapsed()
    }

    pub fn set_ui_collapsed(&mut self, collapsed: bool) {
        self.ui_collapsed = collapsed;
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn set_source_type(&mut self, source_type: impl Into<String>) {
        self.source_type = source_type.into();
    }

    pub fn alt_source(&self) -> &str {
        &self.alt_source
    }

    pub fn alt_src(&self) -> &str {
        self.alt_source()
    }

    pub fn set_alt_source(&mut self, value: impl Into<String>) {
        self.alt_source = value.into();
    }

    pub fn set_alt_src(&mut self, value: impl Into<String>) {
        self.set_alt_source(value);
    }

    pub fn xform_matrix(&self) -> cuemol_core::Matrix4 {
        self.xform_matrix
    }

    pub fn xform_mat(&self) -> cuemol_core::Matrix4 {
        self.xform_matrix()
    }

    pub fn set_xform_matrix(&mut self, value: cuemol_core::Matrix4) {
        self.xform_matrix = value;
    }

    pub fn set_xform_mat(&mut self, value: cuemol_core::Matrix4) {
        self.set_xform_matrix(value);
    }

    pub fn ui_order(&self) -> i32 {
        self.ui_order
    }

    pub fn set_ui_order(&mut self, value: i32) {
        self.ui_order = value;
    }

    pub fn create_renderer(&mut self, type_name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        let mut renderer = Renderer::with_uid(type_name, uid);
        renderer.set_client_obj_uid(self.uid);
        renderer.set_scene_uid(self.scene_uid.unwrap_or(Uid::new(0)));
        self.renderers.insert(uid, renderer);
        uid
    }

    pub fn create_preset_renderer(
        &mut self,
        preset_name: impl Into<String>,
        grp_name: impl Into<String>,
        name_prefix: impl Into<String>,
    ) -> Uid {
        let uid = self.create_renderer(preset_name);
        if let Some(renderer) = self.renderers.get_mut(&uid) {
            renderer.set_group_name(grp_name);
            renderer.set_name(name_prefix);
        }
        uid
    }

    pub fn attach_renderer(&mut self, mut renderer: Renderer) -> bool {
        let uid = renderer.uid();
        if self.renderers.contains_key(&uid) {
            return false;
        }
        renderer.set_client_obj_uid(self.uid);
        renderer.set_scene_uid(self.scene_uid.unwrap_or(Uid::new(0)));
        self.renderers.insert(uid, renderer);
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
        self.renderers.remove(&uid).is_some()
    }

    pub fn rend_count(&self) -> usize {
        self.renderers.len()
    }

    pub fn get_rend_count(&self) -> usize {
        self.rend_count()
    }

    pub fn renderer_by_index(&self, index: usize) -> Option<&Renderer> {
        self.renderers.values().nth(index)
    }

    pub fn get_renderer_by_index(&self, index: usize) -> Option<&Renderer> {
        self.renderer_by_index(index)
    }

    pub fn renderer_uids(&self) -> Vec<Uid> {
        self.renderers.keys().copied().collect()
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

    pub fn search_compatible_renderer_names(&self) -> String {
        "simple,cpk,ballstick,ribbon,tube".to_string()
    }

    pub fn ext_data(&self, _name: &str) -> Option<&T> {
        None
    }

    pub fn get_ext_data(&self, name: &str) -> Option<&T> {
        self.ext_data(name)
    }

    pub fn force_embed(&mut self) {
        self.source = format!("datachunk:{}", self.uid.get());
        self.alt_source.clear();
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_defaults_match_qsys_reference() {
        let object = SceneObject::new(Uid::new(1), ());
        assert_eq!(object.uid(), Uid::new(1));
        assert_eq!(object.scene_uid(), None);
        assert!(object.is_visible());
        assert!(!object.is_ui_locked());
        assert!(!object.modified());
        assert!(!object.is_ui_collapsed());
        assert_eq!(object.name(), "");
        assert_eq!(object.source(), "");
    }

    #[test]
    fn force_embed_sets_datachunk_source() {
        let mut object = SceneObject::new(Uid::new(1), ());
        object.set_source("/path/to/file.pdb");
        object.force_embed();
        assert!(object.source().starts_with("datachunk:"));
        assert_eq!(object.alt_source(), "");
    }
}
