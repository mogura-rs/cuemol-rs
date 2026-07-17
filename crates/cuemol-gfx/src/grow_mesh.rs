use cuemol_core::{Matrix4, Vector4};
use std::f64::consts::PI;

const EPSILON: f64 = 1.0e-4;

#[derive(Debug, Clone, PartialEq)]
pub struct MeshVertex<C = u32> {
    pub v: Vector4,
    pub n: Vector4,
    pub c: C,
}

impl<C> MeshVertex<C> {
    pub fn new(vertex: Vector4, normal: Vector4, color: C) -> Self {
        Self {
            v: vertex,
            n: normal,
            c: color,
        }
    }

    pub fn is_finite(&self) -> bool {
        self.v.x.is_finite()
            && self.v.y.is_finite()
            && self.v.z.is_finite()
            && self.n.x.is_finite()
            && self.n.y.is_finite()
            && self.n.z.is_finite()
    }
}

pub type MeshVert<C = u32> = MeshVertex<C>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshFace {
    pub iv1: usize,
    pub iv2: usize,
    pub iv3: usize,
    pub nmode: i32,
}

impl MeshFace {
    pub const MFMOD_MESH: i32 = 0;
    pub const MFMOD_NORGLN: i32 = 1;
    pub const MFMOD_SPHERE: i32 = 2;
    pub const MFMOD_CYL: i32 = 3;
    pub const MFMOD_MESHXX: i32 = 4;

    pub fn new(iv1: usize, iv2: usize, iv3: usize, nmode: i32) -> Self {
        Self {
            iv1,
            iv2,
            iv3,
            nmode,
        }
    }

    pub fn indices(self) -> [usize; 3] {
        [self.iv1, self.iv2, self.iv3]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrowMesh<C = u32> {
    lighting: bool,
    faces: Vec<MeshFace>,
    vertices: Vec<MeshVertex<C>>,
    pivot: usize,
    mode: i32,
}

impl<C> Default for GrowMesh<C> {
    fn default() -> Self {
        Self {
            lighting: true,
            faces: Vec::new(),
            vertices: Vec::new(),
            pivot: 0,
            mode: Self::GM_TRIGS,
        }
    }
}

impl<C> GrowMesh<C> {
    pub const GM_TRIGS: i32 = 0;
    pub const GM_TRIGSTRIP: i32 = 1;
    pub const GM_TRIGFAN: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_size(&self) -> usize {
        self.vertices.len()
    }

    pub fn get_vertex_size(&self) -> usize {
        self.vertex_size()
    }

    pub fn face_size(&self) -> usize {
        self.faces.len()
    }

    pub fn get_face_size(&self) -> usize {
        self.face_size()
    }

    pub fn lighting(&self) -> bool {
        self.lighting
    }

    pub fn set_lighting(&mut self, value: bool) {
        self.lighting = value;
    }

    pub fn add_vertex(&mut self, vertex: Vector4, normal: Vector4, color: C) -> usize {
        let index = self.vertices.len();
        self.vertices.push(MeshVertex::new(vertex, normal, color));
        index
    }

    pub fn add_vertex_transformed(
        &mut self,
        mut vertex: Vector4,
        mut normal: Vector4,
        color: C,
        transform: Matrix4,
    ) -> usize {
        vertex.w = 1.0;
        transform.xform4d(&mut vertex);
        normal.w = 0.0;
        transform.xform4d(&mut normal);
        self.add_vertex(vertex, normal, color)
    }

    pub fn add_mesh_vertex(&mut self, vertex: MeshVertex<C>) -> usize {
        let index = self.vertices.len();
        self.vertices.push(vertex);
        index
    }

    pub fn add_face(&mut self, iv1: usize, iv2: usize, iv3: usize, mode: i32) {
        self.faces.push(MeshFace::new(iv1, iv2, iv3, mode));
    }

    pub fn add_triangle(&mut self, iv1: usize, iv2: usize, iv3: usize) {
        self.add_face(iv1, iv2, iv3, MeshFace::MFMOD_MESH);
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.faces.clear();
        self.pivot = 0;
    }

    pub fn start(&mut self, mode: i32) {
        self.mode = mode;
        self.pivot = self.vertices.len();
    }

    pub fn end(&mut self) {
        match self.mode {
            Self::GM_TRIGS => self.end_triangles(),
            Self::GM_TRIGSTRIP => self.end_triangle_strip(),
            Self::GM_TRIGFAN => self.end_triangle_fan(),
            _ => {}
        }
    }

    pub fn vertex(&self, index: usize) -> &MeshVertex<C> {
        &self.vertices[index]
    }

    pub fn get_vertex(&self, index: usize) -> &MeshVertex<C> {
        self.vertex(index)
    }

    pub fn vertex_mut(&mut self, index: usize) -> &mut MeshVertex<C> {
        &mut self.vertices[index]
    }

    pub fn face(&self, index: usize) -> &MeshFace {
        &self.faces[index]
    }

    pub fn get_face(&self, index: usize) -> &MeshFace {
        self.face(index)
    }

    pub fn vertices(&self) -> &[MeshVertex<C>] {
        &self.vertices
    }

    pub fn get_vertex_data(&self) -> &[MeshVertex<C>] {
        self.vertices()
    }

    pub fn faces(&self) -> &[MeshFace] {
        &self.faces
    }

    pub fn get_face_data(&self) -> &[MeshFace] {
        self.faces()
    }

    fn end_triangles(&mut self) {
        let vertex_count = self.vertices.len().saturating_sub(self.pivot);
        for index in 0..(vertex_count / 3) {
            self.add_face(
                self.pivot + index * 3,
                self.pivot + index * 3 + 1,
                self.pivot + index * 3 + 2,
                MeshFace::MFMOD_MESH,
            );
        }
    }

    fn end_triangle_strip(&mut self) {
        let vertex_count = self.vertices.len().saturating_sub(self.pivot);
        for index in 2..vertex_count {
            if index % 2 == 0 {
                self.add_face(
                    self.pivot + index - 2,
                    self.pivot + index - 1,
                    self.pivot + index,
                    MeshFace::MFMOD_MESH,
                );
            } else {
                self.add_face(
                    self.pivot + index,
                    self.pivot + index - 1,
                    self.pivot + index - 2,
                    MeshFace::MFMOD_MESH,
                );
            }
        }
    }

    fn end_triangle_fan(&mut self) {
        let vertex_count = self.vertices.len().saturating_sub(self.pivot);
        for index in 2..vertex_count {
            self.add_face(
                self.pivot + index - 1,
                self.pivot + index,
                self.pivot,
                MeshFace::MFMOD_MESH,
            );
        }
    }
}

impl<C: Clone> GrowMesh<C> {
    pub fn copy_vertex(&mut self, original: &MeshVertex<C>) -> usize {
        self.add_vertex(original.v, original.n, original.c.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cylinder<C = u32> {
    pub v1: Vector4,
    pub v2: Vector4,
    pub col: C,
    pub w1: f64,
    pub w2: f64,
    pub bcap: bool,
    pub ndetail: i32,
    pub transform: Option<Matrix4>,
}

impl<C: Default> Default for Cylinder<C> {
    fn default() -> Self {
        Self {
            v1: Vector4::ZERO,
            v2: Vector4::ZERO,
            col: C::default(),
            w1: 1.0,
            w2: 1.0,
            bcap: false,
            ndetail: 1,
            transform: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CylinderList<C = u32> {
    data: Vec<Cylinder<C>>,
}

impl<C> CylinderList<C> {
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        v1: Vector4,
        v2: Vector4,
        w1: f64,
        w2: f64,
        color: C,
        detail: i32,
        cap: bool,
        transform: Option<Matrix4>,
    ) {
        self.data.push(Cylinder {
            v1,
            v2,
            col: color,
            w1,
            w2,
            bcap: cap,
            ndetail: detail,
            transform,
        });
    }

    pub fn erase_all(&mut self) {
        self.data.clear();
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn cylinders(&self) -> &[Cylinder<C>] {
        &self.data
    }
}

impl<C: Clone> CylinderList<C> {
    pub fn make_mesh(&self, mesh: &mut GrowMesh<C>) {
        for cylinder in &self.data {
            tessellate_cylinder(mesh, cylinder);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sphere<C = u32> {
    pub v1: Vector4,
    pub col: C,
    pub r: f64,
    pub ndetail: i32,
    pub transform: Option<Matrix4>,
}

impl<C> Sphere<C> {
    pub fn new(center: Vector4, color: C, radius: f64, detail: i32) -> Self {
        Self {
            v1: center,
            col: color,
            r: radius,
            ndetail: detail,
            transform: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SphereList<C = u32> {
    data: Vec<Sphere<C>>,
}

impl<C> SphereList<C> {
    pub fn add(
        &mut self,
        center: Vector4,
        radius: f64,
        color: C,
        detail: i32,
        transform: Option<Matrix4>,
    ) {
        let mut sphere = Sphere::new(center, color, radius, detail);
        sphere.transform = transform;
        self.data.push(sphere);
    }

    pub fn erase_all(&mut self) {
        self.data.clear();
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn spheres(&self) -> &[Sphere<C>] {
        &self.data
    }
}

impl<C: Clone> SphereList<C> {
    pub fn make_mesh(&self, mesh: &mut GrowMesh<C>, transform: Option<Matrix4>) {
        for sphere in &self.data {
            tessellate_sphere(mesh, sphere, transform);
        }
    }
}

fn tessellate_cylinder<C: Clone>(mesh: &mut GrowMesh<C>, cylinder: &Cylinder<C>) {
    let delta = cylinder.v1 - cylinder.v2;
    let length = delta.length3d();
    if length <= EPSILON || cylinder.ndetail < 0 {
        return;
    }
    let axis = delta.scale(1.0 / length);
    let ex = Vector4::new3(1.0, 0.0, 0.0);
    let ey = Vector4::new3(0.0, 1.0, 0.0);
    let ez = Vector4::new3(0.0, 0.0, 1.0);
    let reference = if axis.dot3d(ex).abs() < 0.9 {
        ex
    } else if axis.dot3d(ey).abs() < 0.9 {
        ey
    } else {
        ez
    };
    let radial = match axis.cross(reference).normalize3d() {
        Ok(value) => value,
        Err(_) => return,
    };
    let rotation = basis_matrix(axis, radial);
    let mut transform = cylinder.transform.unwrap_or(Matrix4::IDENTITY);
    transform = transform
        .mul_mat(Matrix4::make_trans_mat(cylinder.v2))
        .mul_mat(rotation);

    let radial_divisions = 2 * (cylinder.ndetail as usize + 1);
    if radial_divisions == 0 {
        return;
    }
    let angle_step = 2.0 * PI / radial_divisions as f64;
    let bottom_radius = cylinder.w2;
    let top_radius = cylinder.w1;

    let mut bottom_cap = None;
    if cylinder.bcap {
        let center = mesh.add_vertex_transformed(
            Vector4::ZERO,
            Vector4::new3(0.0, 0.0, -1.0),
            cylinder.col.clone(),
            transform,
        );
        for index in 0..radial_divisions {
            let angle = index as f64 * angle_step;
            mesh.add_vertex_transformed(
                Vector4::new3(
                    bottom_radius * angle.cos(),
                    bottom_radius * angle.sin(),
                    0.0,
                ),
                Vector4::new3(0.0, 0.0, -1.0),
                cylinder.col.clone(),
                transform,
            );
        }
        bottom_cap = Some(center);
    }

    let body_start = mesh.vertex_size();
    for (layer, radius) in [bottom_radius, top_radius].into_iter().enumerate() {
        let z = layer as f64 * length;
        for index in 0..radial_divisions {
            let angle = index as f64 * angle_step;
            let cosine = angle.cos();
            let sine = angle.sin();
            mesh.add_vertex_transformed(
                Vector4::new3(radius * cosine, radius * sine, z),
                Vector4::new3(cosine, sine, 0.0),
                cylinder.col.clone(),
                transform,
            );
        }
    }

    let mut top_cap = None;
    if cylinder.bcap {
        let center = mesh.add_vertex_transformed(
            Vector4::new3(0.0, 0.0, length),
            Vector4::new3(0.0, 0.0, 1.0),
            cylinder.col.clone(),
            transform,
        );
        for index in 0..radial_divisions {
            let angle = index as f64 * angle_step;
            mesh.add_vertex_transformed(
                Vector4::new3(top_radius * angle.cos(), top_radius * angle.sin(), length),
                Vector4::new3(0.0, 0.0, 1.0),
                cylinder.col.clone(),
                transform,
            );
        }
        top_cap = Some(center);
    }

    let face_mode = if cylinder.bcap {
        MeshFace::MFMOD_CYL
    } else {
        MeshFace::MFMOD_NORGLN
    };
    if let Some(center) = bottom_cap {
        for index in 0..=radial_divisions {
            let current = index % radial_divisions;
            let next = (index + 1) % radial_divisions;
            mesh.add_face(center, center + 1 + next, center + 1 + current, face_mode);
        }
    }
    for index in 0..radial_divisions {
        let current = index;
        let next = (index + 1) % radial_divisions;
        mesh.add_face(
            body_start + current,
            body_start + next,
            body_start + radial_divisions + next,
            face_mode,
        );
        mesh.add_face(
            body_start + current,
            body_start + radial_divisions + next,
            body_start + radial_divisions + current,
            face_mode,
        );
    }
    if let Some(center) = top_cap {
        for index in 0..=radial_divisions {
            let current = index % radial_divisions;
            let next = (index + 1) % radial_divisions;
            mesh.add_face(center, center + 1 + current, center + 1 + next, face_mode);
        }
    }
}

fn tessellate_sphere<C: Clone>(
    mesh: &mut GrowMesh<C>,
    sphere: &Sphere<C>,
    external_transform: Option<Matrix4>,
) {
    if sphere.r <= 0.0 || sphere.ndetail <= 0 {
        return;
    }
    let transform = sphere
        .transform
        .or(external_transform)
        .unwrap_or(Matrix4::IDENTITY);
    let radius = sphere.r;
    let maximum_edge = PI * radius / f64::from(sphere.ndetail + 1);
    let latitude_count = ((f64::from(sphere.ndetail) / 2.0).ceil() as usize) * 2;
    if latitude_count < 2 {
        return;
    }

    let mut rows = Vec::<Vec<usize>>::with_capacity(latitude_count + 1);
    for latitude in 0..=latitude_count {
        if latitude == 0 {
            rows.push(vec![mesh.add_vertex_transformed(
                sphere.v1 + Vector4::new3(0.0, 0.0, radius),
                Vector4::new3(0.0, 0.0, 1.0),
                sphere.col.clone(),
                transform,
            )]);
        } else if latitude == latitude_count {
            rows.push(vec![mesh.add_vertex_transformed(
                sphere.v1 + Vector4::new3(0.0, 0.0, -radius),
                Vector4::new3(0.0, 0.0, -1.0),
                sphere.col.clone(),
                transform,
            )]);
        } else {
            let theta = latitude as f64 * PI / latitude_count as f64;
            let ring_radius = radius * theta.sin();
            let z = radius * theta.cos();
            let longitude_count = ((ring_radius * 2.0 * PI / maximum_edge).ceil() as usize).max(3);
            let start_phi = (latitude % 2) as f64 * 3.0 / longitude_count as f64;
            let mut row = Vec::with_capacity(longitude_count);
            for longitude in 0..longitude_count {
                let phi = longitude as f64 * 2.0 * PI / longitude_count as f64 + start_phi;
                let local = Vector4::new3(ring_radius * phi.cos(), ring_radius * phi.sin(), z);
                let normal = local.scale(1.0 / radius);
                row.push(mesh.add_vertex_transformed(
                    sphere.v1 + local,
                    normal,
                    sphere.col.clone(),
                    transform,
                ));
            }
            rows.push(row);
        }
    }

    for latitude in 0..latitude_count {
        if latitude == 0 {
            let pivot = rows[0][0];
            let next = &rows[1];
            for index in 0..next.len() {
                mesh.add_face(
                    pivot,
                    next[index],
                    next[(index + 1) % next.len()],
                    MeshFace::MFMOD_SPHERE,
                );
            }
        } else if latitude == latitude_count - 1 {
            let pivot = rows[latitude_count][0];
            let current = &rows[latitude_count - 1];
            for index in 0..current.len() {
                mesh.add_face(
                    current[(index + 1) % current.len()],
                    current[index],
                    pivot,
                    MeshFace::MFMOD_SPHERE,
                );
            }
        } else {
            stitch_sphere_rows(mesh, &rows[latitude], &rows[latitude + 1]);
        }
    }
}

fn stitch_sphere_rows<C>(mesh: &mut GrowMesh<C>, first: &[usize], second: &[usize]) {
    let mut first_index = 0;
    let mut second_index = 0;
    while first_index < first.len() || second_index < second.len() {
        let current_first = first[first_index % first.len()];
        let current_second = second[second_index % second.len()];
        let next_first = first[(first_index + 1) % first.len()];
        let next_second = second[(second_index + 1) % second.len()];
        let choose_first = if first_index + 1 > first.len() {
            false
        } else if second_index + 1 > second.len() {
            true
        } else {
            select_sphere_triangle(mesh, current_first, current_second, next_first, next_second)
        };
        if choose_first {
            mesh.add_face(
                current_first,
                current_second,
                next_first,
                MeshFace::MFMOD_SPHERE,
            );
            first_index += 1;
        } else {
            mesh.add_face(
                current_first,
                current_second,
                next_second,
                MeshFace::MFMOD_SPHERE,
            );
            second_index += 1;
        }
    }
}

fn select_sphere_triangle<C>(
    mesh: &GrowMesh<C>,
    first: usize,
    second: usize,
    next_first: usize,
    next_second: usize,
) -> bool {
    let v_first = mesh.vertex(first).v;
    let v_second = mesh.vertex(second).v;
    let v_next_first = mesh.vertex(next_first).v;
    let v_next_second = mesh.vertex(next_second).v;
    let normal_first = triangle_normal(v_first, v_second, v_next_first);
    let normal_second = triangle_normal(v_first, v_second, v_next_second);
    let det_first = normal_first.dot3d(v_next_second - v_second);
    let det_second = normal_second.dot3d(v_next_first - v_first);
    det_first < 0.0 && det_second >= 0.0
}

fn triangle_normal(first: Vector4, second: Vector4, third: Vector4) -> Vector4 {
    (second - first)
        .cross(third - second)
        .normalize3d()
        .unwrap_or(Vector4::ZERO)
}

fn basis_matrix(axis: Vector4, radial: Vector4) -> Matrix4 {
    let tangent = axis.cross(radial);
    let mut matrix = Matrix4::IDENTITY;
    matrix.values[0][0] = radial.x;
    matrix.values[1][0] = radial.y;
    matrix.values[2][0] = radial.z;
    matrix.values[0][1] = tangent.x;
    matrix.values[1][1] = tangent.y;
    matrix.values[2][1] = tangent.z;
    matrix.values[0][2] = axis.x;
    matrix.values[1][2] = axis.y;
    matrix.values[2][2] = axis.z;
    matrix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_test_vertex(mesh: &mut GrowMesh<u32>, x: f64, y: f64) {
        mesh.add_vertex(
            Vector4::new3(x, y, 0.0),
            Vector4::new3(0.0, 0.0, 1.0),
            0xff00_00ff,
        );
    }

    #[test]
    fn grow_mesh_builds_reference_triangle_modes() {
        let mut triangles = GrowMesh::new();
        triangles.start(GrowMesh::<u32>::GM_TRIGS);
        for index in 0..6 {
            add_test_vertex(&mut triangles, index as f64, 0.0);
        }
        triangles.end();
        assert_eq!(triangles.face_size(), 2);
        assert_eq!(triangles.face(1).indices(), [3, 4, 5]);

        let mut strip = GrowMesh::new();
        strip.start(GrowMesh::<u32>::GM_TRIGSTRIP);
        for (x, y) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
            add_test_vertex(&mut strip, x, y);
        }
        strip.end();
        assert_eq!(strip.faces()[0].indices(), [0, 1, 2]);
        assert_eq!(strip.faces()[1].indices(), [3, 2, 1]);

        let mut fan = GrowMesh::new();
        fan.start(GrowMesh::<u32>::GM_TRIGFAN);
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            add_test_vertex(&mut fan, x, y);
        }
        fan.end();
        assert_eq!(fan.faces()[0].indices(), [1, 2, 0]);
        assert_eq!(fan.faces()[1].indices(), [2, 3, 0]);
    }

    #[test]
    fn transformed_vertex_uses_point_and_direction_w_components() {
        let mut mesh = GrowMesh::new();
        let transform = Matrix4::make_trans_mat(Vector4::new3(5.0, 6.0, 7.0));
        mesh.add_vertex_transformed(
            Vector4::new3(1.0, 2.0, 3.0),
            Vector4::new3(0.0, 0.0, 1.0),
            7_u32,
            transform,
        );
        assert_eq!(mesh.vertex(0).v, Vector4::new4(6.0, 8.0, 10.0, 1.0));
        assert_eq!(mesh.vertex(0).n, Vector4::new3(0.0, 0.0, 1.0));
        assert!(mesh.vertex(0).is_finite());
    }

    #[test]
    fn cylinder_list_matches_reference_detail_and_cap_counts() {
        let mut cylinders = CylinderList::default();
        cylinders.add(
            Vector4::new3(0.0, 0.0, 2.0),
            Vector4::ZERO,
            0.5,
            0.25,
            0xff00_00ff_u32,
            1,
            false,
            None,
        );
        let mut mesh = GrowMesh::new();
        cylinders.make_mesh(&mut mesh);
        assert_eq!(mesh.vertex_size(), 8);
        assert_eq!(mesh.face_size(), 8);
        assert!(mesh
            .vertices()
            .iter()
            .all(|vertex| vertex.v.z.abs() < 1.0e-12 || (vertex.v.z - 2.0).abs() < 1.0e-12));
        assert!(mesh
            .faces()
            .iter()
            .all(|face| face.nmode == MeshFace::MFMOD_NORGLN));

        let mut capped = CylinderList::default();
        capped.add(
            Vector4::new3(0.0, 0.0, 2.0),
            Vector4::ZERO,
            0.5,
            0.25,
            1_u32,
            1,
            true,
            None,
        );
        let mut mesh = GrowMesh::new();
        capped.make_mesh(&mut mesh);
        assert_eq!(mesh.vertex_size(), 18);
        assert_eq!(mesh.face_size(), 18);
        assert!(mesh
            .faces()
            .iter()
            .all(|face| face.nmode == MeshFace::MFMOD_CYL));
    }

    #[test]
    fn sphere_list_generates_closed_reference_mesh() {
        let mut spheres = SphereList::default();
        spheres.add(Vector4::new3(2.0, 3.0, 4.0), 1.0, 42_u32, 4, None);
        let mut mesh = GrowMesh::new();
        spheres.make_mesh(&mut mesh, None);

        assert_eq!(mesh.vertex_size(), 28);
        assert_eq!(mesh.face_size(), 52);
        assert!(mesh
            .faces()
            .iter()
            .all(|face| face.nmode == MeshFace::MFMOD_SPHERE));
        assert!(mesh.faces().iter().all(|face| face
            .indices()
            .iter()
            .all(|index| *index < mesh.vertex_size())));
        for vertex in mesh.vertices() {
            let distance = (vertex.v - Vector4::new4(2.0, 3.0, 4.0, 1.0)).length3d();
            assert!((distance - 1.0).abs() < 1.0e-10);
            assert!((vertex.n.length3d() - 1.0).abs() < 1.0e-10);
        }
    }

    #[test]
    fn per_sphere_transform_overrides_list_transform() {
        let own = Matrix4::make_trans_mat(Vector4::new3(10.0, 0.0, 0.0));
        let external = Matrix4::make_trans_mat(Vector4::new3(0.0, 20.0, 0.0));
        let mut spheres = SphereList::default();
        spheres.add(Vector4::ZERO, 1.0, 1_u32, 2, Some(own));
        let mut mesh = GrowMesh::new();
        spheres.make_mesh(&mut mesh, Some(external));
        assert!((mesh.vertex(0).v.x - 10.0).abs() < 1.0e-12);
        assert!(mesh.vertex(0).v.y.abs() < 1.0e-12);
    }
}
