use cuemol_qsys::Camera;

pub(crate) struct CenterMarkRenderer {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
}

impl CenterMarkRenderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("center-mark"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@vertex fn vs(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
"#
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("center-mark-inversion"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.remove_srgb_suffix(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDst,
                            dst_factor: wgpu::BlendFactor::Zero,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::RED
                        | wgpu::ColorWrites::GREEN
                        | wgpu::ColorWrites::BLUE,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            vertices: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("center-mark-vertices"),
                size: 36 * 8,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &Camera,
        viewport: [f32; 4],
        pixels_per_point: f32,
        swap: bool,
    ) {
        let [x, y, width, height] = viewport;
        if width <= 0. || height <= 0. || !pixels_per_point.is_finite() {
            return;
        }
        let scale = pixels_per_point.max(0.01);
        let segments =
            crate::stereo::center_mark_segments(camera, [width / scale, height / scale], swap);
        let mut vertices = [[0_f32; 2]; 36];
        let mut count = 0;
        for [start, end] in segments {
            let start = start.map(|v| v * scale);
            let end = end.map(|v| v * scale);
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let length = dx.hypot(dy);
            if length < 0.001 {
                continue;
            }
            let normal = [-dy / length * scale * 0.5, dx / length * scale * 0.5];
            let point = |p: [f32; 2], sign: f32| {
                [
                    2. * (p[0] + sign * normal[0]) / width - 1.,
                    1. - 2. * (p[1] + sign * normal[1]) / height,
                ]
            };
            for vertex in [
                point(start, -1.),
                point(end, -1.),
                point(start, 1.),
                point(start, 1.),
                point(end, -1.),
                point(end, 1.),
            ] {
                vertices[count] = vertex;
                count += 1;
            }
        }
        if count == 0 {
            return;
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices[..count]));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("center-mark-inversion"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_viewport(x, y, width, height, 0., 1.);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..count as u32, 0..1);
    }
}
