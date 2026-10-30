use super::*;
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub(super) struct Light {
    pub direction: Vector4,
    pub energy: f64,
    pub shadow: bool,
}

pub(super) fn lights(camera: &Camera, p: &UmbreonRenderParams) -> [Light; 2] {
    let rot = camera
        .rotation()
        .inv()
        .unwrap_or(cuemol_core::Quat::IDENTITY)
        .to_matrix();
    let (intensity, ambient, flash) = balance(p);
    [
        Light {
            direction: normalize(transform_direction(rot, Vector4::new3(1., 1., 1.))),
            energy: intensity * (1. - ambient) * (1. - flash),
            shadow: true,
        },
        Light {
            direction: normalize(transform_direction(rot, Vector4::new3(0., 0., 1.))),
            energy: intensity * (1. - ambient) * flash,
            shadow: false,
        },
    ]
}
fn balance(p: &UmbreonRenderParams) -> (f64, f64, f64) {
    (
        if p.light_intensity >= 0. {
            p.light_intensity
        } else {
            1.3
        },
        if p.ambient_fraction >= 0. {
            p.ambient_fraction.clamp(0., 1.)
        } else if p.use_gi {
            0.6
        } else {
            0.
        },
        if p.flash_fraction >= 0. {
            p.flash_fraction.clamp(0., 1.)
        } else if p.use_gi {
            0.
        } else {
            0.6
        },
    )
}
fn epsilon(hit: Hit) -> f64 {
    (hit.position
        .x
        .abs()
        .max(hit.position.y.abs())
        .max(hit.position.z.abs())
        * 1e-8)
        .max(1e-4)
}
pub(super) fn shadow(
    index: &RayIndex<'_>,
    hit: Hit,
    light: Light,
    p: &UmbreonRenderParams,
    group: Option<usize>,
) -> f64 {
    if !p.shadows || !light.shadow || light.energy <= 0. {
        return 1.;
    }
    let count = p.shadow_samples.clamp(1, 256) as usize;
    (0..count)
        .filter(|&i| {
            !index.occluded(
                Ray {
                    origin: hit.position + hit.normal * epsilon(hit),
                    direction: cone_sample(
                        light.direction,
                        p.light_radius.max(0.).to_radians(),
                        i,
                        count,
                    ),
                },
                f64::INFINITY,
                group,
            )
        })
        .count() as f64
        / count as f64
}

#[derive(Clone, Copy)]
pub(super) struct Occlusion {
    pub open: f64,
    pub contact: f64,
    pub shape: f64,
    pub bent: Vector4,
}
fn hash(mut v: u64) -> u64 {
    v ^= v >> 30;
    v = v.wrapping_mul(0xbf58476d1ce4e5b9);
    v ^= v >> 27;
    v = v.wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}
fn seed(hit: Hit) -> u64 {
    hash(
        hit.position.x.to_bits()
            ^ hit.position.y.to_bits().rotate_left(21)
            ^ hit.position.z.to_bits().rotate_left(42),
    )
}
fn unit(v: u64) -> f64 {
    (v >> 11) as f64 / ((1u64 << 53) as f64)
}
fn sample(normal: Vector4, i: usize, count: usize, seed: u64, low_discrepancy: bool) -> Vector4 {
    let (u, v) = if low_discrepancy {
        (
            (i as f64 / count as f64 + unit(hash(seed))).fract(),
            ((i as u32).reverse_bits() as f64 / 4294967296. + unit(hash(seed.wrapping_add(1))))
                .fract(),
        )
    } else {
        (
            unit(hash(seed ^ (i as u64 * 2))),
            unit(hash(seed ^ (i as u64 * 2 + 1))),
        )
    };
    let (t, b) = tangent_basis(normal);
    let r = u.sqrt();
    let a = std::f64::consts::TAU * v;
    normalize(t * (r * a.cos()) + b * (r * a.sin()) + normal * (1. - u).sqrt())
}
pub(super) fn occlusion(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    p: &UmbreonRenderParams,
    group: Option<usize>,
) -> Occlusion {
    if p.ao_samples <= 0 {
        return Occlusion {
            open: 1.,
            contact: 1.,
            shape: 1.,
            bent: hit.normal,
        };
    }
    let count = p.ao_samples.clamp(1, 256) as usize;
    let radius = if p.ao_distance > 0. {
        p.ao_distance
    } else {
        snapshot
            .bounds
            .map(|(min, max)| (max - min).length3d() * 0.7)
            .unwrap_or(10.)
            .max(1e-3)
    };
    let radii = if p.ao_multi_scale {
        [0.08 * radius, 0.30 * radius, radius]
    } else {
        [radius; 3]
    };
    let weights = if p.ao_multi_scale {
        [0.55, 0.35, 0.10]
    } else {
        [1., 0., 0.]
    };
    let mut obscured = [0.; 3];
    let mut bent = Vector4::new3(0., 0., 0.);
    let rng = seed(hit);
    for i in 0..count {
        let direction = sample(hit.normal, i, count, rng, p.ao_low_discrepancy);
        let distance = index
            .nearest(
                Ray {
                    origin: hit.position + hit.normal * epsilon(hit),
                    direction,
                },
                radius,
                group,
            )
            .map(|h| h.distance)
            .unwrap_or(f64::INFINITY);
        let mut weighted = 0.;
        for k in 0..3 {
            let f = if distance < radii[k] {
                (1. - distance / radii[k])
                    .max(0.)
                    .powf(p.ao_falloff_power.max(0.))
            } else {
                0.
            };
            obscured[k] += f;
            weighted += weights[k] * f;
        }
        bent = bent + direction * (1. - weighted);
    }
    let open = 1. - (0..3).map(|k| weights[k] * obscured[k]).sum::<f64>() / count as f64;
    Occlusion {
        open,
        contact: 1. - obscured[0] / count as f64,
        shape: if p.ao_multi_scale {
            1. - (0.35 * obscured[1] + 0.10 * obscured[2]) / (0.45 * count as f64)
        } else {
            open
        },
        bent: if bent.length3d() > 1e-12 {
            normalize(bent)
        } else {
            hit.normal
        },
    }
}
fn environment(direction: Vector4, camera: &Camera, p: &UmbreonRenderParams) -> [f64; 3] {
    let (intensity, ambient, _) = balance(p);
    let energy = intensity * ambient * p.gi_env_intensity.max(0.);
    if !p.gi_sky_gradient {
        return [energy; 3];
    }
    let up = transform_direction(
        camera
            .rotation()
            .inv()
            .unwrap_or(cuemol_core::Quat::IDENTITY)
            .to_matrix(),
        Vector4::new3(0., 1., 0.),
    );
    let w = (0.5 * (direction.dot3d(up) + 1.)).clamp(0., 1.);
    let ground = p.gi_ground_color;
    let lum = ground[0] * 0.2126 + ground[1] * 0.7152 + ground[2] * 0.0722;
    ground.map(|g| energy * (g + (1. - g) * w) * 2. / (1. + lum))
}
fn material(snapshot: &RenderSceneSnapshot, hit: Hit) -> MaterialPreset {
    named_material(
        hit.transparency_group
            .and_then(|i| snapshot.transparency_groups.get(i))
            .map(|g| g.material.as_str())
            .unwrap_or("default"),
    )
}

pub(super) fn background_specular(
    snapshot: &RenderSceneSnapshot,
    hit: Hit,
    background: [f32; 4],
    params: &UmbreonRenderParams,
) -> [f64; 3] {
    if hit.unlit || params.use_gi {
        return [0.; 3];
    }
    let MaterialPreset::Physical {
        metallic,
        specular,
        reflection,
        ..
    } = material(snapshot, hit)
    else {
        return [0.; 3];
    };
    let amount = if reflection > 0. {
        [reflection; 3]
    } else {
        bsdf::fresnel_base(hit.color, metallic, specular)
    };
    std::array::from_fn(|i| amount[i] * f64::from(background[i]))
}
fn diffuse_weight(m: MaterialPreset) -> f64 {
    match m {
        MaterialPreset::Physical {
            diffuse, metallic, ..
        } => diffuse * (1. - metallic),
        MaterialPreset::Toon { diffuse, .. } => diffuse,
    }
}

pub(super) fn reflected_radiance(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    direction: Vector4,
    camera: &Camera,
    params: &UmbreonRenderParams,
    group: Option<usize>,
) -> [f64; 3] {
    let sky = environment(direction, camera, params);
    let ray = Ray {
        origin: hit.position + hit.normal * epsilon(hit),
        direction,
    };
    let Some(mut reflected) = index.nearest(ray, f64::INFINITY, group) else {
        return sky;
    };
    if reflected.unlit {
        return [0.; 3];
    }
    if reflected.normal.dot3d(direction) > 0. {
        reflected.normal = reflected.normal * -1.;
    }
    let kd = diffuse_weight(material(snapshot, reflected));
    let direct: f64 = lights(camera, params)
        .into_iter()
        .map(|light| {
            light.energy
                * reflected.normal.dot3d(light.direction).max(0.)
                * shadow(index, reflected, light, params, group)
        })
        .sum();
    std::array::from_fn(|i| kd * reflected.color[i] * (direct + sky[i]))
}

pub(super) fn indirect(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
) -> [f64; 3] {
    let count = p.gi_samples.clamp(1, 256) as usize;
    indirect_samples(snapshot, index, hit, camera, p, group, count, None, false)
}

pub(super) fn indirect_at(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    count: usize,
    pixel: [u32; 2],
    geometric_horizon: bool,
) -> [f64; 3] {
    indirect_samples(
        snapshot,
        index,
        hit,
        camera,
        p,
        group,
        count,
        Some(sobol::Sampler::new(pixel, count)),
        geometric_horizon,
    )
}

fn indirect_samples(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    count: usize,
    sampler: Option<sobol::Sampler>,
    geometric_horizon: bool,
) -> [f64; 3] {
    let mut result = [0.; 3];
    let rng = seed(hit);
    let geometric_normal = geometric_horizon.then(|| index.geometric_normal(hit));
    for i in 0..count {
        let direction = sampler.as_ref().map_or_else(
            || sample(hit.normal, i, count, rng, true),
            |sampler| sampler.direction(hit.normal, i),
        );
        if geometric_normal.is_some_and(|normal| direction.dot3d(normal) <= 0.) {
            continue;
        }
        let mut ray = Ray {
            origin: hit.position + hit.normal * epsilon(hit),
            direction,
        };
        let mut throughput = [1.; 3];
        for bounce in 0..p.gi_bounces.clamp(1, 8) {
            let Some(mut h) = index.nearest(ray, f64::INFINITY, group) else {
                let env = environment(ray.direction, camera, p);
                for c in 0..3 {
                    result[c] += throughput[c] * env[c];
                }
                break;
            };
            if h.unlit {
                break;
            }
            if h.normal.dot3d(ray.direction) > 0. {
                h.normal = h.normal * -1.;
            }
            let kd = diffuse_weight(material(snapshot, h));
            let mut direct = 0.;
            for light in lights(camera, p) {
                direct += light.energy
                    * h.normal.dot3d(light.direction).max(0.)
                    * shadow(index, h, light, p, group);
            }
            for c in 0..3 {
                throughput[c] *= kd * h.color[c];
                result[c] += throughput[c] * direct;
            }
            if throughput.iter().copied().fold(0., f64::max) < 1e-5 {
                break;
            }
            ray = Ray {
                origin: h.position + h.normal * epsilon(h),
                direction: sample(
                    h.normal,
                    i,
                    count,
                    hash(rng.wrapping_add(bounce as u64 + 1)),
                    true,
                ),
            };
        }
    }
    result.map(|v| v * p.gi_intensity.max(0.) / count as f64)
}

pub(super) fn shade(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    cache: Option<&RefCell<PixelCache>>,
) -> [f64; 3] {
    if hit.unlit {
        return [hit.color[0], hit.color[1], hit.color[2]];
    }
    let mat = material(snapshot, hit);
    let view = normalize(ray.direction * -1.);
    let ao = cached_occlusion(snapshot, index, hit, p, group, cache);
    let (ambient, kd, metallic, roughness, specular, phong_size, toon) = match mat {
        MaterialPreset::Physical {
            ambient,
            diffuse,
            metallic,
            roughness,
            specular,
            ..
        } => (
            ambient,
            diffuse * (1. - metallic),
            metallic,
            roughness,
            specular,
            0.,
            false,
        ),
        MaterialPreset::Toon {
            ambient,
            diffuse,
            phong,
            phong_size,
        } => (ambient, diffuse, 0., 0., phong, phong_size, true),
    };
    let mut color = [0.; 3];
    let diffuse_ao = 1. - p.ao_diffuse_factor.clamp(0., 1.) * (1. - ao.open);
    let up = transform_direction(
        camera
            .rotation()
            .inv()
            .unwrap_or(cuemol_core::Quat::IDENTITY)
            .to_matrix(),
        Vector4::new3(0., 1., 0.),
    );
    let amb_tint = if p.ao_bent_normal && p.ao_samples > 0 {
        0.7 + 0.3 * ao.bent.dot3d(up)
    } else {
        1.
    };
    let amb = if p.use_gi && p.gi_intensity > 0. && !toon {
        0.
    } else {
        ambient * (1. - p.ao_intensity.clamp(0., 1.) * (1. - ao.open)) * amb_tint
    };
    for c in 0..3 {
        color[c] = hit.color[c] * amb;
    }
    for light in lights(camera, p) {
        if light.energy <= 0. {
            continue;
        }
        let vis = shadow(index, hit, light, p, group);
        let ndl = hit.normal.dot3d(light.direction).max(0.);
        let highlight = if !light.shadow || ndl <= 0. {
            [0.; 3]
        } else if toon {
            let reflected =
                normalize(hit.normal * (2. * hit.normal.dot3d(light.direction)) - light.direction);
            [specular * reflected.dot3d(view).max(0.).powf(phong_size); 3]
        } else {
            bsdf::direct(
                bsdf::fresnel_base(hit.color, metallic, specular),
                roughness,
                hit.normal,
                view,
                light.direction,
            )
        };
        let ndl = if toon && ndl > 0. { 1. } else { ndl };
        for c in 0..3 {
            color[c] += light.energy * vis * (hit.color[c] * kd * ndl * diffuse_ao + highlight[c]);
        }
    }
    if p.use_gi && p.gi_intensity > 0. && !toon {
        let e = cached_indirect(snapshot, index, hit, camera, p, group, cache);
        for c in 0..3 {
            color[c] += hit.color[c] * kd * e[c];
        }
        let pixel = cache.map_or(0, |cache| {
            let uv = cache.borrow().uv;
            let ss = p.supersample.clamp(1, 8) as f64;
            let width = p.width as f64 * ss;
            (uv[1] * p.height as f64 * ss).floor() as u32 * width as u32
                + (uv[0] * width).floor() as u32
        });
        let reflected = bsdf::reflection(
            snapshot,
            index,
            hit,
            ray,
            camera,
            p,
            group,
            bsdf::fresnel_base(hit.color, metallic, specular),
            roughness,
            pixel,
        );
        for c in 0..3 {
            color[c] += reflected[c];
        }
    }
    color
}

#[derive(Clone, Copy)]
struct CachedSample {
    hit: Hit,
    group: Option<usize>,
    indirect: Option<[f64; 3]>,
}
pub(super) struct PixelCache<'a> {
    grid: Option<&'a super::sampling::AoGrid>,
    indirect_grid: Option<&'a super::gi::GiGrid>,
    uv: [f64; 2],
    radius: f64,
    values: Vec<CachedSample>,
}
impl<'a> PixelCache<'a> {
    pub fn new(radius: f64) -> RefCell<Self> {
        RefCell::new(Self {
            grid: None,
            indirect_grid: None,
            uv: [0.; 2],
            radius,
            values: Vec::new(),
        })
    }
    pub fn with_grids(
        radius: f64,
        grid: Option<&'a super::sampling::AoGrid>,
        indirect: Option<&'a super::gi::GiGrid>,
    ) -> RefCell<Self> {
        let cache = Self::new(radius);
        cache.borrow_mut().grid = grid;
        cache.borrow_mut().indirect_grid = indirect;
        cache
    }
    pub fn set_position(&mut self, uv: [f64; 2]) {
        self.uv = uv;
    }
    pub fn reset(&mut self) {
        self.values.clear();
    }
    fn matching(&self, hit: Hit, group: Option<usize>) -> Option<CachedSample> {
        self.values.iter().copied().find(|v| {
            v.group == group
                && (v.hit.order == hit.order
                    || (hit.transparency_group.is_some()
                        && v.hit.transparency_group == hit.transparency_group))
                && v.hit.normal.dot3d(hit.normal) > 0.995
                && (v.hit.position - hit.position).length3d() < self.radius * 2.
        })
    }
}
pub(super) fn cached_occlusion(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    cache: Option<&RefCell<PixelCache>>,
) -> Occlusion {
    if let Some(cache) = cache {
        let cache = cache.borrow();
        if let Some(ao) = cache.grid.and_then(|grid| grid.sample(cache.uv, hit)) {
            return ao;
        }
    }
    occlusion(snapshot, index, hit, p, group)
}
fn cached_indirect(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    cache: Option<&RefCell<PixelCache>>,
) -> [f64; 3] {
    let reduced_grid = cache.is_some_and(|cache| cache.borrow().indirect_grid.is_some());
    if group.is_none() {
        if let Some(value) = cache.and_then(|cache| {
            let cache = cache.borrow();
            cache
                .indirect_grid
                .and_then(|grid| grid.sample(cache.uv, hit))
        }) {
            return value;
        }
    }
    if reduced_grid {
        let uv = cache.unwrap().borrow().uv;
        let ss = p.supersample.clamp(1, 8) as f64;
        return indirect_at(
            snapshot,
            index,
            hit,
            camera,
            p,
            group,
            p.gi_samples.clamp(1, 256) as usize * 4,
            [
                (uv[0] * p.width as f64 * ss) as u32,
                (uv[1] * p.height as f64 * ss) as u32,
            ],
            false,
        );
    }
    if let Some(v) = cache
        .and_then(|c| c.borrow().matching(hit, group))
        .and_then(|v| v.indirect)
    {
        return v;
    }
    let e = indirect(snapshot, index, hit, camera, p, group);
    if let Some(c) = cache {
        let mut c = c.borrow_mut();
        if let Some(v) = c
            .values
            .iter_mut()
            .find(|v| v.hit.position == hit.position && v.group == group)
        {
            v.indirect = Some(e);
        } else {
            c.values.push(CachedSample {
                hit,
                group,
                indirect: Some(e),
            });
        }
    }
    e
}
