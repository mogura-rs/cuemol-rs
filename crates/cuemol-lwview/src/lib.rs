use cuemol_core::{Uid, Vector4};
use cuemol_gfx::{ColorSpec, PixelBuffer, RawHitData};
use cuemol_qsys::{Renderer, Scene, SceneObject};
use cuemol_render::{tessellate_snapshot, RenderSceneSnapshot};
use std::collections::BTreeMap;

pub mod qdf_lwobj;
pub use qdf_lwobj::{read_qdf_lwobj_bytes, write_qdf_lwobj_bytes, QdfLWObjReader, QdfLWObjWriter};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderLWRendererSnapshot {
    pub name: String,
    pub visible: bool,
    pub snapshot: RenderSceneSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderLWObjectSnapshot {
    pub uid: Uid,
    pub name: String,
    pub visible: bool,
    pub renderers: Vec<RenderLWRendererSnapshot>,
}

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
        "qdflwobj"
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
                        converted.set_property_str("data_id", uid.get().to_string());
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
        let objects = source
            .object_uids()
            .into_iter()
            .filter_map(|object_uid| source.object(object_uid))
            .map(|object| {
                let mut snapshot = object.data().clone();
                if object.xform_matrix() != cuemol_core::Matrix4::IDENTITY {
                    snapshot.apply_xform(&object.xform_matrix());
                }
                RenderLWObjectSnapshot {
                    uid: object.uid(),
                    name: object.name().to_string(),
                    visible: object.is_visible(),
                    renderers: vec![RenderLWRendererSnapshot {
                        name: "render".to_string(),
                        visible: true,
                        snapshot,
                    }],
                }
            })
            .collect::<Vec<_>>();
        self.conv_render_objects_to_lw_scene(&objects, target);
        for (name, camera) in source.cameras() {
            let mut camera = camera.clone();
            camera.set_source("");
            target.set_camera(name, camera);
        }
    }

    /// Convert resolved per-renderer snapshots while preserving the source object
    /// and renderer boundaries used by CueMol's lightweight scene converter.
    pub fn conv_render_objects_to_lw_scene(
        &mut self,
        source: &[RenderLWObjectSnapshot],
        target: &mut Scene<LWObject>,
    ) {
        for object in source {
            let mut lw_object = LWObject::new();
            lw_object.set_name(&object.name);
            lw_object.set_visible(object.visible);
            let mut scene_object = SceneObject::new(object.uid, lw_object);
            scene_object.set_name(&object.name);
            scene_object.set_visible(object.visible);
            scene_object.set_class_name("LWObject");
            for source_renderer in &object.renderers {
                let renderer_uid = scene_object.create_renderer("lwrend");
                let data_id = i32::try_from(renderer_uid.get()).unwrap_or(i32::MAX);
                if let Some(renderer) = scene_object.renderer_mut(renderer_uid) {
                    renderer.set_name(&source_renderer.name);
                    renderer.set_visible(source_renderer.visible);
                    renderer.set_property_str("data_id", data_id.to_string());
                }
                append_screen_draw_elements(
                    scene_object.data_mut(),
                    data_id,
                    &source_renderer.snapshot,
                );
                append_hit_data(scene_object.data_mut(), data_id, &source_renderer.snapshot);
                let mut mesh_snapshot = source_renderer.snapshot.clone();
                mesh_snapshot
                    .bonds
                    .retain(|bond| bond.screen_width.is_none());
                let mesh = tessellate_snapshot(&mesh_snapshot);
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
            }
            target.insert_object(scene_object);
        }
        self.conversions += 1;
    }

    pub fn conv_to_lwscene<T: Clone>(&mut self, source: &Scene<T>, target: &mut Scene<LWObject>) {
        self.conv_to_lw_scene(source, target);
    }
}

/// Append one lightweight draw element to a render snapshot while preserving
/// its primitive mode, vertex colors, normals, line width, and pixel coverage.
pub fn append_draw_element_snapshot(target: &mut RenderSceneSnapshot, element: &LWDrawElement) {
    let default_color = element.default_color();
    let (positions, normals, colors, order) = match element.data() {
        LWDrawData::Vertices(vertices) => (
            vertices.as_slice(),
            None,
            vec![default_color; vertices.len()],
            (0..vertices.len()).collect::<Vec<_>>(),
        ),
        LWDrawData::VertexColors(vertices) => {
            let positions = vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>();
            let colors = vertices
                .iter()
                .map(|vertex| vertex.color)
                .collect::<Vec<_>>();
            append_draw_arrays(
                target,
                &positions,
                None,
                &colors,
                &(0..positions.len()).collect::<Vec<_>>(),
                element.draw_mode(),
                element.line_width(),
            );
            return;
        }
        LWDrawData::VertexNormalColorIndices { vertices, indices } => {
            let positions = vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>();
            let normals = vertices
                .iter()
                .map(|vertex| vertex.normal)
                .collect::<Vec<_>>();
            let colors = vertices
                .iter()
                .map(|vertex| vertex.color)
                .collect::<Vec<_>>();
            let order = indices
                .iter()
                .filter_map(|&index| usize::try_from(index).ok())
                .collect::<Vec<_>>();
            append_draw_arrays(
                target,
                &positions,
                Some(&normals),
                &colors,
                &order,
                element.draw_mode(),
                element.line_width(),
            );
            return;
        }
        LWDrawData::Pixel(pixel) => {
            append_pixel_points(target, pixel);
            return;
        }
    };
    append_draw_arrays(
        target,
        positions,
        normals,
        &colors,
        &order,
        element.draw_mode(),
        element.line_width(),
    );
}

fn append_pixel_points(target: &mut RenderSceneSnapshot, pixel: &LWPixelData) {
    let pixels = pixel.pixels();
    let width = pixels.width();
    let pixel_count = width.saturating_mul(pixels.height());
    if width == 0 || pixel_count == 0 {
        return;
    }
    let position = f32_to_vector(pixel.position());
    let base_color = pixel.color();
    for (index, coverage) in pixels.data().iter().copied().take(pixel_count).enumerate() {
        if coverage <= 25 {
            continue;
        }
        let mut color = base_color;
        color[3] = ((u16::from(color[3]) * u16::from(coverage) + 127) / 255) as u8;
        let x = (index % width) as f32 + 0.5;
        let y = (index / width) as f32 + 0.5;
        target.bonds.push(
            cuemol_render::RenderBond::new(position, position, 0.03, Some(lw_color_spec(color)))
                .with_screen_style(1.0, false)
                .with_screen_offset(x, -y)
                .as_screen_point(),
        );
    }
}

fn append_draw_arrays(
    target: &mut RenderSceneSnapshot,
    positions: &[[f32; 3]],
    normals: Option<&[[f32; 3]]>,
    colors: &[[u8; 4]],
    order: &[usize],
    draw_mode: i8,
    line_width: f32,
) {
    use cuemol_gfx::AbstDrawElem;

    match i32::from(draw_mode) {
        AbstDrawElem::DRAW_POINTS => {
            for &index in order {
                let (Some(position), Some(color)) = (positions.get(index), colors.get(index))
                else {
                    continue;
                };
                target.bonds.push(
                    cuemol_render::RenderBond::new(
                        f32_to_vector(*position),
                        f32_to_vector(*position),
                        (line_width * 0.03).max(0.03),
                        Some(lw_color_spec(*color)),
                    )
                    .with_screen_style(line_width, false)
                    .as_screen_point(),
                );
            }
        }
        AbstDrawElem::DRAW_LINE_STRIP | AbstDrawElem::DRAW_LINE_LOOP | AbstDrawElem::DRAW_LINES => {
            for [start, end] in draw_line_indices(order, i32::from(draw_mode)) {
                let (Some(from), Some(to), Some(color), Some(end_color)) = (
                    positions.get(start),
                    positions.get(end),
                    colors.get(start),
                    colors.get(end),
                ) else {
                    continue;
                };
                target.bonds.push(
                    cuemol_render::RenderBond::new(
                        f32_to_vector(*from),
                        f32_to_vector(*to),
                        (line_width * 0.02).max(0.01),
                        Some(lw_color_spec(*color)),
                    )
                    .with_end_color(Some(lw_color_spec(*end_color)))
                    .with_screen_style(line_width, false),
                );
            }
        }
        _ => {
            let triangles = draw_triangle_indices(order, i32::from(draw_mode));
            let mut mesh = cuemol_render::RenderMesh::new();
            for triangle in triangles {
                let Some(points) = triangle
                    .map(|index| positions.get(index).copied())
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let points = [
                    f32_to_vector(points[0]),
                    f32_to_vector(points[1]),
                    f32_to_vector(points[2]),
                ];
                let face_normal = (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .normalize3d()
                    .unwrap_or(Vector4::new3(0.0, 0.0, 1.0));
                for &index in &triangle {
                    let normal = normals
                        .and_then(|normals| normals.get(index))
                        .map_or(face_normal, |normal| f32_to_vector(*normal));
                    let color = colors.get(index).copied().unwrap_or([255; 4]);
                    mesh.vertices.push(cuemol_render::MeshVertex {
                        pick_atom_index: None,
                        position: f32_to_vector(positions[index]),
                        normal,
                        color: lw_color_f32(color),
                    });
                    mesh.indices.push(mesh.indices.len() as u32);
                }
            }
            if !mesh.indices.is_empty() {
                target.meshes.push(mesh);
            }
        }
    }
}

fn draw_line_indices(order: &[usize], draw_mode: i32) -> Vec<[usize; 2]> {
    use cuemol_gfx::AbstDrawElem;

    match draw_mode {
        AbstDrawElem::DRAW_LINES => order
            .chunks_exact(2)
            .map(|pair| [pair[0], pair[1]])
            .collect(),
        AbstDrawElem::DRAW_LINE_STRIP => order.windows(2).map(|pair| [pair[0], pair[1]]).collect(),
        AbstDrawElem::DRAW_LINE_LOOP => {
            let mut pairs = order
                .windows(2)
                .map(|pair| [pair[0], pair[1]])
                .collect::<Vec<_>>();
            if order.len() > 2 {
                pairs.push([order[order.len() - 1], order[0]]);
            }
            pairs
        }
        _ => Vec::new(),
    }
}

fn draw_triangle_indices(order: &[usize], draw_mode: i32) -> Vec<[usize; 3]> {
    use cuemol_gfx::AbstDrawElem;

    match draw_mode {
        AbstDrawElem::DRAW_TRIANGLES => order
            .chunks_exact(3)
            .map(|triangle| [triangle[0], triangle[1], triangle[2]])
            .collect(),
        AbstDrawElem::DRAW_TRIANGLE_STRIP => order
            .windows(3)
            .enumerate()
            .map(|(index, triangle)| {
                if index % 2 == 0 {
                    [triangle[0], triangle[1], triangle[2]]
                } else {
                    [triangle[1], triangle[0], triangle[2]]
                }
            })
            .collect(),
        AbstDrawElem::DRAW_TRIANGLE_FAN | AbstDrawElem::DRAW_POLYGON => order
            .get(1..)
            .unwrap_or_default()
            .windows(2)
            .map(|pair| [order[0], pair[0], pair[1]])
            .collect(),
        AbstDrawElem::DRAW_QUADS => order
            .chunks_exact(4)
            .flat_map(|quad| [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]])
            .collect(),
        AbstDrawElem::DRAW_QUAD_STRIP => order
            .windows(4)
            .step_by(2)
            .flat_map(|quad| [[quad[0], quad[1], quad[3]], [quad[0], quad[3], quad[2]]])
            .collect(),
        _ => Vec::new(),
    }
}

fn f32_to_vector(position: [f32; 3]) -> Vector4 {
    Vector4::new3(
        f64::from(position[0]),
        f64::from(position[1]),
        f64::from(position[2]),
    )
}

fn lw_color_spec(color: [u8; 4]) -> ColorSpec {
    ColorSpec {
        base: cuemol_gfx::ColorBase::Html {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        },
        modifiers: Vec::new(),
    }
}

fn lw_color_f32(color: [u8; 4]) -> [f32; 4] {
    color.map(|component| f32::from(component) / 255.0)
}

fn append_hit_data(object: &mut LWObject, data_id: i32, snapshot: &RenderSceneSnapshot) {
    let indices = snapshot
        .atoms
        .iter()
        .filter_map(|atom| {
            let hit = atom.hit.as_ref()?;
            Some(object.add_point_hit(hit.index, atom.center, &hit.message))
        })
        .collect::<Vec<_>>();
    if !indices.is_empty() {
        object.set_hit_index_set(data_id, 0, indices);
    }
}

fn append_screen_draw_elements(
    object: &mut LWObject,
    data_id: i32,
    snapshot: &RenderSceneSnapshot,
) {
    let points = snapshot
        .bonds
        .iter()
        .filter(|bond| bond.screen_width.is_some() && bond.screen_point)
        .map(|bond| LWVertexColor {
            position: vector_to_f32(bond.from),
            color: render_color_u8(bond.color.as_ref()),
        })
        .collect::<Vec<_>>();
    if let Some(width) = snapshot
        .bonds
        .iter()
        .find(|bond| bond.screen_width.is_some() && bond.screen_point)
        .and_then(|bond| bond.screen_width)
    {
        object.add_draw_element(screen_draw_element(
            data_id,
            cuemol_gfx::AbstDrawElem::DRAW_POINTS as i8,
            width,
            points,
        ));
    }

    let lines = snapshot
        .bonds
        .iter()
        .filter(|bond| bond.screen_width.is_some() && !bond.screen_point)
        .flat_map(|bond| {
            [
                LWVertexColor {
                    position: vector_to_f32(bond.from),
                    color: render_color_u8(bond.color.as_ref()),
                },
                LWVertexColor {
                    position: vector_to_f32(bond.to),
                    color: render_color_u8(bond.end_color.as_ref().or(bond.color.as_ref())),
                },
            ]
        })
        .collect::<Vec<_>>();
    if let Some(width) = snapshot
        .bonds
        .iter()
        .find(|bond| bond.screen_width.is_some() && !bond.screen_point)
        .and_then(|bond| bond.screen_width)
    {
        object.add_draw_element(screen_draw_element(
            data_id,
            cuemol_gfx::AbstDrawElem::DRAW_LINES as i8,
            width,
            lines,
        ));
    }
}

fn screen_draw_element(
    data_id: i32,
    draw_mode: i8,
    line_width: f32,
    vertices: Vec<LWVertexColor>,
) -> LWDrawElement {
    let default_color = vertices.first().map_or([179, 179, 179, 255], |v| v.color);
    let data = if vertices.iter().all(|vertex| vertex.color == default_color) {
        LWDrawData::Vertices(vertices.into_iter().map(|vertex| vertex.position).collect())
    } else {
        LWDrawData::VertexColors(vertices)
    };
    LWDrawElement::new(data_id, draw_mode, line_width, default_color, data)
}

fn render_color_u8(color: Option<&ColorSpec>) -> [u8; 4] {
    color
        .and_then(ColorSpec::rgba8)
        .map_or([179, 179, 179, 255], |rgba| {
            [rgba.r, rgba.g, rgba.b, rgba.a]
        })
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
        assert_eq!(object.data_chunk_reader_name(1), "qdflwobj");
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
        let object = source.object_mut(object_uid).unwrap();
        object.set_visible(false);
        object.create_renderer("simple");
        let mut target = Scene::new(Uid::new(2), "target");

        let mut manager = LWViewerManager::new();
        manager.conv_to_lw_scene(&source, &mut target);
        assert_eq!(manager.conversion_count(), 1);
        assert_eq!(target.object_count(), 1);
        let copied = target.object(object_uid).unwrap();
        assert_eq!(copied.name(), "obj");
        assert!(!copied.is_visible());
        assert_eq!(copied.data().name(), "obj");
        assert_eq!(copied.rend_count(), 1);
        let renderer = copied.renderer_by_index(0).unwrap();
        assert_eq!(
            renderer
                .property_str("data_id")
                .and_then(|value| value.parse::<u64>().ok()),
            Some(renderer.uid().get())
        );
    }

    #[test]
    fn lw_viewer_manager_preserves_screen_points_and_lines() {
        let red = cuemol_gfx::parse_color("#ff0000").unwrap();
        let green = cuemol_gfx::parse_color("#00ff00").unwrap();
        let blue = cuemol_gfx::parse_color("#0000ff").unwrap();
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.bonds.push(
            cuemol_render::RenderBond::new(
                Vector4::new3(1.0, 2.0, 3.0),
                Vector4::new3(1.0, 2.0, 3.0),
                0.06,
                Some(green),
            )
            .with_screen_style(3.0, false)
            .as_screen_point(),
        );
        snapshot.bonds.push(
            cuemol_render::RenderBond::new(
                Vector4::ZERO,
                Vector4::new3(1.0, 0.0, 0.0),
                0.04,
                Some(red),
            )
            .with_end_color(Some(blue))
            .with_screen_style(2.0, false),
        );
        let source = [RenderLWObjectSnapshot {
            uid: Uid::new(10),
            name: "screen-primitives".to_string(),
            visible: true,
            renderers: vec![RenderLWRendererSnapshot {
                name: "lines".to_string(),
                visible: true,
                snapshot,
            }],
        }];
        let mut target = Scene::new(Uid::new(2), "target");
        LWViewerManager::new().conv_render_objects_to_lw_scene(&source, &mut target);

        let object = target.object(Uid::new(10)).unwrap().data();
        assert_eq!(object.draw_elements().len(), 2);
        let points = &object.draw_elements()[0];
        assert_eq!(
            points.draw_mode(),
            cuemol_gfx::AbstDrawElem::DRAW_POINTS as i8
        );
        assert_eq!(points.line_width(), 3.0);
        assert_eq!(points.default_color(), [0, 255, 0, 255]);
        assert!(matches!(points.data(), LWDrawData::Vertices(vertices) if vertices.len() == 1));
        let lines = &object.draw_elements()[1];
        assert_eq!(
            lines.draw_mode(),
            cuemol_gfx::AbstDrawElem::DRAW_LINES as i8
        );
        assert_eq!(lines.line_width(), 2.0);
        let LWDrawData::VertexColors(vertices) = lines.data() else {
            panic!("per-endpoint colors must remain variable");
        };
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
        assert_eq!(vertices[1].color, [0, 0, 255, 255]);
    }

    #[test]
    fn lw_viewer_manager_converts_render_geometry_to_indexed_lwo_data() {
        let mesh = cuemol_render::RenderMesh {
            vertices: vec![
                cuemol_render::MeshVertex {
                    pick_atom_index: None,
                    position: Vector4::new3(0.0, 0.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                cuemol_render::MeshVertex {
                    pick_atom_index: None,
                    position: Vector4::new3(1.0, 0.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.0, 1.0, 0.0, 1.0],
                },
                cuemol_render::MeshVertex {
                    pick_atom_index: None,
                    position: Vector4::new3(0.0, 1.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.0, 0.0, 1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        };
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.meshes.push(mesh);
        snapshot.atoms.push(
            cuemol_render::RenderAtom::new(Vector4::new3(0.5, 0.5, 0.0), 0.0, None)
                .with_hit(9, "mesh feature"),
        );
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
        let renderer = object.renderer_by_index(0).unwrap();
        assert_eq!(
            renderer.property_str("data_id"),
            Some(
                object.data().draw_elements()[0]
                    .data_id()
                    .to_string()
                    .as_str()
            )
        );
        let element = &object.data().draw_elements()[0];
        assert_eq!(element.type_id(), LW_DRAW_VNCI);
        let LWDrawData::VertexNormalColorIndices { vertices, indices } = element.data() else {
            panic!("indexed draw element expected");
        };
        assert_eq!(vertices.len(), 3);
        assert_eq!(vertices[0].color, [255, 0, 0, 255]);
        assert_eq!(indices, &[0, 1, 2]);
        assert_eq!(object.data().hit_data_len(), 1);
        assert_eq!(object.data().hit_data(0).unwrap().message(), "mesh feature");
        assert_eq!(object.data().hit_index_sets().len(), 1);
        assert_eq!(object.data().hit_index_sets()[0].indices(), &[0]);
        assert!(read_qdf_lwobj_bytes(&write_qdf_lwobj_bytes(object.data())).is_ok());
    }

    #[test]
    fn lw_viewer_manager_preserves_object_and_renderer_boundaries() {
        let triangle = |offset: f64, color: [f32; 4]| {
            let mut snapshot = RenderSceneSnapshot::empty();
            snapshot.meshes.push(cuemol_render::RenderMesh {
                vertices: [
                    Vector4::new3(offset, 0.0, 0.0),
                    Vector4::new3(offset + 1.0, 0.0, 0.0),
                    Vector4::new3(offset, 1.0, 0.0),
                ]
                .into_iter()
                .map(|position| cuemol_render::MeshVertex {
                    pick_atom_index: None,
                    position,
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color,
                })
                .collect(),
                indices: vec![0, 1, 2],
            });
            snapshot.recompute_bounds();
            snapshot
        };
        let source = vec![
            RenderLWObjectSnapshot {
                uid: Uid::new(10),
                name: "first".to_string(),
                visible: true,
                renderers: vec![
                    RenderLWRendererSnapshot {
                        name: "red".to_string(),
                        visible: true,
                        snapshot: triangle(0.0, [1.0, 0.0, 0.0, 1.0]),
                    },
                    RenderLWRendererSnapshot {
                        name: "green-hidden".to_string(),
                        visible: false,
                        snapshot: triangle(2.0, [0.0, 1.0, 0.0, 1.0]),
                    },
                ],
            },
            RenderLWObjectSnapshot {
                uid: Uid::new(20),
                name: "second-hidden".to_string(),
                visible: false,
                renderers: vec![RenderLWRendererSnapshot {
                    name: "blue".to_string(),
                    visible: true,
                    snapshot: triangle(4.0, [0.0, 0.0, 1.0, 1.0]),
                }],
            },
        ];
        let mut target = Scene::new(Uid::new(2), "target");
        let mut manager = LWViewerManager::new();
        manager.conv_render_objects_to_lw_scene(&source, &mut target);

        assert_eq!(target.object_uids(), vec![Uid::new(10), Uid::new(20)]);
        let first = target.object(Uid::new(10)).unwrap();
        assert_eq!(first.rend_count(), 2);
        assert_eq!(first.data().draw_elements().len(), 2);
        assert_eq!(first.renderer_by_index(0).unwrap().name(), "red");
        assert_eq!(first.renderer_by_index(1).unwrap().name(), "green-hidden");
        assert!(!first.renderer_by_index(1).unwrap().is_visible());
        let second = target.object(Uid::new(20)).unwrap();
        assert!(!second.is_visible());
        assert_eq!(second.rend_count(), 1);
        assert_eq!(second.data().draw_elements().len(), 1);
    }
}
