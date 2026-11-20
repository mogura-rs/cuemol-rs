//! Pipelines and stages (port of LittleCMS 2.19.1 cmslut.c, MIT).

use super::base::*;
use super::gamma::ToneCurve;
use super::intrp::InterpParams;

pub const SIG_CURVE_SET_ELEM: u32 = 0x6376_7374;
pub const SIG_MATRIX_ELEM: u32 = 0x6D61_7466;
pub const SIG_CLUT_ELEM: u32 = 0x636C_7574;
pub const SIG_XYZ2LAB_ELEM: u32 = 0x6C32_7820;
pub const SIG_LAB2XYZ_ELEM: u32 = 0x7832_6C20;
pub const SIG_NAMED_COLOR_ELEM: u32 = 0x6E63_6C20;
pub const SIG_LABV2_TO_V4: u32 = 0x3220_3420;
pub const SIG_LABV4_TO_V2: u32 = 0x3420_3220;
pub const SIG_IDENTITY_ELEM: u32 = 0x6964_6E20;
pub const SIG_LAB2_FLOAT_PCS: u32 = 0x6432_6C20;
pub const SIG_FLOAT_PCS2_LAB: u32 = 0x6C32_6420;
pub const SIG_XYZ2_FLOAT_PCS: u32 = 0x6432_7820;
pub const SIG_FLOAT_PCS2_XYZ: u32 = 0x7832_6420;

/// Allocation ceiling of the LittleCMS default allocator.
pub const MAX_MEMORY_FOR_ALLOC: u64 = 1024 * 1024 * 512;

#[derive(Clone, Debug)]
pub enum StageData {
    Identity,
    Curves(Vec<ToneCurve>),
    Matrix {
        m: Vec<f64>,
        off: Option<Vec<f64>>,
    },
    Clut16 {
        params: InterpParams,
        table: Vec<u16>,
    },
    ClutFloat {
        params: InterpParams,
        table: Vec<f32>,
    },
    Lab2Xyz,
    Xyz2Lab,
}

#[derive(Clone, Debug)]
pub struct Stage {
    pub typ: u32,
    pub implements: u32,
    pub in_ch: usize,
    pub out_ch: usize,
    pub data: StageData,
}

fn placeholder(typ: u32, in_ch: usize, out_ch: usize, data: StageData) -> Stage {
    Stage {
        typ,
        implements: typ,
        in_ch,
        out_ch,
        data,
    }
}

pub fn cube_size(dims: &[u32], b: usize) -> u32 {
    let mut rv: u64 = 1;
    for &dim in dims[..b].iter().rev() {
        if dim <= 1 {
            return 0;
        }
        if rv > (u32::MAX / dim) as u64 {
            return 0;
        }
        rv *= dim as u64;
    }
    if rv > (u32::MAX / 15) as u64 {
        return 0;
    }
    rv as u32
}

impl Stage {
    pub fn identity(n: usize) -> Stage {
        placeholder(SIG_IDENTITY_ELEM, n, n, StageData::Identity)
    }

    pub fn tone_curves(n: usize, curves: Option<&[ToneCurve]>) -> Option<Stage> {
        let curves = match curves {
            Some(c) => c[..n].to_vec(),
            None => (0..n)
                .map(|_| ToneCurve::gamma(1.0))
                .collect::<Option<Vec<_>>>()?,
        };
        Some(placeholder(
            SIG_CURVE_SET_ELEM,
            n,
            n,
            StageData::Curves(curves),
        ))
    }

    pub fn identity_curves(n: usize) -> Option<Stage> {
        let mut s = Stage::tone_curves(n, None)?;
        s.implements = SIG_IDENTITY_ELEM;
        Some(s)
    }

    pub fn matrix(rows: usize, cols: usize, m: &[f64], off: Option<&[f64]>) -> Option<Stage> {
        let n = rows.checked_mul(cols)?;
        if n == 0
            || n as u64 >= u32::MAX as u64 / cols as u64
            || n as u64 >= u32::MAX as u64 / rows as u64
        {
            return None;
        }
        Some(placeholder(
            SIG_MATRIX_ELEM,
            cols,
            rows,
            StageData::Matrix {
                m: m[..n].to_vec(),
                off: off.map(|o| o[..rows].to_vec()),
            },
        ))
    }

    pub fn clut_float(grid: &[u32], outputs: usize, table: Vec<f32>) -> Option<Self> {
        let inputs = grid.len();
        let params = InterpParams::new(grid, inputs, outputs, false)?;
        Some(placeholder(
            SIG_CLUT_ELEM,
            inputs,
            outputs,
            StageData::ClutFloat { params, table },
        ))
    }

    pub fn clut16_granular(
        points: &[u32],
        in_ch: usize,
        out_ch: usize,
        table: Option<&[u16]>,
    ) -> Option<Stage> {
        if in_ch > MAX_INPUT_DIMENSIONS {
            return None;
        }
        let n = (out_ch as u32).wrapping_mul(cube_size(points, in_ch));
        if n == 0 || n as u64 * 2 > MAX_MEMORY_FOR_ALLOC {
            return None;
        }
        let table = match table {
            Some(t) => t.get(..n as usize)?.to_vec(),
            None => vec![0; n as usize],
        };
        let params = InterpParams::new(points, in_ch, out_ch, false)?;
        Some(placeholder(
            SIG_CLUT_ELEM,
            in_ch,
            out_ch,
            StageData::Clut16 { params, table },
        ))
    }

    pub fn clut16(n: u32, in_ch: usize, out_ch: usize, table: Option<&[u16]>) -> Option<Stage> {
        Stage::clut16_granular(&[n; MAX_INPUT_DIMENSIONS], in_ch, out_ch, table)
    }

    pub fn identity_clut(n: usize) -> Option<Stage> {
        let mut s = Stage::clut16(2, n, n, None)?;
        if !s.sample_clut16(
            |i, o| {
                o[..n].copy_from_slice(&i[..n]);
                true
            },
            false,
        ) {
            return None;
        }
        s.implements = SIG_IDENTITY_ELEM;
        Some(s)
    }

    pub fn lab2xyz() -> Stage {
        placeholder(SIG_LAB2XYZ_ELEM, 3, 3, StageData::Lab2Xyz)
    }

    pub fn xyz2lab() -> Stage {
        placeholder(SIG_XYZ2LAB_ELEM, 3, 3, StageData::Xyz2Lab)
    }

    fn with_impl(s: Option<Stage>, implements: u32) -> Option<Stage> {
        s.map(|mut s| {
            s.implements = implements;
            s
        })
    }

    pub fn lab_v2_to_v4() -> Option<Stage> {
        let k = 65535.0 / 65280.0;
        let m = [k, 0.0, 0.0, 0.0, k, 0.0, 0.0, 0.0, k];
        Self::with_impl(Stage::matrix(3, 3, &m, None), SIG_LABV2_TO_V4)
    }

    pub fn lab_v4_to_v2() -> Option<Stage> {
        let k = 65280.0 / 65535.0;
        let m = [k, 0.0, 0.0, 0.0, k, 0.0, 0.0, 0.0, k];
        Self::with_impl(Stage::matrix(3, 3, &m, None), SIG_LABV4_TO_V2)
    }

    pub fn normalize_from_lab_float() -> Option<Stage> {
        let a1 = [
            1.0 / 100.0,
            0.0,
            0.0,
            0.0,
            1.0 / 255.0,
            0.0,
            0.0,
            0.0,
            1.0 / 255.0,
        ];
        let o1 = [0.0, 128.0 / 255.0, 128.0 / 255.0];
        Self::with_impl(Stage::matrix(3, 3, &a1, Some(&o1)), SIG_LAB2_FLOAT_PCS)
    }

    pub fn normalize_from_xyz_float() -> Option<Stage> {
        let n = 32768.0 / 65535.0;
        let a1 = [n, 0.0, 0.0, 0.0, n, 0.0, 0.0, 0.0, n];
        Self::with_impl(Stage::matrix(3, 3, &a1, None), SIG_XYZ2_FLOAT_PCS)
    }

    pub fn normalize_to_lab_float() -> Option<Stage> {
        let a1 = [100.0, 0.0, 0.0, 0.0, 255.0, 0.0, 0.0, 0.0, 255.0];
        let o1 = [0.0, -128.0, -128.0];
        Self::with_impl(Stage::matrix(3, 3, &a1, Some(&o1)), SIG_FLOAT_PCS2_LAB)
    }

    pub fn normalize_to_xyz_float() -> Option<Stage> {
        let n = 65535.0 / 32768.0;
        let a1 = [n, 0.0, 0.0, 0.0, n, 0.0, 0.0, 0.0, n];
        Self::with_impl(Stage::matrix(3, 3, &a1, None), SIG_FLOAT_PCS2_XYZ)
    }

    pub fn curves(&self) -> Option<&[ToneCurve]> {
        match &self.data {
            StageData::Curves(c) => Some(c),
            _ => None,
        }
    }

    pub fn sample_clut16<F>(&mut self, mut sampler: F, inspect: bool) -> bool
    where
        F: FnMut(&[u16], &mut [u16]) -> bool,
    {
        let StageData::Clut16 { params, table } = &mut self.data else {
            return false;
        };
        let n_inputs = params.n_inputs;
        let n_outputs = params.n_outputs;
        if n_inputs == 0 || n_outputs == 0 || n_inputs > MAX_INPUT_DIMENSIONS {
            return false;
        }
        if n_outputs >= MAX_STAGE_CHANNELS {
            return false;
        }
        let n_samples = params.n_samples;
        let total = cube_size(&n_samples, n_inputs);
        if total == 0 {
            return false;
        }
        let mut input = [0u16; MAX_INPUT_DIMENSIONS + 1];
        let mut out = [0u16; MAX_STAGE_CHANNELS];
        let mut index = 0usize;
        for i in 0..total as i32 {
            let mut rest = i;
            for t in (0..n_inputs).rev() {
                let colorant = (rest as u32) % n_samples[t];
                rest = ((rest as u32) / n_samples[t]) as i32;
                input[t] = quantize_val(colorant as f64, n_samples[t]);
            }
            out[..n_outputs].copy_from_slice(&table[index..index + n_outputs]);
            if !sampler(&input, &mut out) {
                return false;
            }
            if !inspect {
                table[index..index + n_outputs].copy_from_slice(&out[..n_outputs]);
            }
            index += n_outputs;
        }
        true
    }

    pub fn eval(&self, input: &[f32], output: &mut [f32]) {
        match &self.data {
            StageData::Identity => output[..self.in_ch].copy_from_slice(&input[..self.in_ch]),
            StageData::Curves(curves) => {
                for (i, c) in curves.iter().enumerate() {
                    output[i] = c.eval_float(input[i]);
                }
            }
            StageData::Matrix { m, off } => {
                for i in 0..self.out_ch {
                    let mut tmp = 0.0f64;
                    for j in 0..self.in_ch {
                        tmp += input[j] as f64 * m[i * self.in_ch + j];
                    }
                    if let Some(off) = off {
                        tmp += off[i];
                    }
                    output[i] = tmp as f32;
                }
            }
            StageData::Clut16 { params, table } => {
                let mut in16 = [0u16; MAX_STAGE_CHANNELS];
                let mut out16 = [0u16; MAX_STAGE_CHANNELS];
                from_float_to_16(input, &mut in16, self.in_ch);
                params.eval16(table, &in16, &mut out16);
                from_16_to_float(&out16, output, self.out_ch);
            }
            StageData::ClutFloat { params, table } => params.eval_float(table, input, output),
            StageData::Lab2Xyz => {
                let lab = CieLab {
                    l: input[0] as f64 * 100.0,
                    a: input[1] as f64 * 255.0 - 128.0,
                    b: input[2] as f64 * 255.0 - 128.0,
                };
                let xyz = lab2xyz(None, &lab);
                output[0] = (xyz.x / MAX_ENCODEABLE_XYZ) as f32;
                output[1] = (xyz.y / MAX_ENCODEABLE_XYZ) as f32;
                output[2] = (xyz.z / MAX_ENCODEABLE_XYZ) as f32;
            }
            StageData::Xyz2Lab => {
                let xyz = CieXyz {
                    x: input[0] as f64 * MAX_ENCODEABLE_XYZ,
                    y: input[1] as f64 * MAX_ENCODEABLE_XYZ,
                    z: input[2] as f64 * MAX_ENCODEABLE_XYZ,
                };
                let lab = xyz2lab(None, &xyz);
                output[0] = (lab.l / 100.0) as f32;
                output[1] = ((lab.a + 128.0) / 255.0) as f32;
                output[2] = ((lab.b + 128.0) / 255.0) as f32;
            }
        }
    }
}

pub fn from_float_to_16(input: &[f32], out: &mut [u16], n: usize) {
    for i in 0..n {
        out[i] = quick_saturate_word(input[i] as f64 * 65535.0);
    }
}

pub fn from_16_to_float(input: &[u16], out: &mut [f32], n: usize) {
    for i in 0..n {
        out[i] = input[i] as f32 / 65535.0f32;
    }
}

/// Precomputed 8-bit prelinearization + tetrahedral data.
#[derive(Clone, Debug)]
pub struct Prelin8 {
    pub clut_stage: usize,
    pub rx: [u16; 256],
    pub ry: [u16; 256],
    pub rz: [u16; 256],
    pub x0: [u32; 256],
    pub y0: [u32; 256],
    pub z0: [u32; 256],
}

#[derive(Clone, Debug)]
pub struct Prelin16 {
    pub n_inputs: usize,
    pub n_outputs: usize,
    pub curves_in: Option<Vec<ToneCurve>>,
    pub clut_stage: usize,
    pub curves_out: Option<Vec<ToneCurve>>,
}

#[derive(Clone, Debug)]
pub struct Curves16Data {
    pub curves: Vec<Vec<u16>>,
}

#[derive(Clone, Debug)]
pub struct MatShaper8 {
    pub shaper1: [[i32; 256]; 3],
    pub mat: [[i32; 3]; 3],
    pub off: [i32; 3],
    pub shaper2: [Vec<u16>; 3],
}

#[derive(Clone, Debug)]
pub enum Opt16 {
    Identity,
    Clut(usize),
    Prelin16(Box<Prelin16>),
    Prelin8(Box<Prelin8>),
    Curves8(Box<Curves16Data>),
    Curves16(Box<Curves16Data>),
    MatShaper(Box<MatShaper8>),
}

#[derive(Clone, Debug)]
pub struct Pipeline {
    pub stages: Vec<Stage>,
    pub input_channels: usize,
    pub output_channels: usize,
    pub opt: Option<Opt16>,
}

impl Pipeline {
    pub fn new(input_channels: usize, output_channels: usize) -> Option<Pipeline> {
        if input_channels >= MAX_CHANNELS || output_channels >= MAX_CHANNELS {
            return None;
        }
        Some(Pipeline {
            stages: Vec::new(),
            input_channels,
            output_channels,
            opt: None,
        })
    }

    pub fn bless(&mut self) -> bool {
        if let (Some(first), Some(last)) = (self.stages.first(), self.stages.last()) {
            self.input_channels = first.in_ch;
            self.output_channels = last.out_ch;
            for w in self.stages.windows(2) {
                if w[1].in_ch != w[0].out_ch {
                    return false;
                }
            }
        }
        true
    }

    pub fn insert_begin(&mut self, stage: Option<Stage>) -> bool {
        let Some(stage) = stage else { return false };
        self.stages.insert(0, stage);
        self.bless()
    }

    pub fn insert_end(&mut self, stage: Option<Stage>) -> bool {
        let Some(stage) = stage else { return false };
        self.stages.push(stage);
        self.bless()
    }

    pub fn cat(&mut self, other: &Pipeline) -> bool {
        if self.stages.is_empty() && other.stages.is_empty() {
            self.input_channels = other.input_channels;
            self.output_channels = other.output_channels;
        }
        for s in &other.stages {
            if !self.insert_end(Some(s.clone())) {
                return false;
            }
        }
        self.bless()
    }

    pub fn check_stages(&self, types: &[u32]) -> bool {
        self.stages.len() == types.len() && self.stages.iter().zip(types).all(|(s, &t)| s.typ == t)
    }

    pub fn eval_float(&self, input: &[f32], output: &mut [f32]) {
        let mut storage = [[0f32; MAX_STAGE_CHANNELS]; 2];
        let mut phase = 0;
        storage[0][..self.input_channels].copy_from_slice(&input[..self.input_channels]);
        for s in &self.stages {
            let (a, b) = storage.split_at_mut(1);
            if phase == 0 {
                s.eval(&a[0], &mut b[0]);
            } else {
                s.eval(&b[0], &mut a[0]);
            }
            phase ^= 1;
        }
        output[..self.output_channels].copy_from_slice(&storage[phase][..self.output_channels]);
    }

    fn eval16_default(&self, input: &[u16], output: &mut [u16]) {
        let mut storage = [[0f32; MAX_STAGE_CHANNELS]; 2];
        let mut phase = 0;
        from_16_to_float(input, &mut storage[0], self.input_channels);
        for s in &self.stages {
            let (a, b) = storage.split_at_mut(1);
            if phase == 0 {
                s.eval(&a[0], &mut b[0]);
            } else {
                s.eval(&b[0], &mut a[0]);
            }
            phase ^= 1;
        }
        from_float_to_16(&storage[phase], output, self.output_channels);
    }

    fn clut_of(&self, index: usize) -> (&InterpParams, &[u16]) {
        match &self.stages[index].data {
            StageData::Clut16 { params, table } => (params, table),
            _ => unreachable!("optimized pipeline lost its CLUT"),
        }
    }

    pub fn eval16(&self, input: &[u16], output: &mut [u16]) {
        match &self.opt {
            None => self.eval16_default(input, output),
            Some(Opt16::Identity) => {
                output[..self.input_channels].copy_from_slice(&input[..self.input_channels])
            }
            Some(Opt16::Clut(index)) => {
                let (params, table) = self.clut_of(*index);
                params.eval16(table, input, output);
            }
            Some(Opt16::Prelin16(p16)) => {
                let mut abc = [0u16; MAX_INPUT_DIMENSIONS];
                let mut def = [0u16; MAX_CHANNELS];
                for i in 0..p16.n_inputs {
                    abc[i] = match &p16.curves_in {
                        Some(c) => c[i].eval16(input[i]),
                        None => input[i],
                    };
                }
                let (params, table) = self.clut_of(p16.clut_stage);
                params.eval16(table, &abc, &mut def);
                for i in 0..p16.n_outputs {
                    output[i] = match &p16.curves_out {
                        Some(c) => c[i].eval16(def[i]),
                        None => def[i],
                    };
                }
            }
            Some(Opt16::Prelin8(p8)) => {
                let (params, table) = self.clut_of(p8.clut_stage);
                prelin_eval8(p8, params, table, input, output);
            }
            Some(Opt16::Curves8(c)) => {
                for (i, t) in c.curves.iter().enumerate() {
                    output[i] = t[(input[i] >> 8) as usize];
                }
            }
            Some(Opt16::Curves16(c)) => {
                for (i, t) in c.curves.iter().enumerate() {
                    output[i] = t[input[i] as usize];
                }
            }
            Some(Opt16::MatShaper(p)) => mat_shaper_eval16(p, input, output),
        }
    }
}

fn prelin_eval8(p8: &Prelin8, p: &InterpParams, lut: &[u16], input: &[u16], output: &mut [u16]) {
    let r = (input[0] >> 8) as usize;
    let g = (input[1] >> 8) as usize;
    let b = (input[2] >> 8) as usize;
    let xx0 = p8.x0[r] as i32;
    let yy0 = p8.y0[g] as i32;
    let zz0 = p8.z0[b] as i32;
    let rx = p8.rx[r] as i32;
    let ry = p8.ry[g] as i32;
    let rz = p8.rz[b] as i32;
    let xx1 = xx0 + if rx == 0 { 0 } else { p.opta[2] as i32 };
    let yy1 = yy0 + if ry == 0 { 0 } else { p.opta[1] as i32 };
    let zz1 = zz0 + if rz == 0 { 0 } else { p.opta[0] as i32 };
    for o in 0..p.n_outputs {
        output[o] = super::intrp::tetra_dens(lut, o, xx0, xx1, yy0, yy1, zz0, zz1, rx, ry, rz);
    }
}

fn mat_shaper_eval16(p: &MatShaper8, input: &[u16], output: &mut [u16]) {
    let ri = (input[0] & 0xFF) as usize;
    let gi = (input[1] & 0xFF) as usize;
    let bi = (input[2] & 0xFF) as usize;
    let r = p.shaper1[0][ri];
    let g = p.shaper1[1][gi];
    let b = p.shaper1[2][bi];
    let row = |k: usize| {
        p.mat[k][0]
            .wrapping_mul(r)
            .wrapping_add(p.mat[k][1].wrapping_mul(g))
            .wrapping_add(p.mat[k][2].wrapping_mul(b))
            .wrapping_add(p.off[k])
            .wrapping_add(0x2000)
            >> 14
    };
    for k in 0..3 {
        let l = row(k);
        let idx = if l < 0 {
            0
        } else if l > 16384 {
            16384
        } else {
            l as usize
        };
        output[k] = p.shaper2[k][idx];
    }
}
