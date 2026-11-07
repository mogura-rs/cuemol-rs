use crate::{DesktopAppState, GuiRendererSetupOption};
use cuemol_core::{CueMolError, Result, Uid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Molecule,
    Map,
    Surface,
    Lightweight,
}

#[derive(Debug)]
pub struct Target {
    pub object: Uid,
    pub scene: Uid,
    pub name: String,
    pub class: String,
    pub kind: Kind,
    pub options: Vec<GuiRendererSetupOption>,
    pub groups: Vec<String>,
}

impl Target {
    pub fn for_object(state: &DesktopAppState, object: Uid) -> Result<Self> {
        let (scene, name, kind, options, groups) =
            if let Some(map) = state.density_map_summary(object) {
                (
                    map.scene_uid,
                    map.name,
                    Kind::Map,
                    state.density_map_renderer_setup_options(),
                    map.renderer_groups.into_iter().map(|g| g.name).collect(),
                )
            } else if let Some(surface) = state.surface_summary(object) {
                (
                    surface.scene_uid,
                    surface.name,
                    Kind::Surface,
                    state.surface_renderer_setup_options(),
                    surface
                        .renderer_groups
                        .into_iter()
                        .map(|g| g.name)
                        .collect(),
                )
            } else if let Some(lw) = state.lw_object_summary(object) {
                (
                    lw.scene_uid,
                    lw.name,
                    Kind::Lightweight,
                    vec![GuiRendererSetupOption {
                        name: "lwrend".into(),
                        description: "Lightweight renderer".into(),
                        preset: false,
                    }],
                    lw.renderer_groups.into_iter().map(|g| g.name).collect(),
                )
            } else {
                let scene = state
                    .scene_manager()
                    .scene_uid_for_object(object)
                    .ok_or_else(|| CueMolError::not_found("renderer target object"))?;
                let molecule = state.scene_manager().object(object).unwrap();
                (
                    scene,
                    molecule.name().to_string(),
                    Kind::Molecule,
                    state.molecule_renderer_setup_options(scene)?,
                    state
                        .renderer_summaries(object)
                        .into_iter()
                        .filter(|r| r.renderer_type == "*group")
                        .map(|r| r.name)
                        .collect(),
                )
            };
        Ok(Self {
            object,
            scene,
            name,
            class: state.renderer_object_class(object).unwrap_or_default(),
            kind,
            options,
            groups,
        })
    }
}
