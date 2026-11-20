//! Monomer TM-align path of US-align (`TMalign_main` with the default CLI
//! options), translated from the US-align sources by Chengxin Zhang et al.
//!
//! ```text
//! US-align: universal structure alignment of monomeric and complex proteins
//! and nucleic acids
//!
//! References to cite:
//! (1) Chengxin Zhang, Lydia Freddolino, Yang Zhang (2026)
//!     Nat Protoc. 21, 517-541.
//! (2) Chengxin Zhang, Morgan Shine, Anna Marie Pyle, Yang Zhang
//!     (2022) Nat Methods. 19(9), 1109-1115.
//! (3) Chengxin Zhang, Anna Marie Pyle (2022) iScience. 25(10), 105218.
//!
//! DISCLAIMER:
//!   Permission to use, copy, modify, and distribute this program for
//!   any purpose, with or without fee, is hereby granted, provided that
//!   the notices on the head, the reference information, and this
//!   copyright notice appear in all copies or substantial portions of
//!   the Software. It is provided "as is" without express or implied
//!   warranty.
//! ```
//!
//! Floating-point operations keep the original evaluation order so that the
//! results are bitwise identical to the C++ implementation.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

type Coord = [f64; 3];
type Mat3 = [[f64; 3]; 3];

/// One structure with one pivot atom per residue.
pub(crate) struct Chain {
    xyz: Vec<Coord>,
    /// One-letter residue codes; nucleotides are lower case.
    seq: Vec<u8>,
    /// Number of nucleotides minus number of amino acids.
    mol_score: i32,
}

impl Chain {
    /// Builds a chain from packed coordinates and 4-byte residue records
    /// (3-byte right-justified residue name plus a nucleic-acid flag).
    pub(crate) fn from_records(xyz: &[f64], names: &[u8]) -> Self {
        let len = xyz.len() / 3;
        let mut seq = Vec::with_capacity(len);
        let mut mol_score = 0;
        for record in names.chunks_exact(4).take(len) {
            let mut code = residue_code(&record[..3]);
            let nucleic = record[3] != 0;
            if nucleic && code.is_ascii_uppercase() {
                code += b'a' - b'A';
            }
            seq.push(code);
            mol_score += if nucleic { 1 } else { -1 };
        }
        Self {
            xyz: xyz.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(),
            seq,
            mol_score,
        }
    }

    fn len(&self) -> usize {
        self.seq.len()
    }
}

/// Result of [`align`]; the transform maps the moving chain onto the reference.
pub(crate) struct Alignment {
    pub t: [f64; 3],
    pub u: Mat3,
    /// TM-score normalized by the reference length.
    pub tm_reference: f64,
    /// TM-score normalized by the moving length.
    pub tm_moving: f64,
    pub rmsd: f64,
    pub aligned: i32,
    pub sequence_identity: f64,
    pub nucleic: bool,
}

/// One-letter code of a 3-column residue name (`AAmap`).
fn residue_code(name: &[u8]) -> u8 {
    const TABLE: [(&[u8; 3], u8); 43] = [
        (b"ALA", b'A'),
        (b"DAL", b'A'),
        (b"ASX", b'B'),
        (b"CYS", b'C'),
        (b"DCY", b'C'),
        (b"ASP", b'D'),
        (b"DAS", b'D'),
        (b"GLU", b'E'),
        (b"DGL", b'E'),
        (b"PHE", b'F'),
        (b"DPN", b'F'),
        (b"GLY", b'G'),
        (b"HIS", b'H'),
        (b"DHI", b'H'),
        (b"ILE", b'I'),
        (b"DIL", b'I'),
        (b"LYS", b'K'),
        (b"DLY", b'K'),
        (b"LEU", b'L'),
        (b"DLE", b'L'),
        (b"MET", b'M'),
        (b"MED", b'M'),
        (b"MSE", b'M'),
        (b"ASN", b'N'),
        (b"DSG", b'N'),
        (b"PYL", b'O'),
        (b"PRO", b'P'),
        (b"DPR", b'P'),
        (b"GLN", b'Q'),
        (b"DGN", b'Q'),
        (b"ARG", b'R'),
        (b"DAR", b'R'),
        (b"SER", b'S'),
        (b"DSN", b'S'),
        (b"THR", b'T'),
        (b"DTH", b'T'),
        (b"SEC", b'U'),
        (b"VAL", b'V'),
        (b"DVA", b'V'),
        (b"TRP", b'W'),
        (b"DTR", b'W'),
        (b"TYR", b'Y'),
        (b"DTY", b'Y'),
    ];
    if let Some((_, code)) = TABLE.iter().find(|(n, _)| n.as_slice() == name) {
        return *code;
    }
    if name == b"GLX" {
        return b'Z';
    }
    if name.starts_with(b" D") || name.starts_with(b"  ") {
        return name[2].to_ascii_lowercase();
    }
    b'X'
}

/// Pads an atom name to the 4-column PDB form (US-align's "autojustify").
pub(crate) fn pad_atom_name(name: &str) -> String {
    match name.len() {
        1 => format!(" {name}  "),
        2 => format!(" {name} "),
        3 => format!(" {name}"),
        _ => name.to_string(),
    }
}

// libm calls are routed through black_box so that LLVM cannot replace them
// with differently rounded equivalents (e.g. pow(x, 0.5) -> sqrt(x)).
fn pow(x: f64, y: f64) -> f64 {
    x.powf(std::hint::black_box(y))
}

fn dist(x: &Coord, y: &Coord) -> f64 {
    let d1 = x[0] - y[0];
    let d2 = x[1] - y[1];
    let d3 = x[2] - y[2];
    d1 * d1 + d2 * d2 + d3 * d3
}

fn dot(a: &[f64; 3], b: &Coord) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn transform(t: &[f64; 3], u: &Mat3, x: &Coord) -> Coord {
    [
        t[0] + dot(&u[0], x),
        t[1] + dot(&u[1], x),
        t[2] + dot(&u[2], x),
    ]
}

fn do_rotation(x: &[Coord], x1: &mut [Coord], len: usize, t: &[f64; 3], u: &Mat3) {
    for i in 0..len {
        x1[i] = transform(t, u, &x[i]);
    }
}

/// Kabsch superposition; `mode` 0 computes rms only, 1 computes u and t.
fn kabsch(
    x: &[Coord],
    y: &[Coord],
    n: usize,
    mode: i32,
    rms: &mut f64,
    t: &mut [f64; 3],
    u: &mut Mat3,
) -> bool {
    let mut j: usize;
    let mut k: usize;
    let mut l: usize;
    let mut m: usize;
    let mut d: f64;
    let mut p: f64;
    let mut det: f64;
    let mut xc = [0.0; 3];
    let mut yc = [0.0; 3];
    let mut a = [[0.0; 3]; 3];
    let mut b = [[0.0; 3]; 3];
    let mut r = [[0.0; 3]; 3];
    let mut e = [0.0; 3];
    let mut rr = [0.0; 6];
    let mut ss = [0.0; 6];
    let sqrt3 = 1.73205080756888;
    let tol = 0.01;
    let ip = [0usize, 1, 3, 1, 2, 4, 3, 4, 5];
    let ip2312 = [1usize, 2, 0, 1];
    let mut a_failed = false;
    let mut b_failed = false;
    let epsilon = 0.00000001;

    *rms = 0.0;
    let mut rms1 = 0.0;
    let mut e0 = 0.0;
    let mut s1 = [0.0; 3];
    let mut s2 = [0.0; 3];
    let mut sx = [0.0; 3];
    let mut sy = [0.0; 3];
    let mut sz = [0.0; 3];
    for i in 0..3 {
        t[i] = 0.0;
        for jj in 0..3 {
            u[i][jj] = if i == jj { 1.0 } else { 0.0 };
            a[i][jj] = if i == jj { 1.0 } else { 0.0 };
        }
    }
    if n < 1 {
        return false;
    }
    let nf = n as f64;

    for i in 0..n {
        let (c1, c2) = (x[i], y[i]);
        for jj in 0..3 {
            s1[jj] += c1[jj];
            s2[jj] += c2[jj];
        }
        for jj in 0..3 {
            sx[jj] += c1[0] * c2[jj];
            sy[jj] += c1[1] * c2[jj];
            sz[jj] += c1[2] * c2[jj];
        }
    }
    for i in 0..3 {
        xc[i] = s1[i] / nf;
        yc[i] = s2[i] / nf;
    }
    if mode == 2 || mode == 0 {
        for mm in 0..n {
            for nn in 0..3 {
                e0 += (x[mm][nn] - xc[nn]) * (x[mm][nn] - xc[nn])
                    + (y[mm][nn] - yc[nn]) * (y[mm][nn] - yc[nn]);
            }
        }
    }
    for jj in 0..3 {
        r[jj][0] = sx[jj] - s1[0] * s2[jj] / nf;
        r[jj][1] = sy[jj] - s1[1] * s2[jj] / nf;
        r[jj][2] = sz[jj] - s1[2] * s2[jj] / nf;
    }

    det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
    let sigma = det;

    m = 0;
    for jj in 0..3 {
        for i in 0..=jj {
            rr[m] = r[0][i] * r[0][jj] + r[1][i] * r[1][jj] + r[2][i] * r[2][jj];
            m += 1;
        }
    }

    let spur = (rr[0] + rr[2] + rr[5]) / 3.0;
    let cof = (((((rr[2] * rr[5] - rr[4] * rr[4]) + rr[0] * rr[5]) - rr[3] * rr[3])
        + rr[0] * rr[2])
        - rr[1] * rr[1])
        / 3.0;
    det = det * det;

    for i in 0..3 {
        e[i] = spur;
    }

    j = 0;
    if spur > 0.0 {
        d = spur * spur;
        let h = d - cof;
        let g = (spur * cof - det) / 2.0 - spur * h;

        if h > 0.0 {
            let sqrth = h.sqrt();
            d = h * h * h - g * g;
            if d < 0.0 {
                d = 0.0;
            }
            d = d.sqrt().atan2(-g) / 3.0;
            let cth = sqrth * d.cos();
            let sth = sqrth * sqrt3 * d.sin();
            e[0] = (spur + cth) + cth;
            e[1] = (spur - cth) + sth;
            e[2] = (spur - cth) - sth;

            if mode != 0 {
                l = 0;
                while l < 3 {
                    d = e[l];
                    ss[0] = (d - rr[2]) * (d - rr[5]) - rr[4] * rr[4];
                    ss[1] = (d - rr[5]) * rr[1] + rr[3] * rr[4];
                    ss[2] = (d - rr[0]) * (d - rr[5]) - rr[3] * rr[3];
                    ss[3] = (d - rr[2]) * rr[3] + rr[1] * rr[4];
                    ss[4] = (d - rr[0]) * rr[4] + rr[1] * rr[3];
                    ss[5] = (d - rr[0]) * (d - rr[2]) - rr[1] * rr[1];

                    for s in ss.iter_mut() {
                        if s.abs() <= epsilon {
                            *s = 0.0;
                        }
                    }

                    if ss[0].abs() >= ss[2].abs() {
                        j = 0;
                        if ss[0].abs() < ss[5].abs() {
                            j = 2;
                        }
                    } else if ss[2].abs() >= ss[5].abs() {
                        j = 1;
                    } else {
                        j = 2;
                    }

                    d = 0.0;
                    j *= 3;
                    for i in 0..3 {
                        k = ip[i + j];
                        a[i][l] = ss[k];
                        d += ss[k] * ss[k];
                    }

                    if d > epsilon {
                        d = 1.0 / d.sqrt();
                    } else {
                        d = 0.0;
                    }
                    for i in 0..3 {
                        a[i][l] *= d;
                    }
                    l += 2;
                }

                d = a[0][0] * a[0][2] + a[1][0] * a[1][2] + a[2][0] * a[2][2];
                let (m1, m) = if (e[0] - e[1]) > (e[1] - e[2]) {
                    (2, 0)
                } else {
                    (0, 2)
                };
                p = 0.0;
                for i in 0..3 {
                    a[i][m1] -= d * a[i][m];
                    p += a[i][m1] * a[i][m1];
                }
                if p <= tol {
                    p = 1.0;
                    for i in 0..3 {
                        if p < a[i][m].abs() {
                            continue;
                        }
                        p = a[i][m].abs();
                        j = i;
                    }
                    k = ip2312[j];
                    l = ip2312[j + 1];
                    p = (a[k][m] * a[k][m] + a[l][m] * a[l][m]).sqrt();
                    if p > tol {
                        a[j][m1] = 0.0;
                        a[k][m1] = -a[l][m] / p;
                        a[l][m1] = a[k][m] / p;
                    } else {
                        a_failed = true;
                    }
                } else {
                    p = 1.0 / p.sqrt();
                    for i in 0..3 {
                        a[i][m1] *= p;
                    }
                }
                if !a_failed {
                    a[0][1] = a[1][2] * a[2][0] - a[1][0] * a[2][2];
                    a[1][1] = a[2][2] * a[0][0] - a[2][0] * a[0][2];
                    a[2][1] = a[0][2] * a[1][0] - a[0][0] * a[1][2];
                }
            }
        }

        if mode != 0 && !a_failed {
            for l in 0..2 {
                d = 0.0;
                for i in 0..3 {
                    b[i][l] = r[i][0] * a[0][l] + r[i][1] * a[1][l] + r[i][2] * a[2][l];
                    d += b[i][l] * b[i][l];
                }
                if d > epsilon {
                    d = 1.0 / d.sqrt();
                } else {
                    d = 0.0;
                }
                for i in 0..3 {
                    b[i][l] *= d;
                }
            }
            d = b[0][0] * b[0][1] + b[1][0] * b[1][1] + b[2][0] * b[2][1];
            p = 0.0;

            for i in 0..3 {
                b[i][1] -= d * b[i][0];
                p += b[i][1] * b[i][1];
            }

            if p <= tol {
                p = 1.0;
                for i in 0..3 {
                    if p < b[i][0].abs() {
                        continue;
                    }
                    p = b[i][0].abs();
                    j = i;
                }
                k = ip2312[j];
                l = ip2312[j + 1];
                p = (b[k][0] * b[k][0] + b[l][0] * b[l][0]).sqrt();
                if p > tol {
                    b[j][1] = 0.0;
                    b[k][1] = -b[l][0] / p;
                    b[l][1] = b[k][0] / p;
                } else {
                    b_failed = true;
                }
            } else {
                p = 1.0 / p.sqrt();
                for i in 0..3 {
                    b[i][1] *= p;
                }
            }
            if !b_failed {
                b[0][2] = b[1][0] * b[2][1] - b[1][1] * b[2][0];
                b[1][2] = b[2][0] * b[0][1] - b[2][1] * b[0][0];
                b[2][2] = b[0][0] * b[1][1] - b[0][1] * b[1][0];
                for i in 0..3 {
                    for jj in 0..3 {
                        u[i][jj] = b[i][0] * a[jj][0] + b[i][1] * a[jj][1] + b[i][2] * a[jj][2];
                    }
                }
            }

            for i in 0..3 {
                t[i] = ((yc[i] - u[i][0] * xc[0]) - u[i][1] * xc[1]) - u[i][2] * xc[2];
            }
        }
    } else {
        for i in 0..3 {
            t[i] = ((yc[i] - u[i][0] * xc[0]) - u[i][1] * xc[1]) - u[i][2] * xc[2];
        }
    }

    for i in 0..3 {
        if e[i] < 0.0 {
            e[i] = 0.0;
        }
        e[i] = e[i].sqrt();
    }
    d = e[2];
    if sigma < 0.0 {
        d = -d;
    }
    d = (d + e[1]) + e[0];

    if mode == 2 || mode == 0 {
        rms1 = (e0 - d) - d;
        if rms1 < 0.0 {
            rms1 = 0.0;
        }
    }

    *rms = rms1;
    true
}

/// d0 parameters used during the alignment search.
struct SearchParams {
    d0_min: f64,
    lnorm: f64,
    score_d8: f64,
    d0: f64,
    d0_search: f64,
    dcu0: f64,
}

fn parameter_set4search(xlen: usize, ylen: usize) -> SearchParams {
    let dcu0 = 4.25;
    let lnorm = xlen.min(ylen) as f64;
    let mut d0 = if lnorm <= 19.0 {
        0.168
    } else {
        1.24 * pow(lnorm * 1.0 - 15.0, 1.0 / 3.0) - 1.8
    };
    let d0_min = d0 + 0.8;
    d0 = d0_min;
    let d0_search = d0.clamp(4.5, 8.0);
    let score_d8 = 1.5 * pow(lnorm * 1.0, 0.3) + 3.5;
    SearchParams {
        d0_min,
        lnorm,
        score_d8,
        d0,
        d0_search,
        dcu0,
    }
}

/// Returns (Lnorm, d0, d0_search) for the final TM-score.
fn parameter_set4final(len: f64, mol_type: i32) -> (f64, f64, f64) {
    let lnorm = len;
    let d0 = if mol_type > 0 {
        if lnorm <= 11.0 {
            0.3
        } else if lnorm > 11.0 && lnorm <= 15.0 {
            0.4
        } else if lnorm > 15.0 && lnorm <= 19.0 {
            0.5
        } else if lnorm > 19.0 && lnorm <= 23.0 {
            0.6
        } else if lnorm > 23.0 && lnorm < 30.0 {
            0.7
        } else {
            0.6 * pow(lnorm * 1.0 - 0.5, 1.0 / 2.0) - 2.5
        }
    } else {
        let d0_min = 0.5;
        let mut d0 = if lnorm <= 21.0 {
            0.5
        } else {
            1.24 * pow(lnorm * 1.0 - 15.0, 1.0 / 3.0) - 1.8
        };
        if d0 < d0_min {
            d0 = d0_min;
        }
        d0
    };
    (lnorm, d0, d0.clamp(4.5, 8.0))
}

/// Cutoff that admits three pairs when `d * d` admits fewer.
///
/// US-align retries with `(d + 0.5 inc)^2` for inc = 1, 2, ... until three
/// pairs pass, which never ends for NaN or overflowing distances. The first
/// passing `inc` is found by bisection instead (the cutoff grows with `inc`
/// for `d >= 0`); `None` means no `inc` in the original `int` range passes.
fn relaxed_cutoff(xa: &[Coord], ya: &[Coord], n_ali: usize, d: f64) -> Option<f64> {
    let mut dis: Vec<f64> = (0..n_ali)
        .map(|i| dist(&xa[i], &ya[i]))
        .filter(|v| !v.is_nan())
        .collect();
    if dis.len() < 3 {
        return None;
    }
    let third = *dis.select_nth_unstable_by(2, f64::total_cmp).1;
    let cutoff = |inc: i32| {
        let dinc = d + inc as f64 * 0.5;
        dinc * dinc
    };
    let passes = |inc: i32| third < cutoff(inc);
    let (mut fail, mut pass) = (0, i32::MAX);
    if passes(fail) || !passes(pass) {
        return None;
    }
    while pass - fail > 1 {
        let mid = fail + (pass - fail) / 2;
        if passes(mid) {
            pass = mid;
        } else {
            fail = mid;
        }
    }
    Some(cutoff(pass))
}

/// Collects pairs closer than `d` into `i_ali` and returns their count;
/// `standard` normalizes the score by `n_ali` instead of `lnorm`.
fn score_fun8(
    xa: &[Coord],
    ya: &[Coord],
    n_ali: usize,
    d: f64,
    i_ali: &mut [usize],
    score1: &mut f64,
    score_sum_method: i32,
    lnorm: f64,
    score_d8: f64,
    d0: f64,
    standard: bool,
) -> usize {
    let mut score_sum = 0.0;
    let d_tmp = d * d;
    let d02 = d0 * d0;
    let score_d8_cut = score_d8 * score_d8;
    let mut n_cut = 0;
    for i in 0..n_ali {
        let di = dist(&xa[i], &ya[i]);
        if di < d_tmp {
            i_ali[n_cut] = i;
            n_cut += 1;
        }
        if score_sum_method == 8 {
            if di <= score_d8_cut {
                score_sum += 1.0 / (1.0 + di / d02);
            }
        } else {
            score_sum += 1.0 / (1.0 + di / d02);
        }
    }
    if n_cut < 3 && n_ali > 3 {
        if let Some(d_tmp) = relaxed_cutoff(xa, ya, n_ali, d) {
            n_cut = 0;
            for i in 0..n_ali {
                if dist(&xa[i], &ya[i]) < d_tmp {
                    i_ali[n_cut] = i;
                    n_cut += 1;
                }
            }
        }
    }
    *score1 = if standard {
        score_sum / n_ali as f64
    } else {
        score_sum / lnorm
    };
    n_cut
}

/// TM-score search by iterative superposition of fragments; the best
/// transform is written to `t0`/`u0`. `standard` selects
/// `TMscore8_search_standard`.
fn tmscore8_search(
    r1: &mut [Coord],
    r2: &mut [Coord],
    xtm: &[Coord],
    ytm: &[Coord],
    xt: &mut [Coord],
    lali: usize,
    t0: &mut [f64; 3],
    u0: &mut Mat3,
    simplify_step: usize,
    score_sum_method: i32,
    local_d0_search: f64,
    lnorm: f64,
    score_d8: f64,
    d0: f64,
    standard: bool,
) -> f64 {
    let mut score = 0.0;
    let mut rmsd = 0.0;
    let mut k_ali = vec![0usize; lali];
    let mut t = [0.0; 3];
    let mut u = [[0.0; 3]; 3];

    let n_it = 20;
    let n_init_max = 6;
    let mut l_ini = [0usize; 6];
    let mut l_ini_min = 4;
    if lali < l_ini_min {
        l_ini_min = lali;
    }

    let mut n_init = 0;
    let mut i = 0;
    while i < n_init_max - 1 {
        n_init += 1;
        l_ini[i] = (lali as f64 / 2f64.powi(i as i32)) as usize;
        if l_ini[i] <= l_ini_min {
            l_ini[i] = l_ini_min;
            break;
        }
        i += 1;
    }
    if i == n_init_max - 1 {
        n_init += 1;
        l_ini[i] = l_ini_min;
    }

    let mut score_max = -1.0;
    let mut i_ali = vec![0usize; lali];

    for &l_frag in &l_ini[..n_init] {
        let il_max = lali - l_frag;
        let mut i = 0;
        loop {
            let mut ka = 0;
            for k in 0..l_frag {
                let kk = k + i;
                r1[k] = xtm[kk];
                r2[k] = ytm[kk];
                k_ali[ka] = kk;
                ka += 1;
            }

            kabsch(r1, r2, l_frag, 1, &mut rmsd, &mut t, &mut u);
            do_rotation(xtm, xt, lali, &t, &u);

            let mut d = local_d0_search - 1.0;
            let mut n_cut = score_fun8(
                xt,
                ytm,
                lali,
                d,
                &mut i_ali,
                &mut score,
                score_sum_method,
                lnorm,
                score_d8,
                d0,
                standard,
            );
            if score > score_max {
                score_max = score;
                *t0 = t;
                *u0 = u;
            }

            d = local_d0_search + 1.0;
            for _ in 0..n_it {
                ka = 0;
                for k in 0..n_cut {
                    let m = i_ali[k];
                    r1[k] = xtm[m];
                    r2[k] = ytm[m];
                    k_ali[ka] = m;
                    ka += 1;
                }
                kabsch(r1, r2, n_cut, 1, &mut rmsd, &mut t, &mut u);
                do_rotation(xtm, xt, lali, &t, &u);
                n_cut = score_fun8(
                    xt,
                    ytm,
                    lali,
                    d,
                    &mut i_ali,
                    &mut score,
                    score_sum_method,
                    lnorm,
                    score_d8,
                    d0,
                    standard,
                );
                if score > score_max {
                    score_max = score;
                    *t0 = t;
                    *u0 = u;
                }

                if n_cut == ka && i_ali[..n_cut] == k_ali[..n_cut] {
                    break;
                }
            }

            if i < il_max {
                i += simplify_step;
                if i > il_max {
                    i = il_max;
                }
            } else {
                break;
            }
        }
    }
    score_max
}

/// Working arrays shared by the search routines.
struct Work {
    r1: Vec<Coord>,
    r2: Vec<Coord>,
    xtm: Vec<Coord>,
    ytm: Vec<Coord>,
    xt: Vec<Coord>,
    /// Dynamic programming matrices, `(xlen + 1) x (ylen + 1)`.
    score: Vec<f64>,
    path: Vec<bool>,
    val: Vec<f64>,
}

/// Fixed inputs of one alignment.
struct Problem<'a> {
    x: &'a [Coord],
    y: &'a [Coord],
    xlen: usize,
    ylen: usize,
    secx: &'a [u8],
    secy: &'a [u8],
}

impl Work {
    fn new(xlen: usize, ylen: usize) -> Self {
        let minlen = xlen.min(ylen);
        let cells = (xlen + 1) * (ylen + 1);
        Self {
            r1: vec![[0.0; 3]; minlen],
            r2: vec![[0.0; 3]; minlen],
            xtm: vec![[0.0; 3]; minlen],
            ytm: vec![[0.0; 3]; minlen],
            xt: vec![[0.0; 3]; xlen],
            score: vec![0.0; cells],
            path: vec![false; cells],
            val: vec![0.0; cells],
        }
    }

    /// Copies the pairs within `max(d002t, third smallest distance)` to
    /// `r1`/`r2` and returns their count.
    ///
    /// US-align retries with `d002t + 0.5` while fewer than three pairs pass.
    /// The threshold already admits the three closest pairs, so that retry
    /// could only trigger on NaN distances, where it would never end.
    fn select_close_pairs(&mut self, dis: &[f64], mut d002t: f64) -> usize {
        if dis.len() > 2 {
            let mut sorted = dis.to_vec();
            let third = *sorted.select_nth_unstable_by(2, f64::total_cmp).1;
            if d002t < third {
                d002t = third;
            }
        } else if let Some(last) = dis.iter().copied().max_by(f64::total_cmp) {
            if d002t < last {
                d002t = last;
            }
        }
        let mut j = 0;
        for k in 0..dis.len() {
            if dis[k] <= d002t {
                self.r1[j] = self.xtm[k];
                self.r2[j] = self.ytm[k];
                j += 1;
            }
        }
        j
    }

    /// Computes the score quickly in three iterations (`get_score_fast`).
    fn get_score_fast(
        &mut self,
        pr: &Problem,
        invmap: &[i32],
        d0: f64,
        d0_search: f64,
        t: &mut [f64; 3],
        u: &mut Mat3,
    ) -> f64 {
        let (x, y) = (pr.x, pr.y);
        let mut rms = 0.0;
        let mut k = 0;
        for j in 0..pr.ylen {
            let i = invmap[j];
            if i >= 0 {
                let i = i as usize;
                self.r1[k] = x[i];
                self.r2[k] = y[j];
                self.xtm[k] = x[i];
                self.ytm[k] = y[j];
                k += 1;
            }
        }
        kabsch(&self.r1, &self.r2, k, 1, &mut rms, t, u);

        let n_ali = k;
        let mut dis = vec![0.0; n_ali];
        let d00 = d0_search;
        let d002 = d00 * d00;
        let d02 = d0 * d0;

        let mut tmscore = 0.0;
        for k in 0..n_ali {
            let xrot = transform(t, u, &self.xtm[k]);
            let di = dist(&xrot, &self.ytm[k]);
            dis[k] = di;
            tmscore += 1.0 / (1.0 + di / d02);
        }

        let j = self.select_close_pairs(&dis, d002);

        let tmscore1;
        let tmscore2;
        if n_ali != j {
            kabsch(&self.r1, &self.r2, j, 1, &mut rms, t, u);
            let mut score = 0.0;
            for k in 0..n_ali {
                let xrot = transform(t, u, &self.xtm[k]);
                let di = dist(&xrot, &self.ytm[k]);
                dis[k] = di;
                score += 1.0 / (1.0 + di / d02);
            }
            tmscore1 = score;

            let j = self.select_close_pairs(&dis, d002 + 1.0);
            kabsch(&self.r1, &self.r2, j, 1, &mut rms, t, u);
            let mut score = 0.0;
            for k in 0..n_ali {
                let xrot = transform(t, u, &self.xtm[k]);
                let di = dist(&xrot, &self.ytm[k]);
                score += 1.0 / (1.0 + di / d02);
            }
            tmscore2 = score;
        } else {
            tmscore1 = tmscore;
            tmscore2 = tmscore;
        }

        if tmscore1 >= tmscore {
            tmscore = tmscore1;
        }
        if tmscore2 >= tmscore {
            tmscore = tmscore2;
        }
        tmscore
    }

    /// Gapless threading for the initial alignment (`get_initial`).
    fn get_initial(
        &mut self,
        pr: &Problem,
        y2x: &mut [i32],
        d0: f64,
        d0_search: f64,
        t: &mut [f64; 3],
        u: &mut Mat3,
    ) {
        let (xlen, ylen) = (pr.xlen as i32, pr.ylen as i32);
        let min_len = xlen.min(ylen);
        let mut min_ali = min_len / 2;
        if min_ali <= 5 {
            min_ali = 5;
        }
        let n1 = -ylen + min_ali;
        let n2 = xlen - min_ali;

        let mut tmscore_max = -1.0;
        let mut k_best = n1;
        for k in n1..=n2 {
            for j in 0..ylen {
                let i = j + k;
                y2x[j as usize] = if i >= 0 && i < xlen { i } else { -1 };
            }
            let tmscore = self.get_score_fast(pr, y2x, d0, d0_search, t, u);
            if tmscore >= tmscore_max {
                tmscore_max = tmscore;
                k_best = k;
            }
        }

        for j in 0..ylen {
            let i = j + k_best;
            y2x[j as usize] = if i >= 0 && i < xlen { i } else { -1 };
        }
    }

    /// `detailed_search` (`standard == false`) or `detailed_search_standard`
    /// without normalization.
    fn detailed_search(
        &mut self,
        pr: &Problem,
        invmap0: &[i32],
        t: &mut [f64; 3],
        u: &mut Mat3,
        simplify_step: usize,
        score_sum_method: i32,
        local_d0_search: f64,
        lnorm: f64,
        score_d8: f64,
        d0: f64,
        standard: bool,
    ) -> f64 {
        let mut k = 0;
        for i in 0..pr.ylen {
            let j = invmap0[i];
            if j >= 0 {
                self.xtm[k] = pr.x[j as usize];
                self.ytm[k] = pr.y[i];
                k += 1;
            }
        }
        tmscore8_search(
            &mut self.r1,
            &mut self.r2,
            &self.xtm,
            &self.ytm,
            &mut self.xt,
            k,
            t,
            u,
            simplify_step,
            score_sum_method,
            local_d0_search,
            lnorm,
            score_d8,
            d0,
            standard,
        )
    }

    /// Traces back a filled DP matrix into `j2i`.
    fn nw_traceback(&self, len1: usize, len2: usize, gap_open: f64, j2i: &mut [i32]) {
        let w = len2 + 1;
        let (path, val) = (&self.path, &self.val);
        let mut i = len1;
        let mut j = len2;
        while i > 0 && j > 0 {
            if path[i * w + j] {
                j2i[j - 1] = (i - 1) as i32;
                i -= 1;
                j -= 1;
            } else {
                let mut h = val[(i - 1) * w + j];
                if path[(i - 1) * w + j] {
                    h += gap_open;
                }
                let mut v = val[i * w + j - 1];
                if path[i * w + j - 1] {
                    v += gap_open;
                }
                if v >= h {
                    j -= 1;
                } else {
                    i -= 1;
                }
            }
        }
    }

    /// Needleman-Wunsch without gap extension (`NWDP_TM`); `diag(i, j)`
    /// gives the score of aligning residue `i` to `j` (0-based).
    fn nwdp(
        &mut self,
        len1: usize,
        len2: usize,
        gap_open: f64,
        j2i: &mut [i32],
        mut diag: impl FnMut(&Self, usize, usize) -> f64,
    ) {
        let w = len2 + 1;
        for i in 0..=len1 {
            self.val[i * w] = 0.0;
            self.path[i * w] = false;
        }
        for j in 0..=len2 {
            self.val[j] = 0.0;
            self.path[j] = false;
            j2i[j] = -1;
        }
        for i in 1..=len1 {
            for j in 1..=len2 {
                let d = self.val[(i - 1) * w + j - 1] + diag(self, i - 1, j - 1);
                let mut h = self.val[(i - 1) * w + j];
                if self.path[(i - 1) * w + j] {
                    h += gap_open;
                }
                let mut v = self.val[i * w + j - 1];
                if self.path[i * w + j - 1] {
                    v += gap_open;
                }
                if d >= h && d >= v {
                    self.path[i * w + j] = true;
                    self.val[i * w + j] = d;
                } else {
                    self.path[i * w + j] = false;
                    self.val[i * w + j] = if v >= h { v } else { h };
                }
            }
        }
        self.nw_traceback(len1, len2, gap_open, j2i);
    }

    /// `NWDP_TM` on superposed coordinates.
    fn nwdp_superposed(
        &mut self,
        pr: &Problem,
        t: &[f64; 3],
        u: &Mat3,
        d02: f64,
        gap_open: f64,
        j2i: &mut [i32],
    ) {
        // `xt` is scratch here: every reader rewrites it first.
        do_rotation(pr.x, &mut self.xt, pr.xlen, t, u);
        let y = pr.y;
        self.nwdp(pr.xlen, pr.ylen, gap_open, j2i, |work, i, j| {
            let dij = dist(&work.xt[i], &y[j]);
            1.0 / (1.0 + dij / d02)
        });
    }

    /// Iterative dynamic programming (`DP_iter`).
    fn dp_iter(
        &mut self,
        pr: &Problem,
        t: &mut [f64; 3],
        u: &mut Mat3,
        invmap0: &mut [i32],
        g1: usize,
        g2: usize,
        iteration_max: usize,
        local_d0_search: f64,
        lnorm: f64,
        d0: f64,
        score_d8: f64,
    ) -> f64 {
        let gap_open = [-0.6, 0.0];
        let mut invmap = vec![0i32; pr.ylen + 1];
        let mut tmscore_max = -1.0;
        let mut tmscore_old = 0.0;
        let d02 = d0 * d0;
        for g in g1..g2 {
            for iteration in 0..iteration_max {
                self.nwdp_superposed(pr, t, u, d02, gap_open[g], &mut invmap);
                let tmscore = self.detailed_search(
                    pr,
                    &invmap,
                    t,
                    u,
                    40,
                    8,
                    local_d0_search,
                    lnorm,
                    score_d8,
                    d0,
                    false,
                );
                if tmscore > tmscore_max {
                    tmscore_max = tmscore;
                    invmap0[..pr.ylen].copy_from_slice(&invmap[..pr.ylen]);
                }
                if iteration > 0 && (tmscore_old - tmscore).abs() < 0.000001 {
                    break;
                }
                tmscore_old = tmscore;
            }
        }
        tmscore_max
    }

    /// Initial alignment from secondary structure (`get_initial_ss`).
    fn get_initial_ss(&mut self, pr: &Problem, y2x: &mut [i32]) {
        let (secx, secy) = (pr.secx, pr.secy);
        self.nwdp(pr.xlen, pr.ylen, -1.0, y2x, |_, i, j| {
            1.0 * f64::from(u8::from(secx[i] == secy[j]))
        });
    }

    /// Initial alignment from local superposition (`get_initial5`).
    fn get_initial5(
        &mut self,
        pr: &Problem,
        y2x: &mut [i32],
        d0: f64,
        d0_search: f64,
        d0_min: f64,
    ) -> bool {
        let (xlen, ylen) = (pr.xlen, pr.ylen);
        let mut rmsd = 0.0;
        let mut t = [0.0; 3];
        let mut u = [[0.0; 3]; 3];

        let mut d01 = d0 + 1.5;
        if d01 < d0_min {
            d01 = d0_min;
        }
        let d02 = d01 * d01;

        let mut glmax = 0.0;
        let al = xlen.min(ylen);
        let mut invmap = vec![0i32; ylen + 1];

        let jump = |len: usize| {
            let n: usize = if len > 250 {
                45
            } else if len > 200 {
                35
            } else if len > 150 {
                25
            } else {
                15
            };
            n.min(len / 3)
        };
        let n_jump1 = jump(xlen);
        let n_jump2 = jump(ylen);

        let mut n_frag = [20, 100];
        if n_frag[0] > al / 3 {
            n_frag[0] = al / 3;
        }
        if n_frag[1] > al / 2 {
            n_frag[1] = al / 2;
        }

        let mut flag = false;
        for &frag in &n_frag {
            let m1 = xlen + 1 - frag;
            let m2 = ylen + 1 - frag;
            for i in (0..m1).step_by(n_jump1) {
                for j in (0..m2).step_by(n_jump2) {
                    for k in 0..frag {
                        self.r1[k] = pr.x[k + i];
                        self.r2[k] = pr.y[k + j];
                    }
                    kabsch(&self.r1, &self.r2, frag, 1, &mut rmsd, &mut t, &mut u);
                    self.nwdp_superposed(pr, &t, &u, d02, 0.0, &mut invmap);
                    let gl = self.get_score_fast(pr, &invmap, d0, d0_search, &mut t, &mut u);
                    if gl > glmax {
                        glmax = gl;
                        y2x[..ylen].copy_from_slice(&invmap[..ylen]);
                        flag = true;
                    }
                }
            }
        }
        flag
    }

    /// Initial alignment from secondary structure and the previous
    /// alignment `y2x0` (`get_initial_ssplus`).
    fn get_initial_ssplus(
        &mut self,
        pr: &Problem,
        y2x0: &[i32],
        y2x: &mut [i32],
        d0_min: f64,
        d0: f64,
    ) {
        let (x, y) = (pr.x, pr.y);
        let mut rmsd = 0.0;
        let mut t = [0.0; 3];
        let mut u = [[0.0; 3]; 3];
        let mut d01 = d0 + 1.5;
        if d01 < d0_min {
            d01 = d0_min;
        }
        let d02 = d01 * d01;

        let mut k = 0;
        for j in 0..pr.ylen {
            let i = y2x0[j];
            if i >= 0 {
                self.r1[k] = x[i as usize];
                self.r2[k] = y[j];
                k += 1;
            }
        }
        kabsch(&self.r1, &self.r2, k, 1, &mut rmsd, &mut t, &mut u);

        let w = pr.ylen + 1;
        for ii in 0..pr.xlen {
            let xx = transform(&t, &u, &x[ii]);
            for jj in 0..pr.ylen {
                let dij = dist(&xx, &y[jj]);
                self.score[(ii + 1) * w + jj + 1] = if pr.secx[ii] == pr.secy[jj] {
                    1.0 / (1.0 + dij / d02) + 0.5
                } else {
                    1.0 / (1.0 + dij / d02)
                };
            }
        }

        self.nwdp(pr.xlen, pr.ylen, -1.0, y2x, |work, i, j| {
            work.score[(i + 1) * w + j + 1]
        });
    }

    /// Gapless threading of a fragment `ifr` of x (`on_x`) or of y; updates
    /// `y2x` whenever the score reaches `tmscore_max`.
    fn thread_fragment(
        &mut self,
        pr: &Problem,
        ifr: &[i32],
        on_x: bool,
        y2x: &mut [i32],
        tmscore_max: &mut f64,
        d0: f64,
        d0_search: f64,
        t: &mut [f64; 3],
        u: &mut Mat3,
    ) {
        let (xlen, ylen) = (pr.xlen as i32, pr.ylen as i32);
        let fra_min1 = 3;
        let min_ali_for = |min_len: i32| ((min_len as f64 / 2.5) as i32).max(fra_min1);
        let lf = ifr.len() as i32;
        let mut y2x_ = vec![-1i32; pr.ylen + 1];
        let (n1, n2) = if on_x {
            let min_ali = min_ali_for(lf.min(ylen));
            (-ylen + min_ali, lf - min_ali)
        } else {
            let min_ali = min_ali_for(xlen.min(lf));
            (-lf + min_ali, xlen - min_ali)
        };
        for k in n1..=n2 {
            if on_x {
                for j in 0..ylen {
                    let i = j + k;
                    y2x_[j as usize] = if i >= 0 && i < lf {
                        ifr[i as usize]
                    } else {
                        -1
                    };
                }
            } else {
                y2x_[..pr.ylen].fill(-1);
                for j in 0..lf {
                    let i = j + k;
                    if i >= 0 && i < xlen {
                        y2x_[ifr[j as usize] as usize] = i;
                    }
                }
            }
            let tmscore = self.get_score_fast(pr, &y2x_, d0, d0_search, t, u);
            if tmscore >= *tmscore_max {
                *tmscore_max = tmscore;
                y2x[..pr.ylen].copy_from_slice(&y2x_[..pr.ylen]);
            }
        }
    }

    /// Fragment gapless threading (`get_initial_fgt`).
    fn get_initial_fgt(
        &mut self,
        pr: &Problem,
        y2x: &mut [i32],
        d0: f64,
        d0_search: f64,
        dcu0: f64,
        t: &mut [f64; 3],
        u: &mut Mat3,
    ) {
        let (xlen, ylen) = (pr.xlen as i32, pr.ylen as i32);
        let (xstart, xend) = find_max_frag(pr.x, xlen, dcu0);
        let (ystart, yend) = find_max_frag(pr.y, ylen, dcu0);
        let lx = xend - xstart + 1;
        let ly = yend - ystart + 1;
        let l_fr = lx.min(ly);

        // A fragment spanning the whole chain is trimmed to its 10%..89% part.
        let fragment = |start: i32, len: i32, l0: i32| -> Vec<i32> {
            let ifr: Vec<i32> = (start..start + len).collect();
            if len != l0 {
                return ifr;
            }
            let n1 = (l0 as f64 * 0.1) as usize;
            let n2 = (l0 as f64 * 0.89) as usize;
            ifr[n1..=n2].to_vec()
        };

        let mut tmscore_max = -1.0;
        if lx == ly && xlen == ylen {
            let ifr = fragment(xstart, l_fr, xlen);
            self.thread_fragment(pr, &ifr, true, y2x, &mut tmscore_max, d0, d0_search, t, u);
            let ifr = fragment(ystart, ly, xlen);
            self.thread_fragment(pr, &ifr, false, y2x, &mut tmscore_max, d0, d0_search, t, u);
            return;
        }
        let on_x = lx < ly || (lx == ly && xlen < ylen);
        let ifr = fragment(if on_x { xstart } else { ystart }, l_fr, xlen.min(ylen));
        self.thread_fragment(pr, &ifr, on_x, y2x, &mut tmscore_max, d0, d0_search, t, u);
    }
}

/// Longest run of consecutive residues closer than `dcu0` (`find_max_frag`);
/// returns (start, end).
fn find_max_frag(x: &[Coord], len: i32, dcu0: f64) -> (i32, i32) {
    let fra_min = 4;
    let mut start_max = 0;
    let mut end_max = 0;
    let mut lfr_max = 0;

    let mut r_min = (len as f64 * 1.0 / 3.0) as i32;
    if r_min > fra_min {
        r_min = fra_min;
    }

    let mut inc = 0;
    let dcu0_cut = dcu0 * dcu0;
    let mut dcu_cut = dcu0_cut;

    while lfr_max < r_min {
        lfr_max = 0;
        let mut j = 1;
        let mut start = 0;
        for i in 1..len {
            if dist(&x[i as usize - 1], &x[i as usize]) < dcu_cut {
                j += 1;
                if i == len - 1 {
                    if j > lfr_max {
                        lfr_max = j;
                        start_max = start;
                        end_max = i;
                    }
                    j = 1;
                }
            } else {
                if j > lfr_max {
                    lfr_max = j;
                    start_max = start;
                    end_max = i - 1;
                }
                j = 1;
                start = i;
            }
        }

        if lfr_max < r_min {
            // Another pass cannot change anything once the cutoff is infinite
            // (NaN or overflowing distances); US-align would loop forever.
            if !dcu_cut.is_finite() {
                break;
            }
            inc += 1;
            let dinc = pow(1.1, inc as f64) * dcu0;
            dcu_cut = dinc * dinc;
        }
    }
    (start_max, end_max)
}

fn sec_str_protein(dis13: f64, dis14: f64, dis15: f64, dis24: f64, dis25: f64, dis35: f64) -> u8 {
    let mut delta = 2.1;
    if (dis15 - 6.37).abs() < delta
        && (dis14 - 5.18).abs() < delta
        && (dis25 - 5.18).abs() < delta
        && (dis13 - 5.45).abs() < delta
        && (dis24 - 5.45).abs() < delta
        && (dis35 - 5.45).abs() < delta
    {
        return b'H';
    }

    delta = 1.42;
    if (dis15 - 13.0).abs() < delta
        && (dis14 - 10.4).abs() < delta
        && (dis25 - 10.4).abs() < delta
        && (dis13 - 6.1).abs() < delta
        && (dis24 - 6.1).abs() < delta
        && (dis35 - 6.1).abs() < delta
    {
        return b'E';
    }

    if dis15 < 8.0 {
        b'T'
    } else {
        b'C'
    }
}

/// Protein secondary structure: 'C' coil, 'H' helix, 'T' turn, 'E' strand.
fn make_sec_protein(x: &[Coord]) -> Vec<u8> {
    let len = x.len();
    let mut sec = vec![0u8; len + 1];
    for i in 0..len {
        sec[i] = b'C';
        if i >= 2 && i + 2 < len {
            let (j1, j2, j3, j4, j5) = (i - 2, i - 1, i, i + 1, i + 2);
            let d13 = dist(&x[j1], &x[j3]).sqrt();
            let d14 = dist(&x[j1], &x[j4]).sqrt();
            let d15 = dist(&x[j1], &x[j5]).sqrt();
            let d24 = dist(&x[j2], &x[j4]).sqrt();
            let d25 = dist(&x[j2], &x[j5]).sqrt();
            let d35 = dist(&x[j3], &x[j5]).sqrt();
            sec[i] = sec_str_protein(d13, d14, d15, d24, d25, d35);
        }
    }
    sec
}

/// Extends a base-pair stack from (a, b) inward; returns (c, d).
fn sec_str_rna(len: i32, bp: &[bool], a: i32, b: i32) -> (i32, i32) {
    let mut i = 0;
    while i < len {
        if a + i < len - 3 && b - i > 0 {
            if a + i < b - i && bp[((a + i) * len + b - i) as usize] {
                i += 1;
                continue;
            }
            break;
        }
        i += 1;
    }
    (a + i - 1, b - i + 1)
}

/// RNA secondary structure: '.' unpaired, '<' and '>' paired.
fn make_sec_rna(seq: &[u8], x: &[Coord], atom_opt: &str) -> Vec<u8> {
    let len = x.len() as i32;
    let n = x.len();
    let (lb, ub): (f32, f32) = match atom_opt {
        " C4'" => (14.0, 16.0),
        " C5'" => (16.0, 18.0),
        " O3'" => (13.5, 16.5),
        " O5'" => (15.5, 18.5),
        " P  " => (16.5, 21.0),
        _ => (12.5, 15.0),
    };

    let mut sec = vec![0u8; n + 1];
    let mut bp = vec![false; n * n];
    for i in 0..n {
        sec[i] = b'.';
        for j in i + 1..n {
            let (si, sj) = (seq[i], seq[j]);
            if ((si == b'u' || si == b't') && sj == b'a')
                || (si == b'a' && (sj == b'u' || sj == b't'))
                || (si == b'g' && (sj == b'c' || sj == b'u'))
                || ((si == b'c' || si == b'u') && sj == b'g')
            {
                let dis = dist(&x[i], &x[j]).sqrt() as f32;
                let paired = dis > lb && dis < ub;
                bp[i * n + j] = paired;
                bp[j * n + i] = paired;
            }
        }
    }

    let mut stacks = Vec::new();
    for i in 0..len - 2 {
        for j in i + 3..len {
            let at = |a: i32, b: i32| bp[(a * len + b) as usize];
            if !at(i, j) {
                continue;
            }
            if i > 0 && j + 1 < len && at(i - 1, j + 1) {
                continue;
            }
            if !at(i + 1, j - 1) {
                continue;
            }
            let (mut ii, mut jj) = sec_str_rna(len, &bp, i, j);
            if jj < i || j < ii {
                ii = i;
                jj = j;
            }
            stacks.push((i, ii, jj));
        }
    }

    for &(a0, c0, d0) in &stacks {
        let mut j = 0;
        while a0 + j <= c0 {
            sec[(a0 + j) as usize] = b'<';
            sec[(d0 + j) as usize] = b'>';
            j += 1;
        }
    }
    sec
}

fn make_sec(chain: &Chain, atom_name: &str) -> Vec<u8> {
    if chain.mol_score > 0 {
        make_sec_rna(&chain.seq, &chain.xyz, atom_name)
    } else {
        make_sec_protein(&chain.xyz)
    }
}

/// Aligns `mov` (structure 1) onto `reference` (structure 2). `atom_name` is
/// the 4-column pivot atom name selecting the RNA base-pair distance bounds.
pub(crate) fn align(mov: &Chain, reference: &Chain, atom_name: &str) -> Result<Alignment, String> {
    if mov.len() < 3 || reference.len() < 3 {
        return Err("US-align: sequence is too short (<3 residues)".into());
    }
    // The score and DP matrices hold two doubles and a traceback flag per cell.
    const MAX_CELLS: usize = 32 * 1024 * 1024;
    let fits = |w: usize, h: usize| w.checked_mul(h).is_some_and(|n| n <= MAX_CELLS);
    if !fits(mov.len() + 1, reference.len() + 1)
        || [mov, reference]
            .iter()
            .any(|c| c.mol_score > 0 && !fits(c.len(), c.len()))
    {
        return Err("US-align matrix exceeds the memory budget".into());
    }
    let secx = make_sec(mov, atom_name);
    let secy = make_sec(reference, atom_name);
    let mol_type = mov.mol_score + reference.mol_score;
    let pr = Problem {
        x: &mov.xyz,
        y: &reference.xyz,
        xlen: mov.len(),
        ylen: reference.len(),
        secx: &secx,
        secy: &secy,
    };
    let (xlen, ylen) = (pr.xlen, pr.ylen);
    let mut w = Work::new(xlen, ylen);

    let sp = parameter_set4search(xlen, ylen);
    let (d0_min, lnorm, score_d8, d0, d0_search, dcu0) = (
        sp.d0_min,
        sp.lnorm,
        sp.score_d8,
        sp.d0,
        sp.d0_search,
        sp.dcu0,
    );
    let simplify_step = 40;
    let score_sum_method = 8;

    let mut t = [0.0; 3];
    let mut u = [[0.0; 3]; 3];
    let mut invmap0 = vec![-1i32; ylen + 1];
    let mut invmap = vec![-1i32; ylen + 1];
    let mut tmmax = -1.0;
    let ddcc = if lnorm <= 40.0 { 0.1 } else { 0.4 };
    let local_d0_search = d0_search;

    let search = |w: &mut Work, map: &[i32], t: &mut [f64; 3], u: &mut Mat3| {
        w.detailed_search(
            &pr,
            map,
            t,
            u,
            simplify_step,
            score_sum_method,
            local_d0_search,
            lnorm,
            score_d8,
            d0,
            false,
        )
    };
    let dp_iter = |w: &mut Work,
                   t: &mut [f64; 3],
                   u: &mut Mat3,
                   map: &mut [i32],
                   g1: usize,
                   iterations: usize| {
        w.dp_iter(
            &pr,
            t,
            u,
            map,
            g1,
            2,
            iterations,
            local_d0_search,
            lnorm,
            d0,
            score_d8,
        )
    };
    let keep = |tm: f64, tmmax: &mut f64, invmap0: &mut [i32], invmap: &[i32]| {
        if tm > *tmmax {
            *tmmax = tm;
            invmap0[..ylen].copy_from_slice(&invmap[..ylen]);
        }
    };

    w.get_initial(&pr, &mut invmap0, d0, d0_search, &mut t, &mut u);
    let tm = search(&mut w, &invmap0, &mut t, &mut u);
    if tm > tmmax {
        tmmax = tm;
    }
    let tm = dp_iter(&mut w, &mut t, &mut u, &mut invmap, 0, 30);
    keep(tm, &mut tmmax, &mut invmap0, &invmap);

    w.get_initial_ss(&pr, &mut invmap);
    let tm = search(&mut w, &invmap, &mut t, &mut u);
    keep(tm, &mut tmmax, &mut invmap0, &invmap);
    if tm > tmmax * 0.2 {
        let tm = dp_iter(&mut w, &mut t, &mut u, &mut invmap, 0, 30);
        keep(tm, &mut tmmax, &mut invmap0, &invmap);
    }

    if w.get_initial5(&pr, &mut invmap, d0, d0_search, d0_min) {
        let tm = search(&mut w, &invmap, &mut t, &mut u);
        keep(tm, &mut tmmax, &mut invmap0, &invmap);
        if tm > tmmax * ddcc {
            let tm = dp_iter(&mut w, &mut t, &mut u, &mut invmap, 0, 2);
            keep(tm, &mut tmmax, &mut invmap0, &invmap);
        }
    }

    w.get_initial_ssplus(&pr, &invmap0, &mut invmap, d0_min, d0);
    let tm = search(&mut w, &invmap, &mut t, &mut u);
    keep(tm, &mut tmmax, &mut invmap0, &invmap);
    if tm > tmmax * ddcc {
        let tm = dp_iter(&mut w, &mut t, &mut u, &mut invmap, 0, 30);
        keep(tm, &mut tmmax, &mut invmap0, &invmap);
    }

    w.get_initial_fgt(&pr, &mut invmap, d0, d0_search, dcu0, &mut t, &mut u);
    let tm = search(&mut w, &invmap, &mut t, &mut u);
    keep(tm, &mut tmmax, &mut invmap0, &invmap);
    if tm > tmmax * ddcc {
        let tm = dp_iter(&mut w, &mut t, &mut u, &mut invmap, 1, 2);
        keep(tm, &mut tmmax, &mut invmap0, &invmap);
    }

    if !invmap0[..ylen].iter().any(|&i| i >= 0) {
        return Err("US-align: no alignment between the two structures".into());
    }

    w.detailed_search(
        &pr,
        &invmap0,
        &mut t,
        &mut u,
        1,
        8,
        local_d0_search,
        lnorm,
        score_d8,
        d0,
        true,
    );

    let (x, y) = (pr.x, pr.y);
    let mut aligned = Vec::new();
    do_rotation(x, &mut w.xt, xlen, &t, &u);
    let mut k = 0;
    for j in 0..ylen {
        let i = invmap0[j];
        if i >= 0 {
            let i = i as usize;
            let d = dist(&w.xt[i], &y[j]).sqrt();
            if d <= score_d8 {
                aligned.push((i, j));
                w.xtm[k] = x[i];
                w.ytm[k] = y[j];
                w.r1[k] = w.xt[i];
                w.r2[k] = y[j];
                k += 1;
            }
        }
    }
    let n_ali8 = k;

    let mut rmsd0 = 0.0;
    kabsch(&w.r1, &w.r2, n_ali8, 0, &mut rmsd0, &mut t, &mut u);
    rmsd0 = (rmsd0 / n_ali8 as f64).sqrt();

    let mut t0 = [0.0; 3];
    let mut u0 = [[0.0; 3]; 3];
    let (lnorm_a, d0_a, d0_search_a) = parameter_set4final(ylen as f64, mol_type);
    let tm1 = tmscore8_search(
        &mut w.r1,
        &mut w.r2,
        &w.xtm,
        &w.ytm,
        &mut w.xt,
        n_ali8,
        &mut t0,
        &mut u0,
        1,
        0,
        d0_search_a,
        lnorm_a,
        score_d8,
        d0_a,
        false,
    );

    let (lnorm_b, d0_b, d0_search_b) = parameter_set4final(xlen as f64 + 0.0, mol_type);
    let tm2 = tmscore8_search(
        &mut w.r1,
        &mut w.r2,
        &w.xtm,
        &w.ytm,
        &mut w.xt,
        n_ali8,
        &mut t,
        &mut u,
        1,
        0,
        d0_search_b,
        lnorm_b,
        score_d8,
        d0_b,
        false,
    );

    let mut liden = 0.0;
    for &(i, j) in &aligned {
        if mov.seq[i] == reference.seq[j] {
            liden += 1.0;
        }
    }

    Ok(Alignment {
        t: t0,
        u: u0,
        tm_reference: tm1,
        tm_moving: tm2,
        rmsd: rmsd0,
        aligned: n_ali8 as i32,
        sequence_identity: if n_ali8 > 0 {
            liden / n_ali8 as f64
        } else {
            0.0
        },
        nucleic: mol_type > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helix(len: usize, scale: impl Fn(usize) -> f64) -> Chain {
        let xyz: Vec<f64> = (0..len)
            .flat_map(|i| {
                let a = i as f64 * 1.75;
                let s = scale(i);
                [2.3 * a.cos() * s, 2.3 * a.sin() * s, 1.5 * i as f64 * s]
            })
            .collect();
        let names: Vec<u8> = (0..len).flat_map(|_| *b"ALA\0").collect();
        Chain::from_records(&xyz, &names)
    }

    /// Overflowing or NaN distances must not hang the cutoff relaxation loops.
    #[test]
    fn overflowing_and_nan_coordinates_terminate() {
        let reference = helix(40, |_| 1.0);
        for bad in [1e150, 1e200, 1e300, f64::INFINITY, f64::NAN] {
            for moving in [
                helix(40, |_| bad),
                helix(40, |i| if i < 9 { 1.0 } else { bad }),
            ] {
                let _ = align(&moving, &reference, " CA ");
                let _ = align(&reference, &moving, " CA ");
            }
        }
    }
}
