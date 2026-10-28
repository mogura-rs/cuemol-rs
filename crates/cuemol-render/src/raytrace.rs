mod bvh;
mod lighting;
#[cfg(test)]
mod lighting_tests;
mod npr;
use bvh::RayIndex;
use cuemol_core::clock::Instant;
pub use npr::{
    hatch_style_spec, render_umbreon_npr_snapshot, validate_hatch_params, HatchLayer,
    HatchLayerKind, HatchLook, HatchRenderParams, HATCH_STYLES,
};
use std::fs;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};

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

/// Native image filters. Legacy OIDN preferences map to the bilateral filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UmbreonDenoiseMode {
    #[default]
    Bilateral,
    Atrous,
    None,
}

impl UmbreonDenoiseMode {
    pub const ALL: [Self; 3] = [Self::Bilateral, Self::Atrous, Self::None];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Bilateral => "Bilateral",
            Self::Atrous => "A-trous",
            Self::None => "None",
        }
    }

    pub const fn exporter_settings(self) -> (bool, i32) {
        match self {
            Self::Bilateral => (true, 0),
            Self::Atrous => (false, 1),
            Self::None => (false, 0),
        }
    }

    /// Import saved preferences, including the legacy OIDN label.
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
    pub ao_diffuse_factor: f64,
    pub ao_multi_scale: bool,
    pub ao_bent_normal: bool,
    pub ao_low_discrepancy: bool,
    pub ao_falloff_power: f64,
    pub ao_per_pixel: bool,
    pub shadows: bool,
    pub shadow_samples: i32,
    pub light_radius: f64,
    pub light_intensity: f64,
    pub flash_fraction: f64,
    pub ambient_fraction: f64,
    pub show_edge_lines: bool,
    pub crease_limit: f64,
    pub edge_rise: f64,
    pub transparent_background: bool,
    pub use_gi: bool,
    pub gi_samples: i32,
    pub gi_intensity: f64,
    pub gi_env_intensity: f64,
    pub gi_sky_gradient: bool,
    pub gi_ground_color: [f64; 3],
    pub gi_bounces: i32,
    pub per_pixel_blend: bool,
    pub contact_edges: bool,
    pub outline_far_depth: f64,
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
            ao_diffuse_factor: 1.,
            ao_multi_scale: true,
            ao_bent_normal: true,
            ao_low_discrepancy: true,
            ao_falloff_power: 0.,
            ao_per_pixel: true,
            shadows: false,
            shadow_samples: 1,
            light_radius: 0.0,
            light_intensity: -1.,
            flash_fraction: -1.,
            ambient_fraction: -1.,
            show_edge_lines: true,
            crease_limit: -1.0,
            edge_rise: 0.5,
            transparent_background: false,
            use_gi: false,
            gi_samples: 32,
            gi_intensity: 1.0,
            gi_env_intensity: 1.0,
            gi_sky_gradient: true,
            gi_ground_color: [0.4; 3],
            gi_bounces: 4,
            per_pixel_blend: true,
            contact_edges: true,
            outline_far_depth: 0.2,
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
    render_with_hatch(snapshot, camera, background, params, async_state, None)
}

fn render_with_hatch(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    async_state: Option<(&AtomicU32, &AtomicBool)>,
    hatch: Option<&npr::ResolvedHatch>,
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
    let length = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(components))
        .ok_or_else(|| CueMolError::invalid_argument("render dimensions overflow"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|e| CueMolError::invalid_argument(format!("cannot allocate render: {e}")))?;
    pixels.resize(length, 0u8);
    let started = Instant::now();
    let aspect = width as f64 / height as f64;
    let half_h = camera.zoom() * 0.5;
    let half_w = half_h * aspect;
    let inverse_rotation = camera.rotation().inv()?.to_matrix();
    let camera_center = camera.center();
    let index = RayIndex::new(snapshot);

    let render_row = |y: usize, row: &mut [u8]| {
        for x in 0..width {
            let cache = lighting::PixelCache::new(camera.zoom() / height as f64);
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
                    let ray = Ray { origin, direction };
                    let sample = if let Some(hatch) = hatch {
                        npr::trace_npr(
                            snapshot,
                            &index,
                            ray,
                            camera,
                            background,
                            params,
                            hatch,
                            (x * samples + sx) as f32 + 0.5,
                            (y * samples + sy) as f32 + 0.5,
                            Some(&cache),
                        )
                    } else {
                        trace_sample(
                            snapshot,
                            &index,
                            ray,
                            camera,
                            background,
                            params,
                            Some(&cache),
                        )
                    };
                    for index in 0..4 {
                        accumulated[index] += sample[index];
                    }
                }
            }
            let divisor = (samples * samples) as f64;
            let offset = x * components;
            for index in 0..components {
                let denominator = if components == 4 && index < 3 {
                    accumulated[3].max(1e-12)
                } else {
                    divisor
                };
                row[offset + index] =
                    ((accumulated[index] / denominator).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    };
    let completed = std::sync::atomic::AtomicUsize::new(0);
    let row_bytes = width * components;
    let render_rows = |start: usize, rows: &mut [u8]| {
        for (i, row) in rows.chunks_mut(row_bytes).enumerate() {
            if async_state.is_some_and(|(_, cancel)| cancel.load(Ordering::Relaxed)) {
                break;
            }
            render_row(start + i, row);
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some((progress, _)) = async_state {
                progress.fetch_max(
                    ((done as f64 / height as f64) * 1_000_000.).round() as u32,
                    Ordering::Relaxed,
                );
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
                .min(height)
        };
        if workers == 1 {
            render_rows(0, &mut pixels);
        } else {
            let rows = height.div_ceil(workers);
            std::thread::scope(|scope| {
                for (i, chunk) in pixels.chunks_mut(rows * row_bytes).enumerate() {
                    let render = &render_rows;
                    scope.spawn(move || render(i * rows, chunk));
                }
            });
        }
    }
    #[cfg(target_arch = "wasm32")]
    render_rows(0, &mut pixels);
    if async_state.is_some_and(|(_, cancel)| cancel.load(Ordering::Relaxed)) {
        return Ok(UmbreonFrame {
            width: params.width,
            height: params.height,
            components: components as i32,
            pixels: Vec::new(),
            render_seconds: started.elapsed().as_secs_f64(),
        });
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
    order: usize,
    distance: f64,
    position: Vector4,
    normal: Vector4,
    color: [f64; 4],
    unlit: bool,
    transparency_group: Option<usize>,
}

#[derive(Clone, Copy)]
enum MaterialPreset {
    Physical {
        ambient: f64,
        diffuse: f64,
        metallic: f64,
        roughness: f64,
        specular: f64,
    },
    Toon {
        ambient: f64,
        diffuse: f64,
        phong: f64,
        phong_size: f64,
    },
}

fn named_material(name: &str) -> MaterialPreset {
    match name {
        "matte" => MaterialPreset::Physical {
            ambient: 0.30,
            diffuse: 0.80,
            metallic: 0.0,
            roughness: 0.50,
            specular: 0.0,
        },
        "diff_metal" => MaterialPreset::Physical {
            ambient: 0.35,
            diffuse: 0.30,
            metallic: 1.0,
            roughness: 0.549_100_5,
            specular: 0.80,
        },
        "spec_metal" => MaterialPreset::Physical {
            ambient: 0.15,
            diffuse: 0.60,
            metallic: 1.0,
            roughness: 0.374_203_2,
            specular: 0.80,
        },
        "metallic_chrome" => MaterialPreset::Physical {
            ambient: 0.20,
            diffuse: 0.80,
            metallic: 1.0,
            roughness: 0.05,
            specular: 0.50,
        },
        "metallic_copper" => MaterialPreset::Physical {
            ambient: 0.20,
            diffuse: 0.80,
            metallic: 1.0,
            roughness: 0.15,
            specular: 0.50,
        },
        "stone35" => MaterialPreset::Physical {
            ambient: 0.20,
            diffuse: 0.80,
            metallic: 0.0,
            roughness: 0.85,
            specular: 0.25,
        },
        "wood31" | "wood14scl2" => MaterialPreset::Physical {
            ambient: 0.20,
            diffuse: 0.80,
            metallic: 0.0,
            roughness: 0.45,
            specular: 0.50,
        },
        "shadow" => MaterialPreset::Toon {
            ambient: 0.75,
            diffuse: 0.0,
            phong: 0.0,
            phong_size: 40.0,
        },
        "nolighting" => MaterialPreset::Toon {
            ambient: 1.0,
            diffuse: 0.0,
            phong: 0.0,
            phong_size: 40.0,
        },
        "toon1" => MaterialPreset::Toon {
            ambient: 0.0,
            diffuse: 0.8,
            phong: 0.0,
            phong_size: 40.0,
        },
        "toon2" => MaterialPreset::Toon {
            ambient: 0.30,
            diffuse: 0.50,
            phong: 10_000.0,
            phong_size: 50.0,
        },
        _ => MaterialPreset::Physical {
            ambient: 0.20,
            diffuse: 0.80,
            metallic: 0.0,
            roughness: 0.374_203_2,
            specular: 0.40,
        },
    }
}

fn trace_sample(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    cache: Option<&std::cell::RefCell<lighting::PixelCache>>,
) -> [f64; 4] {
    blend_sample(index, params, |group| {
        trace_sample_pass(
            snapshot, index, ray, camera, background, params, group, cache,
        )
    })
}

fn blend_sample(
    index: &RayIndex<'_>,
    params: &UmbreonRenderParams,
    mut pass: impl FnMut(Option<usize>) -> ([f64; 4], f64),
) -> [f64; 4] {
    let (bg, depth) = pass(None);
    if index.translucent_groups.is_empty() {
        return bg;
    }
    let mut weighted = [0.; 4];
    let mut alpha_sum = 0.;
    let mut transmittance = 1.;
    for &(group, alpha) in &index.translucent_groups {
        if alpha <= 0. {
            continue;
        }
        let (layer, layer_depth) = pass(Some(group));
        if params.per_pixel_blend && !(layer_depth < depth * (1. - 1e-5)) {
            continue;
        }
        alpha_sum += alpha;
        transmittance *= 1. - alpha;
        for c in 0..4 {
            weighted[c] += layer[c] * alpha;
        }
    }
    if alpha_sum <= 0. {
        return bg;
    }
    if params.per_pixel_blend {
        std::array::from_fn(|c| {
            bg[c] * transmittance + weighted[c] * (1. - transmittance) / alpha_sum
        })
    } else {
        std::array::from_fn(|c| bg[c] * (1. - alpha_sum) + weighted[c])
    }
}

fn visible_hits(
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    params: &UmbreonRenderParams,
    group: Option<usize>,
) -> Vec<Hit> {
    let mut hits = index.hits(ray, group);
    if params.use_clip_z {
        let rot = camera.rotation().to_matrix();
        let half = camera.slab_depth() * 0.5;
        hits.retain(|hit| {
            (-half..=half).contains(&transform_direction(rot, hit.position - camera.center()).z)
        });
    }
    if !params.show_edge_lines {
        hits.retain(|h| !h.unlit);
    }
    let front = hits
        .iter()
        .filter(|h| !h.unlit)
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
        .copied();
    for h in &mut hits {
        if h.unlit && h.distance == f64::MAX {
            let own_distance = (h.position - ray.origin).length3d();
            if front.is_some_and(|f| {
                f.transparency_group != h.transparency_group
                    || 1. - fog_at(f, camera) > params.outline_far_depth
            }) {
                h.distance = own_distance;
            }
        }
    }
    hits.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then(a.order.cmp(&b.order))
    });
    hits
}
fn section_alpha(snapshot: &RenderSceneSnapshot, hit: Hit) -> f64 {
    let alpha = hit
        .transparency_group
        .map(|i| snapshot.transparency_groups[i].alpha as f64)
        .unwrap_or(1.);
    if alpha > 0. {
        (hit.color[3] / alpha).clamp(0., 1.)
    } else {
        0.
    }
}
fn fog_at(hit: Hit, camera: &Camera) -> f64 {
    if camera.slab_depth() <= 0. {
        return 1.;
    }
    let eye = transform_direction(
        camera.rotation().to_matrix(),
        hit.position - camera.center(),
    );
    (1. + eye.z / (0.5 * camera.slab_depth())).clamp(0., 1.)
}
fn trace_sample_pass(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    ray: Ray,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    visible_group: Option<usize>,
    cache: Option<&std::cell::RefCell<lighting::PixelCache>>,
) -> ([f64; 4], f64) {
    let hits = visible_hits(index, ray, camera, params, visible_group);
    let depth = hits.first().map(|h| h.distance).unwrap_or(f64::INFINITY);
    let mut out = [0.; 4];
    for &hit in &hits {
        let mut shaded = lighting::shade(
            snapshot,
            index,
            hit,
            ray,
            camera,
            params,
            visible_group,
            cache,
        );
        if !hit.unlit && edge_at(index, hit, ray, camera, params, visible_group, false) {
            shaded = [0.; 3];
        }
        let fog = fog_at(hit, camera);
        let alpha = section_alpha(snapshot, hit)
            * if params.transparent_background {
                fog
            } else {
                1.
            };
        let weight = alpha * (1. - out[3]);
        for c in 0..3 {
            let color = if params.transparent_background {
                shaded[c]
            } else {
                background[c] as f64 * (1. - fog) + shaded[c] * fog
            };
            out[c] += color * weight;
        }
        out[3] += weight;
        if out[3] >= 1. - 1e-6 {
            break;
        }
    }
    if !params.transparent_background {
        for c in 0..3 {
            out[c] += background[c] as f64 * (1. - out[3]);
        }
        out[3] = 1.;
    }
    (out, depth)
}

fn edge_at(
    index: &RayIndex<'_>,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    p: &UmbreonRenderParams,
    group: Option<usize>,
    default_edges: bool,
) -> bool {
    if !p.show_edge_lines || (!default_edges && p.crease_limit <= 0. && !p.contact_edges) {
        return false;
    }
    let rotation = camera.rotation().to_matrix();
    let inverse = camera
        .rotation()
        .inv()
        .unwrap_or(cuemol_core::Quat::IDENTITY)
        .to_matrix();
    let step = camera.zoom() / p.height.max(1) as f64 * (0.5 + p.edge_rise.clamp(0., 2.));
    let half = camera.slab_depth() * 0.5;
    for (x, y) in [(step, 0.), (-step, 0.), (0., step), (0., -step)] {
        let offset = transform_direction(inverse, Vector4::new3(x, y, 0.));
        let r = if p.perspective {
            Ray {
                origin: ray.origin,
                direction: normalize(ray.direction + offset / camera.cam_dist().max(1e-6)),
            }
        } else {
            Ray {
                origin: ray.origin + offset,
                direction: ray.direction,
            }
        };
        let adjacent = index.nearest_where(r, f64::INFINITY, group, |h| {
            !h.unlit
                && (!p.use_clip_z
                    || (-half..=half)
                        .contains(&transform_direction(rotation, h.position - camera.center()).z))
        });
        let Some(other) = adjacent else {
            if default_edges {
                return true;
            } else {
                continue;
            }
        };
        let depth_jump = (other.distance - hit.distance).abs();
        if depth_jump > step * 4. {
            if default_edges && other.distance > hit.distance {
                return true;
            } else {
                continue;
            }
        }
        if p.contact_edges && other.transparency_group != hit.transparency_group {
            return true;
        }
        if p.crease_limit > 0. && hit.normal.dot3d(other.normal) < p.crease_limit.to_radians().cos()
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
fn shade_hit(
    snapshot: &RenderSceneSnapshot,
    index: &RayIndex<'_>,
    hit: Hit,
    ray: Ray,
    camera: &Camera,
    params: &UmbreonRenderParams,
    visible_group: Option<usize>,
) -> [f64; 3] {
    lighting::shade(
        snapshot,
        index,
        hit,
        ray,
        camera,
        params,
        visible_group,
        None,
    )
}

#[cfg(test)]
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
    if mode > 0 {
        denoise_atrous(pixels, width, height, components);
        return;
    }
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

// Color-guided B3-spline filtering with increasing sample spacing.
fn denoise_atrous(pixels: &mut [u8], width: usize, height: usize, components: usize) {
    let kernel = [1.0, 4.0, 6.0, 4.0, 1.0];
    let mut source = pixels.to_vec();
    for step in [1_isize, 2, 4] {
        source.copy_from_slice(pixels);
        for y in 0..height {
            for x in 0..width {
                let center = (y * width + x) * components;
                let luma = pixel_luma(&source[center..center + 3]);
                let mut sums = [0.0; 3];
                let mut total = 0.0;
                for (ky, weight_y) in kernel.iter().enumerate() {
                    for (kx, weight_x) in kernel.iter().enumerate() {
                        let nx = (x as isize + (kx as isize - 2) * step)
                            .clamp(0, width as isize - 1) as usize;
                        let ny = (y as isize + (ky as isize - 2) * step)
                            .clamp(0, height as isize - 1)
                            as usize;
                        let offset = (ny * width + nx) * components;
                        if components == 4 && source[offset + 3] != source[center + 3] {
                            continue;
                        }
                        let difference = pixel_luma(&source[offset..offset + 3]) - luma;
                        let weight = weight_x * weight_y * (-difference.abs() / 24.0).exp();
                        for channel in 0..3 {
                            sums[channel] += f64::from(source[offset + channel]) * weight;
                        }
                        total += weight;
                    }
                }
                for channel in 0..3 {
                    pixels[center + channel] =
                        (sums[channel] / total).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

fn pixel_luma(pixel: &[u8]) -> f64 {
    0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
}

#[cfg(test)]
fn scene_hits(snapshot: &RenderSceneSnapshot, ray: Ray, visible_group: Option<usize>) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (atom_index, atom) in snapshot
        .atoms
        .iter()
        .enumerate()
        .filter(|(_, atom)| atom.hit.is_none())
    {
        let transparency_group = transparency_group_for_atom(snapshot, atom_index);
        if let Some(group_index) = transparency_group {
            let group = &snapshot.transparency_groups[group_index];
            if group.alpha < 1.0 - 1.0e-4 && transparency_group != visible_group {
                continue;
            }
        }
        let main_hit = ray_sphere(ray, atom.center, f64::from(atom.radius));
        if let Some((distance, normal)) = main_hit {
            hits.push(Hit {
                order: 0,
                distance,
                position: ray.origin + ray.direction * distance,
                normal,
                color: color_from_spec(atom.color.as_ref()),
                unlit: false,
                transparency_group,
            });
        } else if let Some(edge) = atom.edge.filter(|edge| edge.width > 0.0) {
            if let Some((distance, normal)) =
                ray_sphere(ray, atom.center, f64::from(atom.radius + edge.width))
            {
                hits.push(primitive_edge_hit(
                    ray,
                    distance,
                    normal,
                    edge,
                    transparency_group,
                ));
            }
        }
    }
    for (bond_index, bond) in snapshot.bonds.iter().enumerate() {
        let transparency_group = transparency_group_for_bond(snapshot, bond_index);
        if let Some(group_index) = transparency_group {
            let group = &snapshot.transparency_groups[group_index];
            if group.alpha < 1.0 - 1.0e-4 && transparency_group != visible_group {
                continue;
            }
        }
        let main_hit = if bond.screen_point {
            ray_sphere(ray, bond.from, f64::from(bond.radius))
        } else {
            ray_cylinder(ray, bond.from, bond.to, f64::from(bond.radius))
        };
        if let Some((distance, normal)) = main_hit {
            hits.push(Hit {
                order: 0,
                distance,
                position: ray.origin + ray.direction * distance,
                normal,
                color: color_from_spec(bond.color.as_ref()),
                unlit: false,
                transparency_group,
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
                hits.push(primitive_edge_hit(
                    ray,
                    distance,
                    normal,
                    edge,
                    transparency_group,
                ));
            }
        }
    }
    for (mesh_index, mesh) in snapshot.meshes.iter().enumerate() {
        let transparency_group = transparency_group_for_mesh(snapshot, mesh_index);
        if let Some(group_index) = transparency_group {
            let group = &snapshot.transparency_groups[group_index];
            if group.alpha < 1.0 - 1.0e-4 && transparency_group != visible_group {
                continue;
            }
        }
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
                    order: 0,
                    distance,
                    position: ray.origin + ray.direction * distance,
                    normal: if normal.dot3d(ray.direction) > 0.0 {
                        normal * -1.0
                    } else {
                        normal
                    },
                    color,
                    unlit: v0.is_edge_outline(),
                    transparency_group,
                });
            }
        }
    }
    for (order, hit) in hits.iter_mut().enumerate() {
        hit.order = order;
    }
    hits
}

fn primitive_edge_hit(
    ray: Ray,
    distance: f64,
    normal: Vector4,
    edge: crate::RenderEdgeStyle,
    transparency_group: Option<usize>,
) -> Hit {
    let position = ray.origin + ray.direction * distance;
    Hit {
        order: 0,
        distance: if edge.mode == crate::RenderEdgeMode::Silhouette {
            f64::MAX
        } else {
            distance
        },
        position,
        normal,
        color: edge.color.map(f64::from),
        unlit: true,
        transparency_group,
    }
}

fn transparency_group_for_atom(snapshot: &RenderSceneSnapshot, index: usize) -> Option<usize> {
    snapshot
        .transparency_groups
        .iter()
        .position(|group| group.atom_range.contains(&index))
}

fn transparency_group_for_bond(snapshot: &RenderSceneSnapshot, index: usize) -> Option<usize> {
    snapshot
        .transparency_groups
        .iter()
        .position(|group| group.bond_range.contains(&index))
}

fn transparency_group_for_mesh(snapshot: &RenderSceneSnapshot, index: usize) -> Option<usize> {
    snapshot
        .transparency_groups
        .iter()
        .position(|group| group.mesh_range.contains(&index))
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
    let mut candidates = [None; 4];
    let mut count = 0;
    let discriminant = b * b - 4.0 * a * c;
    if a > 1.0e-12 && discriminant >= 0.0 {
        let root = discriminant.sqrt();
        for distance in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
            let along = oc.dot3d(axis) + distance * ray.direction.dot3d(axis);
            if distance > 1.0e-6 && (0.0..=length).contains(&along) {
                let point = ray.origin + ray.direction * distance;
                let closest = start + axis * along;
                candidates[count] = Some((distance, normalize(point - closest)));
                count += 1;
            }
        }
    }
    let denominator = ray.direction.dot3d(axis);
    if denominator.abs() > 1.0e-12 {
        for (center, normal) in [(start, axis * -1.0), (end, axis)] {
            let distance = (center - ray.origin).dot3d(axis) / denominator;
            let point = ray.origin + ray.direction * distance;
            if distance > 1.0e-6 && (point - center).length3d() <= radius {
                candidates[count] = Some((distance, normal));
                count += 1;
            }
        }
    }
    candidates[..count]
        .iter()
        .flatten()
        .copied()
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
    use crate::{MeshVertex, RenderAtom, RenderMesh, RenderTransparencyGroup};
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
        let hits = scene_hits(&snapshot, ray, None);

        assert_eq!(hits.len(), 1);
        assert!(hits[0].unlit);
        assert_eq!(
            shade_hit(
                &snapshot,
                &RayIndex::new(&snapshot),
                hits[0],
                ray,
                &test_camera(),
                &UmbreonRenderParams::default(),
                None,
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

        let hits = scene_hits(&snapshot, ray, None);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].unlit);
        assert_eq!(hits[0].color, edge.color.map(f64::from));
        assert!(hits[0].distance.is_finite());

        snapshot.atoms[0].edge.as_mut().unwrap().mode = crate::RenderEdgeMode::Silhouette;
        let silhouette_hits = scene_hits(&snapshot, ray, None);
        assert_eq!(silhouette_hits.len(), 1);
        assert_eq!(silhouette_hits[0].distance, f64::MAX);

        let center_ray = Ray {
            origin: Vector4::new3(0.0, 0.0, 5.0),
            direction: Vector4::new3(0.0, 0.0, -1.0),
        };
        let center_hits = scene_hits(&snapshot, center_ray, None);
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

        assert_eq!(scene_hits(&snapshot, ray, None).len(), 1);
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
    fn opaque_geometry_survives_two_nearly_opaque_translucent_sections() {
        let render_with = |translucent_sections: usize| {
            let mut snapshot = RenderSceneSnapshot::empty();
            snapshot.atoms.push(RenderAtom::new(
                Vector4::new3(0.0, 0.0, 0.0),
                1.0,
                color(204, 204, 204, 255),
            ));
            for index in 0..translucent_sections {
                let x = if index == 0 { -2.5 } else { 2.5 };
                let atom_index = snapshot.atoms.len();
                snapshot.atoms.push(RenderAtom::new(
                    Vector4::new3(x, 2.0, 0.0),
                    0.4,
                    color(51, 102, 255, 242),
                ));
                snapshot.transparency_groups.push(RenderTransparencyGroup {
                    name: format!("translucent-{index}"),
                    alpha: 0.95,
                    material: "default".to_string(),
                    atom_range: atom_index..atom_index + 1,
                    bond_range: 0..0,
                    mesh_range: 0..0,
                });
            }
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
            let center = (32 * 64 + 32) * 3;
            frame.pixels[center..center + 3].to_vec()
        };

        let baseline = render_with(0);
        assert!(baseline.iter().all(|channel| (9..250).contains(channel)));
        assert_eq!(render_with(1), baseline);
        assert_eq!(render_with(2), baseline);
    }

    #[test]
    fn translucent_renderer_is_composited_as_one_section() {
        let render_with = |sphere_count: usize, grouped: bool| {
            let mut snapshot = RenderSceneSnapshot::empty();
            for _ in 0..sphere_count {
                snapshot.atoms.push(RenderAtom::new(
                    Vector4::new3(0.0, 0.0, 0.0),
                    1.0,
                    color(204, 51, 51, 128),
                ));
            }
            if grouped {
                snapshot.transparency_groups.push(RenderTransparencyGroup {
                    name: "translucent".to_string(),
                    alpha: 0.5,
                    material: "default".to_string(),
                    atom_range: 0..sphere_count,
                    bond_range: 0..0,
                    mesh_range: 0..0,
                });
            }
            let params = UmbreonRenderParams {
                width: 32,
                height: 32,
                perspective: false,
                supersample: 1,
                show_edge_lines: false,
                ..UmbreonRenderParams::default()
            };
            let frame =
                render_umbreon_snapshot(&snapshot, &test_camera(), [0.0, 0.0, 0.0, 1.0], &params)
                    .unwrap();
            let center = (16 * 32 + 16) * 3;
            frame.pixels[center..center + 3].to_vec()
        };

        let grouped_once = render_with(1, true);
        assert_eq!(render_with(2, true), grouped_once);
        assert_ne!(render_with(2, false), grouped_once);
    }

    #[test]
    fn named_renderer_materials_change_umbreon_shading() {
        let render_with = |material: &str| {
            let mut snapshot = RenderSceneSnapshot::empty();
            snapshot.meshes.push(RenderMesh {
                vertices: [(-2.0, -2.0), (2.0, -2.0), (0.0, 2.0)]
                    .into_iter()
                    .map(|(x, y)| MeshVertex {
                        position: Vector4::new3(x, y, 0.0),
                        normal: Vector4::new3(0.0, 0.0, 1.0),
                        color: [0.5, 0.5, 0.5, 1.0],
                    })
                    .collect(),
                indices: vec![0, 1, 2],
            });
            snapshot.mark_renderer_section(1.0, material);
            let params = UmbreonRenderParams {
                width: 32,
                height: 32,
                perspective: false,
                supersample: 1,
                show_edge_lines: false,
                ..UmbreonRenderParams::default()
            };
            let frame =
                render_umbreon_snapshot(&snapshot, &test_camera(), [0.0, 0.0, 0.0, 1.0], &params)
                    .unwrap();
            let center = (16 * 32 + 16) * 3;
            frame.pixels[center..center + 3].to_vec()
        };

        let default = render_with("default");
        let chrome = render_with("metallic_chrome");
        let shadow = render_with("shadow");
        let nolighting = render_with("nolighting");
        assert!(chrome[0] < default[0]);
        assert!(nolighting[0] > shadow[0]);
        assert_eq!(render_with("unknown-material"), default);
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
    fn atrous_reduces_noise_without_blending_alpha_or_high_contrast_edges() {
        let width = 32;
        let mut pixels = Vec::new();
        for y in 0..16 {
            for x in 0..width {
                let base = if x < 16 { 30_i32 } else { 220 };
                let noise = if (x + y) % 2 == 0 { 9 } else { -9 };
                let alpha = if y < 8 { 255 } else { 0 };
                pixels.extend_from_slice(&[(base + noise) as u8; 3]);
                pixels.push(alpha);
            }
        }
        let original = pixels.clone();
        denoise_atrous(&mut pixels, width, 16, 4);
        let error = |data: &[u8]| -> i32 {
            data.chunks_exact(4)
                .enumerate()
                .map(|(i, p)| {
                    let base = if i % width < 16 { 30 } else { 220 };
                    (i32::from(p[0]) - base).abs()
                })
                .sum()
        };
        assert!(error(&pixels) < error(&original) / 2);
        for (i, (pixel, before)) in pixels
            .chunks_exact(4)
            .zip(original.chunks_exact(4))
            .enumerate()
        {
            assert_eq!(pixel[3], before[3]);
            assert!(if i % width < 16 {
                pixel[0] < 50
            } else {
                pixel[0] > 200
            });
        }
        let mut uniform = vec![123; width * 16 * 3];
        denoise_atrous(&mut uniform, width, 16, 3);
        assert!(uniform.iter().all(|v| *v == 123));
    }

    #[test]
    fn native_denoise_modes_compose_the_two_exporter_knobs() {
        let expected = [
            (UmbreonDenoiseMode::Bilateral, "Bilateral", true, 0),
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
            UmbreonDenoiseMode::Bilateral
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
