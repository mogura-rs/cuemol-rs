use crate::viewport::{CameraUniform, ViewportRenderer, DEPTH_FORMAT};
use cuemol_gfx::{jitter_offset, jitter_sample_count};
use cuemol_qsys::Camera;
use cuemol_render::RenderSceneSnapshot;

pub(crate) enum Effects<'a> {
    None,
    Matrix([[f32; 4]; 4]),
    Export(
        &'a mut crate::effects::ViewportEffects,
        crate::effects::ViewportOptions,
    ),
    Live(
        &'a mut crate::effects::ViewportEffects,
        crate::effects::ViewportOptions,
    ),
}

pub(crate) struct Interaction<'a> {
    pub picker: &'a mut crate::picking::GpuPicking,
    pub revision: u64,
    pub hit: Option<cuemol_qsys::ViewHoverHit>,
    pub color: [f32; 3],
}

pub(crate) fn draw(
    renderer: &mut ViewportRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    size: [u32; 2],
    background: [f64; 4],
    level: i32,
    effects: Effects<'_>,
    labels: Option<(&mut crate::labels::LabelRenderer, &[crate::GuiAtomLabel])>,
) -> Option<Vec<u8>> {
    draw_interactive(
        renderer, device, queue, snapshot, camera, size, background, level, effects, labels, None,
    )
}

pub(crate) fn draw_interactive(
    renderer: &mut ViewportRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    size: [u32; 2],
    background: [f64; 4],
    level: i32,
    mut effects: Effects<'_>,
    mut labels: Option<(&mut crate::labels::LabelRenderer, &[crate::GuiAtomLabel])>,
    mut interaction: Option<Interaction<'_>>,
) -> Option<Vec<u8>> {
    let [width, height] = size.map(|n| n.max(1));
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = |label, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = texture(
        "viewport-export-color",
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(
        "viewport-export-depth",
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("viewport-export-readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let background = wgpu::Color {
        r: background[0],
        g: background[1],
        b: background[2],
        a: background[3],
    };
    let camera_uniform = if let Effects::Matrix(matrix) = &effects {
        CameraUniform::from_mvp(*matrix, [width, height], background)?
    } else {
        CameraUniform::new(camera, [width, height], background, false)
    };
    renderer.upload(device, queue, snapshot);
    if let Some((renderer, labels)) = &mut labels {
        renderer.prepare(device, queue, labels, [width as f32, height as f32], 1.);
    }
    let level = level.clamp(0, 5);
    let live = matches!(effects, Effects::Live(..));
    let count = if live { 33 } else { jitter_sample_count(level) };
    let mut sum = if live {
        Vec::new()
    } else {
        vec![0_u32; width as usize * height as usize * 4]
    };
    for index in 0..count {
        let mut uniform = camera_uniform;
        if !live {
            uniform.view_proj = crate::offscreen::jittered_mvp(
                uniform.view_proj,
                width,
                height,
                jitter_offset(level, index as i32),
            );
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewport-export"),
        });
        if let Some(input) = &mut interaction {
            input.picker.encode(
                device,
                queue,
                &mut encoder,
                snapshot,
                input.revision,
                camera,
                [width, height],
            );
        }
        let pending = match &mut effects {
            Effects::Live(effects, options) => effects.render(
                renderer,
                device,
                queue,
                &mut encoder,
                &color_view,
                camera,
                [width, height],
                background,
                None,
                *options,
            ),
            Effects::Export(effects, options) => {
                effects.render_export(
                    renderer,
                    device,
                    queue,
                    &mut encoder,
                    &color_view,
                    camera,
                    &uniform,
                    [width, height],
                    background,
                    *options,
                );
                false
            }
            Effects::None | Effects::Matrix(_) => {
                renderer.render_with_uniform(
                    &mut encoder,
                    queue,
                    &color_view,
                    &depth_view,
                    &uniform,
                    background,
                    None,
                );
                false
            }
        };
        if pending {
            queue.submit([encoder.finish()]);
            if let Some(input) = &mut interaction {
                input.picker.start_readback();
            }
            continue;
        }
        if let Some(input) = &mut interaction {
            input.picker.render_hover(
                device,
                queue,
                &mut encoder,
                &color_view,
                crate::picking::GpuPicking::stamp(input.revision, camera, [width, height]),
                input.hit,
                input.color,
                [
                    background.r as f32,
                    background.g as f32,
                    background.b as f32,
                ],
                [0., 0., width as f32, height as f32],
                1.,
            );
        }
        if let Some((renderer, _)) = &labels {
            let depth = match &effects {
                Effects::Live(effects, _) | Effects::Export(effects, _) => effects.label_depth(),
                Effects::None | Effects::Matrix(_) => None,
            };
            if let Some(depth) = depth {
                renderer.render(
                    device,
                    &mut encoder,
                    &color_view,
                    depth,
                    [0., 0., width as f32, height as f32],
                );
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
            extent,
        );
        queue.submit([encoder.finish()]);
        if let Some(input) = &mut interaction {
            input.picker.start_readback();
        }
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok()?;
        receiver.recv().ok()?.ok()?;
        let mapped = slice.get_mapped_range();
        if live {
            let rgba = mapped
                .chunks_exact(padded as usize)
                .flat_map(|row| row[..width as usize * 4].iter().copied())
                .collect();
            drop(mapped);
            readback.unmap();
            return Some(rgba);
        }
        for (output, row) in sum
            .chunks_exact_mut(width as usize * 4)
            .zip(mapped.chunks_exact(padded as usize))
        {
            for (sum, value) in output.iter_mut().zip(row) {
                *sum += u32::from(*value);
            }
        }
        drop(mapped);
        readback.unmap();
    }
    if live {
        return None;
    }
    Some(
        sum.into_iter()
            .map(|value| ((value + count as u32 / 2) / count as u32) as u8)
            .collect(),
    )
}
