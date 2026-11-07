use crate::{DesktopAppState, GuiFileRendererTarget, GuiRendererSetupOption};
use cuemol_core::Uid;
use serde_json::Value;

pub const KEY: &str = "cuemol.fopenOptions.rendTypeByObjType";

impl GuiFileRendererTarget {
    pub fn object_class(self) -> &'static str {
        match self {
            Self::Molecule => "MolCoord",
            Self::DensityMap => "DensityMap",
            Self::Surface => "MolSurfObj",
        }
    }
}

impl DesktopAppState {
    pub fn renderer_history_type(
        &self,
        object_class: &str,
        options: &[GuiRendererSetupOption],
    ) -> Option<String> {
        let values: Value = serde_json::from_str(&self.config_value(KEY)).ok()?;
        let value = values.as_object()?.get(object_class)?.as_str()?;
        options
            .iter()
            .any(|option| option.name == value)
            .then(|| value.to_string())
    }

    pub fn remember_renderer_type(&mut self, object_class: &str, renderer_type: &str) {
        if object_class.is_empty() || renderer_type.is_empty() || renderer_type == "*group" {
            return;
        }
        let mut values = serde_json::from_str::<Value>(&self.config_value(KEY))
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        values.insert(object_class.into(), Value::String(renderer_type.into()));
        self.put_config_value(KEY, Value::Object(values).to_string());
    }

    pub fn renderer_object_class(&self, uid: Uid) -> Option<String> {
        if let Some(scene) = self
            .scene_manager()
            .scene_uid_for_object(uid)
            .and_then(|id| self.scene_manager().scene(id))
        {
            if let Some(object) = scene.objects().iter().find(|object| object.uid() == uid) {
                return Some(object.object_type().into());
            }
        }
        self.density_map_summary(uid)
            .map(|object| object.class_name)
            .or_else(|| self.surface_summary(uid).map(|object| object.class_name))
            .or_else(|| self.lw_object_summary(uid).map(|object| object.class_name))
    }
}

pub fn default_type(options: &[GuiRendererSetupOption]) -> Option<String> {
    options
        .iter()
        .find(|option| option.name == "simple")
        .or_else(|| options.iter().find(|option| !option.preset))
        .or_else(|| options.first())
        .map(|option| option.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_class_specific_validated_persistent_and_outside_undo() {
        let mut state = DesktopAppState::new();
        let options = ["simple", "cpk", "isosurf", "Default1RendPreset", "auto"].map(|name| {
            GuiRendererSetupOption {
                name: name.into(),
                description: name.into(),
                preset: name.ends_with("Preset"),
            }
        });
        for value in ["{broken", "[]", "null", "{\"MolCoord\":3}"] {
            state.put_config_value(KEY, value);
            assert_eq!(state.renderer_history_type("MolCoord", &options), None);
        }
        let mut before = state.clone();
        state.remember_renderer_type("MolCoord", "cpk");
        state.remember_renderer_type("DensityMap", "isosurf");
        state.remember_renderer_type("ElePotMap", "simple");
        state.remember_renderer_type("MolCoord", "*group");
        assert_eq!(
            state.renderer_history_type("MolCoord", &options).as_deref(),
            Some("cpk")
        );
        assert_eq!(
            state
                .renderer_history_type("DensityMap", &options)
                .as_deref(),
            Some("isosurf")
        );
        assert_eq!(
            state
                .renderer_history_type("ElePotMap", &options)
                .as_deref(),
            Some("simple")
        );
        before.preserve_host_preferences_from(&state);
        state.restore_snapshot(before);
        assert_eq!(
            state.renderer_history_type("MolCoord", &options).as_deref(),
            Some("cpk")
        );
        let mut restored = DesktopAppState::new();
        restored.load_config_text(&state.config_text()).unwrap();
        assert_eq!(
            restored
                .renderer_history_type("DensityMap", &options)
                .as_deref(),
            Some("isosurf")
        );
        restored.remember_renderer_type("MolCoord", "Default1RendPreset");
        assert_eq!(
            restored
                .renderer_history_type("MolCoord", &options)
                .as_deref(),
            Some("Default1RendPreset")
        );
        assert_eq!(
            restored.renderer_history_type("MolCoord", &options[..3]),
            None
        );
        assert_eq!(default_type(&options).as_deref(), Some("simple"));
    }
}
