use crate::{DesktopAppState, GuiPngExportOptions};
use cuemol_command::image_export::{ImageExportKind, ImageExportTarget};
use cuemol_core::{CueMolError, Result};
use std::{
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) enum PreparedImageExport {
    Png(cuemol_desktop_core::image_export::PreparedPngExport),
    Umbreon(cuemol_command::PreparedRender),
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::Camera;
    use serde_json::json;

    #[test]
    fn cancelled_image_exports_keep_existing_bytes_and_remove_staging_files() {
        let mut state = DesktopAppState::new();
        state.ensure_active_scene();
        let target = ImageExportTarget::capture(&state, &Camera::new(), &json!({})).unwrap();
        let options = GuiPngExportOptions {
            width: 32,
            height: 24,
            resolution_dpi: 150.,
            transparent: true,
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.png");
        std::fs::write(&path, b"existing image").unwrap();
        for kind in [ImageExportKind::Png, ImageExportKind::Umbreon] {
            let prepared =
                PreparedImageExport::capture(&state, &target, kind, options, "").unwrap();
            assert!(prepared
                .write(&path, &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("cancelled"));
            assert_eq!(std::fs::read(&path).unwrap(), b"existing image");
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
        state.destroy_scene(target.scene);
        assert!(PreparedImageExport::capture(
            &state,
            &target,
            ImageExportKind::Umbreon,
            options,
            ""
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing image");
    }
}

impl PreparedImageExport {
    pub(crate) fn capture(
        state: &DesktopAppState,
        target: &ImageExportTarget,
        kind: ImageExportKind,
        options: GuiPngExportOptions,
        selection: &str,
    ) -> Result<Self> {
        target.validate(state)?;
        match kind {
            ImageExportKind::Png => Ok(Self::Png(
                cuemol_desktop_core::image_export::PreparedPngExport::capture(
                    state,
                    target.scene,
                    &target.camera,
                    options,
                    selection,
                    target.swap_stereo_eyes,
                )?,
            )),
            ImageExportKind::Umbreon => Ok(Self::Umbreon(target.prepare_umbreon(state, options)?)),
        }
    }

    pub(crate) fn write(self, path: &Path, cancel: &AtomicBool) -> Result<()> {
        let png = match self {
            Self::Png(job) => job.run(Some(cancel))?,
            Self::Umbreon(job) => job.run_png_with_cancel(cancel)?,
        };
        if cancel.load(Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Image export cancelled"));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending.write_all(&png)?;
        pending.flush()?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Image export cancelled"));
        }
        pending
            .persist(path)
            .map_err(|error| CueMolError::Io(error.error.to_string()))?;
        Ok(())
    }
}
