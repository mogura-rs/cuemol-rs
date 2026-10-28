#![cfg(feature = "offscreen")]
use cuemol_core::Vector4;
use cuemol_desktop_core::picking::GpuPicking;
use cuemol_qsys::Camera;
use cuemol_render::{RenderAtom, RenderPickAtom, RenderPickGroup, RenderSceneSnapshot};

#[test]
fn gpu_pick_buffer_uses_visible_sphere_fragments_and_retains_cached_identity() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&Default::default())).expect("GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut picker = GpuPicking::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    let mut scene = RenderSceneSnapshot::empty();
    scene.atoms.push(
        RenderAtom::new(Vector4::new3(0., 0., 2.), 1., None).with_picking(Some(1), Some(0), false),
    );
    scene.atoms.push(
        RenderAtom::new(Vector4::new3(0., 0., 0.), 2., None).with_picking(Some(1), Some(1), false),
    );
    scene.mark_transparency_group(1.);
    scene.transparency_groups[0].picking = Some(RenderPickGroup {
        object_id: 1,
        renderer_id: 2,
        symm_id: -1,
        symm_name: String::new(),
        gpu_supported: true,
        gpu_atoms: std::sync::Arc::from([]),
        atoms: vec![
            RenderPickAtom {
                position: scene.atoms[0].center,
                atom_id: 10,
                atom_index: 0,
            },
            RenderPickAtom {
                position: scene.atoms[1].center,
                atom_id: 20,
                atom_index: 1,
            },
        ]
        .into(),
    });
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(20.);
    let mut encoder = device.create_command_encoder(&Default::default());
    picker.encode(
        &device,
        &queue,
        &mut encoder,
        &scene,
        1,
        &camera,
        [128, 128],
    );
    queue.submit([encoder.finish()]);
    picker.start_readback();
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .unwrap();
    let image = picker
        .image(GpuPicking::stamp(1, &camera, [128, 128]))
        .expect("completed pick image");
    assert_eq!(
        image
            .hit_at([64., 64.], [128., 128.], 1.)
            .unwrap()
            .hit
            .atom_id,
        10
    );
    assert_eq!(
        image
            .hit_at([77., 64.], [128., 128.], 1.)
            .unwrap()
            .hit
            .atom_id,
        10
    );
    assert_eq!(
        image
            .hit_at([91., 64.], [128., 128.], 1.)
            .unwrap()
            .hit
            .atom_id,
        20
    );
    assert!(image.hit_at([2., 2.], [128., 128.], 1.).is_none());
    let stamp = GpuPicking::stamp(1, &camera, [128, 128]);
    let foreground = image.hit_at([64., 64.], [128., 128.], 1.).unwrap().hit;
    let background = image.hit_at([91., 64.], [128., 128.], 1.).unwrap().hit;
    let pixel = |data: &[u8], x: usize, y: usize| {
        <[u8; 4]>::try_from(&data[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4]).unwrap()
    };
    let front = hover_pixels(&mut picker, &device, &queue, stamp, foreground);
    let center = pixel(&front, 64, 64);
    assert!((i32::from(center[0]) - 89).abs() <= 1, "fill: {center:?}");
    assert!((i32::from(center[1]) - 36).abs() <= 1);
    assert_eq!(pixel(&front, 2, 2), [0, 0, 0, 255]);
    assert_eq!(pixel(&front, 91, 64), [0, 0, 0, 255]);
    let back = hover_pixels(&mut picker, &device, &queue, stamp, background);
    assert_eq!(pixel(&back, 64, 64), [0, 0, 0, 255], "occluded fill");
    assert!(pixel(&back, 90, 64)[0] > 60);
    for size in [[64, 64], [128, 128]] {
        let mut encoder = device.create_command_encoder(&Default::default());
        picker.encode(&device, &queue, &mut encoder, &scene, 1, &camera, size);
        queue.submit([encoder.finish()]);
        picker.start_readback();
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
    }
    let resized = hover_pixels(&mut picker, &device, &queue, stamp, foreground);
    assert_eq!(
        resized, front,
        "resize must replace the bound identity texture"
    );
    assert!(!picker.is_pending());
    let mut encoder = device.create_command_encoder(&Default::default());
    picker.encode(
        &device,
        &queue,
        &mut encoder,
        &scene,
        1,
        &camera,
        [128, 128],
    );
    assert!(!picker.is_pending());
    camera.set_center(Vector4::new3(2., 0., 0.));
    assert!(picker
        .image(GpuPicking::stamp(1, &camera, [128, 128]))
        .is_none());
}

fn hover_pixels(
    picker: &mut GpuPicking,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    stamp: cuemol_desktop_core::picking::PickStamp,
    hit: cuemol_qsys::ViewHoverHit,
) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hover-test-output"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 128 * 128 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    picker.render_hover(
        device,
        queue,
        &mut encoder,
        &view,
        stamp,
        Some(hit),
        [1., 0.4, 0.6],
        [0.; 3],
        [0., 0., 128., 128.],
        1.,
    );
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(128 * 4),
                rows_per_image: Some(128),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .unwrap();
    let result = readback.slice(..).get_mapped_range().to_vec();
    readback.unmap();
    result
}

#[test]
fn gpu_bonds_and_meshes_keep_endpoint_and_flat_vertex_ownership() {
    use cuemol_render::{MeshVertex, RenderBond, RenderMesh};
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&Default::default())).expect("GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut picker = GpuPicking::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    let mut scene = RenderSceneSnapshot::empty();
    scene.bonds.push(
        RenderBond::new(
            Vector4::new3(-2., -1., 0.),
            Vector4::new3(2., -1., 0.),
            0.3,
            None,
        )
        .with_picking(Some(1), Some((0, 1)), false),
    );
    let triangle = |z, owners: [u32; 3]| RenderMesh {
        vertices: [(-2., 1.), (2., 1.), (0., 3.)]
            .into_iter()
            .zip(owners)
            .map(|((x, y), owner)| MeshVertex {
                position: Vector4::new3(x, y, z),
                normal: Vector4::new3(0., 0., 1.),
                color: [1.; 4],
                pick_atom_index: Some(owner),
            })
            .collect(),
        indices: vec![0, 1, 2],
    };
    scene.meshes.push(triangle(0., [0, 0, 1]));
    scene.meshes.push(triangle(3., [u32::MAX; 3]));
    scene.mark_transparency_group(1.);
    scene.transparency_groups[0].picking = Some(RenderPickGroup {
        object_id: 1,
        renderer_id: 2,
        symm_id: -1,
        symm_name: String::new(),
        gpu_supported: true,
        gpu_atoms: std::sync::Arc::from([]),
        atoms: vec![
            RenderPickAtom {
                position: Vector4::new3(-2., -1., 0.),
                atom_id: 10,
                atom_index: 0,
            },
            RenderPickAtom {
                position: Vector4::new3(2., -1., 0.),
                atom_id: 20,
                atom_index: 1,
            },
        ]
        .into(),
    });
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(20.);
    for revision in [1, 2] {
        if revision == 2 {
            scene.transparency_groups[0].alpha = 0.6;
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        picker.encode(
            &device,
            &queue,
            &mut encoder,
            &scene,
            revision,
            &camera,
            [128, 128],
        );
        queue.submit([encoder.finish()]);
        picker.start_readback();
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        let image = picker
            .image(GpuPicking::stamp(revision, &camera, [128, 128]))
            .unwrap();
        if revision == 1 {
            assert_eq!(
                image
                    .hit_at([48., 80.], [128., 128.], 1.)
                    .unwrap()
                    .hit
                    .atom_id,
                10
            );
            assert_eq!(
                image
                    .hit_at([80., 80.], [128., 128.], 1.)
                    .unwrap()
                    .hit
                    .atom_id,
                20
            );
            assert_eq!(
                image
                    .hit_at([48., 40.], [128., 128.], 1.)
                    .unwrap()
                    .hit
                    .atom_id,
                20
            );
            assert!(image.hit_at([48., 68.], [128., 128.], 1.).is_none());
        } else {
            assert!(image.pixels.iter().all(|pixel| pixel[0] == 0));
        }
    }
}
