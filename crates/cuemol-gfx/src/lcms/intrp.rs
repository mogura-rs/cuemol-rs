//! Interpolation routines (port of LittleCMS 2.19.1 cmsintrp.c, MIT).

use super::base::{to_fixed_domain, MAX_INPUT_DIMENSIONS, MAX_STAGE_CHANNELS};

#[derive(Clone, Debug)]
pub struct InterpParams {
    pub n_inputs: usize,
    pub n_outputs: usize,
    pub n_samples: [u32; MAX_INPUT_DIMENSIONS],
    pub domain: [u32; MAX_INPUT_DIMENSIONS],
    pub opta: [u32; MAX_INPUT_DIMENSIONS],
    pub trilinear: bool,
}

impl InterpParams {
    pub fn new(
        n_samples: &[u32],
        n_inputs: usize,
        n_outputs: usize,
        trilinear: bool,
    ) -> Option<Self> {
        if n_inputs > MAX_INPUT_DIMENSIONS || n_inputs == 0 {
            return None;
        }
        if n_inputs >= 4 && n_outputs >= MAX_STAGE_CHANNELS {
            return None;
        }
        let mut p = InterpParams {
            n_inputs,
            n_outputs,
            n_samples: [0; MAX_INPUT_DIMENSIONS],
            domain: [0; MAX_INPUT_DIMENSIONS],
            opta: [0; MAX_INPUT_DIMENSIONS],
            trilinear,
        };
        for i in 0..n_inputs {
            p.n_samples[i] = n_samples[i];
            p.domain[i] = n_samples[i].wrapping_sub(1);
        }
        p.opta[0] = n_outputs as u32;
        for i in 1..n_inputs {
            p.opta[i] = p.opta[i - 1].wrapping_mul(n_samples[n_inputs - i]);
        }
        Some(p)
    }

    pub fn eval16(&self, table: &[u16], input: &[u16], output: &mut [u16]) {
        match self.n_inputs {
            1 => {
                if self.n_outputs == 1 {
                    lin_lerp_1d(self, table, input, output)
                } else {
                    eval_1_input(self, table, input, output)
                }
            }
            2 => bilinear16(self, table, input, output),
            3 => {
                if self.trilinear {
                    trilinear16(self, table, input, output)
                } else {
                    tetrahedral16(self, table, input, output)
                }
            }
            n => eval_n_inputs(self, table, input, output, 0, n),
        }
    }

    pub fn eval_float(&self, table: &[f32], input: &[f32], output: &mut [f32]) {
        if self.n_inputs == 1 {
            if self.n_outputs == 1 {
                lin_lerp_1d_float(self, table, input, output);
            } else {
                eval_1_input_float(self, table, input, output);
            }
            return;
        }
        self.interpolate_float(table, input, 0, &mut output[..self.n_outputs]);
    }

    fn interpolate_float(&self, table: &[f32], input: &[f32], axis: usize, out: &mut [f32]) {
        let axes = self.n_inputs - axis;
        let cell = |i: usize| {
            let stride = self.opta[self.n_inputs - 1 - i] as usize;
            let v = fclamp(input[i]);
            let x = v * self.domain[i] as f32;
            (
                x as usize * stride,
                if v >= 1.0 { 0 } else { stride },
                x.fract(),
            )
        };
        let (x, dx, rx) = cell(axis);
        let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
        if axes > 3 {
            let mut high = [0.0; MAX_STAGE_CHANNELS];
            self.interpolate_float(&table[x..], input, axis + 1, out);
            self.interpolate_float(&table[x + dx..], input, axis + 1, &mut high[..out.len()]);
            for (a, b) in out.iter_mut().zip(high) {
                *a = mix(*a, b, rx);
            }
            return;
        }
        let (y, dy, ry) = cell(axis + 1);
        if axes == 2 {
            for (i, v) in out.iter_mut().enumerate() {
                let t = &table[x + y + i..];
                *v = mix(mix(t[0], t[dx], rx), mix(t[dy], t[dx + dy], rx), ry);
            }
            return;
        }
        let (z, dz, rz) = cell(axis + 2);
        let weights = [rx, ry, rz];
        let strides = [dx, dy, dz];
        let mut order = [0, 1, 2];
        order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]));
        for (i, v) in out.iter_mut().enumerate() {
            let t = &table[x + y + z + i..];
            let mut coefficients = [0.0; 3];
            let mut corner = 0;
            for axis in order {
                coefficients[axis] = t[corner + strides[axis]] - t[corner];
                corner += strides[axis];
            }
            *v = t[0] + coefficients[0] * rx + coefficients[1] * ry + coefficients[2] * rz;
        }
    }
}

fn linear_interp(a: i32, l: i32, h: i32) -> u16 {
    let dif = ((h.wrapping_sub(l)) as u32)
        .wrapping_mul(a as u32)
        .wrapping_add(0x8000);
    ((dif >> 16).wrapping_add(l as u32)) as u16
}

fn lerp(a: i32, l: i32, h: i32) -> i32 {
    let x = h.wrapping_sub(l).wrapping_mul(a);
    (l.wrapping_add(x.wrapping_add(0x8000) >> 16)) as u16 as i32
}

fn lin_lerp_1d(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16]) {
    let at = |i: i64| -> u16 {
        if i < 0 {
            return 0;
        }
        table.get(i as usize).copied().unwrap_or(0)
    };
    if input[0] == 0xffff || p.domain[0] == 0 {
        output[0] = at(p.domain[0] as i64);
    } else {
        let val3 = p.domain[0].wrapping_mul(input[0] as u32) as i32;
        let val3 = to_fixed_domain(val3);
        let cell0 = val3 >> 16;
        let rest = val3 & 0xFFFF;
        let y0 = at(cell0 as i64);
        let y1 = at(cell0 as i64 + 1);
        output[0] = linear_interp(rest, y0 as i32, y1 as i32);
    }
}

fn fclamp(v: f32) -> f32 {
    if v < 1.0e-9f32 || v.is_nan() {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

fn lin_lerp_1d_float(p: &InterpParams, table: &[f32], input: &[f32], output: &mut [f32]) {
    let at = |i: i64| -> f32 {
        if i < 0 {
            return 0.0;
        }
        table.get(i as usize).copied().unwrap_or(0.0)
    };
    let mut val2 = fclamp(input[0]);
    if val2 == 1.0 || p.domain[0] == 0 {
        output[0] = at(p.domain[0] as i64);
    } else {
        val2 *= p.domain[0] as f32;
        let cell0 = (val2 as f64).floor() as i32;
        let cell1 = (val2 as f64).ceil() as i32;
        let rest = val2 - cell0 as f32;
        let y0 = at(cell0 as i64);
        let y1 = at(cell1 as i64);
        output[0] = y0 + (y1 - y0) * rest;
    }
}

fn eval_1_input_float(p: &InterpParams, table: &[f32], input: &[f32], output: &mut [f32]) {
    let at = |i: i64| -> f32 {
        if i < 0 {
            return 0.0;
        }
        table.get(i as usize).copied().unwrap_or(0.0)
    };
    let mut val2 = fclamp(input[0]);
    if val2 == 1.0 || p.domain[0] == 0 {
        let start = p.domain[0].wrapping_mul(p.opta[0]) as i64;
        for o in 0..p.n_outputs {
            output[o] = at(start + o as i64);
        }
    } else {
        val2 *= p.domain[0] as f32;
        let cell0 = (val2 as f64).floor() as i32;
        let cell1 = (val2 as f64).ceil() as i32;
        let rest = val2 - cell0 as f32;
        let c0 = cell0.wrapping_mul(p.opta[0] as i32) as i64;
        let c1 = cell1.wrapping_mul(p.opta[0] as i32) as i64;
        for o in 0..p.n_outputs {
            let y0 = at(c0 + o as i64);
            let y1 = at(c1 + o as i64);
            output[o] = y0 + (y1 - y0) * rest;
        }
    }
}

fn eval_1_input(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16]) {
    if input[0] == 0xffff || p.domain[0] == 0 {
        let y0 = (p.domain[0] * p.opta[0]) as usize;
        for o in 0..p.n_outputs {
            output[o] = table[y0 + o];
        }
    } else {
        let v = (input[0] as i32).wrapping_mul(p.domain[0] as i32);
        let fk = to_fixed_domain(v);
        let k0 = fk >> 16;
        let rk = (fk & 0xFFFF) as u16 as i32;
        let k1 = k0 + i32::from(input[0] != 0xFFFF);
        let kk0 = (p.opta[0] as i32 * k0) as usize;
        let kk1 = (p.opta[0] as i32 * k1) as usize;
        for o in 0..p.n_outputs {
            output[o] = linear_interp(rk, table[kk0 + o] as i32, table[kk1 + o] as i32);
        }
    }
}

fn fixed(input: u16, domain: u32) -> (i32, i32) {
    let f = to_fixed_domain((input as i32).wrapping_mul(domain as i32));
    (f >> 16, f & 0xFFFF)
}

fn bilinear16(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16]) {
    let (x0, rx) = fixed(input[0], p.domain[0]);
    let (y0, ry) = fixed(input[1], p.domain[1]);
    let xx0 = p.opta[1] as i32 * x0;
    let xx1 = xx0
        + if input[0] == 0xFFFF {
            0
        } else {
            p.opta[1] as i32
        };
    let yy0 = p.opta[0] as i32 * y0;
    let yy1 = yy0
        + if input[1] == 0xFFFF {
            0
        } else {
            p.opta[0] as i32
        };
    for o in 0..p.n_outputs {
        let dens = |i: i32, j: i32| table[(i + j) as usize + o] as i32;
        let d00 = dens(xx0, yy0);
        let d01 = dens(xx0, yy1);
        let d10 = dens(xx1, yy0);
        let d11 = dens(xx1, yy1);
        let dx0 = lerp(rx, d00, d10);
        let dx1 = lerp(rx, d01, d11);
        output[o] = lerp(ry, dx0, dx1) as u16;
    }
}

fn trilinear16(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16]) {
    let (x0, rx) = fixed(input[0], p.domain[0]);
    let (y0, ry) = fixed(input[1], p.domain[1]);
    let (z0, rz) = fixed(input[2], p.domain[2]);
    let xx0 = p.opta[2] as i32 * x0;
    let xx1 = xx0
        + if input[0] == 0xFFFF {
            0
        } else {
            p.opta[2] as i32
        };
    let yy0 = p.opta[1] as i32 * y0;
    let yy1 = yy0
        + if input[1] == 0xFFFF {
            0
        } else {
            p.opta[1] as i32
        };
    let zz0 = p.opta[0] as i32 * z0;
    let zz1 = zz0
        + if input[2] == 0xFFFF {
            0
        } else {
            p.opta[0] as i32
        };
    for o in 0..p.n_outputs {
        let dens = |i: i32, j: i32, k: i32| table[(i + j + k) as usize + o] as i32;
        let d000 = dens(xx0, yy0, zz0);
        let d001 = dens(xx0, yy0, zz1);
        let d010 = dens(xx0, yy1, zz0);
        let d011 = dens(xx0, yy1, zz1);
        let d100 = dens(xx1, yy0, zz0);
        let d101 = dens(xx1, yy0, zz1);
        let d110 = dens(xx1, yy1, zz0);
        let d111 = dens(xx1, yy1, zz1);
        let dx00 = lerp(rx, d000, d100);
        let dx01 = lerp(rx, d001, d101);
        let dx10 = lerp(rx, d010, d110);
        let dx11 = lerp(rx, d011, d111);
        let dxy0 = lerp(ry, dx00, dx10);
        let dxy1 = lerp(ry, dx01, dx11);
        output[o] = lerp(rz, dxy0, dxy1) as u16;
    }
}

fn rest_out(c0: i32, c1: i32, c2: i32, c3: i32, rx: i32, ry: i32, rz: i32) -> u16 {
    let rest = c1
        .wrapping_mul(rx)
        .wrapping_add(c2.wrapping_mul(ry))
        .wrapping_add(c3.wrapping_mul(rz))
        .wrapping_add(0x8001);
    c0.wrapping_add(rest.wrapping_add(rest >> 16) >> 16) as u16
}

fn tetrahedral16(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16]) {
    let (x0, rx) = fixed(input[0], p.domain[0]);
    let (y0, ry) = fixed(input[1], p.domain[1]);
    let (z0, rz) = fixed(input[2], p.domain[2]);
    let xx0 = p.opta[2].wrapping_mul(x0 as u32) as usize;
    let mut xx1 = if input[0] == 0xFFFF {
        0
    } else {
        p.opta[2] as usize
    };
    let yy0 = p.opta[1].wrapping_mul(y0 as u32) as usize;
    let mut yy1 = if input[1] == 0xFFFF {
        0
    } else {
        p.opta[1] as usize
    };
    let zz0 = p.opta[0].wrapping_mul(z0 as u32) as usize;
    let mut zz1 = if input[2] == 0xFFFF {
        0
    } else {
        p.opta[0] as usize
    };
    let lut = &table[xx0 + yy0 + zz0..];
    let n = p.n_outputs;
    let fetch = |o: usize, x: usize, y: usize, z: usize| {
        (
            lut[o] as i32,
            lut[x + o] as i32,
            lut[y + o] as i32,
            lut[z + o] as i32,
        )
    };
    if rx >= ry {
        if ry >= rz {
            yy1 += xx1;
            zz1 += yy1;
            for o in 0..n {
                let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
                c3 -= c2;
                c2 -= c1;
                c1 -= c0;
                output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
            }
        } else if rz >= rx {
            xx1 += zz1;
            yy1 += xx1;
            for o in 0..n {
                let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
                c2 -= c1;
                c1 -= c3;
                c3 -= c0;
                output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
            }
        } else {
            zz1 += xx1;
            yy1 += zz1;
            for o in 0..n {
                let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
                c2 -= c3;
                c3 -= c1;
                c1 -= c0;
                output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
            }
        }
    } else if rx >= rz {
        xx1 += yy1;
        zz1 += xx1;
        for o in 0..n {
            let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
            c3 -= c1;
            c1 -= c2;
            c2 -= c0;
            output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
        }
    } else if ry >= rz {
        zz1 += yy1;
        xx1 += zz1;
        for o in 0..n {
            let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
            c1 -= c3;
            c3 -= c2;
            c2 -= c0;
            output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
        }
    } else {
        yy1 += zz1;
        xx1 += yy1;
        for o in 0..n {
            let (c0, mut c1, mut c2, mut c3) = fetch(o, xx1, yy1, zz1);
            c1 -= c2;
            c2 -= c3;
            c3 -= c0;
            output[o] = rest_out(c0, c1, c2, c3, rx, ry, rz);
        }
    }
}

/// The 3D tetrahedral kernel shared by Eval4Inputs and the 8-bit prelinearization.
#[allow(clippy::too_many_arguments)]
pub fn tetra_dens(
    lut: &[u16],
    o: usize,
    xx0: i32,
    xx1: i32,
    yy0: i32,
    yy1: i32,
    zz0: i32,
    zz1: i32,
    rx: i32,
    ry: i32,
    rz: i32,
) -> u16 {
    let dens = |i: i32, j: i32, k: i32| lut[(i + j + k) as usize + o] as i32;
    let c0 = dens(xx0, yy0, zz0);
    let (c1, c2, c3);
    if rx >= ry && ry >= rz {
        c1 = dens(xx1, yy0, zz0) - c0;
        c2 = dens(xx1, yy1, zz0) - dens(xx1, yy0, zz0);
        c3 = dens(xx1, yy1, zz1) - dens(xx1, yy1, zz0);
    } else if rx >= rz && rz >= ry {
        c1 = dens(xx1, yy0, zz0) - c0;
        c2 = dens(xx1, yy1, zz1) - dens(xx1, yy0, zz1);
        c3 = dens(xx1, yy0, zz1) - dens(xx1, yy0, zz0);
    } else if rz >= rx && rx >= ry {
        c1 = dens(xx1, yy0, zz1) - dens(xx0, yy0, zz1);
        c2 = dens(xx1, yy1, zz1) - dens(xx1, yy0, zz1);
        c3 = dens(xx0, yy0, zz1) - c0;
    } else if ry >= rx && rx >= rz {
        c1 = dens(xx1, yy1, zz0) - dens(xx0, yy1, zz0);
        c2 = dens(xx0, yy1, zz0) - c0;
        c3 = dens(xx1, yy1, zz1) - dens(xx1, yy1, zz0);
    } else if ry >= rz && rz >= rx {
        c1 = dens(xx1, yy1, zz1) - dens(xx0, yy1, zz1);
        c2 = dens(xx0, yy1, zz0) - c0;
        c3 = dens(xx0, yy1, zz1) - dens(xx0, yy1, zz0);
    } else if rz >= ry && ry >= rx {
        c1 = dens(xx1, yy1, zz1) - dens(xx0, yy1, zz1);
        c2 = dens(xx0, yy1, zz1) - dens(xx0, yy0, zz1);
        c3 = dens(xx0, yy0, zz1) - c0;
    } else {
        c1 = 0;
        c2 = 0;
        c3 = 0;
    }
    rest_out(c0, c1, c2, c3, rx, ry, rz)
}

fn eval4(p: &InterpParams, table: &[u16], input: &[u16], output: &mut [u16], d: usize) {
    let (k0, rk) = fixed(input[0], p.domain[d]);
    let (x0, rx) = fixed(input[1], p.domain[d + 1]);
    let (y0, ry) = fixed(input[2], p.domain[d + 2]);
    let (z0, rz) = fixed(input[3], p.domain[d + 3]);
    let kk0 = p.opta[3] as i32 * k0;
    let kk1 = kk0
        + if input[0] == 0xFFFF {
            0
        } else {
            p.opta[3] as i32
        };
    let xx0 = p.opta[2] as i32 * x0;
    let xx1 = xx0
        + if input[1] == 0xFFFF {
            0
        } else {
            p.opta[2] as i32
        };
    let yy0 = p.opta[1] as i32 * y0;
    let yy1 = yy0
        + if input[2] == 0xFFFF {
            0
        } else {
            p.opta[1] as i32
        };
    let zz0 = p.opta[0] as i32 * z0;
    let zz1 = zz0
        + if input[3] == 0xFFFF {
            0
        } else {
            p.opta[0] as i32
        };
    let mut tmp1 = [0u16; MAX_STAGE_CHANNELS];
    let mut tmp2 = [0u16; MAX_STAGE_CHANNELS];
    let lut1 = &table[kk0 as usize..];
    for (o, t) in tmp1.iter_mut().enumerate().take(p.n_outputs) {
        *t = tetra_dens(lut1, o, xx0, xx1, yy0, yy1, zz0, zz1, rx, ry, rz);
    }
    let lut2 = &table[kk1 as usize..];
    for (o, t) in tmp2.iter_mut().enumerate().take(p.n_outputs) {
        *t = tetra_dens(lut2, o, xx0, xx1, yy0, yy1, zz0, zz1, rx, ry, rz);
    }
    for i in 0..p.n_outputs {
        output[i] = linear_interp(rk, tmp1[i] as i32, tmp2[i] as i32);
    }
}

fn eval_n_inputs(
    p: &InterpParams,
    table: &[u16],
    input: &[u16],
    output: &mut [u16],
    d: usize,
    n: usize,
) {
    if n == 4 {
        return eval4(p, table, input, output, d);
    }
    let nm = n - 1;
    let fk = to_fixed_domain((input[0] as i32).wrapping_mul(p.domain[d] as i32));
    let k0 = fk >> 16;
    let rk = fk & 0xFFFF;
    let kk0 = (p.opta[nm] as i32 * k0) as usize;
    let kk1 = (p.opta[nm] as i32 * (k0 + i32::from(input[0] != 0xFFFF))) as usize;
    let mut tmp1 = [0u16; MAX_STAGE_CHANNELS];
    let mut tmp2 = [0u16; MAX_STAGE_CHANNELS];
    eval_n_inputs(p, &table[kk0..], &input[1..], &mut tmp1, d + 1, nm);
    eval_n_inputs(p, &table[kk1..], &input[1..], &mut tmp2, d + 1, nm);
    for i in 0..p.n_outputs {
        output[i] = linear_interp(rk, tmp1[i] as i32, tmp2[i] as i32);
    }
}
