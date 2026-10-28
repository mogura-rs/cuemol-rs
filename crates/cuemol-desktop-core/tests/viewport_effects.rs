#![cfg(feature = "offscreen")]

use cuemol_core::Vector4;
use cuemol_desktop_core::{
    effects::{Antialiasing, ViewportEffects, ViewportOptions},
    offscreen::{encode_rgba_png, SceneRenderer},
    viewport::{atom_instance, bond_instance, ViewportRenderer},
};
use cuemol_gfx::{jitter_offset, jitter_sample_count};
use cuemol_qsys::Camera;
use cuemol_render::{MeshVertex, RenderAtom, RenderBond, RenderMesh, RenderSceneSnapshot};
use serde_json::json;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: ViewportRenderer,
    effects: ViewportEffects,
}

impl Gpu {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        eprintln!("Postprocess adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        Some(Self {
            renderer: ViewportRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm),
            effects: ViewportEffects::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm),
            device,
            queue,
        })
    }

    fn draw(
        &mut self,
        camera: &Camera,
        options: ViewportOptions,
        frames: usize,
    ) -> (Vec<u8>, bool) {
        let size = wgpu::Extent3d {
            width: 129,
            height: 127,
            depth_or_array_layers: 1,
        };
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("effects-test"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let padded = (129_u32 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("effects-test-readback"),
            size: u64::from(padded * 127),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut pending = false;
        for _ in 0..frames {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            pending = self.effects.render(
                &self.renderer,
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                camera,
                [129, 127],
                wgpu::Color {
                    r: 0.05,
                    g: 0.08,
                    b: 0.12,
                    a: 1.0,
                },
                None,
                options,
            );
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(127),
                    },
                },
                size,
            );
            self.queue.submit([encoder.finish()]);
        }
        let slice = buffer.slice(..);
        let (send, receive) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        receive.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let rgba = mapped
            .chunks_exact(padded as usize)
            .flat_map(|row| row[..129 * 4].iter().copied())
            .collect();
        drop(mapped);
        buffer.unmap();
        (rgba, pending)
    }
}

fn scene() -> RenderSceneSnapshot {
    let mut scene = RenderSceneSnapshot::empty();
    scene
        .atoms
        .push(RenderAtom::new(Vector4::new3(-1.2, 0.0, 2.6), 2.5, None));
    scene
        .atoms
        .push(RenderAtom::new(Vector4::new3(2.43, 1.57, 1.8), 1.8, None));
    scene.meshes.push(RenderMesh {
        vertices: [(-6.0, -5.0), (6.0, -5.0), (6.0, 5.0), (-6.0, 5.0)]
            .into_iter()
            .map(|(x, y)| MeshVertex {
                position: Vector4::new3(x, y, 0.0),
                normal: Vector4::new3(0.0, 0.0, 1.0),
                color: [0.7, 0.6, 0.5, 1.0],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    let mut line = RenderBond::new(
        Vector4::new3(-5.0, -4.0, 0.2),
        Vector4::new3(5.0, -2.5, 0.2),
        0.1,
        None,
    );
    line.screen_width = Some(2.0);
    scene.bonds.push(line);
    scene
}

fn record(
    name: &str,
    scene: &RenderSceneSnapshot,
    camera: &Camera,
    options: ViewportOptions,
    frames: usize,
    rgba: &[u8],
) {
    let Some(directory) = std::env::var_os("CUEMOL_EFFECTS_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join(format!("{name}-rust.png")),
        encode_rgba_png(rgba, 129, 127),
    )
    .unwrap();
    let atoms=scene.atoms.iter().map(|a| {let a=atom_instance(a);json!({"center_radius":a.center_radius,"color":a.color,"edge":a.edge,"edge_color":a.edge_color})}).collect::<Vec<_>>();
    let bonds=scene.bonds.iter().map(|b| {let b=bond_instance(b);json!({"from_radius":b.from_radius,"to_edge":b.to_edge,"color":b.color,"end_color":b.end_color,"edge_mode":b.edge_mode,"edge_color":b.edge_color})}).collect::<Vec<_>>();
    let meshes=scene.meshes.iter().map(|m| json!({"vertices":m.vertices.iter().map(|v|vec![v.position.x,v.position.y,v.position.z,v.normal.x,v.normal.y,v.normal.z,v.color[0] as f64,v.color[1] as f64,v.color[2] as f64,v.color[3] as f64]).collect::<Vec<_>>(),"indices":m.indices})).collect::<Vec<_>>();
    let offsets = if options.jitter_level > 0 && frames > 1 {
        (0..jitter_sample_count(options.jitter_level))
            .map(|i| jitter_offset(options.jitter_level, i as i32))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let fixture = json!({"size":[129,127],"view":[[1,0,0,-camera.center().x],[0,1,0,-camera.center().y],[0,0,1,-camera.cam_dist()-camera.center().z],[0,0,0,1]],
        "zoom":camera.zoom(),"distance":camera.cam_dist(),"slab":camera.slab_depth(),"perspective":camera.is_perspective(),
        "background":[0.05,0.08,0.12,1.0],"atoms":atoms,"bonds":bonds,"meshes":meshes,
        "effects":{"ao":options.ao_enabled,"radius":options.ao_radius,"power":options.ao_intensity,"slices":options.ao_slices,"steps":options.ao_steps,
            "half":options.ao_half_res && frames==1,"aa":format!("{:?}",options.antialiasing).to_lowercase(),"threshold":options.smaa_threshold,"offsets":offsets}});
    std::fs::write(
        directory.join(format!("{name}.json")),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
}

#[test]
fn gpu_effects_change_contact_shadows_edges_and_refine_without_stale_history() {
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("Skipping postprocess pixels: no GPU adapter");
        return;
    };
    let mut camera = Camera::new();
    camera.set_perspective(false);
    camera.set_cam_dist(40.0);
    camera.set_slab_depth(40.0);
    camera.set_zoom(18.0);
    let scene = scene();
    gpu.renderer.upload(&gpu.device, &gpu.queue, &scene);
    let none = ViewportOptions {
        antialiasing: Antialiasing::None,
        ..Default::default()
    };
    let ao = ViewportOptions {
        ao_enabled: true,
        ..none
    };
    let (plain, pending) = gpu.draw(&camera, none, 1);
    assert!(!pending);
    record("none", &scene, &camera, none, 1, &plain);
    let mut images = std::collections::BTreeMap::new();
    for (name, options, frames) in [
        ("ao", ao, 1),
        (
            "ao-half",
            ViewportOptions {
                ao_half_res: true,
                ..ao
            },
            1,
        ),
        (
            "ao-half-refined",
            ViewportOptions {
                ao_half_res: true,
                ..ao
            },
            2,
        ),
        (
            "fxaa",
            ViewportOptions {
                antialiasing: Antialiasing::Fxaa,
                ..none
            },
            1,
        ),
        (
            "smaa",
            ViewportOptions {
                antialiasing: Antialiasing::Smaa,
                ..none
            },
            1,
        ),
        (
            "smaa-high-threshold",
            ViewportOptions {
                antialiasing: Antialiasing::Smaa,
                smaa_threshold: 0.8,
                ..none
            },
            1,
        ),
        (
            "ao-fxaa",
            ViewportOptions {
                antialiasing: Antialiasing::Fxaa,
                ..ao
            },
            1,
        ),
        (
            "ao-smaa",
            ViewportOptions {
                antialiasing: Antialiasing::Smaa,
                ..ao
            },
            1,
        ),
        (
            "jitter",
            ViewportOptions {
                jitter_level: 2,
                ..none
            },
            5,
        ),
        (
            "ao-jitter",
            ViewportOptions {
                jitter_level: 2,
                ..ao
            },
            5,
        ),
    ] {
        // A new scene upload also exercises invalidation without a camera change.
        gpu.renderer.upload(&gpu.device, &gpu.queue, &scene);
        let (image, pending) = gpu.draw(&camera, options, frames);
        assert_eq!(pending, name == "ao-half");
        assert!(image.chunks_exact(4).all(|p| p[3] == 255));
        record(name, &scene, &camera, options, frames, &image);
        images.insert(name, image);
    }
    let darkened = plain
        .chunks_exact(4)
        .zip(images["ao"].chunks_exact(4))
        .filter(|(a, b)| a[0].saturating_sub(b[0]) > 8)
        .count();
    assert!(darkened > 100, "AO did not darken contacts: {darkened}");
    assert_eq!(images["ao"], images["ao-half-refined"]);
    assert_ne!(images["ao"], images["ao-half"]);
    for name in ["fxaa", "smaa", "jitter"] {
        assert_ne!(plain, images[name], "{name} did not change edges");
    }
    assert_ne!(images["smaa"], images["smaa-high-threshold"]);
    let options = ViewportOptions {
        jitter_level: 2,
        ..ao
    };
    let (cached, pending) = gpu.draw(&camera, options, 1);
    assert!(!pending);
    assert_eq!(cached, images["ao-jitter"]);
    camera.set_zoom(20.0);
    assert!(gpu.draw(&camera, options, 1).1);
    camera.set_zoom(18.0);
    let (reset, pending) = gpu.draw(&camera, options, 1);
    assert!(pending);
    assert_eq!(reset, images["ao"]);
    for (name, distance, perspective) in [("ao-perspective", 40.0, true), ("ao-fog", 50.0, false)] {
        camera.set_perspective(perspective);
        if name == "ao-fog" {
            camera.set_center(Vector4::new3(0.0, 0.0, 10.0));
        }
        camera.set_cam_dist(distance);
        let (image, _) = gpu.draw(&camera, ao, 1);
        record(name, &scene, &camera, ao, 1, &image);
        assert_ne!(image, plain);
    }
    let exporter = SceneRenderer::new().unwrap();
    camera.set_center(Vector4::ZERO);
    camera.set_cam_dist(40.0);
    for (name, options) in [
        ("ao", ao),
        (
            "ao-smaa",
            ViewportOptions {
                antialiasing: Antialiasing::Smaa,
                ..ao
            },
        ),
        (
            "ao-jitter",
            ViewportOptions {
                jitter_level: 2,
                ao_half_res: true,
                ..ao
            },
        ),
    ] {
        let image = exporter
            .render_viewport(&scene, &camera, 129, 127, [0.05, 0.08, 0.12, 1.0], options)
            .unwrap();
        assert_eq!(image, images[name], "live readback differs for {name}");
    }
    let transparent = exporter
        .render_camera_with_effects(&scene, &camera, 129, 127, [0.05, 0.08, 0.12, 0.0], 0, ao)
        .unwrap();
    let raw = exporter
        .render_camera(&scene, &camera, 129, 127, [0.05, 0.08, 0.12, 0.0], 0)
        .unwrap();
    assert!(transparent
        .chunks_exact(4)
        .zip(raw.chunks_exact(4))
        .all(|(a, b)| a[3] == b[3]));
    assert_ne!(transparent, raw);
}
