use crate::{
    commands::{text, uid},
    nodes,
};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{DesktopAppState, GuiWorkspaceObjectKind};
use serde_json::{json, Value};
use std::path::Path;

pub struct PreparedObjectSave {
    pub path: String,
    pub format: String,
    pub bytes: Vec<u8>,
    object: Uid,
    kind: GuiWorkspaceObjectKind,
    link: bool,
}

impl PreparedObjectSave {
    pub fn finish(self, state: &mut DesktopAppState) -> Result<Value> {
        if self.link {
            state.link_workspace_object_export(
                self.kind,
                self.object,
                &self.path,
                &self.format,
                self.bytes,
            )?;
        }
        Ok(json!({"path":self.path,"format":self.format}))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(self, state: &mut DesktopAppState) -> Result<Value> {
        std::fs::write(&self.path, &self.bytes)?;
        self.finish(state)
    }
}

pub fn prepare_save_object(state: &DesktopAppState, args: &Value) -> Result<PreparedObjectSave> {
    crate::validate_args("save_object", args).map_err(CueMolError::invalid_argument)?;
    let object = uid(args, "objId")?;
    let kind = nodes::object_kind(state, object)?;
    let path = text(args, "fileName")?;
    if path.trim().is_empty() {
        return Err(CueMolError::invalid_argument("an output file is required"));
    }
    let format = if let Some(format) = args["format"].as_str() {
        format.to_string()
    } else {
        let extension = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match (kind, extension.as_str()) {
            (GuiWorkspaceObjectKind::Molecule, "pdb" | "ent" | "") => "pdb",
            (GuiWorkspaceObjectKind::Molecule, "sdf" | "mol") => "sdf",
            (GuiWorkspaceObjectKind::Molecule, "pqr") => "pqr",
            (GuiWorkspaceObjectKind::Molecule, "xyzr") => "xyzr",
            (GuiWorkspaceObjectKind::Molecule, "qdf") => "qdfmol",
            (GuiWorkspaceObjectKind::DensityMap, "qdf") => {
                if state
                    .density_map_summary(object)
                    .is_some_and(|map| map.class_name == "ElePotMap")
                {
                    "qdfpot"
                } else {
                    "qdfmap"
                }
            }
            (GuiWorkspaceObjectKind::Surface, "qdf") => "qdfsurf",
            (GuiWorkspaceObjectKind::Lightweight, "qdf") => "qdflwobj",
            _ => {
                return Err(CueMolError::invalid_argument(format!(
                    "no object writer for .{extension}; specify a compatible format"
                )))
            }
        }
        .into()
    };
    let selection = args["selection"]
        .as_str()
        .filter(|value| !value.trim().is_empty() && value.trim() != "*");
    let bytes = state.workspace_object_export_bytes(kind, object, &format, selection)?;
    Ok(PreparedObjectSave {
        path: path.into(),
        format,
        bytes,
        object,
        kind,
        link: selection.is_none() && args["link"] != false,
    })
}
