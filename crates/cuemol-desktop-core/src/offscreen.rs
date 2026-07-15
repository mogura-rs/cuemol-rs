//! Offscreen wgpu renderer: rasterize [`RenderMesh`] triangles to an in-memory
//! RGBA image, with NO window/surface. This is verifiable headless (render a
//! known mesh, read the pixels back), unlike on-screen drawing — and it is the
//! wgpu 3D component the gpui GUI will composite (gpui panels + a wgpu-rendered
//! 3D image, per the user's direction). wgpu compiles shaders at runtime
//! (naga -> Metal), so this needs no Xcode, only the Command Line Tools.

use wgpu::util::DeviceExt;

use cuemol_core::Matrix4;
use cuemol_qsys::Camera;
use cuemol_render::{tessellate_snapshot, RenderMesh, RenderSceneSnapshot};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 4],
    normal: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DepthParams {
    near: f32,
    far: f32,
    perspective: f32,
    _padding: f32,
}

// Flat color in the vertex's world normal, two-sided diffuse (Lambert) shading
// against a fixed light so curved surfaces (spheres/cylinders) get a brightness
// gradient — `abs(dot)` keeps back faces lit (no black interiors) and leaves
// camera-facing flats fully lit.
const MESH_SHADER: &str = r#"
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};
@group(0) @binding(0) var<uniform> mvp: mat4x4<f32>;
struct DepthParams {
    near: f32,
    far: f32,
    perspective: f32,
    padding: f32,
};
@group(0) @binding(1) var<uniform> depth_params: DepthParams;
@vertex
fn vs(@location(0) position: vec4<f32>, @location(1) normal: vec4<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.clip = mvp * position;
    out.color = color;
    out.normal = normal.xyz;
    return out;
}
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.0, 0.0, 1.0));
    let n = normalize(in.normal);
    let lambert = abs(dot(n, light));
    let intensity = 0.4 + 0.6 * lambert;
    return vec4<f32>(in.color.rgb * intensity, in.color.a);
}
@fragment
fn fs_depth(in: VsOut) -> @location(0) vec4<f32> {
    let d = in.clip.z;
    var linear_depth = depth_params.near + d * (depth_params.far - depth_params.near);
    if depth_params.perspective > 0.5 {
        linear_depth = (depth_params.near * depth_params.far) /
            (depth_params.far - d * (depth_params.far - depth_params.near));
    }
    let normalized = clamp(
        (linear_depth - depth_params.near) / (depth_params.far - depth_params.near),
        0.0,
        1.0,
    );
    let gray = 1.0 - normalized;
    return vec4<f32>(gray, gray, gray, 1.0);
}
"#;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Render `meshes` to a `width` x `height` RGBA8 image (4 bytes/pixel, row-major,
/// top-down). `mvp` is the column-major model-view-projection matrix WGSL
/// expects; `clear` is the background colour. Returns `None` when no GPU adapter
/// is available (e.g. a headless CI box), so the caller/test self-skips.
pub fn render_meshes_offscreen(
    meshes: &[RenderMesh],
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    clear: [f64; 4],
) -> Option<Vec<u8>> {
    pollster::block_on(render_impl(
        meshes,
        mvp,
        width.max(1),
        height.max(1),
        clear,
        None,
    ))
}

/// Render a linearized grayscale depth image. Near geometry is white, far
/// geometry and the background are black, matching CueMol's depth export.
pub fn render_meshes_depth_offscreen(
    meshes: &[RenderMesh],
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    near: f64,
    far: f64,
    perspective: bool,
) -> Option<Vec<u8>> {
    pollster::block_on(render_impl(
        meshes,
        mvp,
        width.max(1),
        height.max(1),
        [0.0, 0.0, 0.0, 1.0],
        Some(DepthParams {
            near: near as f32,
            far: far as f32,
            perspective: f32::from(perspective),
            _padding: 0.0,
        }),
    ))
}

/// A persistent wgpu context (device + queue + render pipeline) that an
/// interactive viewer reuses across frames.
///
/// The free `render_*` functions above build a throwaway device and recompile
/// the shader on every call — fine for a one-shot PNG export, but ruinous when a
/// mouse drag re-renders dozens of times a second (device creation + runtime
/// shader compilation dominate). `SceneRenderer` pays that cost once in
/// [`SceneRenderer::new`]; each [`SceneRenderer::render`] only does the per-frame
/// work (vertex upload, MVP uniform, textures, readback).
pub struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    depth_pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
}

impl SceneRenderer {
    /// Build the GPU device + pipeline once. Returns `None` when no GPU adapter
    /// is available (headless CI), so callers self-skip exactly like the free
    /// functions.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Option<Self> {
        let (device, queue) = create_device().await?;
        let (pipeline, depth_pipeline, bind_layout) = build_pipeline(&device);
        Some(Self {
            device,
            queue,
            pipeline,
            depth_pipeline,
            bind_layout,
        })
    }

    /// Render `meshes` to a `width`x`height` RGBA8 image (same output as
    /// [`render_meshes_offscreen`]) reusing the persistent device/pipeline.
    pub fn render(
        &self,
        meshes: &[RenderMesh],
        mvp: [[f32; 4]; 4],
        width: u32,
        height: u32,
        clear: [f64; 4],
    ) -> Option<Vec<u8>> {
        draw_meshes(
            &self.device,
            &self.queue,
            &self.pipeline,
            &self.bind_layout,
            meshes,
            mvp,
            width,
            height,
            clear,
            None,
        )
    }

    /// Tessellate + render a whole scene snapshot (same output as
    /// [`render_snapshot_offscreen`]) reusing the persistent device/pipeline.
    pub fn render_snapshot(
        &self,
        snapshot: &RenderSceneSnapshot,
        mvp: [[f32; 4]; 4],
        width: u32,
        height: u32,
        clear: [f64; 4],
    ) -> Option<Vec<u8>> {
        let mesh = tessellate_snapshot(snapshot);
        self.render(&[mesh], mvp, width, height, clear)
    }

    /// Render meshes as a linearized grayscale depth image while reusing the
    /// persistent device and depth pipeline.
    #[allow(clippy::too_many_arguments)]
    pub fn render_depth(
        &self,
        meshes: &[RenderMesh],
        mvp: [[f32; 4]; 4],
        width: u32,
        height: u32,
        near: f64,
        far: f64,
        perspective: bool,
    ) -> Option<Vec<u8>> {
        draw_meshes(
            &self.device,
            &self.queue,
            &self.depth_pipeline,
            &self.bind_layout,
            meshes,
            mvp,
            width,
            height,
            [0.0, 0.0, 0.0, 1.0],
            Some(DepthParams {
                near: near as f32,
                far: far as f32,
                perspective: if perspective { 1.0 } else { 0.0 },
                _padding: 0.0,
            }),
        )
    }
}

async fn render_impl(
    meshes: &[RenderMesh],
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    clear: [f64; 4],
    depth_params: Option<DepthParams>,
) -> Option<Vec<u8>> {
    // The free-function path builds a throwaway device + pipeline each call
    // (fine for one-shot exports); `SceneRenderer` reuses them across frames.
    let (device, queue) = create_device().await?;
    let (pipeline, depth_pipeline, bind_layout) = build_pipeline(&device);
    let selected_pipeline = depth_params.map(|_| &depth_pipeline).unwrap_or(&pipeline);
    draw_meshes(
        &device,
        &queue,
        selected_pipeline,
        &bind_layout,
        meshes,
        mvp,
        width,
        height,
        clear,
        depth_params,
    )
}

/// Request a wgpu device + queue. Returns `None` when no GPU adapter is present
/// (e.g. a headless CI box), so callers/tests self-skip.
async fn create_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok()?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("offscreen-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .ok()?;
    Some((device, queue))
}

/// Build the mesh render pipeline + its MVP bind-group layout. The pipeline is
/// format/shader-fixed, so it can be built once and reused across frames.
fn build_pipeline(
    device: &wgpu::Device,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
) {
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("offscreen-mvp-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
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

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("offscreen-shader"),
        source: wgpu::ShaderSource::Wgsl(MESH_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("offscreen-pipeline-layout"),
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });
    let make_pipeline = |label, fragment_entry| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fragment_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR_FORMAT,
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
    };
    let pipeline = make_pipeline("offscreen-pipeline", "fs");
    let depth_pipeline = make_pipeline("offscreen-depth-pipeline", "fs_depth");
    (pipeline, depth_pipeline, bind_layout)
}

/// Rasterize `meshes` into a `width`x`height` RGBA8 image using a pre-built
/// `device`/`queue`/`pipeline`. The per-call work (vertex upload, MVP uniform,
/// color/depth textures, readback) is the only thing an interactive viewer must
/// repeat per frame — the device + pipeline are reused across calls.
#[allow(clippy::too_many_arguments)]
fn draw_meshes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    bind_layout: &wgpu::BindGroupLayout,
    meshes: &[RenderMesh],
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    clear: [f64; 4],
    depth_params: Option<DepthParams>,
) -> Option<Vec<u8>> {
    let width = width.max(1);
    let height = height.max(1);

    // Flatten the meshes into one vertex/index buffer.
    let mut vertices: Vec<GpuVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for mesh in meshes {
        let base = vertices.len() as u32;
        for v in &mesh.vertices {
            vertices.push(GpuVertex {
                position: [
                    v.position.x as f32,
                    v.position.y as f32,
                    v.position.z as f32,
                    1.0,
                ],
                normal: [v.normal.x as f32, v.normal.y as f32, v.normal.z as f32, 0.0],
                color: v.color,
            });
        }
        indices.extend(mesh.indices.iter().map(|i| i + base));
    }
    // A degenerate placeholder keeps the buffers non-empty (and draws nothing).
    if vertices.is_empty() {
        vertices.push(GpuVertex {
            position: [0.0, 0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        });
    }

    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen-indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let mvp_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen-mvp"),
        contents: bytemuck::cast_slice(&mvp),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let depth_params = depth_params.unwrap_or(DepthParams {
        near: 0.1,
        far: 1.0,
        perspective: 0.0,
        _padding: 0.0,
    });
    let depth_params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen-depth-params"),
        contents: bytemuck::bytes_of(&depth_params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("offscreen-mvp-bind"),
        layout: bind_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: mvp_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: depth_params_buf.as_entire_binding(),
            },
        ],
    });

    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen-color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen-depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    // Readback buffer: bytes-per-row must be 256-aligned.
    let unpadded = width * 4;
    let padded = unpadded.div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen-readback"),
        size: (padded * height) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("offscreen-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear[0],
                        g: clear[1],
                        b: clear[2],
                        a: clear[3],
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if !indices.is_empty() {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vbuf.slice(..));
            pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = slice.get_mapped_range();

    // Strip the row padding into a tight RGBA buffer.
    let mut out = Vec::with_capacity((unpadded * height) as usize);
    for row in 0..height {
        let start = (row * padded) as usize;
        out.extend_from_slice(&mapped[start..start + unpadded as usize]);
    }
    drop(mapped);
    readback.unmap();
    Some(out)
}

/// A row-major translation matrix.
fn translation(tx: f64, ty: f64, tz: f64) -> Matrix4 {
    let mut m = Matrix4 {
        values: [[0.0; 4]; 4],
    };
    for i in 0..4 {
        m.values[i][i] = 1.0;
    }
    m.values[0][3] = tx;
    m.values[1][3] = ty;
    m.values[2][3] = tz;
    m
}

/// Build the row-major projection matrix for a [`Camera`] — orthographic, or
/// perspective when the camera's `perspective` flag is set. The focal plane at
/// `cam_dist` has half-height `zoom/2` (width scaled by `aspect`) in both modes;
/// near/far bracket `cam_dist` by `slab_depth/2`, mapping to wgpu depth `[0, 1]`.
fn projection_matrix(camera: &Camera, aspect: f64) -> Matrix4 {
    let cam_dist = camera.cam_dist();
    let half_h = (camera.zoom() / 2.0).max(1.0e-6);
    let half_w = (half_h * aspect).max(1.0e-6);
    let slab = camera.slab_depth().max(1.0e-3);
    let near_dist = (cam_dist - slab / 2.0).max(1.0e-3);
    let far_dist = cam_dist + slab / 2.0;
    let mut p = Matrix4 {
        values: [[0.0; 4]; 4],
    };
    if camera.is_perspective() {
        let f = cam_dist / half_h; // 1 / tan(half_fov_y)
        p.values[0][0] = f / aspect;
        p.values[1][1] = f;
        p.values[2][2] = far_dist / (near_dist - far_dist);
        p.values[2][3] = near_dist * far_dist / (near_dist - far_dist);
        p.values[3][2] = -1.0; // perspective divide by -z_view
    } else {
        p.values[0][0] = 1.0 / half_w;
        p.values[1][1] = 1.0 / half_h;
        p.values[2][2] = -1.0 / slab;
        p.values[2][3] = -near_dist / slab;
        p.values[3][3] = 1.0;
    }
    p
}

/// Build the column-major MVP matrix WGSL expects from a scene [`Camera`].
///
/// Mirrors the CueMol camera model: view = `T(0,0,-cam_dist) · R(rotation) ·
/// T(-center)`, then the [`projection_matrix`] (orthographic or perspective).
pub fn camera_mvp(camera: &Camera, aspect: f64) -> [[f32; 4]; 4] {
    let center = camera.center();
    let rotation = camera.rotation().to_matrix();
    let cam_dist = camera.cam_dist();
    let view = translation(0.0, 0.0, -cam_dist)
        .mul_mat(rotation)
        .mul_mat(translation(-center.x, -center.y, -center.z));

    let mvp = projection_matrix(camera, aspect).mul_mat(view);
    // Row-major Matrix4 -> column-major array for WGSL (out[col][row]).
    let mut out = [[0.0f32; 4]; 4];
    for (col, out_col) in out.iter_mut().enumerate() {
        for (row, slot) in out_col.iter_mut().enumerate() {
            *slot = mvp.values[row][col] as f32;
        }
    }
    out
}

/// Near and far distances used by [`camera_mvp`].
pub fn camera_depth_range(camera: &Camera) -> (f64, f64) {
    let slab = camera.slab_depth().max(1.0e-3);
    (
        (camera.cam_dist() - slab / 2.0).max(1.0e-3),
        camera.cam_dist() + slab / 2.0,
    )
}

/// Render a whole molecular scene (atoms + bonds + meshes) to an offscreen RGBA8
/// image — the full 3D image the gpui GUI will composite. Returns `None` when no
/// GPU adapter is available.
pub fn render_snapshot_offscreen(
    snapshot: &RenderSceneSnapshot,
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    clear: [f64; 4],
) -> Option<Vec<u8>> {
    let mesh = tessellate_snapshot(snapshot);
    render_meshes_offscreen(&[mesh], mvp, width, height, clear)
}

/// Render a whole scene as a linearized grayscale depth image.
pub fn render_snapshot_depth_offscreen(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let aspect = width.max(1) as f64 / height.max(1) as f64;
    let mvp = camera_mvp(camera, aspect);
    let (near, far) = camera_depth_range(camera);
    let mesh = tessellate_snapshot(snapshot);
    render_meshes_depth_offscreen(
        &[mesh],
        mvp,
        width,
        height,
        near,
        far,
        camera.is_perspective(),
    )
}

/// Encode an RGBA8 image (`width*height*4` bytes, row-major) as PNG bytes.
pub fn encode_rgba_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    encode_png(rgba, width, height, true, 72.0)
}

/// Encode an RGBA8 render as RGB or RGBA PNG and store its physical resolution.
pub fn encode_png(
    rgba: &[u8],
    width: u32,
    height: u32,
    use_alpha: bool,
    resolution_dpi: f64,
) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(if use_alpha {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        encoder.set_depth(png::BitDepth::Eight);
        let pixels_per_meter = (resolution_dpi.max(0.0) * 39.370_078_740_157_48).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: pixels_per_meter,
            yppu: pixels_per_meter,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header().expect("write PNG header");
        if use_alpha {
            writer.write_image_data(rgba).expect("write PNG data");
        } else {
            let rgb = rgba
                .chunks_exact(4)
                .flat_map(|pixel| pixel[..3].iter().copied())
                .collect::<Vec<_>>();
            writer.write_image_data(&rgb).expect("write PNG data");
        }
    }
    out
}

/// Render a molecular scene straight to PNG bytes — a headless molecular image
/// (the offscreen 3D render, ready to write to a file). Returns `None` if no GPU
/// adapter is available.
pub fn render_snapshot_png(
    snapshot: &RenderSceneSnapshot,
    mvp: [[f32; 4]; 4],
    width: u32,
    height: u32,
    clear: [f64; 4],
) -> Option<Vec<u8>> {
    let rgba = render_snapshot_offscreen(snapshot, mvp, width, height, clear)?;
    Some(encode_rgba_png(&rgba, width, height))
}

/// Render a whole scene to a grayscale depth PNG.
pub fn render_snapshot_depth_png(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let rgba = render_snapshot_depth_offscreen(snapshot, camera, width, height)?;
    Some(encode_rgba_png(&rgba, width.max(1), height.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    use cuemol_gfx::{ColorBase, ColorSpec};
    use cuemol_render::{MeshVertex, RenderAtom, RenderBond};

    const IDENTITY: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    fn pixel(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    }

    fn vert(x: f32, y: f32, color: [f32; 4]) -> MeshVertex {
        MeshVertex {
            position: cuemol_core::Vector4::new3(x as f64, y as f64, 0.0),
            normal: cuemol_core::Vector4::new3(0.0, 0.0, 1.0),
            color,
        }
    }

    #[test]
    fn offscreen_render_rasterizes_a_known_triangle() {
        // A green triangle covering the centre over a black background. If no GPU
        // adapter exists (headless CI), self-skip — same policy as the
        // fixture-dependent compat tests.
        let green = [0.0, 1.0, 0.0, 1.0];
        let mesh = RenderMesh {
            vertices: vec![
                vert(-0.9, -0.9, green),
                vert(0.9, -0.9, green),
                vert(0.0, 0.9, green),
            ],
            indices: vec![0, 1, 2],
        };
        let Some(img) = render_meshes_offscreen(&[mesh], IDENTITY, 64, 64, [0.0, 0.0, 0.0, 1.0])
        else {
            eprintln!("no GPU adapter; skipping offscreen render test");
            return;
        };
        assert_eq!(img.len(), 64 * 64 * 4);
        // Centre pixel is inside the triangle -> green.
        let c = pixel(&img, 64, 32, 32);
        assert!(
            c[1] > 200 && c[0] < 60 && c[2] < 60,
            "centre not green: {c:?}"
        );
        // A top corner is outside the triangle -> black background.
        let corner = pixel(&img, 64, 1, 1);
        assert!(
            corner[0] < 30 && corner[1] < 30 && corner[2] < 30,
            "corner not background: {corner:?}"
        );
    }

    #[test]
    fn offscreen_render_of_no_meshes_is_the_clear_colour() {
        let Some(img) = render_meshes_offscreen(&[], IDENTITY, 16, 16, [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        // Every pixel is the blue clear colour.
        for y in 0..16 {
            for x in 0..16 {
                let p = pixel(&img, 16, x, y);
                assert!(
                    p[2] > 200 && p[0] < 60 && p[1] < 60,
                    "not clear blue: {p:?}"
                );
            }
        }
    }

    #[test]
    fn depth_render_is_white_near_geometry_on_black_background() {
        let mesh = RenderMesh {
            vertices: vec![
                vert(-0.9, -0.9, [1.0; 4]),
                vert(0.9, -0.9, [1.0; 4]),
                vert(0.0, 0.9, [1.0; 4]),
            ],
            indices: vec![0, 1, 2],
        };
        let Some(img) = render_meshes_depth_offscreen(&[mesh], IDENTITY, 64, 64, 0.1, 1.0, false)
        else {
            return;
        };
        let centre = pixel(&img, 64, 32, 32);
        assert!(centre[0] > 240 && centre[1] == centre[0] && centre[2] == centre[0]);
        let corner = pixel(&img, 64, 1, 1);
        assert!(corner[0] < 10 && corner[1] < 10 && corner[2] < 10);
    }

    #[test]
    fn png_encoder_honors_rgb_and_resolution_options() {
        let png_bytes = encode_png(&[10, 20, 30, 40], 1, 1, false, 144.0);
        let decoder = png::Decoder::new(&png_bytes[..]);
        let mut reader = decoder.read_info().expect("read PNG info");
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        let dims = reader.info().pixel_dims.expect("physical resolution");
        assert_eq!(dims.unit, png::Unit::Meter);
        assert!((dims.xppu as i64 - 5669).abs() <= 1);
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode PNG frame");
        assert_eq!(info.buffer_size(), 3);
        assert_eq!(&buf[..3], &[10, 20, 30]);
    }

    fn solid(r: f32, g: f32, b: f32) -> Option<ColorSpec> {
        Some(ColorSpec {
            base: ColorBase::Rgb { r, g, b, a: 1.0 },
            modifiers: Vec::new(),
        })
    }

    #[test]
    fn offscreen_renders_a_full_atom_bond_scene() {
        // Atoms become spheres, bonds cylinders. A red atom at the origin (radius
        // 0.8 in NDC) fills the centre; the corner stays background.
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(Vector4::new3(0.0, 0.0, 0.0), 0.8, solid(1.0, 0.0, 0.0)),
                RenderAtom::new(Vector4::new3(0.6, 0.0, 0.0), 0.3, solid(0.0, 0.0, 1.0)),
            ],
            bonds: vec![RenderBond::new(
                Vector4::new3(0.0, 0.0, 0.0),
                Vector4::new3(0.6, 0.0, 0.0),
                0.1,
                solid(0.5, 0.5, 0.5),
            )],
            meshes: Vec::new(),
            bounds: None,
        };
        // The tessellation itself is always checkable (no GPU needed).
        let mesh = tessellate_snapshot(&snapshot);
        assert!(mesh.vertices.len() > 100 && !mesh.indices.is_empty());

        // Orthographic MVP (column-major for WGSL): x,y pass through to NDC; z in
        // [-2,2] maps to depth [0,1] (z' = 0.25 z + 0.5), so the atom's front
        // surface (nearest depth) occludes the bond behind it.
        const ORTHO: [[f32; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.25, 0.0],
            [0.0, 0.0, 0.5, 1.0],
        ];
        let Some(img) = render_snapshot_offscreen(&snapshot, ORTHO, 64, 64, [0.0, 0.0, 0.0, 1.0])
        else {
            eprintln!("no GPU adapter; skipping scene render test");
            return;
        };
        let centre = pixel(&img, 64, 32, 32);
        assert!(
            centre[0] > 180 && centre[2] < 80,
            "scene centre not the red atom: {centre:?}"
        );
        let corner = pixel(&img, 64, 1, 1);
        assert!(
            corner[0] < 30 && corner[1] < 30 && corner[2] < 30,
            "corner not background: {corner:?}"
        );
    }

    fn project(mvp: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
        // Column-major matrix * column vector.
        let mut r = [0.0f32; 4];
        for i in 0..4 {
            for col in 0..4 {
                r[i] += mvp[col][i] * v[col];
            }
        }
        r
    }

    #[test]
    fn camera_mvp_projects_scene_geometry_into_ndc() {
        // GPU-free oracle: the scene Camera's MVP maps the look-at centre to the
        // NDC origin at mid-depth, and a point `zoom/2` off-centre to the screen
        // edge. Default camera: centre origin, cam_dist 200, zoom 50, slab 50.
        let camera = Camera::new();
        let mvp = camera_mvp(&camera, 1.0);

        let origin = project(mvp, [0.0, 0.0, 0.0, 1.0]);
        assert!(
            origin[0].abs() < 1.0e-4 && origin[1].abs() < 1.0e-4,
            "centre not at NDC origin: {origin:?}"
        );
        assert!(
            (origin[2] - 0.5).abs() < 1.0e-4,
            "centre depth not mid-slab: {origin:?}"
        );
        // World (zoom/2, 0, 0) -> horizontal screen edge; (0, zoom/2, 0) ->
        // vertical edge. (The y SIGN follows the default camera rotation's
        // up-convention; the magnitude is what the projection fixes.)
        let right = project(mvp, [25.0, 0.0, 0.0, 1.0]);
        assert!(
            (right[0].abs() - 1.0).abs() < 1.0e-3,
            "x extent wrong: {right:?}"
        );
        let top = project(mvp, [0.0, 25.0, 0.0, 1.0]);
        assert!(
            (top[1].abs() - 1.0).abs() < 1.0e-3,
            "y extent wrong: {top:?}"
        );
    }

    #[test]
    fn camera_mvp_drives_an_offscreen_scene_render() {
        // The camera MVP feeds the offscreen renderer end-to-end: an atom at the
        // look-at centre lands at the image centre.
        let mut camera = Camera::new();
        camera.set_zoom(10.0); // tighten the view so the atom fills more pixels
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(
                Vector4::new3(0.0, 0.0, 0.0),
                2.0,
                solid(0.0, 1.0, 0.0),
            )],
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        };
        let mvp = camera_mvp(&camera, 1.0);
        let Some(img) = render_snapshot_offscreen(&snapshot, mvp, 64, 64, [0.0, 0.0, 0.0, 1.0])
        else {
            return;
        };
        let centre = pixel(&img, 64, 32, 32);
        assert!(
            centre[1] > 180 && centre[0] < 80,
            "camera-rendered atom not centred green: {centre:?}"
        );
    }

    #[test]
    fn renders_a_scene_to_a_decodable_png() {
        // End-to-end: render a red atom and encode PNG; decoding the PNG back
        // reproduces the image (dimensions + a red centre), so the headless
        // molecular-image export round-trips.
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(
                Vector4::new3(0.0, 0.0, 0.0),
                0.9,
                solid(1.0, 0.0, 0.0),
            )],
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        };
        let Some(png_bytes) =
            render_snapshot_png(&snapshot, IDENTITY, 48, 48, [0.0, 0.0, 0.0, 1.0])
        else {
            return;
        };
        assert!(
            png_bytes.starts_with(&[0x89, b'P', b'N', b'G']),
            "not a PNG"
        );

        let decoder = png::Decoder::new(&png_bytes[..]);
        let mut reader = decoder.read_info().expect("read PNG info");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode PNG frame");
        assert_eq!((info.width, info.height), (48, 48));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let centre = pixel(&buf, 48, 24, 24);
        assert!(
            centre[0] > 150 && centre[2] < 80,
            "decoded PNG centre not the red atom: {centre:?}"
        );
    }

    #[test]
    fn diffuse_lighting_shades_a_sphere_with_a_brightness_gradient() {
        // A white sphere under diffuse shading has a brightness gradient (centre,
        // facing the light, brighter than the rim) — unlike flat shading where
        // every lit pixel is identical. Pin that the gradient exists.
        let snapshot = RenderSceneSnapshot {
            atoms: vec![RenderAtom::new(
                Vector4::new3(0.0, 0.0, 0.0),
                0.9,
                solid(1.0, 1.0, 1.0),
            )],
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        };
        let Some(img) =
            render_snapshot_offscreen(&snapshot, IDENTITY, 96, 96, [0.0, 0.0, 0.0, 1.0])
        else {
            return;
        };
        // Brightness over the sphere's (non-background) pixels.
        let mut min = 255u8;
        let mut max = 0u8;
        for y in 0..96 {
            for x in 0..96 {
                let p = pixel(&img, 96, x, y);
                if p[0] > 40 {
                    // on the sphere (white * intensity >= 0.4*255 ~ 102)
                    min = min.min(p[0]);
                    max = max.max(p[0]);
                }
            }
        }
        // The centre is fully lit (~255); the rim is dimmer (~0.4*255). A real
        // gradient spans well over a flat-shading delta of 0.
        assert!(
            max as i32 - min as i32 > 60,
            "no shading gradient: min={min} max={max}"
        );
        assert!(max > 200, "lit centre too dark: {max}");
    }

    #[test]
    fn rotating_the_camera_changes_the_rendered_view() {
        // The deterministic core of "drag rotates the molecule": rotating the
        // camera and re-rendering produces a substantially DIFFERENT image. An
        // asymmetric 3-atom scene so a rotation can't look the same.
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(Vector4::new3(-3.0, 0.0, 0.0), 1.0, solid(1.0, 0.2, 0.2)),
                RenderAtom::new(Vector4::new3(3.0, 0.0, 0.0), 1.0, solid(0.2, 1.0, 0.2)),
                RenderAtom::new(Vector4::new3(0.0, 2.0, 4.0), 1.0, solid(0.2, 0.2, 1.0)),
            ],
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        };
        let mut camera = Camera::new();
        camera.set_zoom(14.0);
        camera.set_cam_dist(30.0);
        camera.set_slab_depth(30.0);

        let mvp_a = camera_mvp(&camera, 1.0);
        let Some(img_a) = render_snapshot_offscreen(&snapshot, mvp_a, 80, 80, [0.0, 0.0, 0.0, 1.0])
        else {
            return; // no GPU adapter
        };
        camera.set_rotation(camera.rotation().rotate_y(50.0).unwrap());
        let mvp_b = camera_mvp(&camera, 1.0);
        let img_b =
            render_snapshot_offscreen(&snapshot, mvp_b, 80, 80, [0.0, 0.0, 0.0, 1.0]).unwrap();

        // A meaningful fraction of pixels changed -> the view actually rotated.
        let changed = img_a
            .iter()
            .zip(&img_b)
            .filter(|(a, b)| (**a as i32 - **b as i32).abs() > 24)
            .count();
        assert!(
            changed > 300,
            "camera rotation barely changed the view: {changed}"
        );
    }

    #[test]
    fn perspective_projection_foreshortens_with_depth() {
        // GPU-free oracle: a perspective camera divides by view depth, so the
        // same lateral offset projects LARGER when nearer the camera. Default
        // camera: cam_dist 200, zoom 50, slab 50 -> near 175, far 225.
        let mut camera = Camera::new();
        camera.set_perspective(true);
        let p = projection_matrix(&camera, 1.0);
        let project = |v: Vector4| -> [f64; 3] {
            let c = p.mul_vec(v);
            [c.x / c.w, c.y / c.w, c.z / c.w]
        };
        // On-axis at the focal plane -> NDC origin.
        let centre = project(Vector4::new4(0.0, 0.0, -200.0, 1.0));
        assert!(
            centre[0].abs() < 1.0e-5 && centre[1].abs() < 1.0e-5,
            "centre off-axis: {centre:?}"
        );
        // Same x at z=-180 (nearer) vs z=-220 (farther): nearer is larger.
        let nearer = project(Vector4::new4(10.0, 0.0, -180.0, 1.0));
        let farther = project(Vector4::new4(10.0, 0.0, -220.0, 1.0));
        assert!(
            nearer[0] > farther[0] && farther[0] > 0.0,
            "no perspective foreshortening: nearer={nearer:?} farther={farther:?}"
        );
        // Depth still increases near->far within [0,1].
        assert!(
            nearer[2] >= -0.01 && farther[2] <= 1.01 && farther[2] > nearer[2],
            "depth ordering wrong: nearer={nearer:?} farther={farther:?}"
        );
    }

    #[test]
    fn scene_renderer_matches_the_free_function() {
        // The persistent SceneRenderer must produce byte-identical output to the
        // one-shot free function — proving the refactor is behaviour-preserving
        // (the device/pipeline reuse is the only difference).
        let green = [0.0, 1.0, 0.0, 1.0];
        let mesh = RenderMesh {
            vertices: vec![
                vert(-0.9, -0.9, green),
                vert(0.9, -0.9, green),
                vert(0.0, 0.9, green),
            ],
            indices: vec![0, 1, 2],
        };
        let Some(renderer) = SceneRenderer::new() else {
            eprintln!("no GPU adapter; skipping SceneRenderer test");
            return;
        };
        let reused = renderer
            .render(
                std::slice::from_ref(&mesh),
                IDENTITY,
                64,
                64,
                [0.0, 0.0, 0.0, 1.0],
            )
            .expect("SceneRenderer produced no image");
        let oneshot = render_meshes_offscreen(
            std::slice::from_ref(&mesh),
            IDENTITY,
            64,
            64,
            [0.0, 0.0, 0.0, 1.0],
        )
        .expect("free function produced no image");
        assert_eq!(
            reused, oneshot,
            "SceneRenderer output differs from the free function"
        );
    }

    #[test]
    fn scene_renderer_reuses_one_device_across_frames() {
        // The interactive path: build the renderer ONCE, then render two frames
        // with different MVPs from the same device/pipeline. Both must succeed and
        // the differing camera must yield a different image (proving the reused
        // pipeline still honours per-frame MVP uniforms).
        let snapshot = RenderSceneSnapshot {
            atoms: vec![
                RenderAtom::new(Vector4::new3(-3.0, 0.0, 0.0), 1.0, solid(1.0, 0.2, 0.2)),
                RenderAtom::new(Vector4::new3(3.0, 0.0, 0.0), 1.0, solid(0.2, 1.0, 0.2)),
                RenderAtom::new(Vector4::new3(0.0, 2.0, 4.0), 1.0, solid(0.2, 0.2, 1.0)),
            ],
            bonds: Vec::new(),
            meshes: Vec::new(),
            bounds: None,
        };
        let mut camera = Camera::new();
        camera.set_zoom(14.0);
        camera.set_cam_dist(30.0);
        camera.set_slab_depth(30.0);

        let Some(renderer) = SceneRenderer::new() else {
            return;
        };
        let mvp_a = camera_mvp(&camera, 1.0);
        let img_a = renderer
            .render_snapshot(&snapshot, mvp_a, 80, 80, [0.0, 0.0, 0.0, 1.0])
            .expect("frame A");
        camera.set_rotation(camera.rotation().rotate_y(50.0).unwrap());
        let mvp_b = camera_mvp(&camera, 1.0);
        let img_b = renderer
            .render_snapshot(&snapshot, mvp_b, 80, 80, [0.0, 0.0, 0.0, 1.0])
            .expect("frame B");

        let changed = img_a
            .iter()
            .zip(&img_b)
            .filter(|(a, b)| (**a as i32 - **b as i32).abs() > 24)
            .count();
        assert!(
            changed > 300,
            "reused renderer ignored the per-frame MVP: {changed} pixels changed"
        );
    }
}
