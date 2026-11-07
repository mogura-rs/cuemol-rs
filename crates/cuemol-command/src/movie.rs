mod options;
pub use options::{base_name, frame_count, MovieOptions, FORMATS};
#[cfg(not(target_arch = "wasm32"))]
pub mod files;
#[cfg(not(target_arch = "wasm32"))]
mod job;
#[cfg(not(target_arch = "wasm32"))]
pub mod sessions;
#[cfg(not(target_arch = "wasm32"))]
pub use job::{MovieEvent, MovieJob, MovieOutcome};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieResult {
    pub output_dir: String,
    pub base_name: String,
    pub frame_count: u32,
    pub fps: f64,
    pub movie_path: Option<String>,
    pub options: MovieOptions,
}

use cuemol_core::{time_from_seconds, time_to_seconds, CueMolError, Result, Uid};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use cuemol_render::{RenderMesh, RenderSceneSnapshot};
use serde_json::Value;

/// A movie owns one scene copy; advancing frames never edits the live application.
#[derive(Debug)]
pub struct MovieScene {
    state: DesktopAppState,
    scene: Uid,
    view: Uid,
    camera: Camera,
    duration: f64,
}

#[derive(Debug, Default)]
pub struct MovieDisplay {
    pub selection: String,
    pub unit_cell: bool,
    pub symmetry_mates: bool,
    pub density_contour: bool,
    pub density_level: f64,
    pub hydrogen_bonds: bool,
    pub color_proofing: bool,
    pub extra_meshes: Vec<RenderMesh>,
    pub background: [f32; 4],
    pub swap_stereo_eyes: bool,
}

impl MovieScene {
    pub fn capture(state: &DesktopAppState, camera: &Camera, target: &Value) -> Result<Self> {
        let (scene, view, camera) = crate::render_target(state, camera, target)?;
        let animation = state.scene_animation(scene)?.unwrap_or_default();
        if animation.size() == 0 || animation.length() <= 0 {
            return Err(CueMolError::invalid_argument(
                "No animation in render scene",
            ));
        }
        if let Some(error) = crate::animation_graph::Graph::read(&animation).error() {
            return Err(CueMolError::invalid_argument(error));
        }
        let duration = time_to_seconds(animation.length());
        let mut state = state.clone();
        state.activate_view(view)?;
        state.stop_scene_animation(scene, view)?;
        let mut animation = state.scene_animation(scene)?.unwrap_or_default();
        if animation.startcam().is_empty()
            || state
                .scene_manager()
                .scene(scene)
                .and_then(|scene| scene.camera_by_name(animation.startcam()))
                .is_none()
        {
            let mut initial = camera.clone();
            initial.set_name("__current");
            state.save_scene_camera(scene, initial, true)?;
            animation.set_startcam("__current");
            state.set_scene_animation(scene, Some(&animation))?;
        }
        state.set_active_view_camera(&camera)?;
        Ok(Self {
            state,
            scene,
            view,
            camera,
            duration,
        })
    }

    pub fn state(&self) -> &DesktopAppState {
        &self.state
    }
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn source(&self) -> Result<crate::render_history::RenderSource> {
        crate::render_history::RenderSource::capture_scene(&self.state, self.scene)
    }

    pub fn prepare_frame(
        &mut self,
        index: u32,
        fps: f64,
        size: [u32; 2],
    ) -> Result<crate::PreparedRender> {
        if !fps.is_finite() || fps <= 0. {
            return Err(CueMolError::invalid_argument("Frame rate must be positive"));
        }
        self.seek(f64::from(index) / fps)?;
        crate::prepare_render(
            &self.state,
            &self.camera,
            "get_screenshot",
            &serde_json::json!({
                "width":size[0], "height":size[1], "sceneId":self.scene.get(), "viewId":self.view.get(),
            }),
        )
    }

    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        if !seconds.is_finite() || seconds < 0. {
            return Err(CueMolError::invalid_argument("Invalid movie frame time"));
        }
        self.camera = self.state.apply_scene_animation_time(
            self.scene,
            time_from_seconds(seconds.min(self.duration)),
            self.view,
        )?;
        Ok(())
    }

    pub fn snapshot(&self, display: &MovieDisplay) -> Result<RenderSceneSnapshot> {
        let mut snapshot = self.state.active_render_snapshot(&display.selection)?;
        self.state.append_crystal_geometry(
            &mut snapshot,
            display.unit_cell,
            display.symmetry_mates,
        );
        self.state.append_density_geometry(
            &mut snapshot,
            display.density_level,
            display.density_contour,
        );
        self.state
            .append_hbond_geometry(&mut snapshot, display.hydrogen_bonds);
        snapshot.meshes.extend(display.extra_meshes.iter().cloned());
        if display.color_proofing && self.state.apply_color_proof(&mut snapshot).is_err() {
            cuemol_render::apply_generic_color_proof(&mut snapshot);
        }
        Ok(snapshot)
    }
}
