//! GPU post-processing shared by native, browser, and PNG views.
use crate::viewport::{target_clear_color, CameraUniform, ViewportRenderer, DEPTH_FORMAT};
use cuemol_gfx::{jitter_offset, jitter_sample_count, AoConstants};
use cuemol_molstr::qsys::MolScene;
use cuemol_qsys::Camera;

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const FLOAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const SHADER: &str = concat!(
    include_str!("appearance.wgsl"),
    "\n",
    include_str!("effects/common.wgsl"),
    "\n",
    include_str!("effects/ao.wgsl"),
    "\n",
    include_str!("effects/aa.wgsl"),
);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Antialiasing {
    None,
    #[default]
    Fxaa,
    Smaa,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportOptions {
    pub ao_enabled: bool,
    pub ao_radius: f32,
    pub ao_intensity: f32,
    pub ao_slices: u32,
    pub ao_steps: u32,
    pub ao_half_res: bool,
    pub antialiasing: Antialiasing,
    pub jitter_level: i32,
    pub smaa_threshold: f32,
    pub swap_stereo_eyes: bool,
}

impl Default for ViewportOptions {
    fn default() -> Self {
        Self {
            ao_enabled: false,
            ao_radius: 4.0,
            ao_intensity: 2.2,
            ao_slices: 9,
            ao_steps: 3,
            ao_half_res: false,
            antialiasing: Antialiasing::Fxaa,
            jitter_level: 0,
            smaa_threshold: 0.05,
            swap_stereo_eyes: false,
        }
    }
}

impl ViewportOptions {
    pub fn from_scene(scene: &MolScene) -> Self {
        Self::from_attributes(|key| scene.scene_attribute(key))
    }

    pub fn from_attributes<'a>(get: impl Fn(&str) -> Option<&'a str>) -> Self {
        let number = |key, default: f32| {
            get(key)
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .unwrap_or(default)
        };
        let boolean = |key| matches!(get(key), Some("true" | "1"));
        Self {
            ao_enabled: boolean("aoEnabled"),
            ao_radius: number("aoRadius", 4.0).max(0.0),
            ao_intensity: number("aoIntensity", 2.2).max(0.0),
            ao_slices: number("aoSlices", 9.0).clamp(1.0, 16.0) as u32,
            ao_steps: number("aoSteps", 3.0).clamp(1.0, 16.0) as u32,
            ao_half_res: boolean("aoHalfRes"),
            antialiasing: match get("aa_method") {
                Some("none" | "0") => Antialiasing::None,
                Some("smaa" | "2") => Antialiasing::Smaa,
                _ => Antialiasing::Fxaa,
            },
            jitter_level: number("aaJitterLevel", 0.0).clamp(0.0, 5.0) as i32,
            smaa_threshold: number("aaSmaaThreshold", 0.05).clamp(0.0, 1.0),
            swap_stereo_eyes: false,
        }
    }

    pub fn from_state(state: &crate::DesktopAppState) -> Self {
        let mut options = state
            .active_scene()
            .and_then(|uid| state.scene_manager().scene(uid))
            .map(Self::from_scene)
            .unwrap_or_default();
        options.swap_stereo_eyes = state
            .active_view_summary()
            .and_then(|view| state.command_manager().view(view.uid))
            .is_some_and(|view| view.swap_stereo_eyes());
        options
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Parameters {
    depth: [f32; 4],
    ndc: [f32; 4],
    pixel: [f32; 4],
    ao: [f32; 4],
    misc: [f32; 4],
    aa: [f32; 4],
    output: [f32; 4],
}

impl Parameters {
    fn new(camera: &Camera, size: [u32; 2], options: ViewportOptions, half: bool) -> Self {
        let a = AoConstants::from_camera(
            camera.cam_dist(),
            camera.zoom(),
            camera.slab_depth(),
            size[0] as f64 / size[1] as f64,
            size[0] as i32,
            size[1] as i32,
            camera.is_perspective(),
        );
        let ao_size = if half {
            size.map(|n| n.div_ceil(2))
        } else {
            size
        };
        let fog_end = camera.cam_dist() + camera.slab_depth() * 0.5;
        Self {
            depth: [
                a.depth_linearize_mul,
                a.depth_linearize_add,
                a.is_ortho as f32,
                f32::from(options.ao_enabled),
            ],
            ndc: [
                a.ndc_to_view_mul[0],
                a.ndc_to_view_mul[1],
                a.ndc_to_view_add[0],
                a.ndc_to_view_add[1],
            ],
            pixel: [
                1.0 / size[0] as f32,
                1.0 / size[1] as f32,
                1.0 / ao_size[0] as f32,
                1.0 / ao_size[1] as f32,
            ],
            ao: [
                options.ao_radius,
                options.ao_intensity,
                options.ao_slices as f32,
                options.ao_steps as f32,
            ],
            misc: [
                0.0,
                256.0 * ao_size[0] as f32 / size[0] as f32,
                fog_end as f32,
                (1.0 / (fog_end - camera.cam_dist().max(1.0)).max(1.0e-6)) as f32,
            ],
            aa: [options.smaa_threshold, f32::from(half), 1.0, 1.0],
            output: [0.0; 4],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct FrameKey {
    camera: CameraUniform,
    revision: u64,
    options: ViewportOptions,
}

#[derive(Default)]
struct Refinement {
    key: Option<FrameKey>,
    samples: usize,
    refine_half: bool,
}

struct FramePlan {
    draw: bool,
    half: bool,
    sample: Option<usize>,
    count: usize,
    accumulated: usize,
    pending: bool,
}

impl Refinement {
    fn next(&mut self, key: FrameKey) -> FramePlan {
        let options = key.options;
        let count = jitter_sample_count(options.jitter_level.clamp(0, 5));
        if self.key != Some(key) {
            self.key = Some(key);
            self.samples = 0;
            self.refine_half = options.ao_enabled && options.ao_half_res;
            return FramePlan {
                draw: true,
                half: self.refine_half,
                sample: None,
                count,
                accumulated: 0,
                pending: self.refine_half || count > 1,
            };
        }
        let sample = (count > 1 && self.samples < count).then_some(self.samples);
        let draw = sample.is_some() || self.refine_half;
        self.refine_half = false;
        if sample.is_some() {
            self.samples += 1;
        }
        FramePlan {
            draw,
            half: false,
            sample,
            count,
            accumulated: self.samples,
            pending: count > 1 && self.samples < count,
        }
    }
}

struct Targets {
    size: [u32; 2],
    color: wgpu::TextureView,
    normal: wgpu::TextureView,
    depth: wgpu::TextureView,
    encoded_depth: wgpu::TextureView,
    ao: [wgpu::TextureView; 2],
    denoised: [wgpu::TextureView; 2],
    composite: wgpu::TextureView,
    edges: wgpu::TextureView,
    weights: wgpu::TextureView,
    sample: wgpu::TextureView,
    accumulation: wgpu::TextureView,
}

fn target(
    device: &wgpu::Device,
    label: &'static str,
    size: [u32; 2],
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl Targets {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let make = |label, format| target(device, label, size, format);
        let ao = |label| {
            [
                target(device, label, size, COLOR),
                target(device, label, size.map(|n| n.div_ceil(2)), COLOR),
            ]
        };
        Self {
            size,
            color: make("effects-scene", COLOR),
            normal: make("effects-normal", FLOAT),
            depth: make("effects-depth", DEPTH_FORMAT),
            encoded_depth: make("effects-packed-depth", COLOR),
            ao: ao("effects-ao"),
            denoised: ao("effects-denoised"),
            composite: make("effects-composite", COLOR),
            edges: make("effects-edges", COLOR),
            weights: make("effects-weights", COLOR),
            sample: make("effects-sample", COLOR),
            accumulation: make("effects-accumulation", FLOAT),
        }
    }
}

pub struct ViewportEffects {
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    area: wgpu::TextureView,
    search: wgpu::TextureView,
    gtao: wgpu::RenderPipeline,
    denoise: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    fxaa: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    weights: wgpu::RenderPipeline,
    blend: wgpu::RenderPipeline,
    copy: wgpu::RenderPipeline,
    accumulate: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    output_srgb: bool,
    targets: Option<Targets>,
    refinement: Refinement,
    format: wgpu::TextureFormat,
    right_eye: Option<Box<ViewportEffects>>,
    stereo_depth: Option<([u32; 2], wgpu::TextureView)>,
}

impl ViewportEffects {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        for binding in 1..=8 {
            let ty = match binding {
                5 | 6 => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                _ => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            };
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty,
                count: None,
            });
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("effects-layout"),
            entries: &entries,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport-effects"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("effects-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |entry, format, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("fullscreen_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                min_filter: filter,
                mag_filter: filter,
                ..Default::default()
            })
        };
        let table = |label, size: [u32; 2], format, bytes_per_pixel, data: &[u8]| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size[0] * bytes_per_pixel),
                    rows_per_image: Some(size[1]),
                },
                texture.size(),
            );
            texture.create_view(&Default::default())
        };
        Self {
            gtao: pipeline("gtao_fs", COLOR, None),
            denoise: pipeline("denoise_fs", COLOR, None),
            composite: pipeline("composite_fs", COLOR, None),
            fxaa: pipeline("fxaa_fs", COLOR, None),
            edges: pipeline("smaa_edges_fs", COLOR, None),
            weights: pipeline("smaa_weights_fs", COLOR, None),
            blend: pipeline("smaa_blend_fs", COLOR, None),
            copy: pipeline("copy_fs", COLOR, None),
            accumulate: pipeline(
                "accumulate_fs",
                FLOAT,
                Some(wgpu::BlendState {
                    color: add,
                    alpha: add,
                }),
            ),
            present: pipeline("present_fs", format, None),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("effects-parameters"),
                size: std::mem::size_of::<Parameters>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            linear: sampler(wgpu::FilterMode::Linear),
            nearest: sampler(wgpu::FilterMode::Nearest),
            area: table(
                "smaa-area",
                [160, 560],
                wgpu::TextureFormat::Rg8Unorm,
                2,
                include_bytes!("../assets/smaa/area.dat"),
            ),
            search: table(
                "smaa-search",
                [66, 33],
                wgpu::TextureFormat::R8Unorm,
                1,
                include_bytes!("../assets/smaa/search.dat"),
            ),
            layout,
            output_srgb: format.is_srgb(),
            targets: None,
            refinement: Refinement::default(),
            format,
            right_eye: None,
            stereo_depth: None,
        }
    }

    fn ensure_size(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self
            .targets
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            self.targets = Some(Targets::new(device, size));
            self.refinement = Refinement::default();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        output: &wgpu::TextureView,
        color: &wgpu::TextureView,
        auxiliary: &wgpu::TextureView,
        clear: Option<wgpu::Color>,
        region: Option<[f32; 4]>,
    ) {
        let targets = self.targets.as_ref().unwrap();
        let resources = [
            self.uniform.as_entire_binding(),
            wgpu::BindingResource::TextureView(color),
            wgpu::BindingResource::TextureView(auxiliary),
            wgpu::BindingResource::TextureView(&targets.encoded_depth),
            wgpu::BindingResource::TextureView(&targets.normal),
            wgpu::BindingResource::Sampler(&self.linear),
            wgpu::BindingResource::Sampler(&self.nearest),
            wgpu::BindingResource::TextureView(&self.area),
            wgpu::BindingResource::TextureView(&self.search),
        ];
        let entries = resources
            .into_iter()
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource,
            })
            .collect::<Vec<_>>();
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("effects-bindings"),
            layout: &self.layout,
            entries: &entries,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport-effects"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some([x, y, width, height]) = region {
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn postprocess(
        &self,
        device: &wgpu::Device,
        renderer: &ViewportRenderer,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        uniform: &CameraUniform,
        options: ViewportOptions,
        half: bool,
        eye: usize,
    ) {
        let t = self.targets.as_ref().unwrap();
        let pass = |encoder: &mut wgpu::CommandEncoder, pipeline, output, color, aux| {
            self.pass(
                device,
                encoder,
                pipeline,
                output,
                color,
                aux,
                Some(wgpu::Color::TRANSPARENT),
                None,
            )
        };
        let mut color = &t.color;
        if options.ao_enabled {
            let index = usize::from(half);
            pass(encoder, &self.gtao, &t.ao[index], color, color);
            pass(
                encoder,
                &self.denoise,
                &t.denoised[index],
                &t.ao[index],
                &t.ao[index],
            );
            pass(
                encoder,
                &self.composite,
                &t.composite,
                color,
                &t.denoised[index],
            );
            color = &t.composite;
        }
        renderer.render_volumes(encoder, queue, color, &t.depth, uniform, None, true, eye);
        match options.antialiasing {
            Antialiasing::None => pass(encoder, &self.copy, &t.sample, color, color),
            Antialiasing::Fxaa => pass(encoder, &self.fxaa, &t.sample, color, color),
            Antialiasing::Smaa => {
                pass(encoder, &self.edges, &t.edges, color, color);
                pass(encoder, &self.weights, &t.weights, &t.edges, &t.edges);
                pass(encoder, &self.blend, &t.sample, color, &t.weights);
            }
        }
    }

    /// Packed depth from the most recent scene pass, including primitive impostors.
    pub fn label_depth(&self) -> Option<&wgpu::TextureView> {
        self.stereo_depth
            .as_ref()
            .map(|(_, depth)| depth)
            .or_else(|| self.targets.as_ref().map(|targets| &targets.encoded_depth))
    }

    /// Render both eye viewports with shared geometry and independent frame uniforms.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        renderer: &ViewportRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &Camera,
        size: [u32; 2],
        background: wgpu::Color,
        region: Option<[f32; 4]>,
        options: ViewportOptions,
    ) -> bool {
        let size = size.map(|n| n.max(1));
        let eyes =
            cuemol_qsys::stereo::viewports(camera, size.map(f64::from), options.swap_stereo_eyes);
        if eyes.len() == 1 {
            self.right_eye = None;
            self.stereo_depth = None;
            return self.render_eye(
                renderer,
                device,
                queue,
                encoder,
                target,
                &eyes[0].camera,
                size,
                background,
                region,
                options,
                0,
                true,
            );
        }
        if self.right_eye.is_none() {
            self.right_eye = Some(Box::new(Self::new(device, queue, self.format)));
        }
        let left_width = size[0] / 2;
        let right_width = size[0] - left_width;
        let [x, y, width, height] = region.unwrap_or([0., 0., size[0] as f32, size[1] as f32]);
        let split_width = width * left_width as f32 / size[0] as f32;
        let left_pending = self.render_eye(
            renderer,
            device,
            queue,
            encoder,
            target,
            &eyes[0].camera,
            [left_width, size[1]],
            background,
            Some([x, y, split_width, height]),
            options,
            0,
            true,
        );
        let right_pending = self.right_eye.as_mut().unwrap().render_eye(
            renderer,
            device,
            queue,
            encoder,
            target,
            &eyes[1].camera,
            [right_width, size[1]],
            background,
            Some([x + split_width, y, width - split_width, height]),
            options,
            1,
            false,
        );
        if self
            .stereo_depth
            .as_ref()
            .is_none_or(|(old, _)| *old != size)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("stereo-label-depth"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.stereo_depth = Some((size, texture.create_view(&Default::default())));
        }
        let depth = &self.stereo_depth.as_ref().unwrap().1;
        let left = &self.targets.as_ref().unwrap().encoded_depth;
        self.pass(
            device,
            encoder,
            &self.copy,
            depth,
            left,
            left,
            Some(wgpu::Color::WHITE),
            Some([0., 0., left_width as f32, size[1] as f32]),
        );
        let right = self.right_eye.as_ref().unwrap();
        let image = &right.targets.as_ref().unwrap().encoded_depth;
        right.pass(
            device,
            encoder,
            &right.copy,
            depth,
            image,
            image,
            None,
            Some([left_width as f32, 0., right_width as f32, size[1] as f32]),
        );
        left_pending || right_pending
    }

    /// Returns whether another frame is needed to refine AO or temporal AA.
    #[allow(clippy::too_many_arguments)]
    fn render_eye(
        &mut self,
        renderer: &ViewportRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &Camera,
        size: [u32; 2],
        background: wgpu::Color,
        region: Option<[f32; 4]>,
        options: ViewportOptions,
        eye: usize,
        clear: bool,
    ) -> bool {
        let size = size.map(|n| n.max(1));
        self.ensure_size(device, size);
        let mut uniform = CameraUniform::new(camera, size, background, false);
        let plan = self.refinement.next(FrameKey {
            camera: uniform,
            revision: renderer.revision(),
            options,
        });
        let mut params = Parameters::new(camera, size, options, plan.half);
        params.aa[2] = 1.0 / plan.count as f32;
        params.aa[3] = plan.count as f32 / plan.accumulated.max(1) as f32;
        params.output[0] = f32::from(self.output_srgb);
        params.output[1] = f32::from(plan.accumulated > 0);
        if let Some(index) = plan.sample {
            uniform.view_proj = crate::offscreen::jittered_mvp(
                uniform.view_proj,
                size[0],
                size[1],
                jitter_offset(options.jitter_level.clamp(0, 5), index as i32),
            );
            params.misc[0] = (index as f32 * 0.618_034).fract();
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
        let t = self.targets.as_ref().unwrap();
        if plan.draw {
            renderer.render_gbuffer(
                encoder,
                queue,
                &t.color,
                &t.depth,
                &t.normal,
                &t.encoded_depth,
                &uniform,
                background,
                eye,
            );
            self.postprocess(
                device, renderer, queue, encoder, &uniform, options, plan.half, eye,
            );
            if let Some(index) = plan.sample {
                self.pass(
                    device,
                    encoder,
                    &self.accumulate,
                    &t.accumulation,
                    &t.sample,
                    &t.sample,
                    (index == 0).then_some(wgpu::Color::TRANSPARENT),
                    None,
                );
            }
        }
        let color = if plan.accumulated > 0 {
            &t.accumulation
        } else {
            &t.sample
        };
        self.pass(
            device,
            encoder,
            &self.present,
            target,
            color,
            color,
            clear.then(|| target_clear_color(background, self.output_srgb)),
            region,
        );
        plan.pending
    }

    /// Export uses AO at full resolution; spatial AA and temporal history are omitted.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_export(
        &mut self,
        renderer: &ViewportRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &Camera,
        uniform: &CameraUniform,
        size: [u32; 2],
        background: wgpu::Color,
        mut options: ViewportOptions,
    ) {
        self.right_eye = None;
        self.stereo_depth = None;
        let size = size.map(|n| n.max(1));
        self.ensure_size(device, size);
        self.refinement = Refinement::default();
        options.antialiasing = Antialiasing::None;
        let mut params = Parameters::new(camera, size, options, false);
        params.output[0] = f32::from(self.output_srgb);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
        let t = self.targets.as_ref().unwrap();
        renderer.render_gbuffer(
            encoder,
            queue,
            &t.color,
            &t.depth,
            &t.normal,
            &t.encoded_depth,
            uniform,
            background,
            0,
        );
        self.postprocess(device, renderer, queue, encoder, uniform, options, false, 0);
        self.pass(
            device,
            encoder,
            &self.present,
            target,
            &t.sample,
            &t.sample,
            Some(background),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_postprocess_shaders_validate() {
        for source in [SHADER, crate::viewport::MOLECULE_SHADER] {
            let module = naga::front::wgsl::parse_str(source).unwrap();
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
            let options = naga::back::glsl::Options {
                version: naga::back::glsl::Version::Embedded {
                    version: 300,
                    is_webgl: true,
                },
                ..Default::default()
            };
            for entry in &module.entry_points {
                let pipeline = naga::back::glsl::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                    multiview: None,
                };
                let mut output = String::new();
                naga::back::glsl::Writer::new(
                    &mut output,
                    &module,
                    &info,
                    &options,
                    &pipeline,
                    Default::default(),
                )
                .unwrap()
                .write()
                .unwrap_or_else(|error| panic!("WebGL2 shader {}: {error}", entry.name));
            }
        }
    }

    #[test]
    fn temporal_history_refines_once_and_resets_on_scene_or_camera_changes() {
        let mut state = Refinement::default();
        let mut key = FrameKey {
            camera: CameraUniform::new(&Camera::new(), [80, 60], wgpu::Color::BLACK, false),
            revision: 1,
            options: ViewportOptions {
                jitter_level: 2,
                ao_enabled: true,
                ao_half_res: true,
                ..Default::default()
            },
        };
        let first = state.next(key);
        assert!(first.half && first.draw && first.pending);
        assert_eq!(first.sample, None);
        for index in 0..4 {
            let frame = state.next(key);
            assert_eq!(frame.sample, Some(index));
            assert!(!frame.half && frame.draw);
            assert_eq!(frame.pending, index < 3);
        }
        let cached = state.next(key);
        assert!(!cached.draw && !cached.pending);
        assert_eq!(cached.accumulated, 4);
        key.revision += 1;
        assert_eq!(state.next(key).accumulated, 0);
        state.next(key);
        key.camera.view_proj[3][0] += 0.1;
        assert_eq!(state.next(key).sample, None);
        key.options.jitter_level = 0;
        assert!(state.next(key).pending);
        let full = state.next(key);
        assert!(full.draw && !full.half && !full.pending);
        assert!(!state.next(key).draw);
    }
}
