use super::*;
#[cfg(not(target_arch = "wasm32"))]
mod native;

pub(super) fn needs_oidn(p: &UmbreonRenderParams) -> bool {
    p.denoiser == 2 || (p.use_gi && p.gi_denoise && p.gi_denoiser == 2)
}

pub struct OidnRender {
    width: usize,
    height: usize,
    components: i32,
    color: Vec<f32>,
    albedo: Vec<f32>,
    normal: Vec<f32>,
    direct: Option<Vec<f32>>,
    preserve: Vec<bool>,
    render_seconds: f64,
}
impl OidnRender {
    fn finish(self, filtered: Vec<f32>) -> Result<UmbreonFrame> {
        if filtered.len() != self.color.len() || filtered.iter().any(|v| !v.is_finite()) {
            return Err(CueMolError::Io("OIDN returned invalid image data".into()));
        }
        let mut pixels = Vec::with_capacity(self.width * self.height * self.components as usize);
        for (i, value) in filtered.chunks_exact(4).enumerate() {
            for (c, &filtered) in value.iter().take(3).enumerate() {
                let v = if let Some(direct) = &self.direct {
                    direct[4 * i + c] + self.albedo[4 * i + c].max(0.01) * filtered
                } else {
                    filtered
                };
                let v = if self.preserve[i] {
                    if let Some(direct) = &self.direct {
                        direct[4 * i + c] + self.albedo[4 * i + c].max(0.01) * self.color[4 * i + c]
                    } else {
                        self.color[4 * i + c]
                    }
                } else {
                    v
                };
                pixels.push((v.clamp(0., 1.) * 255.).round() as u8);
            }
            if self.components == 4 {
                pixels.push((self.color[4 * i + 3].clamp(0., 1.) * 255.).round() as u8);
            }
        }
        Ok(UmbreonFrame {
            width: self.width as i32,
            height: self.height as i32,
            components: self.components,
            pixels,
            render_seconds: self.render_seconds,
        })
    }
    pub fn run(self) -> Result<UmbreonFrame> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut render = self;
            let start = Instant::now();
            let output = native::filter(
                render.width,
                render.height,
                &render.color,
                &render.albedo,
                &render.normal,
            )?;
            render.render_seconds += start.elapsed().as_secs_f64();
            render.finish(output)
        }
        #[cfg(target_arch = "wasm32")]
        Err(CueMolError::invalid_argument(
            "Browser OIDN requires the asynchronous render API",
        ))
    }
    #[cfg(target_arch = "wasm32")]
    pub async fn run_async(mut self) -> Result<UmbreonFrame> {
        use wasm_bindgen::{prelude::*, JsCast};
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(catch, js_namespace = cuemolOidn, js_name = run)]
            fn filter(
                width: u32,
                height: u32,
                color: js_sys::Float32Array,
                albedo: js_sys::Float32Array,
                normal: js_sys::Float32Array,
            ) -> std::result::Result<js_sys::Promise, JsValue>;
        }
        let start = Instant::now();
        let error = |e: JsValue| {
            CueMolError::Io(format!(
                "OIDN: {}",
                e.as_string().unwrap_or_else(|| format!("{e:?}"))
            ))
        };
        for n in self.normal.chunks_exact_mut(4) {
            for v in &mut n[..3] {
                *v = *v * 0.5 + 0.5;
            }
        }
        let promise = filter(
            self.width as u32,
            self.height as u32,
            js_sys::Float32Array::from(self.color.as_slice()),
            js_sys::Float32Array::from(self.albedo.as_slice()),
            js_sys::Float32Array::from(self.normal.as_slice()),
        )
        .map_err(error)?;
        let output = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(error)?;
        let output = output
            .dyn_into::<js_sys::Float32Array>()
            .map_err(error)?
            .to_vec();
        self.render_seconds += start.elapsed().as_secs_f64();
        self.finish(output)
    }
}
fn rgba(frame: &FloatFrame) -> Vec<f32> {
    frame
        .pixels
        .chunks_exact(frame.components as usize)
        .flat_map(|v| [v[0], v[1], v[2], if v.len() == 4 { v[3] } else { 1. }])
        .collect()
}

pub fn prepare_oidn_render(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    hatch: Option<&HatchRenderParams>,
) -> Result<OidnRender> {
    let resolved = hatch
        .map(|p| npr::ResolvedHatch::new(p, params.supersample))
        .transpose()?;
    let raw = render_float(
        snapshot,
        camera,
        background,
        params,
        None,
        resolved.as_ref(),
    )?;
    prepare(
        snapshot,
        camera,
        background,
        params,
        raw,
        None,
        resolved.as_ref(),
    )
}

#[cfg(target_arch = "wasm32")]
async fn yield_browser() -> Result<()> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name=setTimeout)]
        fn set_timeout(callback: &js_sys::Function, milliseconds: i32);
    }
    let promise = js_sys::Promise::new(&mut |resolve, _| set_timeout(&resolve, 0));
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| CueMolError::Io("Browser scheduling failed".into()))?;
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err(CueMolError::invalid_argument("Render cancelled"))
    } else {
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
async fn render_float_async(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    hatch: Option<&npr::ResolvedHatch>,
    cancel: Option<&AtomicBool>,
) -> Result<FloatFrame> {
    let started = Instant::now();
    let index = RayIndex::new(snapshot);
    let mut frame = FloatFrame {
        width: params.width,
        height: params.height,
        components: if params.transparent_background { 4 } else { 3 },
        pixels: Vec::new(),
        render_seconds: 0.,
    };
    let length = usize::try_from(params.width)
        .ok()
        .zip(usize::try_from(params.height).ok())
        .and_then(|(w, h)| w.checked_mul(h))
        .and_then(|n| n.checked_mul(frame.components as usize))
        .filter(|n| *n > 0)
        .ok_or_else(|| CueMolError::invalid_argument("Invalid render dimensions"))?;
    frame
        .pixels
        .try_reserve_exact(length)
        .map_err(|e| CueMolError::invalid_argument(format!("Cannot allocate render: {e}")))?;
    let mut row = 0;
    let mut rows = 4;
    while row < params.height as usize {
        yield_browser().await?;
        check_cancel(cancel)?;
        let end = (row + rows).min(params.height as usize);
        let tile = render_float_region(
            snapshot,
            camera,
            background,
            params,
            None,
            hatch,
            row..end,
            Some(&index),
        )?;
        rows = if tile.render_seconds > 0.02 {
            (rows / 2).max(1)
        } else if tile.render_seconds < 0.008 {
            (rows * 2).min(32)
        } else {
            rows
        };
        frame.pixels.extend(tile.pixels);
        row = end;
    }
    check_cancel(cancel)?;
    frame.render_seconds = started.elapsed().as_secs_f64();
    Ok(frame)
}
#[cfg(target_arch = "wasm32")]
pub async fn render_umbreon_snapshot_async(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    hatch: Option<&HatchRenderParams>,
    cancel: Option<&AtomicBool>,
) -> Result<UmbreonFrame> {
    let resolved = hatch
        .map(|p| npr::ResolvedHatch::new(p, params.supersample))
        .transpose()?;
    let (settings, background) = resolved
        .as_ref()
        .map(|r| r.render_settings(params, background))
        .unwrap_or_else(|| (params.clone(), background));
    let raw = render_float_async(
        snapshot,
        camera,
        background,
        &settings,
        resolved.as_ref(),
        cancel,
    )
    .await?;
    if needs_oidn(&settings) {
        let direct = if settings.denoiser != 2 && settings.use_gi && resolved.is_none() {
            let mut params = settings.clone();
            params.gi_intensity = 0.;
            Some(render_float_async(snapshot, camera, background, &params, None, cancel).await?)
        } else {
            None
        };
        check_cancel(cancel)?;
        let prepared = prepare_with_direct(
            snapshot,
            camera,
            background,
            &settings,
            raw,
            None,
            resolved.as_ref(),
            direct,
        )?;
        yield_browser().await?;
        check_cancel(cancel)?;
        prepared.run_async().await
    } else {
        let mut frame = raw.into_frame();
        if settings.denoiser == 1 || (settings.use_gi && settings.gi_denoise) {
            denoise_pixels(
                &mut frame.pixels,
                frame.width as usize,
                frame.height as usize,
                frame.components as usize,
                settings.denoiser,
            );
        }
        Ok(frame)
    }
}

pub(super) fn prepare(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    raw: FloatFrame,
    async_state: Option<(&AtomicU32, &AtomicBool)>,
    hatch: Option<&npr::ResolvedHatch>,
) -> Result<OidnRender> {
    prepare_with_direct(
        snapshot,
        camera,
        background,
        params,
        raw,
        async_state,
        hatch,
        None,
    )
}

fn prepare_with_direct(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    raw: FloatFrame,
    async_state: Option<(&AtomicU32, &AtomicBool)>,
    hatch: Option<&npr::ResolvedHatch>,
    direct_override: Option<FloatFrame>,
) -> Result<OidnRender> {
    let width = raw.width as usize;
    let height = raw.height as usize;
    let components = raw.components;
    let mut render_seconds = raw.render_seconds;
    let mut color = rgba(&raw);
    drop(raw);
    let mut albedo = vec![0.; color.len()];
    let mut normal = vec![0.; color.len()];
    let mut preserve = vec![true; width * height];
    let index = RayIndex::new(snapshot);
    let inverse = camera.rotation().inv()?.to_matrix();
    let rotation = camera.rotation().to_matrix();
    let half_h = camera.zoom() * 0.5;
    let half_w = half_h * width as f64 / height as f64;
    for y in 0..height {
        if async_state.is_some_and(|(_, cancel)| cancel.load(Ordering::Relaxed)) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        for x in 0..width {
            let eye_x = ((x as f64 + 0.5) / width as f64 * 2. - 1.) * half_w;
            let eye_y = (1. - (y as f64 + 0.5) / height as f64 * 2.) * half_h;
            let (origin, direction) = if params.perspective {
                (
                    Vector4::new3(0., 0., camera.cam_dist()),
                    normalize(Vector4::new3(eye_x, eye_y, -camera.cam_dist())),
                )
            } else {
                (
                    Vector4::new3(eye_x, eye_y, camera.cam_dist()),
                    Vector4::new3(0., 0., -1.),
                )
            };
            let ray = Ray {
                origin: camera.center() + transform_direction(inverse, origin),
                direction: normalize(transform_direction(inverse, direction)),
            };
            let hit = std::iter::once(None)
                .chain(index.translucent_groups.iter().map(|(g, _)| Some(*g)))
                .filter_map(|group| {
                    index.nearest_where(ray, f64::INFINITY, group, |h| {
                        h.color[3] > 0.
                            && (!params.use_clip_z
                                || transform_direction(rotation, h.position - camera.center())
                                    .z
                                    .abs()
                                    <= camera.slab_depth() * 0.5)
                    })
                })
                .min_by(|a, b| a.distance.total_cmp(&b.distance));
            let o = 4 * (y * width + x);
            if let Some(h) = hit {
                preserve[y * width + x] =
                    h.unlit || edge_at(&index, h, ray, camera, params, h.transparency_group, false);
                let n = transform_direction(rotation, h.normal);
                albedo[o..o + 3].copy_from_slice(&[
                    h.color[0] as f32,
                    h.color[1] as f32,
                    h.color[2] as f32,
                ]);
                normal[o..o + 3].copy_from_slice(&[n.x as f32, n.y as f32, n.z as f32]);
            }
            albedo[o + 3] = 1.;
            normal[o + 3] = 1.;
        }
    }
    let direct = if params.denoiser != 2 && params.use_gi && hatch.is_none() {
        let mut direct_params = params.clone();
        direct_params.gi_intensity = 0.;
        let frame = if let Some(frame) = direct_override {
            frame
        } else {
            render_float(
                snapshot,
                camera,
                background,
                &direct_params,
                async_state,
                None,
            )?
        };
        if frame.pixels.is_empty() {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        render_seconds += frame.render_seconds;
        let direct = rgba(&frame);
        for i in 0..width * height {
            for c in 0..3 {
                let o = i * 4 + c;
                color[o] = (color[o] - direct[o]).max(0.) / albedo[o].max(0.01);
            }
        }
        Some(direct)
    } else {
        None
    };
    Ok(OidnRender {
        width,
        height,
        components,
        color,
        albedo,
        normal,
        direct,
        preserve,
        render_seconds,
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn oidn_removes_noise_with_clean_guides_and_preserves_alpha() {
        let width = 64;
        let height = 48;
        let mut noisy = Vec::new();
        let mut albedo = Vec::new();
        let mut normal = Vec::new();
        let mut expected = Vec::new();
        let mut seed = 719u32;
        for _y in 0..height {
            for x in 0..width {
                let base = if x < width / 2 { 0.22 } else { 0.72 };
                for _ in 0..3 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    noisy.push(base + ((seed >> 8) as f32 / 16777216. - 0.5) * 0.36);
                    expected.push(base);
                }
                noisy.push(0.35);
                albedo.extend_from_slice(&[base, base, base, 1.]);
                normal.extend_from_slice(&[0., 0., 1., 1.]);
            }
        }
        let filtered = native::filter(width, height, &noisy, &albedo, &normal).unwrap();
        let mse = |image: &[f32]| {
            image
                .chunks_exact(4)
                .flat_map(|p| p[..3].iter())
                .zip(&expected)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                / expected.len() as f32
        };
        assert!(
            mse(&filtered) < mse(&noisy) * 0.25,
            "noisy={} filtered={}",
            mse(&noisy),
            mse(&filtered)
        );
        let frame = OidnRender {
            width,
            height,
            components: 4,
            color: noisy,
            albedo,
            normal,
            direct: None,
            preserve: vec![false; width * height],
            render_seconds: 0.,
        }
        .finish(filtered)
        .unwrap();
        assert!(frame.pixels.chunks_exact(4).all(|p| p[3] == 89));
        let left = frame.pixels[(height / 2 * width + 8) * 4];
        let right = frame.pixels[(height / 2 * width + width - 8) * 4];
        assert!((left as i32 - 56).abs() < 8 && (right as i32 - 184).abs() < 8);
    }
    #[test]
    fn indirect_recombination_keeps_direct_edges_and_unlit_annotations() {
        let input = OidnRender {
            width: 2,
            height: 1,
            components: 4,
            color: vec![0., 0., 0., 0.4, 0.3, 0.2, 0.1, 0.8],
            albedo: vec![1., 1., 1., 1., 0.5, 0.25, 0.8, 1.],
            normal: vec![0.; 8],
            direct: Some(vec![0.8, 0., 0.2, 0.4, 0.1, 0.2, 0.3, 0.8]),
            preserve: vec![true, false],
            render_seconds: 0.,
        };
        let frame = input
            .finish(vec![1., 1., 1., 1., 0.4, 0.4, 0.4, 1.])
            .unwrap();
        assert_eq!(&frame.pixels[..4], &[204, 0, 51, 102]);
        for (&actual, expected) in frame.pixels[4..].iter().zip([77u8, 77, 158, 204]) {
            assert!((actual as i16 - expected as i16).abs() <= 1);
        }
    }
}
