//! FFT grid sizing for electron-density map synthesis. Port of the
//! grid-selection logic in C++ `xtal::MapFFT` (`../cuemol2/src/modules/xtal/MapFFT.cpp`).
//!
//! Crystallographic FFTs run fastest on grid sizes that factor into small
//! primes; `calc_prime` rounds a target size up to the nearest such number.
//! The full structure-factor → density inverse FFT builds on this (see the
//! `cuemol-xtal` MTZ→density work).

use cuemol_core::{CueMolError, Matrix4, Result, Vector4};
use cuemol_molstr::GridMap;
use rustfft::{num_complex::Complex, FftPlanner};

use crate::map_renderers::DensityMap;

/// Largest prime factor allowed in an FFT grid dimension (C++ `ftprim`).
const FT_PRIM: i32 = 5;

/// A measured/calculated structure factor: Miller indices, amplitude, phase (deg).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reflection {
    pub h: i32,
    pub k: i32,
    pub l: i32,
    pub f: f32,
    pub phi_deg: f32,
}

/// Stateful structure-factor to density-map transformer.
///
/// This is the Rust counterpart of C++ `xtal::MapFFT`. It accepts either
/// typed reflections or the original row-major float buffer, applies the space
/// group's symmetry operations, selects a suitable FFT grid, and writes the
/// result into an optional owned target map.
#[derive(Debug, Clone, PartialEq)]
pub struct MapFFT {
    cell: [f64; 6],
    space_group: i32,
    grid_size: f64,
    map_resolution: f64,
    reflections: Vec<Reflection>,
    use_phases: bool,
    target: Option<DensityMap>,
    effective_resolution: Option<f64>,
    grid_dimensions: Option<[usize; 3]>,
}

impl Default for MapFFT {
    fn default() -> Self {
        Self {
            cell: [0.0; 6],
            space_group: -1,
            grid_size: 1.0 / 3.0,
            map_resolution: -1.0,
            reflections: Vec::new(),
            use_phases: true,
            target: None,
            effective_resolution: None,
            grid_dimensions: None,
        }
    }
}

impl MapFFT {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_params(
        &mut self,
        cell_a: f64,
        cell_b: f64,
        cell_c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        space_group: i32,
        grid_size: f64,
        map_resolution: f64,
    ) {
        self.cell = [cell_a, cell_b, cell_c, alpha, beta, gamma];
        self.space_group = space_group;
        self.grid_size = grid_size;
        self.map_resolution = map_resolution;
        self.effective_resolution = None;
        self.grid_dimensions = None;
    }

    pub fn cell(&self) -> [f64; 6] {
        self.cell
    }

    pub fn space_group(&self) -> i32 {
        self.space_group
    }

    pub fn grid_size(&self) -> f64 {
        self.grid_size
    }

    pub fn map_resolution(&self) -> f64 {
        self.map_resolution
    }

    pub fn effective_resolution(&self) -> Option<f64> {
        self.effective_resolution
    }

    pub fn grid_dimensions(&self) -> Option<[usize; 3]> {
        self.grid_dimensions
    }

    pub fn reflections(&self) -> &[Reflection] {
        &self.reflections
    }

    pub fn uses_phases(&self) -> bool {
        self.use_phases
    }

    pub fn set_reflections(&mut self, reflections: &[Reflection], use_phases: bool) {
        self.reflections = reflections.to_vec();
        self.use_phases = use_phases;
        self.effective_resolution = None;
        self.grid_dimensions = None;
    }

    /// Import C++ `MapFFT::setData`'s row-major float buffer.
    ///
    /// `phase_index` and `weight_index` use `-1` to indicate an absent column.
    #[allow(clippy::too_many_arguments)]
    pub fn set_data(
        &mut self,
        reflection_count: usize,
        row_stride: usize,
        buffer: &[f32],
        h_index: usize,
        k_index: usize,
        l_index: usize,
        amplitude_index: usize,
        phase_index: isize,
        weight_index: isize,
    ) -> Result<()> {
        if row_stride == 0 {
            return Err(CueMolError::invalid_argument(
                "MapFFT row stride must be positive",
            ));
        }
        let phase_index = usize::try_from(phase_index).ok();
        let weight_index = usize::try_from(weight_index).ok();
        let mut required = h_index.max(k_index).max(l_index).max(amplitude_index);
        if let Some(index) = phase_index {
            required = required.max(index);
        }
        if let Some(index) = weight_index {
            required = required.max(index);
        }
        if required >= row_stride {
            return Err(CueMolError::invalid_argument(
                "MapFFT column index is outside the row stride",
            ));
        }
        let required_len = reflection_count
            .checked_mul(row_stride)
            .ok_or_else(|| CueMolError::invalid_argument("MapFFT buffer size overflow"))?;
        if buffer.len() < required_len {
            return Err(CueMolError::invalid_argument(
                "MapFFT reflection buffer is too short",
            ));
        }

        self.reflections.clear();
        self.reflections.reserve(reflection_count);
        self.use_phases = phase_index.is_some();
        for row in buffer[..required_len].chunks_exact(row_stride) {
            let weight = weight_index.map_or(1.0, |index| row[index]);
            let amplitude = row[amplitude_index] * weight;
            self.reflections.push(Reflection {
                h: row[h_index] as i32,
                k: row[k_index] as i32,
                l: row[l_index] as i32,
                f: if amplitude.is_finite() {
                    amplitude
                } else {
                    0.0
                },
                phi_deg: phase_index
                    .map(|index| row[index])
                    .filter(|value| value.is_finite())
                    .unwrap_or(0.0),
            });
        }
        self.effective_resolution = None;
        self.grid_dimensions = None;
        Ok(())
    }

    /// Set an owned output object, matching the lifetime-independent part of
    /// C++ `setTarget`.
    pub fn set_target(&mut self, target: DensityMap) {
        self.target = Some(target);
    }

    pub fn target(&self) -> Option<&DensityMap> {
        self.target.as_ref()
    }

    pub fn target_mut(&mut self) -> Option<&mut DensityMap> {
        self.target.as_mut()
    }

    pub fn take_target(&mut self) -> Option<DensityMap> {
        self.target.take()
    }

    /// Run the transform and replace the previously supplied target.
    pub fn do_fft(&mut self) -> Result<()> {
        if self.target.is_none() {
            return Err(CueMolError::invalid_argument(
                "MapFFT target map is not set",
            ));
        }
        let map = self.build_density_map()?;
        self.target = Some(map);
        Ok(())
    }

    pub fn do_fft_into(&mut self, target: &mut DensityMap) -> Result<()> {
        *target = self.build_density_map()?;
        Ok(())
    }

    /// Run the configured transform and return a new map.
    pub fn build_density_map(&mut self) -> Result<DensityMap> {
        if self.reflections.is_empty() {
            return Err(CueMolError::invalid_argument(
                "MapFFT has no reflection data",
            ));
        }
        let cell_abc = [self.cell[0], self.cell[1], self.cell[2]];
        if cell_abc
            .iter()
            .any(|length| !length.is_finite() || *length <= 0.0)
        {
            return Err(CueMolError::invalid_argument(
                "MapFFT unit cell lengths must be positive",
            ));
        }
        let grid_size = if self.grid_size.is_finite() && self.grid_size > 0.0 {
            self.grid_size
        } else {
            1.0 / 3.0
        };
        let max_hkl = self.reflections.iter().fold([1_i32; 3], |mut max, r| {
            max[0] = max[0].max(r.h.abs());
            max[1] = max[1].max(r.k.abs());
            max[2] = max[2].max(r.l.abs());
            max
        });
        let resolution = if self.map_resolution.is_finite() && self.map_resolution > 0.1 {
            self.map_resolution
        } else {
            auto_resolution(cell_abc, max_hkl, grid_size)
        };
        if !resolution.is_finite() || resolution <= 0.0 {
            return Err(CueMolError::invalid_argument(
                "MapFFT resolution must be positive",
            ));
        }

        let manager = cuemol_symm::SymmOpManager::new();
        let operations = manager
            .symmetry_operators(self.space_group)
            .ok_or_else(|| {
                CueMolError::parse(format!("invalid space-group number {}", self.space_group))
            })?;
        let symops = operations
            .iter()
            .map(|operation| SymOp::from_matrix(&operation.matrix))
            .collect::<Vec<_>>();
        let grid = synthesize_density_map_with_cell_and_phase_mode(
            &self.reflections,
            &symops,
            self.cell,
            resolution,
            grid_size,
            self.use_phases,
        )?;
        let dimensions = grid.counts;
        let map = DensityMap::from_crystallographic_grid(
            grid,
            [0, 0, 0],
            dimensions,
            self.cell,
            self.space_group,
        );
        self.effective_resolution = Some(resolution);
        self.grid_dimensions = Some(dimensions);
        Ok(map)
    }
}

/// Flat index into a density/reciprocal grid: c fastest, a slowest
/// (matches C++ MapFFT `IND(h,k,l) = l + nc*(k + nb*h)`).
#[inline]
fn grid_index(a: usize, b: usize, c: usize, nb: usize, nc: usize) -> usize {
    c + nc * (b + nb * a)
}

/// In-place separable 3D forward FFT (unnormalized, like FFTW), transforming
/// each axis in turn. Grid is laid out with c fastest, a slowest.
fn fft3d_forward(grid: &mut [Complex<f32>], na: usize, nb: usize, nc: usize) {
    let mut planner = FftPlanner::<f32>::new();
    let fft_a = planner.plan_fft_forward(na);
    let fft_b = planner.plan_fft_forward(nb);
    let fft_c = planner.plan_fft_forward(nc);

    // c axis: contiguous lines of length nc.
    for a in 0..na {
        for b in 0..nb {
            let base = grid_index(a, b, 0, nb, nc);
            fft_c.process(&mut grid[base..base + nc]);
        }
    }
    // b axis: stride nc.
    let mut line = vec![Complex::<f32>::new(0.0, 0.0); nb];
    for a in 0..na {
        for c in 0..nc {
            for (b, slot) in line.iter_mut().enumerate() {
                *slot = grid[grid_index(a, b, c, nb, nc)];
            }
            fft_b.process(&mut line);
            for (b, value) in line.iter().enumerate() {
                grid[grid_index(a, b, c, nb, nc)] = *value;
            }
        }
    }
    // a axis: stride nb*nc.
    let mut line = vec![Complex::<f32>::new(0.0, 0.0); na];
    for b in 0..nb {
        for c in 0..nc {
            for (a, slot) in line.iter_mut().enumerate() {
                *slot = grid[grid_index(a, b, c, nb, nc)];
            }
            fft_a.process(&mut line);
            for (a, value) in line.iter().enumerate() {
                grid[grid_index(a, b, c, nb, nc)] = *value;
            }
        }
    }
}

/// A crystallographic symmetry operation, prepared for structure-factor
/// expansion: the reciprocal-space rotation `(R^-1)^T` (applied to Miller
/// indices) and the real-space fractional translation (the phase shift source).
#[derive(Debug, Clone, Copy)]
pub struct SymOp {
    pub recip: [[f64; 3]; 3],
    pub trans: [f64; 3],
}

impl SymOp {
    pub fn identity() -> Self {
        SymOp {
            recip: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            trans: [0.0, 0.0, 0.0],
        }
    }

    /// Build from a real-space symmetry op (3x3 rotation + fractional
    /// translation); the reciprocal rotation is `(R^-1)^T` (C++ `makeRecipOp`).
    pub fn from_real_space(rotation: [[f64; 3]; 3], translation: [f64; 3]) -> Self {
        SymOp {
            recip: make_recip_op(rotation),
            trans: translation,
        }
    }

    /// Build from a `cuemol-symm` symmetry-operator matrix: the upper-left 3x3 is
    /// the rotation, the last column (`values[i][3]`) the fractional translation.
    pub fn from_matrix(matrix: &Matrix4) -> Self {
        let v = &matrix.values;
        let rotation = [
            [v[0][0], v[0][1], v[0][2]],
            [v[1][0], v[1][1], v[1][2]],
            [v[2][0], v[2][1], v[2][2]],
        ];
        let translation = [v[0][3], v[1][3], v[2][3]];
        SymOp::from_real_space(rotation, translation)
    }
}

/// Inverse of a 3x3 matrix via the adjugate (symops have determinant +/-1).
fn invert3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let inv = 1.0 / det;
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
        ],
    ]
}

/// Reciprocal-space operator `(R^-1)^T` for a real-space rotation (C++ `makeRecipOp`).
fn make_recip_op(rotation: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let inv = invert3(rotation);
    [
        [inv[0][0], inv[1][0], inv[2][0]],
        [inv[0][1], inv[1][1], inv[2][1]],
        [inv[0][2], inv[1][2], inv[2][2]],
    ]
}

fn apply_recip(recip: &[[f64; 3]; 3], h: i32, k: i32, l: i32) -> (i32, i32, i32) {
    let (hf, kf, lf) = (h as f64, k as f64, l as f64);
    let nh = recip[0][0] * hf + recip[0][1] * kf + recip[0][2] * lf;
    let nk = recip[1][0] * hf + recip[1][1] * kf + recip[1][2] * lf;
    let nl = recip[2][0] * hf + recip[2][1] * kf + recip[2][2] * lf;
    (nh.round() as i32, nk.round() as i32, nl.round() as i32)
}

/// Place a structure factor and its Friedel mate into the reciprocal grid.
fn place_reflection(
    grid: &mut [Complex<f32>],
    floc: Complex<f32>,
    hkl: (i32, i32, i32),
    dims: [usize; 3],
) {
    let [na, nb, nc] = dims;
    let h = hkl.0.rem_euclid(na as i32) as usize;
    let k = hkl.1.rem_euclid(nb as i32) as usize;
    let l = hkl.2.rem_euclid(nc as i32) as usize;
    grid[grid_index(h, k, l, nb, nc)] = floc;
    // Friedel mate F(-h) = conj(F(h)).
    let (mh, mk, ml) = ((na - h) % na, (nb - k) % nb, (nc - l) % nc);
    grid[grid_index(mh, mk, ml, nb, nc)] = floc.conj();
}

/// Synthesize a real-space electron-density grid (size `na*nb*nc`, c fastest)
/// from structure factors, expanding each reflection over the given symmetry
/// operations. Port of C++ `MapFFT::doFFT`: amplitudes scaled by 1/V; each
/// reflection is rotated by the reciprocal symop, phase-shifted by the real-space
/// translation, placed with its Friedel conjugate, then an unnormalized forward
/// FFT yields the density (real part). Pass `&[SymOp::identity()]` for P1.
pub fn synthesize_density(
    reflections: &[Reflection],
    symops: &[SymOp],
    cell_abc: [f64; 3],
    na: usize,
    nb: usize,
    nc: usize,
) -> Vec<f64> {
    synthesize_density_with_phase_mode(reflections, symops, cell_abc, na, nb, nc, true)
}

/// Structure-factor synthesis with explicit phase availability.
///
/// Patterson maps have no phase column; matching C++ `MapFFT`, symmetry
/// translation phase shifts are disabled in that mode.
pub fn synthesize_density_with_phase_mode(
    reflections: &[Reflection],
    symops: &[SymOp],
    cell_abc: [f64; 3],
    na: usize,
    nb: usize,
    nc: usize,
    use_phases: bool,
) -> Vec<f64> {
    let fscale = (1.0 / (cell_abc[0] * cell_abc[1] * cell_abc[2])) as f32;
    let mut grid = vec![Complex::<f32>::new(0.0, 0.0); na * nb * nc];

    for sym in symops {
        for refl in reflections {
            let (h, k, l) = apply_recip(&sym.recip, refl.h, refl.k, refl.l);
            // Phase shift from the real-space translation, on the rotated indices.
            let phsh = if use_phases {
                (sym.trans[0] * h as f64 + sym.trans[1] * k as f64 + sym.trans[2] * l as f64)
                    * std::f64::consts::PI
                    * 2.0
            } else {
                0.0
            };
            let ampl = refl.f * fscale;
            let phas = refl.phi_deg * std::f32::consts::PI / 180.0;
            let floc = Complex::from_polar(1.0, phsh as f32) * Complex::from_polar(ampl, phas);
            // Skip (near-)zero missing reflections so they never overwrite data.
            if floc.norm() < 1.0e-8 {
                continue;
            }
            place_reflection(&mut grid, floc, (h, k, l), [na, nb, nc]);
        }
    }

    fft3d_forward(&mut grid, na, nb, nc);
    grid.iter().map(|value| value.re as f64).collect()
}

/// Convenience wrapper: synthesize density in space group P1 (no symmetry).
pub fn synthesize_density_p1(
    reflections: &[Reflection],
    cell_abc: [f64; 3],
    na: usize,
    nb: usize,
    nc: usize,
) -> Vec<f64> {
    synthesize_density(reflections, &[SymOp::identity()], cell_abc, na, nb, nc)
}

/// Auto map resolution from the maximum Miller indices (C++ `checkMapResoln`
/// auto path): the finest sampling each axis can support, divided by the grid
/// fraction. Used when an explicit resolution is not supplied.
pub fn auto_resolution(cell_abc: [f64; 3], max_hkl: [i32; 3], grid_spacing: f64) -> f64 {
    let maxg = (0..3)
        .map(|i| cell_abc[i] / (3.0 * max_hkl[i].max(1) as f64))
        .fold(f64::INFINITY, f64::min);
    maxg / grid_spacing
}

/// End-to-end: structure factors -> electron-density `GridMap` for an orthogonal
/// cell. Picks an FFT grid, runs the symmetry-expanded synthesis, and re-indexes
/// from the synthesis layout (c fastest) to the GridMap layout (x fastest, the
/// convention the map readers and `DensityMap::from_grid_map` use).
pub fn synthesize_density_map(
    reflections: &[Reflection],
    symops: &[SymOp],
    cell_abc: [f64; 3],
    resolution: f64,
    grid_spacing: f64,
) -> GridMap {
    synthesize_density_map_with_cell(
        reflections,
        symops,
        [cell_abc[0], cell_abc[1], cell_abc[2], 90.0, 90.0, 90.0],
        resolution,
        grid_spacing,
    )
    .expect("orthogonal cell parameters are valid")
}

/// End-to-end density synthesis for a full crystallographic unit cell. The FFT
/// placement follows C++ `MapFFT`; the resulting `GridMap` stores non-orthogonal
/// cell axes in its delta vectors, as the CCP4/XPLOR map readers do.
pub fn synthesize_density_map_with_cell(
    reflections: &[Reflection],
    symops: &[SymOp],
    cell: [f64; 6],
    resolution: f64,
    grid_spacing: f64,
) -> Result<GridMap> {
    synthesize_density_map_with_cell_and_phase_mode(
        reflections,
        symops,
        cell,
        resolution,
        grid_spacing,
        true,
    )
}

pub fn synthesize_density_map_with_cell_and_phase_mode(
    reflections: &[Reflection],
    symops: &[SymOp],
    cell: [f64; 6],
    resolution: f64,
    grid_spacing: f64,
    use_phases: bool,
) -> Result<GridMap> {
    let cell_abc = [cell[0], cell[1], cell[2]];
    let (na, nb, nc) = fft_grid_size(cell_abc, resolution, grid_spacing);
    let (na, nb, nc) = (na as usize, nb as usize, nc as usize);
    let density =
        synthesize_density_with_phase_mode(reflections, symops, cell_abc, na, nb, nc, use_phases);

    // Re-index: GridMap values are x-fastest (x + na*(y + nb*z)); synthesis is
    // c-fastest (c + nc*(b + nb*a)) with a along cell-a, b cell-b, c cell-c.
    let mut values = vec![0.0f64; na * nb * nc];
    for a in 0..na {
        for b in 0..nb {
            for c in 0..nc {
                values[a + na * (b + nb * c)] = density[grid_index(a, b, c, nb, nc)];
            }
        }
    }

    let delta = cell_delta_vectors(cell, [na, nb, nc])?;
    Ok(GridMap {
        counts: [na, nb, nc],
        origin: Vector4::ZERO,
        delta,
        values,
    })
}

fn cell_delta_vectors(cell: [f64; 6], grid_intervals: [usize; 3]) -> Result<[Vector4; 3]> {
    if grid_intervals.contains(&0) {
        return Err(CueMolError::parse("invalid density map grid interval"));
    }

    let basis = cell_basis(cell)?;
    Ok([
        basis[0].divide(grid_intervals[0] as f64)?,
        basis[1].divide(grid_intervals[1] as f64)?,
        basis[2].divide(grid_intervals[2] as f64)?,
    ])
}

fn cell_basis(cell: [f64; 6]) -> Result<[Vector4; 3]> {
    let [a, b, c, alpha, beta, gamma] = cell;
    if a <= 0.0 || b <= 0.0 || c <= 0.0 {
        return Err(CueMolError::parse("invalid density map unit cell length"));
    }

    let alpha = alpha.to_radians();
    let beta = beta.to_radians();
    let gamma = gamma.to_radians();
    let sin_gamma = gamma.sin();
    if sin_gamma.abs() < 1.0e-12 {
        return Err(CueMolError::parse("invalid density map unit cell angle"));
    }

    let ax = Vector4::new3(a, 0.0, 0.0);
    let bx = b * gamma.cos();
    let by = b * sin_gamma;
    let cx = c * beta.cos();
    let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / sin_gamma;
    let cz_sq = c * c - cx * cx - cy * cy;
    if cz_sq < -1.0e-8 {
        return Err(CueMolError::parse("invalid density map unit cell geometry"));
    }
    Ok([
        ax,
        Vector4::new3(bx, by, 0.0),
        Vector4::new3(cx, cy, cz_sq.max(0.0).sqrt()),
    ])
}

/// Smallest `n >= start` that is divisible by `base1` and `base2` and whose
/// `n / base2` is `prim`-smooth (factors only into primes <= `prim`). Faithful
/// port of C++ `calcprime`.
pub fn calc_prime(start: i32, base1: i32, base2: i32, prim: i32) -> i32 {
    let mut n = start - 1;
    loop {
        // Advance until n is divisible by both bases.
        loop {
            n += 1;
            if n % base1 == 0 && n % base2 == 0 {
                break;
            }
        }
        // Divide out every prime factor <= prim; if nothing is left, n is good.
        let mut nn = n / base2;
        let mut p = nn.min(prim);
        while p > 1 {
            while nn % p == 0 {
                nn /= p;
            }
            p -= 1;
        }
        if nn == 1 {
            return n;
        }
    }
}

/// FFT grid sizes `(na, nb, nc)` for a unit cell sampled at `resolution` with the
/// given `grid` fraction (C++ default 1/3). The c axis is forced even so the
/// real-output (c2r) transform is usable. Port of C++ `MapFFT::calcgrid`.
pub fn fft_grid_size(cell_abc: [f64; 3], resolution: f64, grid: f64) -> (i32, i32, i32) {
    let target = |length: f64| -> i32 { 2.max((length / (resolution * grid) + 1.0e-8) as i32) };
    let na = calc_prime(target(cell_abc[0]), 1, 1, FT_PRIM);
    let nb = calc_prime(target(cell_abc[1]), 1, 1, FT_PRIM);
    // base2 = 2: the c (fastest) axis must be even for the Hermitian transform.
    let nc = calc_prime(target(cell_abc[2]), 1, 2, FT_PRIM);
    (na, nb, nc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::ScalarObject;

    #[test]
    fn map_fft_defaults_match_reference() {
        let fft = MapFFT::new();
        assert_eq!(fft.cell(), [0.0; 6]);
        assert_eq!(fft.space_group(), -1);
        assert!((fft.grid_size() - 1.0 / 3.0).abs() < 1.0e-12);
        assert_eq!(fft.map_resolution(), -1.0);
        assert!(fft.uses_phases());
        assert!(fft.reflections().is_empty());
        assert!(fft.target().is_none());
    }

    #[test]
    fn map_fft_imports_weighted_rows_and_populates_target() {
        let mut fft = MapFFT::new();
        fft.set_params(10.0, 10.0, 10.0, 90.0, 90.0, 90.0, 1, 1.0 / 3.0, 3.0);
        fft.set_data(1, 6, &[1.0, 0.0, 0.0, 1000.0, 0.0, 0.5], 0, 1, 2, 3, 4, 5)
            .unwrap();
        assert_eq!(fft.reflections()[0].f, 500.0);
        assert!(fft.uses_phases());

        fft.set_target(DensityMap::new());
        fft.do_fft().unwrap();
        let map = fft.target().unwrap();
        assert!((map.den_max() - 1.0).abs() < 1.0e-4);
        assert_eq!(map.cell().space_group(), 1);
        assert_eq!(
            fft.grid_dimensions(),
            Some([map.col_no(), map.row_no(), map.sec_no()])
        );
        assert_eq!(fft.effective_resolution(), Some(3.0));
    }

    #[test]
    fn map_fft_validates_buffer_target_and_space_group() {
        let mut fft = MapFFT::new();
        assert!(fft.set_data(2, 5, &[0.0; 5], 0, 1, 2, 3, 4, -1).is_err());
        fft.set_params(10.0, 10.0, 10.0, 90.0, 90.0, 90.0, 999, 1.0 / 3.0, 3.0);
        fft.set_reflections(
            &[Reflection {
                h: 1,
                k: 0,
                l: 0,
                f: 1000.0,
                phi_deg: 0.0,
            }],
            true,
        );
        assert!(fft.do_fft().is_err());
        fft.set_target(DensityMap::new());
        assert!(fft.do_fft().is_err());
    }

    #[test]
    fn patterson_mode_omits_symmetry_translation_phase() {
        let screw = SymOp::from_real_space(
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0, 0.0, 0.5],
        );
        let reflection = [Reflection {
            h: 0,
            k: 0,
            l: 1,
            f: 1000.0,
            phi_deg: 0.0,
        }];
        let symops = [SymOp::identity(), screw];
        let phased =
            synthesize_density_with_phase_mode(&reflection, &symops, [10.0; 3], 4, 4, 4, true);
        let patterson =
            synthesize_density_with_phase_mode(&reflection, &symops, [10.0; 3], 4, 4, 4, false);

        assert!(phased[0] < -1.9);
        assert!(patterson[0] > 1.9);
    }

    #[test]
    fn calc_prime_rounds_up_to_smooth_numbers() {
        // 10 = 2*5 already smooth; 11 -> 12 = 2^2*3; 13 -> 15 = 3*5.
        assert_eq!(calc_prime(10, 1, 1, 5), 10);
        assert_eq!(calc_prime(11, 1, 1, 5), 12);
        assert_eq!(calc_prime(13, 1, 1, 5), 15);
        // 7 stays only if smooth; 7 is prime > 5 -> 8 = 2^3.
        assert_eq!(calc_prime(7, 1, 1, 5), 8);
        // Powers and products of {2,3,5} are returned unchanged.
        assert_eq!(calc_prime(16, 1, 1, 5), 16);
        assert_eq!(calc_prime(30, 1, 1, 5), 30);
    }

    #[test]
    fn calc_prime_even_axis_forces_divisible_by_two() {
        // base2 = 2: result must be even AND n/2 must be 5-smooth.
        // 13 -> 16 (16/2 = 8 = 2^3); 11 -> 12 (12/2 = 6 = 2*3).
        assert_eq!(calc_prime(13, 1, 2, 5), 16);
        assert_eq!(calc_prime(11, 1, 2, 5), 12);
        assert_eq!(calc_prime(10, 1, 2, 5), 10);
        for start in 2..200 {
            assert_eq!(calc_prime(start, 1, 2, 5) % 2, 0);
        }
    }

    #[test]
    fn fft_grid_size_is_smooth_and_c_even() {
        let (na, nb, nc) = fft_grid_size([50.0, 60.0, 70.0], 2.0, 1.0 / 3.0);
        // a: 50/(2*(1/3)) = 75 -> 75 = 3*5^2 already smooth.
        assert_eq!(na, 75);
        // b: 60/(2*(1/3)) = 90 -> 90 = 2*3^2*5 smooth.
        assert_eq!(nb, 90);
        // c: 70/(2*(1/3)) = 105 (odd) -> next even 5-smooth = 108 (108/2=54=2*3^3).
        assert_eq!(nc, 108);
        assert_eq!(nc % 2, 0);
    }

    // A single reflection F(1,0,0) gives an analytically known density wave.
    // With amplitude A, cell volume V and grid count na along a:
    //   phase 0  -> rho(a,b,c) = (2A/V) cos(2*pi*a/na)
    //   phase 90 -> rho(a,b,c) = (2A/V) sin(2*pi*a/na)
    // (the factor 2 is the Friedel mate). This pins amplitude scaling (1/V),
    // reflection placement, the Friedel conjugate, FFT direction, and phase sign.
    fn density_at(grid: &[f64], a: usize, b: usize, c: usize, nb: usize, nc: usize) -> f64 {
        grid[c + nc * (b + nb * a)]
    }

    #[test]
    fn single_reflection_cosine_density() {
        let (na, nb, nc) = (4usize, 4usize, 4usize);
        let cell = [10.0, 10.0, 10.0]; // V = 1000, so A=1000 -> amplitude 1.0
        let refls = [Reflection {
            h: 1,
            k: 0,
            l: 0,
            f: 1000.0,
            phi_deg: 0.0,
        }];
        let grid = synthesize_density_p1(&refls, cell, na, nb, nc);

        for b in 0..nb {
            for c in 0..nc {
                for a in 0..na {
                    let expected = 2.0 * (2.0 * std::f64::consts::PI * a as f64 / na as f64).cos();
                    assert!(
                        (density_at(&grid, a, b, c, nb, nc) - expected).abs() < 1.0e-4,
                        "rho({a},{b},{c}) expected {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn single_reflection_phase_gives_sine_density() {
        let (na, nb, nc) = (4usize, 4usize, 4usize);
        let cell = [10.0, 10.0, 10.0];
        let refls = [Reflection {
            h: 1,
            k: 0,
            l: 0,
            f: 1000.0,
            phi_deg: 90.0,
        }];
        let grid = synthesize_density_p1(&refls, cell, na, nb, nc);

        for a in 0..na {
            let expected = 2.0 * (2.0 * std::f64::consts::PI * a as f64 / na as f64).sin();
            assert!(
                (density_at(&grid, a, 0, 0, nb, nc) - expected).abs() < 1.0e-4,
                "rho({a},0,0) expected {expected}"
            );
        }
    }

    #[test]
    fn make_recip_op_of_twofold_is_itself() {
        // 2-fold about c: diag(-1,-1,1); its (R^-1)^T is the same matrix.
        let twofold = [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(make_recip_op(twofold), twofold);
    }

    #[test]
    fn symmetry_expansion_matches_explicit_p1_enumeration() {
        // A 2_1 screw along c: x,y,z -> -x,-y,z+1/2. Reflection (1,0,1) maps to
        // (-1,0,1) with a phase shift of 2*pi*(0.5*1) = pi (i.e. +180 deg).
        // Expanding (1,0,1) under {identity, screw} must equal a plain P1 sum that
        // lists the symmetry mate explicitly.
        let (na, nb, nc) = (6usize, 6usize, 6usize);
        let cell = [12.0, 12.0, 12.0];
        let screw = SymOp::from_real_space(
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0, 0.0, 0.5],
        );

        let expanded = synthesize_density(
            &[Reflection {
                h: 1,
                k: 0,
                l: 1,
                f: 1000.0,
                phi_deg: 30.0,
            }],
            &[SymOp::identity(), screw],
            cell,
            na,
            nb,
            nc,
        );
        let explicit = synthesize_density_p1(
            &[
                Reflection {
                    h: 1,
                    k: 0,
                    l: 1,
                    f: 1000.0,
                    phi_deg: 30.0,
                },
                Reflection {
                    h: -1,
                    k: 0,
                    l: 1,
                    f: 1000.0,
                    phi_deg: 30.0 + 180.0,
                },
            ],
            cell,
            na,
            nb,
            nc,
        );

        assert_eq!(expanded.len(), explicit.len());
        for (a, b) in expanded.iter().zip(explicit.iter()) {
            assert!((a - b).abs() < 1.0e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn symop_from_matrix_extracts_rotation_and_translation() {
        // 2_1 screw -x,-y,z+1/2: rotation diag(-1,-1,1), translation (0,0,1/2).
        let m = Matrix4 {
            values: [
                [-1.0, 0.0, 0.0, 0.0],
                [0.0, -1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let s = SymOp::from_matrix(&m);
        assert_eq!(
            s.recip,
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(s.trans, [0.0, 0.0, 0.5]);
    }

    #[test]
    fn symops_from_cuemol_symm_table_integrate() {
        // P-1 (space group 2) = identity + inversion, straight from the symm table.
        let mgr = cuemol_symm::SymmOpManager::new();
        let ops = mgr.symmetry_operators(2).expect("P-1 operators");
        assert_eq!(ops.len(), 2);
        let symops: Vec<SymOp> = ops.iter().map(|o| SymOp::from_matrix(&o.matrix)).collect();
        assert_eq!(
            symops[0].recip,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(
            symops[1].recip,
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]
        );
        // The full chain (table -> Matrix4 -> SymOp -> synthesis) yields finite density.
        let grid = synthesize_density(
            &[Reflection {
                h: 1,
                k: 1,
                l: 0,
                f: 500.0,
                phi_deg: 0.0,
            }],
            &symops,
            [20.0, 20.0, 20.0],
            6,
            6,
            6,
        );
        assert!(grid.iter().all(|v| v.is_finite()));
        assert_eq!(grid.len(), 6 * 6 * 6);
    }

    #[test]
    fn auto_resolution_from_max_indices() {
        // maxg = 10/(3*5) = 2/3; resolution = maxg/(1/3) = 2.0 A.
        let res = auto_resolution([10.0, 10.0, 10.0], [5, 5, 5], 1.0 / 3.0);
        assert!((res - 2.0).abs() < 1.0e-9);
    }

    #[test]
    fn density_map_has_cell_grid_and_cosine_values() {
        // Single F(1,0,0) phase 0 -> rho(a,b,c) = (2A/V) cos(2*pi*a/na).
        let cell = [10.0, 12.0, 14.0];
        let refls = [Reflection {
            h: 1,
            k: 0,
            l: 0,
            f: 1000.0,
            phi_deg: 0.0,
        }];
        let map = synthesize_density_map(&refls, &[SymOp::identity()], cell, 3.0, 1.0 / 3.0);

        let (ga, gb, gc) = fft_grid_size(cell, 3.0, 1.0 / 3.0);
        let [na, nb, nc] = map.counts;
        assert_eq!([na, nb, nc], [ga as usize, gb as usize, gc as usize]);
        assert_eq!(map.values.len(), na * nb * nc);

        // delta * counts reconstructs the orthogonal cell edges.
        assert!((map.delta[0].length3d() * na as f64 - cell[0]).abs() < 1.0e-9);
        assert!((map.delta[1].length3d() * nb as f64 - cell[1]).abs() < 1.0e-9);
        assert!((map.delta[2].length3d() * nc as f64 - cell[2]).abs() < 1.0e-9);

        let amp = 2.0 * 1000.0 / (cell[0] * cell[1] * cell[2]);
        for a in 0..na {
            let expected = amp * (2.0 * std::f64::consts::PI * a as f64 / na as f64).cos();
            for b in 0..nb {
                for c in 0..nc {
                    let got = map.values[a + na * (b + nb * c)];
                    assert!(
                        (got - expected).abs() < 1.0e-3,
                        "rho({a},{b},{c})={got} expected {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn density_map_with_cell_preserves_non_orthogonal_axes() {
        let cell = [10.0, 12.0, 14.0, 90.0, 90.0, 120.0];
        let refls = [Reflection {
            h: 1,
            k: 0,
            l: 0,
            f: 1000.0,
            phi_deg: 0.0,
        }];
        let map =
            synthesize_density_map_with_cell(&refls, &[SymOp::identity()], cell, 3.0, 1.0 / 3.0)
                .unwrap();

        let [na, nb, nc] = map.counts;
        assert!((map.delta[0].length3d() * na as f64 - cell[0]).abs() < 1.0e-9);
        assert!((map.delta[1].length3d() * nb as f64 - cell[1]).abs() < 1.0e-9);
        assert!((map.delta[2].length3d() * nc as f64 - cell[2]).abs() < 1.0e-9);
        assert!(map.delta[1].x < 0.0, "gamma=120 should skew b toward -x");
        assert!(map.delta[1].y > 0.0);
        assert!(map.delta[2].x.abs() < 1.0e-10);
        assert!(map.delta[2].y.abs() < 1.0e-10);
    }
}
