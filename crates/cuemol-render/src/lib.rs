use cuemol_core::Vector4;
use cuemol_gfx::ColorSpec;
use cuemol_qsys::{InOutHandler, SceneExporter as QsysSceneExporter};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderAtom {
    pub center: Vector4,
    pub radius: f32,
    pub color: Option<ColorSpec>,
    pub object_uid: Option<u64>,
    pub atom_index: Option<usize>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderBond {
    pub from: Vector4,
    pub to: Vector4,
    pub radius: f32,
    pub color: Option<ColorSpec>,
    pub object_uid: Option<u64>,
    pub atom_indices: Option<(usize, usize)>,
    pub selected: bool,
}

/// A single triangle-mesh vertex for ribbon/cartoon geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: Vector4,
    pub normal: Vector4,
    /// Resolved RGBA in `[0, 1]`.
    pub color: [f32; 4],
}

/// An indexed triangle mesh, used for ribbon/cartoon geometry that cannot be
/// expressed with the sphere (atom) and line (bond) primitives.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

impl RenderMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSceneSnapshot {
    pub atoms: Vec<RenderAtom>,
    pub bonds: Vec<RenderBond>,
    pub meshes: Vec<RenderMesh>,
    pub bounds: Option<(Vector4, Vector4)>,
}

pub trait RenderBackend {
    type Frame;

    fn begin_frame(&mut self) -> Self::Frame;
    fn draw_atoms(&mut self, frame: &mut Self::Frame, atoms: &[RenderAtom]);
    fn draw_bonds(&mut self, _frame: &mut Self::Frame, _bonds: &[RenderBond]) {}
    fn end_frame(&mut self, frame: Self::Frame);
}

#[derive(Debug, Default)]
pub struct HeadlessBackend {
    pub atom_draw_calls: usize,
    pub bond_draw_calls: usize,
}

#[derive(Debug, Default)]
pub struct HeadlessFrame;

impl RenderBackend for HeadlessBackend {
    type Frame = HeadlessFrame;

    fn begin_frame(&mut self) -> Self::Frame {
        HeadlessFrame
    }

    fn draw_atoms(&mut self, _frame: &mut Self::Frame, atoms: &[RenderAtom]) {
        self.atom_draw_calls += atoms.len();
    }

    fn draw_bonds(&mut self, _frame: &mut Self::Frame, bonds: &[RenderBond]) {
        self.bond_draw_calls += bonds.len();
    }

    fn end_frame(&mut self, _frame: Self::Frame) {}
}

impl RenderAtom {
    pub fn new(center: Vector4, radius: f32, color: Option<ColorSpec>) -> Self {
        Self {
            center,
            radius,
            color,
            object_uid: None,
            atom_index: None,
            selected: false,
        }
    }

    pub fn with_picking(
        mut self,
        object_uid: Option<u64>,
        atom_index: Option<usize>,
        selected: bool,
    ) -> Self {
        self.object_uid = object_uid;
        self.atom_index = atom_index;
        self.selected = selected;
        self
    }
}

impl RenderBond {
    pub fn new(from: Vector4, to: Vector4, radius: f32, color: Option<ColorSpec>) -> Self {
        Self {
            from,
            to,
            radius,
            color,
            object_uid: None,
            atom_indices: None,
            selected: false,
        }
    }

    pub fn with_picking(
        mut self,
        object_uid: Option<u64>,
        atom_indices: Option<(usize, usize)>,
        selected: bool,
    ) -> Self {
        self.object_uid = object_uid;
        self.atom_indices = atom_indices;
        self.selected = selected;
        self
    }
}

impl RenderSceneSnapshot {
    pub fn empty() -> Self {
        Self {
            atoms: Vec::new(),
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        }
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    pub fn mesh_triangle_count(&self) -> usize {
        self.meshes.iter().map(RenderMesh::triangle_count).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportState {
    base: QsysSceneExporter,
    write_count: usize,
}

impl ExportState {
    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
    ) -> Self {
        Self {
            base: QsysSceneExporter::new(name, type_descr, file_ext),
            write_count: 0,
        }
    }

    pub fn base(&self) -> &QsysSceneExporter {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut QsysSceneExporter {
        &mut self.base
    }

    pub fn handler(&self) -> &InOutHandler {
        self.base.handler()
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        self.base.handler_mut()
    }

    pub fn name(&self) -> &str {
        self.handler().name()
    }

    pub fn type_descr(&self) -> &str {
        self.handler().type_descr()
    }

    pub fn file_ext(&self) -> &str {
        self.handler().file_ext()
    }

    pub fn write(&mut self) {
        self.write_count += 1;
    }

    pub fn write_count(&self) -> usize {
        self.write_count
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImgSceneExporter {
    state: ExportState,
    width: i32,
    height: i32,
    resolution_dpi: f64,
    use_alpha: bool,
    aa_opt: i32,
}

impl ImgSceneExporter {
    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
    ) -> Self {
        Self {
            state: ExportState::new(name, type_descr, file_ext),
            width: 500,
            height: 500,
            resolution_dpi: 72.0,
            use_alpha: false,
            aa_opt: 0,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ExportState {
        &mut self.state
    }

    pub fn scene(&self) -> &QsysSceneExporter {
        self.state.base()
    }

    pub fn scene_mut(&mut self) -> &mut QsysSceneExporter {
        self.state.base_mut()
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        self.state.handler_mut()
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn set_width(&mut self, value: i32) {
        self.width = value;
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn set_height(&mut self, value: i32) {
        self.height = value;
    }

    pub fn resolution_dpi(&self) -> f64 {
        self.resolution_dpi
    }

    pub fn set_resolution_dpi(&mut self, value: f64) {
        self.resolution_dpi = value;
    }

    pub fn use_alpha(&self) -> bool {
        self.use_alpha
    }

    pub fn set_use_alpha(&mut self, value: bool) {
        self.use_alpha = value;
    }

    pub fn aa_opt(&self) -> i32 {
        self.aa_opt
    }

    pub fn set_aa_opt(&mut self, value: i32) {
        self.aa_opt = value;
    }

    pub fn write(&mut self) {
        self.state.write();
    }

    pub fn write_count(&self) -> usize {
        self.state.write_count()
    }
}

impl Default for ImgSceneExporter {
    fn default() -> Self {
        Self::new("raw", "Raw RGB image (*.raw)", "*.raw")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PngSceneExporter {
    image: ImgSceneExporter,
    interlace: bool,
    compress: i32,
}

impl PngSceneExporter {
    pub fn new() -> Self {
        Self {
            image: ImgSceneExporter::new("png", "Portable Network Graphics image (*.png)", "*.png"),
            interlace: false,
            compress: 6,
        }
    }

    pub fn image(&self) -> &ImgSceneExporter {
        &self.image
    }

    pub fn image_mut(&mut self) -> &mut ImgSceneExporter {
        &mut self.image
    }

    pub fn handler(&self) -> &InOutHandler {
        self.image.handler()
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        self.image.handler_mut()
    }

    pub fn interlace(&self) -> bool {
        self.interlace
    }

    pub fn set_interlace(&mut self, value: bool) {
        self.interlace = value;
    }

    pub fn compress(&self) -> i32 {
        self.compress
    }

    pub fn set_compress(&mut self, value: i32) {
        self.compress = value;
    }

    pub fn write(&mut self) {
        self.image.write();
    }

    pub fn write_count(&self) -> usize {
        self.image.write_count()
    }
}

impl Default for PngSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PovSceneExporter {
    state: ExportState,
    perspective: bool,
    make_rel_inc_path: bool,
    use_clip_z: bool,
    use_post_blend: bool,
    blend_table: String,
    show_edge_lines: bool,
    crease_limit: f64,
    edge_rise: f64,
    use_pix_imgs: bool,
    img_file_names: String,
}

impl PovSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("pov", "POV-Ray SDL (*.pov)", "*.pov"),
            perspective: true,
            make_rel_inc_path: true,
            use_clip_z: false,
            use_post_blend: false,
            blend_table: String::new(),
            show_edge_lines: true,
            crease_limit: -1.0,
            edge_rise: 0.5,
            use_pix_imgs: false,
            img_file_names: String::new(),
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ExportState {
        &mut self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn perspective(&self) -> bool {
        self.perspective
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.perspective = value;
    }

    pub fn make_rel_inc_path(&self) -> bool {
        self.make_rel_inc_path
    }

    pub fn set_make_rel_inc_path(&mut self, value: bool) {
        self.make_rel_inc_path = value;
    }

    pub fn use_clip_z(&self) -> bool {
        self.use_clip_z
    }

    pub fn set_use_clip_z(&mut self, value: bool) {
        self.use_clip_z = value;
    }

    pub fn use_post_blend(&self) -> bool {
        self.use_post_blend
    }

    pub fn set_use_post_blend(&mut self, value: bool) {
        self.use_post_blend = value;
    }

    pub fn blend_table(&self) -> &str {
        &self.blend_table
    }

    pub fn set_blend_table(&mut self, value: impl Into<String>) {
        self.blend_table = value.into();
    }

    pub fn show_edge_lines(&self) -> bool {
        self.show_edge_lines
    }

    pub fn set_show_edge_lines(&mut self, value: bool) {
        self.show_edge_lines = value;
    }

    pub fn crease_limit(&self) -> f64 {
        self.crease_limit
    }

    pub fn set_crease_limit(&mut self, value: f64) {
        self.crease_limit = value;
    }

    pub fn edge_rise(&self) -> f64 {
        self.edge_rise
    }

    pub fn set_edge_rise(&mut self, value: f64) {
        self.edge_rise = value;
    }

    pub fn use_pix_imgs(&self) -> bool {
        self.use_pix_imgs
    }

    pub fn set_use_pix_imgs(&mut self, value: bool) {
        self.use_pix_imgs = value;
    }

    pub fn img_file_names(&self) -> &str {
        &self.img_file_names
    }

    pub fn set_img_file_names(&mut self, value: impl Into<String>) {
        self.img_file_names = value.into();
    }

    pub fn write(&mut self) {
        self.state.write();
    }

    pub fn write_mesh2_string(&self, vertices: &[Vector4], triangles: &[[usize; 3]]) -> String {
        let valid_triangles = triangles
            .iter()
            .copied()
            .filter(|triangle| triangle.iter().all(|index| *index < vertices.len()))
            .collect::<Vec<_>>();
        let mut output = String::new();
        output.push_str("mesh2 {\n");
        writeln!(&mut output, "  vertex_vectors {{ {}", vertices.len()).unwrap();
        for vertex in vertices {
            writeln!(
                &mut output,
                "    <{:.6}, {:.6}, {:.6}>,",
                vertex.x, vertex.y, vertex.z
            )
            .unwrap();
        }
        output.push_str("  }\n");
        writeln!(&mut output, "  face_indices {{ {}", valid_triangles.len()).unwrap();
        for [i1, i2, i3] in valid_triangles {
            writeln!(&mut output, "    <{i1}, {i2}, {i3}>,").unwrap();
        }
        output.push_str("  }\n");
        output.push_str("}\n");
        output
    }

    pub fn write_atoms_string(&self, atoms: &[RenderAtom]) -> String {
        let mut output = String::new();
        output.push_str("// CueMol POV-Ray scene fragment\n");
        for atom in atoms {
            let (r, g, b, filter) = pov_rgba(atom.color.as_ref());
            writeln!(
                &mut output,
                "sphere {{ <{:.6}, {:.6}, {:.6}>, {:.6} pigment {{ color rgbf <{:.6}, {:.6}, {:.6}, {:.6}> }} }}",
                atom.center.x,
                atom.center.y,
                atom.center.z,
                atom.radius,
                r,
                g,
                b,
                filter
            )
            .unwrap();
        }
        output
    }
}

impl Default for PovSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqoSceneExporter {
    state: ExportState,
    perspective: bool,
    use_clip_z: bool,
    grad_steps: i32,
}

impl MqoSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("mqo", "Metasequoia object file (*.mqo)", "*.mqo"),
            perspective: false,
            use_clip_z: false,
            grad_steps: 16,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn perspective(&self) -> bool {
        self.perspective
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.perspective = value;
    }

    pub fn use_clip_z(&self) -> bool {
        self.use_clip_z
    }

    pub fn set_use_clip_z(&mut self, value: bool) {
        self.use_clip_z = value;
    }

    pub fn grad_steps(&self) -> i32 {
        self.grad_steps
    }

    pub fn set_grad_steps(&mut self, value: i32) {
        self.grad_steps = value;
    }

    pub fn write(&mut self) {
        self.state.write();
    }
}

impl Default for MqoSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbpSceneExporter {
    state: ExportState,
    use_clip_z: bool,
    perspective: bool,
    grad_steps: i32,
}

impl WbpSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("wbp", "Warabi project file (*.wbp)", "*.wbp"),
            use_clip_z: false,
            perspective: false,
            grad_steps: 16,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ExportState {
        &mut self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn use_clip_z(&self) -> bool {
        self.use_clip_z
    }

    pub fn set_use_clip_z(&mut self, value: bool) {
        self.use_clip_z = value;
    }

    pub fn perspective(&self) -> bool {
        self.perspective
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.perspective = value;
    }

    pub fn grad_steps(&self) -> i32 {
        self.grad_steps
    }

    pub fn set_grad_steps(&mut self, value: i32) {
        self.grad_steps = value;
    }

    pub fn write(&mut self) {
        self.state.write();
    }
}

impl Default for WbpSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StlSceneExporter {
    state: ExportState,
    scale: f64,
}

impl StlSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("stl", "StereoLithography (*.stl)", "*.stl"),
            scale: 1.0,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn set_scale(&mut self, value: f64) {
        self.scale = value;
    }

    pub fn write(&mut self) {
        self.state.write();
    }

    pub fn write_binary_stl_bytes(
        &self,
        vertices: &[Vector4],
        triangles: &[[usize; 3]],
    ) -> Vec<u8> {
        let faces = valid_triangle_vertices(vertices, triangles)
            .into_iter()
            .map(|(v1, v2, v3)| {
                (
                    scale_vertex(v1, self.scale),
                    scale_vertex(v2, self.scale),
                    scale_vertex(v3, self.scale),
                )
            })
            .collect::<Vec<_>>();
        let mut output = vec![0_u8; 80];
        output.extend_from_slice(&(faces.len() as u32).to_le_bytes());
        for (v1, v2, v3) in faces {
            let normal = triangle_normal(v1, v2, v3);
            push_vec3_f32(&mut output, normal);
            push_vec3_f32(&mut output, v1);
            push_vec3_f32(&mut output, v2);
            push_vec3_f32(&mut output, v3);
            output.extend_from_slice(&0_u16.to_le_bytes());
        }
        output
    }

    pub fn write_ascii_stl_string(
        &self,
        name: &str,
        vertices: &[Vector4],
        triangles: &[[usize; 3]],
    ) -> String {
        let mut output = String::new();
        writeln!(&mut output, "solid {}", sanitize_stl_name(name)).unwrap();
        for (v1, v2, v3) in valid_triangle_vertices(vertices, triangles) {
            let v1 = scale_vertex(v1, self.scale);
            let v2 = scale_vertex(v2, self.scale);
            let v3 = scale_vertex(v3, self.scale);
            let normal = triangle_normal(v1, v2, v3);
            writeln!(
                &mut output,
                "  facet normal {:.6} {:.6} {:.6}",
                normal.x, normal.y, normal.z
            )
            .unwrap();
            output.push_str("    outer loop\n");
            write_stl_vertex(&mut output, v1);
            write_stl_vertex(&mut output, v2);
            write_stl_vertex(&mut output, v3);
            output.push_str("    endloop\n");
            output.push_str("  endfacet\n");
        }
        writeln!(&mut output, "endsolid {}", sanitize_stl_name(name)).unwrap();
        output
    }
}

impl Default for StlSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum LuxBackgroundMode {
    #[default]
    Transparent = 0,
    Wall = 1,
    Box = 2,
}

impl LuxBackgroundMode {
    pub const BG_TRANSP: i32 = 0;
    pub const BG_WALL: i32 = 1;
    pub const BG_BOX: i32 = 2;

    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            Self::BG_TRANSP => Some(Self::Transparent),
            Self::BG_WALL => Some(Self::Wall),
            Self::BG_BOX => Some(Self::Box),
            _ => None,
        }
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxRendSceneExporter {
    state: ExportState,
    halt_spp: i32,
    output_base: String,
    film_opts: String,
    bg_mode: LuxBackgroundMode,
}

impl LuxRendSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("luxrender", "LuxRender scene (*.lxs)", "*.lxs"),
            halt_spp: 10000,
            output_base: String::new(),
            film_opts: String::new(),
            bg_mode: LuxBackgroundMode::Transparent,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn halt_spp(&self) -> i32 {
        self.halt_spp
    }

    pub fn haltspp(&self) -> i32 {
        self.halt_spp()
    }

    pub fn set_halt_spp(&mut self, value: i32) {
        self.halt_spp = value;
    }

    pub fn set_haltspp(&mut self, value: i32) {
        self.set_halt_spp(value);
    }

    pub fn output_base(&self) -> &str {
        &self.output_base
    }

    pub fn outputbase(&self) -> &str {
        self.output_base()
    }

    pub fn set_output_base(&mut self, value: impl Into<String>) {
        self.output_base = value.into();
    }

    pub fn set_outputbase(&mut self, value: impl Into<String>) {
        self.set_output_base(value);
    }

    pub fn film_opts(&self) -> &str {
        &self.film_opts
    }

    pub fn filmopts(&self) -> &str {
        self.film_opts()
    }

    pub fn set_film_opts(&mut self, value: impl Into<String>) {
        self.film_opts = value.into();
    }

    pub fn set_filmopts(&mut self, value: impl Into<String>) {
        self.set_film_opts(value);
    }

    pub fn bg_mode(&self) -> LuxBackgroundMode {
        self.bg_mode
    }

    pub fn bgmode(&self) -> LuxBackgroundMode {
        self.bg_mode()
    }

    pub fn set_bg_mode(&mut self, value: LuxBackgroundMode) {
        self.bg_mode = value;
    }

    pub fn set_bgmode(&mut self, value: LuxBackgroundMode) {
        self.set_bg_mode(value);
    }

    pub fn write(&mut self) {
        self.state.write();
    }
}

impl Default for LuxRendSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxCoreSceneExporter {
    state: ExportState,
    halt_spp: i32,
    film_opts: String,
    abs_output_base: String,
    rel_output_base: String,
    bg_mode: LuxBackgroundMode,
}

impl LuxCoreSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("luxcore", "LuxCore scene (*.cfg)", "*.cfg"),
            halt_spp: 10000,
            film_opts: String::new(),
            abs_output_base: String::new(),
            rel_output_base: String::new(),
            bg_mode: LuxBackgroundMode::Transparent,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn handler(&self) -> &InOutHandler {
        self.state.handler()
    }

    pub fn halt_spp(&self) -> i32 {
        self.halt_spp
    }

    pub fn haltspp(&self) -> i32 {
        self.halt_spp()
    }

    pub fn set_halt_spp(&mut self, value: i32) {
        self.halt_spp = value;
    }

    pub fn set_haltspp(&mut self, value: i32) {
        self.set_halt_spp(value);
    }

    pub fn film_opts(&self) -> &str {
        &self.film_opts
    }

    pub fn filmopts(&self) -> &str {
        self.film_opts()
    }

    pub fn set_film_opts(&mut self, value: impl Into<String>) {
        self.film_opts = value.into();
    }

    pub fn set_filmopts(&mut self, value: impl Into<String>) {
        self.set_film_opts(value);
    }

    pub fn abs_output_base(&self) -> &str {
        &self.abs_output_base
    }

    pub fn set_abs_output_base(&mut self, value: impl Into<String>) {
        self.abs_output_base = value.into();
    }

    pub fn rel_output_base(&self) -> &str {
        &self.rel_output_base
    }

    pub fn set_rel_output_base(&mut self, value: impl Into<String>) {
        self.rel_output_base = value.into();
    }

    pub fn bg_mode(&self) -> LuxBackgroundMode {
        self.bg_mode
    }

    pub fn bgmode(&self) -> LuxBackgroundMode {
        self.bg_mode()
    }

    pub fn set_bg_mode(&mut self, value: LuxBackgroundMode) {
        self.bg_mode = value;
    }

    pub fn set_bgmode(&mut self, value: LuxBackgroundMode) {
        self.set_bg_mode(value);
    }

    pub fn write(&mut self) {
        self.state.write();
    }
}

impl Default for LuxCoreSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

fn valid_triangle_vertices(
    vertices: &[Vector4],
    triangles: &[[usize; 3]],
) -> Vec<(Vector4, Vector4, Vector4)> {
    triangles
        .iter()
        .filter_map(|[i1, i2, i3]| {
            Some((
                *vertices.get(*i1)?,
                *vertices.get(*i2)?,
                *vertices.get(*i3)?,
            ))
        })
        .collect()
}

fn scale_vertex(vertex: Vector4, scale: f64) -> Vector4 {
    Vector4::new3(vertex.x * scale, vertex.y * scale, vertex.z * scale)
}

fn triangle_normal(v1: Vector4, v2: Vector4, v3: Vector4) -> Vector4 {
    let normal = (v2 - v1).cross(v3 - v1);
    let length = normal.length3d();
    if length <= 1.0e-12 {
        Vector4::ZERO
    } else {
        normal.scale(1.0 / length)
    }
}

fn push_vec3_f32(output: &mut Vec<u8>, value: Vector4) {
    for component in [value.x, value.y, value.z] {
        output.extend_from_slice(&(component as f32).to_le_bytes());
    }
}

fn write_stl_vertex(output: &mut String, vertex: Vector4) {
    writeln!(
        output,
        "      vertex {:.6} {:.6} {:.6}",
        vertex.x, vertex.y, vertex.z
    )
    .unwrap();
}

fn sanitize_stl_name(name: &str) -> String {
    let name = name
        .chars()
        .map(|ch| if ch.is_control() { '_' } else { ch })
        .collect::<String>();
    if name.trim().is_empty() {
        "cuemol".to_string()
    } else {
        name
    }
}

fn pov_rgba(color: Option<&ColorSpec>) -> (f32, f32, f32, f32) {
    let rgba = color.and_then(ColorSpec::rgba8);
    let rgba = rgba.unwrap_or(cuemol_gfx::Rgba8 {
        r: 204,
        g: 204,
        b: 204,
        a: 255,
    });
    (
        f32::from(rgba.r) / 255.0,
        f32::from(rgba.g) / 255.0,
        f32::from(rgba.b) / 255.0,
        1.0 - f32::from(rgba.a) / 255.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_gfx::ColorBase;

    #[test]
    fn headless_backend_records_drawn_atoms() {
        let mut backend = HeadlessBackend::default();
        let mut frame = backend.begin_frame();
        backend.draw_atoms(
            &mut frame,
            &[RenderAtom {
                center: Vector4::new3(0.0, 0.0, 0.0),
                radius: 1.0,
                color: None,
                object_uid: None,
                atom_index: None,
                selected: false,
            }],
        );
        backend.end_frame(frame);
        assert_eq!(backend.atom_draw_calls, 1);
    }

    #[test]
    fn headless_backend_records_drawn_bonds() {
        let mut backend = HeadlessBackend::default();
        let mut frame = backend.begin_frame();
        backend.draw_bonds(
            &mut frame,
            &[RenderBond::new(
                Vector4::new3(0.0, 0.0, 0.0),
                Vector4::new3(1.0, 0.0, 0.0),
                0.1,
                None,
            )],
        );
        backend.end_frame(frame);
        assert_eq!(backend.bond_draw_calls, 1);
    }

    #[test]
    fn image_and_png_exporter_defaults_match_qif() {
        let mut img = ImgSceneExporter::default();
        assert_eq!(img.handler().name(), "raw");
        assert_eq!(img.width(), 500);
        assert_eq!(img.height(), 500);
        assert_eq!(img.resolution_dpi(), 72.0);
        assert!(!img.use_alpha());
        assert_eq!(img.aa_opt(), 0);
        img.write();
        assert_eq!(img.write_count(), 1);

        let png = PngSceneExporter::new();
        assert_eq!(png.handler().name(), "png");
        assert_eq!(png.image().width(), 500);
        assert!(!png.interlace());
        assert_eq!(png.compress(), 6);
    }

    #[test]
    fn vector_scene_exporter_defaults_match_reference() {
        let pov = PovSceneExporter::new();
        assert_eq!(pov.handler().name(), "pov");
        assert!(pov.perspective());
        assert!(pov.make_rel_inc_path());
        assert!(!pov.use_clip_z());
        assert!(!pov.use_post_blend());
        assert!(pov.show_edge_lines());
        assert_eq!(pov.crease_limit(), -1.0);
        assert_eq!(pov.edge_rise(), 0.5);
        assert!(!pov.use_pix_imgs());

        let mqo = MqoSceneExporter::new();
        assert_eq!(mqo.handler().name(), "mqo");
        assert_eq!(mqo.grad_steps(), 16);
        assert!(!mqo.use_clip_z());

        let wbp = WbpSceneExporter::new();
        assert_eq!(wbp.handler().name(), "wbp");
        assert_eq!(wbp.grad_steps(), 16);

        let stl = StlSceneExporter::new();
        assert_eq!(stl.handler().name(), "stl");
        assert_eq!(stl.scale(), 1.0);
    }

    #[test]
    fn lux_exporter_defaults_and_bg_modes_match_qif() {
        assert_eq!(LuxBackgroundMode::BG_TRANSP, 0);
        assert_eq!(LuxBackgroundMode::BG_WALL, 1);
        assert_eq!(LuxBackgroundMode::BG_BOX, 2);
        assert_eq!(LuxBackgroundMode::from_i32(2), Some(LuxBackgroundMode::Box));
        assert_eq!(LuxBackgroundMode::from_i32(9), None);

        let lux = LuxRendSceneExporter::new();
        assert_eq!(lux.handler().name(), "luxrender");
        assert_eq!(lux.halt_spp(), 10000);
        assert_eq!(lux.output_base(), "");
        assert_eq!(lux.film_opts(), "");
        assert_eq!(lux.bg_mode(), LuxBackgroundMode::Transparent);

        let core = LuxCoreSceneExporter::new();
        assert_eq!(core.handler().name(), "luxcore");
        assert_eq!(core.halt_spp(), 10000);
        assert_eq!(core.film_opts(), "");
        assert_eq!(core.bg_mode(), LuxBackgroundMode::Transparent);
    }

    #[test]
    fn stl_and_pov_exporters_write_geometry_fragments() {
        let vertices = [
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
        ];
        let triangles = [[0, 1, 2], [0, 2, 9]];

        let mut stl = StlSceneExporter::new();
        stl.set_scale(2.0);
        let bytes = stl.write_binary_stl_bytes(&vertices, &triangles);
        assert_eq!(bytes.len(), 84 + 50);
        assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 1);
        let ascii = stl.write_ascii_stl_string("tri", &vertices, &triangles);
        assert!(ascii.contains("solid tri"));
        assert!(ascii.contains("vertex 2.000000 0.000000 0.000000"));

        let pov = PovSceneExporter::new();
        let mesh = pov.write_mesh2_string(&vertices, &triangles);
        assert!(mesh.contains("vertex_vectors { 3"));
        assert!(mesh.contains("face_indices { 1"));

        let atoms = [RenderAtom::new(
            Vector4::new3(1.0, 2.0, 3.0),
            1.5,
            Some(ColorSpec {
                base: ColorBase::Html {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 128,
                },
                modifiers: Vec::new(),
            }),
        )];
        let spheres = pov.write_atoms_string(&atoms);
        assert!(spheres.contains("sphere { <1.000000, 2.000000, 3.000000>, 1.500000"));
        assert!(spheres.contains("rgbf <1.000000, 0.000000, 0.000000"));
    }
}
