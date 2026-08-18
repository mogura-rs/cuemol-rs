pub mod raytrace;

use cuemol_core::{CueMolError, Matrix4, Result, Uid, Vector4};
use cuemol_gfx::{CmsXform, ColorSpec, Rgba8};
use cuemol_qsys::{Camera, InOutHandler, SceneExporter as QsysSceneExporter};
use std::fmt::Write as _;
use std::ops::Range;

pub use raytrace::{
    render_umbreon_smoke, render_umbreon_snapshot, UmbreonDenoiseMode, UmbreonFrame,
    UmbreonRenderParams, UmbreonSceneExporter, UmbreonSmokeResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderEdgeMode {
    Edges,
    Silhouette,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderEdgeStyle {
    pub mode: RenderEdgeMode,
    pub width: f32,
    /// Resolved RGBA in `[0, 1]`.
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderHitData {
    pub index: i32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderAtom {
    pub center: Vector4,
    pub radius: f32,
    pub color: Option<ColorSpec>,
    pub object_uid: Option<u64>,
    pub atom_index: Option<usize>,
    pub selected: bool,
    /// Per-atom sphere UV tessellation level (C++ CPK/BallStick `detail`), or
    /// `None` for the default. `tessellate_snapshot` honours it so a styled
    /// renderer's `detail` shows in the GUI without a separate detailed pass.
    pub detail: Option<u32>,
    /// Optional C++ `egtype`/`eglinew`/`egcolor` sphere outline.
    pub edge: Option<RenderEdgeStyle>,
    /// Hidden point used only by the GUI hit-test pass.
    pub hit: Option<RenderHitData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderBond {
    pub from: Vector4,
    pub to: Vector4,
    pub radius: f32,
    pub color: Option<ColorSpec>,
    /// Optional color at `to`; screen-space lines interpolate from `color`.
    pub end_color: Option<ColorSpec>,
    pub object_uid: Option<u64>,
    pub atom_indices: Option<(usize, usize)>,
    pub selected: bool,
    /// Pixel width for primitives drawn in screen space instead of as a 3D tube.
    pub screen_width: Option<f32>,
    /// Use the C++ simple-line `0x00ff` stipple pattern (8 px on, 8 px off).
    pub screen_stipple: bool,
    /// Pixel displacement applied after projecting both endpoints.
    pub screen_offset: [f32; 2],
    /// Draw the projected `from` endpoint as a square point instead of a line.
    pub screen_point: bool,
    /// Optional C++ `egtype`/`eglinew`/`egcolor` cylinder outline.
    pub edge: Option<RenderEdgeStyle>,
}

/// A single triangle-mesh vertex for ribbon/cartoon geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: Vector4,
    pub normal: Vector4,
    /// Resolved RGBA in `[0, 1]`.
    pub color: [f32; 4],
}

impl MeshVertex {
    const CULL_BACK_FACES: f64 = 1.0;
    const EDGE_OUTLINE: f64 = 2.0;
    const SILHOUETTE_OUTLINE: f64 = 3.0;

    /// Mark this vertex as belonging to a mesh that discards back-facing
    /// fragments. The marker is stored in the otherwise-unused normal `w`
    /// component so existing mesh interchange remains unchanged.
    pub fn with_back_face_culling(mut self, enabled: bool) -> Self {
        self.normal.w = if enabled { Self::CULL_BACK_FACES } else { 0.0 };
        self
    }

    pub fn culls_back_faces(&self) -> bool {
        self.normal.w > 0.5
    }

    /// Mark a reversed, expanded mesh as an unlit edge-line outline.
    pub fn with_edge_outline(mut self, silhouette: bool) -> Self {
        self.normal.w = if silhouette {
            Self::SILHOUETTE_OUTLINE
        } else {
            Self::EDGE_OUTLINE
        };
        self
    }

    pub fn is_edge_outline(&self) -> bool {
        self.normal.w > 1.5
    }

    pub fn is_silhouette_outline(&self) -> bool {
        self.normal.w > 2.5
    }
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

    /// World-space vertex positions, for feeding the geometry exporters
    /// (`StlSceneExporter`, `PovSceneExporter`) whose writers take plain point
    /// lists rather than a `RenderMesh`.
    pub fn positions(&self) -> Vec<Vector4> {
        self.vertices.iter().map(|v| v.position).collect()
    }

    /// Triangle vertex-index triples (`indices` grouped in threes; a trailing
    /// partial group, which a well-formed mesh never has, is dropped).
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        self.indices
            .chunks_exact(3)
            .map(|c| [c[0] as usize, c[1] as usize, c[2] as usize])
            .collect()
    }

    /// Serialize as a Wavefront OBJ string (positions + per-vertex normals +
    /// triangle faces). The headless `RenderMesh` counterpart of
    /// `cuemol_surface::SurfaceMesh::to_obj_string`; OBJ indices are 1-based.
    pub fn to_obj_string(&self, name: &str) -> String {
        let mut out = String::new();
        writeln!(&mut out, "# CueMol render mesh: {name}").unwrap();
        for v in &self.vertices {
            writeln!(
                &mut out,
                "v {:.6} {:.6} {:.6}",
                v.position.x, v.position.y, v.position.z
            )
            .unwrap();
        }
        for v in &self.vertices {
            writeln!(
                &mut out,
                "vn {:.6} {:.6} {:.6}",
                v.normal.x, v.normal.y, v.normal.z
            )
            .unwrap();
        }
        for [i, j, k] in self.triangles() {
            writeln!(
                &mut out,
                "f {0}//{0} {1}//{1} {2}//{2}",
                i + 1,
                j + 1,
                k + 1
            )
            .unwrap();
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderTransparencyGroup {
    pub alpha: f32,
    pub atom_range: Range<usize>,
    pub bond_range: Range<usize>,
    pub mesh_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSceneSnapshot {
    pub atoms: Vec<RenderAtom>,
    pub bonds: Vec<RenderBond>,
    pub meshes: Vec<RenderMesh>,
    /// Renderer-sized transparency sections for blendpng-compatible output.
    pub transparency_groups: Vec<RenderTransparencyGroup>,
    pub bounds: Option<(Vector4, Vector4)>,
}

/// Apply CueMol's generic CMYK soft-proof transform to every resolved scene
/// colour. This is the profile-independent fallback used when color proofing
/// is enabled without an external ICC engine.
pub fn apply_generic_color_proof(snapshot: &mut RenderSceneSnapshot) {
    apply_color_transform(snapshot, |rgba| {
        let spec = ColorSpec {
            base: cuemol_gfx::ColorBase::Html {
                r: rgba.r,
                g: rgba.g,
                b: rgba.b,
                a: rgba.a,
            },
            modifiers: Vec::new(),
        };
        let code = spec
            .device_code_generic_cmyk()
            .unwrap_or_else(|| cuemol_gfx::rgba_code(rgba.r, rgba.g, rgba.b, rgba.a));
        Rgba8 {
            r: ((code >> 16) & 0xff) as u8,
            g: ((code >> 8) & 0xff) as u8,
            b: (code & 0xff) as u8,
            a: rgba.a,
        }
    });
}

/// Apply an ICC soft-proof transform created by LittleCMS to every resolved
/// scene colour. Alpha is preserved exactly.
pub fn apply_icc_color_proof(snapshot: &mut RenderSceneSnapshot, transform: &CmsXform) {
    apply_color_transform(snapshot, |rgba| {
        let code = transform.do_xform(cuemol_gfx::rgba_code(rgba.r, rgba.g, rgba.b, rgba.a));
        Rgba8 {
            r: ((code >> 16) & 0xff) as u8,
            g: ((code >> 8) & 0xff) as u8,
            b: (code & 0xff) as u8,
            a: rgba.a,
        }
    });
}

fn apply_color_transform(snapshot: &mut RenderSceneSnapshot, transform: impl Fn(Rgba8) -> Rgba8) {
    let proof_rgba = |color: &mut [f32; 4]| {
        let rgba = transform(Rgba8 {
            r: (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            a: (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        });
        *color = [
            f32::from(rgba.r) / 255.0,
            f32::from(rgba.g) / 255.0,
            f32::from(rgba.b) / 255.0,
            f32::from(rgba.a) / 255.0,
        ];
    };
    let proof_spec = |color: &mut Option<ColorSpec>| {
        let Some(rgba) = color.as_ref().and_then(ColorSpec::rgba8) else {
            return;
        };
        let rgba = transform(rgba);
        *color = Some(ColorSpec {
            base: cuemol_gfx::ColorBase::Html {
                r: rgba.r,
                g: rgba.g,
                b: rgba.b,
                a: rgba.a,
            },
            modifiers: Vec::new(),
        });
    };
    for atom in &mut snapshot.atoms {
        proof_spec(&mut atom.color);
        if let Some(edge) = &mut atom.edge {
            proof_rgba(&mut edge.color);
        }
    }
    for bond in &mut snapshot.bonds {
        proof_spec(&mut bond.color);
        proof_spec(&mut bond.end_color);
        if let Some(edge) = &mut bond.edge {
            proof_rgba(&mut edge.color);
        }
    }
    for mesh in &mut snapshot.meshes {
        for vertex in &mut mesh.vertices {
            let mut spec = Some(ColorSpec {
                base: cuemol_gfx::ColorBase::Rgb {
                    r: vertex.color[0],
                    g: vertex.color[1],
                    b: vertex.color[2],
                    a: vertex.color[3],
                },
                modifiers: Vec::new(),
            });
            proof_spec(&mut spec);
            if let Some(rgba) = spec.and_then(|value| value.rgba8()) {
                vertex.color = [
                    f32::from(rgba.r) / 255.0,
                    f32::from(rgba.g) / 255.0,
                    f32::from(rgba.b) / 255.0,
                    f32::from(rgba.a) / 255.0,
                ];
            }
        }
    }
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
            detail: None,
            edge: None,
            hit: None,
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

    /// Set the per-atom sphere tessellation detail (C++ `detail`).
    pub fn with_detail(mut self, detail: Option<u32>) -> Self {
        self.detail = detail;
        self
    }

    pub fn with_edge(mut self, edge: Option<RenderEdgeStyle>) -> Self {
        self.edge = edge;
        self
    }

    pub fn with_hit(mut self, index: i32, message: impl Into<String>) -> Self {
        self.hit = Some(RenderHitData {
            index,
            message: message.into(),
        });
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
            end_color: None,
            object_uid: None,
            atom_indices: None,
            selected: false,
            screen_width: None,
            screen_stipple: false,
            screen_offset: [0.0, 0.0],
            screen_point: false,
            edge: None,
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

    pub fn with_screen_style(mut self, width: f32, stipple: bool) -> Self {
        self.screen_width = Some(width.max(0.0));
        self.screen_stipple = stipple;
        self
    }

    pub fn with_screen_offset(mut self, x: f32, y: f32) -> Self {
        self.screen_offset = [x, y];
        self
    }

    pub fn as_screen_point(mut self) -> Self {
        self.screen_point = true;
        self
    }

    pub fn with_end_color(mut self, color: Option<ColorSpec>) -> Self {
        self.end_color = color;
        self
    }

    pub fn with_edge(mut self, edge: Option<RenderEdgeStyle>) -> Self {
        self.edge = edge;
        self
    }
}

impl RenderSceneSnapshot {
    pub fn empty() -> Self {
        Self {
            atoms: Vec::new(),
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        }
    }

    pub fn mark_transparency_group(&mut self, alpha: f32) {
        self.transparency_groups.push(RenderTransparencyGroup {
            alpha: alpha.clamp(0.0, 1.0),
            atom_range: 0..self.atoms.len(),
            bond_range: 0..self.bonds.len(),
            mesh_range: 0..self.meshes.len(),
        });
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.iter().filter(|atom| atom.hit.is_none()).count()
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

    /// Apply an affine transform `m` to all geometry in place.
    ///
    /// The headless equivalent of C++ `DisplayContext::multMatrix(xform)` applied
    /// to an object/renderer `xformMat` before `display()` (`Scene::displayRendImpl`,
    /// and the object xform propagated to atom positions via `MolAtom::getPos`).
    /// Positions transform as points (w=1); mesh normals transform by the rotation
    /// part only (w=0) and are renormalized — correct for the rigid superposition
    /// transforms that set `xformMat`. Bounds are recomputed from the transformed
    /// geometry so a moved object still auto-frames correctly.
    pub fn apply_xform(&mut self, m: &Matrix4) {
        for atom in &mut self.atoms {
            m.xform3d(&mut atom.center);
        }
        for bond in &mut self.bonds {
            m.xform3d(&mut bond.from);
            m.xform3d(&mut bond.to);
        }
        for mesh in &mut self.meshes {
            for vertex in &mut mesh.vertices {
                m.xform3d(&mut vertex.position);
                let render_marker = vertex.normal.w;
                // Rotation-only (w=0 drops the translation column).
                let rotated = m.transform_point(Vector4::new4(
                    vertex.normal.x,
                    vertex.normal.y,
                    vertex.normal.z,
                    0.0,
                ));
                let len =
                    (rotated.x * rotated.x + rotated.y * rotated.y + rotated.z * rotated.z).sqrt();
                if len > 1.0e-12 {
                    vertex.normal =
                        Vector4::new3(rotated.x / len, rotated.y / len, rotated.z / len);
                    vertex.normal.w = render_marker;
                }
            }
        }
        self.recompute_bounds();
    }

    /// Recompute the axis-aligned bounds from the current geometry (atom spheres,
    /// bond endpoints, mesh vertices). `None` when the snapshot holds no geometry.
    pub fn recompute_bounds(&mut self) {
        let mut min: Option<Vector4> = None;
        let mut max: Option<Vector4> = None;
        let mut include = |p: Vector4| match (&mut min, &mut max) {
            (Some(lo), Some(hi)) => {
                lo.x = lo.x.min(p.x);
                lo.y = lo.y.min(p.y);
                lo.z = lo.z.min(p.z);
                hi.x = hi.x.max(p.x);
                hi.y = hi.y.max(p.y);
                hi.z = hi.z.max(p.z);
            }
            _ => {
                min = Some(p);
                max = Some(p);
            }
        };
        for atom in self.atoms.iter().filter(|atom| atom.hit.is_none()) {
            let r = f64::from(atom.radius + atom.edge.map_or(0.0, |edge| edge.width.max(0.0)));
            include(Vector4::new3(
                atom.center.x - r,
                atom.center.y - r,
                atom.center.z - r,
            ));
            include(Vector4::new3(
                atom.center.x + r,
                atom.center.y + r,
                atom.center.z + r,
            ));
        }
        for bond in &self.bonds {
            let r = f64::from(bond.radius + bond.edge.map_or(0.0, |edge| edge.width.max(0.0)));
            for point in [bond.from, bond.to] {
                include(Vector4::new3(point.x - r, point.y - r, point.z - r));
                include(Vector4::new3(point.x + r, point.y + r, point.z + r));
            }
        }
        for mesh in &self.meshes {
            for vertex in &mesh.vertices {
                include(vertex.position);
            }
        }
        self.bounds = match (min, max) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            _ => None,
        };
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

impl std::ops::Deref for ExportState {
    type Target = QsysSceneExporter;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for ExportState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImgSceneExporter {
    state: ExportState,
    width: i32,
    height: i32,
    resolution_dpi: f64,
    use_alpha: bool,
    use_depth: bool,
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
            use_depth: false,
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

    pub fn get_width(&self) -> i32 {
        self.width()
    }

    pub fn set_width(&mut self, value: i32) {
        self.width = value;
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn get_height(&self) -> i32 {
        self.height()
    }

    pub fn set_height(&mut self, value: i32) {
        self.height = value;
    }

    pub fn resolution_dpi(&self) -> f64 {
        self.resolution_dpi
    }

    pub fn get_res_dpi(&self) -> f64 {
        self.resolution_dpi()
    }

    pub fn resoln(&self) -> f64 {
        self.resolution_dpi()
    }

    pub fn set_resolution_dpi(&mut self, value: f64) {
        self.resolution_dpi = value;
    }

    pub fn set_res_dpi(&mut self, value: f64) {
        self.set_resolution_dpi(value);
    }

    pub fn set_resoln(&mut self, value: f64) {
        self.set_resolution_dpi(value);
    }

    pub fn use_alpha(&self) -> bool {
        self.use_alpha
    }

    pub fn get_use_alpha(&self) -> bool {
        self.use_alpha()
    }

    pub fn alpha(&self) -> bool {
        self.use_alpha()
    }

    pub fn set_use_alpha(&mut self, value: bool) {
        self.use_alpha = value;
    }

    pub fn set_alpha(&mut self, value: bool) {
        self.set_use_alpha(value);
    }

    pub fn use_depth(&self) -> bool {
        self.use_depth
    }

    pub fn get_use_depth(&self) -> bool {
        self.use_depth()
    }

    pub fn depth(&self) -> bool {
        self.use_depth()
    }

    pub fn set_use_depth(&mut self, value: bool) {
        self.use_depth = value;
    }

    pub fn set_depth(&mut self, value: bool) {
        self.set_use_depth(value);
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

    pub fn attach(&mut self, scene_uid: Uid) {
        self.scene_mut().attach(scene_uid);
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.scene_mut().detach()
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

impl std::ops::Deref for PngSceneExporter {
    type Target = ImgSceneExporter;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl std::ops::DerefMut for PngSceneExporter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.image
    }
}

/// POV-Ray spec/flash lighting macros + lighting invocation, verbatim from C++
/// `PovDisplayContext` (the non-radiosity path). Appended after the camera so a
/// `write_scene_string` document is a complete, renderable scene.
const POV_LIGHTING: &str = concat!(
    "#ifndef (_shadow)\n  #declare _shadow = 0;\n#end\n",
    "#ifndef (_light_spread)\n  #declare _light_spread = 1;\n#end\n",
    "\n",
    "// Spec lighting macro\n",
    "#macro SpecLighting(aLightSpread, aDist, aInten, aShadow)\n",
    "#local v1=<1,1,1>;\n",
    "#local v2=<1,-0.5,-0.5>;\n",
    "#local vecsz=aDist*aLightSpread/10;\n",
    "light_source {\n",
    "   vnormalize(v1)*aDist*2\n",
    "   color rgb aInten\n",
    "#if (aLightSpread>1)\n",
    "   area_light vnormalize(v2)*vecsz, vcross(vnormalize(v1),vnormalize(v2))*vecsz, 10, 10\n",
    "   adaptive 2\n",
    "#else\n",
    "   parallel point_at _center\n",
    "#if (!aShadow)\n",
    "   shadowless\n",
    "#end\n",
    "#end\n",
    "}\n",
    "#end\n",
    "\n",
    "// Flash lighting macro\n",
    "#macro FlashLighting(aInten)\n",
    "light_source {\n",
    "   _camera_pos\n",
    "   color rgb aInten\n",
    "   shadowless\n",
    "#if (!_perspective)\n",
    "   parallel point_at _center\n",
    "#end\n",
    "}\n",
    "#end\n",
    "\n",
    "#ifdef (_radiosity)\n",
    "  #include \"rad_def.inc\"\n",
    "  global_settings { radiosity { Rad_Settings(_radiosity, off, off) } }\n",
    "  #ifndef (_light_inten)\n",
    "    #declare _light_inten=1.6;\n",
    "    #declare _amb_frac=0.5;\n",
    "    #declare _flash_frac=0.5;\n",
    "  #end\n",
    "  sphere {\n",
    "    _center, 1\n",
    "    texture { pigment { color rgb <1,1,1> } finish { diffuse 0 emission _light_inten*_amb_frac } }\n",
    "    hollow on no_shadow scale _distance*10\n",
    "  }\n",
    "#end\n",
    "\n",
    "#ifndef (_light_inten)\n",
    "    #declare _light_inten=1.3;\n",
    "    #declare _flash_frac=0.8/1.3;\n",
    "    #declare _amb_frac=0;\n",
    "#end\n",
    "SpecLighting(_light_spread, _distance, _light_inten*(1-_amb_frac)*(1-_flash_frac), _shadow)\n",
    "FlashLighting(_light_inten*(1-_amb_frac)*_flash_frac)\n",
);

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

    /// Write a complete POV-Ray scene to the configured handler path.
    pub fn write_scene_file(
        &mut self,
        mesh: &RenderMesh,
        center: Vector4,
        size: f64,
        perspective: bool,
    ) -> Result<()> {
        let output = self.write_scene_string(mesh, center, size, perspective);
        self.handler().write_string(&output)?;
        self.state.write();
        Ok(())
    }

    /// Write a complete POV-Ray scene using the full CueMol camera transform.
    pub fn write_scene_file_with_camera(
        &mut self,
        mesh: &RenderMesh,
        camera: &Camera,
    ) -> Result<()> {
        let output = self.write_scene_string_with_camera(mesh, camera)?;
        self.handler().write_string(&output)?;
        self.state.write();
        Ok(())
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

    /// Serialize a COMPLETE, renderable POV-Ray scene wrapping the tessellated
    /// mesh. Emits the C++ `PovDisplayContext` header — background, a camera
    /// (perspective/ortho, framed on `center`/`size`), `global_settings`, and
    /// the spec/flash lighting macros plus their invocation — then a coloured
    /// `mesh2` whose `texture_list` (one entry per distinct face colour)
    /// preserves element colours. Unlike [`Self::write_mesh2_string`] (a
    /// geometry fragment) this is a full scene. World-space geometry: the camera
    /// looks at the molecule `center` (C++ pre-transforms to origin; we keep
    /// world coords).
    pub fn write_scene_string(
        &self,
        mesh: &RenderMesh,
        center: Vector4,
        size: f64,
        perspective: bool,
    ) -> String {
        let distance = (size * 3.0).max(1.0);
        let zoom = (size * 1.2).max(1.0);
        self.write_scene_view_string(
            mesh,
            center,
            distance,
            zoom,
            perspective,
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
            Vector4::new3(0.0, 0.0, -1.0),
            self.use_clip_z.then_some(size * 2.0),
        )
    }

    /// Serialize a complete POV-Ray scene from a CueMol camera, preserving camera
    /// rotation, roll, distance, zoom, centre, and projection mode.
    pub fn write_scene_string_with_camera(
        &self,
        mesh: &RenderMesh,
        camera: &Camera,
    ) -> Result<String> {
        let inverse_rotation = camera.rotation().inv()?.to_matrix();
        let right = inverse_rotation.mul_vec(Vector4::new4(1.0, 0.0, 0.0, 0.0));
        let up = inverse_rotation.mul_vec(Vector4::new4(0.0, 1.0, 0.0, 0.0));
        let forward = inverse_rotation.mul_vec(Vector4::new4(0.0, 0.0, -1.0, 0.0));
        Ok(self.write_scene_view_string(
            mesh,
            camera.center(),
            camera.cam_dist(),
            camera.zoom(),
            camera.is_perspective(),
            right,
            up,
            forward,
            self.use_clip_z.then_some(camera.slab_depth()),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_scene_view_string(
        &self,
        mesh: &RenderMesh,
        center: Vector4,
        distance: f64,
        zoom: f64,
        perspective: bool,
        right: Vector4,
        up: Vector4,
        forward: Vector4,
        slab_depth: Option<f64>,
    ) -> String {
        let location = Vector4::new3(
            center.x - forward.x * distance,
            center.y - forward.y * distance,
            center.z - forward.z * distance,
        );
        let clip_planes = slab_depth.map(|depth| {
            let half_depth = depth.max(1.0e-3) * 0.5;
            let front =
                forward.x * center.x + forward.y * center.y + forward.z * center.z + half_depth;
            let back =
                -forward.x * center.x - forward.y * center.y - forward.z * center.z + half_depth;
            (front, back)
        });
        let persp = i32::from(perspective);
        let mut out = String::new();
        out.push_str("// CueMol POV-Ray scene\n");
        out.push_str("#declare _bgcolor = <1,1,1>;\n");
        out.push_str("#ifndef (_transpbg)\nbackground {color rgb _bgcolor}\n");
        out.push_str("#else\nbackground {color rgbt <1,1,1,0.999>}\n#end\n");
        writeln!(
            &mut out,
            "#declare _center = <{:.6},{:.6},{:.6}>;",
            center.x, center.y, center.z
        )
        .unwrap();
        writeln!(&mut out, "#declare _distance = {distance:.6};").unwrap();
        writeln!(
            &mut out,
            "#ifndef (_perspective)\n  #declare _perspective = {persp};\n#end"
        )
        .unwrap();
        out.push_str("#ifndef (_stereo)\n  #declare _stereo = 0;\n#end\n");
        out.push_str("#ifndef (_iod)\n  #declare _iod = 0.03;\n#end\n");
        writeln!(&mut out, "#declare _zoomy = {zoom:.6};").unwrap();
        out.push_str("#declare _zoomx = _zoomy * image_width/image_height;\n");
        out.push_str("#declare _fovx = 2.0*degrees( atan2(_zoomx, 2.0*_distance) );\n");
        writeln!(
            &mut out,
            "#declare _camera_pos = <{:.6},{:.6},{:.6}> + <{:.6},{:.6},{:.6}>*_stereo*_distance*_iod;",
            location.x, location.y, location.z, right.x, right.y, right.z
        )
        .unwrap();
        out.push_str("camera {\n");
        out.push_str(" #if (_perspective)\n perspective\n");
        writeln!(
            &mut out,
            " up <{:.6},{:.6},{:.6}> * image_height/image_width",
            up.x, up.y, up.z
        )
        .unwrap();
        writeln!(
            &mut out,
            " right <{:.6},{:.6},{:.6}>\n angle _fovx",
            right.x, right.y, right.z
        )
        .unwrap();
        writeln!(
            &mut out,
            " direction <{:.6},{:.6},{:.6}>\n location _camera_pos",
            forward.x, forward.y, forward.z
        )
        .unwrap();
        out.push_str(" #else\n orthographic\n");
        writeln!(
            &mut out,
            " direction <{:.6},{:.6},{:.6}>",
            forward.x, forward.y, forward.z
        )
        .unwrap();
        writeln!(
            &mut out,
            " up <{:.6},{:.6},{:.6}> * _zoomy",
            up.x, up.y, up.z
        )
        .unwrap();
        writeln!(
            &mut out,
            " right <{:.6},{:.6},{:.6}> * _zoomx",
            right.x, right.y, right.z
        )
        .unwrap();
        out.push_str(" location _camera_pos\n");
        out.push_str(" #end\n}\n");
        out.push_str("global_settings {\n  assumed_gamma 1.0\n}\n");
        out.push_str(POV_LIGHTING);

        let (mat_keys, face_mat) = face_material_table(mesh);
        out.push_str("mesh2 {\n");
        writeln!(&mut out, "  vertex_vectors {{ {}", mesh.vertices.len()).unwrap();
        for v in &mesh.vertices {
            writeln!(
                &mut out,
                "    <{:.6}, {:.6}, {:.6}>,",
                v.position.x, v.position.y, v.position.z
            )
            .unwrap();
        }
        out.push_str("  }\n");
        writeln!(&mut out, "  texture_list {{ {}", mat_keys.len()).unwrap();
        for c in &mat_keys {
            if self.use_post_blend {
                writeln!(
                    &mut out,
                    "    texture {{ pigment {{ color rgbt <{:.6}, {:.6}, {:.6}, {:.6}> }} }},",
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    1.0 - c[3] as f32 / 255.0,
                )
                .unwrap();
            } else {
                writeln!(
                    &mut out,
                    "    texture {{ pigment {{ color rgb <{:.6}, {:.6}, {:.6}> }} }},",
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                )
                .unwrap();
            }
        }
        out.push_str("  }\n");
        let tris = mesh.triangles();
        writeln!(&mut out, "  face_indices {{ {}", tris.len()).unwrap();
        for (f, [i, j, k]) in tris.iter().enumerate() {
            writeln!(&mut out, "    <{i}, {j}, {k}>, {},", face_mat[f]).unwrap();
        }
        out.push_str("  }\n");
        if let Some((front, back)) = clip_planes {
            writeln!(
                &mut out,
                "  clipped_by {{ plane {{ <{:.6},{:.6},{:.6}>, {:.6} }} plane {{ <{:.6},{:.6},{:.6}>, {:.6} }} }}",
                forward.x,
                forward.y,
                forward.z,
                front,
                -forward.x,
                -forward.y,
                -forward.z,
                back
            )
            .unwrap();
        }
        out.push_str("}\n");

        // NPR silhouette/crease edge lines (C++ show_edge_lines / GraphEdge),
        // drawn as thin black cylinders. The camera sits at center + distance·z.
        if self.show_edge_lines {
            let edges = extract_edge_lines(mesh, location, self.crease_limit);
            let ew = (zoom * (0.004 / 1.2)).max(1.0e-4);
            let clip = clip_planes.map_or_else(String::new, |(front, back)| {
                format!(
                    " clipped_by {{ plane {{ <{:.6},{:.6},{:.6}>, {:.6} }} plane {{ <{:.6},{:.6},{:.6}>, {:.6} }} }}",
                    forward.x,
                    forward.y,
                    forward.z,
                    front,
                    -forward.x,
                    -forward.y,
                    -forward.z,
                    back
                )
            });
            writeln!(&mut out, "// {} edge lines", edges.len()).unwrap();
            for e in &edges {
                writeln!(
                    &mut out,
                    "cylinder {{ <{:.6},{:.6},{:.6}>, <{:.6},{:.6},{:.6}>, {ew:.6} pigment {{ color rgb <0,0,0> }}{clip} }}",
                    e.a.x, e.a.y, e.a.z, e.b.x, e.b.y, e.b.z,
                )
                .unwrap();
            }
        }
        out
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

    /// Serialize a tessellated mesh as a Metasequoia (`.mqo`) text document —
    /// a faithful port of C++ `MqoDisplayContext` output: the Scene header, a
    /// Material block (one material per distinct face colour, the face colour
    /// being the mean of its three vertex colours = C++ `mixColor`), then an
    /// Object with `vertex`/`face` chunks. Face winding matches C++
    /// (`3 V(i1 i3 i2) M(im)`); MQO indices are 0-based.
    pub fn write_mqo_string(&self, name: &str, mesh: &RenderMesh) -> String {
        let tris = mesh.triangles();
        let (mat_keys, face_mat) = face_material_table(mesh);

        let mut out = String::new();
        out.push_str("Metasequoia Document\n");
        out.push_str("Format Text Ver 1.0\n\n");
        out.push_str("Scene {\n");
        out.push_str("    pos 0.0000 0.0000 100.0000\n");
        out.push_str("    lookat 0.0000 0.0000 0.0000\n");
        out.push_str("    head 0.0000\n");
        out.push_str("    pich 0.0000\n");
        out.push_str("    ortho 0\n");
        out.push_str("    zoom2 10.0000\n");
        out.push_str("    amb 0.250 0.250 0.250\n");
        out.push_str("}\n");

        writeln!(&mut out, "Material {} {{", mat_keys.len()).unwrap();
        for (i, c) in mat_keys.iter().enumerate() {
            writeln!(
                &mut out,
                "    \"{name}_mat{i}\" col({:.6} {:.6} {:.6} {:.6}) dif(0.7) amb(0.1) emi(0.000) spc(0.3) power(0.00)",
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                c[3] as f32 / 255.0,
            )
            .unwrap();
        }
        out.push_str("}\n");

        writeln!(&mut out, "Object \"{name}\" {{").unwrap();
        out.push_str("    visible 15\n");
        out.push_str("    locking 0\n");
        out.push_str("    shading 1\n");
        out.push_str("    facet 59.5\n");
        out.push_str("    color 1 1 1\n");
        out.push_str("    color_type 0\n");
        writeln!(&mut out, "    vertex {} {{", mesh.vertices.len()).unwrap();
        for v in &mesh.vertices {
            writeln!(
                &mut out,
                "{:.6} {:.6} {:.6}",
                v.position.x, v.position.y, v.position.z
            )
            .unwrap();
        }
        out.push_str("    }\n");
        writeln!(&mut out, "    face {} {{", tris.len()).unwrap();
        for (f, [i, j, k]) in tris.iter().enumerate() {
            writeln!(&mut out, "3 V({i} {k} {j}) M({})", face_mat[f]).unwrap();
        }
        out.push_str("    }\n");
        out.push_str("}\n");
        out.push_str("Eof\n");
        out
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

    /// Write an MQO document to the configured handler path.
    pub fn write_mesh_file(&mut self, name: &str, mesh: &RenderMesh) -> Result<()> {
        let output = self.write_mqo_string(name, mesh);
        self.handler().write_string(&output)?;
        self.state.write();
        Ok(())
    }
}

impl Default for MqoSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Default Warabi material body (gradient + highlight), appended after the
/// per-material `Color`/`Opacity` lines — verbatim from C++ `writeDefaultMaterial`.
const WBP_DEFAULT_MATERIAL: &str = concat!(
    "  Gradient : [\n",
    "    { Position : 0.2, Mode : 0, Value : 0.75, TextureScale : 1, TextureOpacity : 1, TextureBlendMode : \"Multiply\", TextureColorOverride : false, TextureOverrideColor : [ 1, 0, 0 ] }\n",
    "    { Position : 0.4, Mode : 0, Value : 0.88, TextureScale : 1, TextureOpacity : 1, TextureBlendMode : \"Multiply\", TextureColorOverride : false, TextureOverrideColor : [ 1, 0, 0 ] }\n",
    "    { Position : 0.6, Mode : 0, Value : 0.88, TextureScale : 1, TextureOpacity : 1, TextureBlendMode : \"Multiply\", TextureColorOverride : false, TextureOverrideColor : [ 1, 0, 0 ] }\n",
    "    { Position : 0.8, Mode : 0, Value : 1, TextureScale : 1, TextureOpacity : 1, TextureBlendMode : \"Multiply\", TextureColorOverride : false, TextureOverrideColor : [ 1, 0, 0 ] }\n",
    "  ]\n",
    "  TextureBlendMode : \"Multiply\"\n",
    "  TextureOpacity : 1\n",
    "  Highlight : false\n",
    "  HighlightSize : 0.3\n",
    "  HighlightSpread : 0\n",
    "  HighlightColor : [ 1, 1, 1 ]\n",
    "  HighlightBlendMode : \"Add\"\n",
    "  HighlightOpacity : 1\n",
    "  Group : -1\n",
);

/// Default Warabi brush block (C++ `writeBrush` no-defs branch).
const WBP_DEFAULT_BRUSH: &str = concat!(
    "Brush \"default\" : {\n",
    "  Width : 1\n",
    "  Color : [ 0, 0, 0 ]\n",
    "  Opacity : 0.7\n",
    "  BlendMode :\"Normal\"\n",
    "  Roundness : 1\n",
    "  Angle : 0\n",
    "  Default : true\n",
    "}\n",
);

/// Scene preamble, opening `Scene { ... Children { External {` (C++
/// `writeScenePreamble`).
const WBP_SCENE_PREAMBLE: &str = concat!(
    "Scene : {\n",
    "  Visible : true\n",
    "  Position : [ 0, 0, 0 ]\n",
    "  Scale : [ 1, 1, 1 ]\n",
    "  Rotation : [ 0, 0, 0 ]\n",
    "  Layer : 0\n",
    "  Group : -1\n",
    "  DropShadow : true\n",
    "  ReceiveShadow : true\n",
    "  ShadowOnly : false\n",
    "  BrushSet : {\n",
    "    NormalEdgeThreshold : 1.0472\n",
    "    SilhouettStyle : 1\n",
    "    ContourStyle : 1\n",
    "    ObjectBorderStyle : 1\n",
    "    MaterialBorderStyle : 1\n",
    "    NormalEdgeStyle : 1\n",
    "  }\n",
    "  Children : {\n",
    "    External : {\n",
);

/// External-import body following `FileName` (C++ `writeMqoImport`).
const WBP_IMPORT_OPTIONS: &str = concat!(
    "      ImportOptions : {\n",
    "        Scale : 1\n",
    "        Digits : 0\n",
    "        Axes : 1056\n",
    "      }\n",
    "      Visible : true\n",
    "      Position : [ 0, 0, 0 ]\n",
    "      Scale : [ 1, 1, 1 ]\n",
    "      Rotation : [ 0, 0, 0 ]\n",
    "      Layer : 0\n",
    "      Group : -1\n",
    "      DropShadow : true\n",
    "      ReceiveShadow : true\n",
    "      ShadowOnly : false\n",
    "      BrushSet : {\n",
    "        NormalEdgeThreshold : 1.0472\n",
    "        SilhouettStyle : 1\n",
    "        ContourStyle : 1\n",
    "        ObjectBorderStyle : 1\n",
    "        MaterialBorderStyle : 1\n",
    "        NormalEdgeStyle : 1\n",
    "      }\n",
);

/// Per-object body following `Object "name" : {` (C++ `writeMqoObj`).
const WBP_OBJECT_BODY: &str = concat!(
    "          Visible : true\n",
    "          Position : [ 0, 0, 0 ]\n",
    "          Scale : [ 1, 1, 1 ]\n",
    "          Rotation : [ 0, 0, 0 ]\n",
    "          Layer : 0\n",
    "          Group : -1\n",
    "          DropShadow : true\n",
    "          ReceiveShadow : true\n",
    "          ShadowOnly : false\n",
    "          BrushSet : {\n",
    "            NormalEdgeThreshold : 1.0472\n",
    "            SilhouettStyle : 2\n",
    "            SilhouettBrush : \"default\"\n",
    "            ContourStyle : 2\n",
    "            ContourBrush : \"default\"\n",
    "            ObjectBorderStyle : 0\n",
    "            MaterialBorderStyle : 0\n",
    "            NormalEdgeStyle : 2\n",
    "            NormalEdgeBrush : \"default\"\n",
    "          }\n",
    "          Smoothing : true\n",
    "          SmoothingAngle : 1.03847\n",
    "          DrawBackFace : false\n",
    "        }\n",
);

/// DirectionalLight + orthographic Camera (C++ `writeWbp` tail).
const WBP_LIGHT_AND_CAMERA: &str = concat!(
    "    DirectionalLight \"Light 1\" : {\n",
    "      Position : [ 500, 500, 500 ]\n",
    "      Color : [ 1, 1, 1 ]\n",
    "      Visible : true\n",
    "      Default : true\n",
    "    }\n",
    "    Camera \"camera1\" : {\n",
    "      Position : [ 0, 0, 1000 ]\n",
    "      Focus : [ 0, 0, 0 ]\n",
    "      Up : [ 0, 1, 0 ]\n",
    "      Fov : 0.100000\n",
    "      Visible : true\n",
    "    }\n",
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbpSceneExporter {
    state: ExportState,
    camera_name: String,
    use_clip_z: bool,
    perspective: bool,
    grad_steps: i32,
}

impl WbpSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("wbp", "Warabi project file (*.wbp)", "*.wbp"),
            camera_name: String::new(),
            use_clip_z: false,
            perspective: false,
            grad_steps: 16,
        }
    }

    /// Serialize a Warabi project (`.wbp`) referencing an external `.mqo`
    /// geometry file — a faithful port of C++ `WbpSceneExporter` (which itself
    /// writes the geometry via `MqoDisplayContext` and wraps it in a `.wbp`):
    /// the `# Warabi Project 1.1 utf8` header, a Material per distinct face
    /// colour, a default Brush, then a Scene whose `Children/External` imports
    /// the `.mqo` and holds the Object, a DirectionalLight, and a Camera.
    /// `mqo_filename` is the relative `.mqo` reference.
    pub fn write_wbp_string(&self, name: &str, mesh: &RenderMesh, mqo_filename: &str) -> String {
        let (mat_keys, _) = face_material_table(mesh);
        let mut out = String::new();
        out.push_str("# Warabi Project 1.1 utf8\n");
        for (i, c) in mat_keys.iter().enumerate() {
            writeln!(&mut out, "Material \"{name}_mat{i}\" : {{").unwrap();
            writeln!(
                &mut out,
                "  Color : [{:.6}, {:.6}, {:.6}]",
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
            )
            .unwrap();
            writeln!(&mut out, "  Opacity : {:.6}", c[3] as f32 / 255.0).unwrap();
            out.push_str(WBP_DEFAULT_MATERIAL);
            out.push_str("}\n");
        }
        out.push_str(WBP_DEFAULT_BRUSH);
        out.push_str(WBP_SCENE_PREAMBLE);
        writeln!(&mut out, "      FileName : \"{mqo_filename}\"").unwrap();
        out.push_str(WBP_IMPORT_OPTIONS);
        out.push_str("      Children : {\n");
        writeln!(&mut out, "        Object \"{name}\" : {{").unwrap();
        out.push_str(WBP_OBJECT_BODY);
        out.push_str("      }\n"); // close Children
        out.push_str("    }\n"); // close External
        out.push_str(WBP_LIGHT_AND_CAMERA);
        out.push_str("  }\n"); // close scene Children
        out.push_str("}\n"); // close Scene
        out
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

    pub fn camera(&self) -> &str {
        &self.camera_name
    }

    pub fn get_camera_name(&self) -> &str {
        self.camera()
    }

    pub fn set_camera(&mut self, value: impl Into<String>) {
        self.camera_name = value.into();
    }

    pub fn set_camera_name(&mut self, value: impl Into<String>) {
        self.set_camera(value);
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

    /// Write a Warabi project and its sibling MQO geometry file.
    pub fn write_scene_files(&mut self, name: &str, mesh: &RenderMesh) -> Result<()> {
        if self.handler().path().is_empty() {
            return Err(CueMolError::invalid_argument("Warabi output path is empty"));
        }
        let project_path = std::path::Path::new(self.handler().path());
        let mqo_path = project_path.with_extension("mqo");
        let mqo_filename = mqo_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("scene.mqo");
        let mqo = MqoSceneExporter::new().write_mqo_string(name, mesh);
        std::fs::write(&mqo_path, mqo)?;
        let project = self.write_wbp_string(name, mesh, mqo_filename);
        self.handler().write_string(&project)?;
        self.state.write();
        Ok(())
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

    /// Write binary STL geometry to the configured handler path.
    pub fn write_mesh_file(&mut self, mesh: &RenderMesh) -> Result<()> {
        let bytes = self.write_binary_stl_bytes(&mesh.positions(), &mesh.triangles());
        self.handler().write_bytes(&bytes)?;
        self.state.write();
        Ok(())
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

    /// Serialize a tessellated mesh as a LuxRender `Shape "mesh"` geometry block
    /// (triangle indices + point list) — the geometry CORE of C++
    /// `LuxRendDisplayContext` output, paralleling the POV mesh writer. This is
    /// a geometry FRAGMENT (no Film/Camera/LightSource/WorldBegin scene chrome —
    /// use [`Self::write_scene_string`] for the complete scene document).
    pub fn write_mesh_string(&self, vertices: &[Vector4], triangles: &[[usize; 3]]) -> String {
        let valid: Vec<[usize; 3]> = triangles
            .iter()
            .copied()
            .filter(|t| t.iter().all(|i| *i < vertices.len()))
            .collect();
        let mut out = String::new();
        out.push_str("Shape \"mesh\"\n");
        out.push_str("  \"integer triindices\" [");
        for (n, [i, j, k]) in valid.iter().enumerate() {
            if n > 0 {
                out.push(' ');
            }
            write!(&mut out, "{i} {j} {k}").unwrap();
        }
        out.push_str("]\n");
        out.push_str("  \"point P\" [");
        for (n, v) in vertices.iter().enumerate() {
            if n > 0 {
                out.push(' ');
            }
            write!(&mut out, "{:.6} {:.6} {:.6}", v.x, v.y, v.z).unwrap();
        }
        out.push_str("]\n");
        out
    }

    /// Serialize a COMPLETE, renderable LuxRender scene (`.lxs`) wrapping the
    /// mesh — a faithful port of C++ `LuxRendDisplayContext`: a `LookAt`/`Camera`
    /// framed on `center`/`size`, a `Film "fleximage"`, `PixelFilter`/`Sampler`,
    /// `WorldBegin` … a distant light, a matte `Material`, the `Shape "mesh"`
    /// geometry, `WorldEnd`. Unlike [`Self::write_mesh_string`] (a geometry
    /// fragment) this is a full scene. World-space: the camera looks at the
    /// molecule `center` (C++ pre-transforms to origin; we keep world coords).
    pub fn write_scene_string(
        &self,
        mesh: &RenderMesh,
        center: Vector4,
        size: f64,
        perspective: bool,
    ) -> String {
        let dist = (size * 3.0).max(1.0);
        let zoom = (size * 1.2).max(1.0);
        let mut out = String::new();
        out.push_str("# CueMol LuxRender exporter output\n\n");
        writeln!(
            &mut out,
            "LookAt {:.6} {:.6} {:.6}  {:.6} {:.6} {:.6}  0 1 0",
            center.x,
            center.y,
            center.z + dist,
            center.x,
            center.y,
            center.z,
        )
        .unwrap();
        if perspective {
            out.push_str("Camera \"perspective\" \"float fov\" [45]\n");
        } else {
            writeln!(
                &mut out,
                "Camera \"orthographic\" \"float screenwindow\" [{:.6} {:.6} {:.6} {:.6}]",
                -zoom / 2.0,
                zoom / 2.0,
                -zoom / 2.0,
                zoom / 2.0,
            )
            .unwrap();
        }
        out.push_str("Film \"fleximage\"\n");
        out.push_str("     \"integer xresolution\" [640]\n");
        out.push_str("     \"integer yresolution\" [480]\n");
        out.push_str("     \"bool write_png\" [\"true\"]\n");
        out.push_str("     \"string write_png_channels\" [\"RGBA\"]\n");
        writeln!(&mut out, "     \"integer haltspp\" [{}]", self.halt_spp()).unwrap();
        out.push_str("PixelFilter \"mitchell\" \"float xwidth\" [2] \"float ywidth\" [2]\n");
        out.push_str("Sampler \"metropolis\"\n");
        out.push_str("\nWorldBegin\n\n");
        out.push_str("AttributeBegin # Light\n");
        out.push_str("LightSource \"distant\"\n  \"color L\" [1 1 1]\n");
        out.push_str("AttributeEnd\n\n");
        out.push_str("Material \"matte\" \"color Kd\" [0.7 0.7 0.7]\n");
        out.push_str("AttributeBegin\n");
        out.push_str(&self.write_mesh_string(&mesh.positions(), &mesh.triangles()));
        out.push_str("AttributeEnd\n\n");
        out.push_str("WorldEnd\n");
        out
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

    /// Write a complete LuxRender scene to the configured handler path.
    pub fn write_scene_file(
        &mut self,
        mesh: &RenderMesh,
        center: Vector4,
        size: f64,
        perspective: bool,
    ) -> Result<()> {
        let output = self.write_scene_string(mesh, center, size, perspective);
        self.handler().write_string(&output)?;
        self.state.write();
        Ok(())
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

    /// ASCII PLY geometry for the mesh — the internal geometry format C++
    /// `LuxCoreDisplayContext` writes alongside the `.scn` (faithful header:
    /// float x/y/z + nx/ny/nz + uchar red/green/blue per vertex, `3 i j k`
    /// faces). Kept internal to LuxCore (C++ has no standalone PLY exporter, so
    /// neither do we — match the reference, don't out-feature it).
    pub fn write_ply_string(&self, mesh: &RenderMesh) -> String {
        let tris = mesh.triangles();
        let mut out = String::new();
        out.push_str("ply\nformat ascii 1.0\ncomment Created by CueMol\n");
        writeln!(&mut out, "element vertex {}", mesh.vertices.len()).unwrap();
        out.push_str("property float x\nproperty float y\nproperty float z\n");
        out.push_str("property float nx\nproperty float ny\nproperty float nz\n");
        out.push_str("property uchar red\nproperty uchar green\nproperty uchar blue\n");
        writeln!(&mut out, "element face {}", tris.len()).unwrap();
        out.push_str("property list uchar uint vertex_indices\n");
        out.push_str("end_header\n");
        for v in &mesh.vertices {
            let q = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            writeln!(
                &mut out,
                "{:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {} {} {}",
                v.position.x,
                v.position.y,
                v.position.z,
                v.normal.x,
                v.normal.y,
                v.normal.z,
                q(v.color[0]),
                q(v.color[1]),
                q(v.color[2]),
            )
            .unwrap();
        }
        for [i, j, k] in tris {
            writeln!(&mut out, "3 {i} {j} {k}").unwrap();
        }
        out
    }

    /// LuxCore `.scn` scene file referencing the mesh `.ply` — faithful to C++
    /// `LuxCoreDisplayContext`: an orthographic camera, constant + distant
    /// lights, a matte material, and the shape→ply / object→material+shape
    /// bindings. `name` is the LuxCore identifier, `ply_filename` the relative
    /// `.ply` reference.
    pub fn write_scn_string(&self, name: &str, ply_filename: &str) -> String {
        let mut out = String::new();
        out.push_str("scene.camera.up = 0 1 0\n");
        out.push_str("scene.camera.lookat.target = 0 0 0\n");
        out.push_str("scene.camera.type = \"orthographic\"\n");
        out.push_str("scene.camera.lookat.orig = 0 0 100\n");
        out.push_str("# Lights\n");
        out.push_str("scene.lights.il.type = constantinfinite\n");
        out.push_str("scene.lights.il.color = 1 1 1\n");
        out.push_str("scene.lights.il.gain = 0.15 0.15 0.15\n");
        out.push_str("scene.lights.distl.type = distant\n");
        out.push_str("scene.lights.distl.color = 1 1 1\n");
        out.push_str("scene.lights.distl.gain = 20 20 20\n");
        out.push_str("scene.lights.distl.direction = 1 1 -1\n");
        writeln!(&mut out, "scene.materials.{name}.type = matte").unwrap();
        writeln!(&mut out, "scene.materials.{name}.kd = 0.7 0.7 0.7").unwrap();
        writeln!(&mut out, "scene.shapes.{name}.type = mesh").unwrap();
        writeln!(&mut out, "scene.shapes.{name}.ply = {ply_filename}").unwrap();
        writeln!(&mut out, "scene.objects.{name}.material = {name}").unwrap();
        writeln!(&mut out, "scene.objects.{name}.shape = {name}").unwrap();
        out
    }

    /// LuxCore `.cfg` render config referencing the `.scn` (film size, linear
    /// tonemap + gamma, RGBA output) — faithful to C++ `LuxCoreDisplayContext`
    /// header. `scene_base` is the relative basename (no extension).
    pub fn write_cfg_string(&self, scene_base: &str, width: i32, height: i32) -> String {
        let mut out = String::new();
        out.push_str("# CueMol LuxCore exporter output\n");
        writeln!(&mut out, "scene.file = {scene_base}.scn").unwrap();
        writeln!(&mut out, "film.width = {width}").unwrap();
        writeln!(&mut out, "film.height = {height}").unwrap();
        out.push_str("film.imagepipeline.0.type = TONEMAP_LINEAR\n");
        out.push_str("film.imagepipeline.1.type = GAMMA_CORRECTION\n");
        out.push_str("film.imagepipeline.1.value = 2.2\n");
        out.push_str("film.outputs.1.type = RGBA_TONEMAPPED\n");
        writeln!(&mut out, "film.outputs.1.filename = {scene_base}.png").unwrap();
        out
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

    /// Write the LuxCore CFG plus sibling SCN and PLY files.
    pub fn write_scene_files(
        &mut self,
        name: &str,
        mesh: &RenderMesh,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if self.handler().path().is_empty() {
            return Err(CueMolError::invalid_argument(
                "LuxCore output path is empty",
            ));
        }
        let cfg_path = std::path::Path::new(self.handler().path());
        let scn_path = cfg_path.with_extension("scn");
        let ply_path = cfg_path.with_extension("ply");
        let scene_base = cfg_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("scene");
        let ply_filename = ply_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("scene.ply");
        std::fs::write(&ply_path, self.write_ply_string(mesh))?;
        std::fs::write(&scn_path, self.write_scn_string(name, ply_filename))?;
        self.handler()
            .write_string(&self.write_cfg_string(scene_base, width, height))?;
        self.state.write();
        Ok(())
    }
}

impl Default for LuxCoreSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! impl_export_state_deref {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl std::ops::Deref for $ty {
                type Target = ExportState;

                fn deref(&self) -> &Self::Target {
                    &self.state
                }
            }

            impl std::ops::DerefMut for $ty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.state
                }
            }
        )+
    };
}

impl_export_state_deref!(
    ImgSceneExporter,
    PovSceneExporter,
    MqoSceneExporter,
    WbpSceneExporter,
    StlSceneExporter,
    LuxRendSceneExporter,
    LuxCoreSceneExporter,
);

/// Build a per-face material table from a mesh: the face colour is the mean of
/// its three vertex colours (C++ `mixColor`), quantised to 8-bit RGBA and
/// deduplicated in first-seen order. Returns `(unique_colours, face -> index)`.
/// Shared by the MQO and WBP exporters. An empty mesh yields one grey default.
fn face_material_table(mesh: &RenderMesh) -> (Vec<[u8; 4]>, Vec<usize>) {
    let quant = |c: [f32; 4]| {
        [
            (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    };
    let tris = mesh.triangles();
    let mut keys: Vec<[u8; 4]> = Vec::new();
    let mut face_mat: Vec<usize> = Vec::with_capacity(tris.len());
    for [i, j, k] in &tris {
        let mut mix = [0.0_f32; 4];
        for idx in [*i, *j, *k] {
            let c = mesh.vertices[idx].color;
            for (acc, ch) in mix.iter_mut().zip(c) {
                *acc += ch / 3.0;
            }
        }
        let key = quant(mix);
        let mi = keys.iter().position(|k2| *k2 == key).unwrap_or_else(|| {
            keys.push(key);
            keys.len() - 1
        });
        face_mat.push(mi);
    }
    if keys.is_empty() {
        keys.push([204, 204, 204, 255]);
    }
    (keys, face_mat)
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

/// A ray for hit-testing / picking: points `origin + t * dir`, `dir` unit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vector4,
    pub dir: Vector4,
}

impl Ray {
    /// Build a ray, normalizing `dir` (a zero `dir` degenerates to +z).
    pub fn new(origin: Vector4, dir: Vector4) -> Self {
        Self {
            origin,
            dir: normalize3(dir),
        }
    }
}

/// Nearest-atom ray pick — the headless core of C++ `gfx::Hittest` (ray–sphere
/// intersection). Returns the index into `snapshot.atoms` of the atom whose vdW
/// sphere the ray first enters (smallest positive `t`), or `None` if the ray
/// hits no atom. Pure geometry: a screen click becomes a ray (via the camera),
/// then this resolves which atom is under it. The analytic ray–sphere test is
/// its own oracle (no GUI click needed to verify).
pub fn pick_atom_by_ray(snapshot: &RenderSceneSnapshot, ray: &Ray) -> Option<usize> {
    let d = ray.dir;
    let mut best: Option<(f64, usize)> = None;
    for (i, atom) in snapshot.atoms.iter().enumerate() {
        if atom.hit.is_some() {
            continue;
        }
        let ox = ray.origin.x - atom.center.x;
        let oy = ray.origin.y - atom.center.y;
        let oz = ray.origin.z - atom.center.z;
        let b = ox * d.x + oy * d.y + oz * d.z;
        let r = atom.radius as f64;
        let cc = ox * ox + oy * oy + oz * oz - r * r;
        let disc = b * b - cc;
        if disc < 0.0 {
            continue; // ray misses the sphere
        }
        let sq = disc.sqrt();
        let mut t = -b - sq; // near intersection
        if t < 0.0 {
            t = -b + sq; // origin inside the sphere: use the far root
        }
        if t < 0.0 {
            continue; // sphere entirely behind the ray
        }
        if best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, i));
        }
    }
    best.map(|(_, i)| i)
}

/// A silhouette/crease edge line segment (two world-space endpoints).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSegment {
    pub a: Vector4,
    pub b: Vector4,
}

/// Extract silhouette + crease + boundary edge lines from a mesh — a faithful
/// port of C++ `RendIntData::calcSilEdgeLines` (the NPR edge-line / `GraphEdge`
/// feature behind POV-Ray's `show_edge_lines`). An edge is emitted when it is:
/// a BOUNDARY edge (only one adjacent face), a CREASE (`acos(n1·n2) >
/// crease_limit`, only when `crease_limit > 0`), or a SILHOUETTE (the view
/// vectors `endpoint - cam` dotted with the two adjacent face normals have
/// opposite signs at either endpoint — one face front-facing, one back).
///
/// Adjacency is keyed by quantised vertex POSITION (not index) so the unwelded
/// tessellated mesh — whose sphere seams/poles duplicate vertices — still forms
/// the closed manifold the algorithm needs (the equivalent of C++ welding the
/// mesh in `buildMeshes`).
pub fn extract_edge_lines(mesh: &RenderMesh, cam: Vector4, crease_limit: f64) -> Vec<EdgeSegment> {
    use std::collections::HashMap;

    type Key = (i64, i64, i64);
    let key = |p: Vector4| -> Key {
        const Q: f64 = 1.0e4;
        (
            (p.x * Q).round() as i64,
            (p.y * Q).round() as i64,
            (p.z * Q).round() as i64,
        )
    };
    let dot3 = |a: Vector4, b: Vector4| a.x * b.x + a.y * b.y + a.z * b.z;

    struct EdgeAcc {
        a: Vector4,
        b: Vector4,
        n: [Option<Vector4>; 2],
    }
    let mut edges: HashMap<(Key, Key), EdgeAcc> = HashMap::new();
    for [i, j, k] in mesh.triangles() {
        let vi = mesh.vertices[i].position;
        let vj = mesh.vertices[j].position;
        let vk = mesh.vertices[k].position;
        let normal = normalize3((vj - vi).cross(vk - vi));
        for (pa, pb) in [(vi, vj), (vj, vk), (vk, vi)] {
            let (ka, kb) = (key(pa), key(pb));
            let (k1, k2, a, b) = if ka <= kb {
                (ka, kb, pa, pb)
            } else {
                (kb, ka, pb, pa)
            };
            let acc = edges.entry((k1, k2)).or_insert(EdgeAcc {
                a,
                b,
                n: [None, None],
            });
            if acc.n[0].is_none() {
                acc.n[0] = Some(normal);
            } else if acc.n[1].is_none() {
                acc.n[1] = Some(normal);
            }
        }
    }

    let mut out = Vec::new();
    for acc in edges.values() {
        let emit = match (acc.n[0], acc.n[1]) {
            (Some(_), None) => true, // boundary / ridge edge
            (Some(n1), Some(n2)) => {
                let crease =
                    crease_limit > 0.0 && dot3(n1, n2).clamp(-1.0, 1.0).acos() > crease_limit;
                let va = acc.a - cam;
                let vb = acc.b - cam;
                let silhouette =
                    dot3(va, n1) * dot3(va, n2) < 0.0 || dot3(vb, n1) * dot3(vb, n2) < 0.0;
                crease || silhouette
            }
            _ => false,
        };
        if emit {
            out.push(EdgeSegment { a: acc.a, b: acc.b });
        }
    }
    out
}

/// Resolve a render colour to linear RGBA (default mid-grey), matching the
/// offscreen renderer's atom/bond flat shading.
fn snapshot_color_rgba(color: &Option<ColorSpec>) -> [f32; 4] {
    match color.as_ref().and_then(ColorSpec::rgba8) {
        Some(c) => [
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            c.a as f32 / 255.0,
        ],
        None => [0.7, 0.7, 0.7, 1.0],
    }
}

fn normalize3(v: Vector4) -> Vector4 {
    let len = v.length3d();
    if len < 1.0e-9 {
        Vector4::new3(0.0, 0.0, 1.0)
    } else {
        v.scale(1.0 / len)
    }
}

/// Default UV-sphere tessellation (rings, segments) — the historical 8×12.
const DEFAULT_SPHERE_RINGS: usize = 8;
const DEFAULT_SPHERE_SEGS: usize = 12;

/// Map a C++ CPK/BallStick `detail` level (≈3 coarse .. 10 fine) to UV-sphere
/// (rings, segments). detail 4 reproduces the historical 8×12 default.
fn sphere_detail_to_rings_segs(detail: u32) -> (usize, usize) {
    let detail = detail.max(2) as usize;
    (detail * 2, detail * 3)
}

/// Tessellate a UV sphere (flat-shaded, radial normals) at the default detail.
fn add_sphere(mesh: &mut RenderMesh, center: Vector4, radius: f64, color: [f32; 4]) {
    add_sphere_detailed(
        mesh,
        center,
        radius,
        color,
        DEFAULT_SPHERE_RINGS,
        DEFAULT_SPHERE_SEGS,
    );
}

/// Tessellate a UV sphere at an explicit (rings, segs) resolution. Poles emit
/// degenerate triangles, kept so the triangle count is a clean `rings*segs*2`.
fn add_sphere_detailed(
    mesh: &mut RenderMesh,
    center: Vector4,
    radius: f64,
    color: [f32; 4],
    rings: usize,
    segs: usize,
) {
    let base = mesh.vertices.len() as u32;
    for i in 0..=rings {
        let phi = std::f64::consts::PI * i as f64 / rings as f64;
        let (sp, cp) = phi.sin_cos();
        for j in 0..=segs {
            let theta = std::f64::consts::TAU * j as f64 / segs as f64;
            let (st, ct) = theta.sin_cos();
            let dir = Vector4::new3(sp * ct, cp, sp * st);
            mesh.vertices.push(MeshVertex {
                position: Vector4::new3(
                    center.x + radius * dir.x,
                    center.y + radius * dir.y,
                    center.z + radius * dir.z,
                ),
                normal: dir,
                color,
            });
        }
    }
    let stride = segs + 1;
    for i in 0..rings {
        for j in 0..segs {
            let a = base + (i * stride + j) as u32;
            let b = base + (i * stride + j + 1) as u32;
            let c = base + ((i + 1) * stride + j) as u32;
            let d = base + ((i + 1) * stride + j + 1) as u32;
            mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
}

/// Tessellate a cylinder between `from` and `to` (flat-shaded) into `mesh`. A
/// zero-length cylinder contributes nothing.
fn add_cylinder(mesh: &mut RenderMesh, from: Vector4, to: Vector4, radius: f64, color: [f32; 4]) {
    add_cylinder_detailed(mesh, from, to, radius, color, 10);
}

fn add_cylinder_detailed(
    mesh: &mut RenderMesh,
    from: Vector4,
    to: Vector4,
    radius: f64,
    color: [f32; 4],
    segments: usize,
) {
    let axis = to - from;
    let len = axis.length3d();
    if len < 1.0e-9 {
        return;
    }
    let tangent = axis.scale(1.0 / len);
    let up = if tangent.x.abs() < 0.9 {
        Vector4::new3(1.0, 0.0, 0.0)
    } else {
        Vector4::new3(0.0, 1.0, 0.0)
    };
    let n1 = normalize3(tangent.cross(up));
    let n2 = normalize3(tangent.cross(n1));
    let segments = segments.max(3);
    let base = mesh.vertices.len() as u32;
    for end in [from, to] {
        for j in 0..=segments {
            let theta = std::f64::consts::TAU * j as f64 / segments as f64;
            let (s, c) = theta.sin_cos();
            let offset = n1.scale(c * radius) + n2.scale(s * radius);
            mesh.vertices.push(MeshVertex {
                position: end + offset,
                normal: normalize3(offset),
                color,
            });
        }
    }
    let stride = (segments + 1) as u32;
    for j in 0..segments as u32 {
        let a = base + j;
        let b = base + j + 1;
        let c = base + stride + j;
        let d = base + stride + j + 1;
        mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
    }
}

fn mark_inverted_edge_geometry(
    mesh: &mut RenderMesh,
    vertex_start: usize,
    index_start: usize,
    mode: RenderEdgeMode,
) {
    let silhouette = mode == RenderEdgeMode::Silhouette;
    for vertex in &mut mesh.vertices[vertex_start..] {
        *vertex = (*vertex).with_edge_outline(silhouette);
    }
    for triangle in mesh.indices[index_start..].chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
}

fn add_sphere_edge(
    mesh: &mut RenderMesh,
    center: Vector4,
    radius: f64,
    detail: Option<u32>,
    edge: RenderEdgeStyle,
) {
    let vertex_start = mesh.vertices.len();
    let index_start = mesh.indices.len();
    let edge_radius = radius + f64::from(edge.width.max(0.0));
    match detail {
        Some(detail) => {
            let (rings, segs) = sphere_detail_to_rings_segs(detail);
            add_sphere_detailed(mesh, center, edge_radius, edge.color, rings, segs);
        }
        None => add_sphere(mesh, center, edge_radius, edge.color),
    }
    mark_inverted_edge_geometry(mesh, vertex_start, index_start, edge.mode);
}

fn add_cylinder_edge(
    mesh: &mut RenderMesh,
    from: Vector4,
    to: Vector4,
    radius: f64,
    edge: RenderEdgeStyle,
) {
    let vertex_start = mesh.vertices.len();
    let index_start = mesh.indices.len();
    add_cylinder(
        mesh,
        from,
        to,
        radius + f64::from(edge.width.max(0.0)),
        edge.color,
    );
    mark_inverted_edge_geometry(mesh, vertex_start, index_start, edge.mode);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomIntrRenderMode {
    Simple,
    Fancy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomIntrCapType {
    Flat,
    Sphere,
    Arrow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrRenderOptions {
    pub mode: AtomIntrRenderMode,
    pub detail: u32,
    pub stipples: [f64; 6],
    pub width: f32,
    pub color: ColorSpec,
    pub start_cap: AtomIntrCapType,
    pub end_cap: AtomIntrCapType,
    pub arrow_height: f64,
    pub arrow_width: f64,
}

impl Default for AtomIntrRenderOptions {
    fn default() -> Self {
        Self {
            mode: AtomIntrRenderMode::Fancy,
            detail: 5,
            stipples: [1.0, 1.0, -1.0, -1.0, -1.0, -1.0],
            width: 0.1,
            color: ColorSpec {
                base: cuemol_gfx::ColorBase::Rgb {
                    r: 1.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
                modifiers: Vec::new(),
            },
            start_cap: AtomIntrCapType::Sphere,
            end_cap: AtomIntrCapType::Sphere,
            arrow_height: 1.0,
            arrow_width: 2.0,
        }
    }
}

fn add_cone_detailed(
    mesh: &mut RenderMesh,
    base_center: Vector4,
    tip: Vector4,
    radius: f64,
    color: [f32; 4],
    segments: usize,
) {
    let axis = tip - base_center;
    let length = axis.length3d();
    if length < 1.0e-9 || radius <= 0.0 {
        return;
    }
    let tangent = axis.scale(1.0 / length);
    let up = if tangent.x.abs() < 0.9 {
        Vector4::new3(1.0, 0.0, 0.0)
    } else {
        Vector4::new3(0.0, 1.0, 0.0)
    };
    let side1 = normalize3(tangent.cross(up));
    let side2 = normalize3(tangent.cross(side1));
    let segments = segments.max(6);

    let side_base = mesh.vertices.len() as u32;
    for index in 0..=segments {
        let angle = std::f64::consts::TAU * index as f64 / segments as f64;
        let (sin, cos) = angle.sin_cos();
        let radial = side1.scale(cos) + side2.scale(sin);
        let normal = normalize3(radial + tangent.scale(radius / length));
        mesh.vertices.push(MeshVertex {
            position: base_center + radial.scale(radius),
            normal,
            color,
        });
        mesh.vertices.push(MeshVertex {
            position: tip,
            normal,
            color,
        });
    }
    for index in 0..segments as u32 {
        let a = side_base + index * 2;
        mesh.indices.extend_from_slice(&[a, a + 2, a + 1]);
    }

    let cap_center = mesh.vertices.len() as u32;
    mesh.vertices.push(MeshVertex {
        position: base_center,
        normal: tangent.scale(-1.0),
        color,
    });
    for index in 0..=segments {
        let angle = std::f64::consts::TAU * index as f64 / segments as f64;
        let (sin, cos) = angle.sin_cos();
        mesh.vertices.push(MeshVertex {
            position: base_center + (side1.scale(cos) + side2.scale(sin)).scale(radius),
            normal: tangent.scale(-1.0),
            color,
        });
    }
    for index in 0..segments as u32 {
        mesh.indices.extend_from_slice(&[
            cap_center,
            cap_center + index + 2,
            cap_center + index + 1,
        ]);
    }
}

fn add_atom_intr_sphere(
    mesh: &mut RenderMesh,
    center: Vector4,
    options: &AtomIntrRenderOptions,
    color: [f32; 4],
) {
    let (rings, segments) = sphere_detail_to_rings_segs(options.detail);
    add_sphere_detailed(
        mesh,
        center,
        f64::from(options.width),
        color,
        rings,
        segments,
    );
}

fn add_atom_intr_arrow(
    mesh: &mut RenderMesh,
    endpoint: Vector4,
    inward: Vector4,
    options: &AtomIntrRenderOptions,
    color: [f32; 4],
) {
    let (_, segments) = sphere_detail_to_rings_segs(options.detail);
    add_cone_detailed(
        mesh,
        endpoint + inward.scale(options.arrow_height),
        endpoint,
        f64::from(options.width) * options.arrow_width,
        color,
        segments,
    );
}

fn add_atom_intr_cylinder(
    mesh: &mut RenderMesh,
    from: Vector4,
    to: Vector4,
    options: &AtomIntrRenderOptions,
    color: [f32; 4],
) {
    let (_, segments) = sphere_detail_to_rings_segs(options.detail);
    add_cylinder_detailed(mesh, from, to, f64::from(options.width), color, segments);
}

fn add_fancy_atom_intr_segment(
    mesh: &mut RenderMesh,
    from: Vector4,
    to: Vector4,
    options: &AtomIntrRenderOptions,
    color: [f32; 4],
) {
    let delta = to - from;
    let length = delta.length3d();
    if length < 0.001 || options.width <= 0.0 {
        return;
    }
    let direction = delta.scale(1.0 / length);
    let mut start = from;
    let mut end = to;
    if options.start_cap == AtomIntrCapType::Arrow {
        start = start + direction.scale(options.arrow_height);
        add_atom_intr_arrow(mesh, from, direction, options, color);
    }
    if options.end_cap == AtomIntrCapType::Arrow {
        end = end - direction.scale(options.arrow_height);
        add_atom_intr_arrow(mesh, to, direction.scale(-1.0), options, color);
    }
    let body_length = (end - start).length3d();
    if body_length < 0.001 {
        return;
    }
    let stipple_count = options
        .stipples
        .iter()
        .position(|value| *value <= 0.0)
        .unwrap_or(options.stipples.len());
    if stipple_count == 0 {
        add_atom_intr_cylinder(mesh, start, end, options, color);
        if options.start_cap == AtomIntrCapType::Sphere {
            add_atom_intr_sphere(mesh, start, options, color);
        }
        if options.end_cap == AtomIntrCapType::Sphere {
            add_atom_intr_sphere(mesh, end, options, color);
        }
        return;
    }

    let round_stipple = matches!(
        (options.start_cap, options.end_cap),
        (AtomIntrCapType::Sphere, _) | (_, AtomIntrCapType::Sphere)
    );
    let mut current = 0.0;
    let mut draw = false;
    let mut previous = start;
    let mut index = 0;
    while current < body_length {
        let position = start + direction.scale(current);
        if draw {
            add_atom_intr_cylinder(mesh, position, previous, options, color);
        }
        if round_stipple {
            add_atom_intr_sphere(mesh, position, options, color);
        }
        current += options.stipples[index % stipple_count];
        draw = !draw;
        previous = position;
        index += 1;
    }
    if draw {
        add_atom_intr_cylinder(mesh, previous, end, options, color);
        if round_stipple {
            add_atom_intr_sphere(mesh, end, options, color);
        }
    }
}

fn append_simple_atom_intr_bond(
    bonds: &mut Vec<RenderBond>,
    from: Vector4,
    to: Vector4,
    object_uid: Option<u64>,
    options: &AtomIntrRenderOptions,
) {
    if options.width <= 0.0 {
        return;
    }
    let radius = (options.width * 0.01).max(0.0025);
    bonds.push(
        RenderBond::new(from, to, radius, Some(options.color.clone()))
            .with_picking(object_uid, None, false)
            .with_screen_style(options.width, options.stipples[0] >= 0.0),
    );
}

/// Build C++ `AtomIntrRenderer` measurement geometry. Fancy mode emits detailed
/// dashed tubes with round/flat/arrow caps; simple mode emits screen-space lines
/// with the reference pixel width and on/off stipple switch.
pub fn atom_intr_snapshot(
    segments: &[(Vector4, Vector4)],
    object_uid: Option<u64>,
) -> RenderSceneSnapshot {
    atom_intr_snapshot_with_options(segments, object_uid, &AtomIntrRenderOptions::default())
}

pub fn atom_intr_snapshot_with_options(
    segments: &[(Vector4, Vector4)],
    object_uid: Option<u64>,
    options: &AtomIntrRenderOptions,
) -> RenderSceneSnapshot {
    let mut bounds: Option<(Vector4, Vector4)> = None;
    let mut extend = |p: Vector4| match &mut bounds {
        None => bounds = Some((p, p)),
        Some((min, max)) => {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }
    };
    let color = snapshot_color_rgba(&Some(options.color.clone()));
    let mut bonds = Vec::new();
    let mut mesh = RenderMesh::new();
    for &(from, to) in segments {
        extend(from);
        extend(to);
        match options.mode {
            AtomIntrRenderMode::Simple => {
                append_simple_atom_intr_bond(&mut bonds, from, to, object_uid, options)
            }
            AtomIntrRenderMode::Fancy => {
                add_fancy_atom_intr_segment(&mut mesh, from, to, options, color)
            }
        }
    }
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds,
        meshes: (!mesh.indices.is_empty())
            .then_some(mesh)
            .into_iter()
            .collect(),
        transparency_groups: Vec::new(),
        bounds,
    }
}

pub fn tessellate_snapshot(snapshot: &RenderSceneSnapshot) -> RenderMesh {
    let mut mesh = RenderMesh::new();
    for atom in snapshot.atoms.iter().filter(|atom| atom.hit.is_none()) {
        if let Some(edge) = atom.edge.filter(|edge| edge.width > 0.0) {
            add_sphere_edge(
                &mut mesh,
                atom.center,
                f64::from(atom.radius),
                atom.detail,
                edge,
            );
        }
        // Honour a per-atom `detail` (a styled renderer's sphere tessellation
        // level) so the GUI shows it; atoms without it use the default sphere.
        match atom.detail {
            Some(detail) => {
                let (rings, segs) = sphere_detail_to_rings_segs(detail);
                add_sphere_detailed(
                    &mut mesh,
                    atom.center,
                    atom.radius as f64,
                    snapshot_color_rgba(&atom.color),
                    rings,
                    segs,
                );
            }
            None => add_sphere(
                &mut mesh,
                atom.center,
                atom.radius as f64,
                snapshot_color_rgba(&atom.color),
            ),
        }
    }
    for bond in &snapshot.bonds {
        if let Some(edge) = bond
            .edge
            .filter(|edge| edge.width > 0.0 && bond.screen_width.is_none())
        {
            if bond.screen_point {
                add_sphere_edge(&mut mesh, bond.from, f64::from(bond.radius), None, edge);
            } else {
                add_cylinder_edge(&mut mesh, bond.from, bond.to, f64::from(bond.radius), edge);
            }
        }
        if bond.screen_point {
            add_sphere(
                &mut mesh,
                bond.from,
                bond.radius as f64,
                snapshot_color_rgba(&bond.color),
            );
        } else {
            add_cylinder(
                &mut mesh,
                bond.from,
                bond.to,
                bond.radius as f64,
                snapshot_color_rgba(&bond.color),
            );
        }
    }
    for extra in &snapshot.meshes {
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(extra.vertices.iter().cloned());
        mesh.indices.extend(extra.indices.iter().map(|i| i + base));
    }
    mesh
}

/// Tessellate a snapshot with an explicit sphere `detail` level (C++
/// CPK/BallStick `detail`, ≈3 coarse .. 10 fine; 4 == the default 8×12). Higher
/// detail yields rounder atom spheres (more triangles); bonds and pre-built
/// meshes are unaffected.
pub fn tessellate_snapshot_with_detail(snapshot: &RenderSceneSnapshot, detail: u32) -> RenderMesh {
    let (rings, segs) = sphere_detail_to_rings_segs(detail);
    let mut mesh = RenderMesh::new();
    for atom in snapshot.atoms.iter().filter(|atom| atom.hit.is_none()) {
        if let Some(edge) = atom.edge.filter(|edge| edge.width > 0.0) {
            add_sphere_edge(
                &mut mesh,
                atom.center,
                f64::from(atom.radius),
                Some(detail),
                edge,
            );
        }
        add_sphere_detailed(
            &mut mesh,
            atom.center,
            atom.radius as f64,
            snapshot_color_rgba(&atom.color),
            rings,
            segs,
        );
    }
    for bond in &snapshot.bonds {
        if let Some(edge) = bond
            .edge
            .filter(|edge| edge.width > 0.0 && bond.screen_width.is_none())
        {
            if bond.screen_point {
                add_sphere_edge(
                    &mut mesh,
                    bond.from,
                    f64::from(bond.radius),
                    Some(detail),
                    edge,
                );
            } else {
                add_cylinder_edge(&mut mesh, bond.from, bond.to, f64::from(bond.radius), edge);
            }
        }
        if bond.screen_point {
            add_sphere_detailed(
                &mut mesh,
                bond.from,
                f64::from(bond.radius),
                snapshot_color_rgba(&bond.color),
                rings,
                segs,
            );
        } else {
            add_cylinder(
                &mut mesh,
                bond.from,
                bond.to,
                f64::from(bond.radius),
                snapshot_color_rgba(&bond.color),
            );
        }
    }
    for extra in &snapshot.meshes {
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(extra.vertices.iter().cloned());
        mesh.indices.extend(extra.indices.iter().map(|i| i + base));
    }
    mesh
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
                detail: None,
                edge: None,
                hit: None,
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
    fn apply_xform_rotates_and_translates_geometry() {
        // Discriminating oracle (advisor-steered): a ROTATION + translation, not a
        // pure translation. A pure-translation xform would pass even with a
        // column-major<->row-major transpose bug; a 90deg-about-Z rotation does not.
        // Column-major (m1..m16) for R_z(+90) [ (x,y,z)->(-y,x,z) ] with translate
        // (0,0,5): cols [0,1,0,0][-1,0,0,0][0,0,1,0][0,0,5,1]. set_ai uses the C++
        // linear (column-major) index, so this is the exact LScrMatrix4D layout.
        let cm = [
            0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 5.0, 1.0,
        ];
        let mut m = Matrix4::IDENTITY;
        for (i, value) in cm.iter().enumerate() {
            m.set_ai(i + 1, *value).unwrap();
        }
        let mut mesh = RenderMesh::new();
        mesh.vertices.push(
            MeshVertex {
                position: Vector4::new3(1.0, 0.0, 0.0),
                normal: Vector4::new3(1.0, 0.0, 0.0),
                color: [1.0, 1.0, 1.0, 1.0],
            }
            .with_edge_outline(true),
        );
        let mut snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom {
                center: Vector4::new3(1.0, 0.0, 0.0),
                radius: 0.5,
                color: None,
                object_uid: None,
                atom_index: None,
                selected: false,
                detail: None,
                edge: None,
                hit: None,
            }],
            bonds: vec![RenderBond::new(
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 0.0, 0.0),
                0.1,
                None,
            )],
            meshes: vec![mesh],
            transparency_groups: Vec::new(),
            bounds: None,
        };
        snapshot.apply_xform(&m);

        let eps = 1.0e-9;
        // atom (1,0,0) -> R_z(90)=(0,1,0) then +translate(0,0,5) = (0,1,5).
        let c = snapshot.atoms[0].center;
        assert!(
            (c.x - 0.0).abs() < eps && (c.y - 1.0).abs() < eps && (c.z - 5.0).abs() < eps,
            "atom center should be (0,1,5), got ({},{},{})",
            c.x,
            c.y,
            c.z
        );
        // bond endpoint (1,0,0) likewise; the bond origin (0,0,0)-> (0,0,5).
        let from = snapshot.bonds[0].from;
        let to = snapshot.bonds[0].to;
        assert!((from.x).abs() < eps && (from.y - 1.0).abs() < eps && (from.z - 5.0).abs() < eps);
        assert!((to.x).abs() < eps && (to.y).abs() < eps && (to.z - 5.0).abs() < eps);
        // normal (1,0,0) -> rotation only (NO translation) = (0,1,0), still unit.
        let n = snapshot.meshes[0].vertices[0].normal;
        assert!(
            (n.x).abs() < eps && (n.y - 1.0).abs() < eps && (n.z).abs() < eps,
            "normal should rotate to (0,1,0) with no translation, got ({},{},{})",
            n.x,
            n.y,
            n.z
        );
        assert!(snapshot.meshes[0].vertices[0].is_silhouette_outline());
        // bounds were recomputed around the transformed geometry.
        let (lo, hi) = snapshot.bounds.expect("bounds recomputed");
        assert!(
            lo.z <= 5.0 && hi.z >= 5.0,
            "bounds should track the +5 z shift"
        );
    }

    #[test]
    fn image_and_png_exporter_defaults_match_qif() {
        let mut img = ImgSceneExporter::default();
        assert_eq!(img.handler().name(), "raw");
        assert_eq!(img.width(), 500);
        assert_eq!(img.height(), 500);
        assert_eq!(img.resolution_dpi(), 72.0);
        assert_eq!(img.resoln(), 72.0);
        img.set_resoln(96.0);
        assert_eq!(img.resolution_dpi(), 96.0);
        assert!(!img.use_alpha());
        assert!(!img.use_depth());
        img.set_alpha(true);
        img.set_depth(true);
        assert!(img.alpha());
        assert!(img.depth());
        assert_eq!(img.aa_opt(), 0);
        img.write();
        assert_eq!(img.write_count(), 1);

        let png = PngSceneExporter::new();
        assert_eq!(png.handler().name(), "png");
        assert_eq!(png.image().width(), 500);
        assert_eq!(png.width(), 500);
        assert!(!png.use_depth());
        assert!(!png.interlace());
        assert_eq!(png.compress(), 6);
    }

    #[test]
    fn generic_color_proof_transforms_all_snapshot_primitive_colors() {
        let blue = Some(ColorSpec {
            base: cuemol_gfx::ColorBase::Html {
                r: 0,
                g: 0,
                b: 255,
                a: 128,
            },
            modifiers: Vec::new(),
        });
        let mut snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(Vector4::ZERO, 1.0, blue.clone())],
            bonds: vec![RenderBond::new(
                Vector4::ZERO,
                Vector4::new3(1.0, 0.0, 0.0),
                0.1,
                blue,
            )],
            meshes: vec![RenderMesh {
                vertices: vec![MeshVertex {
                    position: Vector4::ZERO,
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.0, 0.0, 1.0, 0.5],
                }],
                indices: Vec::new(),
            }],
            transparency_groups: Vec::new(),
            bounds: None,
        };
        apply_generic_color_proof(&mut snapshot);
        for color in [
            snapshot.atoms[0].color.as_ref().unwrap(),
            snapshot.bonds[0].color.as_ref().unwrap(),
        ] {
            let rgba = color.rgba8().unwrap();
            assert_eq!((rgba.r, rgba.g, rgba.b, rgba.a), (0x34, 0x5b, 0xa0, 128));
        }
        let color = snapshot.meshes[0].vertices[0].color;
        assert!((color[0] - 0x34 as f32 / 255.0).abs() < 1.0e-6);
        assert!((color[1] - 0x5b as f32 / 255.0).abs() < 1.0e-6);
        assert!((color[2] - 0xa0 as f32 / 255.0).abs() < 1.0e-6);
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

        let mut wbp = WbpSceneExporter::new();
        assert_eq!(wbp.handler().name(), "wbp");
        assert_eq!(wbp.grad_steps(), 16);
        assert_eq!(wbp.camera(), "");
        wbp.set_camera_name("front");
        assert_eq!(wbp.get_camera_name(), "front");

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
    fn vector_exporters_write_configured_files() {
        let dir = std::env::temp_dir().join(format!(
            "cuemol-render-exporters-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(Vector4::ZERO, 1.0, None)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);

        let mut pov = PovSceneExporter::new();
        pov.handler_mut()
            .set_path(dir.join("scene.pov").to_string_lossy());
        pov.write_scene_file(&mesh, Vector4::ZERO, 4.0, false)
            .unwrap();
        assert_eq!(pov.write_count(), 1);
        assert!(std::fs::read_to_string(dir.join("scene.pov"))
            .unwrap()
            .contains("camera {"));

        let mut mqo = MqoSceneExporter::new();
        mqo.handler_mut()
            .set_path(dir.join("scene.mqo").to_string_lossy());
        mqo.write_mesh_file("scene", &mesh).unwrap();
        assert!(dir.join("scene.mqo").is_file());

        let mut stl = StlSceneExporter::new();
        stl.handler_mut()
            .set_path(dir.join("scene.stl").to_string_lossy());
        stl.write_mesh_file(&mesh).unwrap();
        assert!(std::fs::metadata(dir.join("scene.stl")).unwrap().len() > 84);

        let mut wbp = WbpSceneExporter::new();
        wbp.handler_mut()
            .set_path(dir.join("warabi.wbp").to_string_lossy());
        wbp.write_scene_files("scene", &mesh).unwrap();
        assert!(dir.join("warabi.wbp").is_file());
        assert!(dir.join("warabi.mqo").is_file());

        let mut lux = LuxRendSceneExporter::new();
        lux.handler_mut()
            .set_path(dir.join("scene.lxs").to_string_lossy());
        lux.write_scene_file(&mesh, Vector4::ZERO, 4.0, false)
            .unwrap();
        assert!(dir.join("scene.lxs").is_file());

        let mut core = LuxCoreSceneExporter::new();
        core.handler_mut()
            .set_path(dir.join("scene.cfg").to_string_lossy());
        core.write_scene_files("scene", &mesh, 640, 480).unwrap();
        assert!(dir.join("scene.cfg").is_file());
        assert!(dir.join("scene.scn").is_file());
        assert!(dir.join("scene.ply").is_file());

        std::fs::remove_dir_all(dir).unwrap();
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

        // LuxRender geometry fragment: one valid triangle (the [0 2 9] one is
        // dropped as out-of-range), points listed, indices present.
        let lux = LuxRendSceneExporter::new().write_mesh_string(&vertices, &triangles);
        assert!(lux.contains("Shape \"mesh\""));
        assert!(lux.contains("\"integer triindices\" [0 1 2]"));
        assert!(lux.contains("\"point P\" [0.000000 0.000000 0.000000"));
    }

    #[test]
    fn luxrender_scene_is_a_complete_renderable_document() {
        let c = Vector4::new3(2.0, 0.0, 0.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(c, 1.0, None)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let lxs = LuxRendSceneExporter::new().write_scene_string(&mesh, c, 4.0, false);
        // Scene chrome a fragment lacks: camera framing, film, world, light.
        assert!(lxs.contains("LookAt 2.000000 0.000000"));
        assert!(lxs.contains("Camera \"orthographic\""));
        assert!(lxs.contains("Film \"fleximage\""));
        assert!(lxs.contains("WorldBegin"));
        assert!(lxs.contains("LightSource \"distant\""));
        assert!(lxs.contains("Material \"matte\""));
        assert!(lxs.contains("Shape \"mesh\""));
        assert!(lxs.trim_end().ends_with("WorldEnd"));
    }

    #[test]
    fn tessellated_lone_atom_vertices_lie_on_the_sphere() {
        // Independent geometric oracle (not a writer/reader round-trip): every
        // tessellated vertex of a single sphere is exactly `radius` from the
        // centre, with a unit radial normal.
        let center = Vector4::new3(1.0, 2.0, 3.0);
        let radius = 1.5_f64;
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(center, radius as f32, None)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        // RINGS(8) * SEGS(12) * 2 triangles per sphere.
        assert_eq!(mesh.triangle_count(), 8 * 12 * 2);
        for v in &mesh.vertices {
            let dx = v.position.x - center.x;
            let dy = v.position.y - center.y;
            let dz = v.position.z - center.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!((dist - radius).abs() < 1.0e-6, "vertex off sphere: {dist}");
            let nlen = v.normal.length3d();
            assert!((nlen - 1.0).abs() < 1.0e-6, "normal not unit: {nlen}");
            // Radial: normal points from centre to vertex.
            assert!((v.normal.x * radius - dx).abs() < 1.0e-6);
        }
    }

    #[test]
    fn ray_picks_nearest_atom_sphere() {
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 1.0, None), // 0
                RenderAtom::new(Vector4::new3(5.0, 0.0, 0.0), 1.0, None), // 1
            ],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let down = Vector4::new3(0.0, 0.0, -1.0);
        // Ray straight down -z through atom 0 / atom 1.
        let hit0 = pick_atom_by_ray(&snapshot, &Ray::new(Vector4::new3(0.0, 0.0, 10.0), down));
        assert_eq!(hit0, Some(0));
        let hit1 = pick_atom_by_ray(&snapshot, &Ray::new(Vector4::new3(5.0, 0.0, 10.0), down));
        assert_eq!(hit1, Some(1));
        // Lateral offset (2) > radius (1): miss.
        let miss = pick_atom_by_ray(&snapshot, &Ray::new(Vector4::new3(2.0, 0.0, 10.0), down));
        assert_eq!(miss, None);
        // Tangent: offset exactly == radius (disc == 0) still hits.
        let tan = pick_atom_by_ray(&snapshot, &Ray::new(Vector4::new3(1.0, 0.0, 10.0), down));
        assert_eq!(tan, Some(0));

        // Two atoms along the ray: the nearer one (smaller t) wins.
        let stacked = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 1.0, None), // far
                RenderAtom::new(Vector4::new3(0.0, 0.0, 5.0), 1.0, None), // near (origin at z=10)
            ],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let near = pick_atom_by_ray(&stacked, &Ray::new(Vector4::new3(0.0, 0.0, 10.0), down));
        assert_eq!(near, Some(1));
        // Ray pointing away from both: no hit.
        let away = pick_atom_by_ray(
            &stacked,
            &Ray::new(Vector4::new3(0.0, 0.0, 10.0), Vector4::new3(0.0, 0.0, 1.0)),
        );
        assert_eq!(away, None);
    }

    #[test]
    fn hidden_hit_points_do_not_render_or_change_geometry_bounds() {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.atoms.push(
            RenderAtom::new(Vector4::ZERO, 100.0, None)
                .with_picking(Some(7), None, false)
                .with_hit(3, "feature"),
        );
        snapshot.recompute_bounds();
        assert_eq!(snapshot.atom_count(), 0);
        assert_eq!(snapshot.bounds, None);
        assert_eq!(tessellate_snapshot(&snapshot).triangle_count(), 0);
        assert_eq!(
            pick_atom_by_ray(
                &snapshot,
                &Ray::new(Vector4::new3(0.0, 0.0, 10.0), Vector4::new3(0.0, 0.0, -1.0),),
            ),
            None
        );
    }

    #[test]
    fn edge_lines_detect_boundary_crease_and_exclude_coplanar_diagonals() {
        use std::f64::consts::PI;
        // Flat quad (2 coplanar triangles): the 4 perimeter edges are BOUNDARY,
        // the shared diagonal is coplanar (no crease) and head-on (no
        // silhouette) -> excluded. Expect exactly 4 segments.
        let quad = RenderMesh {
            vertices: [
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ]
            .into_iter()
            .map(|(x, y, z)| MeshVertex {
                position: Vector4::new3(x, y, z),
                normal: Vector4::new3(0.0, 0.0, 1.0),
                color: [1.0; 4],
            })
            .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let cam = Vector4::new3(0.5, 0.5, 100.0);
        assert_eq!(extract_edge_lines(&quad, cam, PI / 4.0).len(), 4);

        // Cube: every one of the 12 cube edges is a 90° CREASE; the 6 face
        // diagonals are coplanar (excluded). Camera at the centre => view
        // vectors point outward through every outward normal => NO silhouette,
        // and the cube is closed => no boundary. So crease-only => exactly 12.
        let corners = [
            (-1.0, -1.0, -1.0),
            (1.0, -1.0, -1.0),
            (1.0, 1.0, -1.0),
            (-1.0, 1.0, -1.0),
            (-1.0, -1.0, 1.0),
            (1.0, -1.0, 1.0),
            (1.0, 1.0, 1.0),
            (-1.0, 1.0, 1.0),
        ];
        let quads = [
            [0, 1, 2, 3],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [1, 2, 6, 5],
            [0, 3, 7, 4],
        ];
        let mut verts = Vec::new();
        let mut indices = Vec::new();
        for q in quads {
            let base = verts.len() as u32;
            for &ci in &q {
                let (x, y, z) = corners[ci];
                verts.push(MeshVertex {
                    position: Vector4::new3(x, y, z),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [0.8; 4],
                });
            }
            // Two triangles per quad, consistently wound (shared diagonal stays
            // coplanar = not a crease).
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let cube = RenderMesh {
            vertices: verts,
            indices,
        };
        let center = Vector4::new3(0.0, 0.0, 0.0);
        let cube_edges = extract_edge_lines(&cube, center, PI / 4.0);
        assert_eq!(cube_edges.len(), 12, "a cube has 12 crease edges");
    }

    #[test]
    fn sphere_detail_controls_tessellation_density() {
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 1.0, None)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        // detail 4 reproduces the default 8x12 = 192 triangles.
        assert_eq!(
            tessellate_snapshot_with_detail(&snapshot, 4).triangle_count(),
            tessellate_snapshot(&snapshot).triangle_count()
        );
        // detail d -> rings 2d, segs 3d -> 12*d^2 triangles; finer = more.
        let coarse = tessellate_snapshot_with_detail(&snapshot, 3).triangle_count();
        let fine = tessellate_snapshot_with_detail(&snapshot, 8).triangle_count();
        assert_eq!(coarse, 12 * 3 * 3);
        assert_eq!(fine, 12 * 8 * 8);
        assert!(fine > coarse);
    }

    #[test]
    fn tessellate_snapshot_emits_sphere_per_atom_and_cylinder_per_bond() {
        let a = Vector4::new3(0.0, 0.0, 0.0);
        let b = Vector4::new3(2.0, 0.0, 0.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(a, 0.45, None),
                RenderAtom::new(b, 0.45, None),
            ],
            bonds: vec![RenderBond::new(a, b, 0.08, None)],
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        // 2 spheres (8*12*2 each) + 1 cylinder (SEGS(10)*2).
        assert_eq!(mesh.triangle_count(), 2 * 8 * 12 * 2 + 10 * 2);
        // A zero-length bond contributes no geometry.
        let degenerate = RenderSceneSnapshot {
            atoms: Vec::new(),
            bonds: vec![RenderBond::new(a, a, 0.08, None)],
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        assert_eq!(tessellate_snapshot(&degenerate).triangle_count(), 0);
    }

    #[test]
    fn tessellate_snapshot_adds_inverted_primitive_edge_hulls() {
        let edge = RenderEdgeStyle {
            mode: RenderEdgeMode::Silhouette,
            width: 0.2,
            color: [0.1, 0.2, 0.3, 1.0],
        };
        let from = Vector4::new3(0.0, 0.0, 0.0);
        let to = Vector4::new3(3.0, 0.0, 0.0);
        let mut snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(from, 1.0, None).with_edge(Some(edge))],
            bonds: vec![RenderBond::new(from, to, 0.25, None).with_edge(Some(edge))],
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        snapshot.recompute_bounds();

        let mesh = tessellate_snapshot(&snapshot);
        let sphere_vertices = (DEFAULT_SPHERE_RINGS + 1) * (DEFAULT_SPHERE_SEGS + 1);
        let cylinder_vertices = 2 * (10 + 1);
        assert_eq!(
            mesh.vertices
                .iter()
                .filter(|vertex| vertex.is_edge_outline())
                .count(),
            sphere_vertices + cylinder_vertices
        );
        assert!(mesh.vertices[..sphere_vertices]
            .iter()
            .all(MeshVertex::is_silhouette_outline));
        assert_eq!(mesh.vertices[0].color, edge.color);
        assert!(
            ((mesh.vertices[0].position - from).length3d() - 1.2).abs() < 1.0e-6,
            "the edge sphere must expand by eglinew"
        );
        let main_sphere = &mesh.vertices[sphere_vertices..sphere_vertices * 2];
        assert!(main_sphere.iter().all(|vertex| !vertex.is_edge_outline()));
        assert_eq!(mesh.indices[0..3], [0, 1, (DEFAULT_SPHERE_SEGS + 1) as u32]);
        let (min, max) = snapshot.bounds.unwrap();
        assert!((min.y + 1.2).abs() < 1.0e-6);
        assert!((max.x - 3.45).abs() < 1.0e-6);
    }

    #[test]
    fn mqo_export_has_metasequoia_structure_and_correct_counts() {
        // Two atoms of different colours + one bond -> a mesh with a few
        // distinct face colours. Verified by STRUCTURE inspection (not a
        // round-trip through an MQO reader).
        let red = Some(ColorSpec {
            base: ColorBase::Html {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            modifiers: Vec::new(),
        });
        let blue = Some(ColorSpec {
            base: ColorBase::Html {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            },
            modifiers: Vec::new(),
        });
        let a = Vector4::new3(0.0, 0.0, 0.0);
        let b = Vector4::new3(2.0, 0.0, 0.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(a, 0.45, red),
                RenderAtom::new(b, 0.45, blue),
            ],
            bonds: vec![RenderBond::new(a, b, 0.08, None)],
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let mqo = MqoSceneExporter::new().write_mqo_string("two", &mesh);

        assert!(mqo.starts_with("Metasequoia Document\nFormat Text Ver 1.0"));
        assert!(mqo.contains("Scene {"));
        assert!(mqo.trim_end().ends_with("Eof"));
        // vertex/face chunk counts match the mesh.
        assert!(mqo.contains(&format!("vertex {} {{", mesh.vertices.len())));
        assert!(mqo.contains(&format!("face {} {{", mesh.triangle_count())));
        // One material per distinct face colour: a red sphere, a blue sphere,
        // and a grey bond -> at least 3 materials, each referenced by an M(idx).
        let mat_count: usize = mqo
            .lines()
            .find_map(|l| {
                l.strip_prefix("Material ")
                    .and_then(|r| r.split(' ').next())
            })
            .and_then(|n| n.parse().ok())
            .expect("Material count line");
        assert!(mat_count >= 3, "expected >=3 face colours, got {mat_count}");
        assert!(mqo.contains("col(1.000000 0.000000 0.000000"));
        assert!(mqo.contains("col(0.000000 0.000000 1.000000"));
        // Face lines are 0-based V(...) M(...).
        let face_lines = mqo.lines().filter(|l| l.starts_with("3 V(")).count();
        assert_eq!(face_lines, mesh.triangle_count());
        assert!(mqo
            .lines()
            .all(|l| !l.starts_with("3 V(") || l.contains(") M(")));
    }

    #[test]
    fn pov_scene_is_a_complete_renderable_document() {
        let red = Some(ColorSpec {
            base: ColorBase::Html {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            modifiers: Vec::new(),
        });
        let c = Vector4::new3(5.0, 5.0, 5.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(c, 1.0, red)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let pov = PovSceneExporter::new().write_scene_string(&mesh, c, 4.0, false);

        // A complete scene has the chrome a fragment lacks: camera, lighting,
        // background, global_settings.
        assert!(pov.contains("camera {"));
        assert!(pov.contains("light_source {"));
        assert!(pov.contains("background {"));
        assert!(pov.contains("global_settings {"));
        assert!(pov.contains("direction <0.000000,0.000000,-1.000000>"));
        assert!(pov.contains("#declare _camera_pos = <5.000000,5.000000,17.000000>"));
        // The camera frames the molecule centre, not the origin.
        assert!(pov.contains("#declare _center = <5.000000,5.000000,5.000000>;"));
        // Coloured geometry: a texture_list carrying the red atom colour + mesh2.
        assert!(pov.contains("mesh2 {"));
        assert!(pov.contains("texture_list {"));
        assert!(pov.contains("pigment { color rgb <1.000000, 0.000000, 0.000000> }"));
        // Balanced braces (the macro/camera/mesh nesting closes cleanly).
        assert_eq!(pov.matches('{').count(), pov.matches('}').count());
    }

    #[test]
    fn pov_scene_uses_full_cuemol_camera_transform() {
        let mesh = RenderMesh::new();
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
        camera.set_cam_dist(10.0);
        camera.set_zoom(6.0);
        camera.set_perspective(true);
        camera.set_rotation(camera.rotation().rotate_y(90.0).unwrap());

        let pov = PovSceneExporter::new()
            .write_scene_string_with_camera(&mesh, &camera)
            .unwrap();
        assert!(pov.contains("#declare _distance = 10.000000;"));
        assert!(pov.contains("#declare _zoomy = 6.000000;"));
        assert!(pov.contains("#declare _perspective = 1;"));
        assert!(
            pov.contains("direction <-1.000000,0.000000,-0.000000>"),
            "{pov}"
        );
        assert!(pov.contains("right <0.000000,0.000000,-1.000000>"), "{pov}");
        assert!(
            pov.contains("#declare _camera_pos = <11.000000,2.000000,3.000000>"),
            "{pov}"
        );
    }

    #[test]
    fn pov_scene_applies_clip_alpha_edge_and_radiosity_options() {
        let mesh = RenderMesh {
            vertices: vec![
                MeshVertex {
                    position: Vector4::new3(-1.0, -1.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.0, 0.0, 0.5],
                },
                MeshVertex {
                    position: Vector4::new3(1.0, -1.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.0, 0.0, 0.5],
                },
                MeshVertex {
                    position: Vector4::new3(0.0, 1.0, 0.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.0, 0.0, 0.5],
                },
            ],
            indices: vec![0, 1, 2],
        };
        let mut camera = Camera::new();
        camera.set_slab_depth(2.0);
        let mut exporter = PovSceneExporter::new();
        exporter.set_use_clip_z(true);
        exporter.set_use_post_blend(true);
        exporter.set_show_edge_lines(false);

        let pov = exporter
            .write_scene_string_with_camera(&mesh, &camera)
            .unwrap();
        assert!(pov.contains("color rgbt <1.000000, 0.000000, 0.000000, 0.498039>"));
        assert!(pov.contains("clipped_by { plane { <0.000000,0.000000,-1.000000>, 1.000000 }"));
        assert!(pov.contains("#ifdef (_radiosity)"));
        assert!(pov.contains("Rad_Settings(_radiosity, off, off)"));
        assert!(pov.contains("SpecLighting(_light_spread, _distance, _light_inten*(1-_amb_frac)*(1-_flash_frac), _shadow)"));
        assert!(!pov.contains("edge lines"));
        assert_eq!(pov.matches('{').count(), pov.matches('}').count());
    }

    #[test]
    fn wbp_wraps_mqo_with_warabi_project_structure() {
        let red = Some(ColorSpec {
            base: ColorBase::Html {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            modifiers: Vec::new(),
        });
        let a = Vector4::new3(0.0, 0.0, 0.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(a, 0.45, red)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let wbp = WbpSceneExporter::new().write_wbp_string("mol", &mesh, "mol.mqo");

        assert!(wbp.starts_with("# Warabi Project 1.1 utf8"));
        // Material derived from the red sphere, referencing the MQO material id.
        assert!(wbp.contains("Material \"mol_mat0\" : {"));
        assert!(wbp.contains("Color : [1.000000, 0.000000, 0.000000]"));
        // External import points at the .mqo geometry file.
        assert!(wbp.contains("External : {"));
        assert!(wbp.contains("FileName : \"mol.mqo\""));
        assert!(wbp.contains("Object \"mol\" : {"));
        assert!(wbp.contains("DirectionalLight \"Light 1\" : {"));
        assert!(wbp.contains("Camera \"camera1\" : {"));
        // Balanced top-level braces (every '{' has a matching '}').
        let opens = wbp.matches('{').count();
        let closes = wbp.matches('}').count();
        assert_eq!(opens, closes, "unbalanced braces in WBP output");
    }

    #[test]
    fn luxcore_writes_ply_geometry_and_scn_cfg_references() {
        let a = Vector4::new3(0.0, 0.0, 0.0);
        let b = Vector4::new3(2.0, 0.0, 0.0);
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(a, 0.45, None)],
            bonds: vec![RenderBond::new(a, b, 0.08, None)],
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let lux = LuxCoreSceneExporter::new();

        let ply = lux.write_ply_string(&mesh);
        assert!(ply.starts_with("ply\nformat ascii 1.0"));
        assert!(ply.contains(&format!("element vertex {}", mesh.vertices.len())));
        assert!(ply.contains(&format!("element face {}", mesh.triangle_count())));
        assert!(ply.contains("end_header"));
        // One face line ("3 i j k") per triangle, one vertex data line each.
        assert_eq!(
            ply.lines().filter(|l| l.starts_with("3 ")).count(),
            mesh.triangle_count()
        );

        let scn = lux.write_scn_string("mol", "mol.ply");
        assert!(scn.contains("scene.shapes.mol.type = mesh"));
        assert!(scn.contains("scene.shapes.mol.ply = mol.ply"));
        assert!(scn.contains("scene.objects.mol.shape = mol"));
        assert!(scn.contains("scene.camera.type = \"orthographic\""));

        let cfg = lux.write_cfg_string("mol", 640, 480);
        assert!(cfg.contains("scene.file = mol.scn"));
        assert!(cfg.contains("film.width = 640"));
        assert!(cfg.contains("film.outputs.1.filename = mol.png"));
    }

    #[test]
    fn render_mesh_obj_string_has_one_v_and_f_line_per_vertex_and_triangle() {
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 1.0, None)],
            bonds: Vec::new(),
            meshes: Vec::new(),
            transparency_groups: Vec::new(),
            bounds: None,
        };
        let mesh = tessellate_snapshot(&snapshot);
        let obj = mesh.to_obj_string("lone");
        let v_lines = obj.lines().filter(|l| l.starts_with("v ")).count();
        let vn_lines = obj.lines().filter(|l| l.starts_with("vn ")).count();
        let f_lines = obj.lines().filter(|l| l.starts_with("f ")).count();
        assert_eq!(v_lines, mesh.vertices.len());
        assert_eq!(vn_lines, mesh.vertices.len());
        assert_eq!(f_lines, mesh.triangle_count());
        // Faces are 1-based and never reference index 0.
        assert!(!obj.contains("f 0"));
    }

    #[test]
    fn atom_intr_snapshot_honors_fancy_stipple_caps_detail_and_color() {
        let segments = [(Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(6.0, 0.0, 0.0))];
        let dashed = atom_intr_snapshot(&segments, Some(7));
        assert_eq!(dashed.bond_count(), 0);
        assert_eq!(dashed.mesh_count(), 1);

        let mut solid_options = AtomIntrRenderOptions::default();
        solid_options.stipples.fill(-1.0);
        solid_options.start_cap = AtomIntrCapType::Arrow;
        solid_options.end_cap = AtomIntrCapType::Flat;
        solid_options.width = 0.25;
        solid_options.arrow_height = 0.8;
        solid_options.arrow_width = 3.0;
        solid_options.detail = 8;
        solid_options.color = ColorSpec {
            base: cuemol_gfx::ColorBase::Rgb {
                r: 1.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            modifiers: Vec::new(),
        };
        let solid = atom_intr_snapshot_with_options(&segments, Some(7), &solid_options);
        assert_eq!(solid.bond_count(), 0);
        assert_eq!(solid.mesh_count(), 1);
        assert!(solid.meshes[0]
            .vertices
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 1.0, 1.0]));
        assert!(solid.meshes[0]
            .vertices
            .iter()
            .any(|vertex| (vertex.position - segments[0].0).length3d() < 1.0e-9));
        assert!(solid.meshes[0].triangle_count() > 40);
        assert!(dashed.meshes[0].triangle_count() > solid.meshes[0].triangle_count());

        let simple = atom_intr_snapshot_with_options(
            &segments,
            Some(7),
            &AtomIntrRenderOptions {
                mode: AtomIntrRenderMode::Simple,
                width: 2.0,
                ..AtomIntrRenderOptions::default()
            },
        );
        assert_eq!(simple.mesh_count(), 0);
        assert_eq!(simple.bond_count(), 1);
        assert!((simple.bonds[0].radius - 0.02).abs() < 1.0e-6);
        assert_eq!(simple.bonds[0].screen_width, Some(2.0));
        assert!(simple.bonds[0].screen_stipple);
    }
}
