#[path = "hover.rs"]
mod hover;
use super::{PickCache, PickImage, PickRecord, PickStamp};
use crate::viewport::*;
use cuemol_core::{BSPTree, Uid};
use cuemol_qsys::{Camera, ViewHoverHit};
use cuemol_render::RenderSceneSnapshot;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use wgpu::util::DeviceExt;

const PICK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Uint;

struct Targets {
    ids: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    size: [u32; 2],
}
struct Readback {
    buffer: Arc<wgpu::Buffer>,
    padded: u32,
    size: [u32; 2],
    stamp: PickStamp,
    records: Arc<[PickRecord]>,
}

pub struct GpuPicking {
    atoms: ReusableGpuBuffer,
    bonds: ReusableGpuBuffer,
    vertices: ReusableGpuBuffer,
    indices: ReusableGpuBuffer,
    quad: wgpu::Buffer,
    camera: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    pipelines: [wgpu::RenderPipeline; 3],
    counts: [u32; 3],
    records: Arc<[PickRecord]>,
    uploaded: Option<u64>,
    targets: Option<Targets>,
    stamp: Option<PickStamp>,
    queued: Option<Readback>,
    progress: Arc<AtomicU8>,
    cache: PickCache,
    record_ids: HashMap<ViewHoverHit, u32>,
    hover: hover::HoverRenderer,
}

impl GpuPicking {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pick-camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pick-camera-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pick-camera-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pick-shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{MOLECULE_SHADER}\n{}", include_str!("pick.wgsl")).into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pick-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipelines = [0, 1, 2].map(|kind| {
            let (vertex, fragment, buffers) = match kind {
                0 => (
                    "atom_vs",
                    "atom_pick_fs",
                    vec![atom_quad_layout(), atom_instance_layout()],
                ),
                1 => (
                    "bond_vs",
                    "bond_pick_fs",
                    vec![atom_quad_layout(), bond_instance_layout()],
                ),
                _ => ("mesh_pick_vs", "mesh_pick_fs", vec![mesh_vertex_layout()]),
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("pick-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    buffers: &buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: PICK_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        });
        Self {
            atoms: ReusableGpuBuffer::new(device, "pick-atoms", wgpu::BufferUsages::VERTEX),
            bonds: ReusableGpuBuffer::new(device, "pick-bonds", wgpu::BufferUsages::VERTEX),
            vertices: ReusableGpuBuffer::new(
                device,
                "pick-mesh-vertices",
                wgpu::BufferUsages::VERTEX,
            ),
            indices: ReusableGpuBuffer::new(device, "pick-mesh-indices", wgpu::BufferUsages::INDEX),
            quad: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pick-quad"),
                contents: bytemuck::cast_slice(&ATOM_QUAD),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            camera,
            camera_group,
            pipelines,
            counts: [0; 3],
            records: Arc::from([]),
            uploaded: None,
            targets: None,
            stamp: None,
            queued: None,
            progress: Arc::new(AtomicU8::new(1)),
            cache: Default::default(),
            record_ids: HashMap::new(),
            hover: hover::HoverRenderer::new(device, format),
        }
    }

    pub fn cache(&self) -> PickCache {
        self.cache.clone()
    }
    pub fn is_pending(&self) -> bool {
        self.progress.load(Ordering::Acquire) == 0 || self.queued.is_some()
    }
    pub fn image(&self, stamp: PickStamp) -> Option<Arc<PickImage>> {
        self.cache
            .lock()
            .ok()?
            .as_ref()
            .filter(|image| image.stamp == stamp)
            .cloned()
    }
    pub fn stamp(revision: u64, camera: &Camera, size: [u32; 2]) -> PickStamp {
        PickStamp {
            revision,
            matrix: crate::offscreen::camera_pick_mvp(
                camera,
                size[0] as f64 / size[1].max(1) as f64,
            ),
            size,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &RenderSceneSnapshot) {
        let mut atom_ids = vec![0u32; scene.atoms.len()];
        let mut bond_ids = vec![[0u32; 2]; scene.bonds.len()];
        let mut mesh_ids: Vec<_> = scene
            .meshes
            .iter()
            .map(|m| vec![0u32; m.vertices.len()])
            .collect();
        let mut records = Vec::new();
        let mut lookup = HashMap::new();
        for section in &scene.transparency_groups {
            let Some(group) = &section.picking else {
                continue;
            };
            if !group.gpu_supported || section.alpha <= 0.6 {
                continue;
            }
            let mut index = BSPTree::new();
            let mut atom_lookup = HashMap::new();
            for atom in group.atoms.iter().chain(group.gpu_atoms.iter()) {
                let hit = ViewHoverHit {
                    renderer_id: Uid::new(group.renderer_id),
                    atom_id: atom.atom_id,
                    symm_id: group.symm_id,
                };
                let id = *lookup.entry(hit).or_insert_with(|| {
                    records.push(PickRecord {
                        hit,
                        object_id: group.object_id,
                        atom_index: atom.atom_index,
                    });
                    records.len() as u32
                });
                atom_lookup.insert(atom.atom_index, id);
            }
            for atom in group.atoms.iter() {
                if let Some(id) = atom_lookup.get(&atom.atom_index) {
                    index.push(atom.position, *id);
                }
            }
            index.build();
            for i in section
                .atom_range
                .clone()
                .filter(|&i| i < scene.atoms.len())
            {
                let atom = &scene.atoms[i];
                atom_ids[i] = atom
                    .atom_index
                    .and_then(|i| atom_lookup.get(&i).copied())
                    .or_else(|| index.nearest(atom.center).copied())
                    .unwrap_or(0);
            }
            for i in section
                .bond_range
                .clone()
                .filter(|&i| i < scene.bonds.len())
            {
                let bond = &scene.bonds[i];
                bond_ids[i] = [
                    bond.atom_indices
                        .and_then(|p| atom_lookup.get(&p.0).copied())
                        .or_else(|| index.nearest(bond.from).copied())
                        .unwrap_or(0),
                    bond.atom_indices
                        .and_then(|p| atom_lookup.get(&p.1).copied())
                        .or_else(|| index.nearest(bond.to).copied())
                        .unwrap_or(0),
                ];
            }
            for i in section
                .mesh_range
                .clone()
                .filter(|&i| i < scene.meshes.len())
            {
                for (vertex, id) in scene.meshes[i].vertices.iter().zip(&mut mesh_ids[i]) {
                    *id = if vertex.is_edge_outline() {
                        0
                    } else if let Some(atom) = vertex.pick_atom_index {
                        atom_lookup.get(&(atom as usize)).copied().unwrap_or(0)
                    } else {
                        index.nearest(vertex.position).copied().unwrap_or(0)
                    };
                }
            }
        }
        let atoms: Vec<_> = scene
            .atoms
            .iter()
            .zip(atom_ids)
            .filter(|(atom, id)| *id != 0 && atom.hit.is_none() && atom_color(atom)[3] > 0.)
            .map(|(atom, id)| {
                let mut value = atom_instance(atom);
                value.color = [id as f32, 0., 0., 1.];
                value.selected = [0.; 4];
                value.edge = [0.; 4];
                value
            })
            .collect();
        let bonds: Vec<_> = scene
            .bonds
            .iter()
            .zip(bond_ids)
            .filter(|(bond, ids)| ids.iter().any(|id| *id != 0) && bond_color(bond)[3] > 0.)
            .map(|(bond, ids)| {
                let mut value = bond_instance(bond);
                value.to_edge[3] = 0.;
                value.edge_mode[0] = 0.;
                value.edge_mode[2] *= 0.5;
                value.screen[0] *= 0.5;
                value.screen[1] *= 0.5;
                value.edge_color = [ids[0] as f32, ids[1] as f32, 0., 0.];
                value
            })
            .collect();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for (mesh, ids) in scene.meshes.iter().zip(mesh_ids) {
            let base = vertices.len() as u32;
            vertices.extend(
                mesh.vertices
                    .iter()
                    .zip(ids.iter().copied())
                    .map(|(vertex, id)| {
                        let mut value = mesh_gpu_vertex(vertex);
                        value.color = [id as f32, 0., 0., 1.];
                        value
                    }),
            );
            for triangle in mesh.indices.chunks_exact(3) {
                if triangle
                    .iter()
                    .any(|&i| ids.get(i as usize).is_some_and(|id| *id != 0))
                    && triangle.iter().any(|&i| {
                        mesh.vertices
                            .get(i as usize)
                            .is_some_and(|v| v.color[3] > 0.)
                    })
                {
                    // OpenGL assigns flat mesh identities from the last vertex.
                    indices.extend([triangle[2], triangle[0], triangle[1]].map(|i| base + i));
                }
            }
        }
        self.counts = [atoms.len() as u32, bonds.len() as u32, indices.len() as u32];
        self.records = records.into();
        self.record_ids = lookup;
        self.atoms.upload(device, queue, &atoms);
        self.bonds.upload(device, queue, &bonds);
        self.vertices.upload(device, queue, &vertices);
        self.indices.upload(device, queue, &indices);
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &RenderSceneSnapshot,
        revision: u64,
        camera: &Camera,
        size: [u32; 2],
    ) {
        if self.is_pending() {
            return;
        }
        if self.progress.load(Ordering::Acquire) == 2 {
            self.stamp = None;
        }
        let stamp = Self::stamp(revision, camera, size);
        if self.stamp == Some(stamp) {
            return;
        }
        if self.uploaded != Some(revision) {
            self.upload(device, queue, scene);
            self.uploaded = Some(revision);
        }
        let size = size.map(|n| {
            n.div_ceil(2)
                .max(1)
                .min(device.limits().max_texture_dimension_2d)
        });
        if self
            .targets
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            self.hover.invalidate_texture();
            let ids = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pick-ids"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PICK_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = ids.create_view(&Default::default());
            self.targets = Some(Targets {
                ids,
                view,
                depth: create_depth_view(device, size[0], size[1]),
                size,
            });
        }
        let target = self.targets.as_ref().unwrap();
        let mut uniform = CameraUniform::new(camera, size, wgpu::Color::TRANSPARENT, false);
        uniform.view_proj = stamp.matrix;
        queue.write_buffer(&self.camera, 0, bytemuck::bytes_of(&uniform));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pick-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.camera_group, &[]);
            for kind in 0..3 {
                if self.counts[kind] == 0 {
                    continue;
                }
                pass.set_pipeline(&self.pipelines[kind]);
                if kind == 2 {
                    pass.set_vertex_buffer(0, self.vertices.slice());
                    pass.set_index_buffer(self.indices.slice(), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..self.counts[2], 0, 0..1);
                } else {
                    pass.set_vertex_buffer(0, self.quad.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        if kind == 0 {
                            self.atoms.slice()
                        } else {
                            self.bonds.slice()
                        },
                    );
                    pass.draw(0..6, 0..self.counts[kind]);
                }
            }
        }
        let padded = (size[0] * 16).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pick-readback"),
            size: u64::from(padded) * u64::from(size[1]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.ids,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queued = Some(Readback {
            buffer,
            padded,
            size,
            stamp,
            records: self.records.clone(),
        });
        self.stamp = Some(stamp);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_hover(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        stamp: PickStamp,
        hit: Option<ViewHoverHit>,
        color: [f32; 3],
        background: [f32; 3],
        region: [f32; 4],
        pixels_per_point: f32,
    ) {
        if self.stamp != Some(stamp) {
            return;
        }
        let Some(id) = hit.and_then(|hit| self.record_ids.get(&hit).copied()) else {
            return;
        };
        let Some(targets) = &self.targets else {
            return;
        };
        self.hover.render(
            device,
            queue,
            encoder,
            target,
            &targets.view,
            targets.size,
            stamp,
            id,
            color,
            background,
            region,
            pixels_per_point,
        );
    }

    pub fn start_readback(&mut self) {
        let Some(readback) = self.queued.take() else {
            return;
        };
        self.progress.store(0, Ordering::Release);
        let progress = self.progress.clone();
        let cache = self.cache.clone();
        let buffer = readback.buffer.clone();
        readback
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_err() {
                    progress.store(2, Ordering::Release);
                    return;
                }
                let mapped = buffer.slice(..).get_mapped_range();
                let pixels = mapped
                    .chunks_exact(readback.padded as usize)
                    .flat_map(|row| row[..readback.size[0] as usize * 16].chunks_exact(16))
                    .map(|pixel| {
                        std::array::from_fn(|i| {
                            u32::from_le_bytes(pixel[i * 4..i * 4 + 4].try_into().unwrap())
                        })
                    })
                    .collect();
                drop(mapped);
                buffer.unmap();
                if let Ok(mut cache) = cache.lock() {
                    *cache = Some(Arc::new(PickImage {
                        stamp: readback.stamp,
                        size: readback.size,
                        pixels,
                        records: readback.records,
                    }));
                }
                progress.store(1, Ordering::Release);
            });
    }
}
