use super::meshing::{LocalMesh, MeshState, Tag, TagList};
use super::sas::{DataCir, DataI};
use super::sas_patches::DataSeg;
use super::vec3::{
    acos_clamped, cmax, cross, cvt_i32, cvt_i64, dot, norm, orthogonalvectors, pysq, Vec3, TWO_PI,
};
use super::Geom;
use std::f64::consts::PI;

#[inline]
fn sqrt_pos(x: f64) -> f64 {
    if x > 0.0 {
        x.sqrt()
    } else {
        0.0
    }
}

/// `size_t` conversion of a C++ signed size.
#[inline]
fn size_t(n: i64) -> u64 {
    n as u64
}

/// `std::vector<T>(n, val)`: libstdc++ length and allocation failures become errors.
fn cpp_vec<T: Clone>(n: u64, val: T) -> Result<Vec<T>, String> {
    let mut v = Vec::new();
    if n > super::MAX_VERTICES as u64 + 1 {
        return Err("cannot create std::vector larger than max_size()".into());
    }
    v.try_reserve_exact(n as usize)
        .map_err(|_| "std::bad_alloc".to_string())?;
    v.resize(n as usize, val);
    Ok(v)
}

/// `std::vector<T>::reserve(n)` with libstdc++ failure semantics.
fn cpp_reserve<T>(v: &mut Vec<T>, n: u64) -> Result<(), String> {
    if n > super::MAX_FACES as u64 {
        return Err("vector::reserve".into());
    }
    v.try_reserve_exact(n as usize)
        .map_err(|_| "std::bad_alloc".to_string())
}

#[inline]
fn imax(a: i32, b: i32) -> i32 {
    if a < b {
        b
    } else {
        a
    }
}

#[inline]
fn rotated(direct: i32, n: Vec3, u: Vec3) -> Vec3 {
    direct as f64
        * Vec3::new(
            n.y * u.z - n.z * u.y,
            n.z * u.x - n.x * u.z,
            n.x * u.y - n.y * u.x,
        )
}

fn orient_face(tri: &[i32; 3], p: &[Vec3], nrm: Vec3) -> Vec3 {
    let a = p[tri[0] as usize];
    let b = p[tri[1] as usize];
    let c = p[tri[2] as usize];
    let mut nv = cross(b - a, c - a);
    nv = nv / norm(nv);
    if dot(nv, nrm) < 0.0 {
        nv = nv * -1.0;
    }
    nv
}

/// Torus arc parameters shared by both toroidal meshers.
struct Arc {
    direct: i32,
    n: Vec3,
    angle: f64,
    r: f64,
    a: Vec3,
    p_k1: Vec3,
}

#[allow(clippy::too_many_arguments)]
fn mesh_toroide(
    ci: Vec3,
    cj: Vec3,
    ri: f64,
    rj: f64,
    arc: &Arc,
    theta1: f64,
    theta2: f64,
    rp: f64,
    d: f64,
    k: i32,
    atom_i: i32,
    atom_j: i32,
    e1: i32,
    e2: i32,
) -> Result<LocalMesh, String> {
    let Arc {
        direct,
        n,
        angle,
        r,
        a,
        p_k1,
    } = *arc;
    let (flag, theta) = if norm(ci - cj)
        > cmax(
            sqrt_pos(pysq(ri + rp) - pysq(r)),
            sqrt_pos(pysq(rj + rp) - pysq(r)),
        ) {
        (1, theta1 + theta2)
    } else {
        (0, (theta1 - theta2).abs())
    };
    let theta0 = PI / 3.0;
    let n_probe = imax(
        cvt_i32((rp * theta / d).floor()).wrapping_add(1),
        cvt_i32((theta / theta0).floor()).wrapping_add(1),
    );
    let rmax = cmax(ri, rj);
    let n_arc = imax(
        cvt_i32((angle / theta0).floor()).wrapping_add(1),
        cvt_i32((r * angle * rmax / ((rp + rmax) * d)).floor()).wrapping_add(1),
    );
    if n_probe < 1 || n_arc < 1 {
        return Err("invalid MeshMS torus subdivision".into());
    }
    let n_point = (i64::from(n_probe) + 1) * (i64::from(n_arc) + 1);
    let mut p = cpp_vec(size_t(n_point as i64).wrapping_add(1), Vec3::ZERO)?;
    let mut t: Vec<[i32; 3]> = Vec::new();
    cpp_reserve(
        &mut t,
        2u64.wrapping_mul(size_t(n_probe as i64))
            .wrapping_mul(size_t(n_arc as i64)),
    )?;
    let mut vids: Vec<TagList> = cpp_vec(size_t(n_point as i64), TagList::new())?;
    let mut vatom = cpp_vec(size_t(n_point as i64), 0i32)?;
    let u = (p_k1 - a) / r;
    let v = rotated(direct, n, u);
    let mut np = 0;
    for i in 0..=n_probe {
        let theta_i = if flag == 1 {
            i as f64 * (theta1 + theta2) / n_probe as f64
        } else {
            i as f64 * (theta1 - theta2) / n_probe as f64
        };
        let dd = (norm(ci - a) - (theta1 - theta_i).tan() * r).abs();
        let ai = ci + dd * n;
        for j in 0..=n_arc {
            let angle_j = j as f64 / n_arc as f64 * angle;
            let p_j = r * angle_j.cos() * u + r * angle_j.sin() * v + a;
            np += 1;
            p[np] = p_j + rp * (ai - p_j) / norm(ai - p_j);
        }
    }
    let idx = |i: i32, j: i32| i * (n_arc + 1) + j + 1;
    for i in 1..=n_probe {
        for j in 1..=n_arc {
            t.push([idx(i - 1, j), idx(i, j - 1), idx(i, j)]);
            t.push([idx(i - 1, j), idx(i, j - 1), idx(i - 1, j - 1)]);
        }
    }
    if t.iter().flatten().any(|&v| v < 1 || v as usize >= p.len()) {
        return Err("invalid MeshMS torus face index".into());
    }
    let nv = t
        .iter()
        .map(|tri| {
            let pc = (p[tri[0] as usize] + p[tri[1] as usize] + p[tri[2] as usize]) / 3.0;
            let nt = (pc - a) - dot(pc - a, n) * n;
            let xp = a + r * nt / norm(nt);
            orient_face(tri, &p, (xp - pc) / norm(xp - pc))
        })
        .collect();
    let full_circle = angle >= TWO_PI - 1e-9;
    for gi in 0..=n_probe {
        for gj in 0..=n_arc {
            let mut tg = TagList::new();
            if gi == 0 {
                tg.push(Tag {
                    kind: 1,
                    i: atom_i,
                    j: 0,
                });
            }
            if gi == n_probe {
                tg.push(Tag {
                    kind: 1,
                    i: atom_j,
                    j: 0,
                });
            }
            if e1 != 0 && gj == 0 {
                tg.push(Tag {
                    kind: 2,
                    i: e1,
                    j: 0,
                });
            }
            if e2 != 0 && gj == n_arc {
                tg.push(Tag {
                    kind: 2,
                    i: e2,
                    j: 0,
                });
            }
            if full_circle && (gj == 0 || gj == n_arc) {
                tg.push(Tag {
                    kind: 3,
                    i: k,
                    j: gi,
                });
            }
            let at = (gi * (n_arc + 1) + gj) as usize;
            vids[at] = tg;
            vatom[at] = if gi <= n_probe / 2 { atom_i } else { atom_j };
        }
    }
    Ok(LocalMesh {
        p,
        t,
        nv,
        vids,
        vatom,
        emit: true,
    })
}

#[allow(clippy::too_many_arguments)]
fn mesh_cusp(
    a1: Vec3,
    arc: &Arc,
    theta1: f64,
    rp: f64,
    d: f64,
    k: i32,
    atom_rim: i32,
    e1: i32,
    e2: i32,
) -> Result<LocalMesh, String> {
    let Arc {
        direct,
        n,
        angle,
        r,
        a,
        p_k1,
    } = *arc;
    let n_probe = cvt_i32((rp * theta1 / d).floor()).wrapping_add(1);
    let theta0 = PI / 3.0;
    let n_arc = cvt_i32((angle / theta0).floor()).wrapping_add(1);
    if n_probe < 1 || n_arc < 1 {
        return Err("invalid MeshMS cusp subdivision".into());
    }
    let n_arc_l = n_arc as i64;
    let mut tt = cpp_vec(size_t(n_probe as i64).wrapping_add(2), 0i64)?;
    let mut n_point: i64 = 1;
    for i in 1..=n_probe {
        let theta_i = acos_clamped(r / rp) + theta1 / n_probe as f64 * i as f64;
        let l_i = r / theta_i.cos();
        let r_i = (l_i - rp) / l_i * r;
        let angle_division = angle / n_arc as f64;
        let ti = cvt_i64((r_i * angle_division / d).floor()).wrapping_add(1);
        if ti < 1 || ti > (super::MAX_VERTICES as i64 - n_point - 1) / n_arc_l {
            return Err(super::SIZE_ERROR.into());
        }
        tt[(i + 1) as usize] = ti;
        n_point += ti * n_arc_l + 1;
    }
    let mut p = cpp_vec(size_t(n_point).wrapping_add(1), Vec3::ZERO)?;
    let mut vids: Vec<TagList> = Vec::new();
    cpp_reserve(&mut vids, size_t(n_point))?;
    let mut np = 0;
    let full_circle = angle >= TWO_PI - 1e-9;
    let u = (p_k1 - a) / r;
    let v = rotated(direct, n, u);
    let theta = acos_clamped(r / rp);
    for i in 0..=n_probe {
        let div_i = tt[(i + 1) as usize] * n_arc_l;
        let (radius_i, ai) = if i == 0 {
            (0.0, Vec3::ZERO)
        } else {
            let cos_arg = r / (theta + theta1 * i as f64 / n_probe as f64).cos();
            let radius_i = (cos_arg - rp) / cos_arg * r;
            let ai = a + sqrt_pos(pysq(cos_arg) - pysq(r)) * (a1 - a) / norm(a1 - a);
            (radius_i, ai)
        };
        let division = (tt[(i + 1) as usize] * n_arc_l) as f64;
        for j in 0..=div_i {
            np += 1;
            p[np] = if i == 0 {
                a1
            } else {
                let angle_j = (j as i32) as f64 / division * angle;
                let p_j = r * angle_j.cos() * u + r * angle_j.sin() * v + a;
                radius_i / r * p_j + (r - radius_i) / r * ai
            };
            let mut tg = TagList::new();
            if i == n_probe {
                tg.push(Tag {
                    kind: 1,
                    i: atom_rim,
                    j: 0,
                });
            }
            if full_circle {
                if j == 0 || j == div_i {
                    tg.push(Tag {
                        kind: 3,
                        i: k,
                        j: i,
                    });
                }
            } else {
                if e1 != 0 && j == 0 {
                    tg.push(Tag {
                        kind: 2,
                        i: e1,
                        j: 0,
                    });
                }
                if e2 != 0 && j == div_i {
                    tg.push(Tag {
                        kind: 2,
                        i: e2,
                        j: 0,
                    });
                }
            }
            vids.push(tg);
        }
    }
    let np_u = n_probe as usize;
    let mut row_off = vec![0i64; np_u + 1];
    for i in 1..=np_u {
        row_off[i] = row_off[i - 1] + tt[i] * n_arc_l + 1;
    }
    let idx = |i: i32, j: i32| row_off[i as usize] as i32 + j + 1;
    let mut t: Vec<[i32; 3]> = Vec::new();
    let nt = (1..=n_probe).fold(0i64, |acc, i| {
        let rows = if i < n_probe { 2 } else { 1 };
        acc.wrapping_add((rows * tt[(i + 1) as usize]).wrapping_mul(n_arc_l))
    });
    cpp_reserve(&mut t, size_t(nt))?;
    for i in 1..=n_probe {
        let tip1 = tt[(i + 1) as usize];
        let ti = tt[i as usize];
        for j in 1..=tip1 * n_arc_l {
            let (jj, jm) = (j as i32, (j - 1) as i32);
            if tip1 == ti + 1 {
                let j1 = (j - (j - 1) / tip1 - 1) as i32;
                t.push([idx(i - 1, j1), idx(i, jm), idx(i, jj)]);
            } else {
                t.push([idx(i - 1, jj), idx(i, jm), idx(i, jj)]);
            }
            if i < n_probe {
                let tip2 = tt[(i + 2) as usize];
                if tip1 == tip2 - 1 {
                    let j1 = (j + (j - 1) / tip1) as i32;
                    t.push([idx(i + 1, j1), idx(i, jm), idx(i, jj)]);
                } else {
                    t.push([idx(i + 1, jm), idx(i, jm), idx(i, jj)]);
                }
            }
        }
    }
    if t.iter().flatten().any(|&v| v < 1 || v as usize >= p.len()) {
        return Err("invalid MeshMS torus face index".into());
    }
    let nv = t
        .iter()
        .map(|tri| {
            let pc = (p[tri[0] as usize] + p[tri[1] as usize] + p[tri[2] as usize]) / 3.0;
            let nt = (pc - a) - (dot(pc - a, n) / norm(n)) * n / norm(n);
            let xp = a + r * nt / norm(nt);
            orient_face(tri, &p, (xp - pc) / norm(xp - pc))
        })
        .collect();
    Ok(LocalMesh {
        p,
        t,
        nv,
        vids,
        vatom: vec![atom_rim; np],
        emit: true,
    })
}

/// Meshes every toroidal patch (segments, then full circles) into `state`.
pub(crate) fn data_sestorpat(
    state: &mut MeshState,
    geom: &Geom,
    di: &DataI,
    ds: &DataSeg,
    dc: &DataCir,
    rp: f64,
    d: f64,
) -> Result<(), String> {
    let c = &geom.centers;
    let rr = &geom.r;
    for i in 1..=ds.nsegment {
        let seg = &ds.segment[i];
        let nc = &ds.ncrasegment[i];
        let arc = Arc {
            direct: seg[4],
            n: Vec3::new(nc[0], nc[1], nc[2]),
            angle: nc[7],
            r: nc[6],
            a: Vec3::new(nc[3], nc[4], nc[5]),
            p_k1: di.i[seg[2] as usize],
        };
        let (r, a, n) = (arc.r, arc.a, arc.n);
        let (si, sj) = (seg[0] as usize, seg[1] as usize);
        let (ri, rj) = (rr[si], rr[sj]);
        let (ci, cj) = (c[si], c[sj]);
        let k = i as i32;
        if r < rp && dot(ci - a, cj - a) < 0.0 {
            let a1 = a - sqrt_pos(pysq(rp) - pysq(r)) * n;
            let theta1 = acos_clamped(r / (ri + rp)) - acos_clamped(r / rp);
            state.add_patch(mesh_cusp(
                a1, &arc, theta1, rp, d, k, seg[0], seg[2], seg[3],
            )?)?;
            let a2 = a + sqrt_pos(pysq(rp) - pysq(r)) * n;
            let theta2 = acos_clamped(r / (rj + rp)) - acos_clamped(r / rp);
            state.add_patch(mesh_cusp(
                a2, &arc, theta2, rp, d, k, seg[1], seg[2], seg[3],
            )?)?;
        } else {
            let theta1 = acos_clamped(r / (ri + rp));
            let theta2 = acos_clamped(r / (rj + rp));
            state.add_patch(mesh_toroide(
                ci, cj, ri, rj, &arc, theta1, theta2, rp, d, k, seg[0], seg[1], seg[2], seg[3],
            )?)?;
        }
    }
    for i in 1..=dc.ncircle {
        let crc = &dc.circle[i];
        let r = crc[9];
        let a = Vec3::new(crc[3], crc[4], crc[5]);
        let n = Vec3::new(crc[6], crc[7], crc[8]);
        let (v1, _) = orthogonalvectors(n);
        let arc = Arc {
            direct: 1,
            n,
            angle: TWO_PI,
            r,
            a,
            p_k1: a + r * v1,
        };
        let (ci_idx, cj_idx) = (crc[1] as i32, crc[2] as i32);
        let (ri, rj) = (rr[ci_idx as usize], rr[cj_idx as usize]);
        let (ci, cj) = (c[ci_idx as usize], c[cj_idx as usize]);
        let k = -(i as i32);
        if r < rp && dot(ci - a, cj - a) < 0.0 {
            let h = sqrt_pos(pysq(rp) - pysq(r));
            let a_i = a + h.copysign(dot(ci - a, n)) * n;
            let a_j = a + h.copysign(dot(cj - a, n)) * n;
            let theta1 = acos_clamped(r / (ri + rp)) - acos_clamped(r / rp);
            let theta2 = acos_clamped(r / (rj + rp)) - acos_clamped(r / rp);
            state.add_patch(mesh_cusp(a_i, &arc, theta1, rp, d, k, ci_idx, 0, 0)?)?;
            state.add_patch(mesh_cusp(a_j, &arc, theta2, rp, d, k, cj_idx, 0, 0)?)?;
        } else {
            let theta1 = acos_clamped(r / (ri + rp));
            let theta2 = acos_clamped(r / (rj + rp));
            state.add_patch(mesh_toroide(
                ci, cj, ri, rj, &arc, theta1, theta2, rp, d, k, ci_idx, cj_idx, 0, 0,
            )?)?;
        }
    }
    Ok(())
}
