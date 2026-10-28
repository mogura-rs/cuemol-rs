use cuemol_core::Vector4;
use cuemol_gfx::SolidColor;
use cuemol_molstr::{parse_selector, Atom, MolCoord, Selector, SolidColoring};
use cuemol_qsys::{
    EditInfo, InOutHandler, MultiGradient, ObjReader, ObjWriter, UndoManagerHandle, View,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

pub mod analytic_ses;
pub mod backends;
pub mod boundary;
mod direct_surface;
pub mod dist_field;
pub mod grid_surface;
pub mod hole_surface;
pub mod msms;
pub mod obj;
pub mod ply;
pub mod qdf;

pub use boundary::{Boundary, BoundaryElement, BoundarySet};
pub use dist_field::{DistFieldSurfBuilder, DistMapMarchingCubes};
pub use grid_surface::{
    build_grid_isosurface, build_grid_isosurface_masked, ScalarField, SurfSphere,
};
pub use hole_surface::HolePathPoint;
pub use msms::{read_msms_surface_files, read_msms_surface_str, sniff_msms_content, MsmsReader};
pub use obj::{read_obj_file, read_obj_str};
pub use ply::{read_ply_bytes, read_ply_file, read_ply_str};
pub use qdf::{QdfPotReader, QdfPotWriter, QdfSurfReader, QdfSurfWriter};

pub type MS2TestRenderer = Ms2TestRenderer;
pub type MSMSFileReader = MsmsFileReader;
pub type OpenDXPotReader = OpenDxPotReader;
pub type PLYFileReader = PlyFileReader;
pub type PQRFileWriter = PqrFileWriter;
pub type XYZRFileWriter = XyzrFileWriter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlyFileReader {
    reader: ObjReader,
}

impl PlyFileReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("ply", "PLY polygon file (*.ply)", "*.ply"),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> MolSurfObj {
        MolSurfObj::new()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> cuemol_molstr::ContentVerdict {
        let text = String::from_utf8_lossy(input);
        match text.lines().map(str::trim).find(|line| !line.is_empty()) {
            Some("ply") => cuemol_molstr::ContentVerdict::Yes,
            _ => cuemol_molstr::ContentVerdict::Unknown,
        }
    }

    pub fn read_file(&self, path: impl AsRef<std::path::Path>) -> cuemol_core::Result<SurfaceMesh> {
        read_ply_file(path)
    }

    pub fn read_bytes(&self, input: &[u8]) -> cuemol_core::Result<SurfaceMesh> {
        read_ply_bytes(input)
    }
}

impl Default for PlyFileReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsmsFileReader {
    reader: ObjReader,
    vertex_file: String,
}

impl MsmsFileReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("msms", "MSMS surface file (*.face/vert)", "*.face"),
            vertex_file: String::new(),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> MolSurfObj {
        MolSurfObj::new()
    }

    pub fn can_handle_content(&self, input: &str) -> cuemol_molstr::ContentVerdict {
        sniff_msms_content(input)
    }

    pub fn vertex_file(&self) -> &str {
        &self.vertex_file
    }

    pub fn set_vertex_file(&mut self, value: impl Into<String>) {
        self.vertex_file = value.into();
    }

    pub fn read_file(
        &self,
        face_path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<SurfaceMesh> {
        if self.vertex_file.is_empty() {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "MSMS vertex file name is not configured",
            ));
        }
        read_msms_surface_files(face_path, &self.vertex_file)
    }

    pub fn read_files(
        &self,
        face_path: impl AsRef<std::path::Path>,
        vertex_path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<SurfaceMesh> {
        read_msms_surface_files(face_path, vertex_path)
    }

    pub fn read_strings(
        &self,
        face_input: &str,
        vertex_input: &str,
    ) -> cuemol_core::Result<SurfaceMesh> {
        read_msms_surface_str(vertex_input, face_input)
    }
}

impl Default for MsmsFileReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenDxPotReader {
    reader: ObjReader,
    smooth: f64,
}

impl OpenDxPotReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("apbs", "APBS Potential Map(*.dx)", "*.dx"),
            smooth: -1.0,
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn smooth(&self) -> f64 {
        self.smooth
    }

    pub fn set_smooth(&mut self, value: f64) {
        self.smooth = value;
    }

    pub fn create_default_obj(&self) -> ElePotMap {
        ElePotMap::new()
    }

    pub fn can_handle_content(&self, input: &str) -> cuemol_molstr::ContentVerdict {
        if input
            .lines()
            .any(|line| line.starts_with("object 1 class gridpositions"))
        {
            cuemol_molstr::ContentVerdict::Yes
        } else {
            cuemol_molstr::ContentVerdict::Unknown
        }
    }

    pub fn read_str(&self, input: &str) -> cuemol_core::Result<ElePotMap> {
        let grid = cuemol_molstr::read_opendx_str(input)?;
        let spacing = axis_aligned_grid_spacing(grid.delta)?;
        let mut map = ElePotMap::new();
        map.set_map_float_array(&grid.values, grid.counts, spacing, grid.origin);
        if self.smooth > 0.0 {
            map.smooth2(self.smooth);
        }
        Ok(map)
    }

    pub fn read_file(&self, path: impl AsRef<std::path::Path>) -> cuemol_core::Result<ElePotMap> {
        let grid = cuemol_molstr::read_opendx_file(path)?;
        let spacing = axis_aligned_grid_spacing(grid.delta)?;
        let mut map = ElePotMap::new();
        map.set_map_float_array(&grid.values, grid.counts, spacing, grid.origin);
        if self.smooth > 0.0 {
            map.smooth2(self.smooth);
        }
        Ok(map)
    }
}

fn axis_aligned_grid_spacing(delta: [Vector4; 3]) -> cuemol_core::Result<[f64; 3]> {
    let mut spacing = [None; 3];
    for vector in delta {
        let components = [vector.x, vector.y, vector.z];
        let nonzero: Vec<_> = components
            .iter()
            .enumerate()
            .filter(|(_, value)| value.abs() > 1.0e-12)
            .collect();
        if nonzero.len() != 1 || !nonzero[0].1.is_finite() {
            return Err(cuemol_core::CueMolError::parse(
                "OpenDX grid delta vectors must be axis-aligned",
            ));
        }
        let (axis, value) = nonzero[0];
        if spacing[axis].replace(*value).is_some() {
            return Err(cuemol_core::CueMolError::parse(
                "OpenDX grid has duplicate delta axes",
            ));
        }
    }
    match spacing {
        [Some(x), Some(y), Some(z)] => Ok([x, y, z]),
        _ => Err(cuemol_core::CueMolError::parse(
            "OpenDX grid is missing a delta axis",
        )),
    }
}

impl Default for OpenDxPotReader {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! impl_obj_reader_deref {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl std::ops::Deref for $ty {
                type Target = ObjReader;

                fn deref(&self) -> &Self::Target {
                    &self.reader
                }
            }

            impl std::ops::DerefMut for $ty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.reader
                }
            }
        )+
    };
}

impl_obj_reader_deref!(PlyFileReader, MsmsFileReader, OpenDxPotReader);

#[derive(Debug, Clone, PartialEq)]
pub struct PqrFileWriter {
    writer: ObjWriter,
    selection: Option<Selector>,
    use_h: bool,
    namespace: String,
}

impl PqrFileWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("pqr", "PQR coordinates(*.pqr)", "*.pqr"),
            selection: None,
            use_h: false,
            namespace: String::new(),
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn selection(&self) -> Option<&Selector> {
        self.selection.as_ref()
    }

    pub fn get_selection(&self) -> Option<&Selector> {
        self.selection()
    }

    pub fn sel(&self) -> Option<&Selector> {
        self.selection()
    }

    pub fn set_selection(&mut self, value: Option<Selector>) {
        self.selection = value;
    }

    pub fn set_sel(&mut self, value: Option<Selector>) {
        self.set_selection(value);
    }

    pub fn use_h(&self) -> bool {
        self.use_h
    }

    pub fn is_use_h(&self) -> bool {
        self.use_h()
    }

    pub fn set_use_h(&mut self, value: bool) {
        self.use_h = value;
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn ns(&self) -> &str {
        self.namespace()
    }

    pub fn get_ns(&self) -> &str {
        self.namespace()
    }

    pub fn set_namespace(&mut self, value: impl Into<String>) {
        self.namespace = value.into();
    }

    pub fn set_ns(&mut self, value: impl Into<String>) {
        self.set_namespace(value);
    }

    pub fn write_pqr_string(&self, molecule: &MolCoord) -> String {
        let mut output = String::new();
        let mut output_index = 0;
        for atom in molecule.atoms() {
            // Drop hydrogens in the default united-atom mode + honour the selection
            // (C++ PQRFileWriter:104 / selected_surface_atoms).
            if !self.use_h && atom.element.trim().eq_ignore_ascii_case("H") {
                continue;
            }
            if atom.alt_loc.is_some_and(|alt| alt != 'A') {
                continue;
            }
            if let Some(selector) = self.selection.as_ref() {
                if !selector.matches_in_molecule(atom, molecule) {
                    continue;
                }
            }
            // C++ PQRFileWriter writes the force-field PARTIAL charge (getCharge) +
            // UNITED-ATOM vdW radius (getVdwRadius), not formal charge / element
            // radius. In the default united-atom mode the heavy atom ABSORBS its
            // bonded hydrogens' charges (the H are dropped); the explicit-H mode
            // writes each atom's own charge. A non-template atom (ligand/ion/water)
            // has no template charge -> 0.0, matching C++ (getCharge false -> 0).
            let charge =
                cuemol_molstr::toppar::charge_for_atom_mode(atom, self.use_h, &self.namespace)
                    .unwrap_or(0.0);
            // Radius matches the charge mode: vdwhr (united-atom) or vdwr
            // (explicit-H), C++ getVdwRadius(atom, m_bUseH).
            let radius = cuemol_molstr::toppar::vdw_radius_for_atom_mode(atom, self.use_h);
            output.push_str(&format!(
                "ATOM {output_index:>6} {atom_name:>4} {res_name:>3} {chain_id} {res_id:>4} \
                 {x:>7.3} {y:>7.3} {z:>7.3} {charge:>11.8} {radius:>7.3}\n",
                atom_name = atom.name,
                res_name = atom.res_name,
                chain_id = atom.chain_id,
                res_id = atom.res_id,
                x = atom.position.x,
                y = atom.position.y,
                z = atom.position.z,
            ));
            output_index += 1;
        }
        output
    }

    pub fn write_string(&self, molecule: &MolCoord) -> String {
        self.write_pqr_string(molecule)
    }
}

impl Default for PqrFileWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for PqrFileWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for PqrFileWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XyzrFileWriter {
    writer: ObjWriter,
    selection: Option<Selector>,
}

impl XyzrFileWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("xyzr", "XYZR coordinates(*.xyzr)", "*.xyzr"),
            selection: None,
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn selection(&self) -> Option<&Selector> {
        self.selection.as_ref()
    }

    pub fn get_selection(&self) -> Option<&Selector> {
        self.selection()
    }

    pub fn sel(&self) -> Option<&Selector> {
        self.selection()
    }

    pub fn set_selection(&mut self, value: Option<Selector>) {
        self.selection = value;
    }

    pub fn set_sel(&mut self, value: Option<Selector>) {
        self.set_selection(value);
    }

    pub fn write_xyzr_string(&self, molecule: &MolCoord) -> String {
        let mut output = String::new();
        for atom in selected_surface_atoms(molecule, self.selection.as_ref(), true) {
            // C++ XYZRFileWriter uses getVdwRadius(atom, false) = the united-atom
            // vdwhr (XYZRFileWriter.cpp:104), not the bare element radius.
            let radius = cuemol_molstr::toppar::vdw_radius_for_atom(atom);
            output.push_str(&format!(
                "{:>8.3} {:>8.3} {:>8.3} {:>4.2}\n",
                atom.position.x, atom.position.y, atom.position.z, radius
            ));
        }
        output
    }

    pub fn write_string(&self, molecule: &MolCoord) -> String {
        self.write_xyzr_string(molecule)
    }
}

impl Default for XyzrFileWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for XyzrFileWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for XyzrFileWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

fn selected_surface_atoms<'a>(
    molecule: &'a MolCoord,
    selection: Option<&Selector>,
    include_hydrogen: bool,
) -> Vec<&'a Atom> {
    molecule
        .atoms()
        .iter()
        .filter(|atom| include_hydrogen || !atom.element.eq_ignore_ascii_case("H"))
        .filter(|atom| {
            selection.is_none_or(|selector| selector.matches_in_molecule(atom, molecule))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceVertex {
    pub position: Vector4,
    pub normal: Vector4,
    /// Source atom identifier used by molecular-surface coloring and hit tests.
    pub info: u32,
}

impl SurfaceVertex {
    pub const fn new(position: Vector4, normal: Vector4) -> Self {
        Self {
            position,
            normal,
            info: 0,
        }
    }

    pub const fn with_info(position: Vector4, normal: Vector4, info: u32) -> Self {
        Self {
            position,
            normal,
            info,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceMesh {
    vertices: Vec<SurfaceVertex>,
    triangles: Vec<[u32; 3]>,
}

impl SurfaceMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.triangles.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.triangles.is_empty()
    }

    pub fn vertices(&self) -> &[SurfaceVertex] {
        &self.vertices
    }

    pub fn push_vertex(&mut self, vertex: SurfaceVertex) {
        self.vertices.push(vertex);
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    pub fn push_triangle(&mut self, triangle: [u32; 3]) {
        self.triangles.push(triangle);
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    pub fn apply_transform(&mut self, transform: &cuemol_core::Matrix4) {
        let inverse = transform.invert().ok();
        for vertex in &mut self.vertices {
            transform.xform3d(&mut vertex.position);
            if let Some(inverse) = inverse {
                let n = vertex.normal;
                let m = inverse.values;
                vertex.normal = Vector4::new3(
                    m[0][0] * n.x + m[1][0] * n.y + m[2][0] * n.z,
                    m[0][1] * n.x + m[1][1] * n.y + m[2][1] * n.z,
                    m[0][2] * n.x + m[1][2] * n.y + m[2][2] * n.z,
                )
                .normalize3d()
                .unwrap_or(n);
            }
        }
        let m = transform.values;
        let determinant = Vector4::new3(m[0][0], m[1][0], m[2][0])
            .cross(Vector4::new3(m[0][1], m[1][1], m[2][1]))
            .dot3d(Vector4::new3(m[0][2], m[1][2], m[2][2]));
        if determinant < 0.0 {
            for triangle in &mut self.triangles {
                triangle.swap(1, 2);
            }
        }
    }

    pub fn to_obj_string(&self, name: &str) -> String {
        let mut output = String::new();
        output.push_str("# CueMol2-rs surface mesh\n");
        output.push_str(&format!("o {}\n", sanitize_obj_name(name)));
        for vertex in &self.vertices {
            output.push_str(&format!(
                "v {:.6} {:.6} {:.6}\n",
                vertex.position.x, vertex.position.y, vertex.position.z
            ));
        }
        for vertex in &self.vertices {
            output.push_str(&format!(
                "vn {:.6} {:.6} {:.6}\n",
                vertex.normal.x, vertex.normal.y, vertex.normal.z
            ));
        }
        for triangle in &self.triangles {
            output.push_str(&format!(
                "f {0}//{0} {1}//{1} {2}//{2}\n",
                triangle[0] + 1,
                triangle[1] + 1,
                triangle[2] + 1
            ));
        }
        output
    }

    pub fn cut_by_plane(
        &mut self,
        normal: Vector4,
        position: Vector4,
        keep_positive: bool,
    ) -> usize {
        self.cut_by_plane_detailed(normal, position, keep_positive, true, false)
    }

    /// Clip triangles against a plane and optionally create the planar section.
    ///
    /// This is the mesh-level counterpart of C++ `CutByPlane2::doit`. Intersected
    /// triangles are split at the plane, normals are interpolated at new vertices,
    /// and all closed cut contours are tessellated together with the even-odd rule
    /// so nested contours become holes.
    pub fn cut_by_plane_detailed(
        &mut self,
        normal: Vector4,
        position: Vector4,
        keep_positive: bool,
        keep_body: bool,
        make_section: bool,
    ) -> usize {
        const CUT_EPS: f64 = 1.0e-3;

        let normal = normalize_or_z(normal);
        let keep_normal = if keep_positive { normal } else { normal * -1.0 };
        let before = self.triangles.len();
        let old_vertices = &self.vertices;
        let mut vertices = Vec::<SurfaceVertex>::new();
        let mut triangles = Vec::<[u32; 3]>::new();
        let mut copied = BTreeMap::<u32, u32>::new();
        let mut intersections = BTreeMap::<(u32, u32), u32>::new();
        let mut plane_edges = BTreeMap::<(u32, u32), usize>::new();

        for triangle in &self.triangles {
            if triangle.iter().any(|&id| id as usize >= old_vertices.len()) {
                continue;
            }

            let mut polygon = Vec::<u32>::with_capacity(4);
            let mut previous = triangle[2];
            let mut previous_distance =
                keep_normal.dot3d(old_vertices[previous as usize].position - position);
            let mut previous_inside = previous_distance >= -CUT_EPS;

            for &current in triangle {
                let current_distance =
                    keep_normal.dot3d(old_vertices[current as usize].position - position);
                let current_inside = current_distance >= -CUT_EPS;

                if current_inside != previous_inside {
                    let id = cut_intersection_vertex(
                        old_vertices,
                        &mut vertices,
                        &mut copied,
                        &mut intersections,
                        previous,
                        current,
                        previous_distance,
                        current_distance,
                        CUT_EPS,
                    );
                    push_distinct(&mut polygon, id);
                }
                if current_inside {
                    let id = copy_cut_vertex(old_vertices, &mut vertices, &mut copied, current);
                    push_distinct(&mut polygon, id);
                }

                previous = current;
                previous_distance = current_distance;
                previous_inside = current_inside;
            }

            if polygon.len() > 1 && polygon.first() == polygon.last() {
                polygon.pop();
            }
            if polygon.len() < 3 {
                continue;
            }

            for edge_index in 0..polygon.len() {
                let a = polygon[edge_index];
                let b = polygon[(edge_index + 1) % polygon.len()];
                if a == b {
                    continue;
                }
                let da = keep_normal.dot3d(vertices[a as usize].position - position);
                let db = keep_normal.dot3d(vertices[b as usize].position - position);
                if da.abs() <= CUT_EPS * 2.0 && db.abs() <= CUT_EPS * 2.0 {
                    let edge = ordered_edge(a, b);
                    *plane_edges.entry(edge).or_default() += 1;
                }
            }

            if keep_body {
                for index in 1..polygon.len() - 1 {
                    triangles.push([polygon[0], polygon[index], polygon[index + 1]]);
                }
            }
        }

        let body_triangle_count = triangles.len();
        if make_section {
            let boundary_edges = plane_edges
                .into_iter()
                .filter_map(|(edge, count)| (count % 2 == 1).then_some(edge))
                .collect::<BTreeSet<_>>();
            let contours = closed_cut_contours(&boundary_edges);
            append_cut_section(
                &mut vertices,
                &mut triangles,
                &contours,
                position,
                keep_normal,
            );
        }

        compact_surface_mesh(&mut vertices, &mut triangles);
        self.vertices = vertices;
        self.triangles = triangles;
        before.saturating_sub(body_triangle_count)
    }
}

fn copy_cut_vertex(
    old_vertices: &[SurfaceVertex],
    vertices: &mut Vec<SurfaceVertex>,
    copied: &mut BTreeMap<u32, u32>,
    old_id: u32,
) -> u32 {
    *copied.entry(old_id).or_insert_with(|| {
        let id = vertices.len() as u32;
        vertices.push(old_vertices[old_id as usize].clone());
        id
    })
}

#[allow(clippy::too_many_arguments)]
fn cut_intersection_vertex(
    old_vertices: &[SurfaceVertex],
    vertices: &mut Vec<SurfaceVertex>,
    copied: &mut BTreeMap<u32, u32>,
    intersections: &mut BTreeMap<(u32, u32), u32>,
    a: u32,
    b: u32,
    distance_a: f64,
    distance_b: f64,
    epsilon: f64,
) -> u32 {
    if distance_a.abs() <= epsilon {
        return copy_cut_vertex(old_vertices, vertices, copied, a);
    }
    if distance_b.abs() <= epsilon {
        return copy_cut_vertex(old_vertices, vertices, copied, b);
    }

    let edge = ordered_edge(a, b);
    *intersections.entry(edge).or_insert_with(|| {
        let denominator = distance_a - distance_b;
        let t = if denominator.abs() <= f64::EPSILON {
            0.5
        } else {
            (distance_a / denominator).clamp(0.0, 1.0)
        };
        let va = &old_vertices[a as usize];
        let vb = &old_vertices[b as usize];
        let position = va.position * (1.0 - t) + vb.position * t;
        let normal = normalize_or_z(va.normal * (1.0 - t) + vb.normal * t);
        let id = vertices.len() as u32;
        vertices.push(SurfaceVertex {
            position,
            normal,
            info: 0,
        });
        id
    })
}

fn ordered_edge(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn push_distinct(values: &mut Vec<u32>, value: u32) {
    if values.last().copied() != Some(value) {
        values.push(value);
    }
}

fn closed_cut_contours(edges: &BTreeSet<(u32, u32)>) -> Vec<Vec<u32>> {
    let mut adjacency = BTreeMap::<u32, Vec<u32>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }
    for neighbours in adjacency.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
    }

    let mut unused = edges.clone();
    let mut contours = Vec::new();
    while let Some(&(start, next)) = unused.iter().next() {
        unused.remove(&ordered_edge(start, next));
        let mut contour = vec![start];
        let mut previous = start;
        let mut current = next;
        let mut closed = false;

        for _ in 0..=edges.len() {
            if current == start {
                closed = true;
                break;
            }
            contour.push(current);
            let Some(candidate) = adjacency.get(&current).and_then(|neighbours| {
                neighbours
                    .iter()
                    .copied()
                    .find(|&id| id != previous && unused.contains(&ordered_edge(current, id)))
            }) else {
                break;
            };
            unused.remove(&ordered_edge(current, candidate));
            previous = current;
            current = candidate;
        }

        if closed && contour.len() >= 3 {
            contours.push(contour);
        }
    }
    contours
}

fn append_cut_section(
    vertices: &mut Vec<SurfaceVertex>,
    triangles: &mut Vec<[u32; 3]>,
    contours: &[Vec<u32>],
    plane_position: Vector4,
    keep_normal: Vector4,
) {
    use lyon::math::point;
    use lyon::path::Path;
    use lyon::tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    };

    if contours.is_empty() {
        return;
    }

    let seed = if keep_normal.x.abs() < 0.9 {
        Vector4::new3(1.0, 0.0, 0.0)
    } else {
        Vector4::new3(0.0, 1.0, 0.0)
    };
    let axis_u = normalize_or_z(keep_normal.cross(seed));
    let axis_v = normalize_or_z(keep_normal.cross(axis_u));
    let project = |value: Vector4| {
        let relative = value - plane_position;
        point(relative.dot3d(axis_u) as f32, relative.dot3d(axis_v) as f32)
    };

    let mut path_builder = Path::builder();
    for contour in contours {
        let Some((&first, rest)) = contour.split_first() else {
            continue;
        };
        path_builder.begin(project(vertices[first as usize].position));
        for &id in rest {
            path_builder.line_to(project(vertices[id as usize].position));
        }
        path_builder.close();
    }
    let path = path_builder.build();
    let mut geometry = VertexBuffers::<[f32; 2], u32>::new();
    let result = FillTessellator::new().tessellate_path(
        &path,
        &FillOptions::even_odd().with_tolerance(1.0e-4),
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
            vertex.position().to_array()
        }),
    );
    if result.is_err() {
        return;
    }

    let cap_normal = keep_normal * -1.0;
    let base = vertices.len() as u32;
    for [x, y] in geometry.vertices {
        vertices.push(SurfaceVertex {
            position: plane_position + axis_u * x as f64 + axis_v * y as f64,
            normal: cap_normal,
            info: 0,
        });
    }
    for indices in geometry.indices.chunks_exact(3) {
        let mut triangle = [base + indices[0], base + indices[1], base + indices[2]];
        let a = vertices[triangle[0] as usize].position;
        let b = vertices[triangle[1] as usize].position;
        let c = vertices[triangle[2] as usize].position;
        if (b - a).cross(c - a).dot3d(cap_normal) < 0.0 {
            triangle.swap(1, 2);
        }
        triangles.push(triangle);
    }
}

fn compact_surface_mesh(vertices: &mut Vec<SurfaceVertex>, triangles: &mut [[u32; 3]]) {
    let mut mapping = BTreeMap::<u32, u32>::new();
    let mut compacted = Vec::new();
    for triangle in triangles.iter_mut() {
        for id in triangle.iter_mut() {
            let new_id = *mapping.entry(*id).or_insert_with(|| {
                let new_id = compacted.len() as u32;
                compacted.push(vertices[*id as usize].clone());
                new_id
            });
            *id = new_id;
        }
    }
    *vertices = compacted;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApbsInputConfig {
    pub molecule_name: String,
    pub pqr_path: String,
    pub dx_path: String,
    pub grid: [usize; 3],
    pub coarse_length: [f64; 3],
    pub fine_length: [f64; 3],
    pub center: Vector4,
    pub temperature: f64,
    pub solvent_dielectric: f64,
    pub solute_dielectric: f64,
    pub ion_charge: f64,
    pub ion_concentration: f64,
    pub ion_radius: f64,
}

impl ApbsInputConfig {
    pub fn new(
        molecule_name: impl Into<String>,
        pqr_path: impl Into<String>,
        dx_path: impl Into<String>,
    ) -> Self {
        Self {
            molecule_name: molecule_name.into(),
            pqr_path: pqr_path.into(),
            dx_path: dx_path.into(),
            grid: [65, 65, 65],
            coarse_length: [80.0, 80.0, 80.0],
            fine_length: [60.0, 60.0, 60.0],
            center: Vector4::ZERO,
            temperature: 298.15,
            solvent_dielectric: 78.54,
            solute_dielectric: 2.0,
            ion_charge: 1.0,
            ion_concentration: 0.15,
            ion_radius: 2.0,
        }
    }

    pub fn to_input_string(&self) -> String {
        format!(
            "read\n  mol pqr {}\nend\n\n\
elec\n  mg-auto\n  dime {} {} {}\n  cglen {:.3} {:.3} {:.3}\n  fglen {:.3} {:.3} {:.3}\n  cgcent {:.3} {:.3} {:.3}\n  fgcent {:.3} {:.3} {:.3}\n  mol 1\n  lpbe\n  bcfl sdh\n  pdie {:.3}\n  sdie {:.3}\n  chgm spl2\n  srfm smol\n  srad 1.400\n  swin 0.300\n  temp {:.2}\n  ion charge {:.1} conc {:.4} radius {:.3}\n  ion charge {:.1} conc {:.4} radius {:.3}\n  write pot dx {}\nend\n\nquit\n",
            self.pqr_path,
            self.grid[0],
            self.grid[1],
            self.grid[2],
            self.coarse_length[0],
            self.coarse_length[1],
            self.coarse_length[2],
            self.fine_length[0],
            self.fine_length[1],
            self.fine_length[2],
            self.center.x,
            self.center.y,
            self.center.z,
            self.center.x,
            self.center.y,
            self.center.z,
            self.solute_dielectric,
            self.solvent_dielectric,
            self.temperature,
            self.ion_charge,
            self.ion_concentration,
            self.ion_radius,
            -self.ion_charge,
            self.ion_concentration,
            self.ion_radius,
            self.dx_path
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceVdwRadii {
    h: f64,
    c: f64,
    n: f64,
    o: f64,
    s: f64,
    p: f64,
    x: f64,
}

impl Default for SurfaceVdwRadii {
    fn default() -> Self {
        Self {
            h: 1.2,
            c: 1.7,
            n: 1.55,
            o: 1.52,
            s: 1.8,
            p: 1.8,
            x: 1.7,
        }
    }
}

impl SurfaceVdwRadii {
    pub fn radius_for_element(&self, element: &str) -> f64 {
        match element.trim().to_ascii_uppercase().as_str() {
            "H" => self.h,
            "C" => self.c,
            "N" => self.n,
            "O" => self.o,
            "S" => self.s,
            "P" => self.p,
            _ => self.x,
        }
    }

    pub fn h(&self) -> f64 {
        self.h
    }

    pub fn set_h(&mut self, value: f64) {
        self.h = value;
    }

    pub fn c(&self) -> f64 {
        self.c
    }

    pub fn set_c(&mut self, value: f64) {
        self.c = value;
    }

    pub fn n(&self) -> f64 {
        self.n
    }

    pub fn set_n(&mut self, value: f64) {
        self.n = value;
    }

    pub fn o(&self) -> f64 {
        self.o
    }

    pub fn set_o(&mut self, value: f64) {
        self.o = value;
    }

    pub fn s(&self) -> f64 {
        self.s
    }

    pub fn set_s(&mut self, value: f64) {
        self.s = value;
    }

    pub fn p(&self) -> f64 {
        self.p
    }

    pub fn set_p(&mut self, value: f64) {
        self.p = value;
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn set_x(&mut self, value: f64) {
        self.x = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutPlane {
    pub cdiv: f64,
    pub normal: Vector4,
    pub position: Vector4,
    pub body: bool,
    pub section: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolSurfObj {
    mesh: SurfaceMesh,
    orig_mol: String,
    orig_sel: String,
    orig_selector: Option<Selector>,
    orig_den: f64,
    orig_prad: f64,
    ses_backend: backends::SesBackend,
    ses_backend_used: Option<backends::SesBackend>,
    cut_planes: Vec<CutPlane>,
    source_molecule: Option<MolCoord>,
}

pub enum SurfaceMoleculeSource<'a> {
    Name(String),
    Molecule(&'a MolCoord),
}

pub trait IntoSurfaceMoleculeSource<'a> {
    fn into_surface_molecule_source(self) -> SurfaceMoleculeSource<'a>;
}

impl<'a> IntoSurfaceMoleculeSource<'a> for &'a MolCoord {
    fn into_surface_molecule_source(self) -> SurfaceMoleculeSource<'a> {
        SurfaceMoleculeSource::Molecule(self)
    }
}

impl<'a> IntoSurfaceMoleculeSource<'a> for &'a str {
    fn into_surface_molecule_source(self) -> SurfaceMoleculeSource<'a> {
        SurfaceMoleculeSource::Name(self.to_string())
    }
}

impl<'a> IntoSurfaceMoleculeSource<'a> for String {
    fn into_surface_molecule_source(self) -> SurfaceMoleculeSource<'a> {
        SurfaceMoleculeSource::Name(self)
    }
}

pub enum SurfaceSelectionSource<'a> {
    Text(String),
    Selector(&'a Selector),
    OwnedSelector(Selector),
}

pub trait IntoSurfaceSelectionSource<'a> {
    fn into_surface_selection_source(self) -> SurfaceSelectionSource<'a>;
}

impl<'a> IntoSurfaceSelectionSource<'a> for &'a Selector {
    fn into_surface_selection_source(self) -> SurfaceSelectionSource<'a> {
        SurfaceSelectionSource::Selector(self)
    }
}

impl<'a> IntoSurfaceSelectionSource<'a> for Selector {
    fn into_surface_selection_source(self) -> SurfaceSelectionSource<'a> {
        SurfaceSelectionSource::OwnedSelector(self)
    }
}

impl<'a> IntoSurfaceSelectionSource<'a> for &'a str {
    fn into_surface_selection_source(self) -> SurfaceSelectionSource<'a> {
        SurfaceSelectionSource::Text(self.to_string())
    }
}

impl<'a> IntoSurfaceSelectionSource<'a> for String {
    fn into_surface_selection_source(self) -> SurfaceSelectionSource<'a> {
        SurfaceSelectionSource::Text(self)
    }
}

impl SurfaceMoleculeSource<'_> {
    fn source_name(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Molecule(_) => "MolCoord".to_string(),
        }
    }
}

impl SurfaceSelectionSource<'_> {
    fn source_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Selector(selector) => selector.dump(),
            Self::OwnedSelector(selector) => selector.dump(),
        }
    }

    fn selector(&self) -> Option<Selector> {
        match self {
            Self::Text(text) => parse_selector(text).ok(),
            Self::Selector(selector) => Some((*selector).clone()),
            Self::OwnedSelector(selector) => Some(selector.clone()),
        }
    }
}

impl Default for MolSurfObj {
    fn default() -> Self {
        Self {
            mesh: SurfaceMesh::new(),
            orig_mol: String::new(),
            orig_sel: String::new(),
            orig_selector: Some(Selector::All),
            orig_den: 0.0,
            orig_prad: 0.0,
            ses_backend: backends::SesBackend::Auto,
            ses_backend_used: None,
            cut_planes: Vec::new(),
            source_molecule: None,
        }
    }
}

impl MolSurfObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clean(&mut self) {
        self.mesh.clear();
        self.cut_planes.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.mesh.vertices.is_empty() && self.mesh.triangles.is_empty()
    }

    pub fn delete_selected(&mut self) {
        self.clean();
    }

    pub fn mesh(&self) -> &SurfaceMesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut SurfaceMesh {
        &mut self.mesh
    }

    pub fn vert_size(&self) -> usize {
        self.mesh.vertex_count()
    }

    pub fn get_vert_size(&self) -> usize {
        self.vert_size()
    }

    pub fn set_vert_size(&mut self, size: usize) -> cuemol_core::Result<()> {
        if size == 0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "surface vertex array size must be positive",
            ));
        }
        self.mesh.vertices = vec![
            SurfaceVertex {
                position: Vector4::ZERO,
                normal: Vector4::ZERO,
                info: 0,
            };
            size
        ];
        Ok(())
    }

    pub fn vertex_at(&self, index: usize) -> Option<&SurfaceVertex> {
        self.mesh.vertices.get(index)
    }

    pub fn get_vert_at(&self, index: usize) -> Option<&SurfaceVertex> {
        self.vertex_at(index)
    }

    pub fn set_vertex(
        &mut self,
        index: usize,
        position: Vector4,
        normal: Vector4,
    ) -> cuemol_core::Result<()> {
        let vertex = self.mesh.vertices.get_mut(index).ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("surface vertex index out of range")
        })?;
        let info = vertex.info;
        *vertex = SurfaceVertex {
            position,
            normal,
            info,
        };
        Ok(())
    }

    pub fn face_size(&self) -> usize {
        self.mesh.triangle_count()
    }

    pub fn get_face_size(&self) -> usize {
        self.face_size()
    }

    pub fn set_face_size(&mut self, size: usize) -> cuemol_core::Result<()> {
        if size == 0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "surface face array size must be positive",
            ));
        }
        self.mesh.triangles = vec![[0; 3]; size];
        Ok(())
    }

    pub fn face_at(&self, index: usize) -> Option<&[u32; 3]> {
        self.mesh.triangles.get(index)
    }

    pub fn get_face_at(&self, index: usize) -> Option<&[u32; 3]> {
        self.face_at(index)
    }

    pub fn set_face(
        &mut self,
        index: usize,
        vertex1: u32,
        vertex2: u32,
        vertex3: u32,
    ) -> cuemol_core::Result<()> {
        if [vertex1, vertex2, vertex3]
            .iter()
            .any(|vertex| *vertex as usize >= self.mesh.vertex_count())
        {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "surface face vertex index out of range",
            ));
        }
        let face = self.mesh.triangles.get_mut(index).ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("surface face index out of range")
        })?;
        *face = [vertex1, vertex2, vertex3];
        Ok(())
    }

    pub fn orig_mol(&self) -> &str {
        &self.orig_mol
    }

    pub fn get_orig_mol_name(&self) -> &str {
        self.orig_mol()
    }

    pub fn set_orig_mol(&mut self, value: impl Into<String>) {
        self.orig_mol = value.into();
    }

    pub fn set_orig_mol_name(&mut self, value: impl Into<String>) {
        self.set_orig_mol(value);
    }

    pub fn orig_sel(&self) -> &str {
        &self.orig_sel
    }

    pub fn orig_selection(&self) -> Option<&Selector> {
        self.orig_selector.as_ref()
    }

    pub fn get_orig_sel(&self) -> Option<&Selector> {
        self.orig_selection()
    }

    pub fn set_orig_sel(&mut self, value: impl Into<String>) {
        self.orig_sel = value.into();
        self.orig_selector = if self.orig_sel.trim().is_empty() {
            Some(Selector::All)
        } else {
            parse_selector(&self.orig_sel).ok()
        };
    }

    pub fn orig_den(&self) -> f64 {
        self.orig_den
    }

    pub fn get_orig_den(&self) -> f64 {
        self.orig_den()
    }

    pub fn set_orig_den(&mut self, value: f64) {
        self.orig_den = value;
    }

    pub fn orig_prad(&self) -> f64 {
        self.orig_prad
    }

    pub fn get_orig_probe_rad(&self) -> f64 {
        self.orig_prad()
    }

    pub fn set_orig_prad(&mut self, value: f64) {
        self.orig_prad = value;
    }

    pub fn set_orig_probe_rad(&mut self, value: f64) {
        self.set_orig_prad(value);
    }

    pub fn ses_backend(&self) -> backends::SesBackend {
        self.ses_backend
    }

    pub fn set_ses_backend(&mut self, backend: backends::SesBackend) {
        self.ses_backend = backend;
    }

    pub fn ses_backend_used(&self) -> Option<backends::SesBackend> {
        self.ses_backend_used
    }

    pub fn set_ses_backend_used(&mut self, backend: Option<backends::SesBackend>) {
        self.ses_backend_used = backend;
    }

    pub fn cut_planes(&self) -> &[CutPlane] {
        &self.cut_planes
    }

    pub fn cut_by_plane(&mut self, cdiv: f64, normal: Vector4, position: Vector4, section: bool) {
        self.cut_by_plane2(cdiv, normal, position, true, section);
    }

    pub fn cut_by_plane2(
        &mut self,
        cdiv: f64,
        normal: Vector4,
        position: Vector4,
        body: bool,
        section: bool,
    ) {
        self.mesh
            .cut_by_plane_detailed(normal, position, true, body, section);
        self.cut_planes.push(CutPlane {
            cdiv,
            normal,
            position,
            body,
            section,
        });
    }

    pub fn cut_by_plane_2(
        &mut self,
        cdiv: f64,
        normal: Vector4,
        position: Vector4,
        body: bool,
        section: bool,
    ) {
        self.cut_by_plane2(cdiv, normal, position, body, section);
    }

    pub fn create_ses_from_mol<'a, M, S>(
        &mut self,
        molecule: M,
        selection: S,
        density: f64,
        probe_radius: f64,
    ) -> cuemol_core::Result<()>
    where
        M: IntoSurfaceMoleculeSource<'a>,
        S: IntoSurfaceSelectionSource<'a>,
    {
        let molecule = molecule.into_surface_molecule_source();
        let selection = selection.into_surface_selection_source();
        let selector = selection.selector().ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("invalid surface atom selection")
        })?;
        if let SurfaceMoleculeSource::Molecule(mol) = &molecule {
            self.create_ses_from_mol_with_backend(
                mol,
                &selector,
                density,
                probe_radius,
                self.ses_backend,
            )?;
        } else {
            self.mesh.clear();
            self.source_molecule = None;
            self.ses_backend_used = None;
            self.cut_planes.clear();
        }
        self.orig_mol = molecule.source_name();
        self.orig_sel = selection.source_text();
        self.orig_selector = Some(selector);
        self.orig_den = density;
        self.orig_prad = probe_radius;
        Ok(())
    }

    pub fn create_ses_from_mol_with_backend(
        &mut self,
        molecule: &MolCoord,
        selection: &Selector,
        density: f64,
        probe_radius: f64,
        backend: backends::SesBackend,
    ) -> cuemol_core::Result<backends::SesBackend> {
        let (mesh, used) =
            backends::build_molecular_surface(molecule, selection, density, probe_radius, backend)?;
        self.mesh = mesh;
        self.ses_backend = backend;
        self.ses_backend_used = Some(used);
        self.orig_selector = Some(selection.clone());
        self.orig_sel = selection.dump();
        self.orig_den = density;
        self.orig_prad = probe_radius;
        self.source_molecule = Some(molecule.clone());
        self.cut_planes.clear();
        Ok(used)
    }

    pub fn create_ses_from_array(
        &mut self,
        spheres: &[SurfSphere],
        density: f64,
        probe_radius: f64,
    ) -> cuemol_core::Result<()> {
        let (mesh, used) =
            backends::build_surface(spheres, density, probe_radius, self.ses_backend)?;
        self.mesh = mesh;
        self.ses_backend_used = Some(used);
        self.cut_planes.clear();
        Ok(())
    }

    pub fn create_sesfrom_array(
        &mut self,
        spheres: &[SurfSphere],
        density: f64,
        probe_radius: f64,
    ) -> cuemol_core::Result<()> {
        self.create_ses_from_array(spheres, density, probe_radius)
    }

    pub fn create_sesfrom_mol<'a, M, S>(
        &mut self,
        molecule: M,
        selection: S,
        density: f64,
        probe_radius: f64,
    ) -> cuemol_core::Result<()>
    where
        M: IntoSurfaceMoleculeSource<'a>,
        S: IntoSurfaceSelectionSource<'a>,
    {
        self.create_ses_from_mol(molecule, selection, density, probe_radius)
    }

    pub fn regenerate_ses(&mut self, density: f64) -> cuemol_core::Result<()> {
        let molecule = self.source_molecule.clone().ok_or_else(|| {
            cuemol_core::CueMolError::undefined_reference(
                "cannot regenerate surface: original molecule is unavailable",
            )
        })?;
        self.regenerate_ses_from_mol(&molecule, None, density, -1.0)
    }

    pub fn regenerate_ses1(&mut self, density: f64) -> cuemol_core::Result<()> {
        self.regenerate_ses(density)
    }

    pub fn regenerate_ses_1(&mut self, density: f64) -> cuemol_core::Result<()> {
        self.regenerate_ses1(density)
    }

    pub fn regenerate_ses_from_mol(
        &mut self,
        molecule: &MolCoord,
        selection: Option<&Selector>,
        density: f64,
        probe_radius: f64,
    ) -> cuemol_core::Result<()> {
        if self.orig_den < 0.0 || self.orig_prad < 0.0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "cannot regenerate surface: invalid original density or probe radius",
            ));
        }
        let density = if density < 0.0 {
            self.orig_den
        } else {
            density
        };
        let probe_radius = if probe_radius < 0.0 {
            self.orig_prad
        } else {
            probe_radius
        };
        if !density.is_finite() || density <= 0.0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "surface density must be positive",
            ));
        }
        if !probe_radius.is_finite() || probe_radius < 0.0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "surface probe radius must be non-negative",
            ));
        }

        let selector = selection
            .cloned()
            .unwrap_or_else(|| parse_selector(&self.orig_sel).unwrap_or(Selector::All));
        self.create_ses_from_mol_with_backend(
            molecule,
            &selector,
            density,
            probe_radius,
            self.ses_backend,
        )?;
        Ok(())
    }

    pub fn create_hole_test1<'a, M>(
        &mut self,
        molecule: M,
        dir_norm: Vector4,
        start_pos: Vector4,
    ) -> cuemol_core::Result<()>
    where
        M: IntoSurfaceMoleculeSource<'a>,
    {
        let molecule = molecule.into_surface_molecule_source();
        let source_name = molecule.source_name();
        let SurfaceMoleculeSource::Molecule(molecule) = molecule else {
            return Err(cuemol_core::CueMolError::undefined_reference(
                "createHoleTest1 requires a resolved molecule",
            ));
        };
        hole_surface::build_hole_test_tube(&mut self.mesh, molecule, dir_norm, start_pos)?;
        self.orig_mol = source_name;
        self.orig_sel = format!("hole-start:{},{},{}", start_pos.x, start_pos.y, start_pos.z);
        self.source_molecule = None;
        self.cut_planes.clear();
        Ok(())
    }

    pub fn create_hole_test_1<'a, M>(
        &mut self,
        molecule: M,
        dir_norm: Vector4,
        start_pos: Vector4,
    ) -> cuemol_core::Result<()>
    where
        M: IntoSurfaceMoleculeSource<'a>,
    {
        self.create_hole_test1(molecule, dir_norm, start_pos)
    }
}

pub type SharedMolSurfObj = Rc<RefCell<MolSurfObj>>;

/// Undo/redo record for molecular-surface geometry edits.
///
/// Like C++ `MolSurfEditInfo`, this keeps the geometry on the opposite side of
/// the current edit and swaps it with the target on both undo and redo. The Rust
/// version also swaps the recorded cut-plane list so the public object state
/// remains consistent with the restored mesh.
#[derive(Debug)]
pub struct MolSurfEditInfo {
    target: Weak<RefCell<MolSurfObj>>,
    mesh: SurfaceMesh,
    cut_planes: Vec<CutPlane>,
}

impl MolSurfEditInfo {
    pub fn setup(target: &SharedMolSurfObj) -> Self {
        let target_ref = target.borrow();
        Self {
            target: Rc::downgrade(target),
            mesh: target_ref.mesh.clone(),
            cut_planes: target_ref.cut_planes.clone(),
        }
    }

    pub fn clear(&mut self) {
        self.mesh.clear();
        self.cut_planes.clear();
    }

    fn swap_target_state(&mut self) -> bool {
        let Some(target) = self.target.upgrade() else {
            return false;
        };
        let mut target = target.borrow_mut();
        std::mem::swap(&mut target.mesh, &mut self.mesh);
        std::mem::swap(&mut target.cut_planes, &mut self.cut_planes);
        true
    }
}

impl EditInfo for MolSurfEditInfo {
    fn undo(&mut self) -> bool {
        self.swap_target_state()
    }

    fn redo(&mut self) -> bool {
        self.swap_target_state()
    }
}

/// Apply a plane cut and add a `MolSurfEditInfo` to an active scene undo
/// transaction. The edit is always performed; the return value reports whether
/// an undo record was accepted by the manager.
#[allow(clippy::too_many_arguments)]
pub fn cut_surface_by_plane_with_undo(
    target: &SharedMolSurfObj,
    undo_manager: &UndoManagerHandle,
    cdiv: f64,
    normal: Vector4,
    position: Vector4,
    body: bool,
    section: bool,
) -> bool {
    let edit = undo_manager
        .with(|manager| manager.is_ok())
        .then(|| MolSurfEditInfo::setup(target));
    target
        .borrow_mut()
        .cut_by_plane2(cdiv, normal, position, body, section);
    edit.is_some_and(|edit| undo_manager.with_mut(|manager| manager.add_edit_info(Box::new(edit))))
}

/// Regenerate a surface from the current molecule and register the geometry
/// swap with an active scene undo transaction.
pub fn regenerate_surface_with_undo(
    target: &SharedMolSurfObj,
    undo_manager: &UndoManagerHandle,
    molecule: &MolCoord,
    selection: Option<&Selector>,
    density: f64,
    probe_radius: f64,
) -> cuemol_core::Result<bool> {
    let edit = undo_manager
        .with(|manager| manager.is_ok())
        .then(|| MolSurfEditInfo::setup(target));
    target
        .borrow_mut()
        .regenerate_ses_from_mol(molecule, selection, density, probe_radius)?;
    Ok(edit
        .is_some_and(|edit| undo_manager.with_mut(|manager| manager.add_edit_info(Box::new(edit)))))
}

/// Build an analytic Connolly SES from convex, saddle, and reentrant patches.
/// `detail` controls tessellation of the Rust analytic implementation.
pub fn build_analytic_mol_surface(
    molecule: &MolCoord,
    selector: &Selector,
    probe_radius: f64,
    detail: usize,
) -> SurfaceMesh {
    let atoms: Vec<(cuemol_core::Vector4, f64)> = molecule
        .atoms()
        .iter()
        .filter(|atom| selector.matches_in_molecule(atom, molecule))
        .map(|atom| {
            (
                atom.position,
                cuemol_molstr::toppar::vdw_radius_for_atom(atom).max(0.1),
            )
        })
        .collect();
    analytic_ses::build_analytic_ses(&atoms, probe_radius.max(0.0), detail)
}

fn sanitize_obj_name(name: &str) -> String {
    let name = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.trim_matches('_').is_empty() {
        "surface".to_string()
    } else {
        name
    }
}

fn normalize_or_z(value: Vector4) -> Vector4 {
    let length = value.length3d();
    if length <= 1.0e-12 {
        Vector4::new3(0.0, 0.0, 1.0)
    } else {
        value.scale(1.0 / length)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolSurfRenderer {
    default_color: SolidColor,
    cull_face: bool,
    draw_mode: i32,
    color_mode: i32,
    target: String,
    width: f64,
    elepot: String,
    color_map_name: String,
    low_param: f64,
    mid_param: f64,
    high_param: f64,
    high_color: SolidColor,
    mid_color: SolidColor,
    low_color: SolidColor,
    multi_grad: MultiGradient,
    ramp_above: bool,
    ramp_value: f64,
    coloring: SolidColoring,
    selection: String,
    show_selection: String,
}

impl Default for MolSurfRenderer {
    fn default() -> Self {
        Self {
            default_color: SolidColor::from_rgba(192.0 / 255.0, 192.0 / 255.0, 1.0, 1.0),
            cull_face: false,
            draw_mode: Self::SFDRAW_FILL,
            color_mode: Self::SFREND_SIMPLE,
            target: String::new(),
            width: 1.2,
            elepot: String::new(),
            color_map_name: String::new(),
            low_param: -10.0,
            mid_param: 0.0,
            high_param: 10.0,
            high_color: SolidColor::from_rgb(0.0, 0.0, 1.0),
            mid_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
            low_color: SolidColor::from_rgb(1.0, 0.0, 0.0),
            multi_grad: MultiGradient::create_default(),
            ramp_above: false,
            ramp_value: 1.4,
            coloring: SolidColoring::new(),
            selection: String::new(),
            show_selection: String::new(),
        }
    }
}

impl MolSurfRenderer {
    pub const SFDRAW_FILL: i32 = 0;
    pub const SFDRAW_LINE: i32 = 1;
    pub const SFDRAW_POINT: i32 = 2;
    pub const SFREND_SIMPLE: i32 = 0;
    pub const SFREND_SCAPOT: i32 = 1;
    pub const SFREND_MOLFANC: i32 = 2;
    pub const SFREND_MULTIGRAD: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "molsurf"
    }

    pub fn default_color(&self) -> &SolidColor {
        &self.default_color
    }

    pub fn get_default_color(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn defaultcolor(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn set_default_color(&mut self, value: SolidColor) {
        self.default_color = value;
    }

    pub fn set_defaultcolor(&mut self, value: SolidColor) {
        self.set_default_color(value);
    }

    pub fn cull_face(&self) -> bool {
        self.cull_face
    }

    pub fn cullface(&self) -> bool {
        self.cull_face()
    }

    pub fn is_cull_face(&self) -> bool {
        self.cull_face()
    }

    pub fn set_cull_face(&mut self, value: bool) {
        self.cull_face = value;
    }

    pub fn set_cullface(&mut self, value: bool) {
        self.set_cull_face(value);
    }

    pub fn draw_mode(&self) -> i32 {
        self.draw_mode
    }

    pub fn drawmode(&self) -> i32 {
        self.draw_mode()
    }

    pub fn get_draw_mode(&self) -> i32 {
        self.draw_mode()
    }

    pub fn set_draw_mode(&mut self, value: i32) {
        self.draw_mode = value;
    }

    pub fn set_drawmode(&mut self, value: i32) {
        self.set_draw_mode(value);
    }

    pub fn color_mode(&self) -> i32 {
        self.color_mode
    }

    pub fn get_color_mode(&self) -> i32 {
        self.color_mode()
    }

    pub fn colormode(&self) -> i32 {
        self.color_mode()
    }

    pub fn set_color_mode(&mut self, value: i32) {
        self.color_mode = value;
    }

    pub fn set_colormode(&mut self, value: i32) {
        self.set_color_mode(value);
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn get_tgt_obj_name(&self) -> &str {
        self.target()
    }

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
    }

    pub fn set_tgt_obj_name(&mut self, value: impl Into<String>) {
        self.set_target(value);
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_line_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.set_width(value);
    }

    pub fn elepot(&self) -> &str {
        &self.elepot
    }

    pub fn get_tgt_ele_pot_name(&self) -> &str {
        self.elepot()
    }

    pub fn set_elepot(&mut self, value: impl Into<String>) {
        self.elepot = value.into();
    }

    pub fn set_tgt_ele_pot_name(&mut self, value: impl Into<String>) {
        self.set_elepot(value);
    }

    pub fn color_map_name(&self) -> &str {
        &self.color_map_name
    }

    pub fn color_mapname(&self) -> &str {
        self.color_map_name()
    }

    pub fn get_color_map_name(&self) -> &str {
        self.color_map_name()
    }

    pub fn get_color_map_obj(&self) -> Option<&MultiGradient> {
        Some(&self.multi_grad)
    }

    pub fn set_color_map_name(&mut self, value: impl Into<String>) {
        self.color_map_name = value.into();
    }

    pub fn set_color_mapname(&mut self, value: impl Into<String>) {
        self.set_color_map_name(value);
    }

    pub fn low_param(&self) -> f64 {
        self.low_param
    }

    pub fn lowpar(&self) -> f64 {
        self.low_param()
    }

    pub fn get_low_par(&self) -> f64 {
        self.low_param()
    }

    pub fn set_low_param(&mut self, value: f64) {
        self.low_param = value;
    }

    pub fn set_lowpar(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn set_low_par(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn mid_param(&self) -> f64 {
        self.mid_param
    }

    pub fn midpar(&self) -> f64 {
        self.mid_param()
    }

    pub fn get_mid_par(&self) -> f64 {
        self.mid_param()
    }

    pub fn set_mid_param(&mut self, value: f64) {
        self.mid_param = value;
    }

    pub fn set_midpar(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn set_mid_par(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn high_param(&self) -> f64 {
        self.high_param
    }

    pub fn highpar(&self) -> f64 {
        self.high_param()
    }

    pub fn get_high_par(&self) -> f64 {
        self.high_param()
    }

    pub fn set_high_param(&mut self, value: f64) {
        self.high_param = value;
    }

    pub fn set_highpar(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn set_high_par(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn high_color(&self) -> &SolidColor {
        &self.high_color
    }

    pub fn highcol(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn get_high_col(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn set_high_color(&mut self, value: SolidColor) {
        self.high_color = value;
    }

    pub fn set_highcol(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn set_high_col(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn mid_color(&self) -> &SolidColor {
        &self.mid_color
    }

    pub fn midcol(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn get_mid_col(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn set_mid_color(&mut self, value: SolidColor) {
        self.mid_color = value;
    }

    pub fn set_midcol(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn set_mid_col(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn low_color(&self) -> &SolidColor {
        &self.low_color
    }

    pub fn lowcol(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn get_low_col(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn set_low_color(&mut self, value: SolidColor) {
        self.low_color = value;
    }

    pub fn set_lowcol(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn set_low_col(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn multi_grad(&self) -> &MultiGradient {
        &self.multi_grad
    }

    pub fn get_multi_grad(&self) -> &MultiGradient {
        self.multi_grad()
    }

    pub fn is_wire_frame(&self) -> bool {
        self.draw_mode == Self::SFDRAW_LINE
    }

    pub fn is_dot_surf(&self) -> bool {
        self.draw_mode == Self::SFDRAW_POINT
    }

    pub fn get_col_schm(&self) -> SolidColoring {
        self.coloring()
    }

    pub fn set_col_schm(&mut self, value: SolidColoring) {
        self.set_coloring(value);
    }

    pub fn get_mol_sel(&self) -> &str {
        self.selection()
    }

    pub fn sel(&self) -> &str {
        self.selection()
    }

    pub fn set_mol_sel(&mut self, value: impl Into<String>) {
        self.set_selection(value);
    }

    pub fn set_sel(&mut self, value: impl Into<String>) {
        self.set_selection(value);
    }

    pub fn ramp_above(&self) -> bool {
        self.ramp_above
    }

    pub fn is_ramp_above(&self) -> bool {
        self.ramp_above()
    }

    pub fn set_ramp_above(&mut self, value: bool) {
        self.ramp_above = value;
    }

    pub fn ramp_value(&self) -> f64 {
        self.ramp_value
    }

    pub fn get_ramp_value(&self) -> f64 {
        self.ramp_value()
    }

    pub fn set_ramp_value(&mut self, value: f64) {
        self.ramp_value = value;
    }

    pub fn coloring(&self) -> SolidColoring {
        self.coloring
    }

    pub fn set_coloring(&mut self, value: SolidColoring) {
        self.coloring = value;
    }

    pub fn selection(&self) -> &str {
        &self.selection
    }

    pub fn set_selection(&mut self, value: impl Into<String>) {
        self.selection = value.into();
    }

    pub fn show_selection(&self) -> &str {
        &self.show_selection
    }

    pub fn get_show_sel(&self) -> &str {
        self.show_selection()
    }

    pub fn showsel(&self) -> &str {
        self.show_selection()
    }

    pub fn set_show_selection(&mut self, value: impl Into<String>) {
        self.show_selection = value.into();
    }

    pub fn set_show_sel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }

    pub fn set_showsel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectSurfRenderer {
    color_mode: i32,
    target: String,
    selection: Selector,
    detail: i32,
    probe_radius: f64,
    surface_type: i32,
    surface_algorithm: i32,
    cull_face: bool,
    draw_mode: i32,
    width: f64,
    show_selection: String,
    vdw_radii: SurfaceVdwRadii,
    elepot: String,
    low_param: f64,
    mid_param: f64,
    high_param: f64,
    high_color: SolidColor,
    mid_color: SolidColor,
    low_color: SolidColor,
    ramp_above: bool,
    ramp_value: f64,
}

impl Default for DirectSurfRenderer {
    fn default() -> Self {
        Self {
            color_mode: Self::DS_MOLFANC,
            target: String::new(),
            selection: Selector::All,
            detail: 6,
            probe_radius: 1.4,
            surface_type: Self::DS_SES,
            surface_algorithm: Self::DS_DISTFIELD,
            cull_face: false,
            draw_mode: Self::SFDRAW_FILL,
            width: 1.2,
            show_selection: String::new(),
            vdw_radii: SurfaceVdwRadii::default(),
            elepot: String::new(),
            low_param: -10.0,
            mid_param: 0.0,
            high_param: 10.0,
            high_color: SolidColor::from_rgb(0.0, 0.0, 1.0),
            mid_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
            low_color: SolidColor::from_rgb(1.0, 0.0, 0.0),
            ramp_above: false,
            ramp_value: 1.4,
        }
    }
}

impl DirectSurfRenderer {
    pub const DS_SCAPOT: i32 = 1;
    pub const DS_MOLFANC: i32 = 2;
    pub const DS_VDW: i32 = 0;
    pub const DS_SAS: i32 = 1;
    pub const DS_SES: i32 = 2;
    pub const DS_EDTSURF: i32 = 0;
    pub const DS_DISTFIELD: i32 = 1;
    pub const DS_MESHMS: i32 = 2;
    pub const DS_MSMS: i32 = Self::DS_MESHMS;
    pub const SFDRAW_FILL: i32 = 0;
    pub const SFDRAW_LINE: i32 = 1;
    pub const SFDRAW_POINT: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "dsurface"
    }

    pub fn color_mode(&self) -> i32 {
        self.color_mode
    }

    pub fn get_color_mode(&self) -> i32 {
        self.color_mode()
    }

    pub fn colormode(&self) -> i32 {
        self.color_mode()
    }

    pub fn set_color_mode(&mut self, value: i32) {
        self.color_mode = value;
    }

    pub fn set_colormode(&mut self, value: i32) {
        self.set_color_mode(value);
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn get_tgt_obj_name(&self) -> &str {
        self.target()
    }

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
    }

    pub fn set_tgt_obj_name(&mut self, value: impl Into<String>) {
        self.set_target(value);
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, value: Selector) {
        self.selection = value;
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn get_detail(&self) -> i32 {
        self.detail()
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn probe_radius(&self) -> f64 {
        self.probe_radius
    }

    pub fn get_probe_radius(&self) -> f64 {
        self.probe_radius()
    }

    pub fn proberad(&self) -> f64 {
        self.probe_radius()
    }

    pub fn set_probe_radius(&mut self, value: f64) {
        self.probe_radius = value;
    }

    pub fn set_proberad(&mut self, value: f64) {
        self.set_probe_radius(value);
    }

    pub fn surface_type(&self) -> i32 {
        self.surface_type
    }

    pub fn get_surf_type(&self) -> i32 {
        self.surface_type()
    }

    pub fn surftype(&self) -> i32 {
        self.surface_type()
    }

    pub fn set_surface_type(&mut self, value: i32) {
        self.surface_type = value;
    }

    pub fn set_surf_type(&mut self, value: i32) {
        self.set_surface_type(value);
    }

    pub fn set_surftype(&mut self, value: i32) {
        self.set_surface_type(value);
    }

    pub fn surface_algorithm(&self) -> i32 {
        self.surface_algorithm
    }

    pub fn get_surf_algor(&self) -> i32 {
        self.surface_algorithm()
    }

    pub fn surfalgor(&self) -> i32 {
        self.surface_algorithm()
    }

    pub fn set_surface_algorithm(&mut self, value: i32) {
        self.surface_algorithm = value;
    }

    pub fn set_surf_algor(&mut self, value: i32) {
        self.set_surface_algorithm(value);
    }

    pub fn set_surfalgor(&mut self, value: i32) {
        self.set_surface_algorithm(value);
    }

    pub fn cull_face(&self) -> bool {
        self.cull_face
    }

    pub fn cullface(&self) -> bool {
        self.cull_face()
    }

    pub fn is_cull_face(&self) -> bool {
        self.cull_face()
    }

    pub fn set_cull_face(&mut self, value: bool) {
        self.cull_face = value;
    }

    pub fn set_cullface(&mut self, value: bool) {
        self.set_cull_face(value);
    }

    pub fn draw_mode(&self) -> i32 {
        self.draw_mode
    }

    pub fn drawmode(&self) -> i32 {
        self.draw_mode()
    }

    pub fn get_draw_mode(&self) -> i32 {
        self.draw_mode()
    }

    pub fn set_draw_mode(&mut self, value: i32) {
        self.draw_mode = value;
    }

    pub fn set_drawmode(&mut self, value: i32) {
        self.set_draw_mode(value);
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_line_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.set_width(value);
    }

    pub fn show_selection(&self) -> &str {
        &self.show_selection
    }

    pub fn get_show_sel(&self) -> &str {
        self.show_selection()
    }

    pub fn showsel(&self) -> &str {
        self.show_selection()
    }

    pub fn set_show_selection(&mut self, value: impl Into<String>) {
        self.show_selection = value.into();
    }

    pub fn set_show_sel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }

    pub fn set_showsel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }

    pub fn vdw_radius(&self, element: &str) -> f64 {
        self.vdw_radii.radius_for_element(element)
    }

    pub fn vdwr_h(&self) -> f64 {
        self.vdw_radii.h()
    }

    pub fn set_vdwr_h(&mut self, value: f64) {
        self.vdw_radii.set_h(value);
    }

    pub fn vdwr_c(&self) -> f64 {
        self.vdw_radii.c()
    }

    pub fn set_vdwr_c(&mut self, value: f64) {
        self.vdw_radii.set_c(value);
    }

    pub fn vdwr_n(&self) -> f64 {
        self.vdw_radii.n()
    }

    pub fn set_vdwr_n(&mut self, value: f64) {
        self.vdw_radii.set_n(value);
    }

    pub fn vdwr_o(&self) -> f64 {
        self.vdw_radii.o()
    }

    pub fn set_vdwr_o(&mut self, value: f64) {
        self.vdw_radii.set_o(value);
    }

    pub fn vdwr_s(&self) -> f64 {
        self.vdw_radii.s()
    }

    pub fn set_vdwr_s(&mut self, value: f64) {
        self.vdw_radii.set_s(value);
    }

    pub fn vdwr_p(&self) -> f64 {
        self.vdw_radii.p()
    }

    pub fn set_vdwr_p(&mut self, value: f64) {
        self.vdw_radii.set_p(value);
    }

    pub fn vdwr_x(&self) -> f64 {
        self.vdw_radii.x()
    }

    pub fn set_vdwr_x(&mut self, value: f64) {
        self.vdw_radii.set_x(value);
    }

    pub fn vdw_radii(&self) -> &SurfaceVdwRadii {
        &self.vdw_radii
    }

    pub fn vdw_radii_mut(&mut self) -> &mut SurfaceVdwRadii {
        &mut self.vdw_radii
    }

    pub fn set_vdw_radius(&mut self, element: &str, value: f64) {
        match element.trim().to_ascii_uppercase().as_str() {
            "H" => self.vdw_radii.set_h(value),
            "C" => self.vdw_radii.set_c(value),
            "N" => self.vdw_radii.set_n(value),
            "O" => self.vdw_radii.set_o(value),
            "S" => self.vdw_radii.set_s(value),
            "P" => self.vdw_radii.set_p(value),
            _ => self.vdw_radii.set_x(value),
        }
    }

    pub fn elepot(&self) -> &str {
        &self.elepot
    }

    pub fn get_tgt_ele_pot_name(&self) -> &str {
        self.elepot()
    }

    pub fn set_elepot(&mut self, value: impl Into<String>) {
        self.elepot = value.into();
    }

    pub fn set_tgt_ele_pot_name(&mut self, value: impl Into<String>) {
        self.set_elepot(value);
    }

    pub fn low_param(&self) -> f64 {
        self.low_param
    }

    pub fn get_low_par(&self) -> f64 {
        self.low_param()
    }

    pub fn lowpar(&self) -> f64 {
        self.low_param()
    }

    pub fn set_low_param(&mut self, value: f64) {
        self.low_param = value;
    }

    pub fn set_low_par(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn set_lowpar(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn mid_param(&self) -> f64 {
        self.mid_param
    }

    pub fn get_mid_par(&self) -> f64 {
        self.mid_param()
    }

    pub fn midpar(&self) -> f64 {
        self.mid_param()
    }

    pub fn set_mid_param(&mut self, value: f64) {
        self.mid_param = value;
    }

    pub fn set_mid_par(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn set_midpar(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn high_param(&self) -> f64 {
        self.high_param
    }

    pub fn get_high_par(&self) -> f64 {
        self.high_param()
    }

    pub fn highpar(&self) -> f64 {
        self.high_param()
    }

    pub fn set_high_param(&mut self, value: f64) {
        self.high_param = value;
    }

    pub fn set_high_par(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn set_highpar(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn high_color(&self) -> &SolidColor {
        &self.high_color
    }

    pub fn get_high_col(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn highcol(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn set_high_color(&mut self, value: SolidColor) {
        self.high_color = value;
    }

    pub fn set_high_col(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn set_highcol(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn mid_color(&self) -> &SolidColor {
        &self.mid_color
    }

    pub fn get_mid_col(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn midcol(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn set_mid_color(&mut self, value: SolidColor) {
        self.mid_color = value;
    }

    pub fn set_mid_col(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn set_midcol(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn low_color(&self) -> &SolidColor {
        &self.low_color
    }

    pub fn get_low_col(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn lowcol(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn set_low_color(&mut self, value: SolidColor) {
        self.low_color = value;
    }

    pub fn set_low_col(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn set_lowcol(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn ramp_above(&self) -> bool {
        self.ramp_above
    }

    pub fn is_ramp_above(&self) -> bool {
        self.ramp_above()
    }

    pub fn set_ramp_above(&mut self, value: bool) {
        self.ramp_above = value;
    }

    pub fn ramp_value(&self) -> f64 {
        self.ramp_value
    }

    pub fn get_ramp_value(&self) -> f64 {
        self.ramp_value()
    }

    pub fn set_ramp_value(&mut self, value: f64) {
        self.ramp_value = value;
    }

    pub fn grid_spacing(&self) -> f64 {
        (direct_surface::voxel_size(self.detail) * 1.43).max(0.15)
    }

    pub fn render_mesh(&self, molecule: &MolCoord) -> SurfaceMesh {
        direct_surface::render(self, molecule)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DirectSurfRenderer2 {
    inner: DirectSurfRenderer,
}

impl DirectSurfRenderer2 {
    pub const DS_SCAPOT: i32 = DirectSurfRenderer::DS_SCAPOT;
    pub const DS_MOLFANC: i32 = DirectSurfRenderer::DS_MOLFANC;
    pub const DS_VDW: i32 = DirectSurfRenderer::DS_VDW;
    pub const DS_SAS: i32 = DirectSurfRenderer::DS_SAS;
    pub const DS_SES: i32 = DirectSurfRenderer::DS_SES;
    pub const SFDRAW_FILL: i32 = DirectSurfRenderer::SFDRAW_FILL;
    pub const SFDRAW_LINE: i32 = DirectSurfRenderer::SFDRAW_LINE;
    pub const SFDRAW_POINT: i32 = DirectSurfRenderer::SFDRAW_POINT;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "dsurf2"
    }

    pub fn as_renderer(&self) -> &DirectSurfRenderer {
        &self.inner
    }

    pub fn as_renderer_mut(&mut self) -> &mut DirectSurfRenderer {
        &mut self.inner
    }

    pub fn into_renderer(self) -> DirectSurfRenderer {
        self.inner
    }

    /// Compatibility alias for the distance-field algorithm.
    pub fn render_mesh(&self, molecule: &MolCoord) -> SurfaceMesh {
        let mut renderer = self.inner.clone();
        renderer.set_surface_algorithm(DirectSurfRenderer::DS_DISTFIELD);
        renderer.render_mesh(molecule)
    }
}

impl std::ops::Deref for DirectSurfRenderer2 {
    type Target = DirectSurfRenderer;

    fn deref(&self) -> &Self::Target {
        self.as_renderer()
    }
}

impl std::ops::DerefMut for DirectSurfRenderer2 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_renderer_mut()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ms2TestRenderer;

impl Ms2TestRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn type_name(&self) -> &'static str {
        "ms2test"
    }

    pub fn render_mesh(&self, molecule: &MolCoord) -> SurfaceMesh {
        let spheres = molecule
            .atoms()
            .iter()
            .map(|atom| grid_surface::SurfSphere {
                center: atom.position,
                radius: 3.0,
            })
            .collect::<Vec<_>>();
        let mut mesh = SurfaceMesh::new();
        grid_surface::build_sphere_union_surface(&mut mesh, &spheres, 0.2);
        mesh
    }

    pub fn center(&self) -> Vector4 {
        Vector4::ZERO
    }

    pub fn get_center(&self) -> Vector4 {
        self.center()
    }

    pub fn is_hit_test_supported(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElePotMap {
    counts: [usize; 3],
    grid_size: [f64; 3],
    origin: Vector4,
    values: Vec<f64>,
    min_value: f64,
    max_value: f64,
    mean_value: f64,
    rmsd_value: f64,
    level_base: f64,
    level_step: f64,
}

impl Default for ElePotMap {
    fn default() -> Self {
        Self {
            counts: [0, 0, 0],
            grid_size: [1.0, 1.0, 1.0],
            origin: Vector4::ZERO,
            values: Vec::new(),
            min_value: 0.0,
            max_value: 0.0,
            mean_value: 0.0,
            rmsd_value: 0.0,
            level_base: 0.0,
            level_step: 0.0,
        }
    }
}

impl ElePotMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "elepot"
    }

    pub fn fit_view(&self, _view: &mut View, _dummy: bool) {}

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn col_no(&self) -> usize {
        self.counts[0]
    }

    pub fn row_no(&self) -> usize {
        self.counts[1]
    }

    pub fn sec_no(&self) -> usize {
        self.counts[2]
    }

    pub fn col_grid_size(&self) -> f64 {
        self.grid_size[0]
    }

    pub fn row_grid_size(&self) -> f64 {
        self.grid_size[1]
    }

    pub fn sec_grid_size(&self) -> f64 {
        self.grid_size[2]
    }

    pub fn origin(&self) -> Vector4 {
        self.origin
    }

    pub fn get_origin(&self) -> Vector4 {
        self.origin()
    }

    pub fn center(&self) -> Vector4 {
        if self.is_empty() {
            return Vector4::ZERO;
        }
        self.conv_to_orth(Vector4::new3(
            self.counts[0] as f64 / 2.0,
            self.counts[1] as f64 / 2.0,
            self.counts[2] as f64 / 2.0,
        ))
    }

    pub fn get_center(&self) -> Vector4 {
        self.center()
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    pub fn set_map_float_array(
        &mut self,
        values: &[f64],
        counts: [usize; 3],
        grid_size: [f64; 3],
        origin: Vector4,
    ) {
        self.counts = counts;
        self.grid_size = grid_size;
        self.origin = origin;
        let expected = counts[0]
            .saturating_mul(counts[1])
            .saturating_mul(counts[2]);
        self.values = values.to_vec();
        self.values.resize(expected, 0.0);
        self.values.truncate(expected);
        self.update_statistics();
    }

    pub fn at_float(&self, col: usize, row: usize, sec: usize) -> f64 {
        if col >= self.counts[0] || row >= self.counts[1] || sec >= self.counts[2] {
            return 0.0;
        }
        let index = col + self.counts[0] * (row + self.counts[1] * sec);
        self.values.get(index).copied().unwrap_or(0.0)
    }

    pub fn is_in_boundary(&self, col: isize, row: isize, sec: isize) -> bool {
        col >= 0
            && row >= 0
            && sec >= 0
            && (col as usize) < self.counts[0]
            && (row as usize) < self.counts[1]
            && (sec as usize) < self.counts[2]
    }

    pub fn at_byte(&self, col: usize, row: usize, sec: usize) -> u8 {
        if self.level_step <= 0.0 {
            return 0;
        }
        ((self.at_float(col, row, sec) - self.level_base) / self.level_step).clamp(0.0, 255.0) as u8
    }

    pub fn min_density(&self) -> f64 {
        self.min_value
    }

    pub fn get_min_density(&self) -> f64 {
        self.min_density()
    }

    pub fn max_density(&self) -> f64 {
        self.max_value
    }

    pub fn get_max_density(&self) -> f64 {
        self.max_density()
    }

    pub fn mean_density(&self) -> f64 {
        self.mean_value
    }

    pub fn get_mean_density(&self) -> f64 {
        self.mean_density()
    }

    pub fn rmsd_density(&self) -> f64 {
        self.rmsd_value
    }

    pub fn get_rmsd_density(&self) -> f64 {
        self.rmsd_density()
    }

    pub fn level_base(&self) -> f64 {
        self.level_base
    }

    pub fn get_level_base(&self) -> f64 {
        self.level_base()
    }

    pub fn level_step(&self) -> f64 {
        self.level_step
    }

    pub fn get_level_step(&self) -> f64 {
        self.level_step()
    }

    pub fn start_col(&self) -> usize {
        0
    }

    pub fn start_row(&self) -> usize {
        0
    }

    pub fn start_sec(&self) -> usize {
        0
    }

    pub fn conv_to_orth(&self, index: Vector4) -> Vector4 {
        Vector4::new3(
            self.origin.x + index.x * self.grid_size[0],
            self.origin.y + index.y * self.grid_size[1],
            self.origin.z + index.z * self.grid_size[2],
        )
    }

    pub fn is_in_range(&self, position: Vector4) -> bool {
        if self.is_empty()
            || self
                .grid_size
                .iter()
                .any(|value| value.abs() <= f64::EPSILON)
        {
            return false;
        }
        let relative = position - self.origin;
        let grid = [
            relative.x / self.grid_size[0],
            relative.y / self.grid_size[1],
            relative.z / self.grid_size[2],
        ];
        grid.iter()
            .zip(self.counts)
            .all(|(value, count)| *value >= 0.0 && *value <= count as f64)
    }

    pub fn value_at(&self, position: Vector4) -> f64 {
        if self.counts.iter().any(|count| *count < 2)
            || self
                .grid_size
                .iter()
                .any(|value| value.abs() <= f64::EPSILON)
        {
            return 0.0;
        }
        let relative = position - self.origin;
        let coordinate = [
            relative.x / self.grid_size[0],
            relative.y / self.grid_size[1],
            relative.z / self.grid_size[2],
        ];
        let base = [
            coordinate[0] as isize,
            coordinate[1] as isize,
            coordinate[2] as isize,
        ];
        if base
            .iter()
            .zip(self.counts)
            .any(|(index, count)| *index < 0 || *index >= count as isize - 1)
        {
            return 0.0;
        }
        let [x, y, z] = [base[0] as usize, base[1] as usize, base[2] as usize];
        let [tx, ty, tz] = [
            coordinate[0] - x as f64,
            coordinate[1] - y as f64,
            coordinate[2] - z as f64,
        ];
        let x00 = (1.0 - tx) * self.at_float(x, y, z) + tx * self.at_float(x + 1, y, z);
        let x10 = (1.0 - tx) * self.at_float(x, y + 1, z) + tx * self.at_float(x + 1, y + 1, z);
        let x01 = (1.0 - tx) * self.at_float(x, y, z + 1) + tx * self.at_float(x + 1, y, z + 1);
        let x11 =
            (1.0 - tx) * self.at_float(x, y + 1, z + 1) + tx * self.at_float(x + 1, y + 1, z + 1);
        let y0 = (1.0 - ty) * x00 + ty * x10;
        let y1 = (1.0 - ty) * x01 + ty * x11;
        (1.0 - tz) * y0 + tz * y1
    }

    pub fn get_value_at(&self, position: Vector4) -> f64 {
        self.value_at(position)
    }

    pub fn smooth(&mut self, radius: f64) {
        if self.is_empty() || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        let limits = self.smoothing_limits(radius);
        let mut offsets = Vec::new();
        for x in -limits[0]..=limits[0] {
            for y in -limits[1]..=limits[1] {
                for z in -limits[2]..=limits[2] {
                    let distance = Vector4::new3(
                        x as f64 * self.grid_size[0],
                        y as f64 * self.grid_size[1],
                        z as f64 * self.grid_size[2],
                    )
                    .length3d();
                    if distance < radius {
                        offsets.push([x, y, z]);
                    }
                }
            }
        }
        if offsets.is_empty() {
            return;
        }
        let mut smoothed = vec![0.0; self.values.len()];
        for z in 0..self.counts[2] {
            for y in 0..self.counts[1] {
                for x in 0..self.counts[0] {
                    let sum = offsets
                        .iter()
                        .filter_map(|offset| {
                            self.signed_value(
                                x as isize + offset[0],
                                y as isize + offset[1],
                                z as isize + offset[2],
                            )
                        })
                        .sum::<f64>();
                    smoothed[self.flat_index(x, y, z)] = sum / offsets.len() as f64;
                }
            }
        }
        self.values = smoothed;
    }

    pub fn smooth2(&mut self, radius: f64) {
        if self.is_empty() || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        let limits = self.smoothing_limits(radius);
        let mut source = self.values.clone();
        let mut target = vec![0.0; self.values.len()];
        for axis in 0..3 {
            let limit = limits[axis];
            let denominator = (limit * 2 + 1) as f64;
            for z in 0..self.counts[2] {
                for y in 0..self.counts[1] {
                    for x in 0..self.counts[0] {
                        let coordinates = [x, y, z];
                        let mut sum = 0.0;
                        for offset in -limit..limit {
                            let mut sample = [x as isize, y as isize, z as isize];
                            sample[axis] = coordinates[axis] as isize + offset;
                            if self.is_in_boundary(sample[0], sample[1], sample[2]) {
                                sum += source[self.flat_index(
                                    sample[0] as usize,
                                    sample[1] as usize,
                                    sample[2] as usize,
                                )];
                            }
                        }
                        target[self.flat_index(x, y, z)] = sum / denominator;
                    }
                }
            }
            std::mem::swap(&mut source, &mut target);
            target.fill(0.0);
        }
        self.values = source;
    }

    fn smoothing_limits(&self, radius: f64) -> [isize; 3] {
        self.grid_size.map(|spacing| {
            if spacing.abs() <= f64::EPSILON {
                0
            } else {
                (radius / spacing.abs()).floor() as isize
            }
        })
    }

    fn signed_value(&self, col: isize, row: isize, sec: isize) -> Option<f64> {
        self.is_in_boundary(col, row, sec)
            .then(|| self.at_float(col as usize, row as usize, sec as usize))
    }

    fn flat_index(&self, col: usize, row: usize, sec: usize) -> usize {
        col + self.counts[0] * (row + self.counts[1] * sec)
    }

    fn update_statistics(&mut self) {
        if self.values.is_empty() {
            self.min_value = 0.0;
            self.max_value = 0.0;
            self.mean_value = 0.0;
            self.rmsd_value = 0.0;
            self.level_base = 0.0;
            self.level_step = 0.0;
            return;
        }
        self.min_value = self.values.iter().copied().fold(f64::INFINITY, f64::min);
        self.max_value = self
            .values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        self.mean_value = self.values.iter().sum::<f64>() / self.values.len() as f64;
        let square_mean =
            self.values.iter().map(|value| value * value).sum::<f64>() / self.values.len() as f64;
        self.rmsd_value = (square_mean - self.mean_value * self.mean_value)
            .max(0.0)
            .sqrt();
        self.level_base = self.min_value;
        self.level_step = (self.max_value - self.min_value) / 256.0;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoleSurfBuilder {
    target_mol: String,
    target_sel: String,
    target_molecule: Option<MolCoord>,
    target_selection: Option<Selector>,
    dir_norm: Vector4,
    start_pos: Vector4,
    surf_den: f64,
    surf_probe_r: f64,
    path: Vec<HolePathPoint>,
    result: Option<MolSurfObj>,
}

impl Default for HoleSurfBuilder {
    fn default() -> Self {
        Self {
            target_mol: String::new(),
            target_sel: String::new(),
            target_molecule: None,
            target_selection: None,
            dir_norm: Vector4::new3(0.0, 0.0, 1.0),
            start_pos: Vector4::new3(0.0, 0.0, 0.0),
            surf_den: 2.0,
            surf_probe_r: 1.4,
            path: Vec::new(),
            result: None,
        }
    }
}

impl HoleSurfBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn target_mol(&self) -> &str {
        &self.target_mol
    }

    pub fn target_mol_name(&self) -> &str {
        self.target_mol()
    }

    pub fn target_molecule(&self) -> Option<&MolCoord> {
        self.target_molecule.as_ref()
    }

    pub fn get_target_mol(&self) -> Option<&MolCoord> {
        self.target_molecule()
    }

    pub fn set_target_mol<'a, M>(&mut self, value: M)
    where
        M: IntoSurfaceMoleculeSource<'a>,
    {
        let source = value.into_surface_molecule_source();
        self.target_mol = source.source_name();
        self.target_molecule = match source {
            SurfaceMoleculeSource::Name(_) => None,
            SurfaceMoleculeSource::Molecule(molecule) => Some(molecule.clone()),
        };
        self.result = None;
        self.path.clear();
    }

    pub fn target_sel(&self) -> &str {
        &self.target_sel
    }

    pub fn target_sel_text(&self) -> &str {
        self.target_sel()
    }

    pub fn target_selection(&self) -> Option<&Selector> {
        self.target_selection.as_ref()
    }

    pub fn get_target_sel(&self) -> Option<&Selector> {
        self.target_selection()
    }

    pub fn set_target_sel<'a, S>(&mut self, value: S)
    where
        S: IntoSurfaceSelectionSource<'a>,
    {
        let source = value.into_surface_selection_source();
        self.target_sel = source.source_text();
        self.target_selection = source.selector();
        self.result = None;
        self.path.clear();
    }

    pub fn dir_norm(&self) -> Vector4 {
        self.dir_norm
    }

    pub fn get_dir_norm(&self) -> Vector4 {
        self.dir_norm()
    }

    pub fn dirnorm(&self) -> Vector4 {
        self.dir_norm()
    }

    pub fn set_dir_norm(&mut self, value: Vector4) {
        self.dir_norm = value;
    }

    pub fn set_dirnorm(&mut self, value: Vector4) {
        self.set_dir_norm(value);
    }

    pub fn start_pos(&self) -> Vector4 {
        self.start_pos
    }

    pub fn get_start_pos(&self) -> Vector4 {
        self.start_pos()
    }

    pub fn startpos(&self) -> Vector4 {
        self.start_pos()
    }

    pub fn set_start_pos(&mut self, value: Vector4) {
        self.start_pos = value;
    }

    pub fn set_startpos(&mut self, value: Vector4) {
        self.set_start_pos(value);
    }

    pub fn surf_den(&self) -> f64 {
        self.surf_den
    }

    pub fn get_surf_den(&self) -> f64 {
        self.surf_den()
    }

    pub fn set_surf_den(&mut self, value: f64) {
        self.surf_den = value;
    }

    pub fn surf_probe_r(&self) -> f64 {
        self.surf_probe_r
    }

    pub fn get_surf_probe_r(&self) -> f64 {
        self.surf_probe_r()
    }

    pub fn set_surf_probe_r(&mut self, value: f64) {
        self.surf_probe_r = value;
    }

    pub fn path(&self) -> &[HolePathPoint] {
        &self.path
    }

    pub fn result(&self) -> Option<&MolSurfObj> {
        self.result.as_ref()
    }

    pub fn get_result(&self) -> Option<&MolSurfObj> {
        self.result()
    }

    pub fn doit(&mut self) -> cuemol_core::Result<()> {
        let molecule = self.target_molecule.as_ref().ok_or_else(|| {
            cuemol_core::CueMolError::undefined_reference(
                "hole surface builder requires a resolved target molecule",
            )
        })?;
        let selector = if let Some(selector) = &self.target_selection {
            selector.clone()
        } else if self.target_sel.trim().is_empty() {
            Selector::All
        } else {
            parse_selector(&self.target_sel)?
        };
        let path =
            hole_surface::trace_hole_path(molecule, &selector, self.dir_norm, self.start_pos)?;
        let mut result = MolSurfObj::new();
        hole_surface::build_hole_ses(result.mesh_mut(), &path, self.surf_den, self.surf_probe_r)?;
        result.orig_mol = self.target_mol.clone();
        result.orig_sel = self.target_sel.clone();
        result.orig_selector = Some(selector);
        result.orig_den = self.surf_den;
        result.orig_prad = self.surf_probe_r;
        self.path = path;
        self.result = Some(result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;
    use std::collections::BTreeMap;

    #[test]
    fn molsurf_obj_records_source_and_cut_planes() {
        let mut obj = MolSurfObj::new();
        obj.create_ses_from_mol("mol1", "protein", 3.0, 1.4)
            .unwrap();
        assert_eq!(obj.orig_mol(), "mol1");
        assert_eq!(obj.orig_sel(), "protein");
        assert!(obj.get_orig_sel().is_some());
        assert_eq!(obj.orig_den(), 3.0);
        assert_eq!(obj.orig_prad(), 1.4);
        obj.cut_by_plane(
            0.5,
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::new3(1.0, 2.0, 3.0),
            true,
        );
        assert_eq!(obj.cut_planes().len(), 1);
        assert!(obj.cut_planes()[0].section);
    }

    #[test]
    fn analytic_mol_surface_lone_atom_is_the_vdw_sphere() {
        let mut molecule = MolCoord::new();
        // A backbone CA in ALA: the C++ production surface uses the UNITED-ATOM
        // radius (getVdwRadius bExplH=false), so this CA's vdW is the CH1 type's
        // vdwhr = 1.95 (from default_params.xml), NOT the bare element 1.70. This
        // exercises the full build path's topology->ParamDB radius resolution.
        molecule.push_atom(surface_atom(1, "CA", "C", 4.0));
        let mesh =
            build_analytic_mol_surface(&molecule, &parse_selector("name CA").unwrap(), 1.4, 6);
        assert!(mesh.triangle_count() > 50);
        let center = cuemol_core::Vector4::new3(4.0, 0.0, 0.0);
        for vtx in mesh.vertices() {
            assert!(
                ((vtx.position - center).length3d() - 1.95).abs() < 1e-9,
                "lone-atom analytic SES vertex on the united-atom (CH1, 1.95) vdW sphere"
            );
        }
    }

    #[test]
    fn analytic_mol_surface_two_atoms_adds_a_reentrant_saddle() {
        let mut lone = MolCoord::new();
        lone.push_atom(surface_atom(1, "C", "C", 0.0));
        let sel = parse_selector("*").unwrap();
        let lone_mesh = build_analytic_mol_surface(&lone, &sel, 1.4, 6);

        let mut pair = MolCoord::new();
        pair.push_atom(surface_atom(1, "C", "C", 0.0));
        pair.push_atom(surface_atom(2, "C", "C", 2.5));
        let pair_mesh = build_analytic_mol_surface(&pair, &sel, 1.4, 6);

        // Two nearby atoms produce a real surface; it is not just two lone spheres
        // (the facing caps are clipped and the saddle torus is added).
        assert!(pair_mesh.triangle_count() > 0);
        assert!(pair_mesh.triangle_count() != 2 * lone_mesh.triangle_count());
    }

    #[test]
    fn molsurf_regeneration_rebuilds_geometry_from_the_original_molecule() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "CA", "C", 0.0));
        let mut surface = MolSurfObj::new();
        surface
            .create_ses_from_mol(&molecule, "name CA", 2.0, 1.4)
            .unwrap();
        let coarse_triangles = surface.mesh().triangle_count();

        surface.regenerate_ses(20.0).unwrap();

        assert_eq!(surface.orig_den(), 20.0);
        assert_eq!(surface.orig_prad(), 1.4);
        assert_eq!(surface.orig_sel(), "name CA");
        assert!(surface.mesh().triangle_count() > coarse_triangles);

        let mut detached = MolSurfObj::new();
        assert!(detached.regenerate_ses(2.0).is_err());
    }

    #[test]
    fn molsurf_backend_controls_creation_arrays_and_regeneration() {
        use backends::SesBackend;
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "CA", "C", 0.0));
        molecule.push_atom(surface_atom(2, "CB", "C", 2.5));
        let mut surface = MolSurfObj::new();
        assert_eq!(surface.ses_backend(), SesBackend::Auto);
        assert_eq!(surface.ses_backend_used(), None);
        surface.set_ses_backend(SesBackend::MeshMs);
        surface
            .create_ses_from_mol(&molecule, "*", 2.0, 1.4)
            .unwrap();
        assert_eq!(surface.ses_backend_used(), Some(SesBackend::MeshMs));
        let meshms = surface.mesh().clone();
        surface.set_ses_backend(SesBackend::Ball);
        assert_eq!(surface.mesh(), &meshms);
        surface.regenerate_ses(-1.0).unwrap();
        assert_eq!(surface.ses_backend_used(), Some(SesBackend::Ball));
        assert_ne!(surface.mesh(), &meshms);
        let (expected, _) = backends::build_molecular_surface(
            &molecule,
            &Selector::All,
            2.0,
            1.4,
            SesBackend::Ball,
        )
        .unwrap();
        assert_eq!(surface.mesh(), &expected);
        let spheres = molecule
            .atoms()
            .iter()
            .map(|atom| SurfSphere {
                center: atom.position,
                radius: cuemol_molstr::toppar::vdw_radius_for_atom(atom),
            })
            .collect::<Vec<_>>();
        surface.create_ses_from_array(&spheres, 2.0, 1.4).unwrap();
        assert_eq!(surface.mesh(), &expected);
        let saved = surface.clone();
        assert!(surface
            .create_ses_from_mol(&molecule, "aid 999", 2.0, 1.4)
            .is_err());
        assert_eq!(surface, saved);
        assert!(surface.create_ses_from_array(&[], 2.0, 1.4).is_err());
        assert_eq!(surface, saved);
    }

    #[test]
    fn molsurf_plane_cut_undo_and_redo_swap_the_complete_structure() {
        let mut surface = MolSurfObj::new();
        *surface.mesh_mut() = cube_mesh(1.0);
        let target = Rc::new(RefCell::new(surface));
        let undo_manager = UndoManagerHandle::new();
        let before = target.borrow().mesh().clone();

        undo_manager.with_mut(|manager| manager.start_txn("cut surface"));
        assert!(cut_surface_by_plane_with_undo(
            &target,
            &undo_manager,
            1.0,
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::ZERO,
            true,
            true,
        ));
        assert!(undo_manager.with_mut(|manager| manager.commit_txn()));
        let after = target.borrow().mesh().clone();
        assert_ne!(after, before);
        assert_eq!(target.borrow().cut_planes().len(), 1);

        assert!(undo_manager.with_mut(|manager| manager.undo()));
        assert_eq!(target.borrow().mesh(), &before);
        assert!(target.borrow().cut_planes().is_empty());

        assert!(undo_manager.with_mut(|manager| manager.redo()));
        assert_eq!(target.borrow().mesh(), &after);
        assert_eq!(target.borrow().cut_planes().len(), 1);
    }

    #[test]
    fn molsurf_regeneration_participates_in_scene_undo_transactions() {
        let mut original = MolCoord::new();
        original.push_atom(surface_atom(1, "CA", "C", 0.0));
        let mut surface = MolSurfObj::new();
        surface
            .create_ses_from_mol(&original, "*", 2.0, 1.4)
            .unwrap();
        let target = Rc::new(RefCell::new(surface));
        let undo_manager = UndoManagerHandle::new();
        let before = target.borrow().mesh().clone();

        let mut changed = original.clone();
        changed.push_atom(surface_atom(2, "CB", "C", 3.0));
        undo_manager.with_mut(|manager| manager.start_txn("regenerate surface"));
        assert!(
            regenerate_surface_with_undo(&target, &undo_manager, &changed, None, 3.0, -1.0,)
                .unwrap()
        );
        assert!(undo_manager.with_mut(|manager| manager.commit_txn()));
        let after = target.borrow().mesh().clone();
        assert_ne!(after, before);

        assert!(undo_manager.with_mut(|manager| manager.undo()));
        assert_eq!(target.borrow().mesh(), &before);
        assert!(undo_manager.with_mut(|manager| manager.redo()));
        assert_eq!(target.borrow().mesh(), &after);
    }

    #[test]
    fn molsurf_obj_builds_selected_atom_mesh_from_molecule() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "N", "N", 0.0));
        molecule.push_atom(surface_atom(2, "CA", "C", 4.0));
        let selector = parse_selector("name CA").unwrap();

        let mut obj = MolSurfObj::new();
        obj.create_ses_from_mol(&molecule, &selector, 2.0, 1.4)
            .unwrap();

        assert_eq!(obj.orig_mol(), "MolCoord");
        assert_eq!(obj.orig_sel(), "name CA");
        // Only the selected CA atom contributes. This is now the real SES, so a
        // lone convex atom is its bare vdW sphere (C vdw 1.7, centred at x = 4) —
        // NOT the vdw+probe (3.1) accessible sphere. Every vertex lies on r = 1.7
        // and the +x extent is ~5.7.
        assert!(obj.mesh().triangle_count() > 50);
        for vertex in obj.mesh().vertices() {
            let dx = vertex.position.x - 4.0;
            let r = (dx * dx
                + vertex.position.y * vertex.position.y
                + vertex.position.z * vertex.position.z)
                .sqrt();
            assert!((r - 1.7).abs() < 0.6, "vertex off CA vdW sphere: r={r}");
        }
        let max_x = obj
            .mesh()
            .vertices()
            .iter()
            .map(|vertex| vertex.position.x)
            .fold(f64::NEG_INFINITY, f64::max);
        // ~5.7 (= 4 + vdw 1.7) within grid tolerance, and far below the 7.1 the
        // accessible (vdw+probe) sphere would reach.
        assert!(max_x > 5.0 && max_x < 6.2, "max_x {max_x}");
    }

    #[test]
    fn surface_mesh_writes_obj_and_cuts_by_plane() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "N", "N", -4.0));
        molecule.push_atom(surface_atom(2, "CA", "C", 4.0));
        let mut obj = MolSurfObj::new();
        obj.create_ses_from_mol(&molecule, "all", 2.0, 1.4).unwrap();
        let before = obj.mesh().triangle_count();
        assert!(before > 0);

        obj.cut_by_plane(
            0.5,
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(0.0, 0.0, 0.0),
            false,
        );
        assert!(obj.mesh().triangle_count() < before);
        assert_eq!(obj.cut_planes().len(), 1);

        let output = obj.mesh().to_obj_string("cut surface");
        assert!(output.contains("o cut_surface"));
        assert!(output.contains("\nv "));
        assert!(output.contains("\nf "));
    }

    #[test]
    fn plane_cut_splits_crossing_triangles_at_the_plane() {
        let mut mesh = cube_mesh(1.0);
        mesh.cut_by_plane_detailed(
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::ZERO,
            true,
            true,
            false,
        );

        assert!(mesh.vertex_count() > 0);
        assert!(mesh
            .vertices()
            .iter()
            .all(|vertex| vertex.position.x >= -1.0e-9));
        assert!(mesh
            .vertices()
            .iter()
            .any(|vertex| vertex.position.x.abs() < 1.0e-9));
        assert!(mesh
            .vertices()
            .iter()
            .any(|vertex| (vertex.position.x - 1.0).abs() < 1.0e-9));
    }

    #[test]
    fn plane_cut_builds_an_outward_facing_section() {
        let mut mesh = cube_mesh(1.0);
        mesh.cut_by_plane_detailed(
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::ZERO,
            true,
            true,
            true,
        );

        let cap_area = section_area(&mesh, Vector4::new3(-1.0, 0.0, 0.0));
        assert!((cap_area - 4.0).abs() < 1.0e-5, "cap area {cap_area}");
    }

    #[test]
    fn section_only_cut_honours_nested_contours_as_holes() {
        let mut mesh = cube_mesh(2.0);
        append_mesh(&mut mesh, cube_mesh(1.0));
        mesh.cut_by_plane_detailed(
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::ZERO,
            true,
            false,
            true,
        );

        assert!(mesh
            .vertices()
            .iter()
            .all(|vertex| vertex.position.x.abs() < 1.0e-9));
        let area = section_area(&mesh, Vector4::new3(-1.0, 0.0, 0.0));
        assert!((area - 12.0).abs() < 1.0e-5, "annular area {area}");
    }

    #[test]
    fn apbs_input_config_writes_reference_shape() {
        let mut config = ApbsInputConfig::new("mol", "mol.pqr", "mol.dx");
        config.center = Vector4::new3(1.0, 2.0, 3.0);
        let input = config.to_input_string();
        assert!(input.contains("mol pqr mol.pqr"));
        assert!(input.contains("cgcent 1.000 2.000 3.000"));
        assert!(input.contains("write pot dx mol.dx"));
    }

    #[test]
    fn surface_io_metadata_matches_qif_reference() {
        let ply = PlyFileReader::new();
        assert_eq!(ply.handler().name(), "ply");
        assert_eq!(ply.handler().type_descr(), "PLY polygon file (*.ply)");
        assert_eq!(ply.handler().file_ext(), "*.ply");
        assert_eq!(
            ply.can_handle_content(b"ply\nformat ascii 1.0\n"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert!(ply.create_default_obj().is_empty());

        let msms = MsmsFileReader::new();
        assert_eq!(msms.handler().name(), "msms");
        assert_eq!(
            msms.handler().type_descr(),
            "MSMS surface file (*.face/vert)"
        );
        assert_eq!(msms.handler().file_ext(), "*.face");
        assert_eq!(msms.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
        assert_eq!(msms.vertex_file(), "");
        assert_eq!(
            msms.can_handle_content("# MSMS solvent excluded surface\n"),
            cuemol_molstr::ContentVerdict::Yes
        );
        assert!(msms.create_default_obj().is_empty());

        let opendx = OpenDxPotReader::new();
        assert_eq!(opendx.handler().name(), "apbs");
        assert_eq!(opendx.handler().type_descr(), "APBS Potential Map(*.dx)");
        assert_eq!(opendx.handler().file_ext(), "*.dx");
        assert_eq!(opendx.smooth(), -1.0);

        let pqr = PqrFileWriter::new();
        assert_eq!(pqr.handler().name(), "pqr");
        assert_eq!(pqr.handler().type_descr(), "PQR coordinates(*.pqr)");
        assert_eq!(pqr.handler().file_ext(), "*.pqr");
        assert_eq!(pqr.handler().category(), InOutHandler::IOH_CAT_OBJWRITER);
        assert_eq!(pqr.selection(), None);
        assert!(!pqr.use_h());
        assert_eq!(pqr.namespace(), "");

        let xyzr = XyzrFileWriter::new();
        assert_eq!(xyzr.handler().name(), "xyzr");
        assert_eq!(xyzr.handler().type_descr(), "XYZR coordinates(*.xyzr)");
        assert_eq!(xyzr.handler().file_ext(), "*.xyzr");
        assert_eq!(xyzr.selection(), None);

        let qdfsurf = QdfSurfReader::new();
        assert_eq!(qdfsurf.handler().name(), "qdfsurf");
        assert_eq!(
            qdfsurf.handler().type_descr(),
            "CueMol molsurf file (*.qdf)"
        );
        assert_eq!(qdfsurf.handler().file_ext(), "*.qdf");
        assert_eq!(
            qdfsurf.handler().category(),
            InOutHandler::IOH_CAT_OBJREADER
        );

        let qdfpot = QdfPotWriter::new();
        assert_eq!(qdfpot.handler().name(), "qdfpot");
        assert_eq!(qdfpot.handler().type_descr(), "CueMol elepot file (*.qdf)");
        assert_eq!(qdfpot.handler().file_ext(), "*.qdf");
        assert_eq!(qdfpot.handler().category(), InOutHandler::IOH_CAT_OBJWRITER);
    }

    #[test]
    fn opendx_reader_builds_interpolated_electrostatic_map() {
        let input = "\
object 1 class gridpositions counts 2 2 2
origin -1 -2 -3
delta 0.5 0 0
delta 0 1 0
delta 0 0 2
object 2 class gridconnections counts 2 2 2
object 3 class array type double rank 0 items 8 data follows
0 1 2
3 4 5
6 7
attribute \"dep\" string \"positions\"
";
        let reader = OpenDxPotReader::new();
        assert_eq!(
            reader.can_handle_content(input),
            cuemol_molstr::ContentVerdict::Yes
        );
        let map = reader.read_str(input).unwrap();
        assert_eq!([map.col_no(), map.row_no(), map.sec_no()], [2, 2, 2]);
        assert_eq!(map.origin(), Vector4::new3(-1.0, -2.0, -3.0));
        assert_eq!(map.at_float(1, 0, 0), 4.0);
        assert_eq!(map.at_float(0, 0, 1), 1.0);
        assert_eq!(map.min_density(), 0.0);
        assert_eq!(map.max_density(), 7.0);
        assert!((map.mean_density() - 3.5).abs() < 1.0e-12);
        assert!((map.value_at(Vector4::new3(-0.75, -1.5, -2.0)) - 3.5).abs() < 1.0e-12);
        assert_eq!(map.at_byte(1, 1, 1), 255);
        assert!(map.is_in_range(Vector4::new3(-1.0, -2.0, -3.0)));
        assert!(!map.is_in_range(Vector4::new3(-1.1, -2.0, -3.0)));
    }

    #[test]
    fn qdf_surface_roundtrip_matches_reference_schema() {
        let mut obj = MolSurfObj::new();
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(1.0, 2.0, 3.0),
            normal: Vector4::new3(0.0, 0.0, 1.0),
            info: 0,
        });
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(2.0, 3.0, 4.0),
            normal: Vector4::new3(0.0, 1.0, 0.0),
            info: 0,
        });
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(3.0, 4.0, 5.0),
            normal: Vector4::new3(1.0, 0.0, 0.0),
            info: 0,
        });
        obj.mesh_mut().push_triangle([0, 1, 2]);

        let bytes = QdfSurfWriter::new().write_bytes(&obj);
        assert_eq!(&bytes[0..4], b"QDF0");
        assert_eq!(qdf::qdf_file_type(&bytes).as_deref(), Some("SRF1"));

        let parsed = QdfSurfReader::new().read_bytes(&bytes).unwrap();
        assert_eq!(parsed.mesh().vertex_count(), 3);
        assert_eq!(parsed.mesh().triangle_count(), 1);
        assert_eq!(parsed.mesh().triangles()[0], [0, 1, 2]);
        assert_eq!(
            parsed.mesh().vertices()[1].position,
            Vector4::new3(2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn qdf_potential_roundtrip_matches_reference_schema() {
        let mut map = ElePotMap::new();
        map.set_map_float_array(
            &[0.0, 1.0, 2.0, 3.0],
            [2, 1, 2],
            [0.5, 0.6, 0.7],
            Vector4::new3(-1.0, -2.0, -3.0),
        );

        let bytes = QdfPotWriter::new().write_bytes(&map);
        assert_eq!(&bytes[0..4], b"QDF0");
        assert_eq!(qdf::qdf_file_type(&bytes).as_deref(), Some("POT1"));

        let parsed = QdfPotReader::new().read_bytes(&bytes).unwrap();
        assert_eq!(parsed.col_no(), 2);
        assert_eq!(parsed.row_no(), 1);
        assert_eq!(parsed.sec_no(), 2);
        assert!((parsed.col_grid_size() - 0.5).abs() < 1.0e-6);
        assert_eq!(parsed.origin(), Vector4::new3(-1.0, -2.0, -3.0));
        assert_eq!(parsed.at_float(0, 0, 0), 0.0);
        assert_eq!(parsed.at_float(1, 0, 0), 1.0);
        assert_eq!(parsed.at_float(0, 0, 1), 2.0);
        assert_eq!(parsed.at_float(1, 0, 1), 3.0);
    }

    #[test]
    fn pqr_and_xyzr_writers_emit_selected_molecule_atoms() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "N", "N", 0.0));
        molecule.push_atom(surface_atom(2, "CA", "C", 1.0));
        molecule.push_atom(surface_atom(3, "H", "H", 2.0));

        let mut pqr = PqrFileWriter::new();
        pqr.set_selection(Some(parse_selector("name CA,H").unwrap()));
        let no_h = pqr.write_pqr_string(&molecule);
        assert_eq!(
            no_h,
            "ATOM      0   CA ALA A    1   1.000   0.000   0.000  0.10400000   1.950\n"
        );

        pqr.set_use_h(true);
        pqr.set_namespace("ns");
        let with_h = pqr.write_pqr_string(&molecule);
        assert_eq!(with_h.lines().count(), 2);
        assert!(with_h.contains(" CA "));
        assert!(with_h.contains(" H "));
        assert!(!with_h.contains("ns:"));

        let mut xyzr = XyzrFileWriter::new();
        xyzr.set_selection(Some(parse_selector("name N").unwrap()));
        let xyzr = xyzr.write_xyzr_string(&molecule);
        // N in ALA -> NH1 united-atom radius 1.60 (vdwhr), not element 1.55.
        assert_eq!(xyzr, "   0.000    0.000    0.000 1.60\n");
    }

    #[test]
    fn pqr_writes_template_partial_charge_and_united_atom_radius() {
        // C++ PQRFileWriter writes the force-field partial charge + united-atom
        // radius. A backbone CA in ALA -> united charge 0.104 (CA 0.058 +
        // topology HA 0.046) and radius 1.95 (CH1 vdwhr, default_params.xml).
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "CA", "C", 0.0));
        let out = PqrFileWriter::new().write_pqr_string(&molecule);
        assert!(
            out.contains("0.1040"),
            "PQR must write the united-atom charge 0.104, got: {out}"
        );
        assert!(
            out.contains("1.950"),
            "PQR must write the united-atom radius 1.95, got: {out}"
        );
    }

    #[test]
    fn molsurf_renderer_defaults_match_qif() {
        let mut renderer = MolSurfRenderer::new();
        assert_eq!(renderer.type_name(), "molsurf");
        assert_eq!(
            renderer.default_color().rgba8(),
            SolidColor::from_rgba(192.0 / 255.0, 192.0 / 255.0, 1.0, 1.0).rgba8()
        );
        assert!(!renderer.cull_face());
        assert_eq!(renderer.draw_mode(), MolSurfRenderer::SFDRAW_FILL);
        assert_eq!(renderer.color_mode(), MolSurfRenderer::SFREND_SIMPLE);
        assert_eq!(renderer.target(), "");
        assert_eq!(renderer.width(), 1.2);
        assert_eq!(renderer.elepot(), "");
        assert_eq!(renderer.color_mapname(), "");
        assert_eq!(renderer.low_param(), -10.0);
        assert_eq!(renderer.mid_param(), 0.0);
        assert_eq!(renderer.high_param(), 10.0);
        assert!(!renderer.ramp_above());
        assert_eq!(renderer.ramp_value(), 1.4);
        assert_eq!(renderer.coloring().type_name(), "SolidColoring");
        renderer.set_sel("name CA");
        assert_eq!(renderer.sel(), "name CA");
        assert!(!renderer.cullface());
        assert_eq!(renderer.drawmode(), MolSurfRenderer::SFDRAW_FILL);
        renderer.set_cullface(true);
        renderer.set_drawmode(MolSurfRenderer::SFDRAW_LINE);
        renderer.set_elepot("potential");
        renderer.set_color_mapname("density");
        assert!(renderer.cull_face());
        assert_eq!(renderer.draw_mode(), MolSurfRenderer::SFDRAW_LINE);
        assert_eq!(renderer.elepot(), "potential");
        assert_eq!(renderer.color_mapname(), "density");
    }

    #[test]
    fn directsurf_renderer_defaults_match_qif() {
        let mut renderer = DirectSurfRenderer::new();
        assert_eq!(renderer.type_name(), "dsurface");
        assert_eq!(renderer.color_mode(), DirectSurfRenderer::DS_MOLFANC);
        assert_eq!(renderer.get_color_mode(), DirectSurfRenderer::DS_MOLFANC);
        assert_eq!(renderer.detail(), 6);
        assert_eq!(renderer.get_detail(), 6);
        assert_eq!(renderer.probe_radius(), 1.4);
        assert_eq!(renderer.get_probe_radius(), 1.4);
        assert_eq!(renderer.surface_type(), DirectSurfRenderer::DS_SES);
        assert_eq!(renderer.get_surf_type(), DirectSurfRenderer::DS_SES);
        assert_eq!(
            renderer.surface_algorithm(),
            DirectSurfRenderer::DS_DISTFIELD
        );
        assert_eq!(renderer.draw_mode(), DirectSurfRenderer::SFDRAW_FILL);
        assert_eq!(renderer.get_draw_mode(), DirectSurfRenderer::SFDRAW_FILL);
        assert_eq!(renderer.width(), 1.2);
        assert_eq!(renderer.get_line_width(), 1.2);
        assert_eq!(renderer.vdw_radius("H"), 1.2);
        assert_eq!(renderer.vdw_radius("Xe"), 1.7);
        assert_eq!(renderer.vdwr_h(), 1.2);
        assert_eq!(renderer.vdwr_c(), 1.7);
        assert_eq!(renderer.vdwr_n(), 1.55);
        assert_eq!(renderer.vdwr_o(), 1.52);
        assert_eq!(renderer.vdwr_s(), 1.8);
        assert_eq!(renderer.vdwr_p(), 1.8);
        assert_eq!(renderer.vdwr_x(), 1.7);
        assert!(!renderer.ramp_above());
        assert!(!renderer.is_ramp_above());
        assert_eq!(renderer.ramp_value(), 1.4);
        renderer.set_cullface(true);
        renderer.set_drawmode(DirectSurfRenderer::SFDRAW_POINT);
        renderer.set_vdwr_h(1.1);
        renderer.set_vdwr_c(1.6);
        renderer.set_vdwr_n(1.5);
        renderer.set_vdwr_o(1.4);
        renderer.set_vdwr_s(1.9);
        renderer.set_vdwr_p(2.0);
        renderer.set_vdwr_x(2.1);
        assert!(renderer.cull_face());
        assert_eq!(renderer.draw_mode(), DirectSurfRenderer::SFDRAW_POINT);
        assert_eq!(renderer.vdw_radius("H"), 1.1);
        assert_eq!(renderer.vdw_radius("C"), 1.6);
        assert_eq!(renderer.vdw_radius("N"), 1.5);
        assert_eq!(renderer.vdw_radius("O"), 1.4);
        assert_eq!(renderer.vdw_radius("S"), 1.9);
        assert_eq!(renderer.vdw_radius("P"), 2.0);
        assert_eq!(renderer.vdw_radius("Xe"), 2.1);

        let mut renderer2 = DirectSurfRenderer2::new();
        assert_eq!(renderer2.type_name(), "dsurf2");
        assert_eq!(renderer2.get_color_mode(), DirectSurfRenderer2::DS_MOLFANC);
        assert_eq!(renderer2.get_surf_type(), DirectSurfRenderer2::DS_SES);
        assert_eq!(renderer2.get_draw_mode(), DirectSurfRenderer2::SFDRAW_FILL);
        assert_eq!(renderer2.get_probe_radius(), 1.4);
        renderer2.set_vdwr_c(1.65);
        assert_eq!(renderer2.vdwr_c(), 1.65);
    }

    #[test]
    fn directsurf_renderer_builds_vdw_sas_and_ses_geometry() {
        let molecule = MolCoord::with_atoms(vec![surface_atom(1, "CA", "C", 0.0)]);
        let mut renderer = DirectSurfRenderer::new();

        renderer.set_surface_type(DirectSurfRenderer::DS_VDW);
        let vdw = renderer.render_mesh(&molecule);
        renderer.set_surface_type(DirectSurfRenderer::DS_SAS);
        let sas = renderer.render_mesh(&molecule);
        renderer.set_surface_type(DirectSurfRenderer::DS_SES);
        let ses = renderer.render_mesh(&molecule);

        let extent = |mesh: &SurfaceMesh| {
            mesh.vertices()
                .iter()
                .map(|vertex| vertex.position.x.abs())
                .fold(0.0_f64, f64::max)
        };
        assert!((extent(&vdw) - 1.7).abs() < renderer.grid_spacing());
        assert!((extent(&sas) - 3.1).abs() < renderer.grid_spacing());
        assert!((extent(&ses) - 1.7).abs() < renderer.grid_spacing());
        assert!(sas.triangle_count() > vdw.triangle_count());

        renderer.set_selection(parse_selector("name N").unwrap());
        assert!(renderer.render_mesh(&molecule).is_empty());

        let mut renderer2 = DirectSurfRenderer2::new();
        renderer2.set_surface_type(DirectSurfRenderer2::DS_VDW);
        let mesh = renderer2.render_mesh(&molecule);
        assert!(mesh.triangle_count() > 0);
        assert!(mesh
            .vertices()
            .iter()
            .all(|vertex| vertex.info == molecule.atoms()[0].serial as u32));
    }

    #[test]
    fn ms2_test_renderer_builds_the_reference_sas_mesh() {
        let renderer = Ms2TestRenderer::new();
        let molecule = MolCoord::with_atoms(vec![surface_atom(1, "CA", "C", 0.0)]);
        let mesh = renderer.render_mesh(&molecule);
        assert_eq!(renderer.type_name(), "ms2test");
        assert!(!renderer.is_hit_test_supported());
        assert_eq!(renderer.get_center(), Vector4::ZERO);
        assert!(mesh.vertex_count() > 100);
        assert!(mesh.triangle_count() > 100);
        let max_x = mesh
            .vertices()
            .iter()
            .map(|vertex| vertex.position.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max_x - 3.0).abs() < 0.3);
    }

    #[test]
    fn elepot_and_hole_builder_state_matches_qif_surface_flow() {
        let elepot = ElePotMap::new();
        assert_eq!(elepot.type_name(), "elepot");
        let mut view = View::new(cuemol_core::Uid::new(1), cuemol_core::Uid::new(2));
        let camera = view.camera();
        elepot.fit_view(&mut view, false);
        assert_eq!(view.camera(), camera);

        let mut molecule = MolCoord::new();
        let mut serial = 1;
        for z in [-2.0, 0.0, 2.0, 4.0, 6.0] {
            for division in 0..12 {
                let angle = division as f64 * std::f64::consts::TAU / 12.0;
                let mut atom = surface_atom(serial, "C", "C", 4.0 * angle.cos());
                atom.position.y = 4.0 * angle.sin();
                atom.position.z = z;
                molecule.push_atom(atom);
                serial += 1;
            }
        }
        let mut builder = HoleSurfBuilder::new();
        assert_eq!(builder.surf_den(), 2.0);
        builder.set_target_mol(&molecule);
        builder.set_target_sel("*");
        assert_eq!(builder.target_mol_name(), "MolCoord");
        assert_eq!(builder.get_target_mol().unwrap().atom_count(), 60);
        assert_eq!(builder.target_sel_text(), "*");
        assert_eq!(builder.get_target_sel().unwrap().dump(), "*");
        builder.set_surf_den(2.0);
        builder.set_surf_probe_r(1.2);
        builder.doit().unwrap();
        let result = builder.result().unwrap();
        assert_eq!(result.orig_mol(), "MolCoord");
        assert_eq!(result.orig_sel(), "*");
        assert_eq!(result.orig_den(), 2.0);
        assert_eq!(result.orig_prad(), 1.2);
        assert!(!builder.path().is_empty());
        assert!(result.mesh().triangle_count() > 0);

        assert_eq!(Ms2TestRenderer::new().type_name(), "ms2test");
    }

    fn surface_atom(serial: i32, name: &str, element: &str, x: f64) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: element.to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        }
    }

    fn cube_mesh(half: f64) -> SurfaceMesh {
        let positions = [
            Vector4::new3(-half, -half, -half),
            Vector4::new3(half, -half, -half),
            Vector4::new3(half, half, -half),
            Vector4::new3(-half, half, -half),
            Vector4::new3(-half, -half, half),
            Vector4::new3(half, -half, half),
            Vector4::new3(half, half, half),
            Vector4::new3(-half, half, half),
        ];
        let mut mesh = SurfaceMesh::new();
        for position in positions {
            mesh.push_vertex(SurfaceVertex {
                position,
                normal: normalize_or_z(position),
                info: 0,
            });
        }
        for triangle in [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ] {
            mesh.push_triangle(triangle);
        }
        mesh
    }

    fn append_mesh(target: &mut SurfaceMesh, source: SurfaceMesh) {
        let offset = target.vertex_count() as u32;
        for vertex in source.vertices {
            target.push_vertex(vertex);
        }
        for [a, b, c] in source.triangles {
            target.push_triangle([a + offset, b + offset, c + offset]);
        }
    }

    fn section_area(mesh: &SurfaceMesh, cap_normal: Vector4) -> f64 {
        mesh.triangles()
            .iter()
            .filter_map(|triangle| {
                let vertices = [
                    &mesh.vertices()[triangle[0] as usize],
                    &mesh.vertices()[triangle[1] as usize],
                    &mesh.vertices()[triangle[2] as usize],
                ];
                vertices
                    .iter()
                    .all(|vertex| vertex.normal.dot3d(cap_normal) > 0.9999)
                    .then(|| {
                        (vertices[1].position - vertices[0].position)
                            .cross(vertices[2].position - vertices[0].position)
                            .length3d()
                            * 0.5
                    })
            })
            .sum()
    }
}
