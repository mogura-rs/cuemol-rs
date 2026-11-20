use super::sas::{DataCir, DataI, Neighbors};
use super::vec3::{
    acos_clamped, arc_angle, circlecenter, cmax, dot, norm, orthogonalvectors, pysq, stable_order,
    Vec3,
};
use super::Geom;

/// SAS circular segments: `segment[k] = {i, j, p1, p2, direct}`,
/// `ncrasegment[k] = {n(3), A(3), r, radian}`; `[0]` dummy.
#[derive(Debug, Clone, Default)]
pub(crate) struct DataSeg {
    pub segment: Vec<[i32; 5]>,
    pub nsegment: usize,
    pub ncrasegment: Vec<[f64; 8]>,
    pub satom: Vec<Vec<i32>>,
}

/// SAS loops: `loops[k]` holds global segment ids (1-based, `[0]` dummy).
#[derive(Debug, Clone, Default)]
pub(crate) struct DataLoop {
    pub loops: Vec<Vec<i32>>,
    pub loops_index: Vec<[i32; 2]>,
}

/// SAS spherical patches: signed entries, `+loop` / `-circle` local to the patch atom.
#[derive(Debug, Clone, Default)]
pub(crate) struct DataPat {
    pub patches: Vec<Vec<i32>>,
    pub npatches: usize,
    pub patches_index: Vec<[i32; 2]>,
    pub patch_atom: Vec<i32>,
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

struct LoopPre {
    u1: Vec<Vec3>,
    jat: Vec<i32>,
    b: Vec<f64>,
    alpha2: Vec<f64>,
    direct: Vec<i32>,
    p3a: Vec<Vec3>,
    n: Vec<Vec3>,
}

#[allow(clippy::too_many_arguments)]
fn build_loop_pre(
    i: i32,
    ci: Vec3,
    ri: f64,
    lp: &[i32],
    loopsize: usize,
    center: &[Vec3],
    pts: &[Vec3],
    segment: &[[i32; 5]],
    ncrasegment: &[[f64; 8]],
) -> LoopPre {
    let mut pk = LoopPre {
        u1: vec![Vec3::ZERO; loopsize + 1],
        jat: vec![0; loopsize + 1],
        b: vec![0.0; loopsize + 1],
        alpha2: vec![0.0; loopsize + 1],
        direct: vec![0; loopsize + 1],
        p3a: vec![Vec3::ZERO; loopsize + 1],
        n: vec![Vec3::ZERO; loopsize + 1],
    };
    for k in 1..=loopsize {
        let seg = &segment[lp[k] as usize];
        let ncra = &ncrasegment[lp[k] as usize];
        let j = if seg[0] == i { seg[1] } else { seg[0] } as usize;
        let u1 = (center[j] - ci) / norm(center[j] - ci);
        let v1 = u1 * ri;
        let v2 = pts[seg[2] as usize] - ci;
        let n = Vec3::new(ncra[0], ncra[1], ncra[2]);
        let a = Vec3::new(ncra[3], ncra[4], ncra[5]);
        let p3 = pts[seg[2] as usize];
        let p4 = pts[seg[3] as usize];
        pk.u1[k] = u1;
        pk.jat[k] = j as i32;
        pk.b[k] = acos_clamped(dot(v2, v1) / pysq(ri));
        pk.alpha2[k] = arc_angle(p3 - a, p4 - a, n, seg[4]);
        pk.direct[k] = seg[4];
        pk.p3a[k] = p3 - a;
        pk.n[k] = n;
    }
    pk
}

fn interiorloop(point: Vec3, ci: Vec3, ri: f64, loopsize: usize, pk: &LoopPre) -> bool {
    let mut nearest = 0;
    let mut theta0 = 0.0;
    let v3 = point - ci;
    let ri2 = pysq(ri);
    for k in 1..=loopsize {
        let v1 = pk.u1[k] * ri;
        let theta = acos_clamped(dot(v3, v1) / ri2) - pk.b[k];
        if k == 1 || theta < theta0 {
            theta0 = theta;
            nearest = k;
        }
    }
    let j_nearest = pk.jat[nearest];
    let v1 = pk.u1[nearest];
    let v = v3 - dot(v1, v3) * v1;
    for k in 1..=loopsize {
        if pk.jat[k] == j_nearest {
            let alpha1 = arc_angle(pk.p3a[k], v, pk.n[k], pk.direct[k]);
            if alpha1 < pk.alpha2[k] {
                return true;
            }
        }
    }
    false
}

/// Loops on atom `i`; `sa[1..=ns]` are its segment ids. Returns 1-based loops with dummies.
fn loopconstruct(i: i32, sa: &[i32], ns: usize, segment: &[[i32; 5]]) -> Vec<Vec<i32>> {
    let mut s = vec![[0i32; 2]; ns + 1];
    for k in 1..=ns {
        let seg = &segment[sa[k] as usize];
        if seg[4] * (2 * (seg[0] == i) as i32 - 1) == 1 {
            s[k] = [seg[3], seg[2]];
        } else {
            s[k] = [seg[2], seg[3]];
        }
    }
    let mut used = vec![false; ns + 1];
    let mut number = 0;
    let mut loops: Vec<Vec<i32>> = vec![Vec::new()];
    for k1 in 1..=ns {
        if !used[k1] && number < ns {
            let mut lp = vec![0, sa[k1]];
            let mut pointer = k1;
            number += 1;
            used[k1] = true;
            while s[k1][0] != s[pointer][1] {
                let mut advanced = false;
                for k2 in k1 + 1..=ns {
                    if !used[k2] && s[k2][0] == s[pointer][1] {
                        pointer = k2;
                        lp.push(sa[pointer]);
                        number += 1;
                        used[pointer] = true;
                        advanced = true;
                    }
                }
                if !advanced {
                    break;
                }
            }
            loops.push(lp);
        }
    }
    loops
}

#[allow(clippy::too_many_arguments)]
fn patchesconstruct(
    i: usize,
    c: &[Vec3],
    r: &[f64],
    segment: &[[i32; 5]],
    ncrasegment: &[[f64; 8]],
    pts: &[Vec3],
    circle: &[[f64; 10]],
    circleindex_i: &[i32],
    loops_i: &[Vec<i32>],
    rp: f64,
) -> Vec<Vec<i32>> {
    let nloops_i = loops_i.len() - 1;
    let ncircleindex_i = circleindex_i.len();
    let ci = c[i];
    let ri = r[i] + rp;

    let mut pre = Vec::with_capacity(nloops_i + 1);
    let mut loop_point = vec![Vec3::ZERO; nloops_i + 1];
    pre.push(None);
    for k in 1..=nloops_i {
        let loopk = &loops_i[k];
        let lsz = loopk.len() - 1;
        pre.push(Some(build_loop_pre(
            i as i32,
            ci,
            ri,
            loopk,
            lsz,
            c,
            pts,
            segment,
            ncrasegment,
        )));
        loop_point[k] = pts[segment[loopk[1] as usize][2] as usize];
    }
    let mut circle_point = vec![Vec3::ZERO; ncircleindex_i + 1];
    for t in 1..=ncircleindex_i {
        let crow = &circle[circleindex_i[t - 1] as usize];
        let (vector1, _) = orthogonalvectors(Vec3::new(crow[6], crow[7], crow[8]));
        circle_point[t] = Vec3::new(crow[3], crow[4], crow[5]) + crow[9] * vector1;
    }

    let mut s0: Vec<i32> = (1..=nloops_i as i32).collect();
    s0.extend((1..=ncircleindex_i as i32).map(|t| -t));

    let mut tree: Vec<TreeNode> = Vec::with_capacity(2 * (nloops_i + ncircleindex_i) + 4);
    tree.push(TreeNode::default());
    tree.push(TreeNode {
        activenode: 1,
        activeelement: s0.first().copied().unwrap_or(0),
        set: s0,
        n1: nloops_i,
        n2: ncircleindex_i,
        leftnode: 0,
    });
    let mut ntree = 1;
    let mut j = 1;
    let iters = 2 * (nloops_i + ncircleindex_i) + 1;
    for _ in 1..=iters {
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
        let loopsize_k = loops_i[k as usize].len() - 1;
        let pk = pre[k as usize].as_ref().expect("loop precompute");
        let node = &tree[j];
        for &ss1 in &node.set[..node.n1] {
            let point = loop_point[ss1 as usize];
            if k == ss1 || interiorloop(point, ci, ri, loopsize_k, pk) {
                s1v.push(ss1);
                left_n1 += 1;
                if t1 && ss1 > k {
                    t1 = false;
                    k1 = ss1;
                }
            } else {
                s2v.push(ss1);
                right_n1 += 1;
                if t2 && ss1 > k {
                    t2 = false;
                    k2 = ss1;
                }
            }
        }
        for &ss2 in &node.set[node.n1..node.n1 + node.n2] {
            let point = circle_point[(-ss2) as usize];
            if interiorloop(point, ci, ri, loopsize_k, pk) {
                s1v.push(ss2);
                left_n2 += 1;
                if t1 {
                    t1 = false;
                    k1 = ss2;
                }
            } else {
                s2v.push(ss2);
                right_n2 += 1;
                if t2 {
                    t2 = false;
                    k2 = ss2;
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
    for node in &mut tree[1..=ntree] {
        if node.activenode == 0 && node.leftnode == 0 {
            let mut entry = Vec::with_capacity(node.set.len() + 1);
            entry.push(0);
            entry.extend_from_slice(&node.set);
            patches.push(entry);
        }
    }
    patches
}

pub(crate) fn data_seg_pat(
    geom: &Geom,
    nb: &Neighbors,
    di: &DataI,
    dc: &DataCir,
    rp: f64,
) -> (DataSeg, DataLoop, DataPat) {
    let m = geom.m;
    let c = &geom.centers;
    let r = &geom.r;
    let pts = &di.i;

    let mut ds = DataSeg {
        segment: vec![[0; 5]],
        ncrasegment: vec![[0.0; 8]],
        satom: vec![Vec::new(); m + 1],
        nsegment: 0,
    };
    let mut nsegment = 0i32;
    let mut alpha1: Vec<f64> = Vec::new();
    for i in 1..=m {
        let row_atoms = nb.of(i);
        for (row0, &jj) in row_atoms.iter().enumerate() {
            let j = jj as usize;
            let icirc = &di.i_circle[i][row0 + 1];
            let npt = icirc.len();
            if j > i && npt > 0 {
                let a = circlecenter(c[i], c[j], r[i] + rp, r[j] + rp);
                let rij = cmax(pysq(r[i] + rp) - pysq(norm(c[i] - a)), 0.0).sqrt();
                let nij = (c[j] - c[i]) / norm(c[j] - c[i]);
                let spoint = icirc[0] as usize;
                let index = di.iijk[spoint];
                let dirsp = di.direction[spoint];
                let (ii, ji) = (i as i32, j as i32);
                let direct = if ii == index[0] && ji == index[1] {
                    dirsp[0]
                } else if ii == index[1] && ji == index[2] {
                    dirsp[1]
                } else if ii == index[0] && ji == index[2] {
                    dirsp[2]
                } else {
                    0
                };
                alpha1.clear();
                alpha1.resize(npt + 1, 0.0);
                let isp = pts[spoint];
                for k in 2..=npt {
                    let pk = icirc[k - 1] as usize;
                    alpha1[k] = arc_angle(isp - a, pts[pk] - a, nij, direct);
                }
                let order = stable_order(&alpha1, npt);
                for k in 1..=npt / 2 {
                    nsegment += 1;
                    let o1 = order[2 * k - 1];
                    let o2 = order[2 * k];
                    let p1 = icirc[o1 - 1];
                    let p2 = icirc[o2 - 1];
                    ds.segment.push([ii, ji, p1, p2, direct]);
                    let radian = alpha1[o2] - alpha1[o1];
                    ds.ncrasegment
                        .push([nij.x, nij.y, nij.z, a.x, a.y, a.z, rij, radian]);
                    ds.satom[i].push(nsegment);
                    ds.satom[j].push(nsegment);
                }
            }
        }
    }
    ds.nsegment = nsegment as usize;

    let mut dl = DataLoop {
        loops: vec![Vec::new()],
        loops_index: vec![[0, 0]; m + 1],
    };
    let mut dp = DataPat {
        patches: vec![Vec::new()],
        npatches: 0,
        patches_index: vec![[0, 0]; m + 1],
        patch_atom: vec![0],
    };
    let mut nloops = 0i32;
    let mut npatches = 0i32;
    for i in 1..=m {
        let nsatom_i = ds.satom[i].len();
        let circleindex_i = &dc.circleindex[i];
        let mut loops_i: Vec<Vec<i32>> = Vec::new();
        if nsatom_i > 0 {
            let mut sa_i = Vec::with_capacity(nsatom_i + 1);
            sa_i.push(0);
            sa_i.extend_from_slice(&ds.satom[i]);
            loops_i = loopconstruct(i as i32, &sa_i, nsatom_i, &ds.segment);
        } else if !circleindex_i.is_empty() {
            loops_i.push(Vec::new());
        }
        if nsatom_i > 0 || !circleindex_i.is_empty() {
            let pr = patchesconstruct(
                i,
                c,
                r,
                &ds.segment,
                &ds.ncrasegment,
                pts,
                &dc.circle,
                circleindex_i,
                &loops_i,
                rp,
            );
            if nsatom_i > 0 {
                let n = (loops_i.len() - 1) as i32;
                dl.loops.extend(loops_i.into_iter().skip(1));
                dl.loops_index[i] = [nloops + 1, nloops + n];
                nloops += n;
            }
            let n = (pr.len() - 1) as i32;
            for p in pr.into_iter().skip(1) {
                dp.patches.push(p);
                dp.patch_atom.push(i as i32);
            }
            dp.patches_index[i] = [npatches + 1, npatches + n];
            npatches += n;
        }
    }
    dp.npatches = npatches as usize;
    (ds, dl, dp)
}
