use cuemol_core::Result;
use cuemol_gfx::{
    AoConstants, DataTexture, DisplayContext, PostProcGpuPrim, RenderTarget, RT_COLOR_NEAREST,
    RT_COLOR_RGBA16F, RT_COLOR_RGBA8, RT_DEPTH_TEX, RT_NORMAL_RGBA16F,
};

use crate::{AAMethod, Scene};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRenderParams {
    pub camera_ao_constants: AoConstants,
    pub ao_noise_offset: f32,
    pub background_transparent: bool,
    pub enable_post_aa: bool,
    pub jitter_active: bool,
    pub jitter_index: i32,
    pub jitter_count: i32,
    pub blit_depth_to_default: bool,
}

impl Default for FrameRenderParams {
    fn default() -> Self {
        Self {
            camera_ao_constants: AoConstants::default(),
            ao_noise_offset: 0.0,
            background_transparent: false,
            enable_post_aa: true,
            jitter_active: false,
            jitter_index: 0,
            jitter_count: 1,
            blit_depth_to_default: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct FrameRenderPipeline {
    scene_target: Option<RenderTarget>,
    ao_target: Option<RenderTarget>,
    ao_denoise_target: Option<RenderTarget>,
    composite_target: Option<RenderTarget>,
    smaa_edge_target: Option<RenderTarget>,
    smaa_weight_target: Option<RenderTarget>,
    jitter_sample_target: Option<RenderTarget>,
    jitter_accum_target: Option<RenderTarget>,
    post_process: Option<PostProcGpuPrim>,
    jitter_supported: bool,
    depth_blit_count: usize,
}

impl FrameRenderPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_size(&mut self, width: usize, height: usize, half_resolution: bool) -> Result<()> {
        resize_target(
            &mut self.scene_target,
            width,
            height,
            RT_COLOR_RGBA8 | RT_DEPTH_TEX | RT_NORMAL_RGBA16F,
        )?;
        let ao_width = if half_resolution {
            width.div_ceil(2)
        } else {
            width
        };
        let ao_height = if half_resolution {
            height.div_ceil(2)
        } else {
            height
        };
        let ao_flags = RT_COLOR_RGBA8 | RT_COLOR_NEAREST;
        resize_target(&mut self.ao_target, ao_width, ao_height, ao_flags)?;
        resize_target(&mut self.ao_denoise_target, ao_width, ao_height, ao_flags)?;
        resize_target(&mut self.composite_target, width, height, RT_COLOR_RGBA8)?;
        resize_target(&mut self.smaa_edge_target, width, height, RT_COLOR_RGBA8)?;
        resize_target(&mut self.smaa_weight_target, width, height, RT_COLOR_RGBA8)?;
        resize_target(
            &mut self.jitter_sample_target,
            width,
            height,
            RT_COLOR_RGBA8,
        )?;
        resize_target(
            &mut self.jitter_accum_target,
            width,
            height,
            RT_COLOR_RGBA16F,
        )?;
        self.jitter_supported = self.jitter_accum_target.is_some();

        if self.post_process.is_none() {
            let mut post_process = PostProcGpuPrim::new();
            post_process.init();
            post_process.set_smaa_textures(
                DataTexture::new(1, 1, 2, true, vec![0, 0])?,
                DataTexture::new(1, 1, 1, false, vec![0])?,
            );
            self.post_process = Some(post_process);
        }
        Ok(())
    }

    pub fn dispose(&mut self) {
        self.post_process = None;
        self.jitter_accum_target = None;
        self.jitter_sample_target = None;
        self.smaa_weight_target = None;
        self.smaa_edge_target = None;
        self.composite_target = None;
        self.ao_denoise_target = None;
        self.ao_target = None;
        self.scene_target = None;
        self.jitter_supported = false;
    }

    pub fn is_ready(&self) -> bool {
        self.scene_target.is_some()
            && self.ao_target.is_some()
            && self.ao_denoise_target.is_some()
            && self.post_process.is_some()
    }

    pub fn jitter_supported(&self) -> bool {
        self.jitter_supported
    }

    pub fn scene_target(&self) -> Option<&RenderTarget> {
        self.scene_target.as_ref()
    }

    pub fn ao_target(&self) -> Option<&RenderTarget> {
        self.ao_target.as_ref()
    }

    pub fn post_process(&self) -> Option<&PostProcGpuPrim> {
        self.post_process.as_ref()
    }

    pub fn depth_blit_count(&self) -> usize {
        self.depth_blit_count
    }

    pub fn render<T>(
        &mut self,
        context: &DisplayContext,
        scene: &Scene<T>,
        params: &FrameRenderParams,
        mut output_target: Option<&mut RenderTarget>,
        scene_render: impl FnOnce(&mut RenderTarget),
    ) -> bool {
        if !self.is_ready() {
            return false;
        }

        let scene_target = self.scene_target.as_mut().unwrap();
        let background = scene.bg_color();
        scene_target.bind();
        scene_target.clear(
            background.fr() as f32,
            background.fg() as f32,
            background.fb() as f32,
            if params.background_transparent {
                0.0
            } else {
                1.0
            },
        );
        scene_render(scene_target);
        scene_target.unbind();

        let mut constants = params.camera_ao_constants;
        constants.effect_radius = scene.ao_radius() as f32;
        constants.final_value_power = scene.ao_intensity() as f32;
        constants.slice_count = scene.ao_slices();
        constants.steps_per_slice = scene.ao_steps();
        constants.ao_noise_offset = params.ao_noise_offset;
        let ao_target = self.ao_target.as_mut().unwrap();
        constants.ao_texel_size = [
            1.0 / ao_target.width() as f32,
            1.0 / ao_target.height() as f32,
        ];
        constants.fog_end = context.fog_end();
        constants.fog_scale = if context.fog_end() > context.fog_start() {
            1.0 / (context.fog_end() - context.fog_start())
        } else {
            0.0
        };
        let mut ao_constants = constants;
        ao_constants.viewport_pixel_size = constants.ao_texel_size;
        ao_constants.max_screenspace_radius =
            256.0 * (constants.viewport_pixel_size[0] / constants.ao_texel_size[0]);

        let post_process = self.post_process.as_mut().unwrap();
        ao_target.bind();
        ao_target.clear(1.0, 1.0, 1.0, 1.0);
        let mut succeeded = post_process.draw_gtao(scene_target, ao_constants, 0);
        ao_target.unbind();

        let denoised = self.ao_denoise_target.as_mut().unwrap();
        denoised.bind();
        succeeded &= post_process.draw_denoise(ao_target, ao_constants);
        denoised.unbind();

        let jitter = params.jitter_active && self.jitter_supported;
        if jitter {
            self.jitter_sample_target.as_mut().unwrap().bind();
        } else if let Some(target) = output_target.as_deref_mut() {
            target.bind();
        }

        if params.enable_post_aa && matches!(scene.aa_method(), AAMethod::Fxaa | AAMethod::Smaa) {
            let composite = self.composite_target.as_mut().unwrap();
            composite.bind();
            succeeded &= post_process.draw_composite(scene_target, Some(denoised), constants);
            composite.unbind();

            match scene.aa_method() {
                AAMethod::Smaa => {
                    let edges = self.smaa_edge_target.as_mut().unwrap();
                    let weights = self.smaa_weight_target.as_mut().unwrap();
                    edges.bind();
                    edges.clear(0.0, 0.0, 0.0, 0.0);
                    succeeded &= post_process.draw_smaa_edges(composite, constants);
                    edges.unbind();
                    weights.bind();
                    weights.clear(0.0, 0.0, 0.0, 0.0);
                    succeeded &= post_process.draw_smaa_weights(edges, constants);
                    weights.unbind();
                    succeeded &= post_process.draw_smaa_blend(composite, weights, constants);
                }
                AAMethod::Fxaa => {
                    succeeded &= post_process.draw_fxaa(composite, constants);
                }
                AAMethod::None => unreachable!(),
            }
        } else {
            succeeded &= post_process.draw_composite(scene_target, Some(denoised), constants);
        }

        if jitter {
            self.jitter_sample_target.as_mut().unwrap().unbind();
            let sample_count = params.jitter_count.max(1) as f32;
            let accumulation = self.jitter_accum_target.as_mut().unwrap();
            accumulation.bind();
            if params.jitter_index == 0 {
                accumulation.clear(0.0, 0.0, 0.0, 0.0);
            }
            succeeded &= post_process.draw_jitter_compose(
                self.jitter_sample_target.as_mut().unwrap(),
                sample_count.recip(),
            );
            accumulation.unbind();
            let completed = (params.jitter_index + 1).max(1) as f32;
            succeeded &= post_process.draw_jitter_compose(accumulation, sample_count / completed);
        } else if let Some(target) = output_target {
            target.unbind();
        }

        if params.blit_depth_to_default {
            self.depth_blit_count += 1;
        }
        succeeded
    }
}

fn resize_target(
    target: &mut Option<RenderTarget>,
    width: usize,
    height: usize,
    flags: u32,
) -> Result<()> {
    if let Some(target) = target {
        target.resize(width, height)
    } else {
        *target = Some(RenderTarget::new(width, height, flags)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use cuemol_core::Uid;
    use cuemol_gfx::{PostProcessPass, SolidColor};

    use super::*;

    #[test]
    fn set_size_reuses_targets_and_switches_ao_resolution() {
        let mut pipeline = FrameRenderPipeline::new();
        assert!(!pipeline.is_ready());
        pipeline.set_size(7, 5, true).unwrap();
        assert!(pipeline.is_ready());
        assert!(pipeline.jitter_supported());
        assert_eq!(pipeline.scene_target().unwrap().width(), 7);
        assert_eq!(pipeline.ao_target().unwrap().width(), 4);
        assert_eq!(pipeline.ao_target().unwrap().height(), 3);
        assert!(pipeline.ao_target().unwrap().uses_nearest_color_filter());

        pipeline.set_size(8, 6, false).unwrap();
        assert_eq!(pipeline.scene_target().unwrap().width(), 8);
        assert_eq!(pipeline.ao_target().unwrap().width(), 8);
        pipeline.dispose();
        assert!(!pipeline.is_ready());
    }

    #[test]
    fn render_runs_reference_smaa_and_jitter_pass_order() {
        let mut pipeline = FrameRenderPipeline::new();
        pipeline.set_size(8, 6, true).unwrap();
        let mut scene = Scene::<()>::new(Uid::new(1), "test");
        scene.set_bg_color(SolidColor::from_rgb(0.25, 0.5, 0.75));
        scene.set_ao_radius(6.0);
        scene.set_ao_intensity(1.5);
        scene.set_aa_method(AAMethod::Smaa);
        let mut context = DisplayContext::new();
        context.set_fog_start(10.0);
        context.set_fog_end(20.0);
        let params = FrameRenderParams {
            jitter_active: true,
            jitter_index: 0,
            jitter_count: 4,
            blit_depth_to_default: true,
            ..FrameRenderParams::default()
        };
        assert!(pipeline.render(&context, &scene, &params, None, |target| {
            target.set_color_pixel(0, 0, 0, [1.0; 4]).unwrap();
        }));

        let passes = pipeline.post_process().unwrap().passes();
        assert!(matches!(passes[0], PostProcessPass::Gtao { .. }));
        assert!(matches!(passes[1], PostProcessPass::Denoise { .. }));
        assert!(matches!(passes[2], PostProcessPass::Composite { .. }));
        assert!(matches!(passes[3], PostProcessPass::SmaaEdges { .. }));
        assert!(matches!(passes[4], PostProcessPass::SmaaWeights { .. }));
        assert!(matches!(passes[5], PostProcessPass::SmaaBlend { .. }));
        assert!(matches!(
            passes[6],
            PostProcessPass::JitterCompose { weight } if (weight - 0.25).abs() < 1.0e-6
        ));
        assert!(matches!(
            passes[7],
            PostProcessPass::JitterCompose { weight } if (weight - 4.0).abs() < 1.0e-6
        ));
        assert_eq!(pipeline.depth_blit_count(), 1);
    }

    #[test]
    fn render_uses_composite_only_for_export_path() {
        let mut pipeline = FrameRenderPipeline::new();
        pipeline.set_size(4, 4, false).unwrap();
        let scene = Scene::<()>::new(Uid::new(1), "test");
        let params = FrameRenderParams {
            enable_post_aa: false,
            background_transparent: true,
            ..FrameRenderParams::default()
        };
        let mut output = RenderTarget::new(4, 4, RT_COLOR_RGBA8).unwrap();
        assert!(pipeline.render(
            &DisplayContext::new(),
            &scene,
            &params,
            Some(&mut output),
            |_| {}
        ));
        assert!(!output.is_bound());
        assert_eq!(
            pipeline
                .scene_target()
                .unwrap()
                .color_attachment(0)
                .unwrap()[0][3],
            0.0
        );
        let passes = pipeline.post_process().unwrap().passes();
        assert_eq!(passes.len(), 3);
        assert!(matches!(passes[2], PostProcessPass::Composite { .. }));
    }
}
