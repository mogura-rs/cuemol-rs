use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MapRegionMode {
    #[default]
    Auto,
    Box,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSamplingRegion {
    pub start: [isize; 3],
    /// Exclusive upper bounds of the sampled grid nodes.
    pub end: [isize; 3],
    pub step: usize,
    pub periodic: bool,
}

impl MapSamplingRegion {
    pub fn node_counts(self) -> [usize; 3] {
        std::array::from_fn(|a| self.end[a].saturating_sub(self.start[a]).max(0) as usize)
    }

    pub fn sampled_counts(self) -> [usize; 3] {
        self.node_counts()
            .map(|n| n.saturating_sub(1) / self.step.max(1) + usize::from(n > 0))
    }

    pub(super) fn range(self) -> GridRange {
        GridRange {
            start: self.start,
            end: self.end,
            periodic: self.periodic,
        }
    }
}

pub fn lod_cell_count(counts: [usize; 3], step: usize) -> u64 {
    counts.into_iter().fold(1u64, |n, count| {
        n.saturating_mul((count.saturating_sub(1) / step.max(1)) as u64)
    })
}

pub fn lod_step_for_budget(counts: [usize; 3], budget: u64) -> usize {
    let mut step = 1;
    while lod_cell_count(counts, step) > budget && step < 64 {
        step *= 2;
    }
    step
}

pub fn lod_align_range(
    lo: isize,
    hi: isize,
    start: isize,
    count: usize,
    step: usize,
) -> (isize, usize) {
    if count == 0 || step == 0 {
        return (start, 0);
    }
    let lo = lo.max(start);
    let hi = hi.min(start.saturating_add_unsigned(count - 1));
    if lo > hi {
        return (start, 0);
    }
    let low = (lo - start) as usize / step * step;
    let high = ((hi - start) as usize).div_ceil(step) * step;
    let high = high.min((count - 1) / step * step).max(low);
    (start + low as isize, high - low)
}

impl MapRenderer {
    pub fn region_mode(&self) -> MapRegionMode {
        self.region_mode
    }
    pub fn set_region_mode(&mut self, value: MapRegionMode) {
        self.region_mode = value;
        self.invalidate_display_cache();
    }
    pub fn region_mode_resolved(&self) -> &'static str {
        if self.region_mode == MapRegionMode::Full
            || (self.region_mode == MapRegionMode::Auto && self.map_type_resolved() == "em")
        {
            "full"
        } else {
            "box"
        }
    }
    pub fn is_full_region(&self, map: &impl ScalarObject) -> bool {
        self.region_mode == MapRegionMode::Full
            || (self.region_mode == MapRegionMode::Auto && map.density_map_kind() == Some("em"))
    }
    pub fn lod(&self) -> usize {
        self.lod
    }
    pub fn set_lod(&mut self, value: usize) {
        self.lod = value;
        self.invalidate_display_cache();
    }
    pub fn lod_budget(&self) -> u32 {
        self.lod_budget
    }
    pub fn set_lod_budget(&mut self, value: u32) {
        self.lod_budget = value.max(1);
        self.invalidate_display_cache();
    }
    pub fn zoom_refine(&self) -> bool {
        self.zoom_refine
    }
    pub fn set_zoom_refine(&mut self, value: bool) {
        self.zoom_refine = value;
        self.invalidate_display_cache();
    }
    pub fn set_view_box(&mut self, center: Vector4, half: f64) {
        self.view_box = half.is_finite().then_some((center, half.max(0.)));
    }
    pub fn set_world_transform(&mut self, value: Matrix4) {
        self.world_transform = value;
    }

    pub fn world_box_to_grid(
        &self,
        map: &impl ScalarObject,
        min: Vector4,
        max: Vector4,
    ) -> Option<([isize; 3], [isize; 3])> {
        let (min, max) = grid_box(map, min, max, self.world_transform)?;
        let counts = [map.col_no(), map.row_no(), map.sec_no()];
        let lo = std::array::from_fn(|a| ((min[a] + 1e-4).floor() as isize).max(0));
        let hi =
            std::array::from_fn(|a| ((max[a] - 1e-4).ceil() as isize).min(counts[a] as isize - 1));
        (0..3).all(|a| lo[a] <= hi[a]).then_some((lo, hi))
    }

    pub fn full_region(
        &self,
        map: &impl ScalarObject,
        boundary_points: Option<&[Vector4]>,
    ) -> Option<MapSamplingRegion> {
        let counts = [map.col_no(), map.row_no(), map.sec_no()];
        if counts.contains(&0) {
            return None;
        }
        let mut lo = [0; 3];
        let mut hi = counts.map(|n| n as isize - 1);
        if self.zoom_refine {
            if let Some((center, half)) = self.view_box {
                let d = Vector4::new3(half * 1.5, half * 1.5, half * 1.5);
                if let Some((a, b)) = self.world_box_to_grid(map, center - d, center + d) {
                    lo = a;
                    hi = b;
                }
            }
        }
        if let Some((a, b)) = self.boundary_grid_box(map, boundary_points) {
            let lower = std::array::from_fn(|i| lo[i].max(a[i]));
            let upper = std::array::from_fn(|i| hi[i].min(b[i]));
            if (0..3).all(|i| lower[i] <= upper[i]) {
                lo = lower;
                hi = upper;
            }
        }
        let nodes = std::array::from_fn(|i| (hi[i] - lo[i] + 1) as usize);
        let step = if self.lod == 0 {
            lod_step_for_budget(nodes, u64::from(self.lod_budget) << 20)
        } else {
            self.lod.max(1)
        };
        let aligned: [(isize, usize); 3] =
            std::array::from_fn(|i| lod_align_range(lo[i], hi[i], 0, counts[i], step));
        Some(MapSamplingRegion {
            start: aligned.map(|v| v.0),
            end: aligned.map(|v| v.0 + v.1 as isize + 1),
            step,
            periodic: false,
        })
    }

    fn boundary_grid_box(
        &self,
        map: &impl ScalarObject,
        points: Option<&[Vector4]>,
    ) -> Option<([isize; 3], [isize; 3])> {
        let points = points?;
        let mut low = *points.first()?;
        let mut high = low;
        for p in points {
            low = Vector4::new3(low.x.min(p.x), low.y.min(p.y), low.z.min(p.z));
            high = Vector4::new3(high.x.max(p.x), high.y.max(p.y), high.z.max(p.z));
        }
        let d = Vector4::new3(self.bndry_rng, self.bndry_rng, self.bndry_rng);
        let (a, b) = grid_box(map, low - d, high + d, Matrix4::IDENTITY)?;
        let counts = [map.col_no(), map.row_no(), map.sec_no()];
        let lo = std::array::from_fn(|i| ((a[i] + 1e-4).floor() as isize).max(0));
        let hi =
            std::array::from_fn(|i| ((b[i] - 1e-4).ceil() as isize).min(counts[i] as isize - 1));
        (0..3).all(|i| lo[i] <= hi[i]).then_some((lo, hi))
    }

    pub fn view_region_requires_rebuild(
        &self,
        map: &impl ScalarObject,
        points: Option<&[Vector4]>,
        previous: MapSamplingRegion,
    ) -> bool {
        if !self.is_full_region(map) || !self.zoom_refine {
            return false;
        }
        let Some((center, half)) = self.view_box else {
            return false;
        };
        if self
            .full_region(map, points)
            .is_some_and(|r| r.step < previous.step)
        {
            return true;
        }
        let d = Vector4::new3(half, half, half);
        let Some((mut lo, mut hi)) = self.world_box_to_grid(map, center - d, center + d) else {
            return false;
        };
        if let Some((a, b)) = self.boundary_grid_box(map, points) {
            for i in 0..3 {
                lo[i] = lo[i].max(a[i]);
                hi[i] = hi[i].min(b[i]);
            }
        }
        (0..3).any(|i| lo[i] <= hi[i] && (lo[i] < previous.start[i] || hi[i] >= previous.end[i]))
    }

    pub(super) fn sampling_region(
        &self,
        map: &impl ScalarObject,
        points: Option<&[Vector4]>,
        box_extent: f64,
        box_limit: Option<[usize; 3]>,
        historical_step: usize,
        upper_node: bool,
    ) -> Option<MapSamplingRegion> {
        if self.is_full_region(map) {
            return self.full_region(map, points);
        }
        let mut range = GridRange::from_center_extent(
            map,
            self.center,
            box_extent,
            self.use_pbc,
            self.world_transform,
        )?;
        if let Some(limit) = box_limit {
            range.limit_node_counts(limit);
        }
        if upper_node {
            range = range.with_upper_boundary_nodes(map);
        }
        Some(MapSamplingRegion {
            start: range.start,
            end: range.end,
            periodic: range.periodic,
            step: if self.lod == 0 {
                historical_step.max(1)
            } else {
                self.lod.max(1)
            },
        })
    }
}

pub(super) fn grid_box(
    map: &impl ScalarObject,
    min: Vector4,
    max: Vector4,
    transform: Matrix4,
) -> Option<([f64; 3], [f64; 3])> {
    let origin = map.conv_to_orth(Vector4::ZERO);
    let axes = [
        Vector4::new3(1., 0., 0.),
        Vector4::new3(0., 1., 0.),
        Vector4::new3(0., 0., 1.),
    ]
    .map(|v| map.conv_to_orth(v) - origin);
    let inverse = Matrix3::new([
        [axes[0].x, axes[1].x, axes[2].x],
        [axes[0].y, axes[1].y, axes[2].y],
        [axes[0].z, axes[1].z, axes[2].z],
    ])
    .invert()
    .ok()?;
    let transform = transform.invert().ok()?;
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for i in 0..8 {
        let p = Vector4::new4(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
            1.,
        );
        let p = inverse.mul_vec(transform.transform_point(p) - origin);
        for (a, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            low[a] = low[a].min(v);
            high[a] = high[a].max(v);
        }
    }
    Some((low, high))
}
