use super::params::{Kind, Layer};

pub fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}
fn combine(h: u32, v: u32) -> u32 {
    hash(
        h ^ v
            .wrapping_add(0x9e3779b9)
            .wrapping_add(h << 6)
            .wrapping_add(h >> 2),
    )
}
fn seed(s: u32, l: u32, i: i32, j: i32, stream: u32) -> u32 {
    let h = hash(s ^ 0x9e3779b9);
    combine(combine(combine(combine(h, l), i as u32), j as u32), stream)
}
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1. / 16777216.)
}
fn signed(h: u32) -> f32 {
    unit(h) * 2. - 1.
}
fn noise1(s: u32, x: f32) -> f32 {
    let xf = x.floor();
    let i = xf as i32;
    let f = x - xf;
    let a = signed(hash(combine(s, i as u32)));
    let b = signed(hash(combine(s, i.wrapping_add(1) as u32)));
    a + (b - a) * f * f * (3. - 2. * f)
}
fn noise2(s: u32, x: f32, y: f32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (i, j) = (xf as i32, yf as i32);
    let (fx, fy) = (x - xf, y - yf);
    let c = |i: i32, j: i32| signed(hash(combine(combine(s, i as u32), j as u32)));
    let (sx, sy) = (fx * fx * (3. - 2. * fx), fy * fy * (3. - 2. * fy));
    let a = c(i, j);
    let b = c(i.wrapping_add(1), j);
    let cc = c(i, j.wrapping_add(1));
    let d = c(i.wrapping_add(1), j.wrapping_add(1));
    let ab = a + (b - a) * sx;
    ab + (cc + (d - cc) * sx - ab) * sy
}
fn level(j: i32, k: i32) -> i32 {
    if j == 0 || k <= 0 {
        0
    } else {
        (k - j.unsigned_abs().trailing_zeros() as i32).max(0)
    }
}
fn gamma(z: f64) -> f64 {
    let c = [
        676.5203681218851,
        -1259.1392167224028,
        771.3234287776531,
        -176.6150291621406,
        12.507343278686905,
        -0.13857109526572012,
        9.984369578019572e-6,
        1.5056327351493116e-7,
    ];
    let z = z - 1.;
    let mut x = 0.9999999999998099;
    for (i, v) in c.into_iter().enumerate() {
        x += v / (z + i as f64 + 1.);
    }
    let t = z + 7.5;
    (2. * std::f64::consts::PI).sqrt() * t.powf(z + 0.5) * (-t).exp() * x
}

#[derive(Debug, Clone)]
pub struct NormalizedLayer {
    pub layer: Layer,
    pub id: u32,
    cos: f32,
    sin: f32,
    step: f32,
    k: i32,
    half_width: f32,
    dot_scale: f32,
    r_fixed: f32,
    half_aa: f32,
    tone_hi: f32,
    tone_lo: f32,
    fade: f32,
    wobble: f32,
    wobwave: f32,
    wjitter: f32,
    slen: f32,
    sgap: f32,
    taper: f32,
    angle_tan: f32,
    lenjitter: f32,
    tooth: f32,
    toothscale: f32,
    p_exp: f32,
    area: f32,
    sx: f32,
    sy: f32,
    dot_cos: f32,
    dot_sin: f32,
    jitter: f32,
    r_max: f32,
    r_tab: [f32; 65],
    pad_line: f32,
    pad_dot: f32,
}
impl NormalizedLayer {
    pub fn new(l: &Layer, id: u32) -> Self {
        let spacing = l.spacing.max(2.);
        let kmax = ((spacing / 2.).log2().floor() as i32).clamp(0, 20);
        let k = if l.kind == Kind::Stipple {
            0
        } else {
            l.subdiv.clamp(0, kmax)
        };
        let step = spacing / (1u32 << k) as f32;
        let tone_hi = l.tonehi.clamp(0., 1.);
        let tone_lo = l.tonelo.clamp(0., tone_hi);
        let fade = if l.fade > 0. {
            l.fade
        } else if l.kind == Kind::Stipple {
            32.
        } else {
            1. / ((tone_hi - tone_lo) / k.max(1) as f32).max(1e-3)
        };
        let p_exp = l.shape.clamp(1., 64.);
        let area = if p_exp >= 16. {
            4.
        } else {
            let g1 = gamma((1. + 1. / p_exp) as f64) as f32;
            let g2 = gamma((1. + 2. / p_exp) as f64) as f32;
            (2. * g1) * (2. * g1) / g2
        };
        let sx = l.aspect.clamp(0.5, 2.).sqrt();
        let sy = 1. / sx;
        let inflate = sx.max(sy);
        let jitter = l.jitter.clamp(0., 0.5);
        let half_aa = l.soft.clamp(0.5, 2.);
        let half_width = 0.5 * l.width.max(0.).min(spacing.max(2. * step));
        let dot_scale = l.dotscale.clamp(0., 4.);
        let angle_tan = l.anglejitter.clamp(0., 15.).to_radians().tan();
        let slen = if l.slen > 0. { l.slen.max(2.) } else { 0. };
        let wjitter = l.wjitter.clamp(0., 1.);
        let lenjitter = l.lenjitter.clamp(0., 0.9);
        let r_max = if l.kind == Kind::Dot && l.invert {
            2f32.powf(1. / p_exp) * (0.5 + jitter) * inflate * step + half_aa
        } else {
            step / area.sqrt()
        };
        let r_fixed = dot_scale * step / area.sqrt();
        let wobble = l.wobble.clamp(0., step);
        let mut r = Self {
            layer: l.clone(),
            id,
            cos: l.angle.to_radians().cos(),
            sin: l.angle.to_radians().sin(),
            step,
            k,
            half_width,
            dot_scale,
            r_fixed,
            half_aa,
            tone_hi,
            tone_lo,
            fade,
            wobble,
            wobwave: l.wobwave.max(2.),
            wjitter,
            slen,
            sgap: l.sgap.max(0.),
            taper: l.taper.clamp(0., 0.9),
            angle_tan,
            lenjitter,
            tooth: l.tooth.clamp(0., 1.),
            toothscale: l.toothscale.max(0.5),
            p_exp,
            area,
            sx,
            sy,
            dot_cos: l.dotangle.to_radians().cos(),
            dot_sin: l.dotangle.to_radians().sin(),
            jitter: jitter * step,
            r_max,
            r_tab: [0.; 65],
            pad_line: half_width * (1. + wjitter) * (1. + 0.6 * wjitter)
                + wobble
                + jitter * step
                + angle_tan * 0.5 * slen * (1. + 0.5 * lenjitter)
                + half_aa,
            pad_dot: jitter * step
                + 2f32.powf((0.5 - 1. / p_exp).max(0.))
                    * inflate
                    * if l.kind == Kind::Stipple {
                        r_fixed
                    } else {
                        r_max
                    }
                + half_aa,
        };
        if l.kind == Kind::Dot {
            let mut coverage = [0f32; 65];
            for (q, cov) in coverage.iter_mut().enumerate() {
                let rad = r.r_max * q as f32 / 64.;
                let mut acc = 0f64;
                for y in 0..16 {
                    for x in 0..16 {
                        let px = (x as f32 + 0.5) / 16. * r.step;
                        let py = (y as f32 + 0.5) / 16. * r.step;
                        let mut c = 0f32;
                        for j in -1..=2 {
                            for i in -1..=2 {
                                c = c.max(r.dot_mark(
                                    rad,
                                    r.lp(px - i as f32 * r.step, py - j as f32 * r.step),
                                ));
                            }
                        }
                        acc += c as f64;
                    }
                }
                *cov = (acc / 256.) as f32;
            }
            let mut q = 0;
            for k in 0..=64 {
                let target = k as f32 / 64.;
                while q < 64 && coverage[q] < target {
                    q += 1;
                }
                r.r_tab[k] = if q == 0 {
                    0.
                } else if coverage[q] < target {
                    r.r_max
                } else {
                    let c0 = coverage[q - 1];
                    let c1 = coverage[q];
                    let f = if c1 > c0 {
                        (target - c0) / (c1 - c0)
                    } else {
                        1.
                    };
                    r.r_max * ((q - 1) as f32 + f) / 64.
                };
            }
        }
        r
    }
    fn seed(&self, i: i32, j: i32, stream: u32) -> u32 {
        seed(self.layer.seed, self.id, i, j, stream)
    }
    fn threshold(&self, lv: i32) -> f32 {
        if self.k <= 0 {
            self.tone_hi
        } else {
            self.tone_hi + (self.tone_lo - self.tone_hi) * lv as f32 / self.k as f32
        }
    }
    fn lp(&self, dx: f32, dy: f32) -> f32 {
        let x = ((self.dot_cos * dx + self.dot_sin * dy) / self.sx).abs();
        let y = ((-self.dot_sin * dx + self.dot_cos * dy) / self.sy).abs();
        if self.p_exp >= 16. {
            x.max(y)
        } else {
            (x.powf(self.p_exp) + y.powf(self.p_exp)).powf(1. / self.p_exp)
        }
    }
    fn dot_mark(&self, r: f32, d: f32) -> f32 {
        if r <= 0. {
            0.
        } else {
            (self.area * r * r / (4. * self.half_aa * self.half_aa)).min(1.)
                * (1. - smooth(r - self.half_aa, r + self.half_aa, d))
        }
    }
    fn paper_tooth(&self, x: f32, y: f32) -> f32 {
        if self.tooth <= 0. {
            1.
        } else {
            1. - self.tooth
                * (0.5 + 0.5 * noise2(self.seed(0, 0, 6), x / self.toothscale, y / self.toothscale))
        }
    }
    fn line(&self, x: f32, y: f32, tone: f32) -> f32 {
        let u = -self.sin * x + self.cos * y;
        let v = self.cos * x + self.sin * y;
        let h = self.half_aa;
        let j0 = ((u - self.pad_line) / self.step).floor() as i32;
        let j1 = ((u + self.pad_line) / self.step).floor() as i32;
        let mut cov = 0f32;
        for j in j0..=j1 {
            let t = self.threshold(level(j, self.k));
            if tone >= t {
                continue;
            }
            let mut w = self.half_width * ((t - tone) * self.fade).min(1.);
            let mut scale = 1.;
            let mut c0 = j as f32 * self.step;
            if self.jitter > 0. {
                c0 += (unit(self.seed(0, j, 1)) - 0.5) * 2. * self.jitter;
            }
            if self.wobble > 0. {
                c0 += self.wobble * noise1(self.seed(0, j, 3), v / self.wobwave);
            }
            if self.wjitter > 0. {
                w *= 1. + self.wjitter * noise1(self.seed(0, j, 4), v / self.wobwave);
            }
            if self.slen > 0. {
                let period = self.slen + self.sgap;
                let vp = v + period * unit(self.seed(0, j, 5));
                let slot = (vp / period).floor();
                let k = slot as i32;
                let ph = vp - slot * period;
                let mut len = self.slen;
                if self.lenjitter > 0. {
                    len = (len * (1. + self.lenjitter * (unit(self.seed(j, k, 9)) - 0.5)))
                        .max(2.)
                        .min(period);
                }
                if ph >= len {
                    continue;
                }
                if self.wjitter > 0. {
                    let pressure = 1. - self.wjitter * unit(self.seed(j, k, 10));
                    w *= pressure;
                    scale = 0.6 + 0.4 * pressure;
                }
                if self.angle_tan > 0. {
                    let vc = v + ((slot + 0.5) * period - vp);
                    let field_scale = (2.5 * self.slen).max(60.);
                    let field = noise2(
                        self.seed(0, 0, 12),
                        j as f32 * self.step / field_scale,
                        vc / field_scale,
                    );
                    let amt = (1.2 * field + 0.45 * signed(self.seed(j, k, 11))).clamp(-1., 1.);
                    c0 += self.angle_tan * amt * (ph - 0.5 * len);
                }
                let s = ph / len;
                let ht = self.seed(j, k, 13);
                let mut a0 = (self.taper * (0.35 + 0.35 * unit(ht))).clamp(0.02, 0.45);
                let mut a1 = (self.taper * (0.9 + 1.1 * unit(hash(ht)))).clamp(0.05, 0.60);
                if ht & 1 != 0 {
                    std::mem::swap(&mut a0, &mut a1);
                }
                let head = (s / a0).min(1.);
                let tail = ((1. - s) / a1).min(1.);
                w *= head * (2. - head) * tail * (2. - tail);
                if self.wjitter > 0. {
                    w *= 1.
                        + 0.6
                            * self.wjitter
                            * noise1(self.seed(j, k, 14), ph / (len * 0.4).max(8.));
                }
            }
            if w <= 0. {
                continue;
            }
            let d = (u - c0).abs();
            let c = (((d + h).min(w) - (d - h).max(-w)) / (2. * h)).clamp(0., 1.) * scale;
            cov = cov.max(c);
        }
        cov * self.paper_tooth(x, y)
    }
    fn radius(&self, coverage: f32) -> f32 {
        let x = coverage.clamp(0., 1.) * 64.;
        let k = x as usize;
        if k >= 64 {
            self.r_tab[64]
        } else {
            self.r_tab[k] + (self.r_tab[k + 1] - self.r_tab[k]) * (x - k as f32)
        }
    }
    fn dots(&self, x: f32, y: f32, tone: f32, stipple: bool) -> f32 {
        let ux = self.cos * x + self.sin * y;
        let uy = -self.sin * x + self.cos * y;
        let effective = if self.dot_scale == 1. {
            tone
        } else {
            (1. - self.dot_scale * self.dot_scale * (1. - tone)).clamp(0., 1.)
        };
        let base = self.radius(1. - effective);
        let mut cov = 0f32;
        let i0 = ((ux - self.pad_dot) / self.step).floor() as i32;
        let i1 = ((ux + self.pad_dot) / self.step).floor() as i32;
        let j0 = ((uy - self.pad_dot) / self.step).floor() as i32;
        let j1 = ((uy + self.pad_dot) / self.step).floor() as i32;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let t = if stipple {
                    self.tone_lo + (self.tone_hi - self.tone_lo) * unit(self.seed(i, j, 15))
                } else {
                    self.threshold(level(i, self.k).max(level(j, self.k)))
                };
                if tone >= t {
                    continue;
                }
                let r =
                    if stipple { self.r_fixed } else { base } * ((t - tone) * self.fade).min(1.);
                let mut cx = i as f32 * self.step;
                let mut cy = j as f32 * self.step;
                if self.jitter > 0. {
                    cx += (unit(self.seed(i, j, 1)) - 0.5) * 2. * self.jitter;
                    cy += (unit(self.seed(i, j, 2)) - 0.5) * 2. * self.jitter;
                }
                let c = self.dot_mark(r, self.lp(ux - cx, uy - cy));
                if stipple || self.jitter > 0. {
                    cov = 1. - (1. - cov) * (1. - c);
                } else {
                    cov = cov.max(c);
                }
            }
        }
        cov * self.paper_tooth(x, y)
    }
    pub fn coverage(&self, x: f32, y: f32, tone: f32) -> f32 {
        match self.layer.kind {
            Kind::Line => self.line(x, y, tone),
            Kind::Dot => self.dots(x, y, tone, false),
            Kind::Stipple => self.dots(x, y, tone, true),
        }
    }
}
