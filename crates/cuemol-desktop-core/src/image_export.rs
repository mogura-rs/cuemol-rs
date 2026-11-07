use crate::{
    effects::{ViewportEffects, ViewportOptions},
    labels::LabelRenderer,
    viewport::ViewportRenderer,
    viewport_readback::{self, Effects},
    DesktopAppState, GuiAtomLabel, GuiPngExportOptions,
};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_qsys::Camera;
use cuemol_render::RenderSceneSnapshot;
use std::sync::atomic::{AtomicBool, Ordering};

/// An immutable image job that can outlive edits to the source scene.
pub struct PreparedPngExport {
    snapshot: RenderSceneSnapshot,
    camera: Camera,
    background: [f64; 4],
    viewport: ViewportOptions,
    labels: Vec<GuiAtomLabel>,
    options: GuiPngExportOptions,
}

impl PreparedPngExport {
    pub fn capture(
        state: &DesktopAppState,
        scene_uid: Uid,
        camera: &Camera,
        options: GuiPngExportOptions,
        selection: &str,
        swap_stereo_eyes: bool,
    ) -> Result<Self> {
        options.validate()?;
        Self::capture_snapshot(
            state,
            scene_uid,
            camera,
            options,
            state.render_snapshot(scene_uid, selection)?,
            swap_stereo_eyes,
        )
    }

    pub(crate) fn capture_snapshot(
        state: &DesktopAppState,
        scene_uid: Uid,
        camera: &Camera,
        options: GuiPngExportOptions,
        mut snapshot: RenderSceneSnapshot,
        swap_stereo_eyes: bool,
    ) -> Result<Self> {
        options.validate()?;
        let scene = state
            .scene_manager()
            .scene(scene_uid)
            .ok_or_else(|| CueMolError::not_found("image export scene"))?;
        state.apply_scene_color_proof_for(scene_uid, &mut snapshot)?;
        let mut background = state.scene_background_color(scene_uid)?.map(f64::from);
        background[3] = if options.transparent { 0. } else { 1. };
        let mut viewport = ViewportOptions::from_scene(scene);
        viewport.swap_stereo_eyes = swap_stereo_eyes;
        let labels = crate::stereo::scene_labels(
            state,
            scene_uid,
            camera,
            [options.width as f32, options.height as f32],
            swap_stereo_eyes,
            10000,
        );
        Ok(Self {
            snapshot,
            camera: camera.clone(),
            background,
            viewport,
            labels,
            options,
        })
    }

    pub fn run(self, cancel: Option<&AtomicBool>) -> Result<Vec<u8>> {
        if cancelled(cancel) {
            return Err(CueMolError::invalid_argument("Image export cancelled"));
        }
        #[cfg(target_arch = "wasm32")]
        return Err(CueMolError::invalid_argument(
            "Browser PNG export requires asynchronous rendering",
        ));
        #[cfg(not(target_arch = "wasm32"))]
        pollster::block_on(async {
            let (device, queue) = crate::offscreen::create_device().await.ok_or_else(|| {
                CueMolError::invalid_argument("no GPU adapter for offscreen render")
            })?;
            self.run_async(&device, &queue, cancel).await
        })
    }

    pub async fn run_async(
        self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<u8>> {
        if cancelled(cancel) {
            return Err(CueMolError::invalid_argument("Image export cancelled"));
        }
        let options = self.options;
        let limits = device.limits();
        let padded = (options.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        if options.width.max(options.height) > limits.max_texture_dimension_2d
            || u64::from(padded) * u64::from(options.height) > limits.max_buffer_size
        {
            return Err(CueMolError::invalid_argument(
                "Image dimensions exceed the GPU limits; choose a smaller size",
            ));
        }
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = ViewportRenderer::new(device, format);
        let mut effects = ViewportEffects::new(device, queue, format);
        let mut labels = LabelRenderer::new(device, format);
        let rgba = viewport_readback::draw_async(
            &mut renderer,
            device,
            queue,
            &self.snapshot,
            &self.camera,
            [options.width, options.height],
            self.background,
            5,
            Effects::Export(&mut effects, self.viewport),
            Some((&mut labels, &self.labels)),
            None,
            cancel,
        )
        .await
        .ok_or_else(|| {
            CueMolError::invalid_argument(if cancelled(cancel) {
                "Image export cancelled"
            } else {
                "GPU image readback failed"
            })
        })?;
        if cancelled(cancel) {
            return Err(CueMolError::invalid_argument("Image export cancelled"));
        }
        Ok(crate::offscreen::encode_png(
            &rgba,
            options.width,
            options.height,
            options.transparent,
            options.resolution_dpi,
        ))
    }
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|value| value.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (DesktopAppState, Uid, Uid, Camera) {
        let mut state = DesktopAppState::new();
        state.open_bytes("atom.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let uid = state.active_scene().unwrap();
        let scene = state.scene_manager_mut().scene_mut(uid).unwrap();
        scene.set_scene_attribute("bgcolor", "#123456");
        scene.set_scene_attribute("use_colproof", "true");
        let object_uid = scene.objects()[0].uid();
        let object = state.scene_manager_mut().object_mut(object_uid).unwrap();
        let label_uid = object.add_renderer("*namelabel", "export label");
        let label = object.renderer_mut(label_uid).unwrap();
        label.set_qsc_children(vec![crate::pse_atom_label_node(0, "Label")]);
        label.set_qsc_attr("color", "#ff00ff");
        label.set_qsc_attr("font_size", "14");
        label.set_qsc_attr("dispx", "12");
        let mut camera = Camera::new();
        camera.set_zoom(8.);
        (state, uid, object_uid, camera)
    }

    fn options() -> GuiPngExportOptions {
        GuiPngExportOptions {
            width: 96,
            height: 64,
            resolution_dpi: 150.,
            transparent: true,
        }
    }

    #[test]
    fn png_capture_preserves_state_and_uses_the_requested_scene_camera_and_labels() {
        fn send<T: Send>() {}
        send::<PreparedPngExport>();
        let (mut state, scene, object, camera) = scene();
        let mut expected = state.render_snapshot(scene, "").unwrap();
        state
            .apply_scene_color_proof_for(scene, &mut expected)
            .unwrap();
        state.new_scene("unrelated active scene", true);
        let before = state.clone();
        let job = PreparedPngExport::capture(&state, scene, &camera, options(), "", true).unwrap();
        assert_eq!(job.snapshot, expected);
        assert_eq!(job.camera, camera);
        assert_eq!(job.labels.len(), 1);
        assert_eq!(job.labels[0].text, "Label");
        assert_eq!(
            job.background,
            [18. / 255., 52. / 255., 86. / 255., 0.].map(|x| x as f32 as f64)
        );
        assert!(job.viewport.swap_stereo_eyes);
        assert!(state.same_undo_state(&before));
        state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .set_visible(false);
        assert_eq!(job.snapshot, expected);
        assert!(job
            .run(Some(&AtomicBool::new(true)))
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        for invalid in [
            GuiPngExportOptions {
                width: 0,
                ..options()
            },
            GuiPngExportOptions {
                width: 8193,
                ..options()
            },
            GuiPngExportOptions {
                resolution_dpi: f64::NAN,
                ..options()
            },
        ] {
            assert!(
                PreparedPngExport::capture(&state, scene, &camera, invalid, "", false).is_err()
            );
        }
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn gpu_png_export_includes_labels_transparency_and_dpi() {
        let (state, scene, _, camera) = scene();
        let job = PreparedPngExport::capture(&state, scene, &camera, options(), "", false).unwrap();
        let labeled = job.run(None).expect("GPU PNG export");
        let mut plain =
            PreparedPngExport::capture(&state, scene, &camera, options(), "", false).unwrap();
        plain.labels.clear();
        let unlabeled = plain.run(None).expect("GPU PNG without labels");
        let decode = |bytes: &[u8]| {
            let mut reader = png::Decoder::new(bytes).read_info().unwrap();
            assert_eq!((reader.info().width, reader.info().height), (96, 64));
            assert_eq!(reader.info().color_type, png::ColorType::Rgba);
            assert_eq!(reader.info().pixel_dims.unwrap().xppu, 5906);
            let mut data = vec![0; reader.output_buffer_size()];
            reader.next_frame(&mut data).unwrap();
            data
        };
        let labeled = decode(&labeled);
        let plain = decode(&unlabeled);
        assert_eq!(labeled[3], 0);
        assert!(labeled.chunks_exact(4).filter(|p| p[3] > 128).count() > 20);
        assert!(
            labeled
                .chunks_exact(4)
                .zip(plain.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
                > 20
        );
    }
}
