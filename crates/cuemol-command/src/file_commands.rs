use crate::commands::{fit, loaded_result, scene, text};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{
    DesktopAppState, GuiDensityMapReaderOptions, GuiFileRendererTarget, GuiLoadedPath,
    GuiRendererSetup,
};
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};

pub fn call_tool(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    match name {
        "load_pdb" | "load_mmcif" | "load_mtz" | "load_ccp4" | "load_msms" | "load_namd"
        | "load_amber" => load_file(state, camera, name, args),
        "fetch_map" => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                fetch_map(state, camera, args)
            }
            #[cfg(target_arch = "wasm32")]
            {
                Err(CueMolError::invalid_argument(
                    "Use the browser map download control",
                ))
            }
        }
        "paste_node" => paste(state, args),
        "calc_elepot" => crate::electrostatics::calculate(state, args),
        _ => Err(CueMolError::not_found(format!("tool {name}"))),
    }
}

pub fn load_file(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    let path = text(args, "path")?;
    if let Some(reader) = args["reader"].as_str() {
        let mut options = args.clone();
        options.as_object_mut().unwrap().remove("path");
        options["url"] = json!("https://localhost/local-file");
        options["reader"] = json!(reader);
        let mut plan = crate::prepare_fetch("fetch_url", &options)?;
        if args["name"].is_null() {
            plan.name = std::path::Path::new(path)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("molecule")
                .into();
        }
        let bytes = std::fs::read(path)?;
        return crate::load_fetched(state, camera, &plan, &bytes);
    }
    let Some(mut options) = state.file_renderer_setup_for_path(path)? else {
        let loaded = state.open_path(path)?;
        if let Some(saved) = state.active_view_camera() {
            *camera = saved;
        }
        return Ok(loaded_result(loaded));
    };
    if let Some(reader) = &mut options.molecule_reader_options {
        if let Some(v) = args["loadModel"].as_bool() {
            reader.ignore_multiple_models = !v;
        }
        if let Some(v) = args["loadAnisou"].as_bool() {
            reader.ignore_anisou = !v;
        }
        if let Some(v) = args["loadAltConf"].as_bool() {
            reader.ignore_alt_conf = !v;
        }
        if let Some(v) = args["loadSegid"].as_bool() {
            reader.load_segid_as_chain = v;
        }
        if let Some(v) = args["build2ndry"].as_bool() {
            reader.calculate_secondary_structure = v;
        }
        if let Some(v) = args["autoTopology"].as_bool() {
            reader.auto_generate_topology = v;
        }
    }
    if let Some(reader) = &mut options.namd_coor_reader_options {
        if let Some(v) = args["psfFile"].as_str() {
            reader.topology_path = v.into();
        }
    }
    if let Some(reader) = &mut options.msms_reader_options {
        if let Some(v) = args["vertFile"].as_str() {
            reader.vertex_path = v.into();
        }
    }
    if let Some(reader) = &mut options.density_map_reader_options {
        match reader {
            GuiDensityMapReaderOptions::Ccp4(reader) => {
                if let Some(v) = args["normalize"].as_bool() {
                    reader.normalize = v;
                }
                if let Some(v) = args["truncateMin"].as_f64() {
                    reader.truncate_min = true;
                    reader.min = v;
                }
                if let Some(v) = args["truncateMax"].as_f64() {
                    reader.truncate_max = true;
                    reader.max = v;
                }
            }
            GuiDensityMapReaderOptions::Mtz(reader) => {
                if let Some(v) = args["columnF"].as_str() {
                    reader.column_f = v.into();
                }
                if let Some(v) = args["columnPhi"].as_str() {
                    reader.use_phase = !v.is_empty();
                    reader.column_phi = v.into();
                }
                if let Some(v) = args["columnWeight"].as_str() {
                    reader.use_weight = !v.is_empty();
                    reader.column_weight = v.into();
                }
                if let Some(v) = args["resolutionLimit"].as_f64() {
                    reader.resolution = v;
                }
                if let Some(v) = args["gridSpacing"].as_f64() {
                    reader.grid_size = v;
                }
            }
        }
    }
    let default_kind = match options.target {
        GuiFileRendererTarget::Molecule => "auto",
        GuiFileRendererTarget::DensityMap => "contour",
        GuiFileRendererTarget::Surface => "molsurf",
    };
    let kind = args["preset"]
        .as_str()
        .or(args["rendererType"].as_str())
        .unwrap_or(default_kind);
    let label = args["name"].as_str().map(str::to_owned).unwrap_or_else(|| {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("object")
            .into()
    });
    let setup = GuiRendererSetup {
        object_name: label,
        renderer_type: kind.into(),
        renderer_name: args["rendererName"].as_str().unwrap_or(kind).into(),
        selection: args["selection"].as_str().map(str::to_owned),
        recenter: args["centerView"] != false,
    };
    let loaded = match options.target {
        GuiFileRendererTarget::Molecule => state.open_path_with_file_reader_and_renderer_setup(
            path,
            options.molecule_reader_options.as_ref(),
            options.namd_coor_reader_options.as_ref(),
            &setup,
        )?,
        GuiFileRendererTarget::DensityMap => {
            let center = (args["mapCenterPolicy"] == "view").then(|| camera.center());
            let loaded = state.open_density_map_path_with_renderer_setup(
                path,
                options.density_map_reader_options.as_ref(),
                center,
                &setup,
            )?;
            if let GuiLoadedPath::Map { map_uid, .. } = &loaded {
                let stride = crate::commands::integer(args, "subsample", 1)?;
                if stride == 0 {
                    return Err(CueMolError::invalid_argument("subsample must be positive"));
                }
                state.set_density_map_sampling(
                    *map_uid,
                    stride,
                    args["mapType"].as_str().unwrap_or("auto"),
                )?;
            }
            loaded
        }
        GuiFileRendererTarget::Surface => state.open_surface_path_with_renderer_setup(
            path,
            options.msms_reader_options.as_ref(),
            &setup,
        )?,
    };
    if let GuiLoadedPath::Object { object_uid, .. } = &loaded {
        if name == "load_amber" {
            if let Some(path) = args["coordFile"].as_str() {
                let restart = cuemol_mdtools::read_amber_restart_file(path)?;
                let object = state.scene_manager_mut().object_mut(*object_uid).unwrap();
                cuemol_mdtools::apply_amber_restart(object.mol_mut(), &restart)?;
            }
        }
        if setup.recenter {
            fit(
                state,
                camera,
                Some(*object_uid),
                args["selection"].as_str(),
                true,
            )?;
        }
    } else if let GuiLoadedPath::Map { map_uid, .. } = &loaded {
        if setup.recenter {
            let mut view = View::new(Uid::new(1), scene(state)?);
            view.set_camera(camera);
            view.set_view_size(800, 600);
            state.fit_density_map_view(*map_uid, &mut view)?;
            *camera = view.camera().clone();
        }
    }
    Ok(loaded_result(loaded))
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_map(state: &mut DesktopAppState, _camera: &mut Camera, args: &Value) -> Result<Value> {
    let id = text(args, "pdbId")?.to_ascii_lowercase();
    if id.len() != 4
        || !id.bytes().next().is_some_and(|c| c.is_ascii_digit())
        || !id.bytes().all(|c| c.is_ascii_alphanumeric())
    {
        return Err(CueMolError::invalid_argument("invalid PDB accession"));
    }
    let diff = args["mapType"] == "fofc";
    let map_type = if diff { "fofc" } else { "2fofc" };
    let map = if args["source"] == "EBI_MTZ" {
        let url = format!("https://www.ebi.ac.uk/pdbe/coordinates/files/{id}_map.mtz");
        let path = crate::transport::download(&url, "mtz")?;
        let mtz = cuemol_xtal::read_mtz_file(&path)?;
        let column = |names: &[&str]| {
            mtz.columns
                .iter()
                .position(|c| names.contains(&c.name.as_str()))
                .ok_or_else(|| {
                    CueMolError::not_found(format!("map coefficient column {}", names.join("/")))
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
        let suffix = if diff { "fo-fc" } else { "2fo-fc" };
        let url=format!("https://files.rcsb.org/pub/pdb/validation_reports/{}/{id}/{id}_validation_{suffix}_map_coef.cif.gz",&id[1..3]);
        let path = crate::transport::download(&url, "cif.gz")?;
        let input = cuemol_molstr::read_text_maybe_compressed(&path)?;
        cuemol_xtal::density_map_from_cif(&input, -1., 1.)?
    };
    let sid = state.ensure_active_scene();
    let label = args["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{id}_{map_type}"));
    let object = state.add_density_map(sid, &label, map);
    let renderers = state.density_map_summary(object).unwrap().renderers;
    for r in renderers {
        state.destroy_density_renderer(object, r.uid)?;
    }
    let positive = state.add_density_renderer(object, "contour")?;
    for (key, value) in [
        ("siglevel", if diff { "3" } else { "1" }),
        ("color", if diff { "green" } else { "blue" }),
    ] {
        crate::nodes::set_renderer_property(state, positive, key, value)?;
    }
    if diff {
        let negative = state.add_density_renderer(object, "contour")?;
        crate::nodes::set_renderer_property(state, negative, "siglevel", "-3")?;
        crate::nodes::set_renderer_property(state, negative, "color", "red")?;
    }
    Ok(json!({"objectId":object.get(),"name":label}))
}

fn paste(state: &mut DesktopAppState, args: &Value) -> Result<Value> {
    use cuemol_desktop_core::GuiPastedNode;
    let pasted = state.paste_command_xml(
        text(args, "kind")?,
        text(args, "form")?,
        text(args, "xml")?,
        args["target"].as_u64().map(Uid::new),
        args["targetType"].as_str(),
    )?;
    Ok(match pasted {
        GuiPastedNode::Camera(name) => json!({"name":name}),
        GuiPastedNode::Style { index, name } => json!({"index":index,"name":name}),
        GuiPastedNode::Object(id) => json!({"objectId":id.get()}),
        GuiPastedNode::Renderers { uids, group } => {
            json!({"rendererIds":uids.into_iter().map(Uid::get).collect::<Vec<_>>(),"groupId":group.map(Uid::get)})
        }
    })
}
