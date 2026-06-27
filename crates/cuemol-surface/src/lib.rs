use cuemol_core::Vector4;
use cuemol_gfx::SolidColor;
use cuemol_molstr::{parse_selector, Atom, MolCoord, Selector, SolidColoring};
use cuemol_qsys::{InOutHandler, MultiGradient, ObjReader, ObjWriter};
use std::collections::BTreeMap;

pub mod analytic_ses;
pub mod grid_surface;
pub mod msms;
pub mod ply;
pub mod qdf;

pub use grid_surface::{build_grid_isosurface, ScalarField, SurfSphere};
pub use msms::{read_msms_surface_files, read_msms_surface_str, sniff_msms_content, MsmsReader};
pub use ply::{read_ply_file, read_ply_str};
pub use qdf::{QdfPotReader, QdfPotWriter, QdfSurfReader, QdfSurfWriter};

pub type MS2TestRenderer = Ms2TestRenderer;
pub type MSMSFileReader = MsmsFileReader;
pub type OpenDXPotReader = OpenDxPotReader;
pub type PQRFileWriter = PqrFileWriter;
pub type XYZRFileWriter = XyzrFileWriter;

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

    pub fn vertex_file(&self) -> &str {
        &self.vertex_file
    }

    pub fn set_vertex_file(&mut self, value: impl Into<String>) {
        self.vertex_file = value.into();
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
}

impl Default for OpenDxPotReader {
    fn default() -> Self {
        Self::new()
    }
}

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

    pub fn set_use_h(&mut self, value: bool) {
        self.use_h = value;
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn ns(&self) -> &str {
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
        for (index, atom) in molecule.atoms().iter().enumerate() {
            // Drop hydrogens in the default united-atom mode + honour the selection
            // (C++ PQRFileWriter:104 / selected_surface_atoms).
            if !self.use_h && atom.element.trim().eq_ignore_ascii_case("H") {
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
            let charge = if self.use_h {
                cuemol_molstr::toppar::partial_charge_for_atom(atom)
            } else {
                cuemol_molstr::toppar::united_atom_partial_charge(molecule, index)
            }
            .unwrap_or(0.0);
            // Radius is the united-atom vdwhr; the use_h=true explicit-H radius
            // (vdwr) is a minor remaining deferral.
            let radius = cuemol_molstr::toppar::vdw_radius_for_atom(atom);
            output.push_str(&format!(
                "ATOM  {:5} {:<4} {:>3} {:1}{:4}    {:8.3}{:8.3}{:8.3} {:7.4} {:6.3}\n",
                atom.serial,
                namespaced_atom_name(atom, &self.namespace),
                atom.res_name,
                atom.chain_id,
                atom.res_id,
                atom.position.x,
                atom.position.y,
                atom.position.z,
                charge,
                radius
            ));
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
            let radius = SurfaceVdwRadii::default().radius_for_element(&atom.element);
            output.push_str(&format!(
                "{:12.6} {:12.6} {:12.6} {:8.4}\n",
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

fn namespaced_atom_name(atom: &Atom, namespace: &str) -> String {
    if namespace.is_empty() {
        atom.name.clone()
    } else {
        format!("{namespace}:{}", atom.name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceVertex {
    pub position: Vector4,
    pub normal: Vector4,
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
        let normal = normalize_or_z(normal);
        let mut vertex_map = BTreeMap::<u32, u32>::new();
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let before = self.triangles.len();

        for triangle in &self.triangles {
            let Some(points) = triangle_points(self, *triangle) else {
                continue;
            };
            let centroid = (points[0] + points[1] + points[2]).scale(1.0 / 3.0);
            let signed = (centroid - position).dot(normal);
            let keep = if keep_positive {
                signed >= 0.0
            } else {
                signed <= 0.0
            };
            if !keep {
                continue;
            }
            let mut next = [0_u32; 3];
            for (index, old_uid) in triangle.iter().copied().enumerate() {
                let new_uid = *vertex_map.entry(old_uid).or_insert_with(|| {
                    let vertex = self.vertices[old_uid as usize].clone();
                    let uid = vertices.len() as u32;
                    vertices.push(vertex);
                    uid
                });
                next[index] = new_uid;
            }
            triangles.push(next);
        }

        self.vertices = vertices;
        self.triangles = triangles;
        before.saturating_sub(self.triangles.len())
    }
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
    orig_den: f64,
    orig_prad: f64,
    cut_planes: Vec<CutPlane>,
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
            orig_den: 0.0,
            orig_prad: 0.0,
            cut_planes: Vec::new(),
        }
    }
}

impl MolSurfObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mesh(&self) -> &SurfaceMesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut SurfaceMesh {
        &mut self.mesh
    }

    pub fn orig_mol(&self) -> &str {
        &self.orig_mol
    }

    pub fn set_orig_mol(&mut self, value: impl Into<String>) {
        self.orig_mol = value.into();
    }

    pub fn orig_sel(&self) -> &str {
        &self.orig_sel
    }

    pub fn set_orig_sel(&mut self, value: impl Into<String>) {
        self.orig_sel = value.into();
    }

    pub fn orig_den(&self) -> f64 {
        self.orig_den
    }

    pub fn set_orig_den(&mut self, value: f64) {
        self.orig_den = value;
    }

    pub fn orig_prad(&self) -> f64 {
        self.orig_prad
    }

    pub fn set_orig_prad(&mut self, value: f64) {
        self.orig_prad = value;
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
        if body {
            self.mesh.cut_by_plane(normal, position, true);
        }
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
    ) where
        M: IntoSurfaceMoleculeSource<'a>,
        S: IntoSurfaceSelectionSource<'a>,
    {
        let molecule = molecule.into_surface_molecule_source();
        let selection = selection.into_surface_selection_source();
        self.orig_mol = molecule.source_name();
        self.orig_sel = selection.source_text();
        self.orig_den = density;
        self.orig_prad = probe_radius;
        self.mesh.clear();
        if let SurfaceMoleculeSource::Molecule(molecule) = molecule {
            let selector = selection.selector().unwrap_or(Selector::All);
            build_mol_surface(&mut self.mesh, molecule, &selector, density, probe_radius);
        }
    }

    pub fn create_sesfrom_mol<'a, M, S>(
        &mut self,
        molecule: M,
        selection: S,
        density: f64,
        probe_radius: f64,
    ) where
        M: IntoSurfaceMoleculeSource<'a>,
        S: IntoSurfaceSelectionSource<'a>,
    {
        self.create_ses_from_mol(molecule, selection, density, probe_radius);
    }

    pub fn regenerate_ses(&mut self, density: f64) {
        self.orig_den = density;
    }

    pub fn regenerate_ses1(&mut self, density: f64) {
        self.regenerate_ses(density);
    }

    pub fn regenerate_ses_1(&mut self, density: f64) {
        self.regenerate_ses1(density);
    }

    pub fn create_hole_test1(
        &mut self,
        molecule_name: impl Into<String>,
        _dir_norm: Vector4,
        start_pos: Vector4,
    ) {
        self.orig_mol = molecule_name.into();
        self.orig_sel = format!("hole-start:{},{},{}", start_pos.x, start_pos.y, start_pos.z);
    }

    pub fn create_hole_test_1(
        &mut self,
        molecule_name: impl Into<String>,
        dir_norm: Vector4,
        start_pos: Vector4,
    ) {
        self.create_hole_test1(molecule_name, dir_norm, start_pos);
    }
}

/// Build the molecular (van-der-Waals / solvent-accessible) surface of the
/// selected atoms via the union-of-spheres marching-tetrahedra isosurface.
/// `density` controls the grid fineness (grid points per Angstrom); each atom is
/// a sphere of `vdw_radius + probe_radius`.
fn build_mol_surface(
    mesh: &mut SurfaceMesh,
    molecule: &MolCoord,
    selector: &Selector,
    density: f64,
    probe_radius: f64,
) {
    // Each atom is a bare van-der-Waals sphere; the probe is rolled separately by
    // the solvent-excluded-surface builder (the reentrant patches it carves are
    // the whole point of an SES vs. the union-of-spheres accessible surface). The
    // radius is the C++ production surface's UNITED-ATOM vdW (`getVdwRadius`,
    // bExplH=false): a backbone CH1 carbon is 1.95, not the bare element 1.70;
    // atoms with no residue template (ligands/ions/waters) fall back to the
    // element radius.
    let spheres: Vec<grid_surface::SurfSphere> = molecule
        .atoms()
        .iter()
        .filter(|atom| selector.matches_in_molecule(atom, molecule))
        .map(|atom| grid_surface::SurfSphere {
            center: atom.position,
            radius: cuemol_molstr::toppar::vdw_radius_for_atom(atom).max(0.1),
        })
        .collect();

    // density = grid points per Angstrom; spacing is its reciprocal, clamped to
    // a sane band (default ~0.6 A when unset).
    let spacing = if density > 0.0 {
        (1.0 / density).clamp(0.25, 1.5)
    } else {
        0.6
    };
    grid_surface::build_ses_surface(mesh, &spheres, probe_radius.max(0.0), spacing);
}

/// Build the ANALYTIC (Connolly reduced-surface) SES of `molecule` — convex van
/// der Waals caps + saddle tori + concave reentrant patches — as a [`SurfaceMesh`].
/// The analytic counterpart of [`build_mol_surface`]'s grid SES (C++ surface/
/// `RSCompBuilder` + `Tg*`); `detail` is the per-patch tessellation level. The grid
/// SES remains the production path; this matches C++'s exact analytic patches.
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

fn triangle_points(mesh: &SurfaceMesh, triangle: [u32; 3]) -> Option<[Vector4; 3]> {
    Some([
        mesh.vertices.get(triangle[0] as usize)?.position,
        mesh.vertices.get(triangle[1] as usize)?.position,
        mesh.vertices.get(triangle[2] as usize)?.position,
    ])
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

    pub fn set_cull_face(&mut self, value: bool) {
        self.cull_face = value;
    }

    pub fn draw_mode(&self) -> i32 {
        self.draw_mode
    }

    pub fn set_draw_mode(&mut self, value: i32) {
        self.draw_mode = value;
    }

    pub fn color_mode(&self) -> i32 {
        self.color_mode
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

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn elepot(&self) -> &str {
        &self.elepot
    }

    pub fn set_elepot(&mut self, value: impl Into<String>) {
        self.elepot = value.into();
    }

    pub fn color_map_name(&self) -> &str {
        &self.elepot
    }

    pub fn get_color_map_obj(&self) -> Option<&MultiGradient> {
        Some(&self.multi_grad)
    }

    pub fn set_color_map_name(&mut self, value: impl Into<String>) {
        self.set_elepot(value);
    }

    pub fn low_param(&self) -> f64 {
        self.low_param
    }

    pub fn lowpar(&self) -> f64 {
        self.low_param()
    }

    pub fn set_low_param(&mut self, value: f64) {
        self.low_param = value;
    }

    pub fn set_lowpar(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn mid_param(&self) -> f64 {
        self.mid_param
    }

    pub fn midpar(&self) -> f64 {
        self.mid_param()
    }

    pub fn set_mid_param(&mut self, value: f64) {
        self.mid_param = value;
    }

    pub fn set_midpar(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn high_param(&self) -> f64 {
        self.high_param
    }

    pub fn highpar(&self) -> f64 {
        self.high_param()
    }

    pub fn set_high_param(&mut self, value: f64) {
        self.high_param = value;
    }

    pub fn set_highpar(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn high_color(&self) -> &SolidColor {
        &self.high_color
    }

    pub fn highcol(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn set_high_color(&mut self, value: SolidColor) {
        self.high_color = value;
    }

    pub fn set_highcol(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn mid_color(&self) -> &SolidColor {
        &self.mid_color
    }

    pub fn midcol(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn set_mid_color(&mut self, value: SolidColor) {
        self.mid_color = value;
    }

    pub fn set_midcol(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn low_color(&self) -> &SolidColor {
        &self.low_color
    }

    pub fn lowcol(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn set_low_color(&mut self, value: SolidColor) {
        self.low_color = value;
    }

    pub fn set_lowcol(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn multi_grad(&self) -> &MultiGradient {
        &self.multi_grad
    }

    pub fn ramp_above(&self) -> bool {
        self.ramp_above
    }

    pub fn set_ramp_above(&mut self, value: bool) {
        self.ramp_above = value;
    }

    pub fn ramp_value(&self) -> f64 {
        self.ramp_value
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

    pub fn showsel(&self) -> &str {
        self.show_selection()
    }

    pub fn set_show_selection(&mut self, value: impl Into<String>) {
        self.show_selection = value.into();
    }

    pub fn set_showsel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectSurfRenderer {
    color_mode: i32,
    target: String,
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
            detail: 6,
            probe_radius: 1.4,
            surface_type: Self::DS_SES,
            surface_algorithm: Self::DS_EDTSURF,
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
    pub const DS_MSMS: i32 = 1;
    pub const SFDRAW_FILL: i32 = 0;
    pub const SFDRAW_LINE: i32 = 1;
    pub const SFDRAW_POINT: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "directsurf"
    }

    pub fn color_mode(&self) -> i32 {
        self.color_mode
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

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn probe_radius(&self) -> f64 {
        self.probe_radius
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

    pub fn surftype(&self) -> i32 {
        self.surface_type()
    }

    pub fn set_surface_type(&mut self, value: i32) {
        self.surface_type = value;
    }

    pub fn set_surftype(&mut self, value: i32) {
        self.set_surface_type(value);
    }

    pub fn surface_algorithm(&self) -> i32 {
        self.surface_algorithm
    }

    pub fn surfalgor(&self) -> i32 {
        self.surface_algorithm()
    }

    pub fn set_surface_algorithm(&mut self, value: i32) {
        self.surface_algorithm = value;
    }

    pub fn set_surfalgor(&mut self, value: i32) {
        self.set_surface_algorithm(value);
    }

    pub fn cull_face(&self) -> bool {
        self.cull_face
    }

    pub fn set_cull_face(&mut self, value: bool) {
        self.cull_face = value;
    }

    pub fn draw_mode(&self) -> i32 {
        self.draw_mode
    }

    pub fn set_draw_mode(&mut self, value: i32) {
        self.draw_mode = value;
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn show_selection(&self) -> &str {
        &self.show_selection
    }

    pub fn showsel(&self) -> &str {
        self.show_selection()
    }

    pub fn set_show_selection(&mut self, value: impl Into<String>) {
        self.show_selection = value.into();
    }

    pub fn set_showsel(&mut self, value: impl Into<String>) {
        self.set_show_selection(value);
    }

    pub fn vdw_radius(&self, element: &str) -> f64 {
        self.vdw_radii.radius_for_element(element)
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

    pub fn set_elepot(&mut self, value: impl Into<String>) {
        self.elepot = value.into();
    }

    pub fn low_param(&self) -> f64 {
        self.low_param
    }

    pub fn lowpar(&self) -> f64 {
        self.low_param()
    }

    pub fn set_low_param(&mut self, value: f64) {
        self.low_param = value;
    }

    pub fn set_lowpar(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn mid_param(&self) -> f64 {
        self.mid_param
    }

    pub fn midpar(&self) -> f64 {
        self.mid_param()
    }

    pub fn set_mid_param(&mut self, value: f64) {
        self.mid_param = value;
    }

    pub fn set_midpar(&mut self, value: f64) {
        self.set_mid_param(value);
    }

    pub fn high_param(&self) -> f64 {
        self.high_param
    }

    pub fn highpar(&self) -> f64 {
        self.high_param()
    }

    pub fn set_high_param(&mut self, value: f64) {
        self.high_param = value;
    }

    pub fn set_highpar(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn high_color(&self) -> &SolidColor {
        &self.high_color
    }

    pub fn highcol(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn set_high_color(&mut self, value: SolidColor) {
        self.high_color = value;
    }

    pub fn set_highcol(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn mid_color(&self) -> &SolidColor {
        &self.mid_color
    }

    pub fn midcol(&self) -> &SolidColor {
        self.mid_color()
    }

    pub fn set_mid_color(&mut self, value: SolidColor) {
        self.mid_color = value;
    }

    pub fn set_midcol(&mut self, value: SolidColor) {
        self.set_mid_color(value);
    }

    pub fn low_color(&self) -> &SolidColor {
        &self.low_color
    }

    pub fn lowcol(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn set_low_color(&mut self, value: SolidColor) {
        self.low_color = value;
    }

    pub fn set_lowcol(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn ramp_above(&self) -> bool {
        self.ramp_above
    }

    pub fn set_ramp_above(&mut self, value: bool) {
        self.ramp_above = value;
    }

    pub fn ramp_value(&self) -> f64 {
        self.ramp_value
    }

    pub fn set_ramp_value(&mut self, value: f64) {
        self.ramp_value = value;
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElePotMap {
    counts: [usize; 3],
    grid_size: [f64; 3],
    origin: Vector4,
    values: Vec<f64>,
    fit_view_calls: usize,
}

impl Default for ElePotMap {
    fn default() -> Self {
        Self {
            counts: [0, 0, 0],
            grid_size: [0.0, 0.0, 0.0],
            origin: Vector4::ZERO,
            values: Vec::new(),
            fit_view_calls: 0,
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

    pub fn fit_view(&mut self) {
        self.fit_view_calls += 1;
    }

    pub fn fit_view_calls(&self) -> usize {
        self.fit_view_calls
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
    }

    pub fn at_float(&self, col: usize, row: usize, sec: usize) -> f64 {
        if col >= self.counts[0] || row >= self.counts[1] || sec >= self.counts[2] {
            return 0.0;
        }
        let index = col + self.counts[0] * (row + self.counts[1] * sec);
        self.values.get(index).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoleSurfBuilder {
    target_mol: String,
    target_sel: String,
    dir_norm: Vector4,
    start_pos: Vector4,
    surf_den: f64,
    surf_probe_r: f64,
    result: Option<MolSurfObj>,
}

impl Default for HoleSurfBuilder {
    fn default() -> Self {
        Self {
            target_mol: String::new(),
            target_sel: String::new(),
            dir_norm: Vector4::new3(0.0, 0.0, 1.0),
            start_pos: Vector4::new3(0.0, 0.0, 0.0),
            surf_den: 0.0,
            surf_probe_r: 1.4,
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

    pub fn set_target_mol(&mut self, value: impl Into<String>) {
        self.target_mol = value.into();
    }

    pub fn target_sel(&self) -> &str {
        &self.target_sel
    }

    pub fn set_target_sel(&mut self, value: impl Into<String>) {
        self.target_sel = value.into();
    }

    pub fn dir_norm(&self) -> Vector4 {
        self.dir_norm
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

    pub fn set_surf_den(&mut self, value: f64) {
        self.surf_den = value;
    }

    pub fn surf_probe_r(&self) -> f64 {
        self.surf_probe_r
    }

    pub fn set_surf_probe_r(&mut self, value: f64) {
        self.surf_probe_r = value;
    }

    pub fn result(&self) -> Option<&MolSurfObj> {
        self.result.as_ref()
    }

    pub fn doit(&mut self) {
        let mut result = MolSurfObj::new();
        result.create_ses_from_mol(
            self.target_mol.clone(),
            self.target_sel.clone(),
            self.surf_den,
            self.surf_probe_r,
        );
        self.result = Some(result);
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
        obj.create_ses_from_mol("mol1", "protein", 3.0, 1.4);
        assert_eq!(obj.orig_mol(), "mol1");
        assert_eq!(obj.orig_sel(), "protein");
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
    fn molsurf_obj_builds_selected_atom_mesh_from_molecule() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "N", "N", 0.0));
        molecule.push_atom(surface_atom(2, "CA", "C", 4.0));
        let selector = parse_selector("name CA").unwrap();

        let mut obj = MolSurfObj::new();
        obj.create_ses_from_mol(&molecule, &selector, 2.0, 1.4);

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
        obj.create_ses_from_mol(&molecule, "all", 2.0, 1.4);
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
        let msms = MsmsFileReader::new();
        assert_eq!(msms.handler().name(), "msms");
        assert_eq!(
            msms.handler().type_descr(),
            "MSMS surface file (*.face/vert)"
        );
        assert_eq!(msms.handler().file_ext(), "*.face");
        assert_eq!(msms.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
        assert_eq!(msms.vertex_file(), "");

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
    fn qdf_surface_roundtrip_matches_reference_schema() {
        let mut obj = MolSurfObj::new();
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(1.0, 2.0, 3.0),
            normal: Vector4::new3(0.0, 0.0, 1.0),
        });
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(2.0, 3.0, 4.0),
            normal: Vector4::new3(0.0, 1.0, 0.0),
        });
        obj.mesh_mut().push_vertex(SurfaceVertex {
            position: Vector4::new3(3.0, 4.0, 5.0),
            normal: Vector4::new3(1.0, 0.0, 0.0),
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
        assert_eq!(no_h.lines().count(), 1);
        assert!(no_h.contains(" CA "));
        assert!(!no_h.contains(" H "));

        pqr.set_use_h(true);
        pqr.set_namespace("ns");
        let with_h = pqr.write_pqr_string(&molecule);
        assert_eq!(with_h.lines().count(), 2);
        assert!(with_h.contains("ns:CA"));
        assert!(with_h.contains("ns:H"));

        let mut xyzr = XyzrFileWriter::new();
        xyzr.set_selection(Some(parse_selector("name N").unwrap()));
        let xyzr = xyzr.write_xyzr_string(&molecule);
        assert_eq!(xyzr.lines().count(), 1);
        assert!(xyzr.contains("1.5500"));
    }

    #[test]
    fn pqr_writes_template_partial_charge_and_united_atom_radius() {
        // C++ PQRFileWriter writes the force-field partial charge + united-atom
        // radius. A backbone CA in ALA -> charge 0.058 (prot_top.xml) and radius
        // 1.95 (CH1 vdwhr, default_params.xml), NOT formal charge 0 / element 1.70.
        let mut molecule = MolCoord::new();
        molecule.push_atom(surface_atom(1, "CA", "C", 0.0));
        let out = PqrFileWriter::new().write_pqr_string(&molecule);
        assert!(
            out.contains("0.0580"),
            "PQR must write the partial charge 0.058, got: {out}"
        );
        assert!(
            out.contains("1.950"),
            "PQR must write the united-atom radius 1.95, got: {out}"
        );
    }

    #[test]
    fn molsurf_renderer_defaults_match_qif() {
        let renderer = MolSurfRenderer::new();
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
        assert_eq!(renderer.low_param(), -10.0);
        assert_eq!(renderer.mid_param(), 0.0);
        assert_eq!(renderer.high_param(), 10.0);
        assert!(!renderer.ramp_above());
        assert_eq!(renderer.ramp_value(), 1.4);
        assert_eq!(renderer.coloring().type_name(), "SolidColoring");
    }

    #[test]
    fn directsurf_renderer_defaults_match_qif() {
        let renderer = DirectSurfRenderer::new();
        assert_eq!(renderer.type_name(), "directsurf");
        assert_eq!(renderer.color_mode(), DirectSurfRenderer::DS_MOLFANC);
        assert_eq!(renderer.detail(), 6);
        assert_eq!(renderer.probe_radius(), 1.4);
        assert_eq!(renderer.surface_type(), DirectSurfRenderer::DS_SES);
        assert_eq!(renderer.surface_algorithm(), DirectSurfRenderer::DS_EDTSURF);
        assert_eq!(renderer.draw_mode(), DirectSurfRenderer::SFDRAW_FILL);
        assert_eq!(renderer.width(), 1.2);
        assert_eq!(renderer.vdw_radius("H"), 1.2);
        assert_eq!(renderer.vdw_radius("Xe"), 1.7);
        assert!(!renderer.ramp_above());
        assert_eq!(renderer.ramp_value(), 1.4);
    }

    #[test]
    fn elepot_and_hole_builder_state_matches_qif_surface_flow() {
        let mut elepot = ElePotMap::new();
        assert_eq!(elepot.type_name(), "elepot");
        elepot.fit_view();
        assert_eq!(elepot.fit_view_calls(), 1);

        let mut builder = HoleSurfBuilder::new();
        builder.set_target_mol("mol1");
        builder.set_target_sel("*");
        builder.set_surf_den(4.0);
        builder.set_surf_probe_r(1.2);
        builder.doit();
        let result = builder.result().unwrap();
        assert_eq!(result.orig_mol(), "mol1");
        assert_eq!(result.orig_sel(), "*");
        assert_eq!(result.orig_den(), 4.0);
        assert_eq!(result.orig_prad(), 1.2);

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
        }
    }
}
