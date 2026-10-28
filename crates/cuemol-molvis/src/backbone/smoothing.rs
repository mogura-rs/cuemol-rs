use super::{SplineSample, Vector4};

pub(super) struct SmoothSpline {
    nodes: Vec<f64>,
    coefficients: Vec<[Vector4; 4]>,
}

impl SmoothSpline {
    pub fn new(
        points: &[Vector4],
        weights: Option<&[f64]>,
        rho: f64,
        start: f64,
        min_basis: usize,
    ) -> Option<Self> {
        if points.len() < 2 {
            return None;
        }
        let solver = Solver::new(points.len(), weights, rho, min_basis)?;
        let fits: Vec<_> = (0..3)
            .map(|axis| {
                let values = points
                    .iter()
                    .map(|p| [p.x, p.y, p.z][axis])
                    .collect::<Vec<_>>();
                solver.solve(&values)
            })
            .collect();
        let nodes = fits[0].0.iter().map(|x| x + start).collect();
        let coefficients = (0..fits[0].1.len())
            .map(|i| {
                std::array::from_fn(|j| {
                    Vector4::new3(fits[0].1[i][j], fits[1].1[i][j], fits[2].1[i][j])
                })
            })
            .collect();
        Some(Self {
            nodes,
            coefficients,
        })
    }

    pub fn interpolate(&self, t: f64) -> SplineSample {
        let i = self
            .nodes
            .partition_point(|&x| x < t)
            .saturating_sub(1)
            .min(self.coefficients.len() - 1);
        let f = t - self.nodes[i];
        let [a, b, c, d] = self.coefficients[i];
        SplineSample {
            position: ((d.scale(f) + c).scale(f) + b).scale(f) + a,
            tangent: (d.scale(3.0 * f) + c.scale(2.0)).scale(f) + b,
            curvature: d.scale(6.0 * f) + c.scale(2.0),
        }
    }
}

enum Solver {
    Reinsch(Reinsch),
    Dense(Dense),
}

impl Solver {
    fn new(n: usize, weights: Option<&[f64]>, rho: f64, min_basis: usize) -> Option<Self> {
        if n >= min_basis {
            if let Some(solver) = Reinsch::new(n, weights, rho) {
                return Some(Self::Reinsch(solver));
            }
        }
        Dense::new(n, weights, rho, n.max(min_basis)).map(Self::Dense)
    }

    fn solve(&self, y: &[f64]) -> (Vec<f64>, Vec<[f64; 4]>) {
        match self {
            Self::Reinsch(solver) => solver.solve(y),
            Self::Dense(solver) => solver.solve(y),
        }
    }
}

fn penalty(rho: f64) -> f64 {
    let max = -f64::EPSILON.log10();
    10.0_f64.powf(rho.clamp(-max, max))
}

struct Reinsch {
    lw: Vec<f64>,
    d: Vec<f64>,
    l1: Vec<f64>,
    l2: Vec<f64>,
}

impl Reinsch {
    fn max_penalty(n: usize) -> f64 {
        let nr = n - 2;
        let mut d = vec![2.0 / 3.0; nr];
        let mut l = vec![0.0; nr];
        for j in 1..nr {
            l[j] = (1.0 / 6.0) / d[j - 1];
            d[j] -= l[j] / 6.0;
        }
        let mut s0 = vec![0.0; nr];
        let mut s1 = vec![0.0; nr];
        let mut s2 = vec![0.0; nr];
        s0[nr - 1] = 1.0 / d[nr - 1];
        for j in (0..nr - 1).rev() {
            s1[j] = -l[j + 1] * s0[j + 1];
            if j + 2 < nr {
                s2[j] = -l[j + 1] * s1[j + 1];
            }
            s0[j] = 1.0 / d[j] - l[j + 1] * s1[j];
        }
        let mut max: f64 = 0.0;
        for i in 0..n {
            let mut sum = 0.0;
            for a in 0..3 {
                let j = i as isize - 2 + a;
                if j < 0 || j as usize >= nr {
                    continue;
                }
                for b in 0..3 {
                    let k = i as isize - 2 + b;
                    if k < 0 || k as usize >= nr {
                        continue;
                    }
                    let low = j.min(k) as usize;
                    let value = match (j - k).abs() {
                        0 => s0[low],
                        1 => s1[low],
                        _ => s2[low],
                    };
                    sum += [1.0, -2.0, 1.0][a as usize] * [1.0, -2.0, 1.0][b as usize] * value;
                }
            }
            max = max.max(sum);
        }
        max
    }

    fn new(n: usize, weights: Option<&[f64]>, rho: f64) -> Option<Self> {
        if n < 3 || weights.is_some_and(|w| w.len() != n || w.contains(&0.0)) {
            return None;
        }
        let max = weights
            .map(|w| w.iter().map(|v| v * v).fold(0.0, f64::max))
            .unwrap_or(1.0);
        let lambda = penalty(rho) * max / Self::max_penalty(n);
        let lw: Vec<_> = (0..n)
            .map(|i| weights.map_or(lambda, |w| lambda / (w[i] * w[i])))
            .collect();
        let nr = n - 2;
        let mut d = vec![0.0; nr];
        let mut l1 = vec![0.0; nr + 1];
        let mut l2 = vec![0.0; nr + 2];
        for j in 0..nr {
            let mut diag = 2.0 / 3.0 + lw[j] + 4.0 * lw[j + 1] + lw[j + 2];
            if j >= 1 {
                diag -= l1[j] * l1[j] * d[j - 1];
            }
            if j >= 2 {
                diag -= l2[j] * l2[j] * d[j - 2];
            }
            if diag <= 0.0 || !diag.is_finite() {
                return None;
            }
            d[j] = diag;
            if j + 1 < nr {
                let mut off = 1.0 / 6.0 - 2.0 * lw[j + 1] - 2.0 * lw[j + 2];
                if j >= 1 {
                    off -= l2[j + 1] * l1[j] * d[j - 1];
                }
                l1[j + 1] = off / diag;
            }
            if j + 2 < nr {
                l2[j + 2] = lw[j + 2] / diag;
            }
        }
        Some(Self { lw, d, l1, l2 })
    }

    fn solve(&self, y: &[f64]) -> (Vec<f64>, Vec<[f64; 4]>) {
        let n = y.len();
        let nr = n - 2;
        let mut z = vec![0.0; nr];
        for j in 0..nr {
            let mut value = y[j] - 2.0 * y[j + 1] + y[j + 2];
            if j >= 1 {
                value -= self.l1[j] * z[j - 1];
            }
            if j >= 2 {
                value -= self.l2[j] * z[j - 2];
            }
            z[j] = value;
        }
        let mut gamma = vec![0.0; n];
        for j in (0..nr).rev() {
            let mut value = z[j] / self.d[j];
            if j + 1 < nr {
                value -= self.l1[j + 1] * gamma[j + 2];
            }
            if j + 2 < nr {
                value -= self.l2[j + 2] * gamma[j + 3];
            }
            gamma[j + 1] = value;
        }
        let g: Vec<_> = (0..n)
            .map(|i| {
                let mut qg = -2.0 * gamma[i];
                if i >= 1 {
                    qg += gamma[i - 1];
                }
                if i + 1 < n {
                    qg += gamma[i + 1];
                }
                y[i] - self.lw[i] * qg
            })
            .collect();
        coefficients(&(0..n).map(|i| i as f64).collect::<Vec<_>>(), &g, &gamma)
    }
}

fn second_derivatives(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut d2 = vec![0.0; n];
    if n < 3 {
        return d2;
    }
    let mut cp = vec![0.0; n];
    let mut rp = vec![0.0; n];
    for j in 1..n - 1 {
        let h0 = x[j] - x[j - 1];
        let h1 = x[j + 1] - x[j];
        let a = h0 / 6.0;
        let den = (h0 + h1) / 3.0 - a * cp[j - 1];
        let right = (y[j + 1] - y[j]) / h1 - (y[j] - y[j - 1]) / h0;
        cp[j] = h1 / 6.0 / den;
        rp[j] = (right - a * rp[j - 1]) / den;
    }
    for j in (1..n - 1).rev() {
        d2[j] = rp[j] - cp[j] * d2[j + 1];
    }
    d2
}

fn spline_value(x: &[f64], y: &[f64], d2: &[f64], u: f64) -> f64 {
    let k = x
        .partition_point(|&x| x < u)
        .saturating_sub(1)
        .min(x.len() - 2);
    let h = x[k + 1] - x[k];
    let a = (x[k + 1] - u) / h;
    let b = 1.0 - a;
    a * y[k] + b * y[k + 1] + ((a * a * a - a) * d2[k] + (b * b * b - b) * d2[k + 1]) * h * h / 6.0
}

fn coefficients(x: &[f64], y: &[f64], d2: &[f64]) -> (Vec<f64>, Vec<[f64; 4]>) {
    let n = x.len();
    let mut c = vec![[0.0; 4]; n + 1];
    for j in 0..n - 1 {
        let h = x[j + 1] - x[j];
        c[j + 1] = [
            y[j],
            (y[j + 1] - y[j]) / h - h * (2.0 * d2[j] + d2[j + 1]) / 6.0,
            0.5 * d2[j],
            (d2[j + 1] - d2[j]) / (6.0 * h),
        ];
    }
    let h0 = x[1] - x[0];
    let h1 = x[n - 1] - x[n - 2];
    let d0 = c[1][1];
    let dn = (y[n - 1] - y[n - 2]) / h1 + h1 * (d2[n - 2] + 2.0 * d2[n - 1]) / 6.0;
    c[0] = [y[0] - d0 * h0, d0, 0.0, 0.0];
    c[n] = [y[n - 1], dn, 0.0, 0.0];
    let mut nodes = Vec::with_capacity(n + 2);
    nodes.push(x[0] - h0);
    nodes.extend_from_slice(x);
    nodes.push(x[n - 1] + h1);
    (nodes, c)
}

struct Dense {
    n: usize,
    m: usize,
    xa: f64,
    xb: f64,
    knots: Vec<f64>,
    w2: Vec<f64>,
    basis: Vec<f64>,
    chol: Vec<f64>,
}

impl Dense {
    fn new(n: usize, weights: Option<&[f64]>, rho: f64, m: usize) -> Option<Self> {
        let xa = 2.0 / (n - 1) as f64;
        let xb = -1.0;
        let max = weights
            .map(|w| w.iter().map(|w| w.abs()).fold(0.0, f64::max))
            .unwrap_or(1.0)
            .max(f64::MIN_POSITIVE);
        let w2: Vec<_> = (0..n)
            .map(|i| (weights.map_or(1.0, |w| w[i]) / max).powi(2))
            .collect();
        let knots: Vec<_> = (0..m)
            .map(|j| 2.0 * j as f64 / (m - 1) as f64 - 1.0)
            .collect();
        let mut basis = vec![0.0; n * m];
        let mut d2 = vec![vec![0.0; m]; m];
        let mut fdmax: f64 = 0.0;
        for b in 0..m {
            let mut y = vec![0.0; m];
            y[b] = 1.0;
            d2[b] = second_derivatives(&knots, &y);
            let mut sum = 0.0;
            for i in 0..n {
                let x = 2.0 * (i as f64 - 0.5 * (n - 1) as f64) / (n - 1) as f64;
                let f = spline_value(&knots, &y, &d2[b], x);
                basis[i * m + b] = f;
                sum += w2[i] * f * f;
            }
            fdmax = fdmax.max(sum);
        }
        let mut amat = vec![0.0; m * m];
        let mut admax: f64 = 0.0;
        for i in 0..m {
            for j in i..m {
                let mut sum = 0.0;
                for k in 0..m - 1 {
                    let (fa, fb, ga, gb) = (d2[i][k], d2[i][k + 1], d2[j][k], d2[j][k + 1]);
                    sum += (knots[k + 1] - knots[k])
                        * (fa * ga
                            + (fa * (gb - ga) + ga * (fb - fa)) / 2.0
                            + (fb - fa) * (gb - ga) / 3.0);
                }
                amat[i * m + j] = sum;
                amat[j * m + i] = sum;
            }
            admax = admax.max(amat[i * m + i].abs());
        }
        let pdecay = penalty(rho) * fdmax / admax;
        let tdecay = fdmax * (1.0 + pdecay) * 10.0 * f64::EPSILON;
        let mut chol = vec![0.0; m * m];
        for a in 0..m {
            for b in 0..=a {
                let mut v = pdecay * amat[a * m + b];
                for i in 0..n {
                    v += w2[i] * basis[i * m + a] * basis[i * m + b];
                }
                if a == b {
                    v += tdecay;
                }
                chol[a * m + b] = v;
            }
        }
        for j in 0..m {
            let mut diag = chol[j * m + j];
            for k in 0..j {
                diag -= chol[j * m + k].powi(2);
            }
            if diag <= 0.0 || !diag.is_finite() {
                return None;
            }
            chol[j * m + j] = diag.sqrt();
            for i in j + 1..m {
                let mut value = chol[i * m + j];
                for k in 0..j {
                    value -= chol[i * m + k] * chol[j * m + k];
                }
                chol[i * m + j] = value / chol[j * m + j];
            }
        }
        Some(Self {
            n,
            m,
            xa,
            xb,
            knots,
            w2,
            basis,
            chol,
        })
    }

    fn solve(&self, y: &[f64]) -> (Vec<f64>, Vec<[f64; 4]>) {
        let (n, m) = (self.n, self.m);
        let mean = y.iter().sum::<f64>() / n as f64;
        let mut upper =
            (y.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / n as f64).sqrt() + mean;
        if upper == mean {
            upper = 2.0 * mean;
        }
        if upper == mean {
            upper = mean + 1.0;
        }
        let sd = upper - mean;
        let mut c = vec![0.0; m];
        for i in 0..n {
            let value = self.w2[i] * (y[i] - mean) / sd;
            for b in 0..m {
                c[b] += value * self.basis[i * m + b];
            }
        }
        for i in 0..m {
            for k in 0..i {
                c[i] -= self.chol[i * m + k] * c[k];
            }
            c[i] /= self.chol[i * m + i];
        }
        for i in (0..m).rev() {
            for k in i + 1..m {
                c[i] -= self.chol[k * m + i] * c[k];
            }
            c[i] /= self.chol[i * m + i];
        }
        let d2 = second_derivatives(&self.knots, &c);
        let (mut nodes, mut coeffs) = coefficients(&self.knots, &c, &d2);
        for node in &mut nodes {
            *node = (*node - self.xb) / self.xa;
        }
        for coeff in &mut coeffs {
            let mut scale = sd;
            for value in coeff.iter_mut() {
                *value *= scale;
                scale *= self.xa;
            }
            coeff[0] += mean;
        }
        (nodes, coeffs)
    }
}
