use super::vec3::{circlecenter, cross, cvt_i64, dot, norm, pysq, Vec3};
use super::Geom;
use std::collections::HashMap;

/// Per-atom intersecting SAS balls in CSR form; atom 0 is a dummy with an empty row.
#[derive(Debug, Clone, Default)]
pub(crate) struct Neighbors {
    pub off: Vec<i32>,
    pub val: Vec<i32>,
}

impl Neighbors {
    #[inline]
    pub fn of(&self, i: usize) -> &[i32] {
        &self.val[self.off[i] as usize..self.off[i + 1] as usize]
    }
    #[inline]
    pub fn count(&self, i: usize) -> usize {
        (self.off[i + 1] - self.off[i]) as usize
    }
}

pub(crate) fn interstructure(geom: &Geom, rp: f64) -> Neighbors {
    let m = geom.m;
    let c = &geom.centers;
    let r = &geom.r;
    let mut nb = Neighbors {
        off: vec![0; m + 2],
        val: Vec::new(),
    };
    if m == 0 {
        return nb;
    }
    let mut rmax_r = r[1];
    for &ra in &r[2..=m] {
        if ra > rmax_r {
            rmax_r = ra;
        }
    }
    let rmax = rmax_r + rp;
    let mut cell = vec![[0i64; 3]; m + 1];
    let mut cells: HashMap<[i64; 3], Vec<usize>> = HashMap::with_capacity(m * 2);
    for a in 1..=m {
        let key = [
            cvt_i64((c[a].x / rmax).floor()),
            cvt_i64((c[a].y / rmax).floor()),
            cvt_i64((c[a].z / rmax).floor()),
        ];
        cell[a] = key;
        cells.entry(key).or_default().push(a);
    }
    let mut rows: Vec<Vec<i32>> = vec![Vec::new(); m + 1];
    for a in 1..=m {
        let ca = c[a];
        let ra = r[a];
        let [ix, iy, iz] = cell[a];
        let found = &mut rows[a];
        for di in -2..=2 {
            for dj in -2..=2 {
                for dk in -2..=2 {
                    let key = [
                        ix.wrapping_add(di),
                        iy.wrapping_add(dj),
                        iz.wrapping_add(dk),
                    ];
                    let Some(list) = cells.get(&key) else {
                        continue;
                    };
                    for &b in list {
                        if b == a {
                            continue;
                        }
                        let dx = c[b].x - ca.x;
                        let dy = c[b].y - ca.y;
                        let dz = c[b].z - ca.z;
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                        if ra + r[b] + 2.0 * rp - dist > 0.0 {
                            found.push(b as i32);
                        }
                    }
                }
            }
        }
        found.sort_unstable();
    }
    let mut total = 0i32;
    for (a, row) in rows.iter().enumerate() {
        nb.off[a] = total;
        total += row.len() as i32;
    }
    nb.off[m + 1] = total;
    nb.val.reserve(total as usize);
    for row in &rows[1..] {
        nb.val.extend_from_slice(row);
    }
    nb
}

/// Triple intersection points and per-atom membership (all 1-based, `[0]` dummy).
#[derive(Debug, Clone, Default)]
pub(crate) struct DataI {
    pub i: Vec<Vec3>,
    pub n_i: usize,
    pub iijk: Vec<[i32; 3]>,
    pub direction: Vec<[i32; 3]>,
    pub high_i: Vec<i32>,
    pub ii: Vec<Vec<i32>>,
    /// `i_circle[i][row]`: point ids on the intersection circle of atom `i` with its `row`-th neighbour.
    pub i_circle: Vec<Vec<Vec<i32>>>,
}

/// Free (uncovered) intersection circles: `{_, ci, cj, A(3), n(3), r}`.
#[derive(Debug, Clone, Default)]
pub(crate) struct DataCir {
    pub circle: Vec<[f64; 10]>,
    pub ncircle: usize,
    pub circleindex: Vec<Vec<i32>>,
}

struct TripleEvent {
    j: usize,
    k: usize,
    row1: usize,
    row2: usize,
    row3: usize,
    y: Vec3,
    dir: [i32; 3],
    true_hight: i32,
}

pub(crate) fn data_i_cir(geom: &Geom, nb: &Neighbors, rp: f64) -> (DataI, DataCir) {
    let m = geom.m;
    let c = &geom.centers;
    let r = &geom.r;
    let mint = |i: usize, row: usize| nb.of(i)[row - 1] as usize;

    let mut rext = vec![0.0; m + 1];
    let mut rext2 = vec![0.0; m + 1];
    for a in 1..=m {
        rext[a] = r[a] + rp;
        rext2[a] = pysq(r[a] + rp);
    }

    let mut di = DataI {
        i: vec![Vec3::ZERO],
        iijk: vec![[0; 3]],
        direction: vec![[0; 3]],
        high_i: vec![1],
        ii: vec![Vec::new(); m + 1],
        i_circle: (0..=m)
            .map(|i| {
                if i == 0 {
                    Vec::new()
                } else {
                    vec![Vec::new(); nb.count(i) + 1]
                }
            })
            .collect(),
        ..Default::default()
    };
    let mut circle_rows: Vec<[f64; 10]> = Vec::new();
    let mut s: i32 = 0;
    let mut triples: Vec<TripleEvent> = Vec::new();

    for i in 1..=m {
        let ri = nb.count(i);
        let mut q_scratch = vec![0.0; ri + 1];
        let mut p_scratch = vec![0.0; ri + 1];
        triples.clear();
        let mut circles: Vec<[f64; 10]> = Vec::new();
        for row1 in 1..=ri {
            let j = mint(i, row1);
            if j <= i {
                continue;
            }
            let mut circletest = 1;
            let a_pt = circlecenter(c[i], c[j], rext[i], rext[j]);
            let disc = rext2[i] - pysq(norm(c[i] - a_pt));
            if disc <= 0.0 {
                continue;
            }
            let normal = c[j] - c[i];
            let dist_normal = norm(normal);
            let nij = normal / dist_normal;
            let rij = disc.sqrt();

            for row in 1..=ri {
                let k = mint(i, row);
                if k != j {
                    let a = a_pt - c[k];
                    let dist_a = norm(a);
                    let ctheta = dot(a, normal) / (dist_a * dist_normal);
                    let q = (1.0 - pysq(ctheta)).sqrt() * dist_a;
                    let pp = pysq(ctheta * dist_a);
                    q_scratch[row] = q;
                    p_scratch[row] = pp;
                    if pysq(q + rij) + pp - rext2[k] < 0.0 {
                        circletest = 0;
                        break;
                    }
                }
            }

            if circletest == 1 {
                for row2 in 1..=ri {
                    let k = mint(i, row2);
                    if k < j
                        && circletest == 1
                        && pysq(-q_scratch[row2] + rij) + p_scratch[row2] - rext2[k] < 0.0
                    {
                        circletest = 0;
                    }
                    let mut row3 = 0;
                    if k > j {
                        let jrow = nb.of(j);
                        if let Ok(pos) = jrow.binary_search(&(k as i32)) {
                            row3 = pos + 1;
                        }
                    }
                    if row3 != 0 {
                        let b = circlecenter(c[k], c[j], rext[k], rext[j]);
                        let cij = c[i] - c[j];
                        let ckj = c[k] - c[j];
                        let cr = cross(cij, ckj);
                        let n = cr / norm(cr);
                        let u = ckj - dot(ckj, cij) * cij / dot(cij, cij);
                        let t = dot(b - a_pt, ckj) / dot(u, ckj);
                        let x1 = a_pt + t * u;
                        let cc = -(pysq(norm(x1 - c[i]))) + rext2[i];
                        if cc > 0.0 {
                            circletest = 0;
                            let s1 = -cc.sqrt();
                            let s2 = cc.sqrt();
                            let y1 = x1 + s1 * n;
                            let y2 = x1 + s2 * n;
                            let true_hight = if s2 < rp { 0 } else { 1 };
                            let blocked = |y: Vec3| {
                                (1..=ri).any(|row| {
                                    let ll = mint(i, row);
                                    ll != j && ll != k && norm(c[ll] - y) < rext[ll]
                                })
                            };
                            let true1 = !blocked(y1);
                            let true2 = !blocked(y2);
                            let ev = |y, dir| TripleEvent {
                                j,
                                k,
                                row1,
                                row2,
                                row3,
                                y,
                                dir,
                                true_hight,
                            };
                            if true1 {
                                triples.push(ev(y1, [1, 1, -1]));
                            }
                            if true2 {
                                triples.push(ev(y2, [-1, -1, 1]));
                            }
                        }
                    }
                }
            }
            if circletest == 1 {
                circles.push([
                    0.0, i as f64, j as f64, a_pt.x, a_pt.y, a_pt.z, nij.x, nij.y, nij.z, rij,
                ]);
            }
        }
        for e in &triples {
            s += 1;
            di.i.push(e.y);
            di.iijk.push([i as i32, e.j as i32, e.k as i32]);
            di.direction.push(e.dir);
            di.ii[i].push(s);
            di.ii[e.j].push(s);
            di.ii[e.k].push(s);
            di.i_circle[i][e.row1].push(s);
            di.i_circle[e.j][e.row3].push(s);
            di.i_circle[i][e.row2].push(s);
            di.high_i.push(if e.true_hight == 0 { 0 } else { 1 });
        }
        circle_rows.extend(circles);
    }
    di.n_i = s as usize;

    let ncircle = circle_rows.len();
    let mut dc = DataCir {
        circle: Vec::with_capacity(ncircle + 1),
        ncircle,
        circleindex: vec![Vec::new(); m + 1],
    };
    dc.circle.push([0.0; 10]);
    dc.circle.extend(circle_rows);
    for k in 1..=ncircle {
        let ci = dc.circle[k][1] as usize;
        let cj = dc.circle[k][2] as usize;
        dc.circleindex[ci].push(k as i32);
        dc.circleindex[cj].push(k as i32);
    }
    (di, dc)
}
