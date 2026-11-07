use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneExportFormat {
    Pov,
    Stl,
    Mqo,
}

impl SceneExportFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "pov" => Ok(Self::Pov),
            "stl" => Ok(Self::Stl),
            "mqo" => Ok(Self::Mqo),
            _ => Err(CueMolError::invalid_argument(
                "unsupported scene export format",
            )),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Pov => "pov",
            Self::Stl => "stl",
            Self::Mqo => "mqo",
        }
    }
}

pub struct PreparedSceneExport {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

pub fn prepare_scene_export(
    state: &DesktopAppState,
    camera: &Camera,
    selection: &str,
    format: SceneExportFormat,
) -> Result<PreparedSceneExport> {
    let scene = state
        .active_scene()
        .ok_or_else(|| CueMolError::invalid_argument("no active scene"))?;
    if format == SceneExportFormat::Pov {
        return Ok(PreparedSceneExport {
            bytes: state
                .scene_pov_source_with_camera(scene, selection, camera)?
                .into_bytes(),
            mime: "text/plain;charset=utf-8",
        });
    }
    let mut snapshot = state.render_snapshot(scene, selection)?;
    state.apply_scene_color_proof_for(scene, &mut snapshot)?;
    let (bytes, mime) = match format {
        SceneExportFormat::Stl => (
            cuemol_render::StlSceneExporter::new().write_binary_stl_bytes_with_camera(
                &cuemol_render::tessellate_snapshot(&snapshot),
                camera,
            ),
            "model/stl",
        ),
        SceneExportFormat::Mqo => (
            cuemol_render::MqoSceneExporter::new()
                .write_snapshot_string_with_camera("scene", &snapshot, camera)
                .into_bytes(),
            "text/plain;charset=utf-8",
        ),
        SceneExportFormat::Pov => unreachable!(),
    };
    Ok(PreparedSceneExport { bytes, mime })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_scene_export(
    state: &DesktopAppState,
    camera: &Camera,
    selection: &str,
    format: SceneExportFormat,
    path: &std::path::Path,
) -> Result<()> {
    if format == SceneExportFormat::Pov {
        state.export_active_scene_pov_with_camera(path, selection, camera, Default::default())?;
    } else {
        let prepared = prepare_scene_export(state, camera, selection, format)?;
        std::fs::write(path, prepared.bytes)?;
    }
    Ok(())
}
