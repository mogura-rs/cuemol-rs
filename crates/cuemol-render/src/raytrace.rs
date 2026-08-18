use std::fs;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_gfx::ColorSpec;
use cuemol_qsys::Camera;

use crate::{ExportState, RenderSceneSnapshot};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UmbreonSmokeResult {
    pub width: i32,
    pub height: i32,
    pub render_seconds: f64,
    pub non_background_pixels: usize,
}

/// User-facing Umbreon denoise modes used by the Tritium render settings.
///
/// Umbreon exposes two independent low-level controls: `gi_denoise` applies
/// OIDN to the indirect-irradiance buffer, while `denoiser` selects a
/// full-frame post-pass. The UI composes those controls into these three
/// mutually exclusive modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UmbreonDenoiseMode {
    #[default]
    Oidn,
    Atrous,
    None,
}

impl UmbreonDenoiseMode {
    pub const ALL: [Self; 3] = [Self::Oidn, Self::Atrous, Self::None];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Oidn => "OIDN",
            Self::Atrous => "A-trous",
            Self::None => "None",
        }
    }

    pub const fn exporter_settings(self) -> (bool, i32) {
        match self {
            Self::Oidn => (true, 0),
            Self::Atrous => (false, 1),
            Self::None => (false, 0),
        }
    }

    /// Parse a Tritium setting, falling back to OIDN like the reference
    /// backend does for an absent or unknown value.
    pub fn from_label_or_default(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|mode| mode.label() == value)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UmbreonRenderParams {
    pub width: i32,
    pub height: i32,
    pub perspective: bool,
    pub use_clip_z: bool,
    pub supersample: i32,
    pub ao_samples: i32,
    pub ao_distance: f64,
    pub ao_intensity: f64,
    pub shadows: bool,
    pub shadow_samples: i32,
    pub light_radius: f64,
    pub show_edge_lines: bool,
    pub crease_limit: f64,
    pub edge_rise: f64,
    pub transparent_background: bool,
    pub use_gi: bool,
    pub gi_samples: i32,
    pub gi_intensity: f64,
    pub gi_env_intensity: f64,
    pub gi_denoise: bool,
    pub denoiser: i32,
}

impl Default for UmbreonRenderParams {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            perspective: true,
            use_clip_z: true,
            supersample: 3,
            ao_samples: 0,
            ao_distance: 1.0e20,
            ao_intensity: 1.0,
            shadows: false,
            shadow_samples: 1,
            light_radius: 0.0,
            show_edge_lines: true,
            crease_limit: -1.0,
            edge_rise: 0.5,
            transparent_background: false,
            use_gi: false,
            gi_samples: 32,
            gi_intensity: 1.0,
            gi_env_intensity: 1.0,
            gi_denoise: true,
            denoiser: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UmbreonFrame {
    pub width: i32,
    pub height: i32,
    pub components: i32,
    pub pixels: Vec<u8>,
    pub render_seconds: f64,
}

#[derive(Debug, Clone)]
struct UmbreonRenderInput {
    snapshot: RenderSceneSnapshot,
    camera: Camera,
    background: [f32; 4],
}

#[derive(Debug)]
struct AsyncRender {
    handle: JoinHandle<Result<UmbreonFrame>>,
    progress: Arc<AtomicU32>,
    phase: Arc<Mutex<&'static str>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct UmbreonSceneExporter {
    state: ExportState,
    params: UmbreonRenderParams,
    input: Option<UmbreonRenderInput>,
    async_render: Option<AsyncRender>,
    was_cancelled: bool,
}

impl Default for UmbreonSceneExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl UmbreonSceneExporter {
    pub fn new() -> Self {
        Self {
            state: ExportState::new("umbreon", "Umbreon ray-traced image (*.png)", "*.png"),
            params: UmbreonRenderParams::default(),
            input: None,
            async_render: None,
            was_cancelled: false,
        }
    }

    pub fn state(&self) -> &ExportState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ExportState {
        &mut self.state
    }

    pub fn scene(&self) -> &cuemol_qsys::SceneExporter {
        self.state.base()
    }

    pub fn scene_mut(&mut self) -> &mut cuemol_qsys::SceneExporter {
        self.state.base_mut()
    }

    pub fn handler(&self) -> &cuemol_qsys::InOutHandler {
        self.state.handler()
    }

    pub fn handler_mut(&mut self) -> &mut cuemol_qsys::InOutHandler {
        self.state.handler_mut()
    }

    pub fn params(&self) -> &UmbreonRenderParams {
        &self.params
    }

    pub fn width(&self) -> i32 {
        self.scene().width()
    }

    pub fn set_width(&mut self, value: i32) {
        self.scene_mut().set_width(value);
    }

    pub fn height(&self) -> i32 {
        self.scene().height()
    }

    pub fn set_height(&mut self, value: i32) {
        self.scene_mut().set_height(value);
    }

    pub fn perspective(&self) -> bool {
        self.params.perspective
    }

    pub fn set_perspective(&mut self, value: bool) {
        self.params.perspective = value;
    }

    pub fn use_clip_z(&self) -> bool {
        self.params.use_clip_z
    }

    pub fn set_use_clip_z(&mut self, value: bool) {
        self.params.use_clip_z = value;
    }

    pub fn supersample(&self) -> i32 {
        self.params.supersample
    }

    pub fn set_supersample(&mut self, value: i32) {
        self.params.supersample = value;
    }

    pub fn ao_samples(&self) -> i32 {
        self.params.ao_samples
    }

    pub fn set_ao_samples(&mut self, value: i32) {
        self.params.ao_samples = value;
    }

    pub fn ao_distance(&self) -> f64 {
        self.params.ao_distance
    }

    pub fn set_ao_distance(&mut self, value: f64) {
        self.params.ao_distance = value;
    }

    pub fn ao_intensity(&self) -> f64 {
        self.params.ao_intensity
    }

    pub fn set_ao_intensity(&mut self, value: f64) {
        self.params.ao_intensity = value;
    }

    pub fn shadows(&self) -> bool {
        self.params.shadows
    }

    pub fn set_shadows(&mut self, value: bool) {
        self.params.shadows = value;
    }

    pub fn shadow_samples(&self) -> i32 {
        self.params.shadow_samples
    }

    pub fn set_shadow_samples(&mut self, value: i32) {
        self.params.shadow_samples = value;
    }

    pub fn light_radius(&self) -> f64 {
        self.params.light_radius
    }

    pub fn set_light_radius(&mut self, value: f64) {
        self.params.light_radius = value;
    }

    pub fn show_edge_lines(&self) -> bool {
        self.params.show_edge_lines
    }

    pub fn set_show_edge_lines(&mut self, value: bool) {
        self.params.show_edge_lines = value;
    }

    pub fn crease_limit(&self) -> f64 {
        self.params.crease_limit
    }

    pub fn set_crease_limit(&mut self, value: f64) {
        self.params.crease_limit = value;
    }

    pub fn edge_rise(&self) -> f64 {
        self.params.edge_rise
    }

    pub fn set_edge_rise(&mut self, value: f64) {
        self.params.edge_rise = value;
    }

    pub fn transparent_background(&self) -> bool {
        self.params.transparent_background
    }

    pub fn set_transparent_background(&mut self, value: bool) {
        self.params.transparent_background = value;
    }

    pub fn use_gi(&self) -> bool {
        self.params.use_gi
    }

    pub fn set_use_gi(&mut self, value: bool) {
        self.params.use_gi = value;
    }

    pub fn gi_samples(&self) -> i32 {
        self.params.gi_samples
    }

    pub fn set_gi_samples(&mut self, value: i32) {
        self.params.gi_samples = value;
    }

    pub fn gi_intensity(&self) -> f64 {
        self.params.gi_intensity
    }

    pub fn set_gi_intensity(&mut self, value: f64) {
        self.params.gi_intensity = value;
    }

    pub fn gi_env_intensity(&self) -> f64 {
        self.params.gi_env_intensity
    }

    pub fn set_gi_env_intensity(&mut self, value: f64) {
        self.params.gi_env_intensity = value;
    }

    pub fn gi_denoise(&self) -> bool {
        self.params.gi_denoise
    }

    pub fn set_gi_denoise(&mut self, value: bool) {
        self.params.gi_denoise = value;
    }

    pub fn denoiser(&self) -> i32 {
        self.params.denoiser
    }

    pub fn set_denoiser(&mut self, value: i32) {
        self.params.denoiser = value;
    }

    pub fn set_denoise_mode(&mut self, mode: UmbreonDenoiseMode) {
        let (gi_denoise, denoiser) = mode.exporter_settings();
        self.set_gi_denoise(gi_denoise);
        self.set_denoiser(denoiser);
    }

    pub fn set_render_input(
        &mut self,
        snapshot: RenderSceneSnapshot,
        camera: Camera,
        background: [f32; 4],
    ) {
        self.input = Some(UmbreonRenderInput {
            snapshot,
            camera,
            background,
        });
    }

    pub fn write_snapshot(
        &mut self,
        snapshot: &RenderSceneSnapshot,
        camera: &Camera,
        background: [f32; 4],
    ) -> Result<UmbreonFrame> {
        self.set_render_input(snapshot.clone(), camera.clone(), background);
        self.write()
    }

    pub fn write(&mut self) -> Result<UmbreonFrame> {
        if self.async_render.is_some() {
            return Err(CueMolError::invalid_argument(
                "umbreon render already in progress",
            ));
        }
        let input = self
            .input
            .as_ref()
            .ok_or_else(|| CueMolError::invalid_argument("umbreon render input is not set"))?;
        let params = self.resolved_params();
        let frame =
            render_umbreon_snapshot(&input.snapshot, &input.camera, input.background, &params)?;
        self.write_frame(&frame)?;
        Ok(frame)
    }

    pub fn begin_render(&mut self) -> Result<()> {
        if self.async_render.is_some() {
            return Err(CueMolError::invalid_argument(
                "umbreon render already in progress",
            ));
        }
        let input = self
            .input
            .clone()
            .ok_or_else(|| CueMolError::invalid_argument("umbreon render input is not set"))?;
        let params = self.resolved_params();
        let progress = Arc::new(AtomicU32::new(0));
        let phase = Arc::new(Mutex::new("Rendering"));
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_progress = Arc::clone(&progress);
        let thread_phase = Arc::clone(&phase);
        let thread_cancel = Arc::clone(&cancel);
        let handle = thread::spawn(move || {
            let result = render_umbreon_snapshot_impl(
                &input.snapshot,
                &input.camera,
                input.background,
                &params,
                Some((&thread_progress, &thread_cancel)),
            );
            *thread_phase.lock().unwrap() = if thread_cancel.load(Ordering::Relaxed) {
                "Cancelled"
            } else {
                "Done"
            };
            result
        });
        self.was_cancelled = false;
        self.async_render = Some(AsyncRender {
            handle,
            progress,
            phase,
            cancel,
        });
        Ok(())
    }

    pub fn get_render_progress(&self) -> f64 {
        self.async_render.as_ref().map_or(0.0, |render| {
            f64::from(render.progress.load(Ordering::Relaxed)) / 1_000_000.0
        })
    }

    pub fn get_render_phase_name(&self) -> String {
        self.async_render.as_ref().map_or_else(
            || "Idle".to_string(),
            |render| render.phase.lock().unwrap().to_string(),
        )
    }

    pub fn is_render_done(&self) -> bool {
        self.async_render
            .as_ref()
            .is_none_or(|render| render.handle.is_finished())
    }

    pub fn cancel_render(&self) {
        if let Some(render) = &self.async_render {
            render.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn end_render(&mut self) -> Result<Option<UmbreonFrame>> {
        let render = self
            .async_render
            .take()
            .ok_or_else(|| CueMolError::invalid_argument("umbreon render is not in progress"))?;
        let cancelled = render.cancel.load(Ordering::Relaxed);
        let result = render
            .handle
            .join()
            .map_err(|_| CueMolError::invalid_argument("umbreon render thread panicked"))?;
        self.was_cancelled = cancelled;
        if cancelled {
            return Ok(None);
        }
        let frame = result?;
        self.write_frame(&frame)?;
        Ok(Some(frame))
    }

    pub fn was_render_cancelled(&self) -> bool {
        self.was_cancelled
    }

    pub fn write_count(&self) -> usize {
        self.state.write_count()
    }

    fn resolved_params(&self) -> UmbreonRenderParams {
        let mut params = self.params.clone();
        params.width = if self.width() > 0 { self.width() } else { 640 };
        params.height = if self.height() > 0 {
            self.height()
        } else {
            480
        };
        params
    }

    fn write_frame(&mut self, frame: &UmbreonFrame) -> Result<()> {
        let path = self.handler().path();
        if path.is_empty() {
            return Err(CueMolError::invalid_argument(
                "umbreon output path is empty",
            ));
        }
        fs::write(path, encode_umbreon_png(frame)?)?;
        self.state.write();
        Ok(())
    }
}

pub fn render_umbreon_snapshot(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
) -> Result<UmbreonFrame> {
    render_umbreon_snapshot_impl(snapshot, camera, background, params, None)
}

fn render_umbreon_snapshot_impl(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    async_state: Option<(&AtomicU32, &AtomicBool)>,
) -> Result<UmbreonFrame> {
    let width = usize::try_from(params.width)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| CueMolError::invalid_argument("umbreon width must be positive"))?;
    let height = usize::try_from(params.height)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| CueMolError::invalid_argument("umbreon height must be positive"))?;
    let samples = params.supersample.clamp(1, 8) as usize;
    let components = if params.transparent_background { 4 } else { 3 };
    let mut pixels = vec![0_u8; width * height * components];
    let started = Instant::now();
    let aspect = width as f64 / height as f64;
    let half_h = camera.zoom() * 0.5;
    let half_w = half_h * aspect;
    let inverse_rotation = camera.rotation().inv()?.to_matrix();
    let camera_center = camera.center();

    for y in 0..height {
        if async_state.is_some_and(|(_, cancel)| cancel.load(Ordering::Relaxed)) {
            return Ok(UmbreonFrame {
                width: params.width,
                height: params.height,
                components: components as i32,
                pixels: Vec::new(),
                render_seconds: started.elapsed().as_secs_f64(),
            });
        }
        for x in 0..width {
            let mut accumulated = [0.0_f64; 4];
            for sy in 0..samples {
                for sx in 0..samples {
                    let fx = (x as f64 + (sx as f64 + 0.5) / samples as f64) / width as f64;
                    let fy = (y as f64 + (sy as f64 + 0.5) / samples as f64) / height as f64;
                    let eye_x = (fx * 2.0 - 1.0) * half_w;
                    let eye_y = (1.0 - fy * 2.0) * half_h;
                    let (eye_origin, eye_direction) = if params.perspective {
                        (
                            Vector4::new3(0.0, 0.0, camera.cam_dist()),
                            normalize(Vector4::new3(eye_x, eye_y, -camera.cam_dist())),
                        )
                    } else {
                        (
                            Vector4::new3(eye_x, eye_y, camera.cam_dist()),
                            Vector4::new3(0.0, 0.0, -1.0),
                        )
                    };
                    let origin = camera_center + transform_direction(inverse_rotation, eye_origin);
                    let direction = normalize(transform_direction(inverse_rotation, eye_direction));
                    let sample = trace_sample(
                        snapshot,
                        Ray { origin, direction },
                        camera,
                        background,
                        params,
                    );
                    for index in 0..4 {
                        accumulated[index] += sample[index];
                    }
                }
            }
            let divisor = (samples * samples) as f64;
            let offset = (y * width + x) * components;
            for index in 0..components {
                pixels[offset + index] =
                    ((accumulated[index] / divisor).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        if let Some((progress, _)) = async_state {
            progress.store(
                (((y + 1) as f64 / height as f64) * 1_000_000.0).round() as u32,
                Ordering::Relaxed,
            );
        }
    }

    if params.denoiser > 0 || (params.use_gi && params.gi_denoise) {
        denoise_pixels(&mut pixels, width, height, components, params.denoiser);
    }

    Ok(UmbreonFrame {
        width: params.width,
        height: params.height,
        components: components as i32,
        pixels,
        render_seconds: started.elapsed().as_secs_f64(),
    })
}

#[derive(Clone, Copy)]
struct Ray {
    origin: Vector4,
    direction: Vector4,
}

#[derive(Clone, Copy)]
struct Hit {
    distance: f64,
    position: Vector4,
    normal: Vector4,
    color: [f64; 4],
    unlit: bool,
}

fn trace_sample(
    snapshot: &RenderSceneSnapshot,
    ray: Ray,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
) -> [f64; 4] {
    let mut hits = scene_hits(snapshot, ray);
    if params.use_clip_z {
        let rotation = camera.rotation().to_matrix();
        let half_slab = camera.slab_depth() * 0.5;
        hits.retain(|hit| {
            let eye = transform_direction(rotation, hit.position - camera.center());
            (-half_slab..=half_slab).contains(&eye.z)
        });
    }
    hits.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    let mut out = if params.transparent_background {
        [0.0, 0.0, 0.0, 0.0]
    } else {
        [
            f64::from(background[0]),
            f64::from(background[1]),
            f64::from(background[2]),
            1.0,
        ]
    };
    for hit in hits.iter().rev() {
        let shaded = shade_hit(snapshot, *hit, ray, camera, params);
        let alpha = hit.color[3].clamp(0.0, 1.0);
        for channel in 0..3 {
            out[channel] = shaded[channel] * alpha + out[channel] * (1.0 - alpha);
        }
        out[3] = alpha + out[3] * (1.0 - alpha);
    }
    out
}

fn shade_hit(
    snapshot: &RenderSceneSnapshot,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    params: &UmbreonRenderParams,
) -> [f64; 3] {
    if hit.unlit {
        return [hit.color[0], hit.color[1], hit.color[2]];
    }
    let camera_position = camera.center()
        + transform_direction(
            camera
                .rotation()
                .inv()
                .unwrap_or(cuemol_core::Quat::IDENTITY)
                .to_matrix(),
            Vector4::new3(0.0, 0.0, camera.cam_dist()),
        );
    let camera_direction = normalize(camera_position - hit.position);
    let light = normalize(Vector4::new3(0.35, 0.55, 1.0) + camera_direction * 0.5);
    let diffuse = hit.normal.dot3d(light).max(0.0);
    let mut ambient = 0.24;
    if params.use_gi {
        let openness = hemisphere_visibility(
            snapshot,
            hit,
            params.gi_samples.clamp(1, 64) as usize,
            f64::INFINITY,
        );
        ambient +=
            0.18 * params.gi_intensity.max(0.0) * params.gi_env_intensity.max(0.0) * openness;
    }
    let mut illumination = ambient + 0.76 * diffuse;
    if params.shadows {
        let sample_count = params.shadow_samples.clamp(1, 64) as usize;
        let angular_radius = params.light_radius.max(0.0).to_radians();
        let mut visible = 0_usize;
        for sample in 0..sample_count {
            let direction = cone_sample(light, angular_radius, sample, sample_count);
            let shadow_ray = Ray {
                origin: hit.position + hit.normal * 1.0e-4,
                direction,
            };
            if !has_occluder(snapshot, shadow_ray, f64::INFINITY) {
                visible += 1;
            }
        }
        let visibility = visible as f64 / sample_count as f64;
        illumination *= 0.45 + 0.55 * visibility;
    }
    if params.ao_samples > 0 {
        let openness = hemisphere_visibility(
            snapshot,
            hit,
            params.ao_samples.clamp(1, 64) as usize,
            params.ao_distance.max(0.0),
        );
        let occlusion = 1.0 - openness;
        illumination *= 1.0 - 0.55 * params.ao_intensity.clamp(0.0, 1.0) * occlusion;
    }
    if params.show_edge_lines {
        let rim = hit.normal.dot3d(ray.direction * -1.0).abs();
        if rim < (0.08 + params.edge_rise.max(0.0) * 0.08).min(0.45) {
            illumination *= 0.12;
        }
    }
    [
        hit.color[0] * illumination,
        hit.color[1] * illumination,
        hit.color[2] * illumination,
    ]
}

fn hemisphere_visibility(
    snapshot: &RenderSceneSnapshot,
    hit: Hit,
    sample_count: usize,
    max_distance: f64,
) -> f64 {
    let mut visible = 0_usize;
    for sample in 0..sample_count {
        let ray = Ray {
            origin: hit.position + hit.normal * 1.0e-4,
            direction: hemisphere_sample(hit.normal, sample, sample_count),
        };
        if !has_occluder(snapshot, ray, max_distance) {
            visible += 1;
        }
    }
    visible as f64 / sample_count as f64
}

fn has_occluder(snapshot: &RenderSceneSnapshot, ray: Ray, max_distance: f64) -> bool {
    scene_hits(snapshot, ray).iter().any(|hit| {
        hit.distance > 1.0e-4 && (!max_distance.is_finite() || hit.distance <= max_distance)
    })
}

fn hemisphere_sample(normal: Vector4, sample: usize, sample_count: usize) -> Vector4 {
    let z = (sample as f64 + 0.5) / sample_count as f64;
    let radius = (1.0 - z * z).sqrt();
    let angle = sample as f64 * 2.399_963_229_728_653;
    let (tangent, bitangent) = tangent_basis(normal);
    normalize(tangent * (radius * angle.cos()) + bitangent * (radius * angle.sin()) + normal * z)
}

fn cone_sample(direction: Vector4, radius: f64, sample: usize, sample_count: usize) -> Vector4 {
    if radius <= 1.0e-9 || sample_count == 1 {
        return direction;
    }
    let radial = ((sample as f64 + 0.5) / sample_count as f64).sqrt() * radius.tan();
    let angle = sample as f64 * 2.399_963_229_728_653;
    let (tangent, bitangent) = tangent_basis(direction);
    normalize(direction + tangent * (radial * angle.cos()) + bitangent * (radial * angle.sin()))
}

fn tangent_basis(normal: Vector4) -> (Vector4, Vector4) {
    let helper = if normal.z.abs() < 0.9 {
        Vector4::new3(0.0, 0.0, 1.0)
    } else {
        Vector4::new3(0.0, 1.0, 0.0)
    };
    let tangent = normalize(cross(helper, normal));
    (tangent, normalize(cross(normal, tangent)))
}

fn denoise_pixels(pixels: &mut [u8], width: usize, height: usize, components: usize, mode: i32) {
    let source = pixels.to_vec();
    let radius = if mode >= 2 { 2_isize } else { 1_isize };
    for y in 0..height {
        for x in 0..width {
            let center_offset = (y * width + x) * components;
            let center_luma = pixel_luma(&source[center_offset..center_offset + 3]);
            let mut sums = [0.0_f64; 3];
            let mut weight_sum = 0.0;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let nx = (x as isize + dx).clamp(0, width as isize - 1) as usize;
                    let ny = (y as isize + dy).clamp(0, height as isize - 1) as usize;
                    let offset = (ny * width + nx) * components;
                    if components == 4
                        && source[offset + 3].abs_diff(source[center_offset + 3]) > 24
                    {
                        continue;
                    }
                    let luma = pixel_luma(&source[offset..offset + 3]);
                    let range_weight = (-(luma - center_luma).abs() / 24.0).exp();
                    let spatial_weight = 1.0 / (1.0 + (dx * dx + dy * dy) as f64);
                    let weight = range_weight * spatial_weight;
                    for channel in 0..3 {
                        sums[channel] += f64::from(source[offset + channel]) * weight;
                    }
                    weight_sum += weight;
                }
            }
            if weight_sum > 0.0 {
                for channel in 0..3 {
                    pixels[center_offset + channel] =
                        (sums[channel] / weight_sum).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

fn pixel_luma(pixel: &[u8]) -> f64 {
    0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
}

fn scene_hits(snapshot: &RenderSceneSnapshot, ray: Ray) -> Vec<Hit> {
    let mut hits = Vec::new();
    for atom in snapshot.atoms.iter().filter(|atom| atom.hit.is_none()) {
        let main_hit = ray_sphere(ray, atom.center, f64::from(atom.radius));
        if let Some((distance, normal)) = main_hit {
            hits.push(Hit {
                distance,
                position: ray.origin + ray.direction * distance,
                normal,
                color: color_from_spec(atom.color.as_ref()),
                unlit: false,
            });
        } else if let Some(edge) = atom.edge.filter(|edge| edge.width > 0.0) {
            if let Some((distance, normal)) =
                ray_sphere(ray, atom.center, f64::from(atom.radius + edge.width))
            {
                hits.push(primitive_edge_hit(ray, distance, normal, edge));
            }
        }
    }
    for bond in &snapshot.bonds {
        let main_hit = if bond.screen_point {
            ray_sphere(ray, bond.from, f64::from(bond.radius))
        } else {
            ray_cylinder(ray, bond.from, bond.to, f64::from(bond.radius))
        };
        if let Some((distance, normal)) = main_hit {
            hits.push(Hit {
                distance,
                position: ray.origin + ray.direction * distance,
                normal,
                color: color_from_spec(bond.color.as_ref()),
                unlit: false,
            });
        } else if let Some(edge) = bond
            .edge
            .filter(|edge| edge.width > 0.0 && bond.screen_width.is_none())
        {
            let radius = f64::from(bond.radius + edge.width);
            let edge_hit = if bond.screen_point {
                ray_sphere(ray, bond.from, radius)
            } else {
                ray_cylinder(ray, bond.from, bond.to, radius)
            };
            if let Some((distance, normal)) = edge_hit {
                hits.push(primitive_edge_hit(ray, distance, normal, edge));
            }
        }
    }
    for mesh in &snapshot.meshes {
        for triangle in mesh.indices.chunks_exact(3) {
            let Some(v0) = mesh.vertices.get(triangle[0] as usize) else {
                continue;
            };
            let Some(v1) = mesh.vertices.get(triangle[1] as usize) else {
                continue;
            };
            let Some(v2) = mesh.vertices.get(triangle[2] as usize) else {
                continue;
            };
            if let Some((distance, u, v)) = ray_triangle(
                ray,
                [v0.position, v1.position, v2.position],
                v0.culls_back_faces(),
            ) {
                let w = 1.0 - u - v;
                let normal = normalize(v0.normal * w + v1.normal * u + v2.normal * v);
                let mut color = [0.0; 4];
                for (channel, value) in color.iter_mut().enumerate() {
                    *value = f64::from(v0.color[channel]) * w
                        + f64::from(v1.color[channel]) * u
                        + f64::from(v2.color[channel]) * v;
                }
                hits.push(Hit {
                    distance,
                    position: ray.origin + ray.direction * distance,
                    normal: if normal.dot3d(ray.direction) > 0.0 {
                        normal * -1.0
                    } else {
                        normal
                    },
                    color,
                    unlit: v0.is_edge_outline(),
                });
            }
        }
    }
    hits
}

fn primitive_edge_hit(
    ray: Ray,
    distance: f64,
    normal: Vector4,
    edge: crate::RenderEdgeStyle,
) -> Hit {
    let position = ray.origin + ray.direction * distance;
    Hit {
        distance: if edge.mode == crate::RenderEdgeMode::Silhouette {
            f64::MAX
        } else {
            distance
        },
        position,
        normal,
        color: edge.color.map(f64::from),
        unlit: true,
    }
}

fn ray_sphere(ray: Ray, center: Vector4, radius: f64) -> Option<(f64, Vector4)> {
    let oc = ray.origin - center;
    let half_b = oc.dot3d(ray.direction);
    let c = oc.dot3d(oc) - radius * radius;
    let discriminant = half_b * half_b - c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let distance = [-half_b - root, -half_b + root]
        .into_iter()
        .find(|distance| *distance > 1.0e-6)?;
    let normal = normalize(ray.origin + ray.direction * distance - center);
    Some((distance, normal))
}

fn ray_cylinder(ray: Ray, start: Vector4, end: Vector4, radius: f64) -> Option<(f64, Vector4)> {
    let axis_vector = end - start;
    let length = axis_vector.length3d();
    if length <= 1.0e-9 {
        return ray_sphere(ray, start, radius);
    }
    let axis = axis_vector * length.recip();
    let oc = ray.origin - start;
    let direction_perp = ray.direction - axis * ray.direction.dot3d(axis);
    let origin_perp = oc - axis * oc.dot3d(axis);
    let a = direction_perp.dot3d(direction_perp);
    let b = 2.0 * direction_perp.dot3d(origin_perp);
    let c = origin_perp.dot3d(origin_perp) - radius * radius;
    let mut candidates = Vec::new();
    let discriminant = b * b - 4.0 * a * c;
    if a > 1.0e-12 && discriminant >= 0.0 {
        let root = discriminant.sqrt();
        for distance in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
            let along = oc.dot3d(axis) + distance * ray.direction.dot3d(axis);
            if distance > 1.0e-6 && (0.0..=length).contains(&along) {
                let point = ray.origin + ray.direction * distance;
                let closest = start + axis * along;
                candidates.push((distance, normalize(point - closest)));
            }
        }
    }
    let denominator = ray.direction.dot3d(axis);
    if denominator.abs() > 1.0e-12 {
        for (center, normal) in [(start, axis * -1.0), (end, axis)] {
            let distance = (center - ray.origin).dot3d(axis) / denominator;
            let point = ray.origin + ray.direction * distance;
            if distance > 1.0e-6 && (point - center).length3d() <= radius {
                candidates.push((distance, normal));
            }
        }
    }
    candidates
        .into_iter()
        .min_by(|left, right| left.0.total_cmp(&right.0))
}

fn ray_triangle(
    ray: Ray,
    triangle: [Vector4; 3],
    cull_back_faces: bool,
) -> Option<(f64, f64, f64)> {
    let edge1 = triangle[1] - triangle[0];
    let edge2 = triangle[2] - triangle[0];
    let p = cross(ray.direction, edge2);
    let determinant = edge1.dot3d(p);
    if (cull_back_faces && determinant <= 1.0e-12)
        || (!cull_back_faces && determinant.abs() < 1.0e-12)
    {
        return None;
    }
    let inverse = determinant.recip();
    let translated = ray.origin - triangle[0];
    let u = translated.dot3d(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(translated, edge1);
    let v = ray.direction.dot3d(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge2.dot3d(q) * inverse;
    (distance > 1.0e-6).then_some((distance, u, v))
}

fn color_from_spec(color: Option<&ColorSpec>) -> [f64; 4] {
    let rgba = color.and_then(ColorSpec::rgba8);
    rgba.map_or([0.75, 0.75, 0.75, 1.0], |rgba| {
        [
            f64::from(rgba.r) / 255.0,
            f64::from(rgba.g) / 255.0,
            f64::from(rgba.b) / 255.0,
            f64::from(rgba.a) / 255.0,
        ]
    })
}

fn normalize(value: Vector4) -> Vector4 {
    value.normalize3d().unwrap_or(Vector4::new3(0.0, 0.0, 1.0))
}

fn transform_direction(matrix: cuemol_core::Matrix4, value: Vector4) -> Vector4 {
    let result = matrix.transform_point(Vector4::new4(value.x, value.y, value.z, 0.0));
    Vector4::new3(result.x, result.y, result.z)
}

fn cross(left: Vector4, right: Vector4) -> Vector4 {
    Vector4::new3(
        left.y * right.z - left.z * right.y,
        left.z * right.x - left.x * right.z,
        left.x * right.y - left.y * right.x,
    )
}

fn encode_umbreon_png(frame: &UmbreonFrame) -> Result<Vec<u8>> {
    let width = u32::try_from(frame.width)
        .map_err(|_| CueMolError::invalid_argument("invalid Umbreon frame width"))?;
    let height = u32::try_from(frame.height)
        .map_err(|_| CueMolError::invalid_argument("invalid Umbreon frame height"))?;
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(match frame.components {
            3 => png::ColorType::Rgb,
            4 => png::ColorType::Rgba,
            _ => {
                return Err(CueMolError::invalid_argument(
                    "Umbreon frame must have RGB or RGBA pixels",
                ));
            }
        });
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder
            .write_header()
            .map_err(|error| CueMolError::Io(error.to_string()))?;
        writer
            .write_image_data(&frame.pixels)
            .map_err(|error| CueMolError::Io(error.to_string()))?;
    }
    Ok(output)
}

pub fn render_umbreon_smoke() -> UmbreonSmokeResult {
    let mut snapshot = RenderSceneSnapshot::empty();
    snapshot.meshes.push(crate::RenderMesh {
        vertices: [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .map(|(x, y)| crate::MeshVertex {
                position: Vector4::new3(x, y, 0.0),
                normal: Vector4::new3(0.0, 0.0, 1.0),
                color: [1.0, 1.0, 1.0, 1.0],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    snapshot.recompute_bounds();
    let mut camera = Camera::new();
    camera.set_zoom(2.5);
    camera.set_cam_dist(5.0);
    camera.set_slab_depth(20.0);
    let params = UmbreonRenderParams {
        width: 64,
        height: 64,
        perspective: false,
        supersample: 1,
        show_edge_lines: false,
        ..UmbreonRenderParams::default()
    };
    let started = Instant::now();
    let frame = render_umbreon_snapshot(&snapshot, &camera, [0.0, 0.0, 0.0, 1.0], &params)
        .expect("Umbreon smoke render");
    let non_background_pixels = frame
        .pixels
        .chunks_exact(frame.components as usize)
        .filter(|pixel| pixel[..3].iter().any(|value| *value > 8))
        .count();
    UmbreonSmokeResult {
        width: frame.width,
        height: frame.height,
        render_seconds: started.elapsed().as_secs_f64(),
        non_background_pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeshVertex, RenderAtom, RenderMesh};
    use cuemol_gfx::{ColorBase, ColorSpec};

    fn color(r: u8, g: u8, b: u8, a: u8) -> Option<ColorSpec> {
        Some(ColorSpec {
            base: ColorBase::Html { r, g, b, a },
            modifiers: Vec::new(),
        })
    }

    fn test_camera() -> Camera {
        let mut camera = Camera::new();
        camera.set_zoom(6.0);
        camera.set_cam_dist(10.0);
        camera.set_slab_depth(40.0);
        camera
    }

    #[test]
    fn umbreon_smoke_renders_a_non_empty_frame() {
        let result = render_umbreon_smoke();
        assert_eq!(result.width, 64);
        assert_eq!(result.height, 64);
        assert!(result.render_seconds >= 0.0);
        assert!(result.non_background_pixels > 1_000);
    }

    #[test]
    fn snapshot_raytracer_renders_spheres_bonds_and_meshes() {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.atoms.push(RenderAtom::new(
            Vector4::new3(-1.0, 0.5, 0.0),
            0.7,
            color(40, 120, 255, 255),
        ));
        snapshot.bonds.push(crate::RenderBond::new(
            Vector4::new3(0.0, -1.0, 0.0),
            Vector4::new3(1.0, 1.0, 0.0),
            0.25,
            color(40, 255, 80, 255),
        ));
        snapshot.meshes.push(RenderMesh {
            vertices: [(-2.0, -2.0), (2.0, -2.0), (0.0, 2.0)]
                .into_iter()
                .map(|(x, y)| MeshVertex {
                    position: Vector4::new3(x, y, -1.0),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: [1.0, 0.2, 0.2, 1.0],
                })
                .collect(),
            indices: vec![0, 1, 2],
        });
        let params = UmbreonRenderParams {
            width: 64,
            height: 64,
            perspective: false,
            supersample: 1,
            show_edge_lines: false,
            ..UmbreonRenderParams::default()
        };
        let frame =
            render_umbreon_snapshot(&snapshot, &test_camera(), [0.0, 0.0, 0.0, 1.0], &params)
                .unwrap();
        assert_eq!(frame.components, 3);
        assert_eq!(frame.pixels.len(), 64 * 64 * 3);
        assert!(
            frame
                .pixels
                .chunks_exact(3)
                .filter(|pixel| pixel.iter().any(|value| *value > 8))
                .count()
                > 500
        );
    }

    #[test]
    fn raytracer_honors_mesh_back_face_culling() {
        let ray = Ray {
            origin: Vector4::new3(0.0, 0.0, 1.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };
        let front = [
            Vector4::new3(-1.0, -1.0, 0.0),
            Vector4::new3(1.0, -1.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
        ];
        let back = [front[0], front[2], front[1]];

        assert!(ray_triangle(ray, front, true).is_some());
        assert!(ray_triangle(ray, back, true).is_none());
        assert!(ray_triangle(ray, back, false).is_some());
    }

    #[test]
    fn raytracer_treats_edge_outline_mesh_as_unlit() {
        let color = [0.75, 0.25, 0.1, 1.0];
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.meshes.push(RenderMesh {
            vertices: [(-1.0, -1.0), (1.0, -1.0), (0.0, 1.0)]
                .into_iter()
                .map(|(x, y)| {
                    MeshVertex {
                        position: Vector4::new3(x, y, 0.0),
                        normal: Vector4::new3(1.0, 0.0, 0.0),
                        color,
                    }
                    .with_edge_outline(false)
                })
                .collect(),
            indices: vec![0, 1, 2],
        });
        let ray = Ray {
            origin: Vector4::new3(0.0, 0.0, 1.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };
        let hits = scene_hits(&snapshot, ray);

        assert_eq!(hits.len(), 1);
        assert!(hits[0].unlit);
        assert_eq!(
            shade_hit(
                &snapshot,
                hits[0],
                ray,
                &test_camera(),
                &UmbreonRenderParams::default(),
            ),
            [
                f64::from(color[0]),
                f64::from(color[1]),
                f64::from(color[2]),
            ]
        );
    }

    #[test]
    fn raytracer_draws_primitive_edges_only_outside_the_base_silhouette() {
        let ray = Ray {
            origin: Vector4::new3(1.05, 0.0, 5.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };
        let edge = crate::RenderEdgeStyle {
            mode: crate::RenderEdgeMode::Edges,
            width: 0.1,
            color: [0.1, 0.2, 0.3, 1.0],
        };
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.atoms.push(
            crate::RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 1.0, None).with_edge(Some(edge)),
        );

        let hits = scene_hits(&snapshot, ray);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].unlit);
        assert_eq!(hits[0].color, edge.color.map(f64::from));
        assert!(hits[0].distance.is_finite());

        snapshot.atoms[0].edge.as_mut().unwrap().mode = crate::RenderEdgeMode::Silhouette;
        let silhouette_hits = scene_hits(&snapshot, ray);
        assert_eq!(silhouette_hits.len(), 1);
        assert_eq!(silhouette_hits[0].distance, f64::MAX);

        let center_ray = Ray {
            origin: Vector4::new3(0.0, 0.0, 5.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };
        let center_hits = scene_hits(&snapshot, center_ray);
        assert_eq!(center_hits.len(), 1);
        assert!(!center_hits[0].unlit);
    }

    #[test]
    fn raytracer_treats_screen_points_as_spheres() {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.bonds.push(
            crate::RenderBond::new(
                Vector4::new3(0.0, 0.0, 0.0),
                Vector4::new3(0.0, 0.0, 0.0),
                0.5,
                color(255, 255, 255, 255),
            )
            .with_screen_style(2.0, false)
            .as_screen_point(),
        );
        let ray = Ray {
            origin: Vector4::new3(0.0, 0.0, 2.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };

        assert_eq!(scene_hits(&snapshot, ray).len(), 1);
    }

    #[test]
    fn transparent_background_and_translucent_front_are_composited() {
        let triangle = |z: f64, rgba: [f32; 4]| RenderMesh {
            vertices: [(-2.0, -2.0), (2.0, -2.0), (0.0, 2.0)]
                .into_iter()
                .map(|(x, y)| MeshVertex {
                    position: Vector4::new3(x, y, z),
                    normal: Vector4::new3(0.0, 0.0, 1.0),
                    color: rgba,
                })
                .collect(),
            indices: vec![0, 1, 2],
        };
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.meshes.push(triangle(0.0, [1.0, 0.0, 0.0, 1.0]));
        snapshot.meshes.push(triangle(1.0, [0.0, 0.0, 1.0, 0.5]));
        let params = UmbreonRenderParams {
            width: 32,
            height: 32,
            perspective: false,
            supersample: 1,
            show_edge_lines: false,
            transparent_background: true,
            ..UmbreonRenderParams::default()
        };
        let frame =
            render_umbreon_snapshot(&snapshot, &test_camera(), [0.0, 0.0, 0.0, 1.0], &params)
                .unwrap();
        assert_eq!(frame.components, 4);
        let center = (16 * 32 + 16) * 4;
        assert!(frame.pixels[center] > 20);
        assert!(frame.pixels[center + 2] > 20);
        assert_eq!(frame.pixels[center + 3], 255);
        assert_eq!(frame.pixels[3], 0);
    }

    #[test]
    fn sampling_and_denoising_parameters_change_the_cpu_pipeline() {
        let normal = Vector4::new3(0.0, 0.0, 1.0);
        for sample in 0..16 {
            let direction = hemisphere_sample(normal, sample, 16);
            assert!(direction.dot3d(normal) > 0.0);
            assert!((direction.length3d() - 1.0).abs() < 1.0e-9);
        }
        assert_ne!(
            cone_sample(normal, 0.2, 0, 8),
            cone_sample(normal, 0.2, 1, 8)
        );

        let mut pixels = vec![0_u8; 5 * 5 * 3];
        let center = (2 * 5 + 2) * 3;
        pixels[center..center + 3].copy_from_slice(&[48, 48, 48]);
        denoise_pixels(&mut pixels, 5, 5, 3, 1);
        assert!(pixels[center] < 48);
        assert!(pixels[center - 3] > 0);
    }

    #[test]
    fn tritium_denoise_modes_compose_the_two_exporter_knobs() {
        let expected = [
            (UmbreonDenoiseMode::Oidn, "OIDN", true, 0),
            (UmbreonDenoiseMode::Atrous, "A-trous", false, 1),
            (UmbreonDenoiseMode::None, "None", false, 0),
        ];
        let mut exporter = UmbreonSceneExporter::new();
        for (mode, label, gi_denoise, denoiser) in expected {
            assert_eq!(mode.label(), label);
            assert_eq!(UmbreonDenoiseMode::from_label_or_default(label), mode);
            exporter.set_denoise_mode(mode);
            assert_eq!(exporter.gi_denoise(), gi_denoise);
            assert_eq!(exporter.denoiser(), denoiser);
        }
        assert_eq!(
            UmbreonDenoiseMode::from_label_or_default("unknown"),
            UmbreonDenoiseMode::Oidn
        );
    }

    #[test]
    fn exporter_defaults_sync_write_and_async_cancel_match_qif() {
        let mut exporter = UmbreonSceneExporter::new();
        assert_eq!(exporter.handler().name(), "umbreon");
        assert!(exporter.perspective());
        assert!(exporter.use_clip_z());
        assert_eq!(exporter.supersample(), 3);
        assert_eq!(exporter.ao_samples(), 0);
        assert!(!exporter.shadows());
        assert!(exporter.show_edge_lines());
        assert!(!exporter.transparent_background());
        assert!(!exporter.use_gi());
        assert_eq!(exporter.gi_samples(), 32);
        assert!(exporter.gi_denoise());
        assert_eq!(exporter.get_render_phase_name(), "Idle");
        assert!(exporter.is_render_done());

        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot
            .atoms
            .push(RenderAtom::new(Vector4::ZERO, 1.0, color(255, 0, 0, 255)));
        exporter.set_width(24);
        exporter.set_height(24);
        exporter.set_supersample(1);
        exporter.set_show_edge_lines(false);
        let path =
            std::env::temp_dir().join(format!("cuemol-umbreon-{}-sync.png", std::process::id()));
        exporter.handler_mut().set_path(path.to_string_lossy());
        let frame = exporter
            .write_snapshot(&snapshot, &test_camera(), [0.0, 0.0, 0.0, 1.0])
            .unwrap();
        assert_eq!(frame.pixels.len(), 24 * 24 * 3);
        assert_eq!(exporter.write_count(), 1);
        assert!(fs::read(&path).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
        let _ = fs::remove_file(path);

        exporter.set_width(128);
        exporter.set_height(128);
        exporter.set_supersample(3);
        exporter.set_render_input(snapshot, test_camera(), [0.0, 0.0, 0.0, 1.0]);
        exporter.begin_render().unwrap();
        exporter.cancel_render();
        assert!(exporter.end_render().unwrap().is_none());
        assert!(exporter.was_render_cancelled());
    }
}
