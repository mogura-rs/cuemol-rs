use cuemol_core::{CueMolError, Matrix4, Quat, Result, Vector4};

use crate::{
    AbstractColor, AbstractDrawAttrs, CylinderList, GrowMesh, LineGpuPrim, Mesh, SolidColor,
    SphereList, TrigGpuPrim, VboRep,
};

const EPSILON: f64 = 1.0e-4;
const INVALID_UID: u64 = u64::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayContext {
    material: String,
    style_names: String,
    alpha: f64,
    line_width: f64,
    line_stipple: u16,
    lighting: bool,
    pixel_scale: f64,
    edge_line_type: i32,
    edge_line_width: f64,
    edge_line_color: SolidColor,
    color: SolidColor,
    fog_enabled: bool,
    fog_start: f32,
    fog_end: f32,
    fog_color: SolidColor,
    ao_enabled: bool,
    projection_matrix: Matrix4,
    viewport: Vector4,
    matrix_stack: Vec<Matrix4>,
    view_id: u64,
    scene_id: u64,
    polygon_mode: i32,
}

impl Default for DisplayContext {
    fn default() -> Self {
        Self {
            material: String::new(),
            style_names: String::new(),
            alpha: 1.0,
            line_width: -1.0,
            line_stipple: u16::MAX,
            lighting: false,
            pixel_scale: 1.0,
            edge_line_type: Self::ELT_NONE,
            edge_line_width: -1.0,
            edge_line_color: SolidColor::from_rgb(0.0, 0.0, 0.0),
            color: SolidColor::from_rgb(0.5, 0.5, 0.5),
            fog_enabled: false,
            fog_start: 0.0,
            fog_end: 0.0,
            fog_color: SolidColor::from_rgb(0.0, 0.0, 0.0),
            ao_enabled: false,
            projection_matrix: Matrix4::IDENTITY,
            viewport: Vector4::ZERO,
            matrix_stack: vec![Matrix4::IDENTITY],
            view_id: INVALID_UID,
            scene_id: INVALID_UID,
            polygon_mode: Self::POLY_FILL,
        }
    }
}

impl DisplayContext {
    pub const POLY_POINT: i32 = 0;
    pub const POLY_LINE: i32 = 1;
    pub const POLY_FILL: i32 = 2;
    pub const POLY_FILL_NORGLN: i32 = 3;
    pub const POLY_FILL_XX: i32 = 4;

    pub const ELT_NONE: i32 = 0;
    pub const ELT_EDGES: i32 = 1;
    pub const ELT_SILHOUETTE: i32 = 2;

    pub const DVA_NONE: i32 = 0;
    pub const DVA_NOEDGE: i32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn view_id(&self) -> u64 {
        self.view_id
    }

    pub fn set_view_id(&mut self, value: u64) {
        self.view_id = value;
    }

    pub fn scene_id(&self) -> u64 {
        self.scene_id
    }

    pub fn set_scene_id(&mut self, value: u64) {
        self.scene_id = value;
    }

    pub fn material(&self) -> &str {
        &self.material
    }

    pub fn set_material(&mut self, value: impl Into<String>) {
        self.material = value.into();
    }

    pub fn style_names(&self) -> &str {
        &self.style_names
    }

    pub fn set_style_names(&mut self, value: impl Into<String>) {
        self.style_names = value.into();
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.alpha = value;
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.line_width = value;
    }

    pub fn line_stipple(&self) -> u16 {
        self.line_stipple
    }

    pub fn set_line_stipple(&mut self, value: u16) {
        self.line_stipple = value;
    }

    pub fn lighting(&self) -> bool {
        self.lighting
    }

    pub fn set_lighting(&mut self, value: bool) {
        self.lighting = value;
    }

    pub fn pixel_scale(&self) -> f64 {
        self.pixel_scale
    }

    pub fn set_pixel_scale(&mut self, value: f64) {
        self.pixel_scale = value;
    }

    pub fn edge_line_type(&self) -> i32 {
        self.edge_line_type
    }

    pub fn set_edge_line_type(&mut self, value: i32) {
        self.edge_line_type = value;
    }

    pub fn edge_line_width(&self) -> f64 {
        self.edge_line_width
    }

    pub fn set_edge_line_width(&mut self, value: f64) {
        self.edge_line_width = value;
    }

    pub fn edge_line_color(&self) -> &SolidColor {
        &self.edge_line_color
    }

    pub fn set_edge_line_color(&mut self, value: SolidColor) {
        self.edge_line_color = value;
    }

    pub fn fog_enabled(&self) -> bool {
        self.fog_enabled
    }

    pub fn enable_fog(&mut self, value: bool) {
        self.fog_enabled = value;
    }

    pub fn fog_start(&self) -> f32 {
        self.fog_start
    }

    pub fn set_fog_start(&mut self, value: f32) {
        self.fog_start = value;
    }

    pub fn fog_end(&self) -> f32 {
        self.fog_end
    }

    pub fn set_fog_end(&mut self, value: f32) {
        self.fog_end = value;
    }

    pub fn fog_color(&self) -> &SolidColor {
        &self.fog_color
    }

    pub fn set_fog_color(&mut self, value: SolidColor) {
        self.fog_color = value;
    }

    pub fn ao_enabled(&self) -> bool {
        self.ao_enabled
    }

    pub fn enable_ao(&mut self, value: bool) {
        self.ao_enabled = value;
    }

    pub fn polygon_mode(&self) -> i32 {
        self.polygon_mode
    }

    pub fn set_polygon_mode(&mut self, value: i32) {
        self.polygon_mode = value;
    }

    pub fn projection_matrix(&self) -> Matrix4 {
        self.projection_matrix
    }

    pub fn set_projection_matrix(&mut self, value: Matrix4) {
        self.projection_matrix = value;
    }

    pub fn viewport(&self) -> Vector4 {
        self.viewport
    }

    pub fn set_viewport(&mut self, value: Vector4) {
        self.viewport = value;
    }

    pub fn push_matrix(&mut self) {
        let top = self.model_view_matrix();
        self.matrix_stack.push(top);
    }

    pub fn pop_matrix(&mut self) -> Result<()> {
        if self.matrix_stack.len() <= 1 {
            return Err(CueMolError::invalid_argument(
                "DisplayContext matrix stack underflow",
            ));
        }
        self.matrix_stack.pop();
        Ok(())
    }

    pub fn mult_matrix(&mut self, value: Matrix4) {
        let top = self
            .matrix_stack
            .last_mut()
            .expect("display context matrix stack is empty");
        *top = top.mul_mat(value);
    }

    pub fn load_matrix(&mut self, value: Matrix4) {
        *self
            .matrix_stack
            .last_mut()
            .expect("display context matrix stack is empty") = value;
    }

    pub fn load_identity(&mut self) {
        self.load_matrix(Matrix4::IDENTITY);
    }

    pub fn clear_matrix_stack(&mut self) {
        self.matrix_stack.clear();
        self.matrix_stack.push(Matrix4::IDENTITY);
    }

    pub fn model_view_matrix(&self) -> Matrix4 {
        *self
            .matrix_stack
            .last()
            .expect("display context matrix stack is empty")
    }

    pub fn rotate(&mut self, value: Quat) {
        self.mult_matrix(value.to_rot_matrix());
    }

    pub fn scale(&mut self, value: Vector4) {
        self.mult_matrix(Matrix4::make_scale_mat(value));
    }

    pub fn translate(&mut self, value: Vector4) {
        self.mult_matrix(Matrix4::make_trans_mat(value));
    }

    pub fn transform_vector(&self, mut value: Vector4) -> Vector4 {
        value.w = 1.0;
        self.model_view_matrix().xform4d(&mut value);
        value
    }

    pub fn transform_normal(&self, mut value: Vector4) -> Vector4 {
        value.w = 0.0;
        self.model_view_matrix().xform4d(&mut value);
        value
    }

    pub fn device_rgb(&self, color: Option<&dyn AbstractColor>) -> Option<[f32; 3]> {
        color.map(|value| {
            let code = value.get_dev_code(self.scene_id);
            [
                f32::from(((code >> 16) & 0xff) as u8) / 255.0,
                f32::from(((code >> 8) & 0xff) as u8) / 255.0,
                f32::from((code & 0xff) as u8) / 255.0,
            ]
        })
    }

    pub fn device_rgba(&self, color: Option<&dyn AbstractColor>) -> Option<[f32; 4]> {
        color.map(|value| {
            let code = value.get_dev_code(self.scene_id);
            [
                f32::from(((code >> 16) & 0xff) as u8) / 255.0,
                f32::from(((code >> 8) & 0xff) as u8) / 255.0,
                f32::from((code & 0xff) as u8) / 255.0,
                f32::from(((code >> 24) & 0xff) as u8) / 255.0,
            ]
        })
    }

    pub fn draw_elem<F>(&self, attrs: &mut dyn AbstractDrawAttrs, create_vbo: F) -> bool
    where
        F: FnOnce(&dyn AbstractDrawAttrs) -> Option<Box<dyn VboRep>>,
    {
        assert!(
            matches!(
                attrs.typ(),
                crate::AbstDrawElem::VA_ATTRS | crate::AbstDrawElem::VA_ATTR_INDS
            ),
            "unsupported draw element type"
        );

        let mut representation = attrs.base_mut().draw_elem_mut().take_vbo();
        if let Some(cached) = representation.as_mut() {
            cached.bind();
            cached.update(attrs);
        } else {
            representation = create_vbo(attrs);
        }

        let Some(mut representation) = representation else {
            return false;
        };
        representation.set_attrib(attrs);
        representation.draw(attrs);
        representation.unbind(attrs);
        attrs.base_mut().draw_elem_mut().set_vbo(representation);
        true
    }

    pub fn make_orthographic_projection(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        slab_near: f32,
        slab_far: f32,
    ) -> Matrix4 {
        let right_left = right - left;
        let top_bottom = top - bottom;
        let far_near = slab_far - slab_near;
        Matrix4 {
            values: [
                [
                    f64::from(2.0 / right_left),
                    0.0,
                    0.0,
                    f64::from(-(right + left) / right_left),
                ],
                [
                    0.0,
                    f64::from(2.0 / top_bottom),
                    0.0,
                    f64::from(-(top + bottom) / top_bottom),
                ],
                [
                    0.0,
                    0.0,
                    f64::from(-2.0 / far_near),
                    f64::from(-(slab_far + slab_near) / far_near),
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn make_orthographic_projection_for_view(
        half_width: f32,
        aspect: f32,
        slab_near: f32,
        slab_far: f32,
    ) -> Matrix4 {
        Self::make_orthographic_projection(
            -half_width * aspect,
            half_width * aspect,
            -half_width,
            half_width,
            slab_near,
            slab_far,
        )
    }

    pub fn make_perspective_projection(
        half_width: f32,
        aspect: f32,
        slab_near: f32,
        slab_far: f32,
        distance: f32,
    ) -> Matrix4 {
        let scale = distance / half_width;
        Matrix4 {
            values: [
                [f64::from(scale / aspect), 0.0, 0.0, 0.0],
                [0.0, f64::from(scale), 0.0, 0.0],
                [
                    0.0,
                    0.0,
                    f64::from((slab_far + slab_near) / (slab_near - slab_far)),
                    f64::from((2.0 * slab_far * slab_near) / (slab_near - slab_far)),
                ],
                [0.0, 0.0, -1.0, 0.0],
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LineDrawAttr {
    position: Vector4,
    color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TriangleVertex {
    position: Vector4,
    normal: Vector4,
    color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawMode {
    None,
    Points,
    Lines,
    LineStrip,
    Triangles,
    TriangleStrip,
    TriangleFan,
}

#[derive(Debug)]
pub struct DisplayList {
    context: DisplayContext,
    previous_position_valid: bool,
    line_object: Option<LineGpuPrim>,
    line_buffer: Vec<LineDrawAttr>,
    vertex_line_width: f64,
    vertex_stipple: bool,
    triangle_object: Option<TrigGpuPrim>,
    triangle_buffer: Vec<TriangleVertex>,
    mesh: GrowMesh<u32>,
    triangle_mesh_object: Option<TrigGpuPrim>,
    valid: bool,
    current_color: u32,
    color_was_set: bool,
    current_normal: Vector4,
    draw_mode: DrawMode,
    previous_position: Vector4,
    previous_color: u32,
    detail: i32,
    cylinders: CylinderList<u32>,
    spheres: SphereList<u32>,
}

impl Default for DisplayList {
    fn default() -> Self {
        let context = DisplayContext::new();
        let current_color = SolidColor::from_rgb(0.5, 0.5, 0.5).code();
        Self {
            context,
            previous_position_valid: false,
            line_object: None,
            line_buffer: Vec::new(),
            vertex_line_width: -1.0,
            vertex_stipple: false,
            triangle_object: None,
            triangle_buffer: Vec::new(),
            mesh: GrowMesh::new(),
            triangle_mesh_object: None,
            valid: false,
            current_color,
            color_was_set: false,
            current_normal: Vector4::new3(1.0, 0.0, 0.0),
            draw_mode: DrawMode::None,
            previous_position: Vector4::ZERO,
            previous_color: 0,
            detail: 5,
            cylinders: CylinderList::default(),
            spheres: SphereList::default(),
        }
    }
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn context(&self) -> &DisplayContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut DisplayContext {
        &mut self.context
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn is_display_list(&self) -> bool {
        true
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn line_object(&self) -> Option<&LineGpuPrim> {
        self.line_object.as_ref()
    }

    pub fn triangle_object(&self) -> Option<&TrigGpuPrim> {
        self.triangle_object.as_ref()
    }

    pub fn triangle_mesh_object(&self) -> Option<&TrigGpuPrim> {
        self.triangle_mesh_object.as_ref()
    }

    pub fn pending_line_count(&self) -> usize {
        self.line_buffer.len() / 2
    }

    pub fn pending_triangle_count(&self) -> usize {
        self.triangle_buffer.len() / 3
    }

    pub fn pending_mesh_face_count(&self) -> usize {
        self.mesh.face_size()
    }

    pub fn color(&mut self, value: &dyn AbstractColor) {
        self.current_color = value.get_dev_code(self.context.scene_id());
        self.color_was_set = true;
    }

    pub fn set_color(&mut self, value: &dyn AbstractColor) {
        self.color(value);
    }

    pub fn set_color_code(&mut self, value: u32) {
        self.current_color = value;
        self.color_was_set = true;
    }

    pub fn normal(&mut self, value: Vector4) {
        let transformed = self.context.transform_normal(value);
        let length = transformed.length3d();
        self.current_normal = if length < EPSILON {
            Vector4::new3(1.0, 0.0, 0.0)
        } else {
            transformed.scale(1.0 / length)
        };
    }

    pub fn vertex(&mut self, value: Vector4) {
        let transformed = self.context.transform_vector(value);
        match self.draw_mode {
            DrawMode::None | DrawMode::Points => {}
            DrawMode::Lines => {
                if self.previous_position_valid {
                    self.draw_line(
                        transformed,
                        self.current_color,
                        self.previous_position,
                        self.previous_color,
                    );
                    self.previous_position_valid = false;
                } else {
                    self.previous_position = transformed;
                    self.previous_color = self.current_color;
                    self.previous_position_valid = true;
                }
                self.capture_line_style();
            }
            DrawMode::LineStrip => {
                if self.previous_position_valid {
                    self.draw_line(
                        transformed,
                        self.current_color,
                        self.previous_position,
                        self.previous_color,
                    );
                }
                self.previous_position = transformed;
                self.previous_color = self.current_color;
                self.previous_position_valid = true;
                self.capture_line_style();
            }
            DrawMode::Triangles => {
                self.triangle_buffer.push(TriangleVertex {
                    position: transformed,
                    normal: self.current_normal,
                    color: self.current_color,
                });
            }
            DrawMode::TriangleStrip | DrawMode::TriangleFan => {
                self.mesh
                    .add_vertex(transformed, self.current_normal, self.current_color);
            }
        }
    }

    pub fn set_polygon_mode(&mut self, value: i32) {
        match value {
            DisplayContext::POLY_POINT => {}
            DisplayContext::POLY_LINE => self.context.set_polygon_mode(DisplayContext::POLY_LINE),
            _ => self.context.set_polygon_mode(DisplayContext::POLY_FILL),
        }
    }

    pub fn start_points(&mut self) {
        self.draw_mode = DrawMode::Points;
    }

    pub fn start_polygon(&mut self) {}

    pub fn start_lines(&mut self) -> Result<()> {
        self.require_idle()?;
        self.draw_mode = DrawMode::Lines;
        self.vertex_line_width = -1.0;
        self.vertex_stipple = false;
        Ok(())
    }

    pub fn start_line_strip(&mut self) -> Result<()> {
        self.require_idle()?;
        self.draw_mode = DrawMode::LineStrip;
        self.vertex_line_width = -1.0;
        self.vertex_stipple = false;
        Ok(())
    }

    pub fn start_triangles(&mut self) -> Result<()> {
        self.require_idle()?;
        self.draw_mode = DrawMode::Triangles;
        Ok(())
    }

    pub fn start_triangle_strip(&mut self) -> Result<()> {
        self.require_idle()?;
        self.draw_mode = DrawMode::TriangleStrip;
        self.mesh.start(GrowMesh::<u32>::GM_TRIGSTRIP);
        Ok(())
    }

    pub fn start_triangle_fan(&mut self) -> Result<()> {
        self.require_idle()?;
        self.draw_mode = DrawMode::TriangleFan;
        self.mesh.start(GrowMesh::<u32>::GM_TRIGFAN);
        Ok(())
    }

    pub fn start_quad_strip(&mut self) {}

    pub fn start_quads(&mut self) {}

    pub fn end(&mut self) {
        match self.draw_mode {
            DrawMode::Lines | DrawMode::LineStrip => self.previous_position_valid = false,
            DrawMode::TriangleStrip | DrawMode::TriangleFan => self.mesh.end(),
            _ => {}
        }
        self.draw_mode = DrawMode::None;
    }

    pub fn sphere(&mut self) {
        let matrix = self.context.model_view_matrix();
        let center = self.context.transform_vector(Vector4::ZERO);
        if has_identity_linear_part(matrix, EPSILON) {
            self.spheres
                .add(center, 1.0, self.current_color, self.detail, None);
        } else {
            self.spheres.add(
                Vector4::ZERO,
                1.0,
                self.current_color,
                self.detail,
                Some(matrix),
            );
        }
    }

    pub fn cone(
        &mut self,
        radius1: f64,
        radius2: f64,
        position1: Vector4,
        position2: Vector4,
        cap: bool,
    ) {
        if (position1 - position2).length3d() < EPSILON {
            return;
        }
        let matrix = self.context.model_view_matrix();
        if has_unitary_linear_part(matrix, EPSILON) {
            let p1 = self.context.transform_vector(position1);
            let p2 = self.context.transform_vector(position2);
            self.cylinders.add(
                p1,
                p2,
                radius1,
                radius2,
                self.current_color,
                self.detail,
                cap,
                None,
            );
        } else {
            self.cylinders.add(
                position1,
                position2,
                radius1,
                radius2,
                self.current_color,
                self.detail,
                cap,
                Some(matrix),
            );
        }
    }

    pub fn draw_mesh(&mut self, source: &Mesh) -> Result<()> {
        if self.mesh.face_size() != 0 || self.triangle_mesh_object.is_some() {
            return Err(CueMolError::invalid_argument(
                "DisplayList already contains an indexed mesh",
            ));
        }
        if source.face_size() == 0 {
            return Ok(());
        }
        for index in 0..source.vertex_size() {
            let color = source
                .color_at(index)
                .ok_or_else(|| CueMolError::undefined_reference("mesh vertex color is not set"))?;
            self.mesh.add_vertex(
                source.vertex(index)?,
                source.normal_at(index)?,
                color.get_dev_code(self.context.scene_id()),
            );
        }
        for face in source.faces().chunks_exact(3) {
            let [vertex1, vertex2, vertex3] = [face[0], face[1], face[2]];
            if vertex1 < 0 || vertex2 < 0 || vertex3 < 0 {
                return Err(CueMolError::invalid_argument(
                    "mesh face contains a negative vertex index",
                ));
            }
            self.mesh
                .add_triangle(vertex1 as usize, vertex2 as usize, vertex3 as usize);
        }
        Ok(())
    }

    pub fn draw_grow_mesh(&mut self, source: &GrowMesh<u32>) -> Result<()> {
        if self.mesh.face_size() != 0 || self.triangle_mesh_object.is_some() {
            return Err(CueMolError::invalid_argument(
                "DisplayList already contains an indexed mesh",
            ));
        }
        if source.face_size() == 0 {
            return Ok(());
        }
        for vertex in source.vertices() {
            self.mesh.add_vertex(vertex.v, vertex.n, vertex.c);
        }
        for face in source.faces() {
            self.mesh.add_face(face.iv1, face.iv2, face.iv3, face.nmode);
        }
        Ok(())
    }

    pub fn record_start(&mut self) -> bool {
        self.line_object = None;
        self.triangle_object = None;
        self.triangle_mesh_object = None;
        self.line_buffer.clear();
        self.triangle_buffer.clear();
        self.mesh.clear();
        self.spheres.erase_all();
        self.cylinders.erase_all();
        self.valid = false;
        self.color_was_set = false;
        self.previous_position_valid = false;
        self.draw_mode = DrawMode::None;
        self.context.clear_matrix_stack();
        true
    }

    pub fn record_end(&mut self) {
        self.valid = true;
        self.convert_to_mesh();
    }

    pub fn build(&mut self, target: &DisplayContext) {
        if self.line_object.is_none() && !self.line_buffer.is_empty() {
            self.create_line_object(target);
        }
        if self.triangle_object.is_none() && !self.triangle_buffer.is_empty() {
            self.create_triangle_object(target);
        }
        if self.triangle_mesh_object.is_none() && self.mesh.face_size() > 0 {
            self.create_triangle_mesh_object(target);
        }
    }

    pub fn call_display_list(&mut self, target: &DisplayContext) {
        self.build(target);
        if let Some(object) = &self.line_object {
            object.draw();
        }
        if let Some(object) = &mut self.triangle_object {
            object.set_edge_line_type(target.edge_line_type());
            object.draw();
        }
        if let Some(object) = &mut self.triangle_mesh_object {
            object.set_edge_line_type(target.edge_line_type());
            object.draw();
        }
    }

    fn require_idle(&self) -> Result<()> {
        if self.draw_mode == DrawMode::None {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(
                "DisplayList drawing block is already active",
            ))
        }
    }

    fn capture_line_style(&mut self) {
        self.vertex_line_width = self.context.line_width();
        self.vertex_stipple = self.context.line_stipple() != u16::MAX;
    }

    fn draw_line(&mut self, p1: Vector4, c1: u32, p2: Vector4, c2: u32) {
        self.line_buffer.push(LineDrawAttr {
            position: p1,
            color: c1,
        });
        self.line_buffer.push(LineDrawAttr {
            position: p2,
            color: c2,
        });
    }

    fn convert_to_mesh(&mut self) {
        self.spheres.make_mesh(&mut self.mesh, None);
        self.spheres.erase_all();
        self.cylinders.make_mesh(&mut self.mesh);
        self.cylinders.erase_all();
    }

    fn create_line_object(&mut self, target: &DisplayContext) {
        let mut object = LineGpuPrim::new();
        object.init();
        object.alloc(self.line_buffer.len() / 2);
        for (index, pair) in self.line_buffer.chunks_exact(2).enumerate() {
            object.set_line(
                index,
                pair[0].position,
                pair[0].color,
                pair[1].position,
                pair[1].color,
            );
        }
        let mut width = self.vertex_line_width;
        if width < 0.0 {
            width = target.line_width();
            if width < 0.0 {
                width = 1.0;
            }
        }
        object.set_line_width((width * target.pixel_scale()) as f32);
        object.set_stipple(self.vertex_stipple);
        object.set_updated(true);
        if !self.color_was_set {
            object.set_use_vert_color(false);
        }
        self.line_buffer.clear();
        self.line_object = Some(object);
    }

    fn create_triangle_object(&mut self, target: &DisplayContext) {
        let mut object = TrigGpuPrim::new();
        object.init();
        object.set_polygon_mode(target.polygon_mode());
        object.alloc(self.triangle_buffer.len(), self.triangle_buffer.len() / 3);
        for (index, vertex) in self.triangle_buffer.iter().enumerate() {
            object.set_vertex(index, vertex.position);
            object.set_normal(index, vertex.normal);
            object.set_color(index, vertex.color);
        }
        for index in 0..self.triangle_buffer.len() / 3 {
            object.set_face(
                index,
                (index * 3) as u32,
                (index * 3 + 1) as u32,
                (index * 3 + 2) as u32,
            );
        }
        object.set_updated(true);
        self.triangle_buffer.clear();
        self.triangle_object = Some(object);
    }

    fn create_triangle_mesh_object(&mut self, target: &DisplayContext) {
        let mut object = TrigGpuPrim::new();
        object.init();
        object.set_polygon_mode(target.polygon_mode());
        object.alloc(self.mesh.vertex_size(), self.mesh.face_size());
        for (index, vertex) in self.mesh.vertices().iter().enumerate() {
            object.set_vertex(index, vertex.v);
            object.set_normal(index, vertex.n);
            object.set_color(index, vertex.c);
        }
        for (index, face) in self.mesh.faces().iter().enumerate() {
            object.set_face(index, face.iv1 as u32, face.iv2 as u32, face.iv3 as u32);
        }
        object.set_updated(true);
        self.mesh.clear();
        self.triangle_mesh_object = Some(object);
    }
}

fn has_identity_linear_part(matrix: Matrix4, tolerance: f64) -> bool {
    (0..3).all(|row| {
        (0..3).all(|column| {
            let expected = if row == column { 1.0 } else { 0.0 };
            (matrix.values[row][column] - expected).abs() < tolerance
        })
    })
}

fn has_unitary_linear_part(matrix: Matrix4, tolerance: f64) -> bool {
    (0..3).all(|row| {
        (0..3).all(|column| {
            let value = (0..3)
                .map(|index| matrix.values[index][row] * matrix.values[index][column])
                .sum::<f64>();
            let expected = if row == column { 1.0 } else { 0.0 };
            (value - expected).abs() < tolerance
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AbstDrawElem;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
    }

    #[test]
    fn display_context_tracks_matrix_stack_and_state() {
        let mut context = DisplayContext::new();
        context.translate(Vector4::new3(2.0, 3.0, 4.0));
        context.push_matrix();
        context.scale(Vector4::new3(2.0, 2.0, 2.0));
        let point = context.transform_vector(Vector4::new3(1.0, 1.0, 1.0));
        assert_eq!(point, Vector4::new4(4.0, 5.0, 6.0, 1.0));
        let normal = context.transform_normal(Vector4::new3(1.0, 0.0, 0.0));
        assert_eq!(normal, Vector4::new4(2.0, 0.0, 0.0, 0.0));
        context.pop_matrix().unwrap();
        assert_eq!(
            context.transform_vector(Vector4::ZERO),
            Vector4::new4(2.0, 3.0, 4.0, 1.0)
        );
        assert!(context.pop_matrix().is_err());
    }

    #[test]
    fn display_context_projection_and_device_color_helpers_match_reference() {
        let orthographic =
            DisplayContext::make_orthographic_projection(-4.0, 4.0, -2.0, 2.0, 1.0, 11.0);
        assert_eq!(orthographic.values[0][0], 0.25);
        assert_eq!(orthographic.values[1][1], 0.5);
        assert_close(orthographic.values[2][2], -0.2);
        assert_close(orthographic.values[2][3], -1.2);
        let perspective = DisplayContext::make_perspective_projection(2.0, 2.0, 1.0, 11.0, 10.0);
        assert_eq!(perspective.values[0][0], 2.5);
        assert_eq!(perspective.values[1][1], 5.0);
        assert_close(perspective.values[2][2], -1.2);
        assert_close(perspective.values[2][3], -2.2);
        assert_eq!(perspective.values[3][2], -1.0);

        let context = DisplayContext::new();
        let color = SolidColor::from_rgba_u8(64, 128, 255, 32);
        assert_eq!(
            context.device_rgba(Some(&color)),
            Some([64.0 / 255.0, 128.0 / 255.0, 1.0, 32.0 / 255.0])
        );
        assert_eq!(context.device_rgb(None), None);
    }

    #[test]
    fn line_recording_preserves_reference_order_and_style() {
        let mut list = DisplayList::new();
        list.record_start();
        list.context_mut().set_line_width(2.5);
        list.context_mut().set_line_stipple(0x00ff);
        list.context_mut().translate(Vector4::new3(10.0, 0.0, 0.0));
        let red = SolidColor::from_rgb_u8(255, 0, 0);
        let blue = SolidColor::from_rgb_u8(0, 0, 255);
        list.color(&red);
        list.start_lines().unwrap();
        list.vertex(Vector4::new3(1.0, 0.0, 0.0));
        list.color(&blue);
        list.vertex(Vector4::new3(2.0, 0.0, 0.0));
        list.end();
        list.record_end();

        let mut target = DisplayContext::new();
        target.set_pixel_scale(2.0);
        list.build(&target);
        let object = list.line_object().unwrap();
        assert_eq!(object.line_width(), 5.0);
        assert!(object.is_stipple());
        assert!(object.is_use_vert_color());
        let line = object.data().unwrap().at(0);
        assert_eq!([line.x1, line.x2], [12.0, 11.0]);
        assert_eq!([line.r1, line.g1, line.b1], [0, 0, 255]);
        assert_eq!([line.r2, line.g2, line.b2], [255, 0, 0]);
    }

    #[test]
    fn uncolored_lines_use_target_uniform_color() {
        let mut list = DisplayList::new();
        list.record_start();
        list.start_lines().unwrap();
        list.vertex(Vector4::ZERO);
        list.vertex(Vector4::new3(1.0, 0.0, 0.0));
        list.end();
        list.record_end();
        list.build(&DisplayContext::new());
        assert!(!list.line_object().unwrap().is_use_vert_color());
    }

    #[test]
    fn triangles_are_transformed_normalized_and_drawn() {
        let mut list = DisplayList::new();
        list.record_start();
        list.context_mut().scale(Vector4::new3(1.0, 2.0, 1.0));
        list.normal(Vector4::new3(0.0, 2.0, 0.0));
        list.start_triangles().unwrap();
        list.vertex(Vector4::new3(0.0, 0.0, 0.0));
        list.vertex(Vector4::new3(1.0, 0.0, 0.0));
        list.vertex(Vector4::new3(0.0, 1.0, 0.0));
        list.end();
        list.record_end();
        let mut target = DisplayContext::new();
        target.set_edge_line_type(DisplayContext::ELT_SILHOUETTE);
        list.call_display_list(&target);
        let object = list.triangle_object().unwrap();
        assert_eq!(object.vertex_size(), 3);
        assert_eq!(object.face_size(), 1);
        assert_eq!(object.edge_line_type(), DisplayContext::ELT_SILHOUETTE);
        assert_eq!(object.draw_count(), 1);
        let data = object.data().unwrap();
        assert_eq!([data.at(2).x, data.at(2).y], [0.0, 2.0]);
        assert_eq!(
            [data.at(0).nx, data.at(0).ny, data.at(0).nz],
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn strip_sphere_and_cone_convert_to_indexed_mesh() {
        let mut list = DisplayList::new();
        list.record_start();
        list.set_detail(1);
        list.start_triangle_strip().unwrap();
        for point in [
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
            Vector4::new3(1.0, 1.0, 0.0),
        ] {
            list.vertex(point);
        }
        list.end();
        list.context_mut().translate(Vector4::new3(3.0, 0.0, 0.0));
        list.sphere();
        list.cone(
            0.5,
            0.25,
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(0.0, 0.0, 2.0),
            true,
        );
        list.record_end();
        assert!(list.pending_mesh_face_count() > 2);
        list.build(&DisplayContext::new());
        let object = list.triangle_mesh_object().unwrap();
        assert!(object.vertex_size() > 4);
        assert!(object.face_size() > 2);
    }

    #[test]
    fn target_wireframe_mode_builds_line_indices() {
        let mut list = DisplayList::new();
        list.record_start();
        list.start_triangles().unwrap();
        list.vertex(Vector4::ZERO);
        list.vertex(Vector4::new3(1.0, 0.0, 0.0));
        list.vertex(Vector4::new3(0.0, 1.0, 0.0));
        list.end();
        list.record_end();
        let mut target = DisplayContext::new();
        target.set_polygon_mode(DisplayContext::POLY_LINE);
        list.build(&target);
        let object = list.triangle_object().unwrap();
        assert_eq!(object.face_size(), 1);
        let data = object.data().unwrap();
        assert_eq!(data.ind_size(), 6);
        assert_eq!(
            data.base().draw_elem().draw_mode(),
            AbstDrawElem::DRAW_LINES
        );
        assert_eq!(
            (0..6).map(|index| *data.at_ind(index)).collect::<Vec<_>>(),
            vec![0, 1, 1, 2, 2, 0]
        );
    }

    #[test]
    fn mesh_input_is_resolved_into_the_indexed_display_list() {
        let mut mesh = Mesh::new();
        mesh.init(3, 1);
        mesh.normal(Vector4::new3(0.0, 0.0, 1.0));
        mesh.set_color(SolidColor::from_rgb_u8(10, 20, 30));
        mesh.set_vertex(0, Vector4::ZERO).unwrap();
        mesh.set_vertex(1, Vector4::new3(1.0, 0.0, 0.0)).unwrap();
        mesh.set_vertex(2, Vector4::new3(0.0, 1.0, 0.0)).unwrap();
        mesh.set_face(0, 0, 1, 2).unwrap();

        let mut list = DisplayList::new();
        list.record_start();
        list.draw_mesh(&mesh).unwrap();
        list.record_end();
        list.build(&DisplayContext::new());
        let object = list.triangle_mesh_object().unwrap();
        assert_eq!(object.vertex_size(), 3);
        assert_eq!(object.face_size(), 1);
        let first = object.data().unwrap().at(0);
        assert_eq!([first.r, first.g, first.b, first.a], [10, 20, 30, 255]);
    }

    #[test]
    fn non_uniform_transforms_are_kept_for_round_objects() {
        let mut list = DisplayList::new();
        list.record_start();
        list.set_detail(1);
        list.context_mut().scale(Vector4::new3(2.0, 3.0, 4.0));
        list.sphere();
        list.cone(1.0, 1.0, Vector4::ZERO, Vector4::new3(0.0, 0.0, 1.0), false);
        list.record_end();
        let radius = list
            .mesh
            .vertices()
            .iter()
            .take(6)
            .map(|vertex| vertex.v.length3d())
            .fold(0.0, f64::max);
        assert_close(radius, 4.0);
    }
}
