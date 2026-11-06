use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileOpenTarget {
    #[default]
    Active,
    New,
}

impl FileOpenTarget {
    pub fn from_config(value: &str) -> Self {
        if value == "new" {
            Self::New
        } else {
            Self::Active
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::New => "new",
        }
    }
}

/// Files submitted together retain the first successful destination.
#[derive(Debug, Clone, Default)]
pub struct FileOpenBatch {
    target: FileOpenTarget,
    scene: Option<Uid>,
}

impl FileOpenBatch {
    pub fn new(target: FileOpenTarget) -> Self {
        Self {
            target,
            scene: None,
        }
    }

    pub fn scene(&self) -> Option<Uid> {
        self.scene
    }

    pub fn with_scene(mut self, scene: Option<Uid>) -> Self {
        self.scene = scene;
        self
    }

    /// Call only after reader options have been accepted.
    pub fn open_object(
        &mut self,
        state: &mut DesktopAppState,
        open: impl FnOnce(&mut DesktopAppState) -> Result<GuiLoadedPath>,
    ) -> Result<GuiLoadedPath> {
        let before = state.clone();
        let result = (|| {
            if let Some(scene) = self.scene {
                if state.scene_manager().scene(scene).is_none() {
                    return Err(CueMolError::NotFound(
                        "file batch destination was closed".into(),
                    ));
                }
                state.set_active_scene(Some(scene));
            } else if self.target == FileOpenTarget::New && !state.active_scene_is_just_created() {
                state.new_scene("Untitled", true);
            }
            open(state)
        })();
        if result.is_err() {
            *state = before;
        } else {
            self.pin(state);
        }
        result
    }

    pub fn pin(&mut self, state: &DesktopAppState) {
        if self.scene.is_none() {
            self.scene = state.active_scene();
        }
    }

    pub fn open_path(&mut self, state: &mut DesktopAppState, path: &Path) -> Result<GuiLoadedPath> {
        let format = guess_path_format(path, &state.stream_manager)?;
        if is_scene_format(format) || format == "pse" {
            let reusable = state
                .active_scene()
                .filter(|scene| state.scene_is_just_created(*scene));
            let scene_uid = state.open_scene_file(path, reusable)?;
            self.pin(state);
            Ok(GuiLoadedPath::Scene { scene_uid })
        } else {
            self.open_object(state, |state| {
                state.open_path_with_format_inner(path, format)
            })
        }
    }

    pub fn open_prepared(
        &mut self,
        state: &mut DesktopAppState,
        file: PreparedFile,
    ) -> Result<GuiLoadedPath> {
        if file.is_scene() {
            let before = state.clone();
            let reusable = state
                .active_scene()
                .filter(|scene| state.scene_is_just_created(*scene));
            match state.open_prepared_in_scene(file, reusable) {
                Ok(loaded) => {
                    self.pin(state);
                    Ok(loaded)
                }
                Err(error) => {
                    *state = before;
                    Err(error)
                }
            }
        } else if file.style_name().is_some() {
            state.open_prepared(file)
        } else {
            self.open_object(state, |state| state.open_prepared(file))
        }
    }
}
