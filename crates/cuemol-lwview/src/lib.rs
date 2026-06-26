use cuemol_core::{Uid, Vector4};
use cuemol_qsys::{Renderer, Scene, SceneObject};
use std::collections::BTreeMap;

pub mod qdf_lwobj;
pub use qdf_lwobj::{read_qdf_lwobj_bytes, write_qdf_lwobj_bytes, QdfLWObjReader, QdfLWObjWriter};

#[derive(Debug, Clone, PartialEq)]
pub struct LWHitData {
    id: i32,
    message: String,
    position: Vector4,
    rendered: bool,
}

impl LWHitData {
    pub fn new(id: i32, position: Vector4, message: impl Into<String>) -> Self {
        Self {
            id,
            message: message.into(),
            position,
            rendered: true,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn position(&self) -> Vector4 {
        self.position
    }

    pub fn is_rendered(&self) -> bool {
        self.rendered
    }

    pub fn set_rendered(&mut self, value: bool) {
        self.rendered = value;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LWDataChunk {
    id: i32,
    section: String,
    payload: Vec<u8>,
}

impl LWDataChunk {
    pub fn new(id: i32, section: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            section: section.into(),
            payload: payload.into(),
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn section(&self) -> &str {
        &self.section
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LWObject {
    name: String,
    visible: bool,
    building: bool,
    data_chunks: Vec<LWDataChunk>,
    hit_data: Vec<LWHitData>,
}

impl Default for LWObject {
    fn default() -> Self {
        Self::new()
    }
}

impl LWObject {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            visible: true,
            building: false,
            data_chunks: Vec::new(),
            hit_data: Vec::new(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        "LWObject"
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, value: bool) {
        self.visible = value;
    }

    pub fn start_build(&mut self) {
        self.building = true;
        self.data_chunks.clear();
        self.hit_data.clear();
    }

    pub fn end_build(&mut self) {
        self.building = false;
    }

    pub fn is_building(&self) -> bool {
        self.building
    }

    pub fn add_data_chunk(
        &mut self,
        section: impl Into<String>,
        payload: impl Into<Vec<u8>>,
    ) -> i32 {
        let id = self.data_chunks.len() as i32;
        self.data_chunks
            .push(LWDataChunk::new(id, section, payload));
        id
    }

    pub fn data_chunk(&self, id: i32) -> Option<&LWDataChunk> {
        self.data_chunks.iter().find(|chunk| chunk.id == id)
    }

    pub fn data_chunks(&self) -> &[LWDataChunk] {
        &self.data_chunks
    }

    pub fn add_point_hit(&mut self, id: i32, position: Vector4, message: impl Into<String>) -> i32 {
        let index = self.hit_data.len() as i32;
        self.hit_data.push(LWHitData::new(id, position, message));
        index
    }

    pub fn hit_data(&self, index: i32) -> Option<&LWHitData> {
        self.hit_data.get(index as usize)
    }

    /// Mutable access to a hit record (used by the QDF serializer to restore the
    /// `rendered` flag on load).
    pub fn hit_data_mut(&mut self, index: i32) -> Option<&mut LWHitData> {
        self.hit_data.get_mut(index as usize)
    }

    pub fn hit_data_len(&self) -> usize {
        self.hit_data.len()
    }

    pub fn is_data_src_writable(&self) -> bool {
        true
    }

    pub fn data_chunk_reader_name(&self, _qdf_version: i32) -> &'static str {
        "qdf_lwobj"
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LWRenderer {
    renderer: Renderer,
    data_id: i32,
    display_cache_valid: bool,
    hittest_cache_valid: bool,
    hit_indices: Vec<i32>,
}

impl Default for LWRenderer {
    fn default() -> Self {
        Self {
            renderer: Renderer::new("lwrend"),
            data_id: 0,
            display_cache_valid: false,
            hittest_cache_valid: false,
            hit_indices: Vec::new(),
        }
    }
}

impl LWRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    pub fn type_name(&self) -> &'static str {
        "lwrend"
    }

    pub fn to_command_string(&self) -> &'static str {
        "lwrend"
    }

    pub fn data_id(&self) -> i32 {
        self.data_id
    }

    pub fn set_data_id(&mut self, value: i32) {
        self.data_id = value;
    }

    pub fn is_compatible_object(&self, _object: &LWObject) -> bool {
        true
    }

    pub fn center(&self) -> Vector4 {
        Vector4::ZERO
    }

    pub fn set_display_cache_valid(&mut self, value: bool) {
        self.display_cache_valid = value;
    }

    pub fn is_display_cache_valid(&self) -> bool {
        self.display_cache_valid
    }

    pub fn invalidate_display_cache(&mut self) {
        self.display_cache_valid = false;
    }

    pub fn set_hittest_cache_valid(&mut self, value: bool) {
        self.hittest_cache_valid = value;
    }

    pub fn is_hittest_cache_valid(&self) -> bool {
        self.hittest_cache_valid
    }

    pub fn invalidate_hittest_cache(&mut self) {
        self.hittest_cache_valid = false;
    }

    pub fn unloading(&mut self) {
        self.invalidate_display_cache();
        self.invalidate_hittest_cache();
    }

    pub fn hit_indices(&self) -> &[i32] {
        &self.hit_indices
    }

    pub fn set_hit_indices(&mut self, values: Vec<i32>) {
        self.hit_indices = values;
    }

    pub fn is_hit_test_supported(&self) -> bool {
        !self.hit_indices.is_empty()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LWViewerManager {
    conversions: usize,
}

impl LWViewerManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn conversion_count(&self) -> usize {
        self.conversions
    }

    pub fn conv_to_lw_scene<T: Clone>(&mut self, source: &Scene<T>, target: &mut Scene<LWObject>) {
        for object_uid in source.object_uids() {
            if let Some(object) = source.object(object_uid) {
                let mut lw_object = LWObject::new();
                lw_object.set_name(object.name());
                lw_object.set_visible(object.is_visible());
                let uid = Uid::new(object.uid().get());
                let mut scene_object = SceneObject::new(uid, lw_object);
                scene_object.set_name(object.name());
                scene_object.set_visible(object.is_visible());
                target.insert_object(scene_object);
            }
        }
        self.conversions += 1;
    }

    pub fn conv_to_lwscene<T: Clone>(&mut self, source: &Scene<T>, target: &mut Scene<LWObject>) {
        self.conv_to_lw_scene(source, target);
    }
}

pub fn renderer_table_json(renderers: &BTreeMap<i32, LWRenderer>) -> String {
    let entries = renderers
        .iter()
        .map(|(id, renderer)| {
            format!(
                "{{\"id\":{id},\"type\":\"{}\",\"data_id\":{}}}",
                renderer.type_name(),
                renderer.data_id()
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lw_object_records_chunks_and_hit_data() {
        let mut object = LWObject::new();
        assert_eq!(object.type_name(), "LWObject");
        assert!(object.is_visible());
        object.set_name("mesh");
        object.start_build();
        assert!(object.is_building());
        let data_id = object.add_data_chunk("mesh:surface", [1_u8, 2, 3]);
        assert_eq!(data_id, 0);
        assert_eq!(object.data_chunk(0).unwrap().payload(), &[1, 2, 3]);
        let hit_index = object.add_point_hit(7, Vector4::new3(1.0, 2.0, 3.0), "atom");
        assert_eq!(hit_index, 0);
        assert_eq!(object.hit_data(0).unwrap().id(), 7);
        assert_eq!(object.hit_data(0).unwrap().message(), "atom");
        assert!(object.is_data_src_writable());
        assert_eq!(object.data_chunk_reader_name(1), "qdf_lwobj");
        object.end_build();
        assert!(!object.is_building());
    }

    #[test]
    fn lw_renderer_defaults_match_qif() {
        let mut renderer = LWRenderer::new();
        assert_eq!(renderer.type_name(), "lwrend");
        assert_eq!(renderer.to_command_string(), "lwrend");
        assert_eq!(renderer.data_id(), 0);
        assert_eq!(renderer.center(), Vector4::ZERO);
        assert!(!renderer.is_hit_test_supported());
        renderer.set_data_id(12);
        renderer.set_display_cache_valid(true);
        renderer.set_hittest_cache_valid(true);
        renderer.set_hit_indices(vec![1, 2]);
        assert!(renderer.is_hit_test_supported());
        renderer.unloading();
        assert!(!renderer.is_display_cache_valid());
        assert!(!renderer.is_hittest_cache_valid());
    }

    #[test]
    fn lw_viewer_manager_copies_qsys_scene_objects() {
        let mut source = Scene::new(Uid::new(1), "source");
        let object_uid = source.add_object("obj", ());
        source.object_mut(object_uid).unwrap().set_visible(false);
        let mut target = Scene::new(Uid::new(2), "target");

        let mut manager = LWViewerManager::new();
        manager.conv_to_lw_scene(&source, &mut target);
        assert_eq!(manager.conversion_count(), 1);
        assert_eq!(target.object_count(), 1);
        let copied = target.object(object_uid).unwrap();
        assert_eq!(copied.name(), "obj");
        assert!(!copied.is_visible());
        assert_eq!(copied.data().name(), "obj");
    }
}
