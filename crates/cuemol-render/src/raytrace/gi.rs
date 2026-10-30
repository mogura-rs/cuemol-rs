use super::*;

#[derive(Clone, Copy, Default)]
struct Cell {
    normal: [f32; 3],
    irradiance: [f32; 3],
    depth: f32,
}

pub(super) struct GiGrid {
    width: usize,
    height: usize,
    start: usize,
    cells: Vec<Cell>,
}

impl GiGrid {
    pub fn build(
        snapshot: &RenderSceneSnapshot,
        index: &RayIndex<'_>,
        camera: &Camera,
        params: &UmbreonRenderParams,
        rays: &sampling::CameraRays,
        region: std::ops::Range<usize>,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Self>> {
        if !params.use_gi
            || params.gi_intensity <= 0.
            || params.supersample <= 1
            || !index.opaque_without_edges
        {
            return Ok(None);
        }
        let width = params.width as usize;
        let height = params.height as usize;
        let start = region.start.saturating_sub(1);
        let end = (region.end + 1).min(height);
        let length = width
            .checked_mul(end - start)
            .ok_or_else(|| CueMolError::invalid_argument("GI grid dimensions overflow"))?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(length)
            .map_err(|e| CueMolError::invalid_argument(format!("cannot allocate GI grid: {e}")))?;
        cells.resize(length, Cell::default());
        let rotation = camera.rotation().to_matrix();
        let half = camera.slab_depth() * 0.5;
        let fill = |start_row: usize, cells: &mut [Cell]| {
            for (offset, row) in cells.chunks_mut(width).enumerate() {
                let y = start + start_row + offset;
                for (x, cell) in row.iter_mut().enumerate() {
                    if cancel.is_some_and(|cancel| cancel.load(Ordering::Relaxed)) {
                        return;
                    }
                    let ray = rays.at(x as f64 + 0.5, y as f64 + 0.5);
                    let Some(mut hit) = index.nearest_visible(ray, |hit| {
                        !hit.unlit
                            && (!params.use_clip_z
                                || (-half..=half).contains(
                                    &transform_direction(rotation, hit.position - camera.center())
                                        .z,
                                ))
                    }) else {
                        continue;
                    };
                    if hit.normal.dot3d(ray.direction) > 0. {
                        hit.normal = hit.normal * -1.;
                    }
                    let irradiance = lighting::indirect_at(
                        snapshot,
                        index,
                        hit,
                        camera,
                        params,
                        None,
                        params.gi_samples.clamp(1, 256) as usize,
                        [x as u32, y as u32],
                        true,
                    );
                    *cell = Cell {
                        normal: [
                            hit.normal.x as f32,
                            hit.normal.y as f32,
                            hit.normal.z as f32,
                        ],
                        irradiance: irradiance.map(|value| value as f32),
                        depth: hit.distance as f32,
                    };
                }
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let workers = if width * height < 65536 {
                1
            } else {
                std::thread::available_parallelism()
                    .map(usize::from)
                    .unwrap_or(1)
                    .min(16)
                    .min(end - start)
            };
            if workers == 1 {
                fill(0, &mut cells);
            } else {
                let rows = 4.min((end - start).div_ceil(workers));
                let tasks = Mutex::new(cells.chunks_mut(width * rows).enumerate());
                std::thread::scope(|scope| {
                    for _ in 0..workers {
                        let tasks = &tasks;
                        let fill = &fill;
                        scope.spawn(move || loop {
                            let Some((offset, cells)) = tasks.lock().unwrap().next() else {
                                break;
                            };
                            fill(offset * rows, cells);
                        });
                    }
                });
            }
        }
        #[cfg(target_arch = "wasm32")]
        fill(0, &mut cells);
        if cancel.is_some_and(|cancel| cancel.load(Ordering::Relaxed)) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        Ok(Some(Self {
            width,
            height,
            start,
            cells,
        }))
    }

    pub fn sample(&self, uv: [f64; 2], hit: Hit) -> Option<[f64; 3]> {
        let x = uv[0] * self.width as f64 - 0.5;
        let y = uv[1] * self.height as f64 - 0.5;
        let bx = x.floor() as isize;
        let by = y.floor() as isize;
        let fraction = [x - x.floor(), y - y.floor()];
        let mut total = 0.;
        let mut irradiance = [0.; 3];
        for dy in 0..2 {
            for dx in 0..2 {
                let cx = (bx + dx).clamp(0, self.width as isize - 1) as usize;
                let cy = (by + dy).clamp(0, self.height as isize - 1) as usize;
                let Some(row) = cy.checked_sub(self.start) else {
                    continue;
                };
                let Some(cell) = self.cells.get(row * self.width + cx) else {
                    continue;
                };
                if cell.depth <= 0. {
                    continue;
                }
                let normal = Vector4::new3(
                    cell.normal[0] as f64,
                    cell.normal[1] as f64,
                    cell.normal[2] as f64,
                );
                let bilinear = if dx == 0 {
                    1. - fraction[0]
                } else {
                    fraction[0]
                } * if dy == 0 {
                    1. - fraction[1]
                } else {
                    fraction[1]
                };
                let weight = bilinear
                    * normal.dot3d(hit.normal).clamp(0., 1.).powi(32)
                    * (-(hit.distance - cell.depth as f64).abs() / (0.02 * hit.distance + 1e-6))
                        .exp();
                total += weight;
                for (out, value) in irradiance.iter_mut().zip(cell.irradiance) {
                    *out += weight * value as f64;
                }
            }
        }
        (total >= 0.3).then(|| irradiance.map(|value| value / total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderAtom;

    fn scene() -> (RenderSceneSnapshot, Camera, UmbreonRenderParams) {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot
            .atoms
            .push(RenderAtom::new(Vector4::ZERO, 0.12, None));
        let mut camera = Camera::new();
        camera.set_zoom(4.);
        camera.set_cam_dist(10.);
        camera.set_slab_depth(100.);
        let params = UmbreonRenderParams {
            width: 8,
            height: 8,
            supersample: 4,
            use_gi: true,
            gi_samples: 8,
            gi_intensity: 1.,
            gi_env_intensity: 1.,
            gi_sky_gradient: false,
            light_intensity: 1.,
            ambient_fraction: 1.,
            show_edge_lines: false,
            gi_denoise: false,
            ..Default::default()
        };
        (snapshot, camera, params)
    }

    #[test]
    fn thin_geometry_missed_by_the_gather_grid_keeps_its_illumination() {
        let (snapshot, camera, params) = scene();
        let index = RayIndex::new(&snapshot);
        let rays = sampling::CameraRays::new(&camera, 8, 8, false).unwrap();
        let grid = GiGrid::build(&snapshot, &index, &camera, &params, &rays, 0..8, None)
            .unwrap()
            .unwrap();
        assert!(grid.cells.iter().all(|cell| cell.depth == 0.));
        let ray = rays.at(3.875, 3.875);
        let hit = index.nearest_visible(ray, |_| true).unwrap();
        assert!(grid.sample([3.875 / 8., 3.875 / 8.], hit).is_none());
        let cache = lighting::PixelCache::with_grids(0.5, None, Some(&grid));
        cache.borrow_mut().set_position([3.875 / 8., 3.875 / 8.]);
        let patched = trace_sample(
            &snapshot,
            &index,
            ray,
            &camera,
            [0.; 4],
            &params,
            Some(&cache),
        );
        let inline_cache = lighting::PixelCache::new(0.5);
        inline_cache
            .borrow_mut()
            .set_position([3.875 / 8., 3.875 / 8.]);
        let full = trace_sample(
            &snapshot,
            &index,
            ray,
            &camera,
            [0.; 4],
            &params,
            Some(&inline_cache),
        );
        assert!(patched[0] > 0.);
        for (a, b) in patched.into_iter().zip(full) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn tiled_gi_matches_the_whole_grid_including_pixel_boundaries() {
        let (mut snapshot, camera, mut params) = scene();
        snapshot.atoms[0].radius = 1.;
        params.width = 33;
        params.height = 259;
        params.supersample = 2;
        let whole = render_float_region(
            &snapshot,
            &camera,
            [0.; 4],
            &params,
            None,
            None,
            0..259,
            None,
        )
        .unwrap();
        let tiled = render_float(&snapshot, &camera, [0.; 4], &params, None, None).unwrap();
        assert_eq!(tiled.pixels, whole.pixels);
        let index = RayIndex::new(&snapshot);
        let rays = sampling::CameraRays::new(&camera, 33, 259, false).unwrap();
        let grid = GiGrid::build(&snapshot, &index, &camera, &params, &rays, 128..129, None)
            .unwrap()
            .unwrap();
        assert_eq!(grid.cells.len(), 33 * 3);
        let cancel = AtomicBool::new(true);
        assert!(GiGrid::build(
            &snapshot,
            &index,
            &camera,
            &params,
            &rays,
            0..259,
            Some(&cancel)
        )
        .is_err());
    }
}
