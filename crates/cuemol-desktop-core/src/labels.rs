use crate::GuiAtomLabel;
use ab_glyph::{point, Font, FontRef, ScaleFont};
use std::collections::{HashMap, HashSet};
use wgpu::util::DeviceExt;

pub struct LabelImage {
    pub width: u32,
    pub height: u32,
    pub logical_size: [f32; 2],
    pub alpha: Vec<u8>,
}

pub fn rasterize_label(label: &GuiAtomLabel, scale: f32) -> LabelImage {
    rasterize_label_limited(label, scale, 4096)
}

fn rasterize_label_limited(label: &GuiAtomLabel, scale: f32, limit: u32) -> LabelImage {
    let font = FontRef::try_from_slice(if label.font_name.to_ascii_lowercase().contains("mono") {
        epaint_default_fonts::HACK_REGULAR
    } else {
        epaint_default_fonts::UBUNTU_LIGHT
    })
    .expect("embedded label font is valid");
    let font_size = if label.font_size.is_finite() {
        label.font_size
    } else {
        12.
    };
    let size = (font_size.clamp(1., 512.) as f32 * scale).clamp(1., 1024.);
    let scaled = font.as_scaled(size);
    let line_height = (scaled.height() + scaled.line_gap()).max(1.);
    let line_width = |line: &str| {
        let mut previous = None;
        let mut width = 0.;
        for ch in line.chars() {
            let id = font.glyph_id(ch);
            if let Some(previous) = previous {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            previous = Some(id);
        }
        width
    };
    let italic = label.font_style.contains("italic") || label.font_style.contains("oblique");
    let bold = label.font_weight.contains("bold")
        || label
            .font_weight
            .parse::<u16>()
            .is_ok_and(|weight| weight >= 600);
    let logical_width = label
        .text
        .split('\n')
        .map(line_width)
        .fold(0., f32::max)
        .ceil();
    let logical_height = (line_height * label.text.split('\n').count() as f32).ceil();
    let width = (logical_width + 4. + if italic { size * 0.2 } else { 0. })
        .ceil()
        .clamp(1., limit as f32) as u32;
    let height = (logical_height + 4.).clamp(1., limit as f32) as u32;
    let mut alpha = vec![0_u8; width as usize * height as usize];
    for (line, text) in label.text.split('\n').enumerate() {
        let mut x = 2.;
        let baseline = 2. + scaled.ascent() + line as f32 * line_height;
        let mut previous = None;
        for ch in text.chars() {
            let id = font.glyph_id(ch);
            if let Some(previous) = previous {
                x += scaled.kern(previous, id);
            }
            let glyph = id.with_scale_and_position(size, point(x, baseline));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|dx, dy, coverage| {
                    let shear = if italic {
                        ((baseline - (bounds.min.y + dy as f32)) * 0.18).round() as i32
                    } else {
                        0
                    };
                    let x = bounds.min.x as i32 + dx as i32 + shear;
                    let y = bounds.min.y as i32 + dy as i32;
                    for x in x..=x + i32::from(bold) {
                        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                            let pixel = &mut alpha[y as usize * width as usize + x as usize];
                            *pixel = (255. * coverage + f32::from(*pixel) * (1. - coverage)).round()
                                as u8;
                        }
                    }
                });
            }
            x += scaled.h_advance(id);
            previous = Some(id);
        }
    }
    LabelImage {
        width,
        height,
        logical_size: [logical_width, logical_height],
        alpha,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    font: String,
    style: String,
    weight: String,
    size: u64,
    scale: u32,
}

impl Key {
    fn new(label: &GuiAtomLabel, scale: f32) -> Self {
        Self {
            text: label.text.clone(),
            font: label.font_name.clone(),
            style: label.font_style.clone(),
            weight: label.font_weight.clone(),
            size: label.font_size.to_bits(),
            scale: scale.to_bits(),
        }
    }
}

struct GlyphTexture {
    view: wgpu::TextureView,
    size: [u32; 2],
    logical: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

pub struct LabelRenderer {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    textures: HashMap<Key, GlyphTexture>,
    vertices: wgpu::Buffer,
    capacity: usize,
    draws: Vec<Key>,
}

impl LabelRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let entries = (0..3)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                count: None,
                ty: if binding == 2 {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                } else {
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }
                },
            })
            .collect::<Vec<_>>();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("label-layout"),
            entries: &entries,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("labels"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("labels.wgsl")
                    .replace(
                        "OUTPUT_SRGB: bool = false",
                        &format!("OUTPUT_SRGB: bool = {}", format.is_srgb()),
                    )
                    .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("label-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("labels"), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4],
            }] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("label-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("label-vertices"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            layout,
            pipeline,
            sampler,
            vertices,
            capacity: 0,
            textures: HashMap::new(),
            draws: Vec::new(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        labels: &[GuiAtomLabel],
        size: [f32; 2],
        scale: f32,
    ) {
        let scale = scale.max(0.1);
        let mut vertices = Vec::new();
        self.draws.clear();
        let mut used = HashSet::new();
        for label in labels
            .iter()
            .filter(|label| !label.text.is_empty() && label.depth.is_finite())
        {
            let key = Key::new(label, scale);
            let texture = self.textures.entry(key.clone()).or_insert_with(|| {
                let image = rasterize_label_limited(
                    label,
                    scale,
                    device.limits().max_texture_dimension_2d.min(4096),
                );
                let extent = wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                };
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("label-glyphs"),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    texture.as_image_copy(),
                    &image.alpha,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(image.width),
                        rows_per_image: Some(image.height),
                    },
                    extent,
                );
                GlyphTexture {
                    view: texture.create_view(&Default::default()),
                    size: [image.width, image.height],
                    logical: image.logical_size,
                }
            });
            let left = label.x - (label.anchor[0] * texture.logical[0] + 2.) / scale;
            let top = label.y - (label.anchor[1] * texture.logical[1] + 2.) / scale;
            let right = left + texture.size[0] as f32 / scale;
            let bottom = top + texture.size[1] as f32 / scale;
            let color = cuemol_gfx::parse_color(&label.color)
                .ok()
                .and_then(|c| c.rgba8())
                .map(|c| [c.r, c.g, c.b, c.a].map(|c| f32::from(c) / 255.))
                .unwrap_or([1., 1., 0., 1.]);
            for (x, y, u, v) in [
                (left, top, 0., 0.),
                (right, top, 1., 0.),
                (left, bottom, 0., 1.),
                (left, bottom, 0., 1.),
                (right, top, 1., 0.),
                (right, bottom, 1., 1.),
            ] {
                vertices.push(Vertex {
                    position: [
                        2. * x / size[0] - 1.,
                        1. - 2. * y / size[1],
                        if label.on_top { -1. } else { label.depth },
                    ],
                    uv: [u, v],
                    color,
                });
            }
            used.insert(key.clone());
            self.draws.push(key);
        }
        self.textures.retain(|key, _| used.contains(key));
        if vertices.len() > self.capacity {
            self.vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("label-vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            self.capacity = vertices.len();
        } else if !vertices.is_empty() {
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        region: [f32; 4],
    ) {
        if self.draws.is_empty() {
            return;
        }
        let groups = self
            .draws
            .iter()
            .map(|key| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("label-textures"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.textures[key].view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(depth),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
            .collect::<Vec<_>>();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("labels"),
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
        pass.set_viewport(region[0], region[1], region[2], region[3], 0., 1.);
        pass.set_scissor_rect(
            region[0] as u32,
            region[1] as u32,
            region[2].max(1.) as u32,
            region[3].max(1.) as u32,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        for (i, group) in groups.iter().enumerate() {
            pass.set_bind_group(0, group, &[]);
            pass.draw(i as u32 * 6..(i as u32 + 1) * 6, 0..1);
        }
    }
}
