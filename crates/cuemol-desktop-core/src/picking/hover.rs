use super::super::PickStamp;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    fill: [f32; 4],
    tones: [f32; 4],
    params: [f32; 4],
}
struct Targets {
    views: [wgpu::TextureView; 2],
    groups: [wgpu::BindGroup; 2],
    size: [u32; 2],
}

pub(super) struct HoverRenderer {
    uniform: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipelines: [wgpu::RenderPipeline; 3],
    targets: Option<Targets>,
    mask_key: Option<(PickStamp, u32, f32)>,
    output_srgb: bool,
}
impl HoverRenderer {
    pub(super) fn invalidate_texture(&mut self) {
        self.targets = None;
        self.mask_key = None;
    }

    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hover-uniform"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hover-layout"),
            entries: &[
                texture(0, wgpu::TextureSampleType::Uint),
                texture(1, wgpu::TextureSampleType::Float { filterable: true }),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hover-mask-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hover-shader"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../appearance.wgsl"),
                    "\n",
                    include_str!("hover.wgsl")
                )
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hover-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipelines = ["mask_horizontal", "mask_vertical", "hover_overlay"].map(|entry| {
            let overlay = entry == "hover_overlay";
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("hover_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: if overlay {
                            format
                        } else {
                            wgpu::TextureFormat::Rgba8Unorm
                        },
                        blend: overlay.then_some(wgpu::BlendState::ALPHA_BLENDING),
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
        });
        Self {
            uniform,
            layout,
            sampler,
            pipelines,
            targets: None,
            mask_key: None,
            output_srgb: format.is_srgb(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        ids: &wgpu::TextureView,
        size: [u32; 2],
        stamp: PickStamp,
        id: u32,
        color: [f32; 3],
        background: [f32; 3],
        region: [f32; 4],
        pixels_per_point: f32,
    ) {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let views = [0, 1].map(|_| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("hover-mask"),
                        size: wgpu::Extent3d {
                            width: size[0],
                            height: size[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            });
            let groups = [1, 0].map(|input| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("hover-group"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(ids),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&views[input]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.uniform.as_entire_binding(),
                        },
                    ],
                })
            });
            self.targets = Some(Targets {
                views,
                groups,
                size,
            });
            self.mask_key = None;
        }
        let sigma = (1.5 * pixels_per_point * 0.5).clamp(0.5, 16.);
        let linear = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let luminance = background
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(v, w)| linear(v) * w)
            .sum::<f32>();
        let tones = if luminance < 0.2 {
            [1., 0.1, 0., 0.]
        } else {
            [0.1, 0.95, 0., 0.]
        };
        let uniform = Uniform {
            fill: [color[0], color[1], color[2], 0.35],
            tones,
            params: [
                id as f32,
                sigma,
                (sigma * 2.).ceil(),
                f32::from(self.output_srgb),
            ],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let targets = self.targets.as_ref().unwrap();
        let key = (stamp, id, sigma);
        let start = if self.mask_key == Some(key) { 2 } else { 0 };
        for index in start..3 {
            let overlay = index == 2;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hover-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if overlay {
                        target
                    } else {
                        &targets.views[index]
                    },
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if overlay {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if overlay {
                pass.set_viewport(region[0], region[1], region[2], region[3], 0., 1.);
            }
            pass.set_pipeline(&self.pipelines[index]);
            pass.set_bind_group(0, &targets.groups[usize::from(index == 1)], &[]);
            pass.draw(0..3, 0..1);
        }
        self.mask_key = Some(key);
    }
}
