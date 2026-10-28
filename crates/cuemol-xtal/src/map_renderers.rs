use cuemol_core::{Matrix3, Matrix4, Vector4};
use cuemol_gfx::SolidColor;
use cuemol_molstr::{GridMap, Selector};
use cuemol_qsys::{DispListRenderer, MultiGradient, ScalarObject, View};
use cuemol_surface::{build_grid_isosurface_masked, MolSurfObj, ScalarField, SurfaceMesh};
use cuemol_symm::CrystalInfo;
use std::sync::Arc;

fn fraction_level(
    hist: &[u64; 256],
    base: f64,
    step: f64,
    min: f64,
    max: f64,
    fraction: f64,
) -> f64 {
    let count = hist.iter().sum::<u64>();
    if count == 0 {
        return 0.;
    }
    if fraction <= 0. {
        return max;
    }
    if fraction >= 1. {
        return min;
    }
    let target = (count as f64 * fraction - 1e-6).ceil() as u64;
    let mut sum = 0;
    for i in (0..256).rev() {
        sum += hist[i];
        if sum >= target {
            return i as f64 * step + base;
        }
    }
    min
}
fn level_fraction(hist: &[u64; 256], base: f64, step: f64, level: f64) -> f64 {
    let count = hist.iter().sum::<u64>();
    if count == 0 {
        return 0.;
    }
    if step <= 0. {
        return if level <= base { 1. } else { 0. };
    }
    let lower = ((level - base) / step + 1e-6).floor();
    if lower > 255. {
        return 0.;
    }
    hist[lower.max(0.) as usize..].iter().sum::<u64>() as f64 / count as f64
}

#[derive(Debug, Clone, PartialEq)]
pub struct DensityMap {
    counts: [usize; 3],
    start: [isize; 3],
    intervals: [usize; 3],
    origin: Vector4,
    delta: [Vector4; 3],
    cell: CrystalInfo,
    min_density: f64,
    max_density: f64,
    mean_density: f64,
    rmsd_density: f64,
    level_base: f64,
    level_step: f64,
    bytes: std::sync::Arc<Vec<u8>>,
    histogram: [u64; 256],
    map_type: u8,
    detected_map_type: u8,
}

impl Default for DensityMap {
    fn default() -> Self {
        Self {
            counts: [0, 0, 0],
            start: [0, 0, 0],
            intervals: [0, 0, 0],
            origin: Vector4::ZERO,
            delta: [Vector4::ZERO, Vector4::ZERO, Vector4::ZERO],
            cell: CrystalInfo::new(0.0, 0.0, 0.0, 90.0, 90.0, 90.0),
            min_density: 0.0,
            max_density: 0.0,
            mean_density: 0.0,
            rmsd_density: 0.0,
            level_base: 0.0,
            level_step: 0.0,
            bytes: std::sync::Arc::new(Vec::new()),
            histogram: [0; 256],
            map_type: 0,
            detected_map_type: 1,
        }
    }
}

impl DensityMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subsample(&mut self, stride: usize) -> cuemol_core::Result<()> {
        if stride == 0 {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "subsample must be positive",
            ));
        }
        if stride == 1 {
            return Ok(());
        }
        let old = self.counts;
        let counts = old.map(|n| n.div_ceil(stride));
        let intervals = self.intervals.map(|n| n.max(1).div_ceil(stride));
        let scale: [f64; 3] =
            std::array::from_fn(|i| self.intervals[i].max(1) as f64 / intervals[i] as f64);
        let mut bytes = Vec::with_capacity(counts.iter().product());
        for z in 0..counts[2] {
            for y in 0..counts[1] {
                for x in 0..counts[0] {
                    let position = [
                        x as f64 * scale[0],
                        y as f64 * scale[1],
                        z as f64 * scale[2],
                    ];
                    let lower = position.map(|v| v.floor() as usize);
                    let mut value = 0.;
                    for dz in 0..=1 {
                        for dy in 0..=1 {
                            for dx in 0..=1 {
                                let side = [dx, dy, dz];
                                let mut weight = 1.;
                                let mut index = [0; 3];
                                for i in 0..3 {
                                    let frac = position[i] - lower[i] as f64;
                                    weight *= if side[i] == 0 { 1. - frac } else { frac };
                                    index[i] = (lower[i] + side[i]).min(old[i].saturating_sub(1));
                                }
                                value += weight
                                    * f64::from(
                                        self.bytes
                                            [(index[2] * old[1] + index[1]) * old[0] + index[0]],
                                    );
                            }
                        }
                    }
                    bytes.push(value.round().clamp(0., 255.) as u8);
                }
            }
        }
        self.origin = self.origin
            + self.delta[0] * self.start[0] as f64
            + self.delta[1] * self.start[1] as f64
            + self.delta[2] * self.start[2] as f64;
        self.start = [0; 3];
        self.counts = counts;
        self.intervals = intervals;
        self.delta = std::array::from_fn(|i| self.delta[i] * scale[i]);
        self.bytes = std::sync::Arc::new(bytes);
        self.rebuild_histogram();
        Ok(())
    }

    pub fn set_map_type(&mut self, value: &str) -> cuemol_core::Result<()> {
        self.map_type = match value {
            "auto" => 0,
            "xtal" => 1,
            "em" => 2,
            _ => {
                return Err(cuemol_core::CueMolError::invalid_argument(
                    "map type must be auto, xtal, or em",
                ))
            }
        };
        Ok(())
    }
    pub fn map_type(&self) -> &str {
        match self.map_type {
            1 => "xtal",
            2 => "em",
            _ => "auto",
        }
    }
    pub fn is_em(&self) -> bool {
        if self.map_type == 0 {
            self.detected_map_type == 2
        } else {
            self.map_type == 2
        }
    }
    pub fn detected_map_type(&self) -> u8 {
        self.detected_map_type
    }
    pub fn set_detected_map_type(&mut self, value: u8) {
        if value == 1 || value == 2 {
            self.detected_map_type = value;
        }
    }
    pub fn set_origin(&mut self, value: Vector4) {
        self.origin = value;
    }
    fn rebuild_histogram(&mut self) {
        self.histogram = [0; 256];
        for value in self.bytes.iter() {
            self.histogram[*value as usize] += 1;
        }
    }
    pub fn level_at_top_fraction(&self, fraction: f64) -> f64 {
        fraction_level(
            &self.histogram,
            self.level_base,
            self.level_step,
            self.min_density,
            self.max_density,
            fraction,
        )
    }
    pub fn top_fraction_at_level(&self, level: f64) -> f64 {
        level_fraction(&self.histogram, self.level_base, self.level_step, level)
    }

    pub fn type_name(&self) -> &'static str {
        "DensityMap"
    }

    pub fn from_grid_map(grid: GridMap) -> Self {
        let mut map = Self::new();
        let intervals = [
            grid.counts[0].max(1),
            grid.counts[1].max(1),
            grid.counts[2].max(1),
        ];
        map.set_map_float_array(
            &grid.values,
            grid.counts,
            [0, 0, 0],
            intervals,
            grid.origin,
            grid.delta,
        );
        map
    }

    /// Build a crystallographic map while preserving its file-header grid
    /// start, unit-cell intervals, cell, and space group.
    pub fn from_crystallographic_grid(
        grid: GridMap,
        start: [isize; 3],
        intervals: [usize; 3],
        cell: [f64; 6],
        space_group: i32,
    ) -> Self {
        // GridMap::origin is the Cartesian position of its first stored voxel,
        // whereas DensityMap::origin is the crystallographic cell origin.
        let origin = grid.origin
            - grid.delta[0].scale(start[0] as f64)
            - grid.delta[1].scale(start[1] as f64)
            - grid.delta[2].scale(start[2] as f64);
        let mut map = Self::new();
        map.set_map_float_array(
            &grid.values,
            grid.counts,
            start,
            intervals,
            origin,
            grid.delta,
        );
        map.set_xtal_params(cell[0], cell[1], cell[2], cell[3], cell[4], cell[5]);
        map.set_space_group(space_group);
        map
    }

    pub fn set_map_float_array(
        &mut self,
        values: &[f64],
        counts: [usize; 3],
        start: [isize; 3],
        intervals: [usize; 3],
        origin: Vector4,
        delta: [Vector4; 3],
    ) {
        self.counts = counts;
        self.start = start;
        self.intervals = intervals;
        self.origin = origin;
        self.delta = delta;
        self.cell = cell_from_delta(delta, intervals);

        self.update_stats(values);
        self.bytes = std::sync::Arc::new(
            values
                .iter()
                .map(|value| self.float_to_byte(*value))
                .collect(),
        );
        self.pad_or_truncate_bytes();
        self.rebuild_histogram();
    }

    pub fn set_map_byte_array(
        &mut self,
        bytes: &[u8],
        counts: [usize; 3],
        min_density: f64,
        max_density: f64,
        mean_density: f64,
        rmsd_density: f64,
    ) {
        self.counts = counts;
        self.min_density = min_density;
        self.max_density = max_density;
        self.mean_density = mean_density;
        self.rmsd_density = rmsd_density;
        self.level_base = min_density;
        self.level_step = (max_density - min_density) / 256.0;
        self.bytes = std::sync::Arc::new(bytes.to_vec());
        self.pad_or_truncate_bytes();
        self.rebuild_histogram();
    }

    pub fn set_map_params(&mut self, start: [isize; 3], intervals: [usize; 3]) {
        self.start = start;
        self.intervals = intervals;
    }

    pub(crate) fn set_quantized_data(
        &mut self,
        bytes: Vec<u8>,
        counts: [usize; 3],
        stats: cuemol_molstr::MapStats,
        base: f64,
        step: f64,
    ) {
        self.counts = counts;
        self.min_density = stats.min;
        self.max_density = stats.max;
        self.mean_density = stats.mean;
        self.rmsd_density = stats.rms;
        self.level_base = base;
        self.level_step = step;
        self.bytes = Arc::new(bytes);
        self.rebuild_histogram();
    }

    pub fn set_xtal_params(&mut self, a: f64, b: f64, c: f64, alpha: f64, beta: f64, gamma: f64) {
        self.cell = CrystalInfo::new(a, b, c, alpha, beta, gamma);
        self.delta = delta_from_cell(a, b, c, alpha, beta, gamma, self.intervals);
    }

    pub fn cell(&self) -> CrystalInfo {
        self.cell
    }

    /// Grid sampling intervals (cell subdivisions) along each axis. Together with
    /// the cell these define the grid spacing; needed by the QDF map serializer.
    pub fn intervals(&self) -> [usize; 3] {
        self.intervals
    }

    /// Override the crystallographic space-group number (the cell's `nsg`)
    /// without disturbing the derived grid deltas. Used when reconstructing a
    /// map from a QDF stream (which stores the space group separately).
    pub fn set_space_group(&mut self, sg: i32) {
        self.cell.set_space_group(sg);
        self.detected_map_type = if sg == 0 { 2 } else { 1 };
    }

    pub fn den_min(&self) -> f64 {
        self.min_density
    }

    pub fn den_max(&self) -> f64 {
        self.max_density
    }

    pub fn den_mean(&self) -> f64 {
        self.mean_density
    }

    pub fn den_sigma(&self) -> f64 {
        self.rmsd_density
    }

    pub fn get_frac_mat(&self) -> cuemol_core::Matrix4 {
        self.cell.frac_mat()
    }

    pub fn get_orth_mat(&self) -> cuemol_core::Matrix4 {
        self.cell.orth_mat()
    }

    pub fn get_histogram_json(&self, min: f64, max: f64, bins: usize) -> String {
        ScalarObject::get_histogram_json(self, min, max, bins)
    }

    pub fn get_histogram(&self, min: f64, max: f64, bins: usize) -> cuemol_qsys::ScalarHistogram {
        ScalarObject::histogram(self, min, max, bins)
    }

    pub fn byte_values(&self) -> &[u8] {
        &self.bytes
    }

    pub fn fit_view_center_and_radius(&self) -> (Vector4, f64) {
        let center = self.center();
        let radius = self.col_no().max(self.row_no()).max(self.sec_no()) as f64
            * self
                .col_grid_size()
                .max(self.row_grid_size())
                .max(self.sec_grid_size())
            / 2.0;
        (center, radius)
    }

    pub fn fit_view(&self, view: &mut View, _dummy: bool) {
        self.fit_view_with_xform(view, Matrix4::IDENTITY);
    }

    pub fn fit_view_with_xform(&self, view: &mut View, object_xform: Matrix4) {
        let orth = object_xform.mul_mat(self.get_orth_mat());
        let eye = orth.mul_mat(Matrix4::make_rot_mat(view.rot_quat()));
        let corners = [
            Vector4::new4(0.0, 0.0, 0.0, 1.0),
            Vector4::new4(1.0, 0.0, 0.0, 1.0),
            Vector4::new4(0.0, 1.0, 0.0, 1.0),
            Vector4::new4(0.0, 0.0, 1.0, 1.0),
            Vector4::new4(1.0, 1.0, 0.0, 1.0),
            Vector4::new4(0.0, 1.0, 1.0, 1.0),
            Vector4::new4(1.0, 0.0, 1.0, 1.0),
            Vector4::new4(1.0, 1.0, 1.0, 1.0),
        ];
        let (min, max) = transformed_bounds(orth, &corners);
        let (eye_min, eye_max) = transformed_bounds(eye, &corners);

        view.set_view_center((min + max).scale(0.5));

        let width = f64::from(view.width());
        let height = f64::from(view.height());
        let frame_aspect = if width > 0.0 && height > 0.0 {
            width / height
        } else {
            1.0
        };
        let map_width = (eye_max.x - eye_min.x).abs();
        let map_height = (eye_max.y - eye_min.y).abs();
        let map_aspect = if map_height > 0.0 {
            map_width / map_height
        } else {
            f64::INFINITY
        };
        let zoom = if map_aspect > frame_aspect {
            map_width / frame_aspect
        } else {
            map_height
        };
        view.set_zoom(zoom);

        let slab_depth = (eye_max.z - eye_min.z).abs();
        if view.slab_depth() > slab_depth {
            return;
        }
        if view.view_dist() * 2.0 > slab_depth {
            view.set_slab_depth(slab_depth);
        } else {
            view.set_view_dist(slab_depth / 2.0);
            view.set_slab_depth(slab_depth);
        }
    }

    fn update_stats(&mut self, values: &[f64]) {
        if values.is_empty() {
            self.min_density = 0.0;
            self.max_density = 0.0;
            self.mean_density = 0.0;
            self.rmsd_density = 0.0;
            self.level_base = 0.0;
            self.level_step = 0.0;
            return;
        }

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        let mut sq_sum = 0.0;
        for value in values {
            min = min.min(*value);
            max = max.max(*value);
            sum += *value;
            sq_sum += value * value;
        }
        let count = values.len() as f64;
        let mean = sum / count;
        let rmsd = (sq_sum / count - mean * mean).max(0.0).sqrt();

        self.min_density = min;
        self.max_density = max;
        self.mean_density = mean;
        self.rmsd_density = rmsd;
        self.level_base = min;
        self.level_step = (max - min) / 256.0;
    }

    fn float_to_byte(&self, value: f64) -> u8 {
        if self.level_step.abs() < 1.0e-12 {
            return 0;
        }
        ((value - self.level_base) / self.level_step).clamp(0.0, 255.0) as u8
    }

    fn pad_or_truncate_bytes(&mut self) {
        let expected = self.counts.iter().product::<usize>();
        std::sync::Arc::make_mut(&mut self.bytes).resize(expected, 0);
    }

    fn flat_index(&self, col: isize, row: isize, sec: isize) -> Option<usize> {
        if col < 0 || row < 0 || sec < 0 {
            return None;
        }
        let col = col as usize;
        let row = row as usize;
        let sec = sec as usize;
        if col >= self.counts[0] || row >= self.counts[1] || sec >= self.counts[2] {
            return None;
        }
        Some(col + (row + sec * self.counts[1]) * self.counts[0])
    }

    fn index_from_position(&self, position: Vector4) -> [isize; 3] {
        let Some(inverse) = inverse_grid_basis(self.delta) else {
            return [isize::MIN; 3];
        };
        let index = inverse.mul_vec(position - self.origin);
        std::array::from_fn(|axis| {
            ([index.x, index.y, index.z][axis] - self.start[axis] as f64).trunc() as isize
        })
    }
}

fn transformed_bounds(matrix: Matrix4, points: &[Vector4]) -> (Vector4, Vector4) {
    let first = matrix.mul_vec(points[0]);
    points
        .iter()
        .skip(1)
        .fold((first, first), |(mut min, mut max), point| {
            let point = matrix.mul_vec(*point);
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
            (min, max)
        })
}

fn cell_from_delta(delta: [Vector4; 3], intervals: [usize; 3]) -> CrystalInfo {
    let axes = [
        delta[0].scale(intervals[0] as f64),
        delta[1].scale(intervals[1] as f64),
        delta[2].scale(intervals[2] as f64),
    ];
    CrystalInfo::new(
        axes[0].length3d(),
        axes[1].length3d(),
        axes[2].length3d(),
        angle_deg_3d(axes[1], axes[2]),
        angle_deg_3d(axes[0], axes[2]),
        angle_deg_3d(axes[0], axes[1]),
    )
}

fn angle_deg_3d(lhs: Vector4, rhs: Vector4) -> f64 {
    let denom = lhs.length3d() * rhs.length3d();
    if denom <= 1.0e-12 {
        return 90.0;
    }
    let dot = lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z;
    (dot / denom).clamp(-1.0, 1.0).acos().to_degrees()
}

fn delta_from_cell(
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
    intervals: [usize; 3],
) -> [Vector4; 3] {
    let fallback = [
        Vector4::new3(a / intervals[0].max(1) as f64, 0.0, 0.0),
        Vector4::new3(0.0, b / intervals[1].max(1) as f64, 0.0),
        Vector4::new3(0.0, 0.0, c / intervals[2].max(1) as f64),
    ];
    if a <= 0.0 || b <= 0.0 || c <= 0.0 {
        return fallback;
    }

    let alpha = alpha.to_radians();
    let beta = beta.to_radians();
    let gamma = gamma.to_radians();
    let sin_gamma = gamma.sin();
    if sin_gamma.abs() < 1.0e-12 {
        return fallback;
    }

    let bx = b * gamma.cos();
    let by = b * sin_gamma;
    let cx = c * beta.cos();
    let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / sin_gamma;
    let cz_sq = c * c - cx * cx - cy * cy;
    if cz_sq < -1.0e-8 {
        return fallback;
    }

    [
        Vector4::new3(a, 0.0, 0.0).scale(1.0 / intervals[0].max(1) as f64),
        Vector4::new3(bx, by, 0.0).scale(1.0 / intervals[1].max(1) as f64),
        Vector4::new3(cx, cy, cz_sq.max(0.0).sqrt()).scale(1.0 / intervals[2].max(1) as f64),
    ]
}

impl ScalarObject for DensityMap {
    fn quant_step(&self) -> f64 {
        self.level_step
    }

    fn base_histogram(&self) -> (f64, f64, f64, Vec<f64>) {
        (
            self.level_base,
            self.level_base + self.level_step * 256.,
            if self.level_step > 0. {
                self.level_step
            } else {
                1.
            },
            self.histogram.iter().map(|count| *count as f64).collect(),
        )
    }

    fn percent_level_histogram(&self) -> Option<[u64; 256]> {
        self.is_em().then_some(self.histogram)
    }

    fn col_no(&self) -> usize {
        self.counts[0]
    }

    fn row_no(&self) -> usize {
        self.counts[1]
    }

    fn sec_no(&self) -> usize {
        self.counts[2]
    }

    fn start_col(&self) -> isize {
        self.start[0]
    }

    fn start_row(&self) -> isize {
        self.start[1]
    }

    fn start_sec(&self) -> isize {
        self.start[2]
    }

    fn col_grid_size(&self) -> f64 {
        self.delta[0].length3d()
    }

    fn row_grid_size(&self) -> f64 {
        self.delta[1].length3d()
    }

    fn sec_grid_size(&self) -> f64 {
        self.delta[2].length3d()
    }

    fn min_density(&self) -> f64 {
        self.min_density
    }

    fn max_density(&self) -> f64 {
        self.max_density
    }

    fn mean_density(&self) -> f64 {
        self.mean_density
    }

    fn rmsd_density(&self) -> f64 {
        self.rmsd_density
    }

    fn level_base(&self) -> f64 {
        self.level_base
    }

    fn level_step(&self) -> f64 {
        self.level_step
    }

    fn center(&self) -> Vector4 {
        self.conv_to_orth(Vector4::new3(
            self.col_no() as f64 / 2.0,
            self.row_no() as f64 / 2.0,
            self.sec_no() as f64 / 2.0,
        ))
    }

    fn origin(&self) -> Vector4 {
        self.origin
    }

    fn value_at(&self, position: Vector4) -> f64 {
        let [col, row, sec] = self.index_from_position(position);
        if self.is_in_boundary(col, row, sec) {
            self.at_float(col, row, sec)
        } else {
            0.0
        }
    }

    fn is_in_range(&self, position: Vector4) -> bool {
        let [col, row, sec] = self.index_from_position(position);
        self.is_in_boundary(col, row, sec)
    }

    fn is_in_boundary(&self, col: isize, row: isize, sec: isize) -> bool {
        self.flat_index(col, row, sec).is_some()
    }

    fn at_byte(&self, col: isize, row: isize, sec: isize) -> u8 {
        self.flat_index(col, row, sec)
            .and_then(|index| self.bytes.get(index).copied())
            .unwrap_or(0)
    }

    fn at_float(&self, col: isize, row: isize, sec: isize) -> f64 {
        f64::from(self.at_byte(col, row, sec)) * self.level_step + self.level_base
    }

    fn conv_to_orth(&self, index: Vector4) -> Vector4 {
        self.origin
            + self.delta[0] * (index.x + self.start[0] as f64)
            + self.delta[1] * (index.y + self.start[1] as f64)
            + self.delta[2] * (index.z + self.start[2] as f64)
    }

    fn is_periodic_grid(&self) -> bool {
        !self.is_em() && self.counts == self.intervals && self.counts.iter().all(|count| *count > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapColorMode {
    Solid,
    Molecule,
    MultiGradient,
}

impl MapColorMode {
    pub fn code(self) -> i32 {
        match self {
            Self::Solid => MapRenderer::MAPREND_SIMPLE,
            Self::Molecule => MapRenderer::MAPREND_MOLFANC,
            Self::MultiGradient => MapRenderer::MAPREND_MULTIGRAD,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MapClientInfo {
    rmsd_density: f64,
    min_density: f64,
    max_density: f64,
    counts: [usize; 3],
    grid_sizes: [f64; 3],
    histogram: Option<[u64; 256]>,
    level_base: f64,
    level_step: f64,
}

impl MapClientInfo {
    fn from_scalar(scalar: &impl ScalarObject) -> Self {
        Self {
            histogram: scalar.percent_level_histogram(),
            level_base: scalar.level_base(),
            level_step: scalar.level_step(),
            rmsd_density: scalar.rmsd_density(),
            min_density: scalar.min_density(),
            max_density: scalar.max_density(),
            counts: [scalar.col_no(), scalar.row_no(), scalar.sec_no()],
            grid_sizes: [
                scalar.col_grid_size(),
                scalar.row_grid_size(),
                scalar.sec_grid_size(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapRenderer {
    disp_list: DispListRenderer,
    color_mode: MapColorMode,
    color: SolidColor,
    siglevel: f64,
    extent: f64,
    center: Vector4,
    use_pbc: bool,
    use_abslevel: bool,
    bndry_rng: f64,
    bndry_molname: String,
    bndry_sel: Option<Selector>,
    multi_grad: MultiGradient,
    color_mapname: String,
    client_info: Option<MapClientInfo>,
}

impl Default for MapRenderer {
    fn default() -> Self {
        Self {
            disp_list: DispListRenderer::new(),
            color_mode: MapColorMode::Solid,
            color: SolidColor::from_rgb(0.0, 0.0, 1.0),
            siglevel: 1.1,
            extent: 15.0,
            center: Vector4::ZERO,
            use_pbc: true,
            use_abslevel: false,
            bndry_rng: 5.0,
            bndry_molname: String::new(),
            bndry_sel: None,
            multi_grad: MultiGradient::new(),
            color_mapname: String::new(),
            client_info: None,
        }
    }
}

impl MapRenderer {
    pub const MAPREND_SIMPLE: i32 = 0;
    pub const MAPREND_MOLFANC: i32 = 3;
    pub const MAPREND_MULTIGRAD: i32 = 4;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn color_mode(&self) -> MapColorMode {
        self.color_mode
    }

    pub fn color_mode_code(&self) -> i32 {
        self.color_mode.code()
    }

    pub fn get_color_mode(&self) -> i32 {
        self.color_mode_code()
    }

    pub fn colormode(&self) -> i32 {
        self.color_mode_code()
    }

    pub fn set_color_mode(&mut self, value: MapColorMode) {
        self.color_mode = value;
        self.invalidate_display_cache();
    }

    pub fn set_color_mode_code(&mut self, value: i32) {
        self.color_mode = match value {
            Self::MAPREND_MOLFANC => MapColorMode::Molecule,
            Self::MAPREND_MULTIGRAD => MapColorMode::MultiGradient,
            _ => MapColorMode::Solid,
        };
        self.invalidate_display_cache();
    }

    pub fn set_colormode(&mut self, value: i32) {
        self.set_color_mode_code(value);
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn get_color(&self) -> &SolidColor {
        self.color()
    }

    pub fn defaultcolor(&self) -> &SolidColor {
        self.color()
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn set_defaultcolor(&mut self, value: SolidColor) {
        self.set_color(value);
    }

    pub fn siglevel(&self) -> f64 {
        self.siglevel
    }

    pub fn get_sig_level(&self) -> f64 {
        self.siglevel()
    }

    pub fn set_siglevel(&mut self, value: f64) {
        self.siglevel = value;
        self.invalidate_display_cache();
    }

    pub fn set_sig_level(&mut self, value: f64) {
        self.set_siglevel(value);
    }

    pub fn extent(&self) -> f64 {
        self.extent
    }

    pub fn get_extent(&self) -> f64 {
        self.extent()
    }

    pub fn set_extent(&mut self, value: f64) {
        self.extent = value;
        self.invalidate_display_cache();
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn get_center(&self) -> Vector4 {
        self.center()
    }

    pub fn set_center(&mut self, value: Vector4) {
        self.center = value;
        self.invalidate_display_cache();
    }

    pub fn use_pbc(&self) -> bool {
        self.use_pbc
    }

    pub fn is_use_pbc(&self) -> bool {
        self.use_pbc()
    }

    pub fn set_use_pbc(&mut self, value: bool) {
        self.use_pbc = value;
        self.invalidate_display_cache();
    }

    pub fn use_abslevel(&self) -> bool {
        self.use_abslevel
    }

    pub fn is_use_abs_lev(&self) -> bool {
        self.use_abslevel()
    }

    pub fn set_use_abslevel(&mut self, value: bool) {
        self.use_abslevel = value;
    }

    pub fn set_use_abs_lev(&mut self, value: bool) {
        self.set_use_abslevel(value);
    }

    fn grid_range(&self, scalar: &impl ScalarObject) -> Option<GridRange> {
        GridRange::from_center_extent(scalar, self.center, self.extent, self.use_pbc)
    }

    pub fn bndry_rng(&self) -> f64 {
        self.bndry_rng
    }

    pub fn get_bndry_rng(&self) -> f64 {
        self.bndry_rng()
    }

    pub fn set_bndry_rng(&mut self, value: f64) {
        self.bndry_rng = value.max(0.0);
        self.invalidate_display_cache();
    }

    pub fn bndry_molname(&self) -> &str {
        &self.bndry_molname
    }

    pub fn get_bndry_mol_name(&self) -> &str {
        self.bndry_molname()
    }

    pub fn set_bndry_molname(&mut self, value: impl Into<String>) {
        let value = value.into();
        if self.bndry_molname != value {
            self.bndry_molname = value;
            self.invalidate_display_cache();
        }
    }

    pub fn set_bndry_mol_name(&mut self, value: impl Into<String>) {
        self.set_bndry_molname(value);
    }

    pub fn bndry_sel(&self) -> Option<&Selector> {
        self.bndry_sel.as_ref()
    }

    pub fn get_bndry_sel(&self) -> Option<&Selector> {
        self.bndry_sel()
    }

    pub fn set_bndry_sel(&mut self, value: Option<Selector>) {
        if self.bndry_sel != value {
            self.bndry_sel = value;
            self.invalidate_display_cache();
        }
    }

    pub fn multi_grad(&self) -> &MultiGradient {
        &self.multi_grad
    }

    pub fn get_multi_grad(&self) -> &MultiGradient {
        self.multi_grad()
    }

    pub fn multi_grad_mut(&mut self) -> &mut MultiGradient {
        &mut self.multi_grad
    }

    pub fn color_mapname(&self) -> &str {
        &self.color_mapname
    }

    pub fn get_color_map_name(&self) -> &str {
        self.color_mapname()
    }

    pub fn get_color_map_obj(&self) -> Option<&MultiGradient> {
        Some(&self.multi_grad)
    }

    pub fn set_color_mapname(&mut self, value: impl Into<String>) {
        self.color_mapname = value.into();
        self.invalidate_display_cache();
    }

    pub fn set_color_map_name(&mut self, value: impl Into<String>) {
        self.set_color_mapname(value);
    }

    pub fn attach_client_scalar(&mut self, scalar: &impl ScalarObject) {
        self.client_info = Some(MapClientInfo::from_scalar(scalar));
        self.invalidate_display_cache();
    }

    pub fn clear_client_scalar(&mut self) {
        self.client_info = None;
        self.invalidate_display_cache();
    }

    pub fn level(&self) -> f64 {
        self.client_info.map_or(0.0, |client| {
            client
                .histogram
                .as_ref()
                .map_or(self.siglevel * client.rmsd_density, |hist| {
                    fraction_level(
                        hist,
                        client.level_base,
                        client.level_step,
                        client.min_density,
                        client.max_density,
                        self.siglevel / 100.,
                    )
                })
        })
    }

    pub fn level_from(&self, scalar: &impl ScalarObject) -> f64 {
        scalar.percent_level_histogram().as_ref().map_or(
            self.siglevel * scalar.rmsd_density(),
            |hist| {
                fraction_level(
                    hist,
                    scalar.level_base(),
                    scalar.level_step(),
                    scalar.min_density(),
                    scalar.max_density(),
                    self.siglevel / 100.,
                )
            },
        )
    }

    pub fn get_level(&self) -> f64 {
        self.level()
    }

    pub fn set_level(&mut self, value: f64) {
        if let Some(client) = self.client_info {
            if let Some(hist) = client.histogram {
                self.set_siglevel(
                    level_fraction(&hist, client.level_base, client.level_step, value) * 100.,
                );
                return;
            }
        }
        let rmsd = self.client_info.map_or(0.0, |client| client.rmsd_density);
        self.set_siglevel(if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            value / rmsd
        });
    }

    pub fn set_level_from(&mut self, scalar: &impl ScalarObject, value: f64) {
        if let Some(hist) = scalar.percent_level_histogram() {
            self.set_siglevel(
                level_fraction(&hist, scalar.level_base(), scalar.level_step(), value) * 100.,
            );
            return;
        }
        let rmsd = scalar.rmsd_density();
        self.set_siglevel(if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            value / rmsd
        });
    }

    pub fn max_level(&self) -> f64 {
        if self.client_info.is_some_and(|c| c.histogram.is_some()) {
            return 100.;
        }
        self.client_info.map_or(0.0, |client| {
            if client.rmsd_density.abs() < 1.0e-12 {
                0.0
            } else {
                client.max_density / client.rmsd_density
            }
        })
    }

    pub fn max_level_from(&self, scalar: &impl ScalarObject) -> f64 {
        if scalar.percent_level_histogram().is_some() {
            return 100.;
        }
        let rmsd = scalar.rmsd_density();
        if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            scalar.max_density() / rmsd
        }
    }

    pub fn get_max_level(&self) -> f64 {
        self.max_level()
    }

    pub fn min_level(&self) -> f64 {
        if self.client_info.is_some_and(|c| c.histogram.is_some()) {
            return 0.;
        }
        self.client_info.map_or(0.0, |client| {
            if client.rmsd_density.abs() < 1.0e-12 {
                0.0
            } else {
                client.min_density / client.rmsd_density
            }
        })
    }

    pub fn min_level_from(&self, scalar: &impl ScalarObject) -> f64 {
        if scalar.percent_level_histogram().is_some() {
            return 0.;
        }
        let rmsd = scalar.rmsd_density();
        if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            scalar.min_density() / rmsd
        }
    }

    pub fn get_min_level(&self) -> f64 {
        self.min_level()
    }

    pub fn display_cache_valid(&self) -> bool {
        self.disp_list.is_display_cache_valid()
    }

    pub fn mark_display_cache_valid(&mut self) {
        self.disp_list.mark_display_cache_valid();
    }

    pub fn invalidate_display_cache(&mut self) {
        self.disp_list.invalidate_display_cache();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapMeshRenderer {
    map: MapRenderer,
    width: f64,
    bufsize: i32,
    autoupdate: bool,
    dragupdate: bool,
}

impl Default for MapMeshRenderer {
    fn default() -> Self {
        Self {
            map: MapRenderer::new(),
            width: 1.0,
            bufsize: 100,
            autoupdate: true,
            dragupdate: false,
        }
    }
}

impl MapMeshRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "contour"
    }

    pub fn map(&self) -> &MapRenderer {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut MapRenderer {
        &mut self.map
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_line_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.map.invalidate_display_cache();
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.set_width(value);
    }

    pub fn bufsize(&self) -> i32 {
        self.bufsize
    }

    pub fn get_buf_size(&self) -> i32 {
        self.bufsize()
    }

    pub fn set_bufsize(&mut self, value: i32) {
        self.bufsize = value.max(10);
        self.map.invalidate_display_cache();
    }

    pub fn set_buf_size(&mut self, value: i32) {
        self.set_bufsize(value);
    }

    pub fn autoupdate(&self) -> bool {
        self.autoupdate
    }

    pub fn set_autoupdate(&mut self, value: bool) {
        self.autoupdate = value;
    }

    pub fn dragupdate(&self) -> bool {
        self.dragupdate
    }

    pub fn set_dragupdate(&mut self, value: bool) {
        self.dragupdate = value;
    }

    pub fn max_extent(&self) -> f64 {
        let Some(client) = self.map.client_info else {
            return 0.0;
        };
        let half = self.bufsize as f64 / 2.0;
        (half * client.grid_sizes[0])
            .min(half * client.grid_sizes[1])
            .min(half * client.grid_sizes[2])
    }

    pub fn max_extent_from(&self, scalar: &impl ScalarObject) -> f64 {
        let half = self.bufsize as f64 / 2.0;
        (half * scalar.col_grid_size())
            .min(half * scalar.row_grid_size())
            .min(half * scalar.sec_grid_size())
    }

    pub fn get_max_extent(&self) -> f64 {
        self.max_extent()
    }

    /// Contour the density into wireframe line segments at the renderer's level
    /// (the classic "chickenwire" map mesh). Ports C++ `MapMeshRenderer`: the
    /// isosurface `density == level` is intersected with the three families of
    /// axis-aligned grid planes, each plane run through 2D marching squares. The
    /// result is `[start, end]` Cartesian segments (consumed by line drawing /
    /// OBJ `l` export), the wireframe analogue of `MapSurfRenderer`'s filled mesh.
    pub fn generate_contour_lines(&self, scalar: &impl ScalarObject) -> Vec<[Vector4; 2]> {
        self.generate_contour_lines_with_boundary(scalar, None)
    }

    pub fn generate_contour_lines_with_boundary(
        &self,
        scalar: &impl ScalarObject,
        boundary_points: Option<&[Vector4]>,
    ) -> Vec<[Vector4; 2]> {
        let level = self.map.level_from(scalar);
        let Some(mut range) = self.map.grid_range(scalar) else {
            return Vec::new();
        };
        range.limit_node_counts([self.bufsize as usize; 3]);
        let [nc, nr, ns] = range.node_counts();
        if nc < 2 || nr < 2 || ns < 2 {
            return Vec::new();
        }

        // Signed field at relative node (i,j,k): negative where density exceeds
        // the level (so the f=0 isoline is the contour).
        let boundary = BoundaryFilter::new(boundary_points, self.map.bndry_rng());
        let node = |i: usize, j: usize, k: usize| -> (f64, Vector4, bool) {
            let logical = [
                i as isize + range.start[0],
                j as isize + range.start[1],
                k as isize + range.start[2],
            ];
            let [i, j, k] = range.map_indices(scalar, logical);
            let value = level - scalar.at_float(i, j, k);
            let [i, j, k] = logical;
            let pos = scalar.conv_to_orth(Vector4::new3(i as f64, j as f64, k as f64));
            (value, pos, boundary.contains(pos))
        };

        let mut out = Vec::new();
        // XY planes (constant k).
        for k in 0..ns {
            for j in 0..nr - 1 {
                for i in 0..nc - 1 {
                    march_squares_cell(
                        [
                            node(i, j, k),
                            node(i + 1, j, k),
                            node(i + 1, j + 1, k),
                            node(i, j + 1, k),
                        ],
                        &mut out,
                    );
                }
            }
        }
        // YZ planes (constant i).
        for i in 0..nc {
            for k in 0..ns - 1 {
                for j in 0..nr - 1 {
                    march_squares_cell(
                        [
                            node(i, j, k),
                            node(i, j + 1, k),
                            node(i, j + 1, k + 1),
                            node(i, j, k + 1),
                        ],
                        &mut out,
                    );
                }
            }
        }
        // XZ planes (constant j).
        for j in 0..nr {
            for k in 0..ns - 1 {
                for i in 0..nc - 1 {
                    march_squares_cell(
                        [
                            node(i, j, k),
                            node(i + 1, j, k),
                            node(i + 1, j, k + 1),
                            node(i, j, k + 1),
                        ],
                        &mut out,
                    );
                }
            }
        }
        out
    }
}

/// Export contour line segments as a Wavefront OBJ string (`v` vertices + `l`
/// line elements) — a real, non-GPU consumer of [`MapMeshRenderer::generate_contour_lines`],
/// the wireframe analogue of the surface mesh's OBJ export.
pub fn contour_lines_to_obj(segments: &[[Vector4; 2]]) -> String {
    let mut obj = String::from("# CueMol density contour wireframe\n");
    for seg in segments {
        for p in seg {
            obj.push_str(&format!("v {} {} {}\n", p.x, p.y, p.z));
        }
    }
    for index in 0..segments.len() {
        let a = index * 2 + 1; // OBJ indices are 1-based
        obj.push_str(&format!("l {} {}\n", a, a + 1));
    }
    obj
}

/// Emit the marching-squares isoline segments (`f = 0`) for one quad cell.
/// Corners are in quad order c00, c10, c11, c01; each is
/// `(signed value, pos, boundary-valid)`
/// with `value < 0` inside. Edges: 0 = c00-c10, 1 = c10-c11, 2 = c11-c01,
/// 3 = c01-c00.
fn march_squares_cell(corners: [(f64, Vector4, bool); 4], out: &mut Vec<[Vector4; 2]>) {
    // Segment table: for each of the 16 inside/outside cases, the pairs of edges
    // the isoline connects (saddle cases 5 and 10 emit two segments).
    const EDGE_PAIRS: [&[(u8, u8)]; 16] = [
        &[],
        &[(3, 0)],
        &[(0, 1)],
        &[(3, 1)],
        &[(1, 2)],
        &[(3, 0), (1, 2)],
        &[(0, 2)],
        &[(2, 3)],
        &[(2, 3)],
        &[(0, 2)],
        &[(0, 1), (2, 3)],
        &[(1, 2)],
        &[(1, 3)],
        &[(0, 1)],
        &[(0, 3)],
        &[],
    ];
    const EDGE_CORNERS: [(usize, usize); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];

    let mut case = 0usize;
    for (c, corner) in corners.iter().enumerate() {
        if corner.0 < 0.0 {
            case |= 1 << c;
        }
    }
    let pairs = EDGE_PAIRS[case];
    if pairs.is_empty() {
        return;
    }

    let crossing = |edge: u8| -> Option<Vector4> {
        let (a, b) = EDGE_CORNERS[edge as usize];
        let (va, pa, valid_a) = corners[a];
        let (vb, pb, valid_b) = corners[b];
        if !valid_a || !valid_b {
            return None;
        }
        let denom = va - vb;
        let t = if denom.abs() < 1.0e-12 {
            0.5
        } else {
            (va / denom).clamp(0.0, 1.0)
        };
        Some(Vector4::new3(
            pa.x + (pb.x - pa.x) * t,
            pa.y + (pb.y - pa.y) * t,
            pa.z + (pb.z - pa.z) * t,
        ))
    };

    for &(ea, eb) in pairs {
        if let (Some(a), Some(b)) = (crossing(ea), crossing(eb)) {
            out.push([a, b]);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSurfaceDrawMode {
    Fill,
    Line,
    Point,
}

impl MapSurfaceDrawMode {
    pub fn code(self) -> i32 {
        match self {
            Self::Fill => MapSurfRenderer::MSRDRAW_FILL,
            Self::Line => MapSurfRenderer::MSRDRAW_LINE,
            Self::Point => MapSurfRenderer::MSRDRAW_POINT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapSurfRenderer {
    map: MapRenderer,
    client_map: Option<Arc<DensityMap>>,
    autoupdate: bool,
    dragupdate: bool,
    cullface: bool,
    drawmode: MapSurfaceDrawMode,
    width: f64,
    binning: i32,
    max_grids: i32,
    glrender_mode: i32,
    omp_thr: i32,
}

impl Default for MapSurfRenderer {
    fn default() -> Self {
        Self {
            map: MapRenderer::new(),
            client_map: None,
            autoupdate: true,
            dragupdate: false,
            cullface: false,
            drawmode: MapSurfaceDrawMode::Fill,
            width: 1.2,
            binning: 1,
            max_grids: 100,
            glrender_mode: Self::MSR_REND_DLIST,
            omp_thr: -1,
        }
    }
}

impl MapSurfRenderer {
    pub const MSRDRAW_FILL: i32 = 0;
    pub const MSRDRAW_LINE: i32 = 1;
    pub const MSRDRAW_POINT: i32 = 2;
    pub const MSR_REND_DLIST: i32 = 0;
    pub const MSR_REND_VBO: i32 = 1;
    pub const MSR_REND_SHADER: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "isosurf"
    }

    pub fn map(&self) -> &MapRenderer {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut MapRenderer {
        &mut self.map
    }

    pub fn client_map(&self) -> Option<&DensityMap> {
        self.client_map.as_deref()
    }

    pub fn attach_client_map(&mut self, map: Arc<DensityMap>) {
        self.map.attach_client_scalar(map.as_ref());
        self.client_map = Some(map);
    }

    pub fn set_client_map(&mut self, map: DensityMap) {
        self.attach_client_map(Arc::new(map));
    }

    pub fn clear_client_map(&mut self) {
        self.client_map = None;
        self.map.clear_client_scalar();
    }

    pub fn autoupdate(&self) -> bool {
        self.autoupdate
    }

    pub fn set_autoupdate(&mut self, value: bool) {
        self.autoupdate = value;
    }

    pub fn dragupdate(&self) -> bool {
        self.dragupdate
    }

    pub fn set_dragupdate(&mut self, value: bool) {
        self.dragupdate = value;
    }

    pub fn cullface(&self) -> bool {
        self.cullface
    }

    pub fn is_cull_face(&self) -> bool {
        self.cullface()
    }

    pub fn set_cullface(&mut self, value: bool) {
        self.cullface = value;
        self.map.invalidate_display_cache();
    }

    pub fn set_cull_face(&mut self, value: bool) {
        self.set_cullface(value);
    }

    pub fn drawmode(&self) -> MapSurfaceDrawMode {
        self.drawmode
    }

    pub fn drawmode_code(&self) -> i32 {
        self.drawmode.code()
    }

    pub fn get_draw_mode(&self) -> i32 {
        self.drawmode_code()
    }

    pub fn set_drawmode(&mut self, value: MapSurfaceDrawMode) {
        self.drawmode = value;
        self.map.invalidate_display_cache();
    }

    pub fn set_draw_mode(&mut self, value: i32) {
        self.set_drawmode(match value {
            Self::MSRDRAW_LINE => MapSurfaceDrawMode::Line,
            Self::MSRDRAW_POINT => MapSurfaceDrawMode::Point,
            _ => MapSurfaceDrawMode::Fill,
        });
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_line_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.map.invalidate_display_cache();
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.set_width(value);
    }

    pub fn binning(&self) -> i32 {
        self.binning
    }

    pub fn get_bin_fac(&self) -> i32 {
        self.binning()
    }

    pub fn set_binning(&mut self, value: i32) {
        self.binning = value.max(1);
        self.map.invalidate_display_cache();
    }

    pub fn set_bin_fac(&mut self, value: i32) {
        self.set_binning(value);
    }

    pub fn max_grids(&self) -> i32 {
        self.max_grids
    }

    pub fn get_max_grids(&self) -> i32 {
        self.max_grids()
    }

    pub fn set_max_grids(&mut self, value: i32) {
        self.max_grids = value.max(1);
        self.map.invalidate_display_cache();
    }

    pub fn max_extent(&self) -> f64 {
        self.map.client_info.map_or(0.0, |client| {
            self.max_grids as f64 * client.grid_sizes[0] / 2.0
        })
    }

    pub fn max_extent_from(&self, scalar: &impl ScalarObject) -> f64 {
        self.max_grids as f64 * scalar.col_grid_size() / 2.0
    }

    pub fn get_max_extent(&self) -> f64 {
        self.max_extent()
    }

    /// Contour the density into a triangle isosurface at the renderer's level.
    ///
    /// Ports C++ `MapSurfRenderer`: the surface is the iso-level set of the
    /// electron density, `density(p) == level` where `level = siglevel * sigma`.
    /// The mesh is extracted by marching the map's own grid (corners are grid
    /// nodes, crossings are mapped to Cartesian via the map transform), reusing
    /// the shared marching-tetrahedra core in `cuemol-surface`. This is the
    /// visual consumer of the MTZ/mmCIF -> density pipeline.
    pub fn generate_surf_obj(&self) -> MolSurfObj {
        self.client_map
            .as_ref()
            .map_or_else(MolSurfObj::new, |map| {
                self.generate_surf_obj_from(map.as_ref())
            })
    }

    pub fn generate_surf_obj_from(&self, scalar: &impl ScalarObject) -> MolSurfObj {
        self.generate_surf_obj_with_boundary(scalar, None)
    }

    pub fn generate_surf_obj_with_boundary(
        &self,
        scalar: &impl ScalarObject,
        boundary_points: Option<&[Vector4]>,
    ) -> MolSurfObj {
        let mut object = MolSurfObj::new();
        let level = self.map.level_from(scalar);
        let extent = self.map.extent().min(self.max_extent_from(scalar));
        let Some(range) =
            GridRange::from_center_extent(scalar, self.map.center(), extent, self.map.use_pbc())
        else {
            return object;
        };
        build_density_isosurface(
            object.mesh_mut(),
            scalar,
            level,
            range,
            self.binning as usize,
            BoundaryFilter::new(boundary_points, self.map.bndry_rng()),
        );
        object
    }

    pub fn glrender_mode(&self) -> i32 {
        self.glrender_mode
    }

    pub fn get_gl_render_mode(&self) -> i32 {
        self.glrender_mode()
    }

    pub fn set_glrender_mode(&mut self, value: i32) {
        self.map.invalidate_display_cache();
        self.glrender_mode = value;
    }

    pub fn set_gl_render_mode(&mut self, value: i32) {
        self.set_glrender_mode(value);
    }

    pub fn omp_thr(&self) -> i32 {
        self.omp_thr
    }

    pub fn get_omp_thr(&self) -> i32 {
        self.omp_thr()
    }

    pub fn set_omp_thr(&mut self, value: i32) {
        self.omp_thr = value;
        self.map.invalidate_display_cache();
    }
}

/// Views a scalar map as the signed field `g(p) = level - density(p)` so that
/// the high-density region (`density > level`) is the interior (`g < 0`) of the
/// contour. The outward gradient is `-grad(density)` (high density to low),
/// taken from grid central differences of the trilinearly interpolated density
/// so vertex normals stay smooth instead of faceted.
///
/// Cartesian indices and gradients use the full cell basis, including skew cells.
struct DensityField<'a, S: ScalarObject> {
    map: &'a S,
    origin: Vector4,
    orth_to_grid: Matrix3,
    periodic: bool,
}

impl<'a, S: ScalarObject> DensityField<'a, S> {
    fn new(map: &'a S, periodic: bool) -> Self {
        let origin = map.conv_to_orth(Vector4::ZERO);
        let axes = [
            map.conv_to_orth(Vector4::new3(1.0, 0.0, 0.0)) - origin,
            map.conv_to_orth(Vector4::new3(0.0, 1.0, 0.0)) - origin,
            map.conv_to_orth(Vector4::new3(0.0, 0.0, 1.0)) - origin,
        ];
        Self {
            map,
            origin,
            orth_to_grid: inverse_grid_basis(axes).unwrap_or(Matrix3::IDENTITY),
            periodic,
        }
    }

    fn cont_index(&self, p: Vector4) -> [f64; 3] {
        let index = self.orth_to_grid.mul_vec(p - self.origin);
        [index.x, index.y, index.z]
    }

    fn node_density(&self, i: isize, j: isize, k: isize) -> f64 {
        let mut index = [i, j, k];
        if self.periodic {
            let counts = [self.map.col_no(), self.map.row_no(), self.map.sec_no()];
            for axis in 0..3 {
                index[axis] = index[axis].rem_euclid(counts[axis] as isize);
            }
        }
        self.map.at_float(index[0], index[1], index[2])
    }

    /// Trilinear density at a continuous (relative) grid index.
    fn density_at(&self, idx: [f64; 3]) -> f64 {
        let x0 = idx[0].floor();
        let y0 = idx[1].floor();
        let z0 = idx[2].floor();
        let fx = idx[0] - x0;
        let fy = idx[1] - y0;
        let fz = idx[2] - z0;
        let (i0, j0, k0) = (x0 as isize, y0 as isize, z0 as isize);
        let n = |di: isize, dj: isize, dk: isize| self.node_density(i0 + di, j0 + dj, k0 + dk);
        let c00 = n(0, 0, 0) * (1.0 - fx) + n(1, 0, 0) * fx;
        let c10 = n(0, 1, 0) * (1.0 - fx) + n(1, 1, 0) * fx;
        let c01 = n(0, 0, 1) * (1.0 - fx) + n(1, 0, 1) * fx;
        let c11 = n(0, 1, 1) * (1.0 - fx) + n(1, 1, 1) * fx;
        let c0 = c00 * (1.0 - fy) + c10 * fy;
        let c1 = c01 * (1.0 - fy) + c11 * fy;
        c0 * (1.0 - fz) + c1 * fz
    }
}

impl<S: ScalarObject> ScalarField for DensityField<'_, S> {
    fn gradient(&self, p: Vector4) -> Vector4 {
        let idx = self.cont_index(p);
        // Central difference of the density along each grid axis (one node step
        // either side), mapped to a Cartesian vector by the axis directions.
        // Outward = toward decreasing density = -grad(density).
        let mut gradient = [0.; 3];
        for axis in 0..3 {
            let mut lo = idx;
            let mut hi = idx;
            lo[axis] -= 1.0;
            hi[axis] += 1.0;
            let d_dens = self.density_at(hi) - self.density_at(lo);
            gradient[axis] = -0.5 * d_dens;
        }
        Matrix3::new(std::array::from_fn(|row| {
            std::array::from_fn(|col| self.orth_to_grid.values[col][row])
        }))
        .mul_vec(Vector4::new3(gradient[0], gradient[1], gradient[2]))
    }
}

fn inverse_grid_basis(axes: [Vector4; 3]) -> Option<Matrix3> {
    Matrix3::new([
        [axes[0].x, axes[1].x, axes[2].x],
        [axes[0].y, axes[1].y, axes[2].y],
        [axes[0].z, axes[1].z, axes[2].z],
    ])
    .invert()
    .ok()
}

#[derive(Debug, Clone, Copy)]
struct BoundaryFilter<'a> {
    points: Option<&'a [Vector4]>,
    range_sq: f64,
}

impl<'a> BoundaryFilter<'a> {
    fn new(points: Option<&'a [Vector4]>, range: f64) -> Self {
        Self {
            points,
            range_sq: range.max(0.0).powi(2),
        }
    }

    fn contains(self, position: Vector4) -> bool {
        self.points.is_none_or(|points| {
            points.iter().any(|point| {
                let delta = position - *point;
                delta.dot3d(delta) <= self.range_sq
            })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridRange {
    /// Relative grid-node index, inclusive.
    start: [isize; 3],
    /// Relative grid-node index, exclusive.
    end: [isize; 3],
    periodic: bool,
}

impl GridRange {
    fn from_center_extent(
        map: &impl ScalarObject,
        center: Vector4,
        extent: f64,
        use_pbc: bool,
    ) -> Option<Self> {
        if !extent.is_finite() || extent <= 0.0 {
            return None;
        }

        let grid_origin = map.origin();
        let relative_origin = map.conv_to_orth(Vector4::ZERO);
        let axes = [
            map.conv_to_orth(Vector4::new3(1.0, 0.0, 0.0)) - relative_origin,
            map.conv_to_orth(Vector4::new3(0.0, 1.0, 0.0)) - relative_origin,
            map.conv_to_orth(Vector4::new3(0.0, 0.0, 1.0)) - relative_origin,
        ];
        let orth_to_grid = Matrix3::new([
            [axes[0].x, axes[1].x, axes[2].x],
            [axes[0].y, axes[1].y, axes[2].y],
            [axes[0].z, axes[1].z, axes[2].z],
        ])
        .invert()
        .ok()?;
        let lower_orth = center - Vector4::new3(extent, extent, extent);
        let upper_orth = center + Vector4::new3(extent, extent, extent);
        let first = orth_to_grid.mul_vec(lower_orth - grid_origin);
        let second = orth_to_grid.mul_vec(upper_orth - grid_origin);
        let grid_lower = [
            first.x.min(second.x),
            first.y.min(second.y),
            first.z.min(second.z),
        ];
        let grid_upper = [
            first.x.max(second.x),
            first.y.max(second.y),
            first.z.max(second.z),
        ];
        let map_start = [map.start_col(), map.start_row(), map.start_sec()];
        let map_counts = [map.col_no(), map.row_no(), map.sec_no()];
        let periodic = use_pbc && map.is_periodic_grid();
        let mut start = [0; 3];
        let mut end = [0; 3];
        for axis in 0..3 {
            let available_end = map_start[axis].saturating_add_unsigned(map_counts[axis]);
            let (lower, upper) = if periodic {
                (
                    grid_lower[axis].floor() as isize,
                    grid_upper[axis].floor() as isize,
                )
            } else {
                (
                    (grid_lower[axis].max(map_start[axis] as f64).floor() as isize)
                        .clamp(map_start[axis], available_end),
                    (grid_upper[axis].min(available_end as f64).floor() as isize)
                        .clamp(map_start[axis], available_end),
                )
            };
            start[axis] = lower.saturating_sub(map_start[axis]);
            end[axis] = upper.saturating_sub(map_start[axis]);
            if end[axis] <= start[axis] {
                return None;
            }
        }
        Some(Self {
            start,
            end,
            periodic,
        })
    }

    fn node_counts(self) -> [usize; 3] {
        [
            self.end[0].saturating_sub(self.start[0]) as usize,
            self.end[1].saturating_sub(self.start[1]) as usize,
            self.end[2].saturating_sub(self.start[2]) as usize,
        ]
    }

    fn limit_node_counts(&mut self, limits: [usize; 3]) {
        for (axis, limit) in limits.into_iter().enumerate() {
            let count = self.end[axis].saturating_sub(self.start[axis]) as usize;
            if count <= limit {
                continue;
            }
            let remove = count - limit;
            self.start[axis] += (remove / 2) as isize;
            self.end[axis] -= (remove - remove / 2) as isize;
        }
    }

    fn with_upper_boundary_nodes(mut self, map: &impl ScalarObject) -> Self {
        let counts = [map.col_no(), map.row_no(), map.sec_no()];
        for (end, count) in self.end.iter_mut().zip(counts) {
            *end = end.saturating_add(1);
            if !self.periodic {
                *end = (*end).min(count as isize);
            }
        }
        self
    }

    fn map_indices(self, map: &impl ScalarObject, mut indices: [isize; 3]) -> [isize; 3] {
        if self.periodic {
            let counts = [map.col_no(), map.row_no(), map.sec_no()];
            for axis in 0..3 {
                indices[axis] = indices[axis].rem_euclid(counts[axis] as isize);
            }
        }
        indices
    }
}

/// Extract the `density == level` isosurface of `map` into `mesh` by marching
/// the selected map grid. Cube corners are grid nodes (values taken exactly
/// from `at_float`, no resampling), and crossings are mapped to Cartesian via
/// `conv_to_orth`. Ports the geometry half of C++ `MapSurfRenderer::marchCube`.
fn build_density_isosurface<S: ScalarObject>(
    mesh: &mut SurfaceMesh,
    map: &S,
    level: f64,
    range: GridRange,
    binning: usize,
    boundary: BoundaryFilter<'_>,
) {
    let range = range.with_upper_boundary_nodes(map);
    let sample_axis = |axis: usize| {
        let mut indices = Vec::new();
        let last = range.end[axis] - 1;
        let mut index = range.start[axis];
        loop {
            indices.push(index);
            if index == last {
                break;
            }
            index = index.saturating_add_unsigned(binning).min(last);
        }
        indices
    };
    let indices = [sample_axis(0), sample_axis(1), sample_axis(2)];
    let dims = [indices[0].len(), indices[1].len(), indices[2].len()];
    if dims[0] < 2 || dims[1] < 2 || dims[2] < 2 {
        return;
    }
    let field = DensityField::new(map, range.periodic);
    let spacing = map
        .col_grid_size()
        .min(map.row_grid_size())
        .min(map.sec_grid_size())
        .max(1.0e-6);

    build_grid_isosurface_masked(
        mesh,
        &field,
        dims,
        // Signed field at node (i,j,k): negative where density exceeds the level.
        |i, j, k| {
            let logical = [indices[0][i], indices[1][j], indices[2][k]];
            let [i, j, k] = range.map_indices(map, logical);
            level - map.at_float(i, j, k)
        },
        // Grid node -> Cartesian (relative index; conv_to_orth adds the start).
        |i, j, k| {
            map.conv_to_orth(Vector4::new3(
                indices[0][i] as f64,
                indices[1][j] as f64,
                indices[2][k] as f64,
            ))
        },
        |i, j, k| {
            boundary.contains(map.conv_to_orth(Vector4::new3(
                indices[0][i] as f64,
                indices[1][j] as f64,
                indices[2][k] as f64,
            )))
        },
        spacing * binning as f64,
    );
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlslMapMeshRenderer {
    map: MapRenderer,
    width: f64,
    bufsize: i32,
    autoupdate: bool,
    dragupdate: bool,
    shader_checked: bool,
    map_texture_ok: bool,
}

impl Default for GlslMapMeshRenderer {
    fn default() -> Self {
        let mut map = MapRenderer::new();
        map.set_color(SolidColor::from_rgb(0.0, 1.0, 0.0));
        Self {
            map,
            width: 1.0,
            bufsize: 100,
            autoupdate: true,
            dragupdate: true,
            shader_checked: false,
            map_texture_ok: false,
        }
    }
}

impl GlslMapMeshRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "gpu_mapmesh"
    }

    pub fn map(&self) -> &MapRenderer {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut MapRenderer {
        &mut self.map
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_line_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.invalidate_display_cache();
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.set_width(value);
    }

    pub fn bufsize(&self) -> i32 {
        self.bufsize
    }

    pub fn get_buf_size(&self) -> i32 {
        self.bufsize()
    }

    pub fn set_bufsize(&mut self, value: i32) {
        self.bufsize = value;
    }

    pub fn set_buf_size(&mut self, value: i32) {
        self.set_bufsize(value);
    }

    pub fn autoupdate(&self) -> bool {
        self.autoupdate
    }

    pub fn set_autoupdate(&mut self, value: bool) {
        self.autoupdate = value;
    }

    pub fn dragupdate(&self) -> bool {
        self.dragupdate
    }

    pub fn set_dragupdate(&mut self, value: bool) {
        self.dragupdate = value;
    }

    pub fn shader_checked(&self) -> bool {
        self.shader_checked
    }

    pub fn set_shader_checked(&mut self, value: bool) {
        self.shader_checked = value;
    }

    pub fn map_texture_ok(&self) -> bool {
        self.map_texture_ok
    }

    pub fn set_map_texture_ok(&mut self, value: bool) {
        self.map_texture_ok = value;
    }

    pub fn max_extent(&self) -> f64 {
        let Some(client) = self.map.client_info else {
            return 0.0;
        };
        (50.0 * client.grid_sizes[0])
            .min(50.0 * client.grid_sizes[1])
            .min(50.0 * client.grid_sizes[2])
    }

    pub fn max_extent_from(&self, scalar: &impl ScalarObject) -> f64 {
        (50.0 * scalar.col_grid_size())
            .min(50.0 * scalar.row_grid_size())
            .min(50.0 * scalar.sec_grid_size())
    }

    pub fn get_max_extent(&self) -> f64 {
        self.max_extent()
    }

    pub fn invalidate_display_cache(&mut self) {
        self.map.invalidate_display_cache();
        self.map_texture_ok = false;
    }
}

pub type GlslMapMeshRenderer2 = GlslMapMeshRenderer;

#[derive(Debug, Clone, PartialEq)]
pub struct GlslMapVolRenderer {
    map: MapRenderer,
    autoupdate: bool,
    dragupdate: bool,
    xferfn_type: i32,
    shader_checked: bool,
    map_texture_ok: bool,
}

impl Default for GlslMapVolRenderer {
    fn default() -> Self {
        let mut map = MapRenderer::new();
        map.set_color(SolidColor::from_rgb(0.0, 1.0, 0.0));
        Self {
            map,
            autoupdate: true,
            dragupdate: true,
            xferfn_type: Self::GLMV_AUTO1,
            shader_checked: false,
            map_texture_ok: false,
        }
    }
}

impl GlslMapVolRenderer {
    pub const GLMV_MULTIGRAD: i32 = 0;
    pub const GLMV_AUTO1: i32 = 1;
    pub const GLMV_AUTO2: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "gpu_mapvol"
    }

    pub fn map(&self) -> &MapRenderer {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut MapRenderer {
        &mut self.map
    }

    pub fn autoupdate(&self) -> bool {
        self.autoupdate
    }

    pub fn set_autoupdate(&mut self, value: bool) {
        self.autoupdate = value;
    }

    pub fn dragupdate(&self) -> bool {
        self.dragupdate
    }

    pub fn set_dragupdate(&mut self, value: bool) {
        self.dragupdate = value;
    }

    pub fn xferfn_type(&self) -> i32 {
        self.xferfn_type
    }

    pub fn get_xfer_fn_type(&self) -> i32 {
        self.xferfn_type()
    }

    pub fn set_xferfn_type(&mut self, value: i32) {
        self.xferfn_type = value;
    }

    pub fn set_xfer_fn_type(&mut self, value: i32) {
        self.set_xferfn_type(value);
    }

    pub fn shader_checked(&self) -> bool {
        self.shader_checked
    }

    pub fn set_shader_checked(&mut self, value: bool) {
        self.shader_checked = value;
    }

    pub fn map_texture_ok(&self) -> bool {
        self.map_texture_ok
    }

    pub fn set_map_texture_ok(&mut self, value: bool) {
        self.map_texture_ok = value;
    }

    pub fn max_extent(&self) -> f64 {
        let Some(client) = self.map.client_info else {
            return 0.0;
        };
        (client.counts[0] as f64 * client.grid_sizes[0] / 2.0)
            .max(client.counts[1] as f64 * client.grid_sizes[1] / 2.0)
            .max(client.counts[2] as f64 * client.grid_sizes[2] / 2.0)
    }

    pub fn max_extent_from(&self, scalar: &impl ScalarObject) -> f64 {
        (scalar.col_no() as f64 * scalar.col_grid_size() / 2.0)
            .max(scalar.row_no() as f64 * scalar.row_grid_size() / 2.0)
            .max(scalar.sec_no() as f64 * scalar.sec_grid_size() / 2.0)
    }

    pub fn get_max_extent(&self) -> f64 {
        self.max_extent()
    }

    pub fn invalidate_display_cache(&mut self) {
        self.map.invalidate_display_cache();
        self.map_texture_ok = false;
    }
}

macro_rules! impl_map_renderer_deref {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl std::ops::Deref for $ty {
                type Target = MapRenderer;

                fn deref(&self) -> &Self::Target {
                    &self.map
                }
            }

            impl std::ops::DerefMut for $ty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.map
                }
            }
        )+
    };
}

impl_map_renderer_deref!(
    MapMeshRenderer,
    MapSurfRenderer,
    GlslMapMeshRenderer,
    GlslMapVolRenderer,
);

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::{parse_selector, read_opendx_str};

    #[test]
    fn skew_cell_scalar_sampling_and_gradients_use_local_grid_indices() {
        let mut values = Vec::new();
        for z in 0..6 {
            for y in 0..6 {
                for x in 0..6 {
                    values.push(x + 2 * y + 4 * z);
                }
            }
        }
        let mut map = DensityMap::new();
        map.set_map_byte_array(&values, [6; 3], 0., 256., 17.5, 1.);
        map.set_map_params([-3, 2, 1], [6; 3]);
        map.set_xtal_params(10., 12., 14., 83., 77., 120.);
        map.set_origin(Vector4::new3(20., -6., 3.));
        assert_eq!(map.at_float(1, 1, 1), 7.);
        let p = map.conv_to_orth(Vector4::new3(1.2, 1.3, 1.4));
        assert_eq!(map.value_at(p), 7.);
        assert!(map.is_in_range(p));
        let field = DensityField::new(&map, false);
        let center = map.conv_to_orth(Vector4::new3(2.5, 2.5, 2.5));
        let gradient = field.gradient(center);
        let origin = map.conv_to_orth(Vector4::ZERO);
        for (direction, expected) in [
            (Vector4::new3(1., 0., 0.), -1.),
            (Vector4::new3(0., 1., 0.), -2.),
            (Vector4::new3(0., 0., 1.), -4.),
        ] {
            let axis = map.conv_to_orth(direction) - origin;
            assert!((gradient.dot3d(axis) - expected).abs() < 1e-10);
        }
        assert_eq!(map.quant_step(), 1.);
        let histogram = map.histogram(0., 256., 256);
        assert_eq!(histogram.values.iter().sum::<f64>(), 216.);
    }

    #[test]
    fn opendx_loads_into_density_map_without_x_z_swap() {
        // End-to-end through the real consumer (read_opendx_str -> from_grid_map ->
        // at_float): an OpenDX 2x2x2 with z-fastest file values 0..7. The voxel
        // (col=1,row=0,sec=0) must read ~4.0 (the file's (1,0,0) value), not ~1.0
        // (which is what a no-transpose load leaves there — the silent x<->z swap).
        // at_float is byte-quantized (level_step 7/256), so ~4.0 vs ~1.0 is the
        // discriminator with ample margin.
        let dx = "\
object 1 class gridpositions counts 2 2 2
origin 0.0 0.0 0.0
delta 1 0 0
delta 0 1 0
delta 0 0 1
object 2 class gridconnections counts 2 2 2
object 3 class array type \"double\" rank 0 items 8 data follows
0 1 2 3 4 5 6 7
attribute \"dep\" string \"positions\"
";
        let map = DensityMap::from_grid_map(read_opendx_str(dx).unwrap());
        assert!(
            (map.at_float(1, 0, 0) - 4.0).abs() < 0.1,
            "OpenDX voxel (1,0,0) should be ~4.0, got {} (a ~1.0 here = the x<->z swap)",
            map.at_float(1, 0, 0)
        );
        assert!((map.at_float(0, 0, 0) - 0.0).abs() < 0.1);
    }

    fn test_grid() -> GridMap {
        GridMap {
            counts: [2, 2, 2],
            origin: Vector4::ZERO,
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 2.0, 0.0),
                Vector4::new3(0.0, 0.0, 3.0),
            ],
            values: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
        }
    }

    #[test]
    fn density_map_from_grid_matches_scalar_object_contract() {
        let map = DensityMap::from_grid_map(test_grid());
        assert_eq!(map.type_name(), "DensityMap");
        assert_eq!(map.col_no(), 2);
        assert_eq!(map.row_no(), 2);
        assert_eq!(map.sec_no(), 2);
        assert_eq!(map.min_density(), 0.0);
        assert_eq!(map.max_density(), 7.0);
        assert_eq!(map.mean_density(), 3.5);
        assert!((map.rmsd_density() - 2.29128784747792).abs() < 1.0e-12);
        assert_eq!(map.level_base(), 0.0);
        assert_eq!(map.level_step(), 7.0 / 256.0);
        assert_eq!(map.at_byte(0, 0, 0), 0);
        assert_eq!(map.at_float(0, 0, 0), 0.0);
        assert!(map.at_float(1, 1, 1) <= 7.0);
        assert!(map.is_in_boundary(1, 1, 1));
        assert!(!map.is_in_boundary(2, 1, 1));
        assert_eq!(
            map.conv_to_orth(Vector4::new3(1.0, 1.0, 1.0)),
            Vector4::new3(1.0, 2.0, 3.0)
        );
        assert!(map.is_in_range(Vector4::new3(1.0, 2.0, 3.0)));
        assert_eq!(map.value_at(Vector4::new3(100.0, 0.0, 0.0)), 0.0);
        assert!(map.histogram_json(0.0, 7.0, 4).contains("\"histo\""));
    }

    #[test]
    fn density_map_from_grid_preserves_cell_angles_from_delta() {
        let mut grid = test_grid();
        grid.delta = [
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(-0.5, 0.8660254037844386, 0.0),
            Vector4::new3(0.0, 0.0, 1.0),
        ];
        let map = DensityMap::from_grid_map(grid);
        let cell = map.cell();
        assert!((cell.a - 2.0).abs() < 1.0e-12);
        assert!((cell.b - 2.0).abs() < 1.0e-12);
        assert!((cell.c - 2.0).abs() < 1.0e-12);
        assert!((cell.alpha - 90.0).abs() < 1.0e-12);
        assert!((cell.beta - 90.0).abs() < 1.0e-12);
        assert!((cell.gamma - 120.0).abs() < 1.0e-10);
    }

    #[test]
    fn crystallographic_grid_preserves_start_and_position_lookup() {
        let start = [3, -2, 5];
        let grid = GridMap {
            counts: [2, 2, 2],
            origin: Vector4::new3(3.0, -4.0, 15.0),
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 2.0, 0.0),
                Vector4::new3(0.0, 0.0, 3.0),
            ],
            values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        };
        let map = DensityMap::from_crystallographic_grid(
            grid,
            start,
            [10, 10, 10],
            [10.0, 20.0, 30.0, 90.0, 90.0, 90.0],
            19,
        );

        assert_eq!([map.start_col(), map.start_row(), map.start_sec()], start);
        assert_eq!(map.intervals(), [10, 10, 10]);
        assert_eq!(map.cell().space_group(), 19);
        let first_position = map.conv_to_orth(Vector4::ZERO);
        assert!((first_position - Vector4::new3(3.0, -4.0, 15.0)).length3d() < 1.0e-12);
        assert!(map.is_in_range(first_position));
        assert!((map.value_at(first_position) - 1.0).abs() < 0.05);
        assert!((map.at_float(0, 0, 0) - 1.0).abs() < 0.05);
        assert!(map.is_in_boundary(0, 0, 0));
        assert!(!map.is_in_boundary(3, -2, 5));
    }

    #[test]
    fn set_xtal_params_updates_non_orthogonal_delta() {
        let mut map = DensityMap::from_grid_map(test_grid());
        map.set_xtal_params(10.0, 12.0, 14.0, 90.0, 90.0, 120.0);

        assert!((map.col_grid_size() - 5.0).abs() < 1.0e-12);
        assert!((map.row_grid_size() - 6.0).abs() < 1.0e-12);
        assert!((map.sec_grid_size() - 7.0).abs() < 1.0e-12);
        let b_axis = map.conv_to_orth(Vector4::new3(0.0, 2.0, 0.0));
        assert!((b_axis.x + 6.0).abs() < 1.0e-10);
        assert!((b_axis.y - 10.392304845413264).abs() < 1.0e-10);
    }

    #[test]
    fn density_map_fit_view_matches_reference_cell_bounds() {
        let mut map = DensityMap::from_grid_map(test_grid());
        map.set_xtal_params(30.0, 10.0, 40.0, 90.0, 90.0, 90.0);
        let mut view = View::new(cuemol_core::Uid::new(1), cuemol_core::Uid::new(1));
        view.set_view_size(200, 100);
        view.set_slab_depth(20.0);

        map.fit_view(&mut view, false);

        let center = view.view_center();
        assert!((center.x - 15.0).abs() < 1.0e-10);
        assert!((center.y - 5.0).abs() < 1.0e-10);
        assert!((center.z - 20.0).abs() < 1.0e-10);
        assert!((view.zoom() - 15.0).abs() < 1.0e-10);
        assert!((view.slab_depth() - 40.0).abs() < 1.0e-10);
        assert!((view.view_dist() - 200.0).abs() < 1.0e-10);
    }

    #[test]
    fn density_map_fit_view_applies_object_transform() {
        let mut map = DensityMap::from_grid_map(test_grid());
        map.set_xtal_params(30.0, 10.0, 40.0, 90.0, 90.0, 90.0);
        let mut view = View::new(cuemol_core::Uid::new(1), cuemol_core::Uid::new(1));
        view.set_view_size(200, 100);

        map.fit_view_with_xform(
            &mut view,
            Matrix4::make_trans_mat(Vector4::new3(7.0, -3.0, 11.0)),
        );

        let center = view.view_center();
        assert!((center.x - 22.0).abs() < 1.0e-10);
        assert!((center.y - 2.0).abs() < 1.0e-10);
        assert!((center.z - 31.0).abs() < 1.0e-10);
        assert!((view.zoom() - 15.0).abs() < 1.0e-10);
    }

    #[test]
    fn map_renderer_defaults_and_level_math_match_qif() {
        let map = DensityMap::from_grid_map(test_grid());
        let mut renderer = MapRenderer::new();
        assert_eq!(renderer.color_mode(), MapColorMode::Solid);
        assert_eq!(renderer.color_mode_code(), MapRenderer::MAPREND_SIMPLE);
        assert_eq!(renderer.colormode(), MapRenderer::MAPREND_SIMPLE);
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgb(0.0, 0.0, 1.0).rgba8()
        );
        assert_eq!(renderer.defaultcolor().rgba8(), renderer.color().rgba8());
        renderer.set_defaultcolor(SolidColor::from_rgb(0.0, 1.0, 0.0));
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(renderer.siglevel(), 1.1);
        assert_eq!(renderer.extent(), 15.0);
        assert_eq!(renderer.center(), Vector4::ZERO);
        assert!(renderer.use_pbc());
        assert!(!renderer.use_abslevel());
        assert_eq!(renderer.bndry_rng(), 5.0);
        assert_eq!(renderer.bndry_molname(), "");
        assert!(renderer.bndry_sel().is_none());
        assert_eq!(renderer.multi_grad().size(), 0);
        assert_eq!(renderer.color_mapname(), "");
        renderer.attach_client_scalar(&map);
        assert!((renderer.level() - 1.1 * map.rmsd_density()).abs() < 1.0e-12);
        assert!((renderer.max_level() - map.max_density() / map.rmsd_density()).abs() < 1.0e-12);

        renderer.mark_display_cache_valid();
        renderer.set_colormode(MapRenderer::MAPREND_MULTIGRAD);
        assert!(!renderer.display_cache_valid());
        renderer.set_bndry_rng(-1.0);
        assert_eq!(renderer.bndry_rng(), 0.0);
        renderer.set_bndry_sel(Some(parse_selector("name CA").unwrap()));
        assert!(renderer.bndry_sel().is_some());
        renderer.set_level(map.rmsd_density() * 2.0);
        assert_eq!(renderer.siglevel(), 2.0);
    }

    #[test]
    fn map_mesh_and_surface_renderer_defaults_match_qif() {
        let map = DensityMap::from_grid_map(test_grid());
        let mut mesh = MapMeshRenderer::new();
        assert_eq!(mesh.type_name(), "contour");
        assert_eq!(mesh.width(), 1.0);
        assert_eq!(mesh.bufsize(), 100);
        assert!(mesh.autoupdate());
        assert!(!mesh.dragupdate());
        mesh.map_mut().attach_client_scalar(&map);
        assert_eq!(mesh.max_extent(), 50.0);
        mesh.set_bufsize(5);
        assert_eq!(mesh.bufsize(), 10);

        let mut surf = MapSurfRenderer::new();
        assert_eq!(surf.type_name(), "isosurf");
        assert!(surf.autoupdate());
        assert!(!surf.dragupdate());
        assert!(!surf.cullface());
        assert_eq!(surf.drawmode(), MapSurfaceDrawMode::Fill);
        assert_eq!(surf.drawmode_code(), MapSurfRenderer::MSRDRAW_FILL);
        assert_eq!(surf.width(), 1.2);
        assert_eq!(surf.binning(), 1);
        assert_eq!(surf.max_grids(), 100);
        assert_eq!(surf.max_extent(), 0.0);
        assert_eq!(surf.glrender_mode(), MapSurfRenderer::MSR_REND_DLIST);
        assert_eq!(MapSurfRenderer::MSR_REND_VBO, 1);
        assert_eq!(MapSurfRenderer::MSR_REND_SHADER, 2);
        assert_eq!(surf.omp_thr(), -1);
        assert!(surf.client_map().is_none());
        assert_eq!(surf.generate_surf_obj().mesh().triangle_count(), 0);
        surf.set_client_map(map.clone());
        assert_eq!(surf.max_extent(), 50.0);
        surf.set_drawmode(MapSurfaceDrawMode::Line);
        assert_eq!(surf.drawmode_code(), MapSurfRenderer::MSRDRAW_LINE);
        surf.set_binning(0);
        assert_eq!(surf.binning(), 1);
        surf.map_mut().set_color_mapname("map1");
        let obj = surf.generate_surf_obj();
        // The contour now triangulates the density grid itself (the level cuts
        // through the 0..7 single-cube grid), not a misrouted molecular SES.
        assert!(obj.mesh().triangle_count() > 0);
    }

    #[test]
    fn density_isosurface_contours_radial_peak_to_a_sphere() {
        // Synthetic map: density(p) = R0 - |p - c|, a radial cone peaking at the
        // grid centre. Its `density == level` isosurface is a sphere of radius
        // R0 - level, an analytic oracle for the contour mesh.
        const N: usize = 24;
        let r0 = 25.0_f64;
        let center = Vector4::new3(12.0, 12.0, 12.0);
        let mut values = vec![0.0_f64; N * N * N];
        for k in 0..N {
            for j in 0..N {
                for i in 0..N {
                    let dx = i as f64 - center.x;
                    let dy = j as f64 - center.y;
                    let dz = k as f64 - center.z;
                    let r = (dx * dx + dy * dy + dz * dz).sqrt();
                    values[i + N * (j + N * k)] = r0 - r;
                }
            }
        }
        let map = DensityMap::from_grid_map(GridMap {
            counts: [N, N, N],
            origin: Vector4::ZERO,
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 1.0, 0.0),
                Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        });

        // Contour at density == 19 => sphere of radius R0 - 19 = 6.
        let mut surf = MapSurfRenderer::new();
        surf.map_mut().set_level_from(&map, 19.0);
        surf.map_mut().set_center(center);
        surf.map_mut().set_extent(8.0);
        assert!((surf.map().level_from(&map) - 19.0).abs() < 1.0e-3);

        let obj = surf.generate_surf_obj_from(&map);
        let mesh = obj.mesh();
        assert!(
            mesh.triangle_count() > 200,
            "too few triangles: {}",
            mesh.triangle_count()
        );

        let radius = 6.0_f64;
        let len = |v: Vector4| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        for v in mesh.vertices() {
            let radial = v.position - center;
            let r = len(radial);
            assert!((r - radius).abs() < 0.6, "vertex off sphere: r={r}");
            // Outward normal aligns with the radial direction (high to low density).
            let dot = (v.normal.x * radial.x + v.normal.y * radial.y + v.normal.z * radial.z) / r;
            assert!(dot > 0.5, "normal not radial-outward: dot={dot}");
        }

        // Total area approximates 4*pi*r^2 (analytic sphere oracle).
        let analytic = 4.0 * std::f64::consts::PI * radius * radius;
        let area: f64 = mesh
            .triangles()
            .iter()
            .map(|t| {
                let a = mesh.vertices()[t[0] as usize].position;
                let b = mesh.vertices()[t[1] as usize].position;
                let c = mesh.vertices()[t[2] as usize].position;
                let ab = b - a;
                let ac = c - a;
                let cx = ab.y * ac.z - ab.z * ac.y;
                let cy = ab.z * ac.x - ab.x * ac.z;
                let cz = ab.x * ac.y - ab.y * ac.x;
                0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
            })
            .sum();
        assert!(
            (area - analytic).abs() / analytic < 0.15,
            "area {area} vs analytic {analytic}"
        );

        surf.set_binning(2);
        let coarse = surf.generate_surf_obj_from(&map);
        assert!(coarse.mesh().triangle_count() > 0);
        assert!(coarse.mesh().triangle_count() < mesh.triangle_count());

        surf.map_mut().set_center(Vector4::ZERO);
        surf.map_mut().set_extent(2.0);
        assert_eq!(surf.generate_surf_obj_from(&map).mesh().triangle_count(), 0);
    }

    #[test]
    fn density_isosurface_handles_offset_origin_and_anisotropic_grid() {
        // Same radial-cone oracle, but on a shifted, anisotropic, non-zero-start
        // grid: density(p) = R0 - |p - cc| in Cartesian, so the `density == level`
        // isosurface is still a Cartesian sphere regardless of grid geometry.
        // Exercises conv_to_orth / start offset / per-axis spacing in DensityField.
        let counts = [24usize, 18, 14];
        let start = [3isize, -2, 5];
        let origin = Vector4::new3(5.0, -3.0, 10.0);
        let delta = [
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(0.0, 1.5, 0.0),
            Vector4::new3(0.0, 0.0, 2.0),
        ];
        // Cartesian of relative node (i,j,k): origin + sum delta*(idx + start).
        let cart = |i: usize, j: usize, k: usize| {
            origin
                + delta[0].scale(i as f64 + start[0] as f64)
                + delta[1].scale(j as f64 + start[1] as f64)
                + delta[2].scale(k as f64 + start[2] as f64)
        };
        let cc = cart(counts[0] / 2, counts[1] / 2, counts[2] / 2);
        let r0 = 25.0_f64;
        let mut values = vec![0.0_f64; counts[0] * counts[1] * counts[2]];
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    let p = cart(i, j, k);
                    let d =
                        ((p.x - cc.x).powi(2) + (p.y - cc.y).powi(2) + (p.z - cc.z).powi(2)).sqrt();
                    values[i + counts[0] * (j + counts[1] * k)] = r0 - d;
                }
            }
        }
        let mut map = DensityMap::new();
        map.set_map_float_array(&values, counts, start, counts, origin, delta);

        let radius = 6.0_f64;
        let mut surf = MapSurfRenderer::new();
        surf.map_mut().set_level_from(&map, r0 - radius); // density == 19 => |p - cc| == 6
        surf.map_mut().set_center(cc);
        surf.map_mut().set_extent(8.0);
        let obj = surf.generate_surf_obj_from(&map);
        let mesh = obj.mesh();
        assert!(
            mesh.triangle_count() > 100,
            "too few triangles: {}",
            mesh.triangle_count()
        );

        let len = |v: Vector4| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        for v in mesh.vertices() {
            let radial = v.position - cc;
            let r = len(radial);
            // z spacing is 2.0, so allow a slightly looser radial tolerance.
            assert!((r - radius).abs() < 1.2, "vertex off sphere: r={r}");
            let dot = (v.normal.x * radial.x + v.normal.y * radial.y + v.normal.z * radial.z) / r;
            assert!(dot > 0.5, "normal not radial-outward: dot={dot}");
        }
    }

    #[test]
    fn contour_lines_trace_the_isosurface_sphere_on_a_radial_peak() {
        // density(p) = R0 - |p - c|: the `density == level` contour is the sphere
        // of radius R0 - level. The chickenwire intersects that sphere with every
        // grid plane, so EVERY contour-line vertex must lie on the sphere.
        const N: usize = 24;
        let r0 = 25.0_f64;
        let center = Vector4::new3(12.0, 12.0, 12.0);
        let mut values = vec![0.0_f64; N * N * N];
        for k in 0..N {
            for j in 0..N {
                for i in 0..N {
                    let dx = i as f64 - center.x;
                    let dy = j as f64 - center.y;
                    let dz = k as f64 - center.z;
                    values[i + N * (j + N * k)] = r0 - (dx * dx + dy * dy + dz * dz).sqrt();
                }
            }
        }
        let map = DensityMap::from_grid_map(GridMap {
            counts: [N, N, N],
            origin: Vector4::ZERO,
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 1.0, 0.0),
                Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        });

        let radius = 6.0_f64;
        let mut mesh = MapMeshRenderer::new();
        mesh.map_mut().set_level_from(&map, r0 - radius);
        mesh.map_mut().set_center(center);
        mesh.map_mut().set_extent(8.0);
        let segments = mesh.generate_contour_lines(&map);

        assert!(
            segments.len() > 200,
            "too few contour segments: {}",
            segments.len()
        );
        // NOTE: this checks crossing-point POSITIONS (all on the sphere), not
        // segment CONNECTIVITY — both endpoints land on the sphere regardless of
        // how `EDGE_PAIRS` pairs them (segments are intra-cell). Correct pairing
        // rests on that hand-verified table, not on this assertion.
        let len = |v: Vector4| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        for seg in &segments {
            for p in seg {
                let r = len(*p - center);
                assert!((r - radius).abs() < 0.7, "contour vertex off sphere: r={r}");
            }
        }

        mesh.map_mut().set_center(Vector4::ZERO);
        mesh.map_mut().set_extent(2.0);
        assert!(mesh.generate_contour_lines(&map).is_empty());
    }

    #[test]
    fn contour_lines_empty_when_level_exceeds_max() {
        let map = DensityMap::from_grid_map(test_grid());
        let mut mesh = MapMeshRenderer::new();
        mesh.map_mut().set_level_from(&map, 100.0);
        assert!(mesh.generate_contour_lines(&map).is_empty());
    }

    #[test]
    fn map_renderers_wrap_full_cell_density_at_periodic_boundary() {
        const N: usize = 4;
        let mut values = vec![0.0; N * N * N];
        for k in 0..N {
            for j in 0..N {
                values[N * (j + N * k)] = 10.0;
            }
        }
        let map = DensityMap::from_grid_map(GridMap {
            counts: [N, N, N],
            origin: Vector4::ZERO,
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 1.0, 0.0),
                Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        });
        assert!(map.is_periodic_grid());

        let center = Vector4::new3(4.0, 1.5, 1.5);
        let mut wire = MapMeshRenderer::new();
        wire.map_mut().set_level_from(&map, 5.0);
        wire.map_mut().set_center(center);
        wire.map_mut().set_extent(1.5);
        wire.map_mut().set_use_pbc(false);
        assert!(wire.generate_contour_lines(&map).is_empty());
        wire.map_mut().set_use_pbc(true);
        let wire_count = wire.generate_contour_lines(&map).len();
        assert!(wire_count > 0);
        wire.map_mut().set_bndry_rng(100.0);
        assert_eq!(
            wire.generate_contour_lines_with_boundary(&map, Some(&[center]))
                .len(),
            wire_count
        );
        assert!(wire
            .generate_contour_lines_with_boundary(&map, Some(&[]))
            .is_empty());

        let mut surface = MapSurfRenderer::new();
        surface.map_mut().set_level_from(&map, 5.0);
        surface.map_mut().set_center(center);
        surface.map_mut().set_extent(1.5);
        surface.map_mut().set_use_pbc(false);
        assert_eq!(
            surface.generate_surf_obj_from(&map).mesh().triangle_count(),
            0
        );
        surface.map_mut().set_use_pbc(true);
        let surface_count = surface.generate_surf_obj_from(&map).mesh().triangle_count();
        assert!(surface_count > 0);
        surface.map_mut().set_bndry_rng(100.0);
        assert_eq!(
            surface
                .generate_surf_obj_with_boundary(&map, Some(&[center]))
                .mesh()
                .triangle_count(),
            surface_count
        );
        assert_eq!(
            surface
                .generate_surf_obj_with_boundary(&map, Some(&[]))
                .mesh()
                .triangle_count(),
            0
        );
    }

    #[test]
    fn contour_lines_obj_export_pairs_vertices_into_lines() {
        let segments = vec![
            [Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(1.0, 0.0, 0.0)],
            [Vector4::new3(1.0, 0.0, 0.0), Vector4::new3(1.0, 1.0, 0.0)],
        ];
        let obj = contour_lines_to_obj(&segments);
        assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 4);
        assert_eq!(obj.lines().filter(|l| l.starts_with("l ")).count(), 2);
        assert!(obj.contains("l 1 2"));
        assert!(obj.contains("l 3 4"));
    }

    #[test]
    fn density_isosurface_is_empty_when_level_exceeds_max() {
        // A level above the map maximum has no crossing => empty mesh, never a
        // spurious surface (guards the inside/outside sign convention).
        let map = DensityMap::from_grid_map(test_grid());
        let mut surf = MapSurfRenderer::new();
        surf.map_mut().set_level_from(&map, 100.0);
        let obj = surf.generate_surf_obj_from(&map);
        assert_eq!(obj.mesh().triangle_count(), 0);
    }

    #[test]
    fn glsl_map_renderers_match_qif_defaults() {
        let map = DensityMap::from_grid_map(test_grid());
        let mut mesh = GlslMapMeshRenderer::new();
        assert_eq!(mesh.type_name(), "gpu_mapmesh");
        assert_eq!(
            mesh.map().color().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(mesh.width(), 1.0);
        assert_eq!(mesh.bufsize(), 100);
        assert!(mesh.autoupdate());
        assert!(mesh.dragupdate());
        assert!(!mesh.shader_checked());
        assert!(!mesh.map_texture_ok());
        mesh.map_mut().attach_client_scalar(&map);
        assert_eq!(mesh.max_extent(), 50.0);
        mesh.invalidate_display_cache();
        assert!(!mesh.map_texture_ok());

        let mut vol = GlslMapVolRenderer::new();
        assert_eq!(vol.type_name(), "gpu_mapvol");
        assert_eq!(
            vol.map().color().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 0.0).rgba8()
        );
        assert!(vol.autoupdate());
        assert!(vol.dragupdate());
        assert_eq!(vol.xferfn_type(), GlslMapVolRenderer::GLMV_AUTO1);
        assert_eq!(GlslMapVolRenderer::GLMV_MULTIGRAD, 0);
        assert_eq!(GlslMapVolRenderer::GLMV_AUTO2, 2);
        assert!(!vol.shader_checked());
        assert!(!vol.map_texture_ok());
        vol.map_mut().attach_client_scalar(&map);
        assert_eq!(vol.max_extent(), 3.0);
        vol.set_xferfn_type(GlslMapVolRenderer::GLMV_AUTO2);
        assert_eq!(vol.xferfn_type(), GlslMapVolRenderer::GLMV_AUTO2);
    }
}

#[cfg(test)]
mod shared_storage_tests {
    use super::DensityMap;
    use std::sync::Arc;

    #[test]
    fn undo_snapshot_shares_voxels_until_grid_is_replaced() {
        let mut map = DensityMap::new();
        map.set_map_byte_array(&[0, 64, 128, 255], [4, 1, 1], 0., 1., 0.5, 0.3);
        let saved = map.clone();
        assert!(Arc::ptr_eq(&map.bytes, &saved.bytes));
        map.subsample(2).unwrap();
        assert!(!Arc::ptr_eq(&map.bytes, &saved.bytes));
        assert_eq!(saved.bytes.as_slice(), &[0, 64, 128, 255]);
        assert_eq!(map.bytes.len(), 2);
    }
}
