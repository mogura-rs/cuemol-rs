use super::vec3::{
    acos_clamped, cmax, cmin, cross, dot, norm, orthogonalvectors, pysq, sign, Vec3, TWO_PI,
};
use std::collections::VecDeque;
use std::f64::consts::PI;

/// Boundary tag for ID-based fusion: kind 1 = atom, 2 = probe, 3 = toroidal seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Tag {
    pub kind: i32,
    pub i: i32,
    pub j: i32,
}

pub(crate) type TagList = Vec<Tag>;

/// Global mesh accumulator; faces are 0-based.
#[derive(Debug, Default)]
pub(crate) struct MeshState {
    pub v: Vec<Vec3>,
    pub f: Vec<[i32; 3]>,
    pub n: Vec<Vec3>,
    pub tags: Vec<TagList>,
    pub vatom: Vec<i32>,
}

/// One patch mesh: `p` is 1-based (`p[0]` dummy), `t` holds 1-based triples into `p`.
#[derive(Debug, Default)]
pub(crate) struct LocalMesh {
    pub p: Vec<Vec3>,
    pub t: Vec<[i32; 3]>,
    pub nv: Vec<Vec3>,
    pub vids: Vec<TagList>,
    pub vatom: Vec<i32>,
    pub emit: bool,
}

impl MeshState {
    pub fn add_patch(&mut self, lm: LocalMesh) -> Result<(), String> {
        if !lm.emit {
            return Ok(());
        }
        let base = self.v.len() as i32;
        let np = lm.p.len().saturating_sub(1);
        super::check_size(self.v.len() + np, self.f.len() + lm.t.len())?;
        self.v.extend_from_slice(&lm.p[lm.p.len().min(1)..]);
        if lm.vids.is_empty() {
            self.tags.resize_with(self.tags.len() + np, Vec::new);
        } else {
            self.tags.extend(lm.vids);
        }
        if lm.vatom.is_empty() {
            self.vatom.resize(self.vatom.len() + np, 0);
        } else {
            self.vatom.extend_from_slice(&lm.vatom);
        }
        self.f.extend(
            lm.t.iter()
                .map(|t| [t[0] - 1 + base, t[1] - 1 + base, t[2] - 1 + base]),
        );
        self.n.extend_from_slice(&lm.nv);
        Ok(())
    }
}

/// Picks the nearest candidate atom; near-ties within 16 epsilons resolve to the smallest atom id.
pub(crate) fn nearest_atom(p: Vec3, a: &[i32], c: &[Vec3]) -> i32 {
    let mut best = 0;
    let mut best_d = dot(p - c[0], p - c[0]);
    for k in 1..a.len() {
        let dk = dot(p - c[k], p - c[k]);
        let tolerance = 16.0 * f64::EPSILON * cmax(cmax(1.0, dk.abs()), best_d.abs());
        if dk < best_d - tolerance || ((dk - best_d).abs() <= tolerance && a[k] < a[best]) {
            best = k;
            best_d = dk;
        }
    }
    a[best]
}

type Edge = [i32; 2];
type Front = VecDeque<Edge>;

const FIVE_PI_OVER_4: f64 = 5.0 * PI / 4.0;
const FIVE_PI_OVER_3: f64 = 5.0 / 3.0 * PI;

#[derive(Default)]
struct ActiveFront {
    meshed: bool,
    ae: Vec<Edge>,
}

#[derive(Default)]
struct Ctx<'a> {
    active: Vec<ActiveFront>,
    rj: f64,
    rj_table: Option<&'a [f64]>,
    pending: Vec<Front>,
    dist: Vec<f64>,
}

/// Mutable patch state threaded through the advancing front.
struct Patch {
    c_sphere: Vec3,
    r_sphere: f64,
    rp: f64,
    d: f64,
    tolerance: f64,
    n_max: i32,
    t: Vec<[i32; 3]>,
    p: Vec<Vec3>,
}

impl Patch {
    #[inline]
    fn pt(&self, idx: i32) -> Vec3 {
        self.p[idx as usize]
    }
    #[inline]
    fn np(&self) -> i32 {
        (self.p.len() - 1) as i32
    }
}

#[inline]
fn e(ae: &Front, m: i32) -> Edge {
    ae[(m - 1) as usize]
}

fn normal_sphere(c_sphere: Vec3, x: Vec3) -> Vec3 {
    let d = x - c_sphere;
    d / (pysq(d.x) + pysq(d.y) + pysq(d.z)).sqrt()
}

fn angle_sphere(e: Edge, f: Edge, n: Vec3, p: &[Vec3]) -> f64 {
    let mut u = p[e[0] as usize] - p[e[1] as usize];
    let mut v = p[f[1] as usize] - p[f[0] as usize];
    u = u - dot(u, n) * n;
    v = v - dot(v, n) * n;
    let t = sign(dot(u, cross(v, n)));
    let nu = (pysq(u.x) + pysq(u.y) + pysq(u.z)).sqrt();
    let nv = (pysq(v.x) + pysq(v.y) + pysq(v.z)).sqrt();
    if nu == 0.0 || nv == 0.0 {
        return 0.0;
    }
    let base = acos_clamped(dot(u, v) / (nu * nv));
    if t < 0.0 {
        base
    } else {
        TWO_PI - base
    }
}

fn angle_vectors(mut u: Vec3, mut v: Vec3, n: Vec3) -> f64 {
    u = u - dot(u, n) * n;
    v = v - dot(v, n) * n;
    let t = sign(dot(u, cross(v, n)));
    let nu = norm(u);
    let nv = norm(v);
    if nu == 0.0 || nv == 0.0 {
        return 0.0;
    }
    let base = acos_clamped(dot(u, v) / (nu * nv));
    if t < 0.0 {
        base
    } else {
        TWO_PI - base
    }
}

fn ne_sphere(e: Edge, n1: Vec3, n2: Vec3, p: &[Vec3]) -> Vec3 {
    let mut u = p[e[1] as usize] - p[e[0] as usize];
    let v = 0.5 * n1 + 0.5 * n2;
    u = u - dot(u, v) * v;
    let ne = Vec3::new(
        u.y * v.z - u.z * v.y,
        u.z * v.x - u.x * v.z,
        u.x * v.y - u.y * v.x,
    );
    ne / (pysq(ne.x) + pysq(ne.y) + pysq(ne.z)).sqrt()
}

fn map_sphere(c_sphere: Vec3, r_sphere: f64, p: Vec3, rp: f64) -> Vec3 {
    let r = if r_sphere > rp {
        r_sphere - rp
    } else {
        r_sphere
    };
    let u = (p - c_sphere) / norm(p - c_sphere);
    c_sphere + r * u
}

#[allow(clippy::too_many_arguments)]
fn arc_division(
    c: Vec3,
    r: f64,
    p1: Vec3,
    p2: Vec3,
    angle: f64,
    n: Vec3,
    ae: &mut Front,
    pa: &mut Patch,
    flag: i32,
    ctx: &Ctx,
) -> Result<(), String> {
    let (r_sphere, rp, d) = (pa.r_sphere, pa.rp, pa.d);
    let theta0 = PI / 3.0;
    let mut n_division = cmax(
        (angle / theta0).floor() + 1.0,
        (r * angle / d).floor() + 1.0,
    );
    if flag == 1 {
        let n_arc = (angle / theta0).floor() + 1.0;
        n_division = n_arc * ((r * angle / d / n_arc).floor() + 1.0);
    }
    if flag == 0 && angle <= TWO_PI && r_sphere > rp && r_sphere < ctx.rj + rp {
        let r_new = r * (ctx.rj * r_sphere) / ((r_sphere - rp) * (ctx.rj + rp));
        n_division = cmax(
            (angle / theta0).floor() + 1.0,
            (r_new * angle / d).floor() + 1.0,
        );
    }
    if !n_division.is_finite()
        || n_division < 1.0
        || n_division > (super::MAX_VERTICES - pa.np() as usize) as f64
    {
        return Err("invalid MeshMS arc subdivision".into());
    }
    let ndiv = n_division as i32;
    if ndiv == 1 && norm(p2 - p1) < 1e-10 && angle < 0.1 {
        return Ok(());
    }
    let u = (p1 - c) / r;
    let v = Vec3::new(
        n.y * u.z - n.z * u.y,
        n.z * u.x - n.x * u.z,
        n.x * u.y - n.y * u.x,
    );
    let np = pa.np();
    for j in 0..ndiv {
        let angle_j = j as f64 / ndiv as f64 * angle;
        let p_j = r * angle_j.cos() * u + r * angle_j.sin() * v + c;
        pa.p.push(p_j);
        ae.push_back([np + j + 1, np + j + 2]);
    }
    Ok(())
}

fn loop_division(
    lp: &[i32],
    segment0: &[[f64; 12]],
    ae: &mut Front,
    pa: &mut Patch,
    ctx: &mut Ctx,
) -> Result<(), String> {
    ae.clear();
    let (c_sphere, r_sphere, rp) = (pa.c_sphere, pa.r_sphere, pa.rp);
    let loopsize = lp.len() - 1;
    let np0 = pa.np();
    for i in 1..=loopsize {
        let s = lp[i] as usize;
        let seg = &segment0[s];
        let mut c = Vec3::new(seg[1], seg[2], seg[3]);
        let n = Vec3::new(seg[4], seg[5], seg[6]);
        let mut r = seg[7];
        let mut p1 = Vec3::new(seg[8], seg[9], seg[10]);
        let seg_next = &segment0[lp[i % loopsize + 1] as usize];
        let p2 = Vec3::new(seg_next[8], seg_next[9], seg_next[10]);
        let angle = seg[11];
        ctx.rj = if r_sphere == rp {
            rp
        } else {
            *ctx.rj_table
                .and_then(|r| r.get(s))
                .ok_or("missing MeshMS neighbour radius")?
        };
        let mut flag = 0;
        if r < rp && r_sphere > rp {
            flag = 1;
        }
        if r_sphere > rp {
            c = c_sphere + (c - c_sphere) * (r_sphere - rp) / r_sphere;
            r = r * (r_sphere - rp) / r_sphere;
            p1 = c_sphere + (p1 - c_sphere) * (r_sphere - rp) / r_sphere;
        }
        arc_division(c, r, p1, p2, angle, n, ae, pa, flag, ctx)?;
    }
    if let Some(last) = ae.back_mut() {
        last[1] = np0 + 1;
    }
    Ok(())
}

fn circle_division(
    circle: &[f64; 9],
    ae: &mut Front,
    pa: &mut Patch,
    ctx: &mut Ctx,
) -> Result<(), String> {
    ae.clear();
    let (c_sphere, r_sphere, rp) = (pa.c_sphere, pa.r_sphere, pa.rp);
    let mut c = Vec3::new(circle[1], circle[2], circle[3]);
    let n = Vec3::new(circle[4], circle[5], circle[6]);
    let mut r = circle[7];
    let mut flag = 0;
    if r < rp && r_sphere > rp {
        flag = 1;
    }
    if r_sphere > rp {
        c = c_sphere + (c - c_sphere) * (r_sphere - rp) / r_sphere;
        r = r * (r_sphere - rp) / r_sphere;
    }
    let (v1, _) = orthogonalvectors(n);
    let p1 = c + r * v1;
    if r_sphere == rp {
        ctx.rj = rp;
    } else if circle[8] > 0.0 {
        ctx.rj = circle[8];
    }
    let np0 = pa.np();
    arc_division(c, r, p1, p1, TWO_PI, n, ae, pa, flag, ctx)?;
    ae.back_mut().ok_or("empty MeshMS boundary circle")?[1] = np0 + 1;
    Ok(())
}

/// Angle filter and nearest-candidate update shared by both dist sweeps.
#[allow(clippy::too_many_arguments)]
fn choose_neighbor(
    pa: &Patch,
    ae: &Front,
    m: i32,
    dist1: f64,
    dist2: f64,
    angles: (f64, f64, Vec3, Vec3),
    index_np: &mut i32,
    edgedist: &mut f64,
) {
    let (angle1, angle2, n1, n2) = angles;
    let nae = ae.len() as i32;
    let mut flag = 1;
    if (m == 3 && angle2 < FIVE_PI_OVER_4) || (m == nae && angle1 < FIVE_PI_OVER_4) {
        flag = 0;
    } else if m > 3 && m < nae {
        let angle1_m = angle_vectors(
            pa.pt(e(ae, nae)[0]) - pa.pt(e(ae, 1)[0]),
            pa.pt(e(ae, m)[0]) - pa.pt(e(ae, 1)[0]),
            n1,
        );
        let angle2_m = angle_vectors(
            pa.pt(e(ae, m)[0]) - pa.pt(e(ae, 2)[0]),
            pa.pt(e(ae, 2)[1]) - pa.pt(e(ae, 2)[0]),
            n2,
        );
        if angle1_m < angle1 || angle2_m < angle2 {
            flag = 0;
        }
    }
    if flag == 1 {
        if *index_np == 0 || dist1 + dist2 < *edgedist {
            *index_np = m;
            *edgedist = dist1 + dist2;
        } else if e(ae, m)[0] == e(ae, *index_np)[0] {
            let em = pa.pt(e(ae, m)[0]);
            let n_m = (em - pa.c_sphere) / norm(em - pa.c_sphere);
            let a1 = angle_vectors(pa.pt(e(ae, 1)[0]) - em, pa.pt(e(ae, m)[1]) - em, n_m);
            let ei = pa.pt(e(ae, *index_np)[0]);
            let a2 = angle_vectors(
                pa.pt(e(ae, 1)[0]) - ei,
                pa.pt(e(ae, *index_np)[1]) - ei,
                n_m,
            );
            if m == nae || a1 > a2 {
                *index_np = m;
            }
        }
    }
}

/// Applies the collapse chosen by a dist sweep; returns true when the front was split.
fn dispatch(
    index_np: i32,
    index_nactive: usize,
    pa: &mut Patch,
    ae: &mut Front,
    ctx: &mut Ctx,
) -> bool {
    let nae = ae.len() as i32;
    if index_nactive == 0 {
        if index_np > 3 && index_np < nae {
            collapse_nonneighbor1(index_np, pa, ae, ctx);
            return true;
        } else if index_np == nae {
            collapse_neighbor(1, pa, ae);
        } else if index_np == 3 {
            collapse_neighbor(2, pa, ae);
        }
        false
    } else {
        collapse_nonneighbor2(index_np, index_nactive, pa, ae, ctx);
        true
    }
}

fn advancing_front_body(pa: &mut Patch, ae: &mut Front, ctx: &mut Ctx) -> Result<(), String> {
    let theta = 0.25;
    let alpha1 = FIVE_PI_OVER_3;
    let h = pa.d * 3.0f64.sqrt() / 2.0;
    let tolerance = pa.tolerance;
    let mut k = 1;
    while k <= pa.n_max {
        k += 1;
        super::check_size(pa.p.len() + 2, pa.t.len() + 3)?;
        let nae = ae.len() as i32;
        if nae < 3 {
            ae.clear();
            break;
        }
        if nae == 3 {
            pa.t.push([e(ae, 1)[0], e(ae, 1)[1], e(ae, 2)[1]]);
            ae.clear();
            break;
        }
        let e1 = e(ae, 1);
        let n1 = normal_sphere(pa.c_sphere, pa.pt(e1[0]));
        let n2 = normal_sphere(pa.c_sphere, pa.pt(e1[1]));
        let angle1 = angle_sphere(e(ae, nae), e1, n1, &pa.p);
        let angle2 = angle_sphere(e1, e(ae, 2), n2, &pa.p);
        let ne = ne_sphere(e1, n1, n2, &pa.p);
        let p = 0.5 * (pa.pt(e1[0]) + pa.pt(e1[1])) + h * ne;
        let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
        let angles = (angle1, angle2, n1, n2);

        let mut index_np = 0;
        let mut index_nactive = 0usize;
        let pa1 = pa.pt(e1[0]);
        let pa2 = pa.pt(e1[1]);
        let mut edgedist = 0.0;
        for m in 3..=nae {
            let pm1 = pa.pt(e(ae, m)[0]);
            let dist1_m = norm(pa1 - pm1);
            let dist2_m = norm(pa2 - pm1);
            if (dist1_m < tolerance
                || dist2_m < tolerance
                || dist1_m + dist2_m < tolerance * theta + norm(pa1 - pa2))
                && dot(ne, pm1 - pa1) > 0.0
                && (e1[0] != e(ae, m)[0] && e1[1] != e(ae, m)[0])
            {
                choose_neighbor(
                    pa,
                    ae,
                    m,
                    dist1_m,
                    dist2_m,
                    angles,
                    &mut index_np,
                    &mut edgedist,
                );
            }
        }
        for (i, af) in ctx.active.iter().enumerate().skip(1) {
            if af.meshed {
                continue;
            }
            for m in 1..af.ae.len() {
                let q = pa.pt(af.ae[m][0]);
                let dist1 = norm(pa.pt(e1[0]) - q);
                let dist2 = norm(pa.pt(e1[1]) - q);
                if (dist1 < tolerance
                    || dist2 < tolerance
                    || dist1 + dist2 < tolerance * theta + norm(pa.pt(e1[0]) - pa.pt(e1[1])))
                    && dot(ne, q - pa.pt(e1[0])) > 0.0
                    && (index_np == 0 || dist1 + dist2 < edgedist)
                {
                    index_np = m as i32;
                    edgedist = dist1 + dist2;
                    index_nactive = i;
                }
            }
        }
        if index_np > 1 {
            if dispatch(index_np, index_nactive, pa, ae, ctx) {
                break;
            }
            continue;
        }

        index_np = 0;
        index_nactive = 0;
        let pa1 = pa.pt(e1[0]);
        let nae_u = nae as usize;
        let mut dist = std::mem::take(&mut ctx.dist);
        if dist.len() < nae_u + 1 {
            dist.resize(nae_u + 1, 0.0);
        }
        for m in 1..=nae {
            dist[m as usize] = norm(x - pa.pt(e(ae, m)[0]));
        }
        edgedist = 0.0;
        for m in 3..=nae {
            let mu = m as usize;
            let m_next = (m % nae + 1) as usize;
            let em = e(ae, m);
            let em1 = e(ae, m - 1);
            let cond = dist[mu] < tolerance
                || dist[mu - 1] + dist[mu]
                    < tolerance * theta + norm(pa.pt(em1[0]) - pa.pt(em1[1]))
                || dist[mu] + dist[m_next] < tolerance * theta + norm(pa.pt(em[0]) - pa.pt(em[1]));
            if cond && dot(ne, pa.pt(em[0]) - pa1) > 0.0 && (e1[0] != em[0] && e1[1] != em[0]) {
                let dist1 = norm(pa.pt(e1[0]) - pa.pt(em[0]));
                let dist2 = norm(pa.pt(e1[1]) - pa.pt(em[0]));
                choose_neighbor(
                    pa,
                    ae,
                    m,
                    dist1,
                    dist2,
                    angles,
                    &mut index_np,
                    &mut edgedist,
                );
            }
        }
        ctx.dist = dist;
        for (i, af) in ctx.active.iter().enumerate().skip(1) {
            if af.meshed {
                continue;
            }
            for m in 1..af.ae.len() {
                let q = pa.pt(af.ae[m][0]);
                let dd = norm(x - q);
                if dd < tolerance && dot(ne, q - pa.pt(e1[0])) > 0.0 {
                    let dist1 = norm(pa.pt(e1[0]) - q);
                    let dist2 = norm(pa.pt(e1[1]) - q);
                    if index_np == 0 || dist1 + dist2 < edgedist {
                        index_np = m as i32;
                        edgedist = dist1 + dist2;
                        index_nactive = i;
                    }
                }
            }
        }
        if index_np > 0 {
            if dispatch(index_np, index_nactive, pa, ae, ctx) {
                break;
            }
            continue;
        }

        if angle1 > alpha1 && angle2 > alpha1 {
            let en = pa.pt(e(ae, nae)[0]);
            let e2 = pa.pt(e(ae, 2)[0]);
            let edgedist_nae = norm(en - pa.pt(e1[0])) + norm(en - pa.pt(e1[1]));
            let edgedist_2 = norm(e2 - pa.pt(e1[0])) + norm(e2 - pa.pt(e1[1]));
            if edgedist_nae < edgedist_2 {
                collapse_neighbor(1, pa, ae);
            } else {
                collapse_neighbor(2, pa, ae);
            }
            continue;
        } else if angle1 > alpha1 {
            let e2 = e(ae, 2);
            let angle1_2 = angle_vectors(
                pa.pt(e(ae, nae)[0]) - pa.pt(e2[0]),
                pa.pt(e2[1]) - pa.pt(e2[0]),
                n2,
            );
            if angle1_2 > angle2 {
                collapse_neighbor(1, pa, ae);
                continue;
            }
        } else if angle2 > alpha1 {
            let angle1_1 = angle_vectors(
                pa.pt(e(ae, nae)[0]) - pa.pt(e1[0]),
                pa.pt(e(ae, 2)[1]) - pa.pt(e1[0]),
                n1,
            );
            if angle1_1 > angle1 {
                collapse_neighbor(2, pa, ae);
                continue;
            }
        }

        pa.p.push(x);
        let np = pa.np();
        pa.t.push([e1[0], np, e1[1]]);
        ae.pop_front();
        ae.push_back([e1[0], np]);
        ae.push_back([np, e1[1]]);
    }
    Ok(())
}

fn collapse_neighbor(index_case: i32, pa: &mut Patch, ae: &mut Front) {
    let gamma = 1.8;
    let tolerance = pa.tolerance;
    let nae = ae.len() as i32;
    let e1 = e(ae, 1);
    if index_case == 1 {
        let en = e(ae, nae);
        if norm(pa.pt(en[0]) - pa.pt(e1[1])) < gamma * tolerance {
            pa.t.push([en[0], e1[1], e1[0]]);
            ae.pop_back();
            ae.pop_front();
            ae.push_back([en[0], e1[1]]);
        } else {
            let p = 0.5 * (pa.pt(en[0]) + pa.pt(e1[1]));
            let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
            if nae == 4 && norm(x - pa.pt(e(ae, 3)[0])) < 1e-10 {
                let e3 = e(ae, 3)[0];
                pa.t.push([en[0], e3, e1[0]]);
                pa.t.push([e3, e1[1], e1[0]]);
                ae.clear();
            } else {
                pa.p.push(x);
                let np = pa.np();
                pa.t.push([en[0], np, e1[0]]);
                pa.t.push([np, e1[1], e1[0]]);
                ae.pop_back();
                ae.pop_front();
                ae.push_back([en[0], np]);
                ae.push_back([np, e1[1]]);
            }
        }
    } else {
        let e2 = e(ae, 2);
        if norm(pa.pt(e1[0]) - pa.pt(e2[1])) < gamma * tolerance {
            pa.t.push([e1[0], e2[1], e1[1]]);
            ae.pop_front();
            ae.pop_front();
            ae.push_back([e1[0], e2[1]]);
        } else {
            let p = 0.5 * (pa.pt(e1[0]) + pa.pt(e2[1]));
            let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
            let en = e(ae, nae)[0];
            if nae == 4 && norm(x - pa.pt(en)) < 1e-10 {
                pa.t.push([e1[0], en, e2[0]]);
                pa.t.push([en, e2[1], e2[0]]);
                ae.clear();
            } else {
                pa.p.push(x);
                let np = pa.np();
                pa.t.push([e1[0], np, e2[0]]);
                pa.t.push([np, e2[1], e2[0]]);
                ae.pop_front();
                ae.pop_front();
                ae.push_back([e1[0], np]);
                ae.push_back([np, e2[1]]);
            }
        }
    }
}

fn collapse_nonneighbor1(m: i32, pa: &mut Patch, ae: &mut Front, ctx: &mut Ctx) {
    let gamma = 1.8;
    let tolerance = pa.tolerance;
    let nae = ae.len() as i32;
    let e1 = e(ae, 1);
    let e2 = e(ae, 2);
    let em = e(ae, m);
    let mut ae1 = Front::with_capacity(ae.len() + 2);
    let mut ae2 = Front::with_capacity(ae.len() + 2);
    let dm1 = norm(pa.pt(em[0]) - pa.pt(e1[0]));
    let dm2 = norm(pa.pt(em[0]) - pa.pt(e1[1]));
    let push_range = |dst: &mut Front, lo: i32, hi: i32| {
        for kk in lo..=hi {
            dst.push_back(e(ae, kk));
        }
    };
    if dm1 <= gamma * tolerance && dm2 <= gamma * tolerance {
        pa.t.push([em[0], e1[1], e1[0]]);
        push_range(&mut ae1, 2, m - 1);
        ae1.push_back([em[0], e1[1]]);
        push_range(&mut ae2, m, nae);
        ae2.push_back([e1[0], em[0]]);
    } else if dm1 > gamma * tolerance && dm2 > gamma * tolerance {
        let p = 0.5 * (pa.pt(e1[0]) + pa.pt(em[0]));
        let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
        pa.p.push(x);
        let p = 0.5 * (pa.pt(e2[0]) + pa.pt(em[0]));
        let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
        pa.p.push(x);
        let np = pa.np();
        if norm(pa.pt(em[0]) - pa.pt(e1[0])) > norm(pa.pt(em[0]) - pa.pt(e1[1])) {
            pa.t.push([e1[0], np - 1, e2[0]]);
            pa.t.push([np - 1, em[0], np]);
            pa.t.push([np, e2[0], np - 1]);
        } else {
            pa.t.push([e1[0], np - 1, np]);
            pa.t.push([np - 1, em[0], np]);
            pa.t.push([np, e2[0], e1[0]]);
        }
        push_range(&mut ae1, 2, m - 1);
        ae1.push_back([em[0], np]);
        ae1.push_back([np, e1[1]]);
        push_range(&mut ae2, m, nae);
        ae2.push_back([e1[0], np - 1]);
        ae2.push_back([np - 1, em[0]]);
    } else if dm1 > gamma * tolerance {
        let p = 0.5 * (pa.pt(e1[0]) + pa.pt(em[0]));
        let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
        pa.p.push(x);
        let np = pa.np();
        pa.t.push([e1[0], np, e2[0]]);
        pa.t.push([np, em[0], e1[1]]);
        push_range(&mut ae1, 2, m - 1);
        ae1.push_back([em[0], e1[1]]);
        push_range(&mut ae2, m, nae);
        ae2.push_back([e1[0], np]);
        ae2.push_back([np, em[0]]);
    } else if dm2 > gamma * tolerance {
        let p = 0.5 * (pa.pt(e2[0]) + pa.pt(em[0]));
        let x = map_sphere(pa.c_sphere, pa.r_sphere, p, pa.rp);
        pa.p.push(x);
        let np = pa.np();
        pa.t.push([em[0], np, e1[0]]);
        pa.t.push([np, e2[0], e1[0]]);
        push_range(&mut ae1, 2, m - 1);
        ae1.push_back([em[0], np]);
        ae1.push_back([np, e1[1]]);
        push_range(&mut ae2, m, nae);
        ae2.push_back([e1[0], em[0]]);
    }
    ctx.pending.push(ae2);
    ctx.pending.push(ae1);
}

fn collapse_nonneighbor2(
    m: i32,
    index_nactive: usize,
    pa: &mut Patch,
    ae: &mut Front,
    ctx: &mut Ctx,
) {
    ctx.active[index_nactive].meshed = true;
    let ae0 = std::mem::take(&mut ctx.active[index_nactive].ae);
    let mu = m as usize;
    let e1 = e(ae, 1);
    pa.t.push([ae0[mu][0], e1[1], e1[0]]);
    let mut ae1 = Front::with_capacity(ae.len() + ae0.len() + 1);
    ae1.extend(ae.iter().skip(1).copied());
    ae1.push_back([e1[0], ae0[mu][0]]);
    ae1.extend(ae0[mu..].iter().copied());
    ae1.extend(ae0[1..mu].iter().copied());
    ae1.push_back([ae0[mu][0], e1[1]]);
    ctx.active[index_nactive].ae = ae0;
    ctx.pending.push(ae1);
}

fn compute_nv(t: &[[i32; 3]], p: &[Vec3], c_sphere: Vec3, arg_nv: f64) -> Vec<Vec3> {
    t.iter()
        .map(|tri| {
            let a = p[tri[0] as usize];
            let b = p[tri[1] as usize];
            let c = p[tri[2] as usize];
            let mut nv = cross(b - a, c - a);
            nv = nv / norm(nv);
            let s = sign(dot(nv, a - c_sphere));
            if s != arg_nv {
                nv = nv * -1.0;
            }
            nv
        })
        .collect()
}

/// Meshes one convex or concave spherical patch with the advancing front.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mesh_sphpat(
    c_sphere: Vec3,
    r_sphere: f64,
    loops: &[Vec<i32>],
    segment0: &[[f64; 12]],
    circle0: &[[f64; 9]],
    patches: &[i32],
    patchesize: usize,
    rp: f64,
    d: f64,
    rj: Option<&[f64]>,
    boundary_tag: Option<Tag>,
) -> Result<LocalMesh, String> {
    let mut out = LocalMesh::default();
    let tolerance = 0.8 * cmin(d, r_sphere);
    if r_sphere == 0.0 || patchesize == 0 {
        return Ok(out);
    }
    let steps = (4.0 * PI * pysq(r_sphere) / (3.0f64.sqrt() * pysq(tolerance) / 4.0)).floor();
    if !steps.is_finite() || steps > super::MAX_FACES as f64 {
        return Err(super::SIZE_ERROR.into());
    }
    let n_max = steps as i32;
    let mut ctx = Ctx {
        rj_table: rj,
        ..Default::default()
    };
    let mut pa = Patch {
        c_sphere,
        r_sphere,
        rp,
        d,
        tolerance,
        n_max,
        t: Vec::new(),
        p: vec![Vec3::ZERO],
    };
    let mut ae = Front::new();
    let k = patches[1];
    if k > 0 {
        loop_division(&loops[k as usize], segment0, &mut ae, &mut pa, &mut ctx)?;
    } else {
        circle_division(&circle0[(-k) as usize], &mut ae, &mut pa, &mut ctx)?;
    }
    let nactive = patchesize.saturating_sub(1);
    ctx.active = Vec::with_capacity(nactive + 1);
    ctx.active.push(ActiveFront::default());
    let mut ring = Front::new();
    for i in 1..=nactive {
        let kk = patches[i + 1];
        if kk > 0 {
            loop_division(&loops[kk as usize], segment0, &mut ring, &mut pa, &mut ctx)?;
        } else {
            circle_division(&circle0[(-kk) as usize], &mut ring, &mut pa, &mut ctx)?;
        }
        let mut ae0 = Vec::with_capacity(ring.len() + 1);
        ae0.push([0, 0]);
        ae0.extend(ring.iter().copied());
        ctx.active.push(ActiveFront {
            meshed: false,
            ae: ae0,
        });
    }
    let np_boundary = pa.np() as usize;
    ctx.pending.push(ae);
    while let Some(mut front) = ctx.pending.pop() {
        advancing_front_body(&mut pa, &mut front, &mut ctx)?;
    }
    if !pa.t.is_empty() {
        let arg_nv = if r_sphere == rp { -1.0 } else { 1.0 };
        out.nv = compute_nv(&pa.t, &pa.p, c_sphere, arg_nv);
        if let Some(tag) = boundary_tag {
            let np = pa.np() as usize;
            out.vids = vec![Vec::new(); np];
            for v in &mut out.vids[..np_boundary] {
                v.push(tag);
            }
        }
        out.p = pa.p;
        out.t = pa.t;
        out.emit = true;
    }
    Ok(out)
}
