//! Pure-Rust port of the MeshMS analytic solvent-excluded surface mesher
//! (github.com/CueMol/MeshMS), translated from the serial C++ implementation.

mod concave;
mod convex;
mod fusion;
mod meshing;
mod sas;
mod sas_patches;
mod toroidal;
mod vec3;

use meshing::MeshState;

const MAX_VERTICES: usize = 48_000_000;
const MAX_FACES: usize = 96_000_000;
const SIZE_ERROR: &str = "MeshMS surface exceeds the mesh budget";

fn check_size(vertices: usize, faces: usize) -> Result<(), String> {
    if vertices > MAX_VERTICES || faces > MAX_FACES {
        return Err(SIZE_ERROR.into());
    }
    Ok(())
}
use std::collections::HashMap;
use vec3::{cross, pysq, Vec3};

/// Atom geometry, 1-based with a dummy row 0.
#[derive(Debug, Clone, Default)]
pub(crate) struct Geom {
    pub m: usize,
    pub centers: Vec<Vec3>,
    pub r: Vec<f64>,
}

/// Triangle mesh with per-vertex normals and 1-based owning atoms (0 = unknown).
#[derive(Debug, Clone, Default)]
pub(crate) struct MeshResult {
    pub verts: Vec<Vec3>,
    pub vnormals: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
    pub atom_id: Vec<u32>,
}

fn geom_from_array(xyzr: &[[f64; 4]]) -> Result<Geom, String> {
    let m = xyzr.len();
    let mut g = Geom {
        m,
        centers: vec![Vec3::ZERO; m + 1],
        r: vec![0.0; m + 1],
    };
    for (a, c) in xyzr.iter().enumerate() {
        if !c.iter().all(|v| v.is_finite()) || c[3] < 0.0 {
            return Err(format!(
                "meshms: atom {a} has a non-finite coordinate or a negative radius"
            ));
        }
        g.centers[a + 1] = Vec3::new(c[0], c[1], c[2]);
        g.r[a + 1] = c[3];
    }
    Ok(g)
}

/// Connected components of the SAS-intersection graph, ordered by smallest member.
fn sas_components(xyzr: &[[f64; 4]], rp: f64) -> Result<Vec<Vec<u32>>, String> {
    let m = xyzr.len();
    let mut parent: Vec<usize> = (0..=m).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    let nb = sas::interstructure(&geom_from_array(xyzr)?, rp);
    for a in 1..=m {
        for &b in nb.of(a) {
            let ra = find(&mut parent, a);
            let rb = find(&mut parent, b as usize);
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let mut comps: Vec<Vec<u32>> = Vec::new();
    let mut comp_of_root = vec![usize::MAX; m + 1];
    for a in 1..=m {
        let r = find(&mut parent, a);
        if comp_of_root[r] == usize::MAX {
            comp_of_root[r] = comps.len();
            comps.push(Vec::new());
        }
        comps[comp_of_root[r]].push((a - 1) as u32);
    }
    Ok(comps)
}

/// Density-independent arrangement of one connected component.
struct RsComponents {
    geom: Geom,
    rp: f64,
    di: sas::DataI,
    dc: sas::DataCir,
    ds: sas_patches::DataSeg,
    dl: sas_patches::DataLoop,
    dp: sas_patches::DataPat,
    concave: concave::ConcaveDecomp,
}

/// The error paths of the exterior (eSAS) extraction; its flags feed no mesher.
fn check_exterior(
    geom: &Geom,
    dc: &sas::DataCir,
    ds: &sas_patches::DataSeg,
    dl: &sas_patches::DataLoop,
    dp: &sas_patches::DataPat,
) -> Result<(), String> {
    let m = geom.m;
    for i in 1..=m {
        let [pstart, pend] = dp.patches_index[i];
        if pstart <= 0 {
            continue;
        }
        let ii = i as i32;
        for j in pstart..=pend {
            for &pjk in &dp.patches[j as usize][1..] {
                let touches = if pjk > 0 {
                    let ln = (dl.loops_index[i][0] + pjk - 1) as usize;
                    dl.loops[ln][1..].iter().all(|&sn| {
                        let seg = &ds.segment[sn as usize];
                        seg[0] == ii || seg[1] == ii
                    })
                } else {
                    let cid = dc.circleindex[i][(-pjk - 1) as usize] as usize;
                    dc.circle[cid][1] as i32 == ii || dc.circle[cid][2] as i32 == ii
                };
                if !touches {
                    return Err(
                        "meshms: exterior extraction lost the segment/atom incidence \
                         (neither endpoint of the segment is the current atom)"
                            .into(),
                    );
                }
            }
        }
    }
    let c = &geom.centers;
    let r = &geom.r;
    let leftmost = |coord: fn(&Vec3) -> f64| {
        let mut best = coord(&c[1]) - r[1];
        let mut i_left = 1;
        for i in 2..=m {
            let v = coord(&c[i]) - r[i];
            if v < best {
                best = v;
                i_left = i;
            }
        }
        i_left
    };
    let mut i_left = leftmost(|v| v.x);
    if dp.patches_index[i_left][0] == 0 {
        i_left = leftmost(|v| v.y);
    }
    let [pstart, pend] = dp.patches_index[i_left];
    if pstart == 0 && pend >= 0 {
        return Err("Error: there exists isolated SAS-ball!".into());
    }
    Ok(())
}

fn compute_rs(geom: Geom, rp: f64) -> Result<RsComponents, String> {
    let inter = sas::interstructure(&geom, rp);
    let (di, dc) = sas::data_i_cir(&geom, &inter, rp);
    let (ds, dl, dp) = sas_patches::data_seg_pat(&geom, &inter, &di, &dc, rp);
    check_exterior(&geom, &dc, &ds, &dl, &dp)?;
    let concave = concave::precompute_concave(&geom, &di, rp, &inter);
    Ok(RsComponents {
        geom,
        rp,
        di,
        dc,
        ds,
        dl,
        dp,
        concave,
    })
}

/// Oriented, fused component mesh: vertices, faces and 1-based owning atoms.
type ComponentMesh = (Vec<Vec3>, Vec<[i32; 3]>, Vec<i32>);

fn build_mesh(rs: &RsComponents, d: f64) -> Result<ComponentMesh, String> {
    let (geom, rp) = (&rs.geom, rs.rp);
    let mut state = MeshState::default();
    convex::data_sessphpat_convex(
        &mut state, geom, &rs.di, &rs.dc, &rs.ds, &rs.dl, &rs.dp, rp, d,
    )?;
    concave::sesconcavepat_mesh(&mut state, geom, &rs.di, &rs.concave, rp, d)?;
    toroidal::data_sestorpat(&mut state, geom, &rs.di, &rs.ds, &rs.dc, rp, d)?;
    for (f, n) in state.f.iter_mut().zip(&state.n) {
        let a = state.v[f[0] as usize];
        let b = state.v[f[1] as usize];
        let c = state.v[f[2] as usize];
        if vec3::dot(cross(b - a, c - a), *n) < 0.0 {
            f.swap(1, 2);
        }
    }
    Ok(fusion::fuse_by_id(
        &state.v,
        &state.f,
        &state.tags,
        &state.vatom,
        1e-6,
    ))
}

/// Icosphere for an atom without SAS neighbours: its SES is the vdW sphere.
fn append_isolated_sphere(
    out: &mut MeshResult,
    orig: u32,
    xyzr: [f64; 4],
    mesh_size: f64,
) -> Result<(), String> {
    let r = xyzr[3];
    if r.is_nan() || r <= 0.0 {
        return Ok(());
    }
    let t = (1.0 + 5.0f64.sqrt()) / 2.0;
    let mut dirs: Vec<Vec3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|&[x, y, z]| {
        let d = Vec3::new(x, y, z);
        d / (pysq(d.x) + pysq(d.y) + pysq(d.z)).sqrt()
    })
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    let edge0 = 4.0 / (10.0 + 2.0 * 5.0f64.sqrt()).sqrt();
    let mut level = 0;
    while level < 7 && edge0 * r / (1u32 << level) as f64 > mesh_size {
        level += 1;
    }
    let subdivisions = 4usize.pow(level);
    check_size(
        out.verts.len() + 10 * subdivisions + 2,
        out.faces.len() + 20 * subdivisions,
    )?;
    for _ in 0..level {
        let mut midpoint: HashMap<(u32, u32), u32> = HashMap::new();
        let mut mid = |a: u32, b: u32, dirs: &mut Vec<Vec3>| {
            *midpoint.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let mut m = dirs[a as usize] + dirs[b as usize];
                m = m / (pysq(m.x) + pysq(m.y) + pysq(m.z)).sqrt();
                dirs.push(m);
                (dirs.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for f in &faces {
            let ab = mid(f[0], f[1], &mut dirs);
            let bc = mid(f[1], f[2], &mut dirs);
            let ca = mid(f[2], f[0], &mut dirs);
            next.extend([[f[0], ab, ca], [f[1], bc, ab], [f[2], ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }
    let base = out.verts.len() as u32;
    let c = Vec3::new(xyzr[0], xyzr[1], xyzr[2]);
    for d in &dirs {
        out.verts
            .push(Vec3::new(c.x + d.x * r, c.y + d.y * r, c.z + d.z * r));
        out.vnormals.push(*d);
        out.atom_id.push(orig + 1);
    }
    out.faces.extend(
        faces
            .iter()
            .map(|f| [base + f[0], base + f[1], base + f[2]]),
    );
    Ok(())
}

/// Area-weighted per-vertex normals.
fn vnormals_from_faces(v: &[Vec3], f: &[[u32; 3]]) -> Vec<Vec3> {
    let mut nv = vec![Vec3::ZERO; v.len()];
    for t in f {
        let a = v[t[0] as usize];
        let b = v[t[1] as usize];
        let c = v[t[2] as usize];
        let fn_ = cross(b - a, c - a);
        for &x in t {
            nv[x as usize] = nv[x as usize] + fn_;
        }
    }
    for n in &mut nv {
        let mut len = (pysq(n.x) + pysq(n.y) + pysq(n.z)).sqrt();
        if len == 0.0 {
            len = 1.0;
        }
        *n = *n / len;
    }
    nv
}

/// `remove_flaps(build_mesh_from_cache(compute_rs_from_array(xyzr, rp), mesh_size, true))`.
pub(crate) fn build_surface(
    xyzr: &[[f64; 4]],
    rp: f64,
    mesh_size: f64,
) -> Result<MeshResult, String> {
    let mut comps = Vec::new();
    let mut isolated = Vec::new();
    for comp in sas_components(xyzr, rp)? {
        if comp.len() == 1 {
            isolated.push(comp[0]);
            continue;
        }
        let sub: Vec<[f64; 4]> = comp.iter().map(|&i| xyzr[i as usize]).collect();
        comps.push((compute_rs(geom_from_array(&sub)?, rp)?, comp));
    }
    let mut out = MeshResult::default();
    for (rs, orig) in &comps {
        let (v, f, a) = build_mesh(rs, mesh_size)?;
        let base = out.verts.len() as u32;
        check_size(out.verts.len() + v.len(), out.faces.len() + f.len())?;
        out.verts.extend_from_slice(&v);
        out.faces.extend(
            f.iter()
                .map(|t| [base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]),
        );
        out.atom_id.extend(a.iter().map(|&id| {
            if id == 0 {
                0
            } else {
                orig[(id - 1) as usize] + 1
            }
        }));
    }
    for &i in &isolated {
        append_isolated_sphere(&mut out, i, xyzr[i as usize], mesh_size)?;
    }
    let faces: Vec<[i32; 3]> = out
        .faces
        .iter()
        .map(|t| [t[0] as i32, t[1] as i32, t[2] as i32])
        .collect();
    out.faces = fusion::remove_nonmanifold_flaps(&faces, 4)
        .iter()
        .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect();
    out.vnormals = vnormals_from_faces(&out.verts, &out.faces);
    Ok(out)
}

/// MeshMS surface kernel: interleaved position/normal vertices, triangles, backend 1, owners.
pub(crate) fn surface_kernel(
    xyzr: &[f64],
    mesh_size: f64,
    probe: f64,
) -> cuemol_core::Result<crate::backends::KernelMesh> {
    use cuemol_core::CueMolError;
    let atoms: Vec<[f64; 4]> = xyzr
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect();
    let count = u32::try_from(atoms.len())
        .map_err(|_| CueMolError::invalid_argument("too many SES atoms"))?;
    let result = build_surface(&atoms, probe, mesh_size).map_err(CueMolError::invalid_argument)?;
    if result.verts.is_empty() || result.faces.is_empty() {
        return Err(CueMolError::invalid_argument(
            "MeshMS returned an empty surface",
        ));
    }
    if result.verts.len() > MAX_VERTICES || result.faces.len() > MAX_FACES {
        return Err(CueMolError::invalid_argument(
            "MeshMS surface exceeds the vertex budget",
        ));
    }
    let vertices = result
        .verts
        .iter()
        .zip(&result.vnormals)
        .flat_map(|(v, n)| [v.x, v.y, v.z, n.x, n.y, n.z])
        .collect();
    let owners = result
        .atom_id
        .iter()
        .map(|&o| if o > 0 && o <= count { o - 1 } else { u32::MAX })
        .collect();
    let triangles = result.faces.iter().flatten().copied().collect();
    Ok((vertices, triangles, 1, owners))
}
