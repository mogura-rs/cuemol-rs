use crate::commands::{fit, loaded_result};
use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiRendererSetup};
use cuemol_qsys::Camera;
use serde_json::{json, Value};

#[path = "emdb.rs"]
mod emdb;

#[derive(Debug, Clone)]
enum FetchMetadata {
    Pdb { id: String },
    Emdb { id: String },
}

#[derive(Debug, Clone)]
pub struct FetchPlan {
    pub url: String,
    pub filename: String,
    pub name: String,
    pub arguments: Value,
    pub map_coefficients: bool,
    pub request_body: Option<Value>,
    pub emdb_id: Option<String>,
    metadata: Option<FetchMetadata>,
}
impl FetchPlan {
    pub fn is_metadata(&self) -> bool {
        self.metadata.is_some()
    }
    pub fn optional_metadata(&self) -> bool {
        matches!(self.metadata, Some(FetchMetadata::Emdb { .. }))
    }
}

pub fn advance_fetch(plan: &FetchPlan, status: u16, bytes: &[u8]) -> Result<Option<FetchPlan>> {
    emdb::advance(plan, status, bytes)
}

pub fn prepare_fetch(name: &str, args: &Value) -> Result<FetchPlan> {
    crate::validate_args(name, args).map_err(CueMolError::invalid_argument)?;
    if name == "fetch_emdb" {
        return emdb::prepare(args);
    }
    let map_coefficients = name == "fetch_map";
    let (url, filename, label) = if matches!(name, "fetch_pdb" | "fetch_map") {
        let id = args["pdbId"].as_str().unwrap().to_ascii_lowercase();
        if id.len() != 4
            || !id.as_bytes()[0].is_ascii_digit()
            || !id.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(CueMolError::invalid_argument("invalid PDB accession"));
        }
        if map_coefficients {
            let diff = args["mapType"] == "fofc";
            let label = format!("{id}_{}", if diff { "fofc" } else { "2fofc" });
            if args["source"] == "EBI_MTZ" {
                (
                    format!("https://www.ebi.ac.uk/pdbe/coordinates/files/{id}_map.mtz"),
                    format!("{id}.mtz"),
                    label,
                )
            } else {
                let suffix = if diff { "fo-fc" } else { "2fo-fc" };
                (format!("https://files.rcsb.org/pub/pdb/validation_reports/{}/{id}/{id}_validation_{suffix}_map_coef.cif.gz",&id[1..3]),format!("{id}.cif.gz"),label)
            }
        } else {
            let format = if args["format"] == "pdb" {
                "pdb"
            } else {
                "cif"
            };
            (
                format!(
                    "https://files.rcsb.org/download/{}.{format}",
                    id.to_ascii_uppercase()
                ),
                format!("{id}.{format}"),
                id.to_ascii_uppercase(),
            )
        }
    } else if name == "fetch_url" {
        let url = args["url"].as_str().unwrap();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(CueMolError::invalid_argument("URL must use HTTP or HTTPS"));
        }
        let format = match args["reader"].as_str().unwrap() {
            "mmcif" => "cif",
            "ccp4map" => "map",
            "apbs" => "dx",
            "xplormap" => "xplor",
            "mtzmap" => "mtz",
            s => s,
        };
        if !format.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CueMolError::invalid_argument("invalid reader"));
        }
        (
            url.to_string(),
            format!("download.{format}"),
            "download".into(),
        )
    } else {
        return Err(CueMolError::invalid_argument("not a download tool"));
    };
    Ok(FetchPlan {
        url,
        filename,
        name: args["name"].as_str().unwrap_or(&label).to_string(),
        arguments: args.clone(),
        map_coefficients,
        request_body: None,
        emdb_id: None,
        metadata: None,
    })
}

pub fn load_fetched(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    plan: &FetchPlan,
    bytes: &[u8],
) -> Result<Value> {
    let previous = state.clone();
    let previous_camera = camera.clone();
    let result = (|| {
        let args = &plan.arguments;
        if plan.map_coefficients {
            let diff = args["mapType"] == "fofc";
            let map = if plan.filename.ends_with(".mtz") {
                let mtz = cuemol_xtal::read_mtz_bytes(bytes)?;
                let column = |names: &[&str]| {
                    mtz.columns
                        .iter()
                        .position(|c| names.contains(&c.name.as_str()))
                        .ok_or_else(|| {
                            CueMolError::not_found(format!("map column {}", names.join("/")))
                        })
                };
                let f = column(if diff {
                    &["DELFWT", "FOFCWT"]
                } else {
                    &["FWT", "2FOFCWT"]
                })?;
                let phi = column(if diff {
                    &["PHDELWT", "PHFOFCWT"]
                } else {
                    &["PHWT", "PH2FOFCWT"]
                })?;
                cuemol_xtal::density_map_from_mtz(&mtz, f, Some(phi), -1., 1.)?
            } else {
                let decoded = cuemol_core::compressed::decode_bytes(bytes)?;
                let text =
                    std::str::from_utf8(&decoded).map_err(|e| CueMolError::parse(e.to_string()))?;
                cuemol_xtal::density_map_from_cif(text, -1., 1.)?
            };
            let scene = state.ensure_active_scene();
            let object = state.add_density_map(scene, &plan.name, map);
            let positive = state.add_density_renderer(object, "contour")?;
            crate::nodes::set_renderer_property(
                state,
                positive,
                "siglevel",
                if diff { "3" } else { "1" },
            )?;
            crate::nodes::set_renderer_property(
                state,
                positive,
                "color",
                if diff { "green" } else { "blue" },
            )?;
            if diff {
                let negative = state.add_density_renderer(object, "contour")?;
                crate::nodes::set_renderer_property(state, negative, "siglevel", "-3")?;
                crate::nodes::set_renderer_property(state, negative, "color", "red")?;
            }
            return Ok(json!({"objectId":object.get(),"name":plan.name}));
        }
        let loaded = if args["pdbId"].is_string() {
            let format = if args["format"] == "pdb" {
                "pdb"
            } else {
                "mmcif"
            };
            let mut reader =
                cuemol_desktop_core::molecule_reader_options_for_format(format).unwrap();
            crate::file_commands::apply_structure_reader_options(&mut reader, args);
            state.open_prepared(
                cuemol_desktop_core::PreparedFile::parse_with_molecule_options(
                    &plan.filename,
                    bytes,
                    &reader,
                )?,
            )?
        } else {
            state.open_bytes(&plan.filename, bytes)?
        };
        if let (Some(id), GuiLoadedPath::Map { map_uid, .. }) = (&plan.emdb_id, &loaded) {
            state.set_density_map_sampling(*map_uid, 1, "em")?;
            let setup = GuiRendererSetup {
                object_name: plan.name.clone(),
                renderer_type: "isosurf".into(),
                renderer_name: "isosurf".into(),
                selection: None,
                recenter: true,
                map_center_policy: None,
            };
            state.configure_loaded_bytes(&loaded, &setup, None)?;
            if let Some(level) = args["level"].as_f64() {
                let renderer = state.density_map_summary(*map_uid).unwrap().renderers[0].uid;
                crate::nodes::set_renderer_property(state, renderer, "use_abslevel", "true")?;
                crate::nodes::set_renderer_property(state, renderer, "level", &level.to_string())?;
            }
            crate::file_commands::apply_map_center_policy(
                state,
                camera,
                *map_uid,
                &json!({"mapCenterPolicy":"moveViewCenter"}),
            )?;
            return Ok(json!({"objectId":map_uid.get(),"name":plan.name,"emdbId":id}));
        }
        let default = if matches!(loaded, GuiLoadedPath::Map { .. }) {
            "contour"
        } else {
            "auto"
        };
        let kind = args["preset"]
            .as_str()
            .or(args["rendererType"].as_str())
            .unwrap_or(default);
        let setup = GuiRendererSetup {
            object_name: plan.name.clone(),
            renderer_type: kind.into(),
            renderer_name: args["rendererName"].as_str().unwrap_or(kind).into(),
            selection: args["selection"].as_str().map(str::to_string),
            recenter: args["centerView"] != false,
            map_center_policy: None,
        };
        state.configure_loaded_bytes(&loaded, &setup, None)?;
        match loaded {
            GuiLoadedPath::Object { object_uid, .. } if setup.recenter => fit(
                state,
                camera,
                Some(object_uid),
                args["selection"].as_str(),
                true,
            )?,
            GuiLoadedPath::Map { map_uid, .. } => {
                crate::file_commands::apply_map_center_policy(state, camera, map_uid, args)?;
            }
            GuiLoadedPath::Scene { .. } => *camera = state.active_view_camera().unwrap_or_default(),
            _ => {}
        }
        Ok(loaded_result(loaded))
    })();
    if result.is_err() {
        *state = previous;
        *camera = previous_camera;
    }
    result
}
