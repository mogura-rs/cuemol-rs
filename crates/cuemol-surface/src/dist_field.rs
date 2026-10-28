//! Distance-field molecular surfaces and standalone marching cubes.
//!
//! This ports C++ `DistMapMarchingCubes` and `DistFieldSurfBuilder`, including
//! welded grid-edge vertices, atom-id propagation, and the two-pass SES path.

use crate::{SurfaceMesh, SurfaceVertex};
use cuemol_core::{parallel_for, Vector4};
use cuemol_gfx::{
    CUBE_EDGE_CONNECTION, CUBE_EDGE_FLAGS, CUBE_VERTEX_OFFSET, TRIANGLE_CONNECTION_TABLE,
};
use std::collections::HashMap;
use std::sync::OnceLock;

const FAR_OUTSIDE: f32 = 1.0e6;

fn mesh_float_vector(value: Vector4) -> Vector4 {
    Vector4::new3(
        value.x as f32 as f64,
        value.y as f32 as f64,
        value.z as f32 as f64,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridEdgeKey {
    lower: [usize; 3],
    axis: usize,
}

/// Standalone marching cubes over a regular scalar grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DistMapMarchingCubes {
    data: Vec<f32>,
    id_field: Option<Vec<i32>>,
    dims: [usize; 3],
    iso_level: f32,
    mesh: SurfaceMesh,
}

impl DistMapMarchingCubes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copy a scalar field using C++ layout `(i * ny + j) * nz + k`.
    pub fn set_field(&mut self, data: &[f32], nx: usize, ny: usize, nz: usize) {
        let expected = nx.saturating_mul(ny).saturating_mul(nz);
        if expected != data.len() {
            self.data.clear();
            self.id_field = None;
            self.dims = [0; 3];
            return;
        }
        self.data.clear();
        self.data.extend_from_slice(data);
        self.id_field = None;
        self.dims = [nx, ny, nz];
    }

    pub fn set_id_field(&mut self, ids: &[i32]) {
        self.id_field = (ids.len() == self.data.len()).then(|| ids.to_vec());
    }

    pub fn clear_id_field(&mut self) {
        self.id_field = None;
    }

    pub fn set_iso_level(&mut self, level: f32) {
        self.iso_level = level;
    }

    pub fn iso_level(&self) -> f32 {
        self.iso_level
    }

    pub fn vertices(&self) -> &[SurfaceVertex] {
        self.mesh.vertices()
    }

    pub fn get_verts(&self) -> &[SurfaceVertex] {
        self.vertices()
    }

    pub fn faces(&self) -> &[[u32; 3]] {
        self.mesh.triangles()
    }

    pub fn get_faces(&self) -> &[[u32; 3]] {
        self.faces()
    }

    pub fn mesh(&self) -> &SurfaceMesh {
        &self.mesh
    }

    pub fn build(&mut self) {
        self.mesh.clear();
        let [nx, ny, nz] = self.dims;
        if self.data.len() != nx.saturating_mul(ny).saturating_mul(nz) || nx < 2 || ny < 2 || nz < 2
        {
            return;
        }

        let slabs = (0..nx - 1).map(|_| OnceLock::new()).collect::<Vec<_>>();
        parallel_for(0, nx - 1, |i| {
            let mut triangles = Vec::<[GridEdgeKey; 3]>::new();
            for j in 0..ny - 1 {
                for k in 0..nz - 1 {
                    let mut mask = 0usize;
                    for (corner, offset) in CUBE_VERTEX_OFFSET.iter().enumerate() {
                        if self.value_at(i + offset[0], j + offset[1], k + offset[2])
                            <= self.iso_level
                        {
                            mask |= 1 << corner;
                        }
                    }
                    if CUBE_EDGE_FLAGS[mask] == 0 {
                        continue;
                    }

                    let table = &TRIANGLE_CONNECTION_TABLE[mask];
                    let mut index = 0;
                    while table[index] >= 0 {
                        triangles.push([
                            grid_edge_key([i, j, k], table[index] as usize),
                            grid_edge_key([i, j, k], table[index + 1] as usize),
                            grid_edge_key([i, j, k], table[index + 2] as usize),
                        ]);
                        index += 3;
                    }
                }
            }
            slabs[i]
                .set(triangles)
                .expect("marching-cubes slab is initialized once");
        });

        let mut edge_cache = HashMap::<GridEdgeKey, u32>::new();
        for slab in slabs {
            for keys in slab
                .into_inner()
                .expect("parallel marching-cubes slab was initialized")
            {
                let triangle = keys.map(|key| {
                    if let Some(&id) = edge_cache.get(&key) {
                        return id;
                    }
                    let id = self.mesh.vertex_count() as u32;
                    let vertex = self.make_vertex(key);
                    self.mesh.push_vertex(vertex);
                    edge_cache.insert(key, id);
                    id
                });
                if triangle[0] != triangle[1]
                    && triangle[1] != triangle[2]
                    && triangle[0] != triangle[2]
                {
                    self.mesh.push_triangle(triangle);
                }
            }
        }
    }

    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.dims[1] + j) * self.dims[2] + k
    }

    fn value_at(&self, i: usize, j: usize, k: usize) -> f32 {
        self.data[self.index(i, j, k)]
    }

    fn gradient_at(&self, i: usize, j: usize, k: usize) -> Vector4 {
        let [nx, ny, nz] = self.dims;
        let im = i.saturating_sub(1);
        let ip = (i + 1).min(nx - 1);
        let jm = j.saturating_sub(1);
        let jp = (j + 1).min(ny - 1);
        let km = k.saturating_sub(1);
        let kp = (k + 1).min(nz - 1);
        Vector4::new3(
            (self.value_at(ip, j, k) - self.value_at(im, j, k)) as f64,
            (self.value_at(i, jp, k) - self.value_at(i, jm, k)) as f64,
            (self.value_at(i, j, kp) - self.value_at(i, j, km)) as f64,
        )
    }

    fn make_vertex(&self, key: GridEdgeKey) -> SurfaceVertex {
        let g0 = key.lower;
        let mut g1 = g0;
        g1[key.axis] += 1;
        let v0 = self.value_at(g0[0], g0[1], g0[2]);
        let v1 = self.value_at(g1[0], g1[1], g1[2]);
        let delta = v1 - v0;
        let t = if delta == 0.0 {
            0.5
        } else {
            ((self.iso_level - v0) / delta).clamp(0.0, 1.0)
        } as f64;
        let p0 = Vector4::new3(g0[0] as f64, g0[1] as f64, g0[2] as f64);
        let p1 = Vector4::new3(g1[0] as f64, g1[1] as f64, g1[2] as f64);
        let n0 = self.gradient_at(g0[0], g0[1], g0[2]);
        let n1 = self.gradient_at(g1[0], g1[1], g1[2]);
        let normal = n0 + (n1 - n0) * t;
        let length = normal.length3d();
        let normal = if length > 1.0e-10 {
            normal / length
        } else {
            Vector4::new3(0.0, 0.0, 1.0)
        };
        let info = self
            .id_field
            .as_ref()
            .map(|ids| {
                let source = if v0 <= v1 { g0 } else { g1 };
                ids[self.index(source[0], source[1], source[2])] as u32
            })
            .unwrap_or(0);
        SurfaceVertex {
            position: mesh_float_vector(p0 + (p1 - p0) * t),
            normal: mesh_float_vector(normal),
            info,
        }
    }
}

fn grid_edge_key(cell: [usize; 3], edge: usize) -> GridEdgeKey {
    let [corner0, corner1] = CUBE_EDGE_CONNECTION[edge];
    let offset0 = CUBE_VERTEX_OFFSET[corner0];
    let offset1 = CUBE_VERTEX_OFFSET[corner1];
    let grid0 = [
        cell[0] + offset0[0],
        cell[1] + offset0[1],
        cell[2] + offset0[2],
    ];
    let grid1 = [
        cell[0] + offset1[0],
        cell[1] + offset1[1],
        cell[2] + offset1[2],
    ];
    let axis = (0..3)
        .find(|&axis| grid0[axis] != grid1[axis])
        .expect("marching-cubes edge has distinct endpoints");
    GridEdgeKey {
        lower: [
            grid0[0].min(grid1[0]),
            grid0[1].min(grid1[1]),
            grid0[2].min(grid1[2]),
        ],
        axis,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DistanceFieldAtom {
    position: Vector4,
    radius: f64,
    id: i32,
}

/// Molecular surface builder based on a signed distance field.
#[derive(Debug, Clone, PartialEq)]
pub struct DistFieldSurfBuilder {
    probe_radius: f64,
    grid_spacing: f64,
    surface_type: i32,
    atoms: Vec<DistanceFieldAtom>,
    field: Vec<f32>,
    id_grid: Vec<i32>,
    dims: [usize; 3],
    origin: Vector4,
    mesh: SurfaceMesh,
}

impl Default for DistFieldSurfBuilder {
    fn default() -> Self {
        Self {
            probe_radius: 1.4,
            grid_spacing: 0.5,
            surface_type: Self::SURF_SES,
            atoms: Vec::new(),
            field: Vec::new(),
            id_grid: Vec::new(),
            dims: [0; 3],
            origin: Vector4::ZERO,
            mesh: SurfaceMesh::new(),
        }
    }
}

impl DistFieldSurfBuilder {
    pub const SURF_VDW: i32 = 0;
    pub const SURF_SAS: i32 = 1;
    pub const SURF_SES: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_probe_radius(&mut self, radius: f64) {
        self.probe_radius = radius;
    }

    pub fn probe_radius(&self) -> f64 {
        self.probe_radius
    }

    pub fn get_probe_radius(&self) -> f64 {
        self.probe_radius()
    }

    pub fn set_grid_spacing(&mut self, spacing: f64) {
        self.grid_spacing = spacing;
    }

    pub fn grid_spacing(&self) -> f64 {
        self.grid_spacing
    }

    pub fn get_grid_spacing(&self) -> f64 {
        self.grid_spacing()
    }

    pub fn set_surface_type(&mut self, surface_type: i32) {
        self.surface_type = surface_type;
    }

    pub fn set_surf_type(&mut self, surface_type: i32) {
        self.set_surface_type(surface_type);
    }

    pub fn surface_type(&self) -> i32 {
        self.surface_type
    }

    pub fn get_surf_type(&self) -> i32 {
        self.surface_type()
    }

    pub fn clear_atoms(&mut self) {
        self.atoms.clear();
    }

    pub fn add_atom(&mut self, position: Vector4, radius: f64, id: i32) {
        self.atoms.push(DistanceFieldAtom {
            position,
            radius,
            id,
        });
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn get_atom_count(&self) -> usize {
        self.atom_count()
    }

    pub fn vertices(&self) -> &[SurfaceVertex] {
        self.mesh.vertices()
    }

    pub fn get_verts(&self) -> &[SurfaceVertex] {
        self.vertices()
    }

    pub fn faces(&self) -> &[[u32; 3]] {
        self.mesh.triangles()
    }

    pub fn get_faces(&self) -> &[[u32; 3]] {
        self.faces()
    }

    pub fn mesh(&self) -> &SurfaceMesh {
        &self.mesh
    }

    pub fn into_mesh(self) -> SurfaceMesh {
        self.mesh
    }

    pub fn grid_dims(&self) -> [usize; 3] {
        self.dims
    }

    pub fn get_grid_dim_x(&self) -> usize {
        self.dims[0]
    }

    pub fn get_grid_dim_y(&self) -> usize {
        self.dims[1]
    }

    pub fn get_grid_dim_z(&self) -> usize {
        self.dims[2]
    }

    pub fn build(&mut self) {
        self.mesh.clear();
        self.field.clear();
        self.id_grid.clear();
        self.dims = [0; 3];

        if self.atoms.is_empty()
            || !self.grid_spacing.is_finite()
            || self.grid_spacing <= 0.0
            || !self.probe_radius.is_finite()
            || self.probe_radius < 0.0
        {
            return;
        }

        let probe = self.probe_radius;
        let probe_for_pad = if self.surface_type == Self::SURF_VDW {
            0.0
        } else {
            probe
        };
        self.setup_grid(probe_for_pad);
        if self.dims.contains(&0) {
            return;
        }

        let centers = self
            .atoms
            .iter()
            .map(|atom| atom.position)
            .collect::<Vec<_>>();
        let ids = self.atoms.iter().map(|atom| atom.id).collect::<Vec<_>>();
        let offset = if self.surface_type == Self::SURF_VDW {
            0.0
        } else {
            probe
        };
        let radii = self
            .atoms
            .iter()
            .map(|atom| atom.radius + offset)
            .collect::<Vec<_>>();
        self.fill_field(&centers, &radii, &ids);

        let mut first_pass = DistMapMarchingCubes::new();
        first_pass.set_field(&self.field, self.dims[0], self.dims[1], self.dims[2]);
        first_pass.set_id_field(&self.id_grid);
        first_pass.build();

        if self.surface_type != Self::SURF_SES {
            self.emit_mesh(&first_pass, false);
            return;
        }
        if first_pass.vertices().is_empty() {
            return;
        }

        let second_centers = first_pass
            .vertices()
            .iter()
            .map(|vertex| {
                self.origin
                    + Vector4::new3(
                        vertex.position.x * self.grid_spacing,
                        vertex.position.y * self.grid_spacing,
                        vertex.position.z * self.grid_spacing,
                    )
            })
            .collect::<Vec<_>>();
        let second_radii = vec![probe; second_centers.len()];
        let second_ids = first_pass
            .vertices()
            .iter()
            .map(|vertex| vertex.info as i32)
            .collect::<Vec<_>>();
        self.fill_field(&second_centers, &second_radii, &second_ids);

        let mut second_pass = DistMapMarchingCubes::new();
        second_pass.set_field(&self.field, self.dims[0], self.dims[1], self.dims[2]);
        second_pass.set_id_field(&self.id_grid);
        second_pass.build();
        self.emit_mesh(&second_pass, true);
        self.prune_components(1.5 * probe);
    }

    fn setup_grid(&mut self, probe_for_pad: f64) {
        let spacing = self.grid_spacing;
        let mut lower = self.atoms[0].position;
        let mut upper = lower;
        let mut max_radius = 0.0_f64;
        for atom in &self.atoms {
            lower.x = lower.x.min(atom.position.x);
            lower.y = lower.y.min(atom.position.y);
            lower.z = lower.z.min(atom.position.z);
            upper.x = upper.x.max(atom.position.x);
            upper.y = upper.y.max(atom.position.y);
            upper.z = upper.z.max(atom.position.z);
            max_radius = max_radius.max(atom.radius);
        }
        let pad = max_radius + 2.0 * probe_for_pad + 3.0 * spacing;
        self.origin = Vector4::new3(lower.x - pad, lower.y - pad, lower.z - pad);
        self.dims = [
            ((upper.x - lower.x + 2.0 * pad) / spacing).ceil() as usize + 1,
            ((upper.y - lower.y + 2.0 * pad) / spacing).ceil() as usize + 1,
            ((upper.z - lower.z + 2.0 * pad) / spacing).ceil() as usize + 1,
        ];
    }

    fn fill_field(&mut self, centers: &[Vector4], radii: &[f64], ids: &[i32]) {
        #[derive(Debug, Clone, Copy)]
        struct GridBox {
            lower: [usize; 3],
            upper: [usize; 3],
        }

        let [nx, ny, nz] = self.dims;
        let spacing = self.grid_spacing;
        let origin = self.origin;
        let boxes = centers
            .iter()
            .zip(radii)
            .map(|(center, radius)| {
                let reach = radius + 2.0 * spacing;
                let lower = [
                    ((center.x - reach - origin.x) / spacing).floor() as isize,
                    ((center.y - reach - origin.y) / spacing).floor() as isize,
                    ((center.z - reach - origin.z) / spacing).floor() as isize,
                ];
                let upper = [
                    ((center.x + reach - origin.x) / spacing).ceil() as isize,
                    ((center.y + reach - origin.y) / spacing).ceil() as isize,
                    ((center.z + reach - origin.z) / spacing).ceil() as isize,
                ];
                GridBox {
                    lower: [
                        lower[0].clamp(0, nx as isize - 1) as usize,
                        lower[1].clamp(0, ny as isize - 1) as usize,
                        lower[2].clamp(0, nz as isize - 1) as usize,
                    ],
                    upper: [
                        upper[0].clamp(0, nx as isize - 1) as usize,
                        upper[1].clamp(0, ny as isize - 1) as usize,
                        upper[2].clamp(0, nz as isize - 1) as usize,
                    ],
                }
            })
            .collect::<Vec<_>>();

        let slabs = (0..nx).map(|_| OnceLock::new()).collect::<Vec<_>>();
        parallel_for(0, nx, |i| {
            let mut field = vec![FAR_OUTSIDE; ny * nz];
            let mut id_grid = vec![-1; ny * nz];
            let x = origin.x + i as f64 * spacing;
            for atom_index in 0..centers.len() {
                let bounds = boxes[atom_index];
                if i < bounds.lower[0] || i > bounds.upper[0] {
                    continue;
                }
                let center = centers[atom_index];
                let dx = x - center.x;
                let dx2 = dx * dx;
                for j in bounds.lower[1]..=bounds.upper[1] {
                    let dy = origin.y + j as f64 * spacing - center.y;
                    let dxy2 = dx2 + dy * dy;
                    for k in bounds.lower[2]..=bounds.upper[2] {
                        let dz = origin.z + k as f64 * spacing - center.z;
                        let value = (dxy2 + dz * dz).sqrt() - radii[atom_index];
                        let index = j * nz + k;
                        if (value as f32) < field[index] {
                            field[index] = value as f32;
                            id_grid[index] = ids[atom_index];
                        }
                    }
                }
            }
            slabs[i]
                .set((field, id_grid))
                .expect("distance-field slab is initialized once");
        });

        self.field = Vec::with_capacity(nx * ny * nz);
        self.id_grid = Vec::with_capacity(nx * ny * nz);
        for slab in slabs {
            let (field, ids) = slab
                .into_inner()
                .expect("parallel distance-field slab was initialized");
            self.field.extend(field);
            self.id_grid.extend(ids);
        }
    }

    fn emit_mesh(&mut self, marching: &DistMapMarchingCubes, negate_normals: bool) {
        let normal_sign = if negate_normals { -1.0 } else { 1.0 };
        let mut mesh = SurfaceMesh::new();
        for vertex in marching.vertices() {
            mesh.push_vertex(SurfaceVertex {
                position: mesh_float_vector(
                    self.origin
                        + Vector4::new3(
                            vertex.position.x * self.grid_spacing,
                            vertex.position.y * self.grid_spacing,
                            vertex.position.z * self.grid_spacing,
                        ),
                ),
                normal: vertex.normal * normal_sign,
                info: vertex.info,
            });
        }
        for &face in marching.faces() {
            mesh.push_triangle(face);
        }
        self.mesh = mesh;
    }

    fn nearest_atom_residual(&self, position: Vector4) -> f64 {
        self.atoms
            .iter()
            .map(|atom| (position - atom.position).length3d() - atom.radius)
            .fold(f64::INFINITY, f64::min)
    }

    fn prune_components(&mut self, max_residual: f64) {
        let vertex_count = self.mesh.vertex_count();
        if vertex_count == 0 {
            return;
        }

        let mut parent = (0..vertex_count).collect::<Vec<_>>();
        for &[a, b, c] in self.mesh.triangles() {
            union(&mut parent, a as usize, b as usize);
            union(&mut parent, b as usize, c as usize);
        }

        let mut decided = vec![false; vertex_count];
        let mut keep = vec![false; vertex_count];
        for index in 0..vertex_count {
            let root = root(&mut parent, index);
            if !decided[root] {
                decided[root] = true;
                keep[root] =
                    self.nearest_atom_residual(self.mesh.vertices()[index].position) < max_residual;
            }
        }

        let mut mapping = vec![None; vertex_count];
        let mut compacted = SurfaceMesh::new();
        for (index, mapped) in mapping.iter_mut().enumerate() {
            let root = root(&mut parent, index);
            if keep[root] {
                *mapped = Some(compacted.vertex_count() as u32);
                compacted.push_vertex(self.mesh.vertices()[index].clone());
            }
        }
        for &[a, b, c] in self.mesh.triangles() {
            let (Some(a), Some(b), Some(c)) = (
                mapping[a as usize],
                mapping[b as usize],
                mapping[c as usize],
            ) else {
                continue;
            };
            compacted.push_triangle([a, b, c]);
        }
        self.mesh = compacted;
    }
}

fn root(parent: &mut [usize], mut value: usize) -> usize {
    while parent[value] != value {
        parent[value] = parent[parent[value]];
        value = parent[value];
    }
    value
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = root(parent, left);
    let right = root(parent, right);
    if left != right {
        parent[left] = right;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent_x(vertices: &[SurfaceVertex]) -> f64 {
        vertices
            .iter()
            .map(|vertex| vertex.position.x.abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn single_sphere_vdw_and_sas_match_reference_radii() {
        let mut builder = DistFieldSurfBuilder::new();
        builder.set_grid_spacing(0.3);
        builder.set_probe_radius(1.4);
        builder.add_atom(Vector4::ZERO, 2.0, 37);

        builder.set_surface_type(DistFieldSurfBuilder::SURF_VDW);
        builder.build();
        assert!(builder.vertices().len() > 100);
        assert!((extent_x(builder.vertices()) - 2.0).abs() < 0.6);
        assert!(builder.vertices().iter().all(|vertex| vertex.info == 37));

        builder.set_surface_type(DistFieldSurfBuilder::SURF_SAS);
        builder.build();
        assert!(builder.vertices().len() > 100);
        assert!((extent_x(builder.vertices()) - 3.4).abs() < 0.6);
        assert!(builder
            .vertices()
            .iter()
            .all(|vertex| vertex.normal.dot3d(vertex.position) > 0.0));
    }

    #[test]
    fn single_sphere_ses_is_the_vdw_surface() {
        let mut builder = DistFieldSurfBuilder::new();
        builder.set_surface_type(DistFieldSurfBuilder::SURF_SES);
        builder.set_grid_spacing(0.35);
        builder.set_probe_radius(1.4);
        builder.add_atom(Vector4::ZERO, 2.0, 5);
        builder.build();

        assert!(builder.vertices().len() > 100);
        assert!((extent_x(builder.vertices()) - 2.0).abs() < 0.7);
        assert!(builder
            .vertices()
            .iter()
            .all(|vertex| vertex.normal.dot3d(vertex.position) > 0.0));
    }

    #[test]
    fn two_atom_ses_stays_between_vdw_and_accessible_surfaces() {
        let centers = [Vector4::ZERO, Vector4::new3(3.0, 0.0, 0.0)];
        let radius = 2.0;
        let probe = 1.4;
        let spacing = 0.35;
        let mut builder = DistFieldSurfBuilder::new();
        builder.set_surface_type(DistFieldSurfBuilder::SURF_SES);
        builder.set_grid_spacing(spacing);
        builder.set_probe_radius(probe);
        builder.add_atom(centers[0], radius, 10);
        builder.add_atom(centers[1], radius, 11);
        builder.build();

        assert!(builder.vertices().len() > 200);
        assert!(builder.grid_dims().iter().all(|&dimension| dimension > 2));
        for vertex in builder.vertices() {
            let nearest = centers
                .iter()
                .map(|center| (vertex.position - *center).length3d())
                .fold(f64::INFINITY, f64::min);
            assert!(nearest > radius - 2.0 * spacing);
            assert!(nearest < radius + probe + 2.0 * spacing);
            assert!(vertex.info == 10 || vertex.info == 11);
        }

        builder.clear_atoms();
        builder.build();
        assert!(builder.mesh().is_empty());
        assert_eq!(builder.grid_dims(), [0; 3]);
    }

    #[test]
    fn marching_cubes_welds_vertices_and_propagates_ids() {
        let dims = [5, 5, 5];
        let mut values = Vec::new();
        let mut ids = Vec::new();
        for i in 0..dims[0] {
            for j in 0..dims[1] {
                for k in 0..dims[2] {
                    let dx = i as f32 - 2.0;
                    let dy = j as f32 - 2.0;
                    let dz = k as f32 - 2.0;
                    values.push((dx * dx + dy * dy + dz * dz).sqrt() - 1.5);
                    ids.push(91);
                }
            }
        }

        let mut marching = DistMapMarchingCubes::new();
        marching.set_field(&values, dims[0], dims[1], dims[2]);
        marching.set_id_field(&ids);
        marching.build();
        assert!(!marching.vertices().is_empty());
        assert!(!marching.faces().is_empty());
        assert!(marching.vertices().iter().all(|vertex| vertex.info == 91));
        assert!(marching.faces().iter().all(|face| {
            face.iter()
                .all(|&index| (index as usize) < marching.vertices().len())
                && face[0] != face[1]
                && face[1] != face[2]
                && face[0] != face[2]
        }));
        assert!(marching.vertices().len() < marching.faces().len() * 2);
    }
}
