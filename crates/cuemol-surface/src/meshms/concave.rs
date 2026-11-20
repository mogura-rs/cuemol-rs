use super::meshing::{mesh_sphpat, nearest_atom, LocalMesh, MeshState, Tag};
use super::sas::{DataI, Neighbors};
use super::vec3::{
    acos_clamped, arc_angle, cmax, cmin, cross, dot, norm, orthogonalvectors, pysq, sign,
    stable_order, Vec3, TWO_PI,
};
use super::Geom;
use std::f64::consts::PI;

const EPSILON: f64 = 1.0e-10;

#[inline]
fn rget3<const N: usize>(row: &[f64; N], c0: usize) -> Vec3 {
    Vec3::new(row[c0], row[c0 + 1], row[c0 + 2])
}

#[inline]
fn rset3<const N: usize>(row: &mut [f64; N], c0: usize, v: Vec3) {
    row[c0] = v.x;
    row[c0 + 1] = v.y;
    row[c0 + 2] = v.z;
}

#[inline]
fn det3(r0: Vec3, r1: Vec3, r2: Vec3) -> f64 {
    dot(r0, cross(r1, r2))
}

#[inline]
fn alpha(u: Vec3, v: Vec3, n: Vec3) -> f64 {
    arc_angle(u, v, n, 1)
}

/// Is `x1` inside the tetrahedron `(x, xi, xj, xk)`?
fn test2(x1: Vec3, x: Vec3, xi: Vec3, xj: Vec3, xk: Vec3) -> bool {
    let d0 = sign(det3(xi - x, xj - x, xk - x));
    let d1 = sign(det3(x1 - x, xj - x, xk - x));
    let d2 = sign(det3(xi - x, x1 - x, xk - x));
    let d3 = sign(det3(xi - x, xj - x, x1 - x));
    d0 == d1 && d0 == d2 && d0 == d3
}

/// `circle` rows: `[_, c(3), r, j]`.
#[allow(clippy::too_many_arguments)]
fn test1(x1: Vec3, x: Vec3, xi: Vec3, xj: Vec3, xk: Vec3, k: usize, circle: &[[f64; 6]]) -> bool {
    for (i, row) in circle.iter().enumerate().skip(1) {
        if i != k && norm(x1 - rget3(row, 1)) <= row[4] - EPSILON {
            return false;
        }
    }
    let d0 = sign(det3(xi - x, xj - x, xk - x));
    let d1 = sign(det3(x1 - x, xj - x, xk - x));
    let d2 = sign(det3(xi - x, x1 - x, xk - x));
    !(d0 != d1 || d0 != d2)
}

/// Intersection of three probes of radius `r`.
fn intersection_three(c1: Vec3, c2: Vec3, c3: Vec3, r: f64) -> Option<(Vec3, Vec3)> {
    let a = 0.5 * (c1 + c2);
    let b = 0.5 * (c2 + c3);
    let cij = c1 - c2;
    let ckj = c3 - c2;
    let cr = cross(cij, ckj);
    let n = cr / norm(cr);
    let u = (c3 - c2) - dot(c3 - c2, c1 - c2) * (c1 - c2) / pysq(norm(c1 - c2));
    let t = dot(b - a, c3 - c2) / dot(u, c3 - c2);
    let x1 = a + t * u;
    let c = -pysq(norm(x1 - c1)) + pysq(r);
    if c > 0.0 {
        let sq = c.sqrt();
        Some((x1 - sq * n, x1 + sq * n))
    } else {
        None
    }
}

/// Intersection of two coplanar circles.
fn intersection_circle(c1: Vec3, r1: f64, c2: Vec3, r2: f64, n: Vec3) -> (Vec3, Vec3) {
    let dd = norm(c1 - c2);
    let t = (pysq(r1) - pysq(r2) + pysq(dd)) / (2.0 * dd);
    let v = (c2 - c1) / norm(c2 - c1);
    let o = c1 + t * v;
    let mut u = cross(n, v);
    u = u / norm(u);
    let h = cmax(pysq(r1) - pysq(t), 0.0).sqrt();
    (o + h * u, o - h * u)
}

#[derive(Default)]
struct TreeNode {
    activenode: i32,
    set: Vec<i32>,
    activeelement: i32,
    n1: usize,
    n2: usize,
    leftnode: usize,
}

/// Reorders `boundary[1..=n]` by angle about `n_axis` (rows `[_, x(3), j]`).
fn sort_boundary(c: Vec3, boundary: &mut [[f64; 5]], n: usize, n_axis: Vec3) {
    let mut alpha1 = vec![0.0; n + 1];
    let x0 = rget3(&boundary[1], 1);
    for k in 2..=n {
        alpha1[k] = alpha(x0 - c, rget3(&boundary[k], 1) - c, n_axis);
    }
    let order = stable_order(&alpha1, n);
    let sorted: Vec<[f64; 5]> = order[1..].iter().map(|&o| boundary[o]).collect();
    boundary[1..=n].copy_from_slice(&sorted);
}

struct SortSegOut {
    a: Vec<f64>,
    k1: Vec<i32>,
    k2: Vec<i32>,
}

/// Probe-local context shared by the decomposition helpers.
struct ProbeCtx<'a> {
    pts: &'a [Vec3],
    hight_set: &'a [i32],
    k: Vec<i32>,
    probe: f64,
    x: Vec3,
    xi: Vec3,
    xj: Vec3,
    xk: Vec3,
}

impl ProbeCtx<'_> {
    #[inline]
    fn center(&self, idx: usize) -> Vec3 {
        self.pts[self.hight_set[self.k[idx] as usize] as usize]
    }
    #[inline]
    fn kn(&self) -> usize {
        self.k.len() - 1
    }
}

fn sort_segment(
    pc: &ProbeCtx,
    j: usize,
    atom_row: &[i32],
    c: Vec3,
    n: Vec3,
    r: f64,
    i_probe: &[Vec3],
) -> SortSegOut {
    let nn = atom_row.len() - 1;
    let point: Vec<Vec3> = atom_row
        .iter()
        .enumerate()
        .map(|(k, &a)| {
            if k == 0 {
                Vec3::ZERO
            } else {
                i_probe[a as usize]
            }
        })
        .collect();
    let mut alpha1 = vec![0.0; nn + 1];
    let x0 = point[1];
    for k in 2..=nn {
        alpha1[k] = alpha(x0 - c, point[k] - c, n);
    }
    let order = stable_order(&alpha1, nn);
    let mut out = SortSegOut {
        a: vec![0.0],
        k1: vec![0],
        k2: vec![0],
    };
    for k in 1..=nn {
        let knext = k % nn + 1;
        let pk = point[order[k]];
        let pkn = point[order[knext]];
        if norm(pk - pkn) > EPSILON {
            let mut x_middle = 0.5 * (pk + pkn);
            if norm(x_middle - c) != 0.0 {
                x_middle = c + (x_middle - c) / norm(x_middle - c) * r;
            } else {
                let v1 = pk - c;
                let u = -1.0 * cross(v1, n);
                x_middle = c + r * u / norm(u);
            }
            if alpha(pk - c, x_middle - c, n) > PI {
                x_middle = 2.0 * c - x_middle;
            }
            let blocked = (1..=pc.kn()).any(|i| i != j && norm(x_middle - pc.center(i)) < pc.probe);
            if !blocked && test2(x_middle, pc.x, pc.xi, pc.xj, pc.xk) {
                let theta = alpha1[order[knext]] - alpha1[order[k]];
                out.a.push(if theta < 0.0 { theta + TWO_PI } else { theta });
                out.k1.push(atom_row[order[k]]);
                out.k2.push(atom_row[order[knext]]);
            }
        }
    }
    out
}

/// `segment` rows: `[_, c(3), n(3), r, angle, k1, k2]`.
fn interiorloop_concave(
    point: Vec3,
    x: Vec3,
    probe: f64,
    lp: &[i32],
    i_probe: &[Vec3],
    segment: &[[f64; 11]],
    direct: i32,
) -> bool {
    let loopsize = lp.len() - 1;
    let mut theta0 = 0.0;
    let mut nearest = 0;
    for k in 1..=loopsize {
        let seg = &segment[lp[k] as usize];
        let mut v1 = rget3(seg, 4) * probe;
        if direct == -1 {
            v1 = -1.0 * v1;
        }
        let v2 = i_probe[seg[9] as i32 as usize] - x;
        let v3 = point - x;
        let theta =
            acos_clamped(dot(v3, v1) / pysq(probe)) - acos_clamped(dot(v2, v1) / pysq(probe));
        if k == 1 || theta < theta0 {
            theta0 = theta;
            nearest = k;
        }
    }
    if theta0 < 0.0 {
        return false;
    }
    let segn = &segment[lp[nearest] as usize];
    for k in 1..=loopsize {
        let seg = &segment[lp[k] as usize];
        if norm(rget3(seg, 1) - rget3(segn, 1)) == 0.0 && (seg[7] - segn[7]).abs() == 0.0 {
            let mut v1 = rget3(seg, 4) * probe;
            if direct == -1 {
                v1 = -1.0 * v1;
            }
            let v3 = point - x;
            let v = v3 - (dot(v1, v3) * v1) / pysq(probe);
            let n = rget3(seg, 4);
            let a = rget3(seg, 1);
            let p9 = i_probe[seg[9] as i32 as usize];
            let p10 = i_probe[seg[10] as i32 as usize];
            if alpha(p9 - a, v, n) < alpha(p9 - a, p10 - a, n) {
                return true;
            }
        }
    }
    false
}

fn loopconstruct_concave(
    segment: &[[f64; 11]],
    n_segment: usize,
    map_same: &[i32],
) -> Vec<Vec<i32>> {
    let s0 = |k: usize| map_same[segment[k][9] as i32 as usize];
    let s1 = |k: usize| map_same[segment[k][10] as i32 as usize];
    let mut used = vec![false; n_segment + 1];
    let mut ntrue = 0;
    let mut loops = vec![Vec::new()];
    for k1 in 1..=n_segment {
        if !used[k1] && ntrue < n_segment {
            let mut pointer = k1;
            ntrue += 1;
            used[k1] = true;
            let mut lp = vec![0, pointer as i32];
            let mut times = 0;
            while s0(k1) != s1(pointer) {
                for k2 in k1 + 1..=n_segment {
                    if !used[k2] && s0(k2) == s1(pointer) {
                        pointer = k2;
                        lp.push(pointer as i32);
                        used[pointer] = true;
                        ntrue += 1;
                    }
                }
                times += 1;
                if times > 100 {
                    break;
                }
            }
            loops.push(lp);
        }
    }
    loops
}

/// `circle` rows: `[_, c(3), n(3), r]`. Returns patches (1-based, `[0]` dummy rows).
fn patchesconstruct_concave(
    ci_sphere: Vec3,
    i_probe: &[Vec3],
    segment: &[[f64; 11]],
    circle: &[[f64; 8]],
    loops: &[Vec<i32>],
    direct: i32,
    rp: f64,
) -> Vec<Vec<i32>> {
    let nloops = loops.len() - 1;
    let n_circle = circle.len() - 1;
    let mut s0: Vec<i32> = (1..=nloops as i32).collect();
    s0.extend((1..=n_circle as i32).map(|t| -t));
    let mut tree = vec![
        TreeNode::default(),
        TreeNode {
            activenode: 1,
            activeelement: s0.first().copied().unwrap_or(0),
            set: s0,
            n1: nloops,
            n2: n_circle,
            leftnode: 0,
        },
    ];
    let mut ntree = 1;
    let mut j = 1;
    for _ in 1..=2 * (nloops + n_circle) + 1 {
        if j > ntree {
            break;
        }
        if tree[j].activenode != 1 {
            j += 1;
            continue;
        }
        if tree[j].set.len() == 1 {
            tree[j].activenode = 0;
            j += 1;
            continue;
        }
        let k = tree[j].activeelement;
        if k <= 0 {
            tree[j].activenode = 0;
            j += 1;
            continue;
        }
        let (mut s1v, mut s2v) = (Vec::new(), Vec::new());
        let (mut k1, mut k2) = (0, 0);
        let (mut t1, mut t2) = (true, true);
        let (mut left_n1, mut left_n2, mut right_n1, mut right_n2) = (0, 0, 0, 0);
        let loopk = &loops[k as usize];
        let node = &tree[j];
        for &ssi in &node.set[..node.n1] {
            let point = i_probe[segment[loops[ssi as usize][1] as usize][9] as i32 as usize];
            if ssi == k
                || interiorloop_concave(point, ci_sphere, rp, loopk, i_probe, segment, direct)
            {
                s1v.push(ssi);
                left_n1 += 1;
                if t1 && ssi > k {
                    t1 = false;
                    k1 = ssi;
                }
            } else {
                s2v.push(ssi);
                right_n1 += 1;
                if t2 && ssi > k {
                    t2 = false;
                    k2 = ssi;
                }
            }
        }
        for &ssi in &node.set[node.n1..node.n1 + node.n2] {
            let crow = &circle[(-ssi) as usize];
            let (vector1, _) = orthogonalvectors(rget3(crow, 4));
            let point = rget3(crow, 1) + crow[7] * vector1;
            if interiorloop_concave(point, ci_sphere, rp, loopk, i_probe, segment, direct) {
                s1v.push(ssi);
                left_n2 += 1;
                if t1 {
                    t1 = false;
                    k1 = ssi;
                }
            } else {
                s2v.push(ssi);
                right_n2 += 1;
                if t2 {
                    t2 = false;
                    k2 = ssi;
                }
            }
        }
        if s2v.is_empty() {
            if t1 {
                tree[j].activenode = 0;
                j += 1;
            } else {
                tree[j].activeelement = k1;
            }
        } else {
            tree[j].activenode = 0;
            tree[j].leftnode = ntree + 1;
            ntree += 2;
            tree.push(TreeNode {
                activenode: if t1 { 0 } else { 1 },
                set: s1v,
                activeelement: k1,
                n1: left_n1,
                n2: left_n2,
                leftnode: 0,
            });
            tree.push(TreeNode {
                activenode: if t2 { 0 } else { 1 },
                set: s2v,
                activeelement: k2,
                n1: right_n1,
                n2: right_n2,
                leftnode: 0,
            });
            j += 1;
        }
    }
    let mut patches = vec![Vec::new()];
    for node in &tree[1..=ntree] {
        if node.activenode == 0 && node.leftnode == 0 {
            let mut row = Vec::with_capacity(node.set.len() + 1);
            row.push(0);
            row.extend_from_slice(&node.set);
            patches.push(row);
        }
    }
    patches
}

/// One cSAS probe's density-independent decomposition.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProbePatchSet {
    k1: usize,
    loops: Vec<Vec<i32>>,
    segment0: Vec<[f64; 12]>,
    circle0: Vec<[f64; 9]>,
    patches: Vec<Vec<i32>>,
    atoms: [i32; 3],
    centers: [Vec3; 3],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConcaveDecomp {
    probes: Vec<ProbePatchSet>,
}

fn decomp_construct_concavepat(
    k1: usize,
    i_probe: &[Vec3],
    segment: &[[f64; 11]],
    circle_interior: &[[f64; 8]],
    direct: i32,
    pts: &[Vec3],
    rp: f64,
) -> ProbePatchSet {
    let n_probe = i_probe.len() - 1;
    let n_segment = segment.len() - 1;
    let mut map_same = vec![0i32; n_probe + 1];
    let mut samepoint_rep: Vec<usize> = vec![0];
    for i in 1..=n_probe {
        let found = samepoint_rep[1..]
            .iter()
            .position(|&rep| norm(i_probe[rep] - i_probe[i]) < EPSILON);
        match found {
            Some(jj) => map_same[i] = (jj + 1) as i32,
            None => {
                samepoint_rep.push(i);
                map_same[i] = (samepoint_rep.len() - 1) as i32;
            }
        }
    }
    let mut loops = loopconstruct_concave(segment, n_segment, &map_same);
    let patches = patchesconstruct_concave(
        pts[k1],
        i_probe,
        segment,
        circle_interior,
        &loops,
        direct,
        rp,
    );

    let mut segment0 = vec![[0.0; 12]; n_segment + 1];
    for (s0row, seg) in segment0.iter_mut().zip(segment).skip(1) {
        if direct == 1 {
            s0row[1..=7].copy_from_slice(&seg[1..=7]);
            rset3(s0row, 8, i_probe[seg[9] as i32 as usize]);
        } else {
            rset3(s0row, 1, rget3(seg, 1));
            rset3(s0row, 4, -1.0 * rget3(seg, 4));
            s0row[7] = seg[7];
            rset3(s0row, 8, i_probe[seg[10] as i32 as usize]);
        }
        s0row[11] = seg[8];
    }
    let mut circle0 = vec![[0.0; 9]; circle_interior.len()];
    for (c0, ci) in circle0.iter_mut().zip(circle_interior).skip(1) {
        c0[1..=7].copy_from_slice(&ci[1..=7]);
        if direct == -1 {
            rset3(c0, 4, -1.0 * rget3(ci, 4));
        }
    }
    if direct == -1 {
        for lp in &mut loops[1..] {
            lp[1..].reverse();
        }
    }
    ProbePatchSet {
        k1,
        loops,
        segment0,
        circle0,
        patches,
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn data_concavepat(
    i: usize,
    hight_set: &[i32],
    k_in: &[i32],
    pts: &[Vec3],
    iijk: &[[i32; 3]],
    c: &[Vec3],
    probe: f64,
    direction: &[[i32; 3]],
) -> ProbePatchSet {
    let i0 = hight_set[i] as usize;
    let [a_i, a_j, a_k] = iijk[i0];
    let dir_i0 = direction[i0][0];
    let x = pts[i0];
    let ci = c[a_i as usize];
    let cj = c[a_j as usize];
    let ck = c[a_k as usize];
    let ni = (ci - x) / norm(ci - x);
    let nj = (cj - x) / norm(cj - x);
    let nk = (ck - x) / norm(ck - x);
    let mut nij = cross(ni, nj);
    let mut njk = cross(nj, nk);
    let mut nki = cross(nk, ni);
    nij = nij / norm(nij);
    njk = njk / norm(njk);
    nki = nki / norm(nki);
    let xi = x + probe * ni;
    let xj = x + probe * nj;
    let xk = x + probe * nk;
    let any_eq = |a: i32, j0: usize| iijk[j0].contains(&a);

    let kn_in = k_in.len() - 1;
    let mut remove = vec![false; kn_in + 1];
    let edge_angle = |n_a: Vec3, c_a: Vec3, c_b: Vec3, x0: Vec3| {
        let cba = c_b - c_a;
        let u = -1.0 * n_a - (dot(-1.0 * n_a, cba) / norm(cba)) * cba / norm(cba);
        let v = (x0 - c_a) - (dot(x0 - c_a, cba) / norm(cba)) * cba / norm(cba);
        let alpha0 = alpha(u, v, cba);
        if dir_i0 == -1 {
            TWO_PI - alpha0
        } else {
            alpha0
        }
    };
    // (count, best index, best angle) per edge ij, jk, ki
    let mut best = [(0, 0usize, 0.0f64); 3];
    let edges = [
        (a_i, a_j, ni, ci, cj),
        (a_j, a_k, nj, cj, ck),
        (a_k, a_i, nk, ck, ci),
    ];
    for j in 1..=kn_in {
        let j0 = hight_set[k_in[j] as usize] as usize;
        let x0 = pts[j0];
        for (e, &(aa, ab, n_a, c_a, c_b)) in edges.iter().enumerate() {
            if any_eq(aa, j0) && any_eq(ab, j0) {
                remove[j] = true;
                best[e].0 += 1;
                let alpha0 = edge_angle(n_a, c_a, c_b, x0);
                if best[e].0 == 1 || alpha0 > best[e].2 {
                    best[e].1 = j;
                    best[e].2 = alpha0;
                }
            }
        }
    }
    for b in &best {
        if b.1 > 0 {
            remove[b.1] = false;
        }
    }
    let mut k = vec![0i32];
    k.extend((1..=kn_in).filter(|&j| !remove[j]).map(|j| k_in[j]));
    let pc = ProbeCtx {
        pts,
        hight_set,
        k,
        probe,
        x,
        xi,
        xj,
        xk,
    };
    let kn = pc.kn();

    let mut circle_ij = vec![[0.0; 6]];
    let mut circle_jk = vec![[0.0; 6]];
    let mut circle_ki = vec![[0.0; 6]];
    for j in 1..=kn {
        let x0 = pc.center(j);
        for (circles, nvec) in [
            (&mut circle_ij, nij),
            (&mut circle_jk, njk),
            (&mut circle_ki, nki),
        ] {
            let t = dot(x - x0, nvec);
            if t.abs() < probe {
                let cc = x0 + t * nvec;
                let rr = cmax(pysq(probe) - pysq(t), 0.0).sqrt();
                let mut row = [0.0; 6];
                rset3(&mut row, 1, cc);
                row[4] = rr;
                row[5] = j as f64;
                circles.push(row);
            }
        }
    }

    let min_abs_dist = |boundary: &[[f64; 5]], xpt: Vec3| {
        let mut best = f64::MAX;
        for row in &boundary[1..] {
            let d3 = rget3(row, 1) - xpt;
            best = cmin(best, d3.x.abs() + d3.y.abs() + d3.z.abs());
        }
        best
    };
    let boundary_for = |circles: &[[f64; 6]], nvec: Vec3, p1: Vec3, p2: Vec3, tri: [Vec3; 3]| {
        let mut boundary = vec![[0.0; 5]; 3];
        rset3(&mut boundary[1], 1, p1);
        rset3(&mut boundary[2], 1, p2);
        for (kk, crow) in circles.iter().enumerate().skip(1) {
            let cc = rget3(crow, 1);
            let rr = crow[4];
            let nrm = norm(cc - x);
            if probe - rr < nrm && nrm < probe + rr {
                let (x1, x2) = intersection_circle(x, probe, cc, rr, nvec);
                for xp in [x1, x2] {
                    if test1(xp, x, tri[0], tri[1], tri[2], kk, circles)
                        && min_abs_dist(&boundary, xp) > EPSILON
                    {
                        let mut row = [0.0; 5];
                        rset3(&mut row, 1, xp);
                        row[4] = crow[5];
                        boundary.push(row);
                    }
                }
            }
        }
        boundary
    };
    let mut boundary_ij = boundary_for(&circle_ij, nij, xi, xj, [xi, xj, xk]);
    let mut boundary_jk = boundary_for(&circle_jk, njk, xj, xk, [xj, xk, xi]);
    let mut boundary_ki = boundary_for(&circle_ki, nki, xk, xi, [xk, xi, xj]);

    let mut i_interior: Vec<(Vec3, usize, usize)> = vec![(Vec3::ZERO, 0, 0)];
    for j in 1..=kn {
        let x0 = pc.center(j);
        for kk in j + 1..=kn {
            let y0 = pc.center(kk);
            if let Some((x1, x2)) = intersection_three(x, x0, y0, probe) {
                for xp in [x1, x2] {
                    if test2(xp, x, xi, xj, xk) {
                        let blocked =
                            (1..=kn).any(|t| t != j && t != kk && norm(xp - pc.center(t)) < probe);
                        if !blocked {
                            i_interior.push((xp, j, kk));
                        }
                    }
                }
            }
        }
    }

    let n_ij = boundary_ij.len() - 1;
    let n_jk = boundary_jk.len() - 1;
    let n_ki = boundary_ki.len() - 1;
    sort_boundary(x, &mut boundary_ij, n_ij, nij);
    sort_boundary(x, &mut boundary_jk, n_jk, njk);
    sort_boundary(x, &mut boundary_ki, n_ki, nki);

    let n_boundary = n_ij + n_jk + n_ki;
    let n_interior = i_interior.len() - 1;
    let n_probe = n_boundary + n_interior;
    let mut i_probe = Vec::with_capacity(n_probe + 1);
    i_probe.push(Vec3::ZERO);
    for b in [&boundary_ij, &boundary_jk, &boundary_ki] {
        i_probe.extend(b[1..].iter().map(|row| rget3(row, 1)));
    }
    i_probe.extend(i_interior[1..].iter().map(|e| e.0));

    let mut atom: Vec<Vec<i32>> = vec![vec![0]; kn + 1];
    for (j, row) in atom.iter_mut().enumerate().skip(1) {
        for kk in 1..=n_probe {
            if kk <= n_boundary {
                let owner = if kk <= n_ij {
                    boundary_ij[kk][4]
                } else if kk <= n_ij + n_jk {
                    boundary_jk[kk - n_ij][4]
                } else {
                    boundary_ki[kk - n_ij - n_jk][4]
                };
                if owner as i32 as usize == j {
                    row.push(kk as i32);
                }
            } else {
                let e = &i_interior[kk - n_boundary];
                if e.1 == j {
                    row.push(kk as i32);
                }
                if e.2 == j {
                    row.push(kk as i32);
                }
            }
        }
    }

    let mut circle_interior = vec![[0.0; 8]];
    for (j, atom_j) in atom.iter().enumerate().skip(1) {
        if atom_j.len() == 1 {
            let x1 = 0.5 * (pc.center(j) + x);
            if test2(x1, x, xi, xj, xk) {
                let r1 = cmax(pysq(probe) - pysq(norm(x - x1)), 0.0).sqrt();
                let n1 = dir_i0 as f64 * (x1 - x) / norm(x1 - x);
                let blocked = (1..=kn).any(|kk| j != kk && norm(x1 - pc.center(kk)) < probe);
                if !blocked {
                    let mut row = [0.0; 8];
                    rset3(&mut row, 1, x1);
                    rset3(&mut row, 4, n1);
                    row[7] = r1;
                    circle_interior.push(row);
                }
            }
        }
    }

    let mut segment: Vec<[f64; 11]> = vec![[0.0; 11]];
    let mut j = 1;
    while j < n_boundary {
        let uj = i_probe[j] - x;
        let vj = i_probe[j + 1] - x;
        let aj = acos_clamped(dot(uj, vj) / pysq(probe));
        let (nvec, j1) = if j <= n_ij {
            (nij, if j + 1 < n_ij { j + 1 } else { j + 2 })
        } else if j <= n_ij + n_jk {
            (njk, if j + 1 < n_ij + n_jk { j + 1 } else { j + 2 })
        } else {
            (nki, if j + 1 < n_boundary { j + 1 } else { 1 })
        };
        let mut row = [0.0; 11];
        rset3(&mut row, 1, x);
        rset3(&mut row, 4, nvec);
        row[7] = probe;
        row[8] = aj;
        row[9] = j as f64;
        row[10] = j1 as f64;
        segment.push(row);
        j += 2;
    }
    for (j, atom_j) in atom.iter().enumerate().skip(1) {
        if atom_j.len() > 1 {
            let hk = pc.center(j);
            let x1 = 0.5 * (hk + x);
            let r1 = cmax(pysq(probe) - pysq(norm(x - x1)), 0.0).sqrt();
            let n1 = dir_i0 as f64 * (hk - x) / norm(hk - x);
            let ss = sort_segment(&pc, j, atom_j, x1, n1, r1, &i_probe);
            for t in 1..ss.a.len() {
                let mut row = [0.0; 11];
                rset3(&mut row, 1, x1);
                rset3(&mut row, 4, n1);
                row[7] = r1;
                row[8] = ss.a[t];
                row[9] = ss.k1[t] as f64;
                row[10] = ss.k2[t] as f64;
                segment.push(row);
            }
        }
    }

    let mut pd =
        decomp_construct_concavepat(i0, &i_probe, &segment, &circle_interior, dir_i0, pts, probe);
    pd.atoms = [a_i, a_j, a_k];
    pd.centers = [ci, cj, ck];
    pd
}

fn build_neighbors(
    hight_set: &[i32],
    inverse_hight: &[i32],
    di: &DataI,
    inter: &Neighbors,
    rp: f64,
) -> Vec<Vec<i32>> {
    let pts = &di.i;
    (0..hight_set.len())
        .map(|i| {
            if i == 0 {
                return Vec::new();
            }
            let hs = hight_set[i] as usize;
            let x1 = pts[hs];
            let i1 = di.iijk[hs][0] as usize;
            let mut row: Vec<i32> = Vec::new();
            for &j1 in inter.of(i1) {
                for &k1 in &di.ii[j1 as usize] {
                    let vid = inverse_hight[k1 as usize];
                    if k1 as usize != hs
                        && norm(x1 - pts[k1 as usize]) < 2.0 * rp
                        && vid != 0
                        && !row.contains(&vid)
                    {
                        row.push(vid);
                    }
                }
            }
            row
        })
        .collect()
}

/// Density-independent concave decomposition of every cSAS probe.
pub(crate) fn precompute_concave(
    geom: &Geom,
    di: &DataI,
    rp: f64,
    inter: &Neighbors,
) -> ConcaveDecomp {
    let s = di.n_i;
    let mut hight_set = vec![0i32];
    let mut inverse_hight = vec![0i32; s + 1];
    for (i, inverse) in inverse_hight.iter_mut().enumerate().skip(1) {
        if di.high_i[i] == 0 {
            hight_set.push(i as i32);
            *inverse = (hight_set.len() - 1) as i32;
        }
    }
    let neighbor = build_neighbors(&hight_set, &inverse_hight, di, inter, rp);
    let mut probes = vec![ProbePatchSet::default()];
    for (i, row) in neighbor.iter().enumerate().skip(1) {
        let mut k_in = Vec::with_capacity(row.len() + 1);
        k_in.push(0);
        k_in.extend_from_slice(row);
        probes.push(data_concavepat(
            i,
            &hight_set,
            &k_in,
            &di.i,
            &di.iijk,
            &geom.centers,
            rp,
            &di.direction,
        ));
    }
    ConcaveDecomp { probes }
}

fn fill_vatom_nearest3(lm: &mut LocalMesh, atoms: &[i32; 3], centers: &[Vec3; 3]) {
    if !lm.emit {
        return;
    }
    lm.vatom = lm.p[1..]
        .iter()
        .map(|&p| nearest_atom(p, atoms, centers))
        .collect();
}

/// Meshes the concave patches: the cSAS probe decompositions, then the simple triangles.
pub(crate) fn sesconcavepat_mesh(
    state: &mut MeshState,
    geom: &Geom,
    di: &DataI,
    decomp: &ConcaveDecomp,
    rp: f64,
    d: f64,
) -> Result<(), String> {
    let pts = &di.i;
    for pd in &decomp.probes[1..] {
        let tag = Tag {
            kind: 2,
            i: pd.k1 as i32,
            j: 0,
        };
        for prow in &pd.patches[1..] {
            let mut lm = mesh_sphpat(
                pts[pd.k1],
                rp,
                &pd.loops,
                &pd.segment0,
                &pd.circle0,
                prow,
                prow.len() - 1,
                rp,
                d,
                None,
                Some(tag),
            )?;
            fill_vatom_nearest3(&mut lm, &pd.atoms, &pd.centers);
            state.add_patch(lm)?;
        }
    }

    let c_atoms = &geom.centers;
    let r = &geom.r;
    for (i, &c) in pts.iter().enumerate().take(di.n_i + 1).skip(1) {
        if di.high_i[i] != 1 {
            continue;
        }
        let [a_i, a_j, a_k] = di.iijk[i];
        let ci = c_atoms[a_i as usize];
        let cj = c_atoms[a_j as usize];
        let ck = c_atoms[a_k as usize];
        let ni = (ci - c) / norm(ci - c);
        let nj = (cj - c) / norm(cj - c);
        let nk = (ck - c) / norm(ck - c);
        let mut nij = cross(ni, nj);
        let mut njk = cross(nj, nk);
        let mut nki = cross(nk, ni);
        nij = nij / norm(nij);
        njk = njk / norm(njk);
        nki = nki / norm(nki);
        let mut spoint_i = c + (ci - c) * rp / (r[a_i as usize] + rp);
        let mut spoint_j = c + (cj - c) * rp / (r[a_j as usize] + rp);
        let mut spoint_k = c + (ck - c) * rp / (r[a_k as usize] + rp);
        let angle_i = acos_clamped(dot(spoint_i - c, spoint_j - c) / pysq(rp));
        let angle_j = acos_clamped(dot(spoint_j - c, spoint_k - c) / pysq(rp));
        let angle_k = acos_clamped(dot(spoint_k - c, spoint_i - c) / pysq(rp));
        let loop1 = if di.direction[i][0] == -1 {
            nij = -1.0 * nij;
            njk = -1.0 * njk;
            nki = -1.0 * nki;
            let si = spoint_i;
            spoint_i = spoint_j;
            spoint_j = spoint_k;
            spoint_k = si;
            vec![0, 1, 3, 2]
        } else {
            vec![0, 1, 2, 3]
        };
        let loops = vec![vec![0], loop1];
        let mut segment0 = vec![[0.0; 12]; 4];
        for (row, (nv, sp, ang)) in segment0.iter_mut().skip(1).zip([
            (nij, spoint_i, angle_i),
            (njk, spoint_j, angle_j),
            (nki, spoint_k, angle_k),
        ]) {
            rset3(row, 1, c);
            rset3(row, 4, nv);
            row[7] = rp;
            rset3(row, 8, sp);
            row[11] = ang;
        }
        let tag = Tag {
            kind: 2,
            i: i as i32,
            j: 0,
        };
        let mut lm = mesh_sphpat(
            c,
            rp,
            &loops,
            &segment0,
            &[],
            &[0, 1],
            1,
            rp,
            d,
            None,
            Some(tag),
        )?;
        fill_vatom_nearest3(&mut lm, &[a_i, a_j, a_k], &[ci, cj, ck]);
        state.add_patch(lm)?;
    }
    Ok(())
}
