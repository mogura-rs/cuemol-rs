use crate::viewport::{CameraUniform, DEPTH_FORMAT};
use cuemol_core::Vector4;
use cuemol_render::RenderVolume;
use std::{borrow::Cow, sync::Arc};
use wgpu::util::DeviceExt;

pub const VOLUME_SHADER: &str = include_str!("volumes.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Parameters {
    world_to_texture: [[f32; 4]; 4],
    lower: [f32; 4],
    span: [f32; 4],
}

struct Grid {
    source: RenderVolume,
    _density: wgpu::Texture,
    transfer: wgpu::Texture,
    parameters: [wgpu::Buffer; 2],
    bindings: [wgpu::BindGroup; 2],
}

pub struct VolumeRenderer {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    linear_pipeline: wgpu::RenderPipeline,
    grids: Vec<Grid>,
}

impl VolumeRenderer {
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = |binding, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("volume-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture(1, wgpu::TextureViewDimension::D3),
                texture(2, wgpu::TextureViewDimension::D2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("volume-pipeline-layout"),
            bind_group_layouts: &[camera_layout, &layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("volume-shader"),
            source: wgpu::ShaderSource::Wgsl(VOLUME_SHADER.into()),
        });
        let create = |format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("volume-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        Self {
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("volume-sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            pipeline: create(format),
            linear_pipeline: create(wgpu::TextureFormat::Rgba8Unorm),
            grids: Vec::new(),
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, volumes: &[RenderVolume]) {
        self.grids.truncate(volumes.len());
        for (index, source) in volumes.iter().enumerate() {
            if let Some(grid) = self.grids.get_mut(index).filter(|grid| {
                grid.source.counts == source.counts
                    && Arc::ptr_eq(&grid.source.values, &source.values)
            }) {
                if grid.source.transfer != source.transfer {
                    write_transfer(queue, &grid.transfer, source);
                }
                grid.source = source.clone();
                continue;
            }
            let (counts, bytes) =
                texture_grid(source, device.limits().max_texture_dimension_3d as usize);
            let size = wgpu::Extent3d {
                width: counts[0] as u32,
                height: counts[1] as u32,
                depth_or_array_layers: counts[2] as u32,
            };
            let density = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("volume-density"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &density,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width),
                    rows_per_image: Some(size.height),
                },
                size,
            );
            let transfer = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("volume-transfer"),
                size: wgpu::Extent3d {
                    width: 256,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            write_transfer(queue, &transfer, source);
            let parameters = std::array::from_fn(|_| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("volume-parameters"),
                    contents: bytemuck::bytes_of(&<Parameters as bytemuck::Zeroable>::zeroed()),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
            });
            let density_view = density.create_view(&Default::default());
            let transfer_view = transfer.create_view(&Default::default());
            let bindings = std::array::from_fn(|eye| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("volume-bindings"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: parameters[eye].as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&density_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&transfer_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            });
            let grid = Grid {
                source: source.clone(),
                _density: density,
                transfer,
                parameters,
                bindings,
            };
            if index < self.grids.len() {
                self.grids[index] = grid;
            } else {
                self.grids.push(grid);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera_bindings: &wgpu::BindGroup,
        camera: &CameraUniform,
        region: Option<[f32; 4]>,
        linear: bool,
        output_srgb: bool,
        eye_index: usize,
    ) {
        let vector = |v: [f32; 4]| Vector4::new3(v[0] as f64, v[1] as f64, v[2] as f64);
        let right = vector(camera.right);
        let up = vector(camera.up);
        let front = right.cross(up);
        let eye = vector(camera.eye);
        let mut planes = Vec::new();
        for (index, grid) in self.grids.iter().enumerate() {
            let Some(slices) = grid.source.slices(eye, right, up, front) else {
                continue;
            };
            let matrix = grid.source.world_to_texture();
            let parameters = Parameters {
                world_to_texture: std::array::from_fn(|col| {
                    std::array::from_fn(|row| matrix.values[row][col] as f32)
                }),
                lower: [
                    slices.lower[0] as f32,
                    slices.lower[1] as f32,
                    slices.lower[2] as f32,
                    slices.thickness as f32,
                ],
                span: [
                    (slices.upper[0] - slices.lower[0]) as f32,
                    (slices.upper[1] - slices.lower[1]) as f32,
                    grid.source.alpha,
                    f32::from(output_srgb),
                ],
            };
            queue.write_buffer(
                &grid.parameters[eye_index],
                0,
                bytemuck::bytes_of(&parameters),
            );
            planes.extend((0..slices.count).map(|plane| {
                (
                    slices.lower[2] + f64::from(plane) * slices.thickness,
                    index,
                    plane,
                )
            }));
        }
        if planes.is_empty() {
            return;
        }
        planes.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("volume-view"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some([x, y, width, height]) = region {
            pass.set_viewport(x, y, width, height, 0., 1.);
        }
        pass.set_pipeline(if linear {
            &self.linear_pipeline
        } else {
            &self.pipeline
        });
        pass.set_bind_group(0, camera_bindings, &[]);
        let mut index = 0;
        while index < planes.len() {
            let (_, grid, first) = planes[index];
            let mut end = index + 1;
            while end < planes.len()
                && planes[end].1 == grid
                && planes[end].2 == first + (end - index) as u32
            {
                end += 1;
            }
            pass.set_bind_group(1, &self.grids[grid].bindings[eye_index], &[]);
            pass.draw(0..6, first..first + (end - index) as u32);
            index = end;
        }
    }
}

fn write_transfer(queue: &wgpu::Queue, texture: &wgpu::Texture, source: &RenderVolume) {
    let colors = source
        .transfer
        .map(|rgba| rgba.map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&colors),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(1024),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 256,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

fn texture_grid(volume: &RenderVolume, limit: usize) -> ([usize; 3], Cow<'_, [u8]>) {
    let counts = volume.counts.map(|n| n.min(limit.max(1)));
    if counts == volume.counts {
        return (counts, Cow::Borrowed(volume.values.as_slice()));
    }
    let mut bytes = Vec::with_capacity(counts.iter().product());
    for z in 0..counts[2] {
        for y in 0..counts[1] {
            for x in 0..counts[0] {
                let index = std::array::from_fn::<_, 3, _>(|axis| {
                    ((([x, y, z][axis] as f64 + 0.5) * volume.counts[axis] as f64
                        / counts[axis] as f64
                        - 0.5)
                        .round() as usize)
                        .min(volume.counts[axis] - 1)
                });
                bytes.push(
                    volume.values
                        [index[0] + volume.counts[0] * (index[1] + volume.counts[1] * index[2])],
                );
            }
        }
    }
    (counts, Cow::Owned(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_shader_validates_for_native_and_browser_targets() {
        let module = naga::front::wgsl::parse_str(VOLUME_SHADER).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        for stage in [naga::ShaderStage::Vertex, naga::ShaderStage::Fragment] {
            let options = naga::back::glsl::Options {
                version: naga::back::glsl::Version::Embedded {
                    version: 300,
                    is_webgl: true,
                },
                ..Default::default()
            };
            let pipeline = naga::back::glsl::PipelineOptions {
                shader_stage: stage,
                entry_point: if stage == naga::ShaderStage::Vertex {
                    "vs"
                } else {
                    "fs"
                }
                .into(),
                multiview: None,
            };
            naga::back::glsl::Writer::new(
                &mut String::new(),
                &module,
                &info,
                &options,
                &pipeline,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .unwrap()
            .write()
            .unwrap();
        }
    }
}
