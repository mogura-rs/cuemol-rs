use cuemol_core::{CueMolError, Result, Uid, Vector4};
use cuemol_desktop_core::{DesktopAppState, GuiAtomLabel, GuiPovSceneOptions};
use cuemol_qsys::Camera;
use serde_json::{Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::pov::{PovProcessOptions, PovProcessTask, PovProgress};

pub(crate) struct PreparedPovRender {
    task: PovProcessTask,
    labels: Vec<GuiAtomLabel>,
    progress: PovProgress,
    _work: tempfile::TempDir,
}

impl PreparedPovRender {
    pub(crate) fn prepare(
        state: &DesktopAppState,
        scene: Uid,
        camera: &Camera,
        settings: &Map<String, Value>,
        size: [u32; 2],
    ) -> Result<Self> {
        let mut values = crate::render_editor::defaults();
        values.extend(settings.clone());
        let text = |key: &str| values.get(key).and_then(Value::as_str).unwrap_or("");
        let flag = |key: &str| values.get(key).and_then(Value::as_bool).unwrap_or(false);
        let number = |key: &str| values.get(key).and_then(Value::as_f64).unwrap_or(0.);
        let stereo = match text("stereoMode") {
            "none" => 0,
            "left" => -1,
            "right" => 1,
            _ => {
                return Err(CueMolError::invalid_argument(
                    "POV-Ray stereo mode must be none, left or right",
                ))
            }
        };
        let radiosity = [
            "Disable",
            "Default",
            "Debug",
            "Fast",
            "Normal",
            "2-Bounce",
            "Final",
            "Outdoor LQ",
            "Outdoor HQ",
            "Outdoor Light",
            "Indoor LQ",
            "Indoor HQ",
        ]
        .iter()
        .position(|label| *label == text("povray.radiosityMode"))
        .ok_or_else(|| CueMolError::invalid_argument("invalid POV-Ray radiosity mode"))?
            as i32
            - 1;
        let perspective = match text("projection") {
            "perspective" => true,
            "orthographic" => false,
            _ => return Err(CueMolError::invalid_argument("invalid render projection")),
        };
        for (key, min, max) in [("numThreads", 1., 32.), ("povray.lightSpread", 1., 10.)] {
            let value = number(key);
            if !value.is_finite() || value.fract() != 0. || !(min..=max).contains(&value) {
                return Err(CueMolError::invalid_argument(format!(
                    "{key} is outside its supported range"
                )));
            }
        }
        let options = PovProcessOptions {
            width: size[0],
            height: size[1],
            threads: number("numThreads") as u32,
            stereo,
            stereo_depth: number("stereoDepth"),
            perspective,
            transparent: flag("transparentBg"),
            shadow: flag("povray.shadow"),
            light_spread: number("povray.lightSpread") as u32,
            light_intensity: number("povray.lightIntensity"),
            flash_fraction: number("povray.flashFraction"),
            ambient_fraction: number("povray.ambientFraction"),
            radiosity,
            post_blend: flag("postBlend"),
            ..Default::default()
        };
        crate::pov::validate_options(&options).map_err(CueMolError::invalid_argument)?;
        let (executable, include_dir) = tool_paths(state)?;
        let work = tempfile::Builder::new()
            .prefix("cuemol-render-pov-")
            .tempdir()?;
        let input_path = work.path().join("render.pov");
        let mut camera = camera.clone();
        camera.set_perspective(perspective);
        state.export_scene_pov_with_camera(
            &input_path,
            scene,
            "",
            &camera,
            GuiPovSceneOptions {
                perspective: Some(perspective),
                use_clip_z: flag("clipPlane"),
                use_post_blend: options.post_blend,
                show_edge_lines: flag("edgeLines"),
                use_pixel_labels: flag("pixelLabels"),
            },
        )?;
        if stereo != 0 {
            let right = camera
                .rotation()
                .inv()?
                .to_matrix()
                .mul_vec(Vector4::new4(1., 0., 0., 0.));
            let offset = right.scale(f64::from(stereo) * camera.cam_dist() * options.stereo_depth);
            camera.set_center(camera.center() + offset);
        }
        let labels = crate::pov_labels::collect_scene_pov_labels(
            state,
            scene,
            &camera,
            size[0],
            size[1],
            flag("pixelLabels"),
        );
        Ok(Self {
            task: PovProcessTask {
                executable: executable.to_string_lossy().into_owned(),
                include_dir,
                input_path,
                output_path: work.path().join("render.png"),
                options,
            },
            labels,
            progress: Default::default(),
            _work: work,
        })
    }

    pub(crate) fn progress(&self) -> PovProgress {
        self.progress.clone()
    }

    pub(crate) fn run(&self, cancel: &AtomicBool) -> Result<cuemol_render::UmbreonFrame> {
        let process_cancel = Arc::new(AtomicBool::new(cancel.load(Ordering::Relaxed)));
        let started = std::time::Instant::now();
        crate::pov::run_pov_process_with_progress(
            self.task.clone(),
            process_cancel.clone(),
            || {
                process_cancel.store(cancel.load(Ordering::Relaxed), Ordering::Relaxed);
            },
            Some(self.progress.clone()),
        )
        .map_err(CueMolError::Io)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        let dimensions = image::image_dimensions(&self.task.output_path)
            .map_err(|error| CueMolError::Io(format!("Invalid POV-Ray output: {error}")))?;
        if dimensions != (self.task.options.width, self.task.options.height) {
            return Err(CueMolError::invalid_argument(
                "POV-Ray PNG dimensions do not match the request",
            ));
        }
        let image = image::open(&self.task.output_path)
            .map_err(|error| CueMolError::Io(format!("Invalid POV-Ray output: {error}")))?;
        if image.width() != self.task.options.width || image.height() != self.task.options.height {
            return Err(CueMolError::invalid_argument(
                "POV-Ray PNG dimensions do not match the request",
            ));
        }
        let mut image = image.to_rgba8();
        crate::pov_labels::overlay_labels(&mut image, &self.labels);
        Ok(cuemol_render::UmbreonFrame {
            width: self.task.options.width as i32,
            height: self.task.options.height as i32,
            pixels: image.into_raw(),
            components: 4,
            render_seconds: started.elapsed().as_secs_f64(),
        })
    }
}

pub(crate) fn tool_paths(state: &DesktopAppState) -> Result<(PathBuf, String)> {
    let resolved = crate::external::resolve(state, "povray")?;
    let path = if resolved.components().count() == 1 && !resolved.is_file() {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join(&resolved))
            .find(|path| path.is_file())
            .unwrap_or(resolved)
    } else {
        resolved
    };
    let executable = absolute_path(&path)?;
    if !executable.is_file() {
        return Err(CueMolError::not_found(
            "POV-Ray executable; choose its path in Settings > Tools",
        ));
    }
    let include = state.config_value("cuemol2.ui.render.pov-inc-path");
    let include_dir = if include.trim().is_empty() {
        String::new()
    } else {
        let path = absolute_path(Path::new(include.trim()))?;
        if !path.is_dir() {
            return Err(CueMolError::not_found(
                "POV-Ray include directory; choose its path in Settings > Tools",
            ));
        }
        path.to_string_lossy().into_owned()
    };
    Ok((executable, include_dir))
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf> {
    let path = if let Ok(suffix) = path.strip_prefix("~") {
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .ok_or_else(|| CueMolError::not_found("home directory"))?
            .join(suffix)
    } else {
        path.to_path_buf()
    };
    Ok(if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(executable: &Path) -> (DesktopAppState, Camera, Uid) {
        let mut state = DesktopAppState::new();
        state.open_bytes("pov.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
        state.put_config_value(
            "cuemol2.ui.render.pov-exe-path",
            executable.to_string_lossy().into_owned(),
        );
        let scene = state.active_scene().unwrap();
        let mut camera = Camera::new();
        camera.set_zoom(8.);
        camera.set_perspective(false);
        state.set_active_view_camera(&camera).unwrap();
        crate::call_tool(&mut state, &mut camera,"set_render_settings",&json!({"settings":"backend=povray; width=64; height=48; unit=px; dpi=150; projection=orthographic; transparentBg=true; postBlend=true; pixelLabels=false; numThreads=2; povray.shadow=true; povray.radiosityMode=Disable; povray.lightIntensity=1.2; povray.flashFraction=0.4"})).unwrap();
        (state, camera, scene)
    }

    #[test]
    fn prepared_process_owns_files_and_preserves_scene_settings_and_dpi() {
        let directory = tempfile::tempdir().unwrap();
        let image_path = directory.path().join("fixture.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([20, 40, 60, 128]))
            .save(&image_path)
            .unwrap();
        let executable = directory.path().join("povray");
        std::fs::write(&executable,format!("#!/bin/sh\nfor arg in \"$@\"; do case \"$arg\" in +O*) out=${{arg#'+O'}};; esac; done\necho rendering-fixture\ncp '{}' \"$out\"\n",image_path.display())).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (mut state, camera, scene) = fixture(&executable);
        let settings = crate::scene_render_settings(&state, scene).unwrap();
        let prepared =
            PreparedPovRender::prepare(&state, scene, &camera, &settings, [64, 48]).unwrap();
        assert_eq!(prepared.task.options.flash_fraction, 0.4);
        assert_eq!(prepared.task.options.light_intensity, 1.2);
        assert!(!prepared.task.options.perspective);
        assert!(
            prepared.task.options.shadow
                && prepared.task.options.transparent
                && prepared.task.options.post_blend
        );
        let work = prepared._work.path().to_path_buf();
        assert!(prepared.task.input_path.is_file());
        drop(prepared);
        assert!(!work.exists());
        state.new_scene("active other", true);
        let before = state.clone();
        let render = crate::prepare_render(
            &state,
            &camera,
            "get_screenshot",
            &json!({"sceneId":scene.get()}),
        )
        .unwrap();
        let progress = render.pov_progress().unwrap();
        let result = render.run().unwrap();
        assert!(progress.text().contains("rendering-fixture"));
        assert!(before.same_undo_state(&state));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(result["_image"].as_str().unwrap())
            .unwrap();
        let decoder = png::Decoder::new(bytes.as_slice()).read_info().unwrap();
        assert_eq!((decoder.info().width, decoder.info().height), (64, 48));
        assert_eq!(decoder.info().pixel_dims.unwrap().xppu, 5906);
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [20, 40, 60, 128]
        );
        assert!(crate::prepare_render(
            &state,
            &camera,
            "render_image",
            &json!({"sceneId":scene.get(),"fileName":"unused.png","style":"npr"})
        )
        .unwrap()
        .pov_progress()
        .is_none());
        for (key, value) in [
            ("numThreads", json!(0)),
            ("povray.radiosityMode", json!("unknown")),
            ("stereoMode", json!("both")),
        ] {
            let mut invalid = settings.clone();
            invalid.insert(key.into(), value);
            assert!(
                PreparedPovRender::prepare(&state, scene, &camera, &invalid, [64, 48]).is_err()
            );
        }
    }

    #[test]
    fn failed_and_cancelled_render_leave_the_requested_output_intact() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("povray");
        let destination = directory.path().join("existing.png");
        std::fs::write(&destination, b"keep").unwrap();
        let marker = directory.path().join("started");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\ntouch '{}'\necho render-error >&2\nexit 2\n",
                marker.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (state, camera, _) = fixture(&executable);
        let args = json!({"fileName":destination});
        let error = crate::prepare_render(&state, &camera, "render_image", &args)
            .unwrap()
            .run_with_cancel(&AtomicBool::new(true))
            .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(!marker.exists());
        let error = crate::prepare_render(&state, &camera, "render_image", &args)
            .unwrap()
            .run()
            .unwrap_err();
        assert!(error.to_string().contains("render-error"));
        assert!(marker.exists());
        assert_eq!(std::fs::read(destination).unwrap(), b"keep");
    }

    #[test]
    #[ignore = "requires CUEMOL_TEST_POVRAY pointing to a real renderer"]
    fn prepared_pov_real_scene_png_labels_and_file_output() {
        let executable = std::env::var("CUEMOL_TEST_POVRAY").unwrap();
        let (mut state, mut camera, scene) = fixture(Path::new(&executable));
        let object = state.active_object_summaries()[0].uid;
        crate::call_tool(
            &mut state,
            &mut camera,
            "set_atom_labels",
            &json!({"objId":object.get(),"selection":"*","format":"C1"}),
        )
        .unwrap();
        let render = |state: &DesktopAppState, camera: &Camera| {
            let result = crate::prepare_render(
                state,
                camera,
                "get_screenshot",
                &json!({"sceneId":scene.get()}),
            )
            .unwrap()
            .run()
            .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(result["_image"].as_str().unwrap())
                .unwrap();
            image::load_from_memory(&bytes).unwrap().to_rgba8()
        };
        let without = render(&state, &camera);
        assert_eq!(without.dimensions(), (64, 48));
        assert!(without.pixels().any(|pixel| pixel[3] > 0));
        assert!(without.pixels().any(|pixel| pixel[3] < 255));
        crate::call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":"pixelLabels=true"}),
        )
        .unwrap();
        let with = render(&state, &camera);
        assert_ne!(with, without);
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("render image.png");
        let result =
            crate::prepare_render(&state, &camera, "render_image", &json!({"fileName":file}))
                .unwrap()
                .run()
                .unwrap();
        assert_eq!(result["width"], 64);
        assert_eq!(image::open(file).unwrap().to_rgba8(), with);
    }
}
