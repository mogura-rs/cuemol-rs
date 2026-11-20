use std::ops::{Add, Div, Mul, Neg, Sub};

pub(crate) const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    #[inline]
    fn mul(self, a: Vec3) -> Vec3 {
        Vec3::new(a.x * self, a.y * self, a.z * self)
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn div(self, s: f64) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}

/// Multiplication by -1.0, as written in the original source.
impl Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        self * -1.0
    }
}

#[inline]
pub(crate) fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

#[inline]
pub(crate) fn norm(v: Vec3) -> f64 {
    dot(v, v).sqrt()
}

#[inline]
pub(crate) fn unit(v: Vec3) -> Vec3 {
    v / norm(v)
}

#[inline]
pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

#[inline]
pub(crate) fn triple(u: Vec3, v: Vec3, n: Vec3) -> f64 {
    dot(u, cross(v, n))
}

/// numpy sign: 0 for zero and NaN.
#[inline]
pub(crate) fn sign(x: f64) -> f64 {
    ((x > 0.0) as i32 - (x < 0.0) as i32) as f64
}

/// Python scalar `x ** 2`; the reference build folds `pow(x, 2.0)` to `x * x`.
#[inline]
pub(crate) fn pysq(x: f64) -> f64 {
    x * x
}

/// `static_cast<int>(x)` as compiled for x86-64: NaN and out-of-range values give `i32::MIN`.
#[inline]
pub(crate) fn cvt_i32(x: f64) -> i32 {
    if x > -2_147_483_649.0 && x < 2_147_483_648.0 {
        x as i32
    } else {
        i32::MIN
    }
}

/// `static_cast<int64_t>(x)` as compiled for x86-64: NaN and out-of-range values give `i64::MIN`.
#[inline]
pub(crate) fn cvt_i64(x: f64) -> i64 {
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
        x as i64
    } else {
        i64::MIN
    }
}

/// `std::max(a, b)`.
#[inline]
pub(crate) fn cmax(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

/// `std::min(a, b)`.
#[inline]
pub(crate) fn cmin(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

#[inline]
pub(crate) fn acos_clamped(x: f64) -> f64 {
    x.clamp(-1.0, 1.0).acos()
}

#[inline]
pub(crate) fn arc_angle(u: Vec3, v: Vec3, n: Vec3, direct: i32) -> f64 {
    let t = sign(triple(u, v, n));
    let base = acos_clamped(dot(u, v) / (norm(u) * norm(v)));
    if direct as f64 * t > 0.0 {
        base
    } else {
        TWO_PI - base
    }
}

pub(crate) fn orthogonalvectors(n: Vec3) -> (Vec3, Vec3) {
    let (a, b, c) = (n.x, n.y, n.z);
    let (x, y, z);
    if a != 0.0 && b != 0.0 {
        x = -sign(a) * b;
        y = sign(a) * a;
        z = 0.0;
    } else if b != 0.0 && c != 0.0 {
        x = 0.0;
        y = -sign(b) * c;
        z = sign(b) * b;
    } else if c != 0.0 && a != 0.0 {
        x = sign(c) * c;
        y = 0.0;
        z = -sign(c) * a;
    } else if a != 0.0 {
        x = 0.0;
        y = 1.0;
        z = 0.0;
    } else if b != 0.0 {
        x = 0.0;
        y = 0.0;
        z = 1.0;
    } else {
        x = 1.0;
        y = 0.0;
        z = 0.0;
    }
    let v1 = unit(Vec3::new(x, y, z));
    let v2 = unit(Vec3::new(b * z - c * y, c * x - a * z, a * y - b * x));
    (v1, v2)
}

#[inline]
pub(crate) fn circlecenter(c1: Vec3, c2: Vec3, r1: f64, r2: f64) -> Vec3 {
    let d = norm(c1 - c2);
    let t = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    c1 + (c2 - c1) * t / d
}

/// Stable ascending argsort of `key[1..=n]` with `<` (1-based result, `[0]` dummy).
pub(crate) fn stable_order(key: &[f64], n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::with_capacity(n + 1);
    order.push(0);
    for i in 1..=n {
        let mut pos = order.len();
        while pos > 1 && key[i] < key[order[pos - 1]] {
            pos -= 1;
        }
        order.insert(pos, i);
    }
    order
}
