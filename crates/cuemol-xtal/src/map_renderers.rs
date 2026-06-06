use cuemol_core::Vector4;
use cuemol_gfx::SolidColor;
use cuemol_molstr::{GridMap, Selector};
use cuemol_qsys::{DispListRenderer, MultiGradient, ScalarObject};
use cuemol_surface::MolSurfObj;
use cuemol_symm::CrystalInfo;

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
    bytes: Vec<u8>,
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
            bytes: Vec::new(),
        }
    }
}

impl DensityMap {
    pub fn new() -> Self {
        Self::default()
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
        self.cell = CrystalInfo::new(
            delta[0].length3d() * intervals[0] as f64,
            delta[1].length3d() * intervals[1] as f64,
            delta[2].length3d() * intervals[2] as f64,
            90.0,
            90.0,
            90.0,
        );

        self.update_stats(values);
        self.bytes = values
            .iter()
            .map(|value| self.float_to_byte(*value))
            .collect();
        self.pad_or_truncate_bytes();
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
        self.bytes = bytes.to_vec();
        self.pad_or_truncate_bytes();
    }

    pub fn set_map_params(&mut self, start: [isize; 3], intervals: [usize; 3]) {
        self.start = start;
        self.intervals = intervals;
    }

    pub fn set_xtal_params(&mut self, a: f64, b: f64, c: f64, alpha: f64, beta: f64, gamma: f64) {
        self.cell = CrystalInfo::new(a, b, c, alpha, beta, gamma);
        self.delta = [
            Vector4::new3(a / self.intervals[0].max(1) as f64, 0.0, 0.0),
            Vector4::new3(0.0, b / self.intervals[1].max(1) as f64, 0.0),
            Vector4::new3(0.0, 0.0, c / self.intervals[2].max(1) as f64),
        ];
    }

    pub fn cell(&self) -> CrystalInfo {
        self.cell
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
        self.bytes.resize(expected, 0);
    }

    fn flat_index(&self, col: isize, row: isize, sec: isize) -> Option<usize> {
        let col = col - self.start[0];
        let row = row - self.start[1];
        let sec = sec - self.start[2];
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
        let rel = position - self.origin;
        [
            project_axis(rel, self.delta[0]).floor() as isize + self.start[0],
            project_axis(rel, self.delta[1]).floor() as isize + self.start[1],
            project_axis(rel, self.delta[2]).floor() as isize + self.start[2],
        ]
    }
}

impl ScalarObject for DensityMap {
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

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn siglevel(&self) -> f64 {
        self.siglevel
    }

    pub fn set_siglevel(&mut self, value: f64) {
        self.siglevel = value;
        self.invalidate_display_cache();
    }

    pub fn extent(&self) -> f64 {
        self.extent
    }

    pub fn set_extent(&mut self, value: f64) {
        self.extent = value;
        self.invalidate_display_cache();
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn set_center(&mut self, value: Vector4) {
        self.center = value;
        self.invalidate_display_cache();
    }

    pub fn use_pbc(&self) -> bool {
        self.use_pbc
    }

    pub fn set_use_pbc(&mut self, value: bool) {
        self.use_pbc = value;
        self.invalidate_display_cache();
    }

    pub fn use_abslevel(&self) -> bool {
        self.use_abslevel
    }

    pub fn set_use_abslevel(&mut self, value: bool) {
        self.use_abslevel = value;
    }

    pub fn bndry_rng(&self) -> f64 {
        self.bndry_rng
    }

    pub fn set_bndry_rng(&mut self, value: f64) {
        self.bndry_rng = value.max(0.0);
        self.invalidate_display_cache();
    }

    pub fn bndry_molname(&self) -> &str {
        &self.bndry_molname
    }

    pub fn set_bndry_molname(&mut self, value: impl Into<String>) {
        let value = value.into();
        if self.bndry_molname != value {
            self.bndry_molname = value;
            self.invalidate_display_cache();
        }
    }

    pub fn bndry_sel(&self) -> Option<&Selector> {
        self.bndry_sel.as_ref()
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

    pub fn multi_grad_mut(&mut self) -> &mut MultiGradient {
        &mut self.multi_grad
    }

    pub fn color_mapname(&self) -> &str {
        &self.color_mapname
    }

    pub fn get_color_map_obj(&self) -> Option<&MultiGradient> {
        Some(&self.multi_grad)
    }

    pub fn set_color_mapname(&mut self, value: impl Into<String>) {
        self.color_mapname = value.into();
        self.invalidate_display_cache();
    }

    pub fn level(&self, scalar: &impl ScalarObject) -> f64 {
        self.siglevel * scalar.rmsd_density()
    }

    pub fn set_level(&mut self, scalar: &impl ScalarObject, value: f64) {
        let rmsd = scalar.rmsd_density();
        self.set_siglevel(if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            value / rmsd
        });
    }

    pub fn max_level(&self, scalar: &impl ScalarObject) -> f64 {
        let rmsd = scalar.rmsd_density();
        if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            scalar.max_density() / rmsd
        }
    }

    pub fn min_level(&self, scalar: &impl ScalarObject) -> f64 {
        let rmsd = scalar.rmsd_density();
        if rmsd.abs() < 1.0e-12 {
            0.0
        } else {
            scalar.min_density() / rmsd
        }
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

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.map.invalidate_display_cache();
    }

    pub fn bufsize(&self) -> i32 {
        self.bufsize
    }

    pub fn set_bufsize(&mut self, value: i32) {
        self.bufsize = value.max(10);
        self.map.invalidate_display_cache();
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

    pub fn max_extent(&self, scalar: &impl ScalarObject) -> f64 {
        let half = self.bufsize as f64 / 2.0;
        (half * scalar.col_grid_size())
            .min(half * scalar.row_grid_size())
            .min(half * scalar.sec_grid_size())
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

    pub fn set_cullface(&mut self, value: bool) {
        self.cullface = value;
        self.map.invalidate_display_cache();
    }

    pub fn drawmode(&self) -> MapSurfaceDrawMode {
        self.drawmode
    }

    pub fn drawmode_code(&self) -> i32 {
        self.drawmode.code()
    }

    pub fn set_drawmode(&mut self, value: MapSurfaceDrawMode) {
        self.drawmode = value;
        self.map.invalidate_display_cache();
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.map.invalidate_display_cache();
    }

    pub fn binning(&self) -> i32 {
        self.binning
    }

    pub fn set_binning(&mut self, value: i32) {
        self.binning = value;
        self.map.invalidate_display_cache();
    }

    pub fn max_grids(&self) -> i32 {
        self.max_grids
    }

    pub fn set_max_grids(&mut self, value: i32) {
        self.max_grids = value;
    }

    pub fn max_extent(&self, scalar: &impl ScalarObject) -> f64 {
        self.max_grids as f64 * scalar.col_grid_size() / 2.0
    }

    pub fn generate_surf_obj(&self, scalar: &impl ScalarObject) -> MolSurfObj {
        let mut object = MolSurfObj::new();
        object.create_ses_from_mol(
            self.map.color_mapname().to_string(),
            "",
            self.map.level(scalar),
            self.width,
        );
        object
    }

    pub fn glrender_mode(&self) -> i32 {
        self.glrender_mode
    }

    pub fn set_glrender_mode(&mut self, value: i32) {
        self.map.invalidate_display_cache();
        self.glrender_mode = value;
    }

    pub fn omp_thr(&self) -> i32 {
        self.omp_thr
    }

    pub fn set_omp_thr(&mut self, value: i32) {
        self.omp_thr = value;
        self.map.invalidate_display_cache();
    }
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

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.invalidate_display_cache();
    }

    pub fn bufsize(&self) -> i32 {
        self.bufsize
    }

    pub fn set_bufsize(&mut self, value: i32) {
        self.bufsize = value;
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

    pub fn max_extent(&self, scalar: &impl ScalarObject) -> f64 {
        (50.0 * scalar.col_grid_size())
            .min(50.0 * scalar.row_grid_size())
            .min(50.0 * scalar.sec_grid_size())
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

    pub fn set_xferfn_type(&mut self, value: i32) {
        self.xferfn_type = value;
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

    pub fn max_extent(&self, scalar: &impl ScalarObject) -> f64 {
        (scalar.col_no() as f64 * scalar.col_grid_size() / 2.0)
            .max(scalar.row_no() as f64 * scalar.row_grid_size() / 2.0)
            .max(scalar.sec_no() as f64 * scalar.sec_grid_size() / 2.0)
    }

    pub fn invalidate_display_cache(&mut self) {
        self.map.invalidate_display_cache();
        self.map_texture_ok = false;
    }
}

fn project_axis(value: Vector4, axis: Vector4) -> f64 {
    let denom = axis.sqlen3d();
    if denom.abs() < 1.0e-12 {
        0.0
    } else {
        value.dot3d(axis) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::parse_selector;

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
    fn map_renderer_defaults_and_level_math_match_qif() {
        let map = DensityMap::from_grid_map(test_grid());
        let mut renderer = MapRenderer::new();
        assert_eq!(renderer.color_mode(), MapColorMode::Solid);
        assert_eq!(renderer.color_mode_code(), MapRenderer::MAPREND_SIMPLE);
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgb(0.0, 0.0, 1.0).rgba8()
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
        assert!((renderer.level(&map) - 1.1 * map.rmsd_density()).abs() < 1.0e-12);
        assert!(
            (renderer.max_level(&map) - map.max_density() / map.rmsd_density()).abs() < 1.0e-12
        );

        renderer.mark_display_cache_valid();
        renderer.set_color_mode(MapColorMode::MultiGradient);
        assert!(!renderer.display_cache_valid());
        renderer.set_bndry_rng(-1.0);
        assert_eq!(renderer.bndry_rng(), 0.0);
        renderer.set_bndry_sel(Some(parse_selector("name CA").unwrap()));
        assert!(renderer.bndry_sel().is_some());
        renderer.set_level(&map, map.rmsd_density() * 2.0);
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
        assert_eq!(mesh.max_extent(&map), 50.0);
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
        assert_eq!(surf.max_extent(&map), 50.0);
        assert_eq!(surf.glrender_mode(), MapSurfRenderer::MSR_REND_DLIST);
        assert_eq!(MapSurfRenderer::MSR_REND_VBO, 1);
        assert_eq!(MapSurfRenderer::MSR_REND_SHADER, 2);
        assert_eq!(surf.omp_thr(), -1);
        surf.set_drawmode(MapSurfaceDrawMode::Line);
        assert_eq!(surf.drawmode_code(), MapSurfRenderer::MSRDRAW_LINE);
        surf.map_mut().set_color_mapname("map1");
        let obj = surf.generate_surf_obj(&map);
        assert_eq!(obj.orig_mol(), "map1");
        assert_eq!(obj.orig_prad(), 1.2);
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
        assert_eq!(mesh.max_extent(&map), 50.0);
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
        assert_eq!(vol.max_extent(&map), 3.0);
        vol.set_xferfn_type(GlslMapVolRenderer::GLMV_AUTO2);
        assert_eq!(vol.xferfn_type(), GlslMapVolRenderer::GLMV_AUTO2);
    }
}
