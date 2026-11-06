use crate::commands::{fit, loaded_result, text};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::{
    DesktopAppState, GuiDensityMapReaderOptions, GuiFileRendererTarget, GuiLoadedPath,
    GuiRendererSetup,
};
use cuemol_qsys::Camera;
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
        "fetch_emdb" => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                crate::transport::fetch_emdb(state, camera, args)
            }
            #[cfg(target_arch = "wasm32")]
            {
                Err(CueMolError::invalid_argument(
                    "Use fetch_emdb in the browser console",
                ))
            }
        }
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
        "copy_node" => copy(state, args),
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
    map_center_policy(args)?;
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
        apply_structure_reader_options(reader, args);
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
                reader.subsample = crate::commands::integer(args, "subsample", 1)?;
                if reader.subsample == 0 {
                    return Err(CueMolError::invalid_argument("subsample must be positive"));
                }
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
            let loaded = state.open_density_map_path_with_renderer_setup(
                path,
                options.density_map_reader_options.as_ref(),
                None,
                &setup,
            )?;
            if let GuiLoadedPath::Map { map_uid, .. } = &loaded {
                state.set_density_map_sampling(
                    *map_uid,
                    1,
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
        apply_map_center_policy(state, camera, *map_uid, args)?;
    }
    Ok(loaded_result(loaded))
}

pub(crate) fn map_center_policy(args: &Value) -> Result<&str> {
    let policy = args["mapCenterPolicy"].as_str().unwrap_or("auto");
    if matches!(policy, "auto" | "moveViewCenter" | "setMapCenter" | "view") {
        Ok(policy)
    } else {
        Err(CueMolError::invalid_argument("invalid map center policy"))
    }
}

pub(crate) fn apply_map_center_policy(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    map_uid: Uid,
    args: &Value,
) -> Result<()> {
    let renderers = state
        .density_map_summary(map_uid)
        .ok_or_else(|| CueMolError::not_found("density map"))?
        .renderers
        .into_iter()
        .map(|renderer| renderer.uid)
        .collect::<Vec<_>>();
    apply_map_renderer_center_policy(state, camera, map_uid, &renderers, args)
}

pub(crate) fn apply_map_renderer_center_policy(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    map_uid: Uid,
    renderers: &[Uid],
    args: &Value,
) -> Result<()> {
    let policy = map_center_policy(args)?;
    if state.density_map_kind(map_uid).is_none() {
        return Ok(());
    }
    let move_view = match policy {
        "auto" => state
            .density_map_kind(map_uid)
            .is_some_and(|(_, kind)| kind == "em"),
        "moveViewCenter" => true,
        _ => false,
    };
    if move_view {
        let mut view = crate::properties::current_fit_view(state, camera)?;
        state.fit_density_map_view(map_uid, &mut view)?;
        *camera = view.camera();
    } else {
        for &renderer in renderers {
            state.set_density_renderer_center(map_uid, renderer, camera.center())?;
        }
    }
    Ok(())
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

fn copy(state: &DesktopAppState, args: &Value) -> Result<Value> {
    let id = crate::commands::uid(args, "nodeId")?;
    let payload = if text(args, "nodeType")? == "object" {
        let scene = crate::commands::scene(state)?;
        if state.scene_manager().scene_uid_for_object(id) != Some(scene)
            && state.scene_manager().scene_uid_for_surface(id) != Some(scene)
            && state.scene_manager().scene_uid_for_lw_object(id) != Some(scene)
            && !state
                .density_map_summary(id)
                .is_some_and(|map| map.scene_uid == scene)
        {
            return Err(CueMolError::not_found("object in active scene"));
        }
        state.copy_workspace_object(id)?.to_text_payload()?
    } else {
        crate::clipboard::copy_payload(state, &crate::clipboard::renderer_target(state, id)?)?
    };
    let xml = String::from_utf8(payload.bytes)
        .map_err(|_| CueMolError::parse("Clipboard data is not UTF-8"))?;
    Ok(json!({"kind":payload.kind,"form":payload.form,"xml":xml}))
}

pub fn scene_save_options(
    state: &DesktopAppState,
    args: &Value,
) -> Result<cuemol_desktop_core::GuiSceneSaveOptions> {
    use cuemol_desktop_core::{GuiQscCompression as Compression, GuiQscVersion as Version};
    let mut options = state.active_scene_save_options()?;
    let encoding = !args["compress"].is_null() || !args["base64"].is_null();
    options.version = match args["version"].as_str() {
        Some("QDF0") if encoding => {
            return Err(CueMolError::invalid_argument(
                "compress and base64 need version QDF1.",
            ))
        }
        Some("QDF0") => Version::Qdf0,
        Some("QDF1") => Version::Qdf1,
        Some(_) => return Err(CueMolError::invalid_argument("Invalid QSC version")),
        None if encoding => Version::Qdf1,
        None => options.version,
    };
    if let Some(embed) = args["embedAll"].as_bool() {
        options.embed_all = embed;
    }
    if let Some(base64) = args["base64"].as_bool() {
        options.base64 = base64;
    }
    if let Some(compression) = args["compress"].as_str() {
        options.compression = match compression {
            "none" => Compression::None,
            "gzip" => Compression::Gzip,
            "xzip" => Compression::Xz,
            _ => return Err(CueMolError::invalid_argument("Invalid QSC compression")),
        };
    }
    if options.version == Version::Qdf0 {
        options.compression = Compression::None;
        options.base64 = false;
    }
    Ok(options)
}

pub(crate) fn apply_structure_reader_options(
    reader: &mut cuemol_desktop_core::GuiMoleculeReaderOptions,
    args: &Value,
) {
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
