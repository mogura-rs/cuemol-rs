use super::*;

pub(super) fn fresnel_base(color: [f64; 4], metallic: f64, specular: f64) -> [f64; 3] {
    let dielectric = 0.08 * specular;
    std::array::from_fn(|i| dielectric + (color[i] - dielectric) * metallic)
}

fn fresnel(base: [f64; 3], cosine: f64) -> [f64; 3] {
    let weight = (1. - cosine).clamp(0., 1.).powi(5);
    base.map(|value| value + (1. - value) * weight)
}

fn lambda(alpha: f64, cosine: f64) -> f64 {
    let squared = (cosine * cosine).max(1e-12);
    0.5 * ((1. + alpha * alpha * (1. - squared) / squared).sqrt() - 1.)
}

pub(super) fn direct(
    base: [f64; 3],
    roughness: f64,
    normal: Vector4,
    view: Vector4,
    light: Vector4,
) -> [f64; 3] {
    let ndl = normal.dot3d(light);
    if ndl <= 0. || base.iter().all(|value| *value <= 0.) {
        return [0.; 3];
    }
    let half = normalize(light + view);
    let ndh = normal.dot3d(half);
    if ndh <= 0. {
        return [0.; 3];
    }
    let ndv = normal.dot3d(view).max(1e-4);
    let alpha = (roughness * roughness).max(1e-3);
    let denominator = ndh * ndh * (alpha * alpha - 1.) + 1.;
    let masking = 1. / (1. + lambda(alpha, ndv) + lambda(alpha, ndl));
    let weight = alpha * alpha * masking / (denominator * denominator * 4. * ndv);
    fresnel(base, view.dot3d(half)).map(|value| weight * value)
}

fn visible_normal(view: Vector4, alpha: f64, sample: [f32; 2]) -> Vector4 {
    let stretched = normalize(Vector4::new3(alpha * view.x, alpha * view.y, view.z));
    let tangent = if stretched.z < 0.999 {
        normalize(cross(Vector4::new3(0., 0., 1.), stretched))
    } else {
        Vector4::new3(1., 0., 0.)
    };
    let bitangent = cross(stretched, tangent);
    let radius = f64::from(sample[0]).sqrt();
    let angle = std::f64::consts::TAU * f64::from(sample[1]);
    let x = radius * angle.cos();
    let blend = 0.5 * (1. + stretched.z);
    let y = (1. - blend) * (1. - x * x).max(0.).sqrt() + blend * radius * angle.sin();
    let z = (1. - x * x - y * y).max(0.).sqrt();
    let projected = tangent * x + bitangent * y + stretched * z;
    normalize(Vector4::new3(
        alpha * projected.x,
        alpha * projected.y,
        projected.z.max(0.),
    ))
}

pub(super) fn reflection(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    params: &UmbreonRenderParams,
    group: Option<usize>,
    base: [f64; 3],
    roughness: f64,
    pixel: u32,
) -> [f64; 3] {
    if base.iter().all(|value| *value <= 0.) {
        return [0.; 3];
    }
    let alpha = roughness * roughness;
    let view = ray.direction * -1.;
    if alpha < 0.015 {
        let direction = normalize(ray.direction + hit.normal * (2. * view.dot3d(hit.normal)));
        let radiance =
            lighting::reflected_radiance(snapshot, index, hit, direction, camera, params, group);
        let fresnel = fresnel(base, view.dot3d(hit.normal).max(0.));
        return std::array::from_fn(|i| fresnel[i] * radiance[i]);
    }
    let (tangent, bitangent) = sobol::frame(hit.normal);
    let local_view = normalize(Vector4::new3(
        view.dot3d(tangent),
        view.dot3d(bitangent),
        view.dot3d(hit.normal).max(1e-4),
    ));
    let masking_view = lambda(alpha, local_view.z);
    let sampler = sobol::Sampler::glossy(pixel);
    let mut result = [0.; 3];
    for sample in 0..8 {
        let half = visible_normal(local_view, alpha, sampler.sample(sample));
        let cosine = local_view.dot3d(half);
        let reflected = half * (2. * cosine) - local_view;
        if reflected.z <= 0. {
            continue;
        }
        let weight = (1. + masking_view) / (1. + masking_view + lambda(alpha, reflected.z));
        let direction =
            normalize(tangent * reflected.x + bitangent * reflected.y + hit.normal * reflected.z);
        let radiance =
            lighting::reflected_radiance(snapshot, index, hit, direction, camera, params, group);
        let fresnel = fresnel(base, cosine);
        for i in 0..3 {
            result[i] += weight * fresnel[i] * radiance[i] / 8.;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_ggx_has_the_normal_incidence_peak_and_no_backside_lobe() {
        let normal = Vector4::new3(0., 0., 1.);
        let base = fresnel_base([0.8, 0.2, 0.1, 1.], 0., 0.5);
        assert_eq!(base, [0.04; 3]);
        for (roughness, expected) in [(0.5, 0.16), (1., 0.01)] {
            for value in direct(base, roughness, normal, normal, normal) {
                assert!((value - expected).abs() < 1e-12);
            }
        }
        assert_eq!(direct(base, 0.5, normal, normal, normal * -1.), [0.; 3]);
        assert_eq!(direct([0.; 3], 0.5, normal, normal, normal), [0.; 3]);
    }

    #[test]
    fn dielectric_gloss_retains_sky_and_metal_mirrors_keep_the_pigment() {
        let scene = RenderSceneSnapshot::empty();
        let index = RayIndex::new(&scene);
        let normal = Vector4::new3(0., 0., 1.);
        let hit = Hit {
            order: usize::MAX,
            distance: 10.,
            position: Vector4::ZERO,
            normal,
            color: [0.8, 0.2, 0.1, 1.],
            unlit: false,
            is_volume: false,
            transparency_group: None,
        };
        let ray = Ray {
            origin: normal * 10.,
            direction: normal * -1.,
        };
        let mut camera = Camera::new();
        camera.set_cam_dist(10.);
        let params = UmbreonRenderParams {
            use_gi: true,
            light_intensity: 1.,
            ambient_fraction: 1.,
            gi_env_intensity: 1.,
            gi_sky_gradient: false,
            ..Default::default()
        };
        let gloss = reflection(
            &scene,
            &index,
            hit,
            ray,
            &camera,
            &params,
            None,
            fresnel_base(hit.color, 0., 0.4),
            0.3742032,
            0,
        );
        assert!(gloss[0] > 0.02 && gloss[0] < 0.06);
        assert_eq!(gloss[0], gloss[1]);
        assert_eq!(gloss[1], gloss[2]);
        let metal = reflection(
            &scene,
            &index,
            hit,
            ray,
            &camera,
            &params,
            None,
            fresnel_base(hit.color, 1., 0.5),
            0.05,
            0,
        );
        assert_eq!(metal, [0.8, 0.2, 0.1]);
        assert_eq!(
            reflection(&scene, &index, hit, ray, &camera, &params, None, [0.; 3], 0.5, 0),
            [0.; 3]
        );
    }
}
