//! Shared instanced molecular pipelines for native and browser views.
use cuemol_gfx::Rgba8;
use cuemol_render::{MeshVertex, RenderAtom, RenderBond, RenderSceneSnapshot};
use wgpu::util::DeviceExt;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
pub struct ReusableGpuBuffer {
    pub buffer: wgpu::Buffer,
    pub capacity: wgpu::BufferAddress,
    pub label: &'static str,
    pub usage: wgpu::BufferUsages,
}

impl ReusableGpuBuffer {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: create_scene_buffer(device, label, usage, MIN_SCENE_BUFFER_CAPACITY),
            capacity: MIN_SCENE_BUFFER_CAPACITY,
            label,
            usage,
        }
    }

    pub fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        values: &[T],
    ) {
        let bytes = bytemuck::cast_slice(values);
        if bytes.is_empty() {
            return;
        }
        let required = bytes.len() as wgpu::BufferAddress;
        if required > self.capacity {
            self.capacity = scene_buffer_capacity(required);
            self.buffer = create_scene_buffer(device, self.label, self.usage, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }

    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

pub fn create_atom_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("atom-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("atom-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("atom_vs"),
            buffers: &[atom_quad_layout(), atom_instance_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("atom_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
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
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_bond_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bond-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bond-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("bond_vs"),
            buffers: &[atom_quad_layout(), bond_instance_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("bond_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
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
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_mesh_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mesh-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mesh-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("mesh_vs"),
            buffers: &[mesh_vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("mesh_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
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
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn atom_quad_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATOM_QUAD_ATTRS,
    }
}

pub fn atom_instance_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<AtomInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATOM_INSTANCE_ATTRS,
    }
}

pub fn bond_instance_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<BondInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &BOND_INSTANCE_ATTRS,
    }
}

pub fn mesh_vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeshGpuVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_VERTEX_ATTRS,
    }
}

pub fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub const MIN_SCENE_BUFFER_CAPACITY: wgpu::BufferAddress = 256;

pub fn snapshot_upload_required(uploaded: Option<u64>, current: u64) -> bool {
    uploaded != Some(current)
}

pub fn scene_buffer_capacity(required: wgpu::BufferAddress) -> wgpu::BufferAddress {
    required
        .max(MIN_SCENE_BUFFER_CAPACITY)
        .checked_next_power_of_two()
        .unwrap_or(required)
}

pub fn create_scene_buffer(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
    capacity: wgpu::BufferAddress,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity.max(MIN_SCENE_BUFFER_CAPACITY),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn atom_instance(atom: &RenderAtom) -> AtomInstance {
    let color = atom_color(atom);
    let edge = edge_gpu_style(atom.edge);
    AtomInstance {
        center_radius: [
            atom.center.x as f32,
            atom.center.y as f32,
            atom.center.z as f32,
            atom.radius.max(0.08),
        ],
        color,
        selected: [if atom.selected { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        edge: [edge.0, edge.1, 0.0, 0.0],
        edge_color: edge.2,
    }
}

pub fn bond_instance(bond: &RenderBond) -> BondInstance {
    let color = bond_color(bond);
    let end_color = if bond.selected {
        color
    } else {
        bond.end_color
            .as_ref()
            .and_then(cuemol_gfx::ColorSpec::rgba8)
            .map_or(color, |rgba| rgba_to_float(Some(rgba)))
    };
    let edge = edge_gpu_style(bond.edge);
    BondInstance {
        from_radius: [
            bond.from.x as f32,
            bond.from.y as f32,
            bond.from.z as f32,
            bond.radius.max(0.0025),
        ],
        to_edge: [bond.to.x as f32, bond.to.y as f32, bond.to.z as f32, edge.0],
        color,
        end_color,
        edge_color: edge.2,
        edge_mode: [edge.1, 0.0, bond.screen_width.unwrap_or(0.0), 0.0],
    }
}

pub fn edge_gpu_style(edge: Option<cuemol_render::RenderEdgeStyle>) -> (f32, f32, [f32; 4]) {
    edge.filter(|edge| edge.width > 0.0)
        .map_or((0.0, 0.0, [0.0, 0.0, 0.0, 1.0]), |edge| {
            (
                edge.width,
                match edge.mode {
                    cuemol_render::RenderEdgeMode::Edges => 1.0,
                    cuemol_render::RenderEdgeMode::Silhouette => 2.0,
                },
                edge.color,
            )
        })
}

pub fn mesh_gpu_vertex(vertex: &MeshVertex) -> MeshGpuVertex {
    MeshGpuVertex {
        position: [
            vertex.position.x as f32,
            vertex.position.y as f32,
            vertex.position.z as f32,
        ],
        normal: [
            vertex.normal.x as f32,
            vertex.normal.y as f32,
            vertex.normal.z as f32,
        ],
        color: vertex.color,
        cull_back_faces: vertex.normal.w as f32,
    }
}

pub fn atom_color(atom: &RenderAtom) -> [f32; 4] {
    let mut color = rgba_to_float(atom.color.as_ref().and_then(cuemol_gfx::ColorSpec::rgba8));
    if atom.selected {
        color[0] = (color[0] * 0.4 + 0.0 * 0.6).min(1.0);
        color[1] = (color[1] * 0.4 + 0.95 * 0.6).min(1.0);
        color[2] = (color[2] * 0.4 + 1.0 * 0.6).min(1.0);
    }
    color
}

pub fn bond_color(bond: &RenderBond) -> [f32; 4] {
    if bond.selected {
        [0.0, 0.90, 1.0, 1.0]
    } else {
        rgba_to_float(bond.color.as_ref().and_then(cuemol_gfx::ColorSpec::rgba8))
    }
}

pub fn rgba_to_float(rgba: Option<Rgba8>) -> [f32; 4] {
    let rgba = rgba.unwrap_or(Rgba8 {
        r: 196,
        g: 201,
        b: 211,
        a: 255,
    });
    [
        f32::from(rgba.r) / 255.0,
        f32::from(rgba.g) / 255.0,
        f32::from(rgba.b) / 255.0,
        f32::from(rgba.a) / 255.0,
    ]
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub light: [f32; 4],
    pub pixel_scale: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub corner: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AtomInstance {
    pub center_radius: [f32; 4],
    pub color: [f32; 4],
    pub selected: [f32; 4],
    pub edge: [f32; 4],
    pub edge_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BondInstance {
    pub from_radius: [f32; 4],
    pub to_edge: [f32; 4],
    pub color: [f32; 4],
    pub end_color: [f32; 4],
    pub edge_color: [f32; 4],
    pub edge_mode: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshGpuVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub cull_back_faces: f32,
}

pub const ATOM_QUAD_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
pub const ATOM_INSTANCE_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x4
];
pub const BOND_INSTANCE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x4,
    6 => Float32x4
];
pub const MESH_VERTEX_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32];

pub const ATOM_QUAD: [QuadVertex; 6] = [
    QuadVertex {
        corner: [-1.0, -1.0],
    },
    QuadVertex {
        corner: [1.0, -1.0],
    },
    QuadVertex { corner: [1.0, 1.0] },
    QuadVertex {
        corner: [-1.0, -1.0],
    },
    QuadVertex { corner: [1.0, 1.0] },
    QuadVertex {
        corner: [-1.0, 1.0],
    },
];

pub const MOLECULE_SHADER: &str = include_str!("molecule.wgsl");

/// Persistent GPU buffers; scene geometry is uploaded only when it changes.
pub struct ViewportRenderer {
    atoms: ReusableGpuBuffer,
    bonds: ReusableGpuBuffer,
    vertices: ReusableGpuBuffer,
    indices: ReusableGpuBuffer,
    quad: wgpu::Buffer,
    camera: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    atom_pipeline: wgpu::RenderPipeline,
    bond_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    counts: [u32; 3],
}

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("molecule"),
            source: wgpu::ShaderSource::Wgsl(MOLECULE_SHADER.into()),
        });
        Self {
            atoms: ReusableGpuBuffer::new(device, "atoms", wgpu::BufferUsages::VERTEX),
            bonds: ReusableGpuBuffer::new(device, "bonds", wgpu::BufferUsages::VERTEX),
            vertices: ReusableGpuBuffer::new(device, "mesh-vertices", wgpu::BufferUsages::VERTEX),
            indices: ReusableGpuBuffer::new(device, "mesh-indices", wgpu::BufferUsages::INDEX),
            quad: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("billboard"),
                contents: bytemuck::cast_slice(&ATOM_QUAD),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            atom_pipeline: create_atom_pipeline(device, &shader, &layout, format),
            bond_pipeline: create_bond_pipeline(device, &shader, &layout, format),
            mesh_pipeline: create_mesh_pipeline(device, &shader, &layout, format),
            camera,
            bind_group,
            counts: [0; 3],
        }
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &RenderSceneSnapshot,
    ) {
        let atoms: Vec<_> = scene
            .atoms
            .iter()
            .filter(|a| a.hit.is_none())
            .map(atom_instance)
            .collect();
        let bonds: Vec<_> = scene.bonds.iter().map(bond_instance).collect();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for mesh in &scene.meshes {
            let base = vertices.len() as u32;
            vertices.extend(mesh.vertices.iter().map(mesh_gpu_vertex));
            indices.extend(mesh.indices.iter().map(|i| base + i));
        }
        self.counts = [atoms.len() as u32, bonds.len() as u32, indices.len() as u32];
        self.atoms.upload(device, queue, &atoms);
        self.bonds.upload(device, queue, &bonds);
        self.vertices.upload(device, queue, &vertices);
        self.indices.upload(device, queue, &indices);
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera: &cuemol_qsys::Camera,
        size: [u32; 2],
        background: wgpu::Color,
    ) {
        let q = camera.rotation();
        let rotation =
            glam::Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32).normalize();
        let right = rotation * glam::Vec3::X;
        let up = rotation * glam::Vec3::Y;
        queue.write_buffer(
            &self.camera,
            0,
            bytemuck::bytes_of(&CameraUniform {
                view_proj: crate::offscreen::camera_mvp(
                    camera,
                    size[0] as f64 / size[1].max(1) as f64,
                ),
                right: [right.x, right.y, right.z, 0.0],
                up: [up.x, up.y, up.z, 0.0],
                light: [0.25, 0.45, 0.85, 0.0],
                pixel_scale: [
                    camera.zoom() as f32 / size[1].max(1) as f32,
                    camera.cam_dist() as f32,
                    if camera.is_perspective() { 1.0 } else { 0.0 },
                    0.0,
                ],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("molecule-view"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        if self.counts[1] > 0 {
            pass.set_pipeline(&self.bond_pipeline);
            pass.set_vertex_buffer(0, self.quad.slice(..));
            pass.set_vertex_buffer(1, self.bonds.slice());
            pass.draw(0..6, 0..self.counts[1]);
        }
        if self.counts[0] > 0 {
            pass.set_pipeline(&self.atom_pipeline);
            pass.set_vertex_buffer(0, self.quad.slice(..));
            pass.set_vertex_buffer(1, self.atoms.slice());
            pass.draw(0..6, 0..self.counts[0]);
        }
        if self.counts[2] > 0 {
            pass.set_pipeline(&self.mesh_pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice());
            pass.set_index_buffer(self.indices.slice(), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.counts[2], 0, 0..1);
        }
    }
}
