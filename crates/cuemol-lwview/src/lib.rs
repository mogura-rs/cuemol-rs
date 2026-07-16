use cuemol_core::{Uid, Vector4};
use cuemol_gfx::{PixelBuffer, RawHitData};
use cuemol_qsys::{Renderer, Scene, SceneObject};
use cuemol_render::{tessellate_snapshot, RenderSceneSnapshot};
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

pub const LW_DRAW_VNC: i8 = 1;
pub const LW_DRAW_VC: i8 = 2;
pub const LW_DRAW_VN: i8 = 3;
pub const LW_DRAW_V: i8 = 4;
pub const LW_DRAW_VNCI: i8 = 5;
pub const LW_DRAW_PIXEL: i8 = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LWVertexColor {
    pub position: [f32; 3],
    pub color: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LWVertexNormalColor {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LWPixelData {
    position: [f32; 3],
    color: [u8; 4],
    pixels: PixelBuffer,
}

impl LWPixelData {
    pub fn new(position: [f32; 3], color: [u8; 4], pixels: PixelBuffer) -> Self {
        Self {
            position,
            color,
            pixels,
        }
    }

    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    pub fn color(&self) -> [u8; 4] {
        self.color
    }

    pub fn pixels(&self) -> &PixelBuffer {
        &self.pixels
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LWDrawData {
    Vertices(Vec<[f32; 3]>),
    VertexColors(Vec<LWVertexColor>),
    VertexNormalColorIndices {
        vertices: Vec<LWVertexNormalColor>,
        indices: Vec<i32>,
    },
    Pixel(LWPixelData),
}

impl LWDrawData {
    pub fn type_id(&self) -> i8 {
        match self {
            Self::Vertices(_) => LW_DRAW_V,
            Self::VertexColors(_) => LW_DRAW_VC,
            Self::VertexNormalColorIndices { .. } => LW_DRAW_VNCI,
            Self::Pixel(_) => LW_DRAW_PIXEL,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LWDrawElement {
    data_id: i32,
    draw_mode: i8,
    line_width: f32,
    default_color: [u8; 4],
    data: LWDrawData,
}

impl LWDrawElement {
    pub fn new(
        data_id: i32,
        draw_mode: i8,
        line_width: f32,
        default_color: [u8; 4],
        data: LWDrawData,
    ) -> Self {
        Self {
            data_id,
            draw_mode,
            line_width,
            default_color,
            data,
        }
    }

    pub fn data_id(&self) -> i32 {
        self.data_id
    }

    pub fn type_id(&self) -> i8 {
        self.data.type_id()
    }

    pub fn draw_mode(&self) -> i8 {
        self.draw_mode
    }

    pub fn line_width(&self) -> f32 {
        self.line_width
    }

    pub fn default_color(&self) -> [u8; 4] {
        self.default_color
    }

    pub fn data(&self) -> &LWDrawData {
        &self.data
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LWHitIndexSet {
    data_id: i32,
    hit_type: i8,
    indices: Vec<i32>,
}

impl LWHitIndexSet {
    pub fn new(data_id: i32, hit_type: i8, indices: Vec<i32>) -> Self {
        Self {
            data_id,
            hit_type,
            indices,
        }
    }

    pub fn data_id(&self) -> i32 {
        self.data_id
    }

    pub fn hit_type(&self) -> i8 {
        self.hit_type
    }

    pub fn indices(&self) -> &[i32] {
        &self.indices
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LWObject {
    name: String,
    visible: bool,
    building: bool,
    draw_elements: Vec<LWDrawElement>,
    hit_data: Vec<LWHitData>,
    hit_index_sets: Vec<LWHitIndexSet>,
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
            draw_elements: Vec::new(),
            hit_data: Vec::new(),
            hit_index_sets: Vec::new(),
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
        self.draw_elements.clear();
        self.hit_data.clear();
        self.hit_index_sets.clear();
    }

    pub fn end_build(&mut self) {
        self.building = false;
    }

    pub fn is_building(&self) -> bool {
        self.building
    }

    pub fn add_draw_element(&mut self, element: LWDrawElement) {
        self.draw_elements.push(element);
    }

    pub fn draw_elements(&self) -> &[LWDrawElement] {
        &self.draw_elements
    }

    pub fn draw_elements_for(&self, data_id: i32) -> impl Iterator<Item = &LWDrawElement> {
        self.draw_elements
            .iter()
            .filter(move |element| element.data_id == data_id)
    }

    pub fn add_point_hit(&mut self, id: i32, position: Vector4, message: impl Into<String>) -> i32 {
        if let Some(index) = self.hit_data.iter().position(|hit| hit.id == id) {
            return index as i32;
        }
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

    pub fn set_hit_index_set(&mut self, data_id: i32, hit_type: i8, indices: Vec<i32>) {
        if let Some(set) = self
            .hit_index_sets
            .iter_mut()
            .find(|set| set.data_id == data_id)
        {
            *set = LWHitIndexSet::new(data_id, hit_type, indices);
        } else {
            self.hit_index_sets
                .push(LWHitIndexSet::new(data_id, hit_type, indices));
        }
    }

    pub fn hit_index_sets(&self) -> &[LWHitIndexSet] {
        &self.hit_index_sets
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

    pub fn with_uid(uid: Uid) -> Self {
        Self {
            renderer: Renderer::with_uid("lwrend", uid),
            ..Self::default()
        }
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

    pub fn get_data_id(&self) -> i32 {
        self.data_id()
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

    pub fn draw_elements<'a>(&self, object: &'a LWObject) -> Vec<&'a LWDrawElement> {
        object.draw_elements_for(self.data_id).collect()
    }

    pub fn restore_hit_indices(&mut self, object: &LWObject) {
        self.hit_indices = object
            .hit_index_sets()
            .iter()
            .find(|set| set.data_id() == self.data_id)
            .map_or_else(Vec::new, |set| set.indices().to_vec());
    }

    pub fn interp_hit(&self, hit_data: &dyn RawHitData, object: &LWObject) -> String {
        let size = hit_data.data_size(self.renderer.uid());
        if size == 0 {
            return String::new();
        }
        let Some(hit_index) = hit_data.data_at(self.renderer.uid(), 0, 0) else {
            return String::new();
        };
        let Some(hit) = object.hit_data(hit_index) else {
            return String::new();
        };
        let position = hit.position();
        format!(
            "\"objtype\":\"LWObject\",\"size\":{size},\"message\":\"{}\",\"x\":{},\"y\":{},\"z\":{},",
            escape_json(hit.message()),
            position.x,
            position.y,
            position.z,
        )
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
                scene_object.set_class_name("LWObject");
                for renderer_uid in object.renderer_uids() {
                    let Some(renderer) = object.renderer(renderer_uid) else {
                        continue;
                    };
                    if renderer.type_name() == "*selection" {
                        continue;
                    }
                    let uid = scene_object.create_renderer("lwrend");
                    if let Some(converted) = scene_object.renderer_mut(uid) {
                        converted.set_name(if renderer.name().is_empty() {
                            format!("({})", renderer.type_name())
                        } else {
                            renderer.name().to_string()
                        });
                        converted.set_visible(renderer.is_visible());
                    }
                }
                target.insert_object(scene_object);
            }
        }
        for (name, camera) in source.cameras() {
            let mut camera = camera.clone();
            camera.set_source("");
            target.set_camera(name, camera);
        }
        self.conversions += 1;
    }

    /// Convert already-resolved render snapshots into C++-compatible lightweight
    /// triangle data. This is the Rust equivalent of rendering each source object
    /// through `LWRendDisplayContext`: spheres and cylinders are tessellated, then
    /// stored as indexed vertex/normal/color elements under an `lwrend` data ID.
    pub fn conv_render_scene_to_lw_scene(
        &mut self,
        source: &Scene<RenderSceneSnapshot>,
        target: &mut Scene<LWObject>,
    ) {
        for object_uid in source.object_uids() {
            let Some(object) = source.object(object_uid) else {
                continue;
            };
            let mut snapshot = object.data().clone();
            if object.xform_matrix() != cuemol_core::Matrix4::IDENTITY {
                snapshot.apply_xform(&object.xform_matrix());
            }
            let mesh = tessellate_snapshot(&snapshot);
            let mut lw_object = LWObject::new();
            lw_object.set_name(object.name());
            lw_object.set_visible(object.is_visible());
            let mut scene_object = SceneObject::new(object.uid(), lw_object);
            scene_object.set_name(object.name());
            scene_object.set_visible(object.is_visible());
            scene_object.set_class_name("LWObject");
            let renderer_uid = scene_object.create_renderer("lwrend");
            let data_id = i32::try_from(renderer_uid.get()).unwrap_or(i32::MAX);
            if let Some(renderer) = scene_object.renderer_mut(renderer_uid) {
                renderer.set_name("render");
            }
            if !mesh.indices.is_empty() {
                let vertices = mesh
                    .vertices
                    .iter()
                    .map(|vertex| LWVertexNormalColor {
                        position: vector_to_f32(vertex.position),
                        normal: vector_to_f32(vertex.normal),
                        color: vertex.color.map(f32_color_component),
                    })
                    .collect();
                let indices = mesh.indices.iter().map(|&index| index as i32).collect();
                scene_object.data_mut().add_draw_element(LWDrawElement::new(
                    data_id,
                    cuemol_gfx::AbstDrawElem::DRAW_TRIANGLES as i8,
                    1.0,
                    [255; 4],
                    LWDrawData::VertexNormalColorIndices { vertices, indices },
                ));
            }
            target.insert_object(scene_object);
        }
        for (name, camera) in source.cameras() {
            let mut camera = camera.clone();
            camera.set_source("");
            target.set_camera(name, camera);
        }
        self.conversions += 1;
    }

    pub fn conv_to_lwscene<T: Clone>(&mut self, source: &Scene<T>, target: &mut Scene<LWObject>) {
        self.conv_to_lw_scene(source, target);
    }
}

fn vector_to_f32(value: Vector4) -> [f32; 3] {
    [value.x as f32, value.y as f32, value.z as f32]
}

fn f32_color_component(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
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
    fn lw_object_records_draw_elements_and_hit_data() {
        let mut object = LWObject::new();
        assert_eq!(object.type_name(), "LWObject");
        assert!(object.is_visible());
        object.set_name("mesh");
        object.start_build();
        assert!(object.is_building());
        object.add_draw_element(LWDrawElement::new(
            7,
            1,
            1.0,
            [255; 4],
            LWDrawData::Vertices(vec![[1.0, 2.0, 3.0]]),
        ));
        assert_eq!(object.draw_elements().len(), 1);
        assert_eq!(object.draw_elements()[0].data_id(), 7);
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
    fn lw_renderer_selects_data_and_hits_by_data_id() {
        let mut object = LWObject::new();
        object.add_draw_element(LWDrawElement::new(
            12,
            1,
            1.0,
            [255; 4],
            LWDrawData::Vertices(vec![[0.0, 0.0, 0.0]]),
        ));
        object.add_draw_element(LWDrawElement::new(
            13,
            1,
            1.0,
            [255; 4],
            LWDrawData::Vertices(vec![[1.0, 0.0, 0.0]]),
        ));
        object.set_hit_index_set(12, 0, vec![3, 4]);
        let mut renderer = LWRenderer::new();
        renderer.set_data_id(12);
        renderer.restore_hit_indices(&object);
        assert_eq!(renderer.draw_elements(&object).len(), 1);
        assert_eq!(renderer.hit_indices(), &[3, 4]);
    }

    #[test]
    fn lw_renderer_interprets_hit_metadata_from_the_client_object() {
        let mut object = LWObject::new();
        object.add_point_hit(7, Vector4::new3(1.0, 2.0, 3.0), "atom \"CA\"");
        let mut renderer = LWRenderer::with_uid(Uid::new(5));
        renderer.set_hit_indices(vec![0]);
        let mut context = cuemol_gfx::HittestContext::new();
        context.add_hit(Uid::new(5), 0.2, vec![0]);
        let mut data = cuemol_gfx::HitData::new();
        data.create_nearest(&context);

        assert_eq!(
            renderer.interp_hit(&data, &object),
            "\"objtype\":\"LWObject\",\"size\":1,\"message\":\"atom \\\"CA\\\"\",\"x\":1,\"y\":2,\"z\":3,"
        );
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

    #[test]
    fn lw_viewer_manager_converts_render_geometry_to_indexed_lwo_data() {
        let mesh = cuemol_render::RenderMesh {
            vertices: vec![
                cuemol_render::MeshVertex {
                    position: Vector4::new3(0.0, 0.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                cuemol_render::MeshVertex {
                    position: Vector4::new3(1.0, 0.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.0, 1.0, 0.0, 1.0],
                },
                cuemol_render::MeshVertex {
                    position: Vector4::new3(0.0, 1.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.0, 0.0, 1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        };
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.meshes.push(mesh);
        snapshot.recompute_bounds();
        let mut source = Scene::new(Uid::new(1), "source");
        let object_uid = source.add_object("mesh", snapshot);
        let mut target = Scene::new(Uid::new(2), "target");

        let mut manager = LWViewerManager::new();
        manager.conv_render_scene_to_lw_scene(&source, &mut target);

        let object = target.object(object_uid).unwrap();
        assert_eq!(object.class_name(), "LWObject");
        assert_eq!(object.rend_count(), 1);
        assert_eq!(object.data().draw_elements().len(), 1);
        let element = &object.data().draw_elements()[0];
        assert_eq!(element.type_id(), LW_DRAW_VNCI);
        let LWDrawData::VertexNormalColorIndices { vertices, indices } = element.data() else {
            panic!("indexed draw element expected");
        };
        assert_eq!(vertices.len(), 3);
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
        assert_eq!(indices, &[0, 1, 2]);
        assert!(read_qdf_lwobj_bytes(&write_qdf_lwobj_bytes(object.data())).is_ok());
    }
}
