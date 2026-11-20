use super::meshing::{Tag, TagList};
use super::vec3::{cvt_i64, dot, Vec3};

fn find(parent: &mut [usize], mut x: usize) -> usize {
    let mut root = x;
    while parent[root] != root {
        root = parent[root];
    }
    while parent[x] != root {
        let next = parent[x];
        parent[x] = root;
        x = next;
    }
    root
}

/// Welds tagged boundary vertices that share a tag and coincide within `eps`.
/// Returns the welded vertices, the remapped non-degenerate faces and the per-vertex atom ids.
pub(crate) fn fuse_by_id(
    v: &[Vec3],
    f: &[[i32; 3]],
    tags: &[TagList],
    atom_id: &[i32],
    eps: f64,
) -> (Vec<Vec3>, Vec<[i32; 3]>, Vec<i32>) {
    let n = v.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let eps2 = eps * eps;
    let inv_eps = 1.0 / eps;

    let mut ent: Vec<(Tag, [i64; 3], usize)> = Vec::new();
    for (i, tl) in tags.iter().enumerate().take(n) {
        if tl.is_empty() {
            continue;
        }
        let cell = [
            cvt_i64((v[i].x * inv_eps).floor()),
            cvt_i64((v[i].y * inv_eps).floor()),
            cvt_i64((v[i].z * inv_eps).floor()),
        ];
        ent.extend(tl.iter().map(|&tag| (tag, cell, i)));
    }
    ent.sort_unstable();

    for p in 0..ent.len() {
        let (ptag, pc, pidx) = ent[p];
        for &(qtag, qc, qidx) in &ent[p + 1..] {
            if qtag != ptag || qc[0] > pc[0].wrapping_add(1) {
                break;
            }
            if qc[1] < pc[1].wrapping_sub(1)
                || qc[1] > pc[1].wrapping_add(1)
                || qc[2] < pc[2].wrapping_sub(1)
                || qc[2] > pc[2].wrapping_add(1)
            {
                continue;
            }
            if qidx == pidx {
                continue;
            }
            let i = pidx.max(qidx);
            let j = pidx.min(qidx);
            let dv = v[i] - v[j];
            if dot(dv, dv) < eps2 {
                let ra = find(&mut parent, i);
                let rb = find(&mut parent, j);
                if ra != rb {
                    parent[ra.max(rb)] = ra.min(rb);
                }
            }
        }
    }

    let mut root_new = vec![usize::MAX; n];
    let mut o2n = vec![0usize; n];
    let mut v2 = Vec::new();
    let mut atom_id2 = Vec::new();
    for (i, new_index) in o2n.iter_mut().enumerate() {
        let r = find(&mut parent, i);
        if root_new[r] == usize::MAX {
            root_new[r] = v2.len();
            v2.push(v[r]);
            atom_id2.push(atom_id[r]);
        }
        *new_index = root_new[r];
    }
    let f2 = f
        .iter()
        .filter_map(|t| {
            let na = o2n[t[0] as usize];
            let nb = o2n[t[1] as usize];
            let nc = o2n[t[2] as usize];
            (na != nb && nb != nc && nc != na).then_some([na as i32, nb as i32, nc as i32])
        })
        .collect();
    (v2, f2, atom_id2)
}

/// Drops doubled "flap" face fans hanging off non-manifold edges (vertices untouched).
pub(crate) fn remove_nonmanifold_flaps(f: &[[i32; 3]], passes: usize) -> Vec<[i32; 3]> {
    fn ekey(u: i32, v: i32) -> (i32, i32) {
        if u < v {
            (u, v)
        } else {
            (v, u)
        }
    }
    fn slots_from(t: &[i32; 3], u: i32, mut fun: impl FnMut(i32)) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let e = ekey(a, b);
            if e.0 == u {
                fun(e.1);
            }
        }
    }
    let mut cur = f.to_vec();
    if cur.is_empty() {
        return cur;
    }
    for _ in 0..passes {
        let nv = cur
            .iter()
            .flat_map(|t| t.iter())
            .map(|&x| x as usize + 1)
            .max()
            .unwrap_or(0);
        let mut off = vec![0usize; nv + 1];
        for t in &cur {
            for &x in t {
                off[x as usize + 1] += 1;
            }
        }
        for x in 0..nv {
            off[x + 1] += off[x];
        }
        let mut face = vec![0usize; cur.len() * 3];
        let mut fill = off[..nv].to_vec();
        for (fi, t) in cur.iter().enumerate() {
            for &x in t {
                face[fill[x as usize]] = fi;
                fill[x as usize] += 1;
            }
        }
        let (off, face) = (&off, &face);
        let faces_at = |x: usize| {
            let (b, e) = (off[x], off[x + 1]);
            (b..e)
                .filter(move |&k| !(k > b && face[k - 1] == face[k]))
                .map(move |k| face[k])
        };

        let mut nm_edges: Vec<(i32, i32)> = Vec::new();
        let mut nbr: Vec<i32> = Vec::new();
        for u in 0..nv {
            nbr.clear();
            for fi in faces_at(u) {
                slots_from(&cur[fi], u as i32, |v| nbr.push(v));
            }
            nbr.sort_unstable();
            let mut i = 0;
            while i < nbr.len() {
                let mut j = i;
                while j < nbr.len() && nbr[j] == nbr[i] {
                    j += 1;
                }
                if j - i >= 3 {
                    nm_edges.push((u as i32, nbr[i]));
                }
                i = j;
            }
        }
        if nm_edges.is_empty() {
            break;
        }

        let mut drop = vec![false; cur.len()];
        let mut any_drop = false;
        for &(eu, ev) in &nm_edges {
            for fi in faces_at(eu as usize) {
                let t = &cur[fi];
                let mut on_edge = false;
                slots_from(t, eu, |v| on_edge = on_edge || v == ev);
                if !on_edge {
                    continue;
                }
                let Some(&third) = t.iter().find(|&&x| x != eu && x != ev) else {
                    continue;
                };
                let (tb, te) = (off[third as usize], off[third as usize + 1]);
                let all_on_edge = face[tb..te]
                    .iter()
                    .all(|&g| cur[g].contains(&eu) && cur[g].contains(&ev));
                if all_on_edge {
                    for &g in &face[tb..te] {
                        drop[g] = true;
                    }
                    any_drop = any_drop || te > tb;
                }
            }
        }
        if !any_drop {
            break;
        }
        cur = cur
            .iter()
            .zip(&drop)
            .filter(|(_, &d)| !d)
            .map(|(t, _)| *t)
            .collect();
    }
    cur
}
