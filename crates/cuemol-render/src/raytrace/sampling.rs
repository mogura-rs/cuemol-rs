use super::*;

pub(super) struct CameraRays {
    inverse: cuemol_core::Matrix4,
    center: Vector4,
    distance: f64,
    half_width: f64,
    half_height: f64,
    width: usize,
    height: usize,
    perspective: bool,
}

impl CameraRays {
    pub fn new(camera: &Camera, width: usize, height: usize, perspective: bool) -> Result<Self> {
        Ok(Self {
            inverse: camera.rotation().inv()?.to_matrix(),
            center: camera.center(),
            distance: camera.cam_dist(),
            half_width: camera.zoom() * 0.5 * width as f64 / height as f64,
            half_height: camera.zoom() * 0.5,
            width,
            height,
            perspective,
        })
    }

    pub fn at(&self, x: f64, y: f64) -> Ray {
        let x = (x * 2. / self.width as f64 - 1.) * self.half_width;
        let y = (1. - y * 2. / self.height as f64) * self.half_height;
        let (origin, direction) = if self.perspective {
            (
                Vector4::new3(0., 0., self.distance),
                normalize(Vector4::new3(x, y, -self.distance)),
            )
        } else {
            (
                Vector4::new3(x, y, self.distance),
                Vector4::new3(0., 0., -1.),
            )
        };
        Ray {
            origin: self.center + transform_direction(self.inverse, origin),
            direction: normalize(transform_direction(self.inverse, direction)),
        }
    }
}

fn first_hit(
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    params: &UmbreonRenderParams,
) -> Option<Hit> {
    let rotation = camera.rotation().to_matrix();
    let half = camera.slab_depth() * 0.5;
    index.nearest_where(ray, f64::INFINITY, Some(usize::MAX), |hit| {
        hit.color[3] > 0.
            && (!params.use_clip_z
                || (-half..=half)
                    .contains(&transform_direction(rotation, hit.position - camera.center()).z))
    })
}

#[derive(Clone, Copy, Default)]
struct AoCell {
    normal: [f32; 3],
    bent: [f32; 3],
    depth: f32,
    open: f32,
    contact: f32,
    shape: f32,
}

pub(super) struct AoGrid {
    width: usize,
    height: usize,
    start: usize,
    cells: Vec<AoCell>,
}

impl AoGrid {
    pub fn build(
        snapshot: &RenderSceneSnapshot,
        index: &RayIndex<'_>,
        camera: &Camera,
        params: &UmbreonRenderParams,
        rays: &CameraRays,
        region: std::ops::Range<usize>,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Self>> {
        let samples = params.supersample.clamp(1, 8) as usize;
        let divisor = if params.ao_res_div < 0 {
            samples
        } else {
            params.ao_res_div as usize
        };
        if params.use_gi || params.ao_samples <= 0 || divisor <= 1 {
            return Ok(None);
        }
        let width = (rays.width * samples).div_ceil(divisor).max(1);
        let height = (rays.height * samples).div_ceil(divisor).max(1);
        let start = (region.start * height / rays.height).saturating_sub(1);
        let end = ((region.end * height).div_ceil(rays.height) + 1).min(height);
        let length = width
            .checked_mul(end - start)
            .ok_or_else(|| CueMolError::invalid_argument("AO grid dimensions overflow"))?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(length)
            .map_err(|e| CueMolError::invalid_argument(format!("cannot allocate AO grid: {e}")))?;
        for y in start..end {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(CueMolError::invalid_argument("Render cancelled"));
            }
            for x in 0..width {
                let ray = rays.at(
                    (x as f64 + 0.5) * rays.width as f64 / width as f64,
                    (y as f64 + 0.5) * rays.height as f64 / height as f64,
                );
                let cell = first_hit(index, ray, camera, params)
                    .filter(|h| !h.unlit && !h.is_volume)
                    .map(|hit| {
                        let ao =
                            lighting::occlusion(snapshot, index, hit, params, Some(usize::MAX));
                        let array = |v: Vector4| [v.x as f32, v.y as f32, v.z as f32];
                        AoCell {
                            normal: array(hit.normal),
                            bent: array(ao.bent),
                            depth: hit.distance as f32,
                            open: ao.open as f32,
                            contact: ao.contact as f32,
                            shape: ao.shape as f32,
                        }
                    })
                    .unwrap_or_default();
                cells.push(cell);
            }
        }
        Ok(Some(Self {
            width,
            height,
            start,
            cells,
        }))
    }

    pub fn sample(&self, uv: [f64; 2], hit: Hit) -> Option<lighting::Occlusion> {
        let x = uv[0] * self.width as f64 - 0.5;
        let y = uv[1] * self.height as f64 - 0.5;
        let bx = x.floor() as isize;
        let by = y.floor() as isize;
        let fraction = [x - x.floor(), y - y.floor()];
        let mut sum = 0.;
        let mut out = lighting::Occlusion {
            open: 0.,
            contact: 0.,
            shape: 0.,
            bent: Vector4::ZERO,
        };
        for dy in 0..2 {
            for dx in 0..2 {
                let cx = bx + dx;
                let cy = by + dy;
                if cx < 0 || cx >= self.width as isize || cy < self.start as isize {
                    continue;
                }
                let Some(cell) = self
                    .cells
                    .get((cy as usize - self.start) * self.width + cx as usize)
                else {
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
                let dot = normal.dot3d(hit.normal).clamp(0., 1.);
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
                    * dot.powi(32)
                    * (-(hit.distance - cell.depth as f64).abs() / (0.02 * hit.distance + 1e-6))
                        .exp();
                sum += weight;
                out.open += weight * cell.open as f64;
                out.contact += weight * cell.contact as f64;
                out.shape += weight * cell.shape as f64;
                out.bent = out.bent
                    + Vector4::new3(
                        cell.bent[0] as f64,
                        cell.bent[1] as f64,
                        cell.bent[2] as f64,
                    ) * weight;
            }
        }
        if sum < 0.3 {
            return None;
        }
        out.open /= sum;
        out.contact /= sum;
        out.shape /= sum;
        out.bent = out.bent.normalize3d().unwrap_or(hit.normal);
        Some(out)
    }
}

#[derive(Clone, Copy)]
struct Center {
    color: [f64; 4],
    geometry: Option<(u8, usize)>,
    group: Option<usize>,
    normal: Vector4,
    depth: f64,
}

pub(super) struct AdaptiveGrid {
    width: usize,
    start: usize,
    centers: Vec<Center>,
    refine: Vec<bool>,
    pub samples: usize,
}

impl AdaptiveGrid {
    pub fn build(
        index: &RayIndex<'_>,
        camera: &Camera,
        params: &UmbreonRenderParams,
        rays: &CameraRays,
        region: std::ops::Range<usize>,
        shade: impl Fn(f64, f64) -> [f64; 4],
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Self>> {
        let ss = params.supersample.clamp(1, 8) as usize;
        if params.aa_mode <= 0 || params.use_gi || (ss <= 1 && params.aa_depth <= 1) {
            return Ok(None);
        }
        let start = region.start.saturating_sub(3);
        let end = (region.end + 3).min(rays.height);
        let length = rays
            .width
            .checked_mul(end - start)
            .ok_or_else(|| CueMolError::invalid_argument("AA grid dimensions overflow"))?;
        let mut centers = Vec::new();
        centers
            .try_reserve_exact(length)
            .map_err(|e| CueMolError::invalid_argument(format!("cannot allocate AA grid: {e}")))?;
        let sub = (ss / 2) as f64 + 0.5;
        for y in start..end {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(CueMolError::invalid_argument("Render cancelled"));
            }
            for x in 0..rays.width {
                let px = x as f64 + sub / ss as f64;
                let py = y as f64 + sub / ss as f64;
                let hit = first_hit(index, rays.at(px, py), camera, params);
                centers.push(Center {
                    color: shade(px, py),
                    geometry: hit.map(|h| index.geometry_id(h)),
                    group: hit.and_then(|h| h.transparency_group),
                    normal: hit.map_or(Vector4::ZERO, |h| h.normal),
                    depth: hit.map_or(0., |h| {
                        camera.cam_dist()
                            - transform_direction(
                                camera.rotation().to_matrix(),
                                h.position - camera.center(),
                            )
                            .z
                    }),
                });
            }
        }
        let mut raw = vec![false; length];
        let width = rays.width;
        let rows = end - start;
        let pixel = camera.zoom()
            / rays.height as f64
            / if params.perspective {
                camera.cam_dist().max(1e-9)
            } else {
                1.
            };
        let pair = |a: Center, b: Center, za: f64, zb: f64| {
            a.geometry != b.geometry
                || a.group != b.group
                || a.color
                    .into_iter()
                    .zip(b.color)
                    .any(|(x, y)| (x - y).abs() > 0.1)
                || (a.geometry.is_some()
                    && (1. - a.normal.dot3d(b.normal) > 0.3
                        || (b.depth - a.depth).abs()
                            > (3. * (a.depth - za).abs().min((zb - b.depth).abs())).max(
                                12. * pixel
                                    * if params.perspective {
                                        a.depth.abs().min(b.depth.abs())
                                    } else {
                                        1.
                                    },
                            )))
        };
        for y in 0..rows {
            for x in 0..width {
                let i = y * width + x;
                if ss > 1 {
                    let mut first = None;
                    for sy in 0..ss {
                        for sx in 0..ss {
                            let ray = rays.at(
                                x as f64 + (sx as f64 + 0.5) / ss as f64,
                                (start + y) as f64 + (sy as f64 + 0.5) / ss as f64,
                            );
                            let identity = first_hit(index, ray, camera, params)
                                .map(|hit| (index.geometry_id(hit), hit.transparency_group));
                            if let Some(previous) = first {
                                raw[i] |= identity != previous;
                            } else {
                                first = Some(identity);
                            }
                        }
                    }
                }
                for (step, coordinate, count) in [(1, x, width), (width, y, rows)] {
                    if coordinate + 1 >= count {
                        continue;
                    }
                    let a = centers[i];
                    let b = centers[i + step];
                    let za = if coordinate > 0 {
                        centers[i - step].depth
                    } else {
                        2. * a.depth - b.depth
                    };
                    let zb = if coordinate + 2 < count {
                        centers[i + 2 * step].depth
                    } else {
                        2. * b.depth - a.depth
                    };
                    if pair(a, b, za, zb) {
                        raw[i] = true;
                        raw[i + step] = true;
                    }
                }
            }
        }
        let mut refine = raw.clone();
        for y in 0..rows {
            for x in 0..width {
                if !raw[y * width + x] {
                    continue;
                }
                for yy in y.saturating_sub(1)..=(y + 1).min(rows - 1) {
                    for xx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                        refine[yy * width + xx] = true;
                    }
                }
            }
        }
        let fine = (params.aa_depth.clamp(0, 128) as usize).div_ceil(ss).max(1);
        Ok(Some(Self {
            width,
            start,
            centers,
            refine,
            samples: ss * fine,
        }))
    }

    pub fn center(&self, x: usize, y: usize) -> Option<[f64; 4]> {
        let index = (y - self.start) * self.width + x;
        (!self.refine[index]).then_some(self.centers[index].color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (RenderSceneSnapshot, Camera, UmbreonRenderParams) {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.atoms.push(crate::RenderAtom::new(
            Vector4::new3(0.2, 0.1, 0.),
            1.,
            None,
        ));
        snapshot.atoms.push(crate::RenderAtom::new(
            Vector4::new3(0.9, 0., 0.8),
            0.4,
            None,
        ));
        let mut camera = Camera::new();
        camera.set_zoom(5.);
        camera.set_cam_dist(10.);
        camera.set_slab_depth(30.);
        let params = UmbreonRenderParams {
            width: 31,
            height: 29,
            supersample: 3,
            aa_mode: 1,
            aa_depth: 4,
            ao_samples: 8,
            ao_res_div: -1,
            ao_distance: 2.,
            gi_denoise: false,
            transparent_background: true,
            ..Default::default()
        };
        (snapshot, camera, params)
    }

    #[test]
    fn adaptive_edges_and_coarse_ao_are_independent_of_render_tiles() {
        let (snapshot, camera, params) = scene();
        let index =
            RayIndex::with_pixel_scale(&snapshot, (camera.zoom() / params.height as f64) as f32);
        let expected = render_float(&snapshot, &camera, [0.; 4], &params, None, None).unwrap();
        let mut actual = Vec::new();
        for start in (0..29).step_by(5) {
            actual.extend(
                render_float_region(
                    &snapshot,
                    &camera,
                    [0.; 4],
                    &params,
                    None,
                    None,
                    start..(start + 5).min(29),
                    Some(&index),
                )
                .unwrap()
                .pixels,
            );
        }
        assert_eq!(actual, expected.pixels);
        let rays = CameraRays::new(&camera, 31, 29, params.perspective).unwrap();
        let grid = AdaptiveGrid::build(
            &index,
            &camera,
            &params,
            &rays,
            0..29,
            |x, y| {
                trace_sample(
                    &snapshot,
                    &index,
                    rays.at(x, y),
                    &camera,
                    [0.; 4],
                    &params,
                    None,
                )
            },
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(grid.samples, 6);
        assert!(grid.refine.iter().any(|v| *v));
        assert!(grid.refine.iter().any(|v| !v));
        let mut gi = params.clone();
        gi.use_gi = true;
        assert!(AdaptiveGrid::build(
            &index,
            &camera,
            &gi,
            &rays,
            0..29,
            |_, _| panic!("GI must use the grid path"),
            None
        )
        .unwrap()
        .is_none());
        assert!(
            AoGrid::build(&snapshot, &index, &camera, &gi, &rays, 0..29, None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn coarse_ao_uses_bilateral_guides_and_rejects_occluded_layers() {
        let grid = AoGrid {
            width: 2,
            height: 2,
            start: 0,
            cells: [0.2, 0.4, 0.6, 0.8]
                .map(|open| AoCell {
                    depth: 1.,
                    normal: [0., 0., 1.],
                    bent: [0., 0., 1.],
                    open,
                    contact: open,
                    shape: open,
                })
                .to_vec(),
        };
        let mut hit = Hit {
            order: 0,
            distance: 1.,
            position: Vector4::ZERO,
            normal: Vector4::new3(0., 0., 1.),
            color: [1.; 4],
            unlit: false,
            is_volume: false,
            transparency_group: None,
        };
        let ao = grid.sample([0.5, 0.5], hit).unwrap();
        assert!((ao.open - 0.5).abs() < 1e-6);
        assert_eq!(ao.bent, hit.normal);
        assert!(grid.sample([0., 0.], hit).is_none());
        hit.distance = 3.;
        assert!(grid.sample([0.5, 0.5], hit).is_none());
        hit.distance = 1.;
        hit.normal = hit.normal * -1.;
        assert!(grid.sample([0.5, 0.5], hit).is_none());
    }

    #[test]
    fn bounded_sampling_buffers_match_a_single_render_region() {
        let (snapshot, camera, mut params) = scene();
        params.width = 13;
        params.height = 259;
        params.ao_samples = 2;
        params.supersample = 2;
        params.aa_depth = 3;
        let tiled = render_float(&snapshot, &camera, [0.; 4], &params, None, None).unwrap();
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
        assert_eq!(tiled.pixels, whole.pixels);
    }
}
