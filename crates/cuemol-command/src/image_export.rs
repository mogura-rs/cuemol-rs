use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{DesktopAppState, GuiPngExportOptions};
use cuemol_qsys::Camera;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageExportKind {
    Png,
    Umbreon,
}

#[derive(Debug, Clone)]
pub struct ImageExportTarget {
    pub scene: Uid,
    pub view: Uid,
    pub camera: Camera,
    pub swap_stereo_eyes: bool,
}

impl ImageExportTarget {
    pub fn capture(state: &DesktopAppState, camera: &Camera, args: &Value) -> Result<Self> {
        if !args.is_object() {
            return Err(CueMolError::invalid_argument(
                "image export target must be an object",
            ));
        }
        let (scene, view, camera) = crate::render_target(state, camera, args)?;
        let swap_stereo_eyes = state
            .command_manager()
            .view(view)
            .is_some_and(|view| view.swap_stereo_eyes());
        Ok(Self {
            scene,
            view,
            camera,
            swap_stereo_eyes,
        })
    }

    pub fn validate(&self, state: &DesktopAppState) -> Result<()> {
        if state.scene_manager().scene(self.scene).is_none()
            || !state
                .command_manager()
                .view(self.view)
                .is_some_and(|view| view.scene_uid() == self.scene)
        {
            return Err(CueMolError::not_found("image export scene or view"));
        }
        Ok(())
    }

    pub fn args(&self) -> Value {
        json!({"sceneId": self.scene.get(), "viewId": self.view.get()})
    }

    pub fn prepare_umbreon(
        &self,
        state: &DesktopAppState,
        options: GuiPngExportOptions,
    ) -> Result<crate::PreparedRender> {
        self.validate(state)?;
        crate::commands::prepare_umbreon_export_target(state, self, options)
    }
}
