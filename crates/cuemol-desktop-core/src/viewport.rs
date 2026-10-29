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

    pub fn upload_iter<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: usize,
        values: impl IntoIterator<Item = T>,
    ) {
        if count == 0 {
            return;
        }
        let required = count
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|bytes| wgpu::BufferSize::new(bytes as u64))
            .expect("invalid GPU upload size");
        if required.get() > self.capacity {
            self.capacity = scene_buffer_capacity(required.get());
            self.buffer = create_scene_buffer(device, self.label, self.usage, self.capacity);
        }
        let mut staging = queue
            .write_buffer_with(&self.buffer, 0, required)
            .expect("GPU staging allocation failed");
        let mut values = values.into_iter();
        for destination in staging.chunks_exact_mut(std::mem::size_of::<T>()) {
            let value = values.next().expect("GPU upload has too few values");
            destination.copy_from_slice(bytemuck::bytes_of(&value));
        }
        assert!(values.next().is_none(), "GPU upload has too many values");
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
    create_atom_pipeline_with_normals(device, shader, camera_layout, format, false)
}

fn create_atom_pipeline_with_normals(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    normals: bool,
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
            entry_point: Some(if normals {
                "atom_gbuffer_fs"
            } else {
                "atom_fs"
            }),
            targets: &color_targets(format, normals),
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
    create_bond_pipeline_with_normals(device, shader, camera_layout, format, false)
}

fn create_bond_pipeline_with_normals(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    normals: bool,
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
            entry_point: Some(if normals {
                "bond_gbuffer_fs"
            } else {
                "bond_fs"
            }),
            targets: &color_targets(format, normals),
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
    create_mesh_pipeline_with_normals(device, shader, camera_layout, format, false)
}

fn create_mesh_pipeline_with_normals(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    normals: bool,
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
            entry_point: Some(if normals {
                "mesh_gbuffer_fs"
            } else {
                "mesh_fs"
            }),
            targets: &color_targets(format, normals),
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

fn color_targets(
    format: wgpu::TextureFormat,
    normals: bool,
) -> Vec<Option<wgpu::ColorTargetState>> {
    let mut targets = vec![Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    if normals {
        // G-buffer outputs have alpha one and share the WebGL2 blend state.
        for format in [
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba8Unorm,
        ] {
            targets.push(Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            }));
        }
    }
    targets
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
        edge_mode: [
            edge.1,
            f32::from(bond.screen_stipple),
            bond.screen_width.unwrap_or(0.0),
            f32::from(bond.screen_point),
        ],
        screen: [bond.screen_offset[0], bond.screen_offset[1], 0.0, 0.0],
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
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub eye: [f32; 4],
    pub background: [f32; 4],
    pub fog: [f32; 4],
    pub pixel_scale: [f32; 4],
    pub viewport: [f32; 4],
}

impl CameraUniform {
    pub fn from_mvp(
        view_proj: [[f32; 4]; 4],
        size: [u32; 2],
        background: wgpu::Color,
    ) -> Option<Self> {
        let matrix = glam::Mat4::from_cols_array_2d(&view_proj);
        let inverse = matrix.inverse();
        if !inverse.is_finite() {
            return None;
        }
        let unproject = |x, y, z| {
            let p = inverse * glam::Vec4::new(x, y, z, 1.);
            p.truncate() / p.w
        };
        let near = unproject(0., 0., 0.);
        let far = unproject(0., 0., 1.);
        let center = unproject(0., 0., 0.5);
        let right = (unproject(1., 0., 0.) - near).normalize_or_zero();
        let mut up = (unproject(0., 1., 0.) - near).normalize_or_zero();
        if right.cross(up).dot(near - far) < 0. {
            up = -up;
        }
        let front = right.cross(up).normalize_or_zero();
        if !front.is_finite() || front.length_squared() == 0. {
            return None;
        }
        let depth_row = glam::Vec3::new(view_proj[0][3], view_proj[1][3], view_proj[2][3]);
        let scale = depth_row.length();
        let perspective = scale > 0.;
        let eye = if perspective {
            near + front * ((depth_row.dot(near) + view_proj[3][3]) / scale)
        } else {
            near + front * (near - far).length().max(1.)
        };
        let zoom = (unproject(0., 1., 0.5) - center).length() * 2.;
        Some(Self {
            view_proj,
            right: right.extend(0.).to_array(),
            up: up.extend(0.).to_array(),
            eye: eye.extend(0.).to_array(),
            background: [
                background.r as f32,
                background.g as f32,
                background.b as f32,
                background.a as f32,
            ],
            fog: [f32::MAX, 1., 0., 0.],
            pixel_scale: [
                zoom / size[1].max(1) as f32,
                (eye - center).dot(front),
                f32::from(perspective),
                0.,
            ],
            viewport: [size[0].max(1) as f32, size[1].max(1) as f32, 0., 0.],
        })
    }

    pub fn new(
        camera: &cuemol_qsys::Camera,
        size: [u32; 2],
        background: wgpu::Color,
        output_srgb: bool,
    ) -> Self {
        let q = camera.rotation();
        let rotation =
            glam::Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32).normalize();
        let right = rotation * glam::Vec3::X;
        let up = rotation * glam::Vec3::Y;
        let front = right.cross(up);
        let center = camera.center();
        let eye = glam::Vec3::new(center.x as f32, center.y as f32, center.z as f32)
            + front * camera.cam_dist() as f32;
        let fog_start = camera.cam_dist().max(1.0);
        let fog_end = camera.cam_dist() + camera.slab_depth() * 0.5;
        Self {
            view_proj: crate::offscreen::camera_mvp(
                camera,
                size[0].max(1) as f64 / size[1].max(1) as f64,
            ),
            right: [right.x, right.y, right.z, 0.0],
            up: [up.x, up.y, up.z, 0.0],
            eye: [eye.x, eye.y, eye.z, 0.0],
            background: [
                background.r as f32,
                background.g as f32,
                background.b as f32,
                background.a as f32,
            ],
            fog: [
                fog_end as f32,
                (1.0 / (fog_end - fog_start).max(1.0e-6)) as f32,
                0.0,
                0.0,
            ],
            viewport: [size[0].max(1) as f32, size[1].max(1) as f32, 0.0, 0.0],
            pixel_scale: [
                camera.zoom() as f32 / size[1].max(1) as f32,
                camera.cam_dist() as f32,
                f32::from(camera.is_perspective()),
                f32::from(output_srgb),
            ],
        }
    }
}

pub fn target_clear_color(mut color: wgpu::Color, output_srgb: bool) -> wgpu::Color {
    if output_srgb {
        let linear = |c: f64| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        color.r = linear(color.r);
        color.g = linear(color.g);
        color.b = linear(color.b);
    }
    color
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
    pub screen: [f32; 4],
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
pub const BOND_INSTANCE_ATTRS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x4,
    6 => Float32x4,
    7 => Float32x4
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

pub const MOLECULE_SHADER: &str = concat!(
    include_str!("appearance.wgsl"),
    "\n",
    include_str!("molecule.wgsl")
);

#[derive(Default)]
struct PrimitiveDraw {
    atoms: std::ops::Range<u32>,
    bonds: std::ops::Range<u32>,
    indices: std::ops::Range<u32>,
    transparent: bool,
}

fn primitive_draws(scene: &RenderSceneSnapshot) -> Vec<PrimitiveDraw> {
    let prefix = |counts: Vec<u32>| {
        let mut result = Vec::with_capacity(counts.len() + 1);
        result.push(0_u32);
        for count in counts {
            result.push(result.last().copied().unwrap() + count);
        }
        result
    };
    let offsets = [
        prefix(
            scene
                .atoms
                .iter()
                .map(|atom| u32::from(atom.hit.is_none()))
                .collect(),
        ),
        prefix(vec![1; scene.bonds.len()]),
        prefix(
            scene
                .meshes
                .iter()
                .map(|mesh| mesh.indices.len() as u32)
                .collect(),
        ),
    ];
    if scene.transparency_groups.is_empty() {
        return vec![PrimitiveDraw {
            atoms: 0..*offsets[0].last().unwrap(),
            bonds: 0..*offsets[1].last().unwrap(),
            indices: 0..*offsets[2].last().unwrap(),
            transparent: false,
        }];
    }
    let mut covered = offsets
        .each_ref()
        .map(|values| vec![false; values.len() - 1]);
    let mut draws = Vec::new();
    for group in &scene.transparency_groups {
        let ranges = [&group.atom_range, &group.bond_range, &group.mesh_range];
        let mut output = [0..0, 0..0, 0..0];
        for kind in 0..3 {
            let end = ranges[kind].end.min(covered[kind].len());
            let start = ranges[kind].start.min(end);
            covered[kind][start..end].fill(true);
            output[kind] = offsets[kind][start]..offsets[kind][end];
        }
        let [atoms, bonds, indices] = output;
        draws.push(PrimitiveDraw {
            atoms,
            bonds,
            indices,
            transparent: group.alpha < 1.0,
        });
    }
    for kind in 0..3 {
        let mut start = 0;
        while start < covered[kind].len() {
            if covered[kind][start] {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < covered[kind].len() && !covered[kind][end] {
                end += 1;
            }
            let range = offsets[kind][start]..offsets[kind][end];
            let mut draw = PrimitiveDraw::default();
            match kind {
                0 => draw.atoms = range,
                1 => draw.bonds = range,
                _ => draw.indices = range,
            }
            draws.push(draw);
            start = end;
        }
    }
    draws.sort_by_key(|draw| draw.transparent);
    draws
}

/// Persistent GPU buffers; scene geometry is uploaded only when it changes.
pub struct ViewportRenderer {
    atoms: ReusableGpuBuffer,
    bonds: ReusableGpuBuffer,
    vertices: ReusableGpuBuffer,
    indices: ReusableGpuBuffer,
    quad: wgpu::Buffer,
    camera: [wgpu::Buffer; 2],
    bind_group: [wgpu::BindGroup; 2],
    atom_pipeline: wgpu::RenderPipeline,
    bond_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    normal_atom_pipeline: wgpu::RenderPipeline,
    normal_bond_pipeline: wgpu::RenderPipeline,
    normal_mesh_pipeline: wgpu::RenderPipeline,
    volumes: crate::volumes::VolumeRenderer,
    revision: u64,
    draws: Vec<PrimitiveDraw>,
    output_srgb: bool,
}

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("camera"),
                size: std::mem::size_of::<CameraUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
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
        let bind_group = std::array::from_fn(|eye| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("camera-bind-group"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera[eye].as_entire_binding(),
                }],
            })
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
            atom_pipeline: create_atom_pipeline_with_normals(
                device, &shader, &layout, format, false,
            ),
            normal_atom_pipeline: create_atom_pipeline_with_normals(
                device,
                &shader,
                &layout,
                wgpu::TextureFormat::Rgba8Unorm,
                true,
            ),
            bond_pipeline: create_bond_pipeline_with_normals(
                device, &shader, &layout, format, false,
            ),
            normal_bond_pipeline: create_bond_pipeline_with_normals(
                device,
                &shader,
                &layout,
                wgpu::TextureFormat::Rgba8Unorm,
                true,
            ),
            mesh_pipeline: create_mesh_pipeline_with_normals(
                device, &shader, &layout, format, false,
            ),
            normal_mesh_pipeline: create_mesh_pipeline_with_normals(
                device,
                &shader,
                &layout,
                wgpu::TextureFormat::Rgba8Unorm,
                true,
            ),
            volumes: crate::volumes::VolumeRenderer::new(device, &layout, format),
            camera,
            bind_group,
            draws: Vec::new(),
            revision: 0,
            output_srgb: format.is_srgb(),
        }
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &RenderSceneSnapshot,
    ) {
        self.draws = primitive_draws(scene);
        self.revision = self.revision.wrapping_add(1);
        self.atoms.upload_iter(
            device,
            queue,
            scene.atoms.iter().filter(|atom| atom.hit.is_none()).count(),
            scene
                .atoms
                .iter()
                .filter(|atom| atom.hit.is_none())
                .map(atom_instance),
        );
        self.bonds.upload_iter(
            device,
            queue,
            scene.bonds.len(),
            scene.bonds.iter().map(bond_instance),
        );
        self.vertices.upload_iter(
            device,
            queue,
            scene.meshes.iter().map(|mesh| mesh.vertices.len()).sum(),
            scene
                .meshes
                .iter()
                .flat_map(|mesh| mesh.vertices.iter().map(mesh_gpu_vertex)),
        );
        let mut base = 0_u32;
        self.indices.upload_iter(
            device,
            queue,
            scene.meshes.iter().map(|mesh| mesh.indices.len()).sum(),
            scene.meshes.iter().flat_map(|mesh| {
                let offset = base;
                base += mesh.vertices.len() as u32;
                mesh.indices.iter().map(move |index| offset + index)
            }),
        );
        self.volumes.upload(device, queue, &scene.volumes);
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
        self.render_with_uniform(
            encoder,
            queue,
            target,
            depth,
            &CameraUniform::new(camera, size, background, self.output_srgb),
            background,
            None,
        );
    }

    pub fn render_region(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera: &cuemol_qsys::Camera,
        size: [u32; 2],
        background: wgpu::Color,
        region: [f32; 4],
    ) {
        self.render_with_uniform(
            encoder,
            queue,
            target,
            depth,
            &CameraUniform::new(camera, size, background, self.output_srgb),
            background,
            Some(region),
        );
    }

    pub(crate) fn render_with_uniform(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        uniform: &CameraUniform,
        background: wgpu::Color,
        region: Option<[f32; 4]>,
    ) {
        self.render_attachments(
            encoder, queue, target, depth, uniform, background, region, None, 0,
        );
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_volumes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        uniform: &CameraUniform,
        region: Option<[f32; 4]>,
        linear: bool,
        eye: usize,
    ) {
        self.volumes.render(
            encoder,
            queue,
            target,
            depth,
            &self.bind_group[eye],
            uniform,
            region,
            linear,
            !linear && self.output_srgb,
            eye,
        );
    }

    pub(crate) fn render_gbuffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        normal: &wgpu::TextureView,
        encoded_depth: &wgpu::TextureView,
        uniform: &CameraUniform,
        background: wgpu::Color,
        eye: usize,
    ) {
        self.render_attachments(
            encoder,
            queue,
            target,
            depth,
            uniform,
            background,
            None,
            Some((normal, encoded_depth)),
            eye,
        );
    }

    fn render_attachments(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        uniform: &CameraUniform,
        background: wgpu::Color,
        region: Option<[f32; 4]>,
        normal: Option<(&wgpu::TextureView, &wgpu::TextureView)>,
        eye: usize,
    ) {
        queue.write_buffer(&self.camera[eye], 0, bytemuck::bytes_of(uniform));
        let output_srgb = self.output_srgb && normal.is_none();
        let mut attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(target_clear_color(background, output_srgb)),
                store: wgpu::StoreOp::Store,
            },
        })];
        if let Some((normal, encoded_depth)) = normal {
            for (view, clear) in [
                (normal, wgpu::Color::TRANSPARENT),
                (encoded_depth, wgpu::Color::WHITE),
            ] {
                attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                }));
            }
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("molecule-view"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some([x, y, width, height]) = region {
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
        }
        pass.set_bind_group(0, &self.bind_group[eye], &[]);
        for draw in &self.draws {
            if !draw.bonds.is_empty() {
                pass.set_pipeline(if normal.is_some() {
                    &self.normal_bond_pipeline
                } else {
                    &self.bond_pipeline
                });
                pass.set_vertex_buffer(0, self.quad.slice(..));
                pass.set_vertex_buffer(1, self.bonds.slice());
                pass.draw(0..6, draw.bonds.clone());
            }
            if !draw.atoms.is_empty() {
                pass.set_pipeline(if normal.is_some() {
                    &self.normal_atom_pipeline
                } else {
                    &self.atom_pipeline
                });
                pass.set_vertex_buffer(0, self.quad.slice(..));
                pass.set_vertex_buffer(1, self.atoms.slice());
                pass.draw(0..6, draw.atoms.clone());
            }
            if !draw.indices.is_empty() {
                pass.set_pipeline(if normal.is_some() {
                    &self.normal_mesh_pipeline
                } else {
                    &self.mesh_pipeline
                });
                pass.set_vertex_buffer(0, self.vertices.slice());
                pass.set_index_buffer(self.indices.slice(), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(draw.indices.clone(), 0, 0..1);
            }
        }
        drop(pass);
        if normal.is_none() {
            self.render_volumes(encoder, queue, target, depth, uniform, region, false, eye);
        }
    }
}
