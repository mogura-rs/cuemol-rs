mod ink;
mod params;
#[cfg(test)]
mod tests;

use super::{lighting, normalize, Hit, Ray, RayIndex, UmbreonFrame, UmbreonRenderParams};
use crate::RenderSceneSnapshot;
use cuemol_core::{CueMolError, Result};
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
    pub base: String,
    pub ink_model: String,
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
            base: String::new(),
            ink_model: String::new(),
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
    match p.coloring.to_ascii_lowercase().as_str() {
        "colored" | "colored ink" | "colored ink on paper" | "colored ink on color fill" => {
            look.albedo_ink = true
        }
        "monochrome" | "ink on paper" | "flat color" | "color fill" | "ink on color fill" => {
            look.albedo_ink = false
        }
        _ => {}
    }
    match p.base.to_ascii_lowercase().as_str() {
        "paper" => look.albedo_base = false,
        "albedo" => look.albedo_base = true,
        _ => {}
    }
    match p.ink_model.to_ascii_lowercase().as_str() {
        "fixed" => look.albedo_ink = false,
        "albedo" => look.albedo_ink = true,
        _ => {}
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
    pub(super) fn render_settings(
        &self,
        params: &UmbreonRenderParams,
        background: [f32; 4],
    ) -> (UmbreonRenderParams, [f32; 4]) {
        let mut settings = params.clone();
        if !self.look.over {
            settings.use_gi = false;
            settings.gi_denoise = false;
            settings.denoiser = 0;
        }
        let background = [
            self.look.paper[0],
            self.look.paper[1],
            self.look.paper[2],
            background[3],
        ];
        (settings, background)
    }

    pub(super) fn new(p: &HatchRenderParams, samples: i32) -> Result<Self> {
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
    let (settings, background) = resolved.render_settings(params, background);
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
    group: Option<usize>,
    cache: Option<&std::cell::RefCell<lighting::PixelCache>>,
) -> f32 {
    let tr = &hatch.look.tone;
    let view = normalize(ray.direction * -1.);
    let mut settings = p.clone();
    if settings.light_intensity < 0. {
        settings.light_intensity = 1.55;
    }
    let wrap = (tr.wrap as f64).max(0.);
    let mut diffuse = 0.;
    let mut specular = 0.;
    for light in lighting::lights(camera, &settings) {
        let visibility = lighting::shadow(index, hit, light, p, group);
        diffuse += ((hit.normal.dot3d(light.direction) + wrap) / (1. + wrap)).clamp(0., 1.)
            * light.energy
            * visibility;
        if light.shadow {
            let reflected =
                normalize(hit.normal * (2. * hit.normal.dot3d(light.direction)) - light.direction);
            specular += reflected.dot3d(view).max(0.).powf(30.) * light.energy * visibility;
        }
    }
    let ao = lighting::cached_occlusion(snapshot, index, hit, p, group, cache);
    let mut t = tr.ambient as f64 + tr.diffuse as f64 * diffuse;
    t *= ao.contact.powf(tr.contact.max(0.) as f64) * ao.shape.powf(tr.shape.max(0.) as f64);
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
    cache: Option<&std::cell::RefCell<lighting::PixelCache>>,
) -> [f64; 4] {
    super::blend_sample(index, p, |group| {
        trace_npr_pass(
            snapshot, index, ray, camera, background, p, h, x, y, group, cache,
        )
    })
}
fn trace_npr_pass(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    background: [f32; 4],
    p: &UmbreonRenderParams,
    h: &ResolvedHatch,
    x: f32,
    y: f32,
    group: Option<usize>,
    cache: Option<&std::cell::RefCell<lighting::PixelCache>>,
) -> ([f64; 4], f64) {
    let hits = super::visible_hits(index, ray, camera, p, group);
    let depth = hits.first().map(|h| h.distance).unwrap_or(f64::INFINITY);
    let mut output = [0.; 4];
    for &hit in &hits {
        let fog = super::fog_at(hit, camera);
        let color = if hit.unlit {
            [
                hit.color[0] as f32,
                hit.color[1] as f32,
                hit.color[2] as f32,
            ]
        } else {
            let mut raw = hit_tone(snapshot, index, hit, ray, camera, p, h, group, cache);
            if h.look.tonefog {
                raw = 1. - (1. - raw) * fog as f32;
            }
            let shaded = if h.look.over {
                lighting::shade(snapshot, index, hit, ray, camera, p, group, cache)
                    .map(|v| v as f32)
            } else {
                [0.; 3]
            };
            let edge = super::edge_at(index, hit, ray, camera, p, group, h.default_edges);
            let style = index.edges.style(hit.transparency_group);
            let composed = h.composite(
                x,
                y,
                raw,
                [
                    hit.color[0] as f32,
                    hit.color[1] as f32,
                    hit.color[2] as f32,
                ],
                shaded,
                edge && style.is_none(),
            );
            if let Some(style) = style.filter(|_| edge) {
                [style.color[0], style.color[1], style.color[2]]
            } else {
                composed
            }
        };
        let alpha =
            super::section_alpha(snapshot, hit) * if p.transparent_background { fog } else { 1. };
        let weight = alpha * (1. - output[3]);
        for c in 0..3 {
            let color = if p.transparent_background {
                color[c] as f64
            } else {
                background[c] as f64 * (1. - fog) + color[c] as f64 * fog
            };
            output[c] += color * weight;
        }
        output[3] += weight;
        if output[3] >= 1. - 1e-6 {
            break;
        }
    }
    if !p.transparent_background {
        for c in 0..3 {
            output[c] += background[c] as f64 * (1. - output[3]);
        }
        output[3] = 1.;
    }
    (output, depth)
}
