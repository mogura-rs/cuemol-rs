mod ink;
mod params;
#[cfg(test)]
mod tests;

use super::{
    cone_sample, hemisphere_visibility, normalize, transform_direction, Hit, Ray, RayIndex,
    UmbreonFrame, UmbreonRenderParams,
};
use crate::RenderSceneSnapshot;
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_qsys::Camera;
use ink::{smooth, NormalizedLayer};
use params::Look;
pub use params::STYLES as HATCH_STYLES;
pub use params::{Kind as HatchLayerKind, Layer as HatchLayer, Look as HatchLook};

#[derive(Debug, Clone)]
pub struct HatchRenderParams {
    pub density: f64,
    pub width_scale: f64,
    pub ink: [u8; 3],
    pub paper: [u8; 3],
    pub colored: bool,
    pub style: String,
    pub coloring: String,
    pub layers_spec: String,
    pub tone_spec: String,
    pub custom_ink: bool,
    pub custom_paper: bool,
    pub default_edges: bool,
    pub tone_strength: f64,
    pub tone_curve: f64,
}
impl Default for HatchRenderParams {
    fn default() -> Self {
        Self {
            density: 1.,
            width_scale: 1.,
            ink: [0; 3],
            paper: [255; 3],
            colored: false,
            style: "richardson".into(),
            coloring: "Style default".into(),
            layers_spec: String::new(),
            tone_spec: String::new(),
            custom_ink: false,
            custom_paper: false,
            default_edges: true,
            tone_strength: 1.,
            tone_curve: 1.,
        }
    }
}
pub fn hatch_style_spec(name: &str) -> Result<String> {
    Ok(Look::style(name)?.spec())
}
pub fn validate_hatch_params(params: &HatchRenderParams) -> Result<()> {
    resolve_look(params).map(|_| ())
}
fn resolve_look(p: &HatchRenderParams) -> Result<Look> {
    let mut look = Look::style(&p.style)?;
    look.apply_spec(&p.layers_spec, 1)?;
    look.apply_spec(&p.tone_spec, 6)?;
    for v in [p.density, p.width_scale, p.tone_strength, p.tone_curve] {
        if !v.is_finite() || v < 0. {
            return Err(CueMolError::invalid_argument(
                "hatch scales must be finite and nonnegative",
            ));
        }
    }
    look.tone.strength *= p.tone_strength as f32;
    look.tone.curve *= p.tone_curve as f32;
    match p.coloring.to_ascii_lowercase().as_str() {
        "style default" | "" => {}
        "monochrome" | "ink on paper" => {
            look.albedo_base = false;
            look.albedo_ink = false;
            look.ink = [0.; 3];
        }
        "colored" | "colored ink" | "colored ink on paper" => {
            look.albedo_base = false;
            look.albedo_ink = true;
        }
        "flat color" | "color fill" | "ink on color fill" => {
            look.albedo_base = true;
            look.albedo_ink = false;
        }
        "colored ink on color fill" => {
            look.albedo_base = true;
            look.albedo_ink = true;
        }
        v => {
            return Err(CueMolError::invalid_argument(format!(
                "unknown hatch coloring: {v}"
            )))
        }
    }
    if p.colored {
        look.albedo_base = true;
    }
    if p.custom_ink {
        look.ink = p.ink.map(|v| v as f32 / 255.);
        look.albedo_ink = false;
    }
    if p.custom_paper {
        look.paper = p.paper.map(|v| v as f32 / 255.);
    }
    Ok(look)
}
pub(super) struct ResolvedHatch {
    look: Look,
    layers: Vec<NormalizedLayer>,
    default_edges: bool,
}
impl ResolvedHatch {
    fn new(p: &HatchRenderParams, samples: i32) -> Result<Self> {
        let look = resolve_look(p)?;
        let ss = samples.clamp(1, 8) as f32;
        let layers = look
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut l = l.clone();
                l.spacing = l.spacing / (p.density.clamp(0.1, 10.) as f32) * ss;
                l.width *= p.width_scale.clamp(0.1, 8.) as f32 * ss;
                l.dotscale *= p.width_scale.clamp(0.1, 8.) as f32;
                l.soft *= ss;
                l.wobble *= ss;
                l.wobwave *= ss;
                l.slen *= ss;
                l.sgap *= ss;
                l.toothscale *= ss;
                NormalizedLayer::new(&l, i as u32)
            })
            .collect();
        Ok(Self {
            look,
            layers,
            default_edges: p.default_edges,
        })
    }
    fn display_tone(&self, raw: f32) -> f32 {
        let tr = &self.look.tone;
        let mut t = ((raw - tr.black) / (tr.white - tr.black).max(1e-4)).clamp(0., 1.);
        if tr.gamma != 1. {
            t = t.powf(tr.gamma.max(0.01));
        }
        t = if t <= 0.0031308 {
            12.92 * t
        } else {
            1.055 * t.powf(1. / 2.4) - 0.055
        };
        if tr.hl < 1. {
            t += (1. - t) * smooth((tr.hl - tr.hlsoft.max(1e-4)).max(0.), tr.hl, t);
        }
        t = 1. - (tr.strength.max(0.) * (1. - t).powf(tr.curve.clamp(0.1, 10.))).clamp(0., 1.);
        if self.look.levels > 1 {
            let n = (self.look.levels - 1) as f32;
            t = (t * n).round() / n;
        }
        t
    }
    fn composite(
        &self,
        x: f32,
        y: f32,
        raw: f32,
        albedo: [f32; 3],
        shaded: [f32; 3],
        edge: bool,
    ) -> [f32; 3] {
        let t = self.display_tone(raw);
        let look = &self.look;
        let mut base = if look.over {
            shaded
        } else if look.albedo_base {
            albedo
        } else {
            look.paper
        };
        if look.albedo_base && look.albedoquant > 1 {
            let n = look.albedoquant as f32;
            base = base.map(|v| (v * n).round() / n);
        }
        if edge {
            base = look.ink;
        }
        let mut ink = if look.albedo_ink { albedo } else { look.ink };
        if look.inkshade < 1. {
            ink = ink.map(|v| v * (look.inkshade + (1. - look.inkshade) * t));
        }
        let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let lb = luma(base);
        let li = luma(ink);
        if look.mincontrast > 0. && lb - li < look.mincontrast {
            let target = if lb >= look.mincontrast {
                (lb - look.mincontrast).max(0.)
            } else {
                (lb + look.mincontrast).min(1.)
            };
            ink = if li > 1e-4 {
                ink.map(|v| (v * target / li).clamp(0., 1.))
            } else {
                [if lb >= look.mincontrast { 0. } else { target }; 3]
            };
        }
        for layer in &self.layers {
            let c = layer.coverage(x, y, t) * layer.layer.opacity.clamp(0., 1.);
            for i in 0..3 {
                base[i] *= 1. - c * (1. - ink[i] * layer.layer.inkscale.clamp(0., 1.));
            }
        }
        base
    }
}

pub fn render_umbreon_npr_snapshot(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    hatch: &HatchRenderParams,
) -> Result<UmbreonFrame> {
    let resolved = ResolvedHatch::new(hatch, params.supersample)?;
    let mut settings = params.clone();
    if !resolved.look.over {
        settings.use_gi = false;
        settings.gi_denoise = false;
        settings.denoiser = 0;
    }
    let background = [
        resolved.look.paper[0],
        resolved.look.paper[1],
        resolved.look.paper[2],
        background[3],
    ];
    super::render_with_hatch(
        snapshot,
        camera,
        background,
        &settings,
        None,
        Some(&resolved),
    )
}

fn hit_tone(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    p: &UmbreonRenderParams,
    hatch: &ResolvedHatch,
) -> f32 {
    let tr = &hatch.look.tone;
    let rot = camera
        .rotation()
        .inv()
        .unwrap_or(cuemol_core::Quat::IDENTITY)
        .to_matrix();
    let view = normalize(ray.direction * -1.);
    let key = normalize(transform_direction(rot, Vector4::new3(1., 1., 1.)));
    let flash = normalize(transform_direction(rot, Vector4::new3(0., 0., 1.)));
    let intensity = if p.light_intensity >= 0. {
        p.light_intensity
    } else {
        1.55
    };
    let af = if p.ambient_fraction >= 0. {
        p.ambient_fraction.clamp(0., 1.)
    } else {
        0.
    };
    let ff = if p.flash_fraction >= 0. {
        p.flash_fraction.clamp(0., 1.)
    } else {
        0.6
    };
    let wrap = (tr.wrap as f64).max(0.);
    let mut diffuse = 0.;
    let mut specular = 0.;
    for (direction, energy, shadow) in [
        (key, intensity * (1. - af) * (1. - ff), true),
        (flash, intensity * (1. - af) * ff, false),
    ] {
        let mut visibility = 1.;
        if shadow && p.shadows {
            let count = p.shadow_samples.clamp(1, 256) as usize;
            visibility = (0..count)
                .filter(|&i| {
                    !index.occluded(
                        Ray {
                            origin: hit.position + hit.normal * 1e-4,
                            direction: cone_sample(
                                direction,
                                p.light_radius.max(0.).to_radians(),
                                i,
                                count,
                            ),
                        },
                        f64::INFINITY,
                        None,
                    )
                })
                .count() as f64
                / count as f64;
        }
        diffuse += ((hit.normal.dot3d(direction) + wrap) / (1. + wrap)).clamp(0., 1.)
            * energy
            * visibility;
        if shadow {
            let reflected = normalize(hit.normal * (2. * hit.normal.dot3d(direction)) - direction);
            specular += reflected.dot3d(view).max(0.).powf(30.) * energy * visibility;
        }
    }
    let ao = if p.ao_samples > 0 {
        hemisphere_visibility(
            snapshot,
            index,
            hit,
            p.ao_samples.clamp(1, 256) as usize,
            if p.ao_distance > 0. {
                p.ao_distance
            } else {
                10.
            },
            None,
        )
    } else {
        1.
    };
    let mut t = tr.ambient as f64 + tr.diffuse as f64 * diffuse;
    t *= ao.powf(tr.contact.max(0.) as f64) * ao.powf(tr.shape.max(0.) as f64);
    if tr.rim > 0. {
        let edge = (1. - hit.normal.dot3d(view).clamp(0., 1.)).powf(tr.rimpow.max(0.01) as f64);
        t *= 1. - tr.rim.min(1.) as f64 * edge * (1. - tr.rimbias as f64 * t.clamp(0., 1.));
    }
    if tr.speccut > 0. && specular > tr.speccut as f64 {
        t = 1.;
    }
    t as f32
}
pub(super) fn trace_npr(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    background: [f32; 4],
    p: &UmbreonRenderParams,
    h: &ResolvedHatch,
    x: f32,
    y: f32,
) -> [f64; 4] {
    let mut hits = index.hits(ray, Some(usize::MAX));
    if p.use_clip_z {
        let rotation = camera.rotation().to_matrix();
        let half = camera.slab_depth() * 0.5;
        hits.retain(|hit| {
            let eye = transform_direction(rotation, hit.position - camera.center());
            (-half..=half).contains(&eye.z)
        });
    }
    hits.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then(a.order.cmp(&b.order))
    });
    let mut output = if p.transparent_background {
        [0.; 4]
    } else {
        [
            background[0] as f64,
            background[1] as f64,
            background[2] as f64,
            1.,
        ]
    };
    for &hit in hits.iter().rev() {
        let eye = transform_direction(
            camera.rotation().to_matrix(),
            hit.position - camera.center(),
        );
        let fog = if camera.slab_depth() > 0. {
            (1. + eye.z / (0.5 * camera.slab_depth())).clamp(0., 1.)
        } else {
            1.
        };
        let mut color = if hit.unlit {
            [
                hit.color[0] as f32,
                hit.color[1] as f32,
                hit.color[2] as f32,
            ]
        } else {
            let mut raw = hit_tone(snapshot, index, hit, ray, camera, p, h);
            if h.look.tonefog {
                raw = 1. - (1. - raw) * fog as f32;
            }
            let shaded = if h.look.over {
                super::shade_hit(snapshot, index, hit, ray, camera, p, None).map(|v| v as f32)
            } else {
                [0.; 3]
            };
            let edge = h.default_edges
                && p.show_edge_lines
                && hit.normal.dot3d(ray.direction * -1.).abs()
                    < (0.025 + p.edge_rise.max(0.) * 0.02).min(0.2);
            h.composite(
                x,
                y,
                raw,
                [
                    hit.color[0] as f32,
                    hit.color[1] as f32,
                    hit.color[2] as f32,
                ],
                shaded,
                edge,
            )
        };
        let mut alpha = hit.color[3].clamp(0., 1.);
        if p.transparent_background {
            alpha *= fog;
        } else {
            for i in 0..3 {
                color[i] = background[i] * (1. - fog as f32) + color[i] * fog as f32;
            }
        }
        for c in 0..3 {
            output[c] = color[c] as f64 * alpha + output[c] * (1. - alpha);
        }
        output[3] = alpha + output[3] * (1. - alpha);
    }
    output
}
