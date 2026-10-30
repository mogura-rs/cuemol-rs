use super::{
    pymol::{boolean, number, selections, tool},
    Action, RenderedImage, Session,
};
use base64::Engine;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

fn extension(path: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let path = path
        .strip_suffix(".gz")
        .or_else(|| path.strip_suffix(".bz2"))
        .or_else(|| path.strip_suffix(".xz"))
        .unwrap_or(path);
    Path::new(path)
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn reader(format: &str) -> Result<&str, String> {
    Ok(match format {
        "cif" => "mmcif",
        "ent" => "pdb",
        "ccp4" | "map" | "mrc" => "ccp4map",
        "xplor" => "xplormap",
        "mol" => "sdf",
        "top" => "amberprm",
        "dx" => "apbs",
        "mtz" => "mtzmap",
        "brk" => "pdb",
        "qdf" => "qdfmol",
        "pdb" | "mmcif" | "mmcifmap" | "ccp4map" | "xplormap" | "mtzmap" | "brixmap" | "sdf"
        | "mol2" | "apbs" | "pqr" | "amberprm" | "psf" | "gro" | "qsc" | "pse" | "qdfmol" => format,
        _ => return Err(format!("Unknown file reader: {format}")),
    })
}

pub(super) fn load<'a>(
    session: &Session,
    name: &str,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let mut actions = Vec::new();
    if name == "load" {
        let filename = arg("filename");
        let format = if arg("format").is_empty() {
            extension(filename)
        } else {
            arg("format").to_ascii_lowercase()
        };
        if format == "pml" {
            return session.read_script(filename);
        }
        let remote = filename.starts_with("http://") || filename.starts_with("https://");
        let format = reader(&format)?;
        let is_map = matches!(
            format,
            "ccp4map" | "xplormap" | "mtzmap" | "mmcifmap" | "brixmap" | "apbs"
        );
        let mut args = json!({"centerView":arg("zoom") != "0"});
        if !matches!(format, "qsc" | "pse") {
            args["rendererType"] = json!(if is_map { "contour" } else { "simple" });
            args["rendererName"] = json!(if is_map { "contour" } else { "pym:lines" });
            if is_map {
                args["mapCenterPolicy"] = json!("view");
            }
        }
        if !arg("object").is_empty() {
            args["name"] = json!(arg("object"));
        }
        if remote {
            args["url"] = json!(filename);
            args["reader"] = json!(format);
            actions.push(tool("fetch_url", args)?);
        } else {
            args["path"] = json!(session.resolve_path(filename).to_string_lossy());
            if !arg("format").is_empty() {
                args["reader"] = json!(format);
            }
            actions.push(tool("load_file", args)?);
        }
        return Ok(actions);
    }
    let kind = arg("type").to_ascii_lowercase();
    let assembly = kind
        .strip_prefix("pdb")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0);
    if !["", "cif", "mmcif", "pdb", "2fofc", "fofc"].contains(&kind.as_str()) && assembly.is_none()
    {
        return Err("Fetch types are cif, pdb, pdb1.., 2fofc and fofc".into());
    }
    let codes = arg("code")
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if codes.is_empty() {
        return Err("A PDB code is required".into());
    }
    if codes.len() > 1 && !arg("name").is_empty() {
        return Err("An object name cannot be used with multiple PDB codes".into());
    }
    for code in codes {
        if !code.is_ascii()
            || !(4..=5).contains(&code.len())
            || !code.as_bytes()[0].is_ascii_digit()
            || !code.bytes().all(|ch| ch.is_ascii_alphanumeric())
        {
            return Err(format!("Invalid PDB code: {code}"));
        }
        let id = code[..4].to_ascii_lowercase();
        let chain = &code[4..];
        if ["2fofc", "fofc"].contains(&kind.as_str()) {
            if !chain.is_empty() {
                return Err("A density map cannot be fetched for one chain".into());
            }
            actions.push(tool("fetch_map", json!({"pdbId":id,"mapType":kind,"name":if arg("name").is_empty() {format!("{id}_{kind}")} else {arg("name").to_string()}}))?);
            continue;
        }
        let label = if !arg("name").is_empty() {
            arg("name").to_string()
        } else if chain.is_empty() {
            id.clone()
        } else {
            code.into()
        };
        let mut args = json!({"rendererType":"simple","rendererName":"pym:lines","centerView":arg("zoom") != "0","name":label});
        if let Some(assembly) = assembly {
            args["url"] = json!(format!(
                "https://files.rcsb.org/download/{id}.pdb{assembly}"
            ));
            args["reader"] = json!("pdb");
            actions.push(tool("fetch_url", args)?);
        } else {
            args["pdbId"] = json!(id);
            args["format"] = json!(if kind == "pdb" { "pdb" } else { "mmcif" });
            actions.push(tool("fetch_pdb", args)?);
        }
        if !chain.is_empty() {
            actions.push(Action::KeepChain {
                name: label,
                chain: chain.into(),
            });
        }
    }
    Ok(actions)
}

fn pixels(value: &str, dpi: f64) -> Result<u32, String> {
    let value = value.trim().to_ascii_lowercase();
    let (value, scale) = if let Some(number) = value.strip_suffix("cm") {
        (number.trim(), dpi / 2.54)
    } else if let Some(number) = value.strip_suffix("in") {
        (number.trim(), dpi)
    } else {
        (value.as_str(), 1.)
    };
    if scale <= 0. {
        return Err("Image dimensions in in/cm require a positive DPI".into());
    }
    let value = number(if value.is_empty() { "0" } else { value })? * scale;
    if !(0. ..=8192.).contains(&value) {
        return Err("Image dimensions must be between zero and 8192 pixels".into());
    }
    Ok(value.round() as u32)
}

pub(super) fn image<'a>(
    session: &Session,
    state: &DesktopAppState,
    camera: &Camera,
    name: &str,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let dpi = if name == "png" {
        number(arg("dpi"))?
    } else {
        -1.
    };
    let mut width = pixels(arg("width"), dpi)?;
    let mut height = pixels(arg("height"), dpi)?;
    let path = if name == "ray" {
        None
    } else {
        let mut path = session.resolve_path(arg("filename"));
        if path.extension().is_none() {
            path.set_extension("png");
        }
        Some(path.to_string_lossy().into_owned())
    };
    let ray = name == "ray" || boolean(arg("ray"))?;
    if width == 0 && height == 0 && !ray {
        if let Some(bytes) = session.cached_render(state, camera) {
            return Ok(vec![Action::WriteFile {
                path: path.unwrap(),
                bytes,
                mime: "image/png".into(),
            }]);
        }
    }
    let view_width = state
        .runtime_value("viewport:width")
        .parse::<f64>()
        .unwrap_or(800.)
        .max(1.);
    let view_height = state
        .runtime_value("viewport:height")
        .parse::<f64>()
        .unwrap_or(600.)
        .max(1.);
    if width > 0 && height == 0 {
        height = (width as f64 * view_height / view_width).round().max(1.) as u32;
    } else if height > 0 && width == 0 {
        width = (height as f64 * view_width / view_height).round().max(1.) as u32;
    } else if width == 0 && height == 0 {
        width = view_width as u32;
        height = view_height as u32;
    }
    Ok(vec![Action::Render {
        width,
        height,
        ray,
        dpi: (dpi > 0.).then_some(dpi),
        path,
    }])
}

pub(super) fn save<'a>(
    session: &Session,
    state: &DesktopAppState,
    camera: &Camera,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let path = session
        .resolve_path(arg("filename"))
        .to_string_lossy()
        .into_owned();
    let format = if arg("format").is_empty() {
        extension(&path)
    } else {
        arg("format").to_ascii_lowercase()
    };
    if format == "qsc" {
        return Ok(vec![tool("save_scene", json!({"path":path}))?]);
    }
    if format == "png" {
        return image(session, state, camera, "png", &|key| match key {
            "filename" => arg("filename"),
            "dpi" => "-1",
            _ => "0",
        });
    }
    if matches!(format.as_str(), "stl" | "mqo" | "pov") {
        return Ok(vec![tool(
            "export_scene",
            json!({"path":path,"format":format}),
        )?]);
    }
    if !["pdb", "ent", "sdf", "mol", "pqr"].contains(&format.as_str()) {
        return Err(format!("Unsupported molecule output format: {format}"));
    }
    let (items, _) = selections(state, arg("selection"))?;
    let item = items.first().ok_or("No molecule to save")?;
    let mut actions = Vec::new();
    if items.len() > 1 {
        actions.push(Action::Output(format!(
            "Saving {} only; name a molecule to choose another.",
            item.name
        )));
    }
    let bytes = state
        .molecule_export_bytes(item.id, &item.expression, &format)
        .map_err(|e| e.to_string())?;
    actions.push(Action::WriteFile {
        path,
        bytes: Arc::new(bytes),
        mime: "chemical/x-pdb".into(),
    });
    Ok(actions)
}

impl Session {
    pub fn sync_revision(&mut self, revision: u64) {
        if self.revision != revision {
            self.render = None;
        }
        self.revision = revision;
    }

    pub fn accept_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    pub fn scene_changed(&mut self) {
        self.render = None;
    }

    fn cached_render(&self, state: &DesktopAppState, camera: &Camera) -> Option<Arc<Vec<u8>>> {
        self.render
            .as_ref()
            .filter(|image| {
                image.scene == state.active_scene()
                    && super::same_camera_view(&image.camera, camera)
            })
            .map(|image| image.bytes.clone())
    }

    pub fn remember_render(
        &mut self,
        state: &DesktopAppState,
        camera: &Camera,
        result: &Value,
    ) -> Result<Arc<Vec<u8>>, String> {
        let bytes = Arc::new(
            base64::engine::general_purpose::STANDARD
                .decode(
                    result["_image"]
                        .as_str()
                        .ok_or("Rendered image is missing")?,
                )
                .map_err(|e| e.to_string())?,
        );
        self.render = Some(RenderedImage {
            scene: state.active_scene(),
            camera: camera.clone(),
            bytes: bytes.clone(),
        });
        Ok(bytes)
    }

    pub(super) fn open_log(&mut self, filename: &str, mode: &str) -> Result<Vec<Action>, String> {
        if !["w", "a"].contains(&mode) {
            return Err("Log mode must be w or a".into());
        }
        if ["py", "pym"].contains(&extension(filename).as_str()) {
            return Err("Only command logs can be replayed; use a .pml file".into());
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = filename;
            Err("Command log files require the native application".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Write;
            let path = self.resolve_path(filename);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(mode == "w")
                .append(mode == "a")
                .open(&path)
                .map_err(|e| e.to_string())?;
            if mode == "a" {
                file.write_all(b"\n").map_err(|e| e.to_string())?;
            }
            self.command_log = Some(path.clone());
            Ok(vec![Action::Output(format!(
                "Logging to {}",
                path.display()
            ))])
        }
    }

    pub(super) fn close_log(&mut self) -> Vec<Action> {
        if self.command_log.take().is_some() {
            vec![Action::Output("Command log closed.".into())]
        } else {
            Vec::new()
        }
    }

    pub(super) fn log_line(&mut self, source: &str) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &self.command_log {
            use std::io::Write;
            let result = std::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .and_then(|mut file| {
                    file.write_all(source.as_bytes())?;
                    if !source.ends_with('\n') {
                        file.write_all(b"\n")?;
                    }
                    Ok(())
                });
            if let Err(error) = result {
                self.command_log = None;
                return Err(format!("Command log stopped: {error}"));
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = source;
        Ok(())
    }
}
