use crate::commands::{fit, loaded_result};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiRendererSetup};
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct FetchPlan {
    pub url: String,
    pub filename: String,
    pub name: String,
    pub arguments: Value,
    pub map_coefficients: bool,
}
pub fn prepare_fetch(name: &str, args: &Value) -> Result<FetchPlan> {
    crate::validate_args(name, args).map_err(CueMolError::invalid_argument)?;
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
        let loaded = state.open_bytes(&plan.filename, bytes)?;
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
        };
        state.configure_loaded_bytes(
            &loaded,
            &setup,
            (args["mapCenterPolicy"] == "view").then(|| camera.center()),
        )?;
        match loaded {
            GuiLoadedPath::Object { object_uid, .. } if setup.recenter => fit(
                state,
                camera,
                Some(object_uid),
                args["selection"].as_str(),
                true,
            )?,
            GuiLoadedPath::Map { scene_uid, map_uid } if setup.recenter => {
                let mut view = View::new(Uid::new(1), scene_uid);
                view.set_camera(camera);
                view.set_view_size(800, 600);
                state.fit_density_map_view(map_uid, &mut view)?;
                *camera = view.camera();
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
