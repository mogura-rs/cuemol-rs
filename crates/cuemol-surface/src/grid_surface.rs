//! Grid-based molecular surface generation.
//!
//! Two surfaces, both extracted by marching tetrahedra over a signed grid field:
//! - [`build_sphere_union_surface`] — the union-of-spheres signed-distance field
//!   `f(p) = min_i(|p - c_i| - r_i)`, i.e. the van-der-Waals / solvent-ACCESSIBLE
//!   surface (each atom a sphere of `vdw + probe`).
//! - [`build_ses_surface`] — the solvent-EXCLUDED (Connolly) surface: the probe
//!   is rolled by computing the distance transform to the accessible region and
//!   contouring `dist == probe`, which carves the reentrant patches in crevices
//!   that the union-of-spheres surface cannot represent.
//!
//! Both are real, smooth, closed surfaces — a large step up from the original
//! one-octahedron-per-atom placeholder.

use cuemol_core::Vector4;
use std::collections::HashMap;

use crate::{SurfaceMesh, SurfaceVertex};

/// One atom sphere contributing to the surface field.
#[derive(Debug, Clone, Copy)]
pub struct SurfSphere {
    pub center: Vector4,
    pub radius: f64,
}

/// Uniform spatial hash of spheres so each grid sample only tests nearby atoms
/// instead of all of them (the field is a `min` over spheres, dominated by the
/// closest few). Bucket edge = the largest radius plus a margin, so the nearest
/// surface always lies within the 27 neighbouring buckets.
struct SphereGrid<'a> {
    spheres: &'a [SurfSphere],
    buckets: HashMap<[i64; 3], Vec<usize>>,
    inv_cell: f64,
}

impl<'a> SphereGrid<'a> {
    fn new(spheres: &'a [SurfSphere], cell: f64) -> Self {
        let inv_cell = 1.0 / cell;
        let mut buckets: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
        for (index, sphere) in spheres.iter().enumerate() {
            buckets
                .entry(Self::key(sphere.center, inv_cell))
                .or_default()
                .push(index);
        }
        Self {
            spheres,
            buckets,
            inv_cell,
        }
    }

    fn key(p: Vector4, inv_cell: f64) -> [i64; 3] {
        [
            (p.x * inv_cell).floor() as i64,
            (p.y * inv_cell).floor() as i64,
            (p.z * inv_cell).floor() as i64,
        ]
    }

    /// Signed distance to the nearest sphere surface (negative inside any sphere).
    fn field(&self, p: Vector4) -> f64 {
        let [kx, ky, kz] = Self::key(p, self.inv_cell);
        let mut best = f64::INFINITY;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(indices) = self.buckets.get(&[kx + dx, ky + dy, kz + dz]) {
                        for &i in indices {
                            let s = self.spheres[i];
                            let d = dist(p, s.center) - s.radius;
                            if d < best {
                                best = d;
                            }
                        }
                    }
                }
            }
        }
        best
    }
}

fn dist(a: Vector4, b: Vector4) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn at(x: f64, y: f64, z: f64) -> Vector4 {
    Vector4::new4(x, y, z, 1.0)
}

/// Build the union-of-spheres molecular surface into `mesh`, sampling the field
/// at the given grid `spacing` (Angstrom). Vertices carry outward normals (the
/// normalised field gradient) and are welded so shared edges are not duplicated.
pub fn build_sphere_union_surface(mesh: &mut SurfaceMesh, spheres: &[SurfSphere], spacing: f64) {
    if spheres.is_empty() || spacing <= 0.0 {
        return;
    }

    let max_radius = spheres.iter().fold(0.0_f64, |m, s| m.max(s.radius));
    let margin = max_radius + spacing * 2.0;

    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for s in spheres {
        for (axis, value) in [s.center.x, s.center.y, s.center.z].into_iter().enumerate() {
            lo[axis] = lo[axis].min(value);
            hi[axis] = hi[axis].max(value);
        }
    }
    let origin = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let counts = [
        ((hi[0] + margin - origin[0]) / spacing).ceil() as usize + 1,
        ((hi[1] + margin - origin[1]) / spacing).ceil() as usize + 1,
        ((hi[2] + margin - origin[2]) / spacing).ceil() as usize + 1,
    ];

    let grid = SphereGrid::new(spheres, max_radius + spacing);
    let coord = |i: usize, axis: usize| origin[axis] + i as f64 * spacing;

    // The sphere field and the marching are now driven by the generic grid
    // isosurface mesher: each node value is the signed sphere field, each node
    // position is its lattice point, and the field supplies outward gradients.
    build_grid_isosurface(
        mesh,
        &grid,
        counts,
        |ix, iy, iz| grid.field(at(coord(ix, 0), coord(iy, 1), coord(iz, 2))),
        |ix, iy, iz| at(coord(ix, 0), coord(iy, 1), coord(iz, 2)),
        spacing,
    );
}

/// A signed scalar field over Cartesian space whose `f < 0` region is the
/// interior of the surface and whose gradient points *outward* (toward
/// increasing `f`). [`build_grid_isosurface`] uses the gradient to orient
/// triangles and as the per-vertex normal, so the field source decides how the
/// gradient is sampled (analytic for spheres, grid central-differences for
/// density maps).
pub trait ScalarField {
    /// Outward-pointing gradient of the field at Cartesian point `p`.
    fn gradient(&self, p: Vector4) -> Vector4;
}

impl ScalarField for SphereGrid<'_> {
    fn gradient(&self, p: Vector4) -> Vector4 {
        // Central difference of the union-of-spheres field; the field is smooth,
        // so a small step is fine. Points radially outward (toward +f).
        let h = 1.0e-3;
        let dx = self.field(at(p.x + h, p.y, p.z)) - self.field(at(p.x - h, p.y, p.z));
        let dy = self.field(at(p.x, p.y + h, p.z)) - self.field(at(p.x, p.y - h, p.z));
        let dz = self.field(at(p.x, p.y, p.z + h)) - self.field(at(p.x, p.y, p.z - h));
        Vector4::new3(dx, dy, dz)
    }
}

// Cube corner offsets (unit cube), indices 0..8.
const CUBE_CORNERS: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];
// Six tetrahedra sharing the main diagonal 0-6 (table-free, no ambiguous cases).
const CUBE_TETS: [[usize; 4]; 6] = [
    [0, 1, 2, 6],
    [0, 2, 3, 6],
    [0, 3, 7, 6],
    [0, 7, 4, 6],
    [0, 4, 5, 6],
    [0, 5, 1, 6],
];

/// Extract the `f = 0` isosurface of a signed field sampled on a regular index
/// grid `[0..dims]` into `mesh` by marching tetrahedra.
///
/// `value(i, j, k)` returns the exact signed field at node `(i, j, k)` (negative
/// inside the surface); `position(i, j, k)` maps a node to its Cartesian
/// coordinate (so non-uniform / sheared grids are handled by the caller); and
/// `field` supplies the outward gradient used for vertex normals and triangle
/// winding. `weld_scale` is the characteristic node spacing used to weld shared
/// crossing points.
///
/// This is the shared core behind both the molecular union-of-spheres surface
/// and the crystallographic density isosurface (`xtal::MapSurfRenderer`).
pub fn build_grid_isosurface<F, V, P>(
    mesh: &mut SurfaceMesh,
    field: &F,
    dims: [usize; 3],
    value: V,
    position: P,
    weld_scale: f64,
) where
    F: ScalarField,
    V: Fn(usize, usize, usize) -> f64,
    P: Fn(usize, usize, usize) -> Vector4,
{
    if dims[0] < 2 || dims[1] < 2 || dims[2] < 2 {
        return;
    }

    // Precompute node field values once (each interior node is shared by up to
    // eight cubes); a flat array is faster than re-sampling or hashing.
    let stride_y = dims[0];
    let stride_z = dims[0] * dims[1];
    let mut vals = vec![0.0_f64; dims[0] * dims[1] * dims[2]];
    for iz in 0..dims[2] {
        for iy in 0..dims[1] {
            for ix in 0..dims[0] {
                vals[ix + iy * stride_y + iz * stride_z] = value(ix, iy, iz);
            }
        }
    }

    let mut welder = VertexWelder::new(weld_scale);

    for iz in 0..dims[2] - 1 {
        for iy in 0..dims[1] - 1 {
            for ix in 0..dims[0] - 1 {
                let mut corner_pos = [Vector4::ZERO; 8];
                let mut corner_val = [0.0_f64; 8];
                for (c, off) in CUBE_CORNERS.iter().enumerate() {
                    let (cx, cy, cz) = (ix + off[0], iy + off[1], iz + off[2]);
                    corner_pos[c] = position(cx, cy, cz);
                    corner_val[c] = vals[cx + cy * stride_y + cz * stride_z];
                }
                for tet in &CUBE_TETS {
                    march_tetra(
                        [
                            corner_pos[tet[0]],
                            corner_pos[tet[1]],
                            corner_pos[tet[2]],
                            corner_pos[tet[3]],
                        ],
                        [
                            corner_val[tet[0]],
                            corner_val[tet[1]],
                            corner_val[tet[2]],
                            corner_val[tet[3]],
                        ],
                        field,
                        &mut welder,
                        mesh,
                    );
                }
            }
        }
    }
}

/// Emit the isosurface triangles for one tetrahedron (isolevel 0). Inside = field
/// < 0. Crossing points on each inside/outside edge are interpolated; triangles
/// are wound so their normal points outward (toward increasing field).
fn march_tetra<F: ScalarField>(
    pos: [Vector4; 4],
    val: [f64; 4],
    field: &F,
    welder: &mut VertexWelder,
    mesh: &mut SurfaceMesh,
) {
    let inside: Vec<usize> = (0..4).filter(|&i| val[i] < 0.0).collect();
    let outside: Vec<usize> = (0..4).filter(|&i| val[i] >= 0.0).collect();
    let crossings: Vec<Vector4> = match (inside.len(), outside.len()) {
        (1, 3) | (3, 1) => {
            let (lone, others) = if inside.len() == 1 {
                (inside[0], outside.clone())
            } else {
                (outside[0], inside.clone())
            };
            others
                .iter()
                .map(|&o| edge_point(pos[lone], val[lone], pos[o], val[o]))
                .collect()
        }
        (2, 2) => {
            // Quad in cyclic order so consecutive points share a tet vertex.
            vec![
                edge_point(
                    pos[inside[0]],
                    val[inside[0]],
                    pos[outside[0]],
                    val[outside[0]],
                ),
                edge_point(
                    pos[inside[0]],
                    val[inside[0]],
                    pos[outside[1]],
                    val[outside[1]],
                ),
                edge_point(
                    pos[inside[1]],
                    val[inside[1]],
                    pos[outside[1]],
                    val[outside[1]],
                ),
                edge_point(
                    pos[inside[1]],
                    val[inside[1]],
                    pos[outside[0]],
                    val[outside[0]],
                ),
            ]
        }
        _ => return, // all inside or all outside: no surface
    };

    if crossings.len() == 3 {
        emit_triangle(
            [crossings[0], crossings[1], crossings[2]],
            field,
            welder,
            mesh,
        );
    } else if crossings.len() == 4 {
        emit_triangle(
            [crossings[0], crossings[1], crossings[2]],
            field,
            welder,
            mesh,
        );
        emit_triangle(
            [crossings[0], crossings[2], crossings[3]],
            field,
            welder,
            mesh,
        );
    }
}

/// Linear interpolation of the `f = 0` crossing on edge a->b.
fn edge_point(a: Vector4, fa: f64, b: Vector4, fb: f64) -> Vector4 {
    let denom = fa - fb;
    let t = if denom.abs() < 1.0e-12 {
        0.5
    } else {
        (fa / denom).clamp(0.0, 1.0)
    };
    at(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

/// Push one triangle, welding its vertices and orienting it so the face normal
/// points outward (same hemisphere as the field gradient at the centroid).
fn emit_triangle<F: ScalarField>(
    tri: [Vector4; 3],
    field: &F,
    welder: &mut VertexWelder,
    mesh: &mut SurfaceMesh,
) {
    let edge1 = sub(tri[1], tri[0]);
    let edge2 = sub(tri[2], tri[0]);
    let face = cross(edge1, edge2);
    if length(face) < 1.0e-12 {
        return; // degenerate
    }
    let centroid = at(
        (tri[0].x + tri[1].x + tri[2].x) / 3.0,
        (tri[0].y + tri[1].y + tri[2].y) / 3.0,
        (tri[0].z + tri[1].z + tri[2].z) / 3.0,
    );
    let grad = field.gradient(centroid);
    let mut order = [0usize, 1, 2];
    if dot(face, grad) < 0.0 {
        order = [0, 2, 1]; // flip winding to face outward
    }

    let mut indices = [0u32; 3];
    for (slot, &corner) in order.iter().enumerate() {
        let normal = normalize(field.gradient(tri[corner]));
        indices[slot] = welder.weld(mesh, tri[corner], normal);
    }
    mesh.push_triangle(indices);
}

fn sub(a: Vector4, b: Vector4) -> Vector4 {
    Vector4::new3(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn cross(a: Vector4, b: Vector4) -> Vector4 {
    Vector4::new3(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn dot(a: Vector4, b: Vector4) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn length(v: Vector4) -> f64 {
    dot(v, v).sqrt()
}

fn normalize(v: Vector4) -> Vector4 {
    let len = length(v);
    if len < 1.0e-12 {
        Vector4::new3(0.0, 0.0, 1.0)
    } else {
        Vector4::new3(v.x / len, v.y / len, v.z / len)
    }
}

/// Welds vertices that fall on the same quantised position so the mesh shares
/// edges (each crossing point is produced by several tetrahedra).
struct VertexWelder {
    map: HashMap<[i64; 3], u32>,
    quantum: f64,
}

impl VertexWelder {
    fn new(spacing: f64) -> Self {
        Self {
            map: HashMap::new(),
            quantum: (spacing * 1.0e-3).max(1.0e-6),
        }
    }

    fn weld(&mut self, mesh: &mut SurfaceMesh, position: Vector4, normal: Vector4) -> u32 {
        let key = [
            (position.x / self.quantum).round() as i64,
            (position.y / self.quantum).round() as i64,
            (position.z / self.quantum).round() as i64,
        ];
        if let Some(&index) = self.map.get(&key) {
            return index;
        }
        let index = mesh.vertex_count() as u32;
        mesh.push_vertex(SurfaceVertex { position, normal });
        self.map.insert(key, index);
        index
    }
}

/// Build the solvent-EXCLUDED surface (SES / Connolly) of a set of atom spheres
/// (each `radius` = van-der-Waals radius) rolled by a probe of `probe` radius,
/// sampled at grid `spacing`. Unlike [`build_sphere_union_surface`] (which makes
/// the solvent-ACCESSIBLE / vdW surface), this carves the reentrant probe
/// patches in crevices.
///
/// Grid algorithm (no vendored BALL): the probe centre can sit anywhere outside
/// every `vdw + probe` sphere — call that accessible region `A` (kept to the
/// exterior-connected component by a flood fill, so buried voids stay excluded).
/// The SES is then the level set `dist(p, A) == probe`: a point is inside the
/// molecule when it is farther than a probe radius from any accessible solvent.
/// For one convex atom this reduces to the bare vdW sphere (radius `vdw`), the
/// defining difference from the accessible surface (radius `vdw + probe`).
pub fn build_ses_surface(mesh: &mut SurfaceMesh, spheres: &[SurfSphere], probe: f64, spacing: f64) {
    if spheres.is_empty() || spacing <= 0.0 {
        return;
    }
    let probe = probe.max(0.0);
    let max_vdw = spheres.iter().fold(0.0_f64, |m, s| m.max(s.radius));
    let margin = max_vdw + probe + spacing * 3.0;

    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for s in spheres {
        for (axis, value) in [s.center.x, s.center.y, s.center.z].into_iter().enumerate() {
            lo[axis] = lo[axis].min(value);
            hi[axis] = hi[axis].max(value);
        }
    }
    let origin = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let counts = [
        ((hi[0] + margin - origin[0]) / spacing).ceil() as usize + 1,
        ((hi[1] + margin - origin[1]) / spacing).ceil() as usize + 1,
        ((hi[2] + margin - origin[2]) / spacing).ceil() as usize + 1,
    ];
    let total = counts[0] * counts[1] * counts[2];
    if counts[0] < 2 || counts[1] < 2 || counts[2] < 2 {
        return;
    }
    let coord = |i: usize, axis: usize| origin[axis] + i as f64 * spacing;
    let flat = |i: usize, j: usize, k: usize| i + counts[0] * (j + counts[1] * k);

    // Accessible candidates: nodes outside every (vdw + probe) sphere.
    let expanded: Vec<SurfSphere> = spheres
        .iter()
        .map(|s| SurfSphere {
            center: s.center,
            radius: s.radius + probe,
        })
        .collect();
    let grid = SphereGrid::new(&expanded, max_vdw + probe + spacing);
    let mut is_candidate = vec![false; total];
    for k in 0..counts[2] {
        for j in 0..counts[1] {
            for i in 0..counts[0] {
                let p = at(coord(i, 0), coord(j, 1), coord(k, 2));
                if grid.field(p) >= 0.0 {
                    is_candidate[flat(i, j, k)] = true;
                }
            }
        }
    }

    // Flood fill the exterior-connected accessible region from the grid boundary
    // (6-connectivity), so probe-inaccessible interior voids are NOT solvent.
    let mut accessible = vec![false; total];
    let mut stack: Vec<(usize, usize, usize)> = Vec::new();
    for k in 0..counts[2] {
        for j in 0..counts[1] {
            for i in 0..counts[0] {
                let on_boundary = i == 0
                    || j == 0
                    || k == 0
                    || i == counts[0] - 1
                    || j == counts[1] - 1
                    || k == counts[2] - 1;
                let id = flat(i, j, k);
                if on_boundary && is_candidate[id] && !accessible[id] {
                    accessible[id] = true;
                    stack.push((i, j, k));
                }
            }
        }
    }
    while let Some((i, j, k)) = stack.pop() {
        let mut visit =
            |ni: usize, nj: usize, nk: usize, stack: &mut Vec<(usize, usize, usize)>| {
                let id = flat(ni, nj, nk);
                if is_candidate[id] && !accessible[id] {
                    accessible[id] = true;
                    stack.push((ni, nj, nk));
                }
            };
        if i > 0 {
            visit(i - 1, j, k, &mut stack);
        }
        if i + 1 < counts[0] {
            visit(i + 1, j, k, &mut stack);
        }
        if j > 0 {
            visit(i, j - 1, k, &mut stack);
        }
        if j + 1 < counts[1] {
            visit(i, j + 1, k, &mut stack);
        }
        if k > 0 {
            visit(i, j, k - 1, &mut stack);
        }
        if k + 1 < counts[2] {
            visit(i, j, k + 1, &mut stack);
        }
    }

    // Exact squared Euclidean distance transform to the accessible region, then
    // the SES field g = probe - dist (negative = molecule interior).
    const FAR: f64 = 1.0e18;
    let seed: Vec<f64> = accessible
        .iter()
        .map(|&a| if a { 0.0 } else { FAR })
        .collect();
    let sq = edt_squared_3d(&seed, counts);
    let g: Vec<f64> = sq.iter().map(|d| probe - d.sqrt() * spacing).collect();

    let field = GridScalarField {
        g: &g,
        origin,
        spacing,
        counts,
    };
    build_grid_isosurface(
        mesh,
        &field,
        counts,
        |i, j, k| g[flat(i, j, k)],
        |i, j, k| at(coord(i, 0), coord(j, 1), coord(k, 2)),
        spacing,
    );
}

/// A scalar field backed by values precomputed at grid nodes (the SES `g`
/// field). Gradients come from trilinear central differences of the stored grid.
struct GridScalarField<'a> {
    g: &'a [f64],
    origin: [f64; 3],
    spacing: f64,
    counts: [usize; 3],
}

impl GridScalarField<'_> {
    /// Trilinear sample of `g` at a continuous node index (clamped in range).
    fn sample(&self, ci: f64, cj: f64, ck: f64) -> f64 {
        let clampi = |v: f64, n: usize| (v.floor() as isize).clamp(0, n as isize - 1) as usize;
        let i0 = clampi(ci, self.counts[0]);
        let j0 = clampi(cj, self.counts[1]);
        let k0 = clampi(ck, self.counts[2]);
        let i1 = (i0 + 1).min(self.counts[0] - 1);
        let j1 = (j0 + 1).min(self.counts[1] - 1);
        let k1 = (k0 + 1).min(self.counts[2] - 1);
        let fx = (ci - i0 as f64).clamp(0.0, 1.0);
        let fy = (cj - j0 as f64).clamp(0.0, 1.0);
        let fz = (ck - k0 as f64).clamp(0.0, 1.0);
        let at_node =
            |i: usize, j: usize, k: usize| self.g[i + self.counts[0] * (j + self.counts[1] * k)];
        let c00 = at_node(i0, j0, k0) * (1.0 - fx) + at_node(i1, j0, k0) * fx;
        let c10 = at_node(i0, j1, k0) * (1.0 - fx) + at_node(i1, j1, k0) * fx;
        let c01 = at_node(i0, j0, k1) * (1.0 - fx) + at_node(i1, j0, k1) * fx;
        let c11 = at_node(i0, j1, k1) * (1.0 - fx) + at_node(i1, j1, k1) * fx;
        let c0 = c00 * (1.0 - fy) + c10 * fy;
        let c1 = c01 * (1.0 - fy) + c11 * fy;
        c0 * (1.0 - fz) + c1 * fz
    }
}

impl ScalarField for GridScalarField<'_> {
    fn gradient(&self, p: Vector4) -> Vector4 {
        let ci = (p.x - self.origin[0]) / self.spacing;
        let cj = (p.y - self.origin[1]) / self.spacing;
        let ck = (p.z - self.origin[2]) / self.spacing;
        // Central difference of g (one node either side). g increases toward the
        // solvent, so +grad(g) is the outward normal.
        let dx = self.sample(ci + 1.0, cj, ck) - self.sample(ci - 1.0, cj, ck);
        let dy = self.sample(ci, cj + 1.0, ck) - self.sample(ci, cj - 1.0, ck);
        let dz = self.sample(ci, cj, ck + 1.0) - self.sample(ci, cj, ck - 1.0);
        Vector4::new3(dx, dy, dz)
    }
}

/// Exact squared Euclidean distance transform of `seed` (0 at source nodes, a
/// large value elsewhere) by three separable 1D passes (Felzenszwalb-Huttenlocher).
fn edt_squared_3d(seed: &[f64], counts: [usize; 3]) -> Vec<f64> {
    let [nx, ny, nz] = counts;
    let mut d = seed.to_vec();
    let flat = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);

    for k in 0..nz {
        for j in 0..ny {
            let row: Vec<f64> = (0..nx).map(|i| d[flat(i, j, k)]).collect();
            let out = edt_squared_1d(&row);
            for i in 0..nx {
                d[flat(i, j, k)] = out[i];
            }
        }
    }
    for k in 0..nz {
        for i in 0..nx {
            let row: Vec<f64> = (0..ny).map(|j| d[flat(i, j, k)]).collect();
            let out = edt_squared_1d(&row);
            for j in 0..ny {
                d[flat(i, j, k)] = out[j];
            }
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let row: Vec<f64> = (0..nz).map(|k| d[flat(i, j, k)]).collect();
            let out = edt_squared_1d(&row);
            for k in 0..nz {
                d[flat(i, j, k)] = out[k];
            }
        }
    }
    d
}

/// 1D squared distance transform: `out[q] = min_p (q - p)^2 + f[p]`.
fn edt_squared_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0; n];
    let mut v = vec![0usize; n]; // parabola source locations in the lower envelope
    let mut z = vec![0.0f64; n + 1]; // envelope boundaries
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let vk = v[k] as f64;
            let s = ((f[q] + (q * q) as f64) - (f[v[k]] + vk * vk)) / (2.0 * q as f64 - 2.0 * vk);
            if s > z[k] {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f64::INFINITY;
                break;
            }
            if k == 0 {
                v[0] = q;
                z[1] = f64::INFINITY;
                break;
            }
            k -= 1;
        }
    }
    let mut k = 0usize;
    for (q, out_q) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let dq = q as f64 - v[k] as f64;
        *out_q = dq * dq + f[v[k]];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface_area(mesh: &SurfaceMesh) -> f64 {
        mesh.triangles()
            .iter()
            .map(|tri| {
                let a = mesh.vertices()[tri[0] as usize].position;
                let b = mesh.vertices()[tri[1] as usize].position;
                let c = mesh.vertices()[tri[2] as usize].position;
                0.5 * length(cross(sub(b, a), sub(c, a)))
            })
            .sum()
    }

    #[test]
    fn single_sphere_surface_matches_analytic() {
        let radius = 3.0;
        let spheres = [SurfSphere {
            center: at(0.0, 0.0, 0.0),
            radius,
        }];
        let mut mesh = SurfaceMesh::new();
        build_sphere_union_surface(&mut mesh, &spheres, 0.4);

        assert!(mesh.triangle_count() > 100, "too few triangles");
        // Every vertex sits on the sphere of the given radius (grid tolerance).
        for v in mesh.vertices() {
            assert!(
                (length(v.position) - radius).abs() < 0.4,
                "vertex off sphere"
            );
        }
        // Vertex normals point radially outward.
        for v in mesh.vertices() {
            let radial = normalize(v.position);
            assert!(dot(v.normal, radial) > 0.8, "normal not outward-radial");
        }
        // Total area approximates 4*pi*r^2 (within marching-grid discretisation).
        let analytic = 4.0 * std::f64::consts::PI * radius * radius;
        let area = surface_area(&mesh);
        assert!(
            (area - analytic).abs() / analytic < 0.1,
            "area {area} vs analytic {analytic}"
        );
    }

    #[test]
    fn two_overlapping_spheres_fuse() {
        // Two spheres 2A apart with radius 2 overlap; their union surface area is
        // strictly less than two separate spheres (the overlap is internal).
        let spheres = [
            SurfSphere {
                center: at(-1.0, 0.0, 0.0),
                radius: 2.0,
            },
            SurfSphere {
                center: at(1.0, 0.0, 0.0),
                radius: 2.0,
            },
        ];
        let mut mesh = SurfaceMesh::new();
        build_sphere_union_surface(&mut mesh, &spheres, 0.3);
        assert!(mesh.triangle_count() > 0);
        let two_full = 2.0 * 4.0 * std::f64::consts::PI * 4.0;
        assert!(surface_area(&mesh) < two_full, "overlap not removed");
        // No vertex lies deep inside the other sphere (would mean a hole).
        for v in mesh.vertices() {
            let inside_left = length(sub(v.position, at(-1.0, 0.0, 0.0))) < 2.0 - 0.3;
            let inside_right = length(sub(v.position, at(1.0, 0.0, 0.0))) < 2.0 - 0.3;
            assert!(!(inside_left || inside_right), "vertex inside a sphere");
        }
    }

    #[test]
    fn empty_input_yields_empty_mesh() {
        let mut mesh = SurfaceMesh::new();
        build_sphere_union_surface(&mut mesh, &[], 0.5);
        assert_eq!(mesh.vertex_count(), 0);
        assert_eq!(mesh.triangle_count(), 0);
    }

    #[test]
    fn ses_of_one_atom_is_the_bare_vdw_sphere() {
        // The defining SES property: a single convex atom has NO reentrant
        // patches, so its solvent-EXCLUDED surface is the vdW sphere (radius
        // vdw) — NOT the solvent-ACCESSIBLE sphere (radius vdw + probe).
        let vdw = 3.0;
        let probe = 1.4;
        let spheres = [SurfSphere {
            center: at(0.0, 0.0, 0.0),
            radius: vdw,
        }];
        let mut mesh = SurfaceMesh::new();
        build_ses_surface(&mut mesh, &spheres, probe, 0.4);

        assert!(mesh.triangle_count() > 100, "too few triangles");
        for v in mesh.vertices() {
            let r = length(v.position);
            assert!(
                (r - vdw).abs() < 0.6,
                "SES vertex at r={r}, expected vdw={vdw}"
            );
            // Definitively NOT the accessible-surface radius.
            assert!(
                (r - (vdw + probe)).abs() > 0.5,
                "SES collapsed onto the accessible (vdw+probe) sphere"
            );
        }
        // Outward radial normals.
        for v in mesh.vertices() {
            assert!(
                dot(v.normal, normalize(v.position)) > 0.6,
                "normal not outward"
            );
        }
        // Area approximates 4*pi*vdw^2.
        let analytic = 4.0 * std::f64::consts::PI * vdw * vdw;
        let area = surface_area(&mesh);
        assert!(
            (area - analytic).abs() / analytic < 0.12,
            "area {area} vs analytic {analytic}"
        );
    }

    #[test]
    fn ses_is_smaller_than_the_accessible_surface() {
        // Same atom: SES (vdw) encloses less area than the accessible/vdW-union
        // surface (vdw + probe).
        let spheres = [SurfSphere {
            center: at(0.0, 0.0, 0.0),
            radius: 3.0,
        }];
        let mut ses = SurfaceMesh::new();
        build_ses_surface(&mut ses, &spheres, 1.4, 0.4);
        let mut sas = SurfaceMesh::new();
        build_sphere_union_surface(
            &mut sas,
            &[SurfSphere {
                center: at(0.0, 0.0, 0.0),
                radius: 3.0 + 1.4,
            }],
            0.4,
        );
        assert!(surface_area(&ses) < surface_area(&sas));
    }

    #[test]
    fn ses_of_two_atoms_fuses_without_holes() {
        // Two overlapping atoms -> one connected solvent-excluded surface with no
        // vertex inside either atom (the probe carves the crevice, not the cores).
        let spheres = [
            SurfSphere {
                center: at(-2.0, 0.0, 0.0),
                radius: 3.0,
            },
            SurfSphere {
                center: at(2.0, 0.0, 0.0),
                radius: 3.0,
            },
        ];
        let mut mesh = SurfaceMesh::new();
        build_ses_surface(&mut mesh, &spheres, 1.4, 0.4);
        assert!(mesh.triangle_count() > 100);
        for v in mesh.vertices() {
            let inside_left = length(sub(v.position, at(-2.0, 0.0, 0.0))) < 3.0 - 0.6;
            let inside_right = length(sub(v.position, at(2.0, 0.0, 0.0))) < 3.0 - 0.6;
            assert!(!(inside_left || inside_right), "vertex inside an atom core");
        }
        // Fused: less area than two separate vdW spheres.
        let two_full = 2.0 * 4.0 * std::f64::consts::PI * 9.0;
        assert!(surface_area(&mesh) < two_full, "atoms did not fuse");
    }

    #[test]
    fn ses_empty_input_yields_empty_mesh() {
        let mut mesh = SurfaceMesh::new();
        build_ses_surface(&mut mesh, &[], 1.4, 0.5);
        assert_eq!(mesh.triangle_count(), 0);
    }

    #[test]
    fn ses_flood_fill_excludes_a_sealed_buried_cavity() {
        // A dense shell of atoms on a sphere of radius `shell`: adjacent
        // vdw+probe spheres overlap, so the shell is sealed and the probe cannot
        // reach the interior void. The flood fill (seeded only from the grid
        // boundary) must therefore mark the void as molecule-interior, leaving
        // NO inner cavity surface — only the outer shell. Without the flood fill,
        // the void would be treated as accessible solvent and an inner surface
        // would appear near radius `shell - vdw`.
        let shell = 6.0_f64;
        let vdw = 2.0_f64;
        let probe = 1.4_f64;
        let n = 80usize;
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt()); // ~2.399963
        let spheres: Vec<SurfSphere> = (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let theta = golden * i as f64;
                SurfSphere {
                    center: at(shell * r * theta.cos(), shell * y, shell * r * theta.sin()),
                    radius: vdw,
                }
            })
            .collect();

        let mut mesh = SurfaceMesh::new();
        build_ses_surface(&mut mesh, &spheres, probe, 0.5);
        assert!(mesh.triangle_count() > 100, "shell produced no surface");

        // Every vertex belongs to the OUTER surface: none lies deep in the
        // excluded interior (well inside the shell). The inner cavity wall, were
        // the flood fill broken, would sit near radius `shell - vdw` = 4.
        for v in mesh.vertices() {
            assert!(
                length(v.position) > shell - vdw + 0.6,
                "vertex inside the sealed cavity at r={} (flood fill leaked)",
                length(v.position)
            );
        }
    }
}
