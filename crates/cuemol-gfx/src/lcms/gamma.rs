//! Tone curves (port of LittleCMS 2.19.1 cmsgamma.c, MIT).

use super::base::{quantize_val, quick_saturate_word, MATRIX_DET_TOLERANCE};
use super::intrp::InterpParams;

const MAX_NODES_IN_CURVE: u32 = 4097;
const MINUS_INF: f32 = -1e22f32;
const PLUS_INF: f32 = 1e22f32;

#[derive(Clone, Debug)]
pub struct CurveSegment {
    pub x0: f32,
    pub x1: f32,
    pub typ: i32,
    pub params: [f64; 10],
    pub sampled: Vec<f32>,
}

impl CurveSegment {
    pub fn parametric(x0: f32, x1: f32, typ: i32, params: [f64; 10]) -> Self {
        CurveSegment {
            x0,
            x1,
            typ,
            params,
            sampled: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToneCurve {
    pub segments: Vec<CurveSegment>,
    seg_interp: Vec<Option<InterpParams>>,
    pub table16: Vec<u16>,
    interp: InterpParams,
}

fn parameter_count(typ: i32) -> Option<usize> {
    Some(match typ.unsigned_abs() {
        1 => 1,
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 7,
        6 => 4,
        7 => 5,
        8 => 5,
        108 => 1,
        109 => 1,
        _ => return None,
    })
}

fn sigmoid_base(k: f64, t: f64) -> f64 {
    (1.0 / (1.0 + (-k * t).exp())) - 0.5
}

fn inverted_sigmoid_base(k: f64, t: f64) -> f64 {
    -((1.0 / (t + 0.5)) - 1.0).ln() / k
}

fn sigmoid_factory(k: f64, t: f64) -> f64 {
    let correction = 0.5 / sigmoid_base(k, 1.0);
    correction * sigmoid_base(k, 2.0 * t - 1.0) + 0.5
}

fn inverse_sigmoid_factory(k: f64, t: f64) -> f64 {
    let correction = 0.5 / sigmoid_base(k, 1.0);
    (inverted_sigmoid_base(k, (t - 0.5) / correction) + 1.0) / 2.0
}

pub fn eval_parametric(typ: i32, p: &[f64; 10], r: f64) -> f64 {
    let tol = MATRIX_DET_TOLERANCE;
    match typ {
        1 => {
            if r < 0.0 {
                if (p[0] - 1.0).abs() < tol {
                    r
                } else {
                    0.0
                }
            } else {
                r.powf(p[0])
            }
        }
        -1 => {
            if r < 0.0 {
                if (p[0] - 1.0).abs() < tol {
                    r
                } else {
                    0.0
                }
            } else if p[0].abs() < tol {
                PLUS_INF as f64
            } else {
                r.powf(1.0 / p[0])
            }
        }
        2 => {
            if p[1].abs() < tol {
                0.0
            } else {
                let disc = -p[2] / p[1];
                if r >= disc {
                    let e = p[1] * r + p[2];
                    if e > 0.0 {
                        e.powf(p[0])
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }
        }
        -2 => {
            if p[0].abs() < tol || p[1].abs() < tol {
                0.0
            } else {
                let mut val = if r < 0.0 {
                    0.0
                } else {
                    (r.powf(1.0 / p[0]) - p[2]) / p[1]
                };
                if val < 0.0 {
                    val = 0.0;
                }
                val
            }
        }
        3 => {
            if p[1].abs() < tol {
                0.0
            } else {
                let mut disc = -p[2] / p[1];
                if disc < 0.0 {
                    disc = 0.0;
                }
                if r >= disc {
                    let e = p[1] * r + p[2];
                    if e > 0.0 {
                        e.powf(p[0]) + p[3]
                    } else {
                        0.0
                    }
                } else {
                    p[3]
                }
            }
        }
        -3 => {
            if p[0].abs() < tol || p[1].abs() < tol {
                0.0
            } else if r >= p[3] {
                let e = r - p[3];
                if e > 0.0 {
                    (e.powf(1.0 / p[0]) - p[2]) / p[1]
                } else {
                    0.0
                }
            } else {
                -p[2] / p[1]
            }
        }
        4 => {
            if r >= p[4] {
                let e = p[1] * r + p[2];
                if e > 0.0 {
                    e.powf(p[0])
                } else {
                    0.0
                }
            } else {
                r * p[3]
            }
        }
        -4 => {
            let e = p[1] * p[4] + p[2];
            let disc = if e < 0.0 { 0.0 } else { e.powf(p[0]) };
            if r >= disc {
                if p[0].abs() < tol || p[1].abs() < tol {
                    0.0
                } else {
                    (r.powf(1.0 / p[0]) - p[2]) / p[1]
                }
            } else if p[3].abs() < tol {
                0.0
            } else {
                r / p[3]
            }
        }
        5 => {
            if r >= p[4] {
                let e = p[1] * r + p[2];
                if e > 0.0 {
                    e.powf(p[0]) + p[5]
                } else {
                    p[5]
                }
            } else {
                r * p[3] + p[6]
            }
        }
        -5 => {
            let disc = p[3] * p[4] + p[6];
            if r >= disc {
                let e = r - p[5];
                if e < 0.0 {
                    0.0
                } else if p[0].abs() < tol || p[1].abs() < tol {
                    0.0
                } else {
                    (e.powf(1.0 / p[0]) - p[2]) / p[1]
                }
            } else if p[3].abs() < tol {
                0.0
            } else {
                (r - p[6]) / p[3]
            }
        }
        6 => {
            let e = p[1] * r + p[2];
            if p[0] == 1.0 {
                e + p[3]
            } else if e < 0.0 {
                p[3]
            } else {
                e.powf(p[0]) + p[3]
            }
        }
        -6 => {
            if p[0].abs() < tol || p[1].abs() < tol {
                0.0
            } else {
                let e = r - p[3];
                if e < 0.0 {
                    0.0
                } else {
                    (e.powf(1.0 / p[0]) - p[2]) / p[1]
                }
            }
        }
        7 => {
            let e = p[2] * r.powf(p[0]) + p[3];
            if e <= 0.0 {
                p[4]
            } else {
                p[1] * e.log10() + p[4]
            }
        }
        -7 => {
            if p[0].abs() < tol || p[1].abs() < tol || p[2].abs() < tol {
                0.0
            } else {
                ((10.0f64.powf((r - p[4]) / p[1]) - p[3]) / p[2]).powf(1.0 / p[0])
            }
        }
        8 => p[0] * p[1].powf(p[2] * r + p[3]) + p[4],
        -8 => {
            let disc = r - p[4];
            if disc < 0.0 {
                0.0
            } else if p[0].abs() < tol || p[2].abs() < tol {
                0.0
            } else {
                ((disc / p[0]).ln() / p[1].ln() - p[3]) / p[2]
            }
        }
        108 => {
            if p[0].abs() < tol {
                0.0
            } else {
                (1.0 - (1.0 - r).powf(1.0 / p[0])).powf(1.0 / p[0])
            }
        }
        -108 => 1.0 - (1.0 - r.powf(p[0])).powf(p[0]),
        109 => sigmoid_factory(p[0], r),
        -109 => inverse_sigmoid_factory(p[0], r),
        _ => 0.0,
    }
}

impl ToneCurve {
    fn allocate(
        n_entries: u32,
        segments: Vec<CurveSegment>,
        values: Option<&[u16]>,
    ) -> Option<Self> {
        if n_entries > 65530 {
            return None;
        }
        if n_entries == 0 && segments.is_empty() {
            return None;
        }
        let table16 = match values {
            Some(v) => v[..n_entries as usize].to_vec(),
            None => vec![0; n_entries as usize],
        };
        let seg_interp = segments
            .iter()
            .map(|s| {
                if s.typ == 0 {
                    InterpParams::new(&[s.sampled.len() as u32], 1, 1, false)
                } else {
                    None
                }
            })
            .collect();
        let interp = InterpParams::new(&[n_entries], 1, 1, false)?;
        Some(ToneCurve {
            segments,
            seg_interp,
            table16,
            interp,
        })
    }

    pub fn n_entries(&self) -> u32 {
        self.table16.len() as u32
    }

    fn eval_segmented(&self, r: f64) -> f64 {
        for (seg, interp) in self.segments.iter().zip(&self.seg_interp).rev() {
            if r > seg.x0 as f64 && r <= seg.x1 as f64 {
                let out = if seg.typ == 0 {
                    let r1 = ((r - seg.x0 as f64) as f32) / (seg.x1 - seg.x0);
                    let mut out32 = [0f32];
                    if let Some(p) = interp {
                        p.eval_float(&seg.sampled, &[r1], &mut out32);
                    }
                    out32[0] as f64
                } else {
                    eval_parametric(seg.typ, &seg.params, r)
                };
                if out.is_infinite() {
                    return PLUS_INF as f64;
                }
                return out;
            }
        }
        MINUS_INF as f64
    }

    pub fn tabulated16(n_entries: u32, values: Option<&[u16]>) -> Option<Self> {
        Self::allocate(n_entries, Vec::new(), values)
    }

    pub fn segmented(segments: Vec<CurveSegment>) -> Option<Self> {
        let mut n_grid = 4096;
        if segments.len() == 1 && segments[0].typ == 1 {
            n_grid = if (segments[0].params[0] - 1.0).abs() < 0.001 {
                2
            } else {
                4096
            };
        }
        let mut g = Self::allocate(n_grid, segments, None)?;
        for i in 0..n_grid {
            let r = i as f64 / (n_grid - 1) as f64;
            let val = g.eval_segmented(r);
            g.table16[i as usize] = quick_saturate_word(val * 65535.0);
        }
        Some(g)
    }

    pub fn tabulated_float(values: &[f32]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let mut p0 = [0.0; 10];
        p0[0] = 1.0;
        p0[3] = values[0] as f64;
        let mut p2 = [0.0; 10];
        p2[0] = 1.0;
        p2[3] = values[values.len() - 1] as f64;
        let segs = vec![
            CurveSegment::parametric(MINUS_INF, 0.0, 6, p0),
            CurveSegment {
                x0: 0.0,
                x1: 1.0,
                typ: 0,
                params: [0.0; 10],
                sampled: values.to_vec(),
            },
            CurveSegment::parametric(1.0, PLUS_INF, 6, p2),
        ];
        Self::segmented(segs)
    }

    pub fn parametric(typ: i32, params: &[f64]) -> Option<Self> {
        let count = parameter_count(typ)?;
        let mut p = [0.0; 10];
        p[..count].copy_from_slice(&params[..count]);
        Self::segmented(vec![CurveSegment::parametric(MINUS_INF, PLUS_INF, typ, p)])
    }

    pub fn gamma(g: f64) -> Option<Self> {
        Self::parametric(1, &[g])
    }

    pub fn eval16(&self, v: u16) -> u16 {
        let mut out = [0u16];
        self.interp.eval16(&self.table16, &[v], &mut out);
        out[0]
    }

    pub fn eval_float(&self, v: f32) -> f32 {
        if self.segments.is_empty() {
            let input = quick_saturate_word(v as f64 * 65535.0);
            let out = self.eval16(input);
            return (out as f64 / 65535.0) as f32;
        }
        self.eval_segmented(v as f64) as f32
    }

    pub fn is_descending(&self) -> bool {
        match (self.table16.first(), self.table16.last()) {
            (Some(a), Some(b)) => a > b,
            _ => false,
        }
    }

    pub fn is_linear(&self) -> bool {
        let n = self.n_entries();
        for (i, &v) in self.table16.iter().enumerate() {
            let diff = (v as i32 - quantize_val(i as f64, n) as i32).abs();
            if diff > 0x0f {
                return false;
            }
        }
        true
    }

    pub fn is_monotonic(&self) -> bool {
        let n = self.table16.len();
        if n < 2 {
            return true;
        }
        let t = &self.table16;
        if self.is_descending() {
            let mut last = t[0] as i32;
            for &v in &t[1..] {
                if v as i32 - last > 2 {
                    return false;
                }
                last = v as i32;
            }
        } else {
            let mut last = t[n - 1] as i32;
            for i in (0..n - 1).rev() {
                if t[i] as i32 - last > 2 {
                    return false;
                }
                last = t[i] as i32;
            }
        }
        true
    }

    fn get_interval(&self, input: f64) -> i32 {
        let domain = self.interp.domain[0];
        if domain < 1 {
            return -1;
        }
        let t = &self.table16;
        let check = |i: usize| -> bool {
            let y0 = t[i] as f64;
            let y1 = t[i + 1] as f64;
            if y0 <= y1 {
                input >= y0 && input <= y1
            } else {
                input >= y1 && input <= y0
            }
        };
        if t[0] < t[domain as usize] {
            for i in (0..domain as usize).rev() {
                if check(i) {
                    return i as i32;
                }
            }
        } else {
            for i in 0..domain as usize {
                if check(i) {
                    return i as i32;
                }
            }
        }
        -1
    }

    pub fn reverse_ex(&self, n_result: u32) -> Option<ToneCurve> {
        if self.segments.len() == 1
            && self.segments[0].typ > 0
            && parameter_count(self.segments[0].typ).is_some()
        {
            return ToneCurve::parametric(-self.segments[0].typ, &self.segments[0].params);
        }
        let mut out = ToneCurve::tabulated16(n_result, None)?;
        let ascending = !self.is_descending();
        let (mut a, mut b) = (0.0f64, 0.0f64);
        let n_entries = self.n_entries();
        for i in 0..n_result as usize {
            let y = i as f64 * 65535.0 / (n_result - 1) as f64;
            let j = self.get_interval(y);
            if j >= 0 {
                let j = j as usize;
                let x1 = self.table16[j] as f64;
                let x2 = self.table16[j + 1] as f64;
                let y1 = (j as f64 * 65535.0) / (n_entries - 1) as f64;
                let y2 = ((j + 1) as f64 * 65535.0) / (n_entries - 1) as f64;
                if x1 == x2 {
                    out.table16[i] = quick_saturate_word(if ascending { y2 } else { y1 });
                    continue;
                }
                a = (y2 - y1) / (x2 - x1);
                b = y2 - a * x2;
            }
            out.table16[i] = quick_saturate_word(a * y + b);
        }
        Some(out)
    }

    pub fn reverse(&self) -> Option<ToneCurve> {
        self.reverse_ex(4096)
    }

    pub fn estimate_gamma(&self, precision: f64) -> f64 {
        let (mut sum, mut sum2, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for i in 1..(MAX_NODES_IN_CURVE - 1) {
            let x = i as f64 / (MAX_NODES_IN_CURVE - 1) as f64;
            let y = self.eval_float(x as f32) as f64;
            if y > 0.0 && y < 1.0 && x > 0.07 {
                let gamma = y.ln() / x.ln();
                sum += gamma;
                sum2 += gamma * gamma;
                n += 1.0;
            }
        }
        if n <= 1.0 {
            return -1.0;
        }
        let std = ((n * sum2 - sum * sum) / (n * (n - 1.0))).sqrt();
        if std > precision {
            return -1.0;
        }
        sum / n
    }
}
