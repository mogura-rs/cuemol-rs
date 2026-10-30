use crate::commands::{fit, integer, renderer_owner, scene, text, uid};
use crate::nodes;
use cuemol_core::{CueMolError, LDom2Node, Matrix4, Result, Uid};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::qsys::{AtomIntrKind, AtomIntrLine};
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn number(args: &Value, key: &str, default: f64) -> f64 {
    args[key].as_f64().unwrap_or(default)
}
fn selection(args: &Value) -> &str {
    args["selection"].as_str().unwrap_or("*")
}

pub fn call_tool(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    match name {
        "get_hatch_style" => {
            let name = args["style"].as_str().unwrap_or("richardson");
            Ok(
                json!({"style":name,"spec":cuemol_render::hatch_style_spec(name)?,"styles":cuemol_render::HATCH_STYLES}),
            )
        }
        "delete_atoms" => {
            Ok(json!({"deleted":state.delete_atoms_in_object(uid(args,"objId")?,selection(args))?}))
        }
        "rename_chain" => Ok(
            json!({"changed":state.change_chain_in_object(uid(args,"objId")?,selection(args),text(args,"chain")?)?}),
        ),
        "renumber_residues" => {
            let value = i32::try_from(
                args["value"]
                    .as_i64()
                    .ok_or_else(|| CueMolError::invalid_argument("invalid residue number"))?,
            )
            .map_err(|_| CueMolError::invalid_argument("residue number overflow"))?;
            Ok(
                json!({"changed":state.change_residue_indices_in_object(uid(args,"objId")?,selection(args),args["mode"]=="shift",value,args["renumber"]==true)?}),
            )
        }
        "merge_molecules" => Ok(
            json!({"atoms":state.merge_molecules(uid(args,"fromId")?,selection(args),uid(args,"toId")?,args["copy"]==true)?}),
        ),
        "set_secondary_structure" => {
            let id = uid(args, "objId")?;
            let changed = if args["selection"].is_null() {
                state.assign_secondary_structure_in_object(
                    id,
                    args["ignoreBulge"] == true,
                    number(args, "helixGapAngle", 0.),
                )?
            } else {
                let kind = match text(args, "type")? {
                    "coil" => 0,
                    "strand" => 1,
                    "helix" => 2,
                    "helix310" => 3,
                    "helixpi" => 4,
                    _ => return Err(CueMolError::invalid_argument("secondary structure type")),
                };
                state.set_secondary_structure_in_object(id, selection(args), kind)?
            };
            Ok(json!({"changed":changed}))
        }
        "set_symmetry" => {
            state.change_crystal_info(
                uid(args, "objId")?,
                number(args, "a", 0.),
                number(args, "b", 0.),
                number(args, "c", 0.),
                number(args, "alpha", 90.),
                number(args, "beta", 90.),
                number(args, "gamma", 90.),
                i32::try_from(integer(args, "spaceGroup", 1)?)
                    .map_err(|_| CueMolError::invalid_argument("invalid space group"))?,
            )?;
            Ok(json!({"objectId":args["objId"]}))
        }
        "show_unit_cell" => {
            Ok(json!({"rendererId":state.show_molecule_unit_cell(uid(args,"objId")?)?.get()}))
        }
        "show_symmetry" => {
            let id = uid(args, "objId")?;
            let r = state.show_molecule_symmetry(id, args["extent"].as_f64(), camera.center())?;
            if args["unitCell"] == true {
                state.show_molecule_unit_cell(id)?;
            }
            Ok(json!({"rendererId":r.get()}))
        }
        "create_symmetry_copy" => {
            let source = uid(args, "objId")?;
            let values = text(args, "matrix")?
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(str::parse::<f64>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| CueMolError::invalid_argument("invalid matrix"))?;
            if values.len() != 16 || values.iter().any(|n| !n.is_finite()) {
                return Err(CueMolError::invalid_argument(
                    "matrix requires 16 finite row-major numbers",
                ));
            }
            let mut matrix = Matrix4::new();
            for row in 0..4 {
                for col in 0..4 {
                    matrix.set_at(row + 1, col + 1, values[row * 4 + col])?;
                }
            }
            let mut mol = state
                .scene_manager()
                .object(source)
                .ok_or_else(|| CueMolError::not_found("molecule"))?
                .mol()
                .clone();
            mol.xform_by_mat(matrix, None);
            let sid = scene(state)?;
            let id = state
                .scene_manager_mut()
                .scene_mut(sid)
                .unwrap()
                .add_molecule(text(args, "name")?, mol);
            let kind = args["preset"]
                .as_str()
                .or(args["rendererType"].as_str())
                .unwrap_or("simple");
            state.new_renderer_from_setup(
                sid,
                id,
                kind,
                args["rendererName"].as_str().unwrap_or(kind),
                args["selection"].as_str(),
            )?;
            if args["centerView"] == true {
                fit(state, camera, Some(id), None, true)?;
            }
            Ok(json!({"objectId":id.get()}))
        }
        "create_group" => Ok(
            json!({"rendererId":state.create_renderer_group(uid(args,"objId")?,args["name"].as_str().unwrap_or("group"))?.get()}),
        ),
        "create_surface" => {
            let id = uid(args, "objId")?;
            let sid = scene(state)?;
            let label = args["name"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or(state.suggest_object_name(sid, "surface")?);
            let surf = state.create_molecular_surface_object_with_backend(
                id,
                selection(args),
                &label,
                number(args, "density", 2.),
                number(args, "probeRadius", 1.4),
                args["backend"].as_str().unwrap_or("auto"),
            )?;
            Ok(
                json!({"objectId":surf.get(),"name":label,"backend":state.molecular_surface_backend(surf)}),
            )
        }
        "recalc_surface" => {
            let id = uid(args, "surfId")?;
            let count = state.regenerate_surface_object_with_backend(
                id,
                number(args, "density", -1.),
                args["backend"].as_str(),
            )?;
            Ok(
                json!({"objectId":id.get(),"triangles":count,"backend":state.molecular_surface_backend(id)}),
            )
        }
        "create_surface_from_map" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            Ok(json!({"objectId":state.generate_density_surface_object(owner,id)?.get()}))
        }
        "cut_surface" => {
            let (normal, position) = DesktopAppState::clipping_plane_from_camera(camera);
            let r = state.cut_loaded_surface_by_plane(
                uid(args, "surfId")?,
                number(args, "density", 5.),
                normal,
                position,
                args["mode"].as_str().unwrap_or("full"),
            )?;
            Ok(
                json!({"objectId":r.target_uid.get(),"sectionId":r.section_uid.map(Uid::get),"triangles":r.target_triangle_count,"sectionTriangles":r.section_triangle_count}),
            )
        }
        "analyze_interactions" => {
            let r = state.define_atom_contacts(
                uid(args, "objId")?,
                selection(args),
                args["obj2Id"].as_u64().map(Uid::new),
                args["selection2"].as_str(),
                number(args, "minDist", 1.5),
                number(args, "maxDist", 4.),
                args["includeCarbon"] != true,
                integer(args, "maxLabels", 100)?,
                args["labelSet"].as_str().unwrap_or("contacts"),
            )?;
            Ok(json!({"count":r.count,"rendererId":r.renderer_uid.get()}))
        }
        "list_interactions" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            Ok(
                json!({"entries":state.atom_intr_def_summaries(owner,id)?.iter().enumerate().map(|(i,r)|json!({"number":i+1,"id":r.id,"mode":r.mode,"atoms":r.atoms})).collect::<Vec<_>>()}),
            )
        }
        "remove_interaction" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            let list = state.atom_intr_def_summaries(owner, id)?;
            let n = integer(args, "number", 0)?
                .checked_sub(1)
                .ok_or_else(|| CueMolError::invalid_argument("interaction numbers start at 1"))?;
            let entry = list
                .get(n)
                .ok_or_else(|| CueMolError::not_found("interaction"))?;
            Ok(json!({"removed":state.remove_atom_intr_defs(owner,id,&[entry.id])?}))
        }
        "list_map_contours" => {
            let mut rows = Vec::new();
            for map in state.active_density_map_summaries() {
                let unit = state.density_map_level_unit(map.uid)?;
                for r in map.renderers {
                    rows.push(json!({"objectId":map.uid.get(),"rendId":r.uid.get(),"name":r.name,"type":r.renderer_type,"visible":r.visible,"alpha":r.alpha,"unit":unit,"level":r.specific.get("siglevel").and_then(|s|s.parse::<f64>().ok()),"properties":r.specific}));
                }
            }
            Ok(json!({"contours":rows}))
        }
        "set_map_contour" => {
            let id = uid(args, "rendId")?;
            for (arg, prop) in [
                ("level", "siglevel"),
                ("extent", "extent"),
                ("color", "color"),
                ("alpha", "alpha"),
            ] {
                if !args[arg].is_null() {
                    nodes::set_renderer_property(
                        state,
                        id,
                        prop,
                        &args[arg]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or(args[arg].to_string()),
                    )?;
                }
            }
            let owner = renderer_owner(state, id)?;
            Ok(json!({"rendId":id.get(),"unit":state.density_map_level_unit(owner)?}))
        }
        "recenter_map" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            state.set_density_renderer_center(owner, id, camera.center())?;
            Ok(json!({"rendId":id.get()}))
        }
        "list_renderer_styles" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            let (sid, _, kind, _) = state.workspace_renderer_style_state(owner, id)?;
            Ok(
                json!({"styles":state.renderer_style_options(sid,&kind).iter().map(|s|json!({"name":s.name,"description":s.description})).collect::<Vec<_>>()}),
            )
        }
        "apply_renderer_style" | "set_renderer_styles" => {
            let id = uid(args, "rendId")?;
            let owner = renderer_owner(state, id)?;
            if name == "apply_renderer_style" {
                state.apply_renderer_style_option(owner, id, text(args, "style")?, "renderer")?;
            } else {
                state.set_workspace_renderer_styles(owner, id, text(args, "styles")?)?;
            }
            Ok(json!({"rendererId":id.get()}))
        }
        "rename_camera" | "move_camera" => {
            let sid = scene(state)?;
            let source = text(args, "name")?;
            let s = state.scene_manager_mut().scene_mut(sid).unwrap();
            let mut cameras = s.cameras().to_vec();
            let from = cameras
                .iter()
                .position(|c| c.name() == source)
                .ok_or_else(|| CueMolError::not_found("camera"))?;
            if name == "rename_camera" {
                let label = text(args, "newName")?;
                if label.trim().is_empty() || cameras.iter().any(|c| c.name() == label) {
                    return Err(CueMolError::invalid_argument(
                        "camera name is empty or already used",
                    ));
                }
                cameras[from].set_name(label);
            } else {
                let to = nodes::move_target(args, from, cameras.len())?;
                let c = cameras.remove(from);
                cameras.insert(to, c);
            }
            s.replace_cameras(cameras);
            Ok(json!({"name":args["newName"].as_str().unwrap_or(source)}))
        }
        "clear_camera_visibility" => Ok(
            json!({"cleared":state.clear_scene_camera_visibility_settings(scene(state)?,text(args,"name")?)?}),
        ),
        "set_camera_visibility" => {
            let mut entries = Vec::new();
            for part in text(args, "entries")?
                .split(';')
                .map(str::trim)
                .filter(|p| !p.is_empty())
            {
                let (flag, path) = part.split_once(char::is_whitespace).ok_or_else(|| {
                    CueMolError::invalid_argument("expected on/off and node path")
                })?;
                if !matches!(flag, "on" | "off") {
                    return Err(CueMolError::invalid_argument(
                        "visibility must be on or off",
                    ));
                }
                let (id, object) = resolve_node(state, path.trim())?;
                entries.push((id, flag == "on", object));
            }
            Ok(
                json!({"entries":state.set_scene_camera_visibility_settings(scene(state)?,text(args,"name")?,&entries)?}),
            )
        }
        "save_object" => {
            state.save_workspace_object_as(
                nodes::object_kind(state, uid(args, "objId")?)?,
                uid(args, "objId")?,
                text(args, "fileName")?,
            )?;
            Ok(json!({"path":args["fileName"]}))
        }
        "convert_to_morph" => {
            let (r, created) = state.initialize_morph_object(uid(args, "objId")?)?;
            Ok(json!({"objectId":r.object_uid.get(),"frames":r.frame_count,"created":created}))
        }
        "list_morph_frames" => {
            let id = uid(args, "objId")?;
            Ok(
                json!({"objectId":id.get(),"value":state.morph_frame_value(id),"frames":state.morph_frame_summaries(id).iter().map(|f|json!({"number":f.index+1,"name":f.name,"source":f.source})).collect::<Vec<_>>()}),
            )
        }
        "add_morph_frame" => {
            let id = uid(args, "objId")?;
            let before = args["before"]
                .as_u64()
                .map(|i| {
                    i.checked_sub(1)
                        .ok_or_else(|| CueMolError::invalid_argument("frame numbers start at 1"))
                })
                .transpose()?
                .map(|i| i as usize);
            if args["file"].is_null() == args["fromObjId"].is_null() {
                return Err(CueMolError::invalid_argument(
                    "give exactly one of file or fromObjId",
                ));
            }
            let r = if let Some(path) = args["file"].as_str() {
                state.insert_morph_frame_from_path(id, path, "", before)?
            } else {
                state.insert_morph_frame_from_object(
                    id,
                    uid(args, "fromObjId")?,
                    "frame",
                    before,
                )?
            };
            Ok(json!({"objectId":id.get(),"frames":r.frame_count}))
        }
        "remove_morph_frame" => {
            let frame = integer(args, "frame", 0)?
                .checked_sub(1)
                .ok_or_else(|| CueMolError::invalid_argument("frame numbers start at 1"))?;
            let r = state.remove_morph_frame(uid(args, "objId")?, frame)?;
            Ok(json!({"frames":r.frame_count}))
        }
        "add_bond" | "remove_bond" | "set_atom_label" | "set_atom_labels" | "measure_geometry"
        | "measure_selections" => molecule_annotations(state, name, args),
        "list_paint" | "set_paint" | "add_paint" | "add_paint_at" | "remove_paint"
        | "move_paint" | "clear_paint" | "set_default_color" | "set_coloring_prop" => {
            coloring(state, name, args)
        }
        "set_multigrad_nodes" | "set_elepot_coloring" => surface_coloring(state, name, args),
        _ => crate::properties::call_tool(state, camera, name, args),
    }
}

pub fn resolve_node(state: &DesktopAppState, path: &str) -> Result<(Uid, bool)> {
    let data = crate::commands::scene_state(state)?;
    if let Some(id) = path.strip_prefix('#').and_then(|s| s.parse::<u64>().ok()) {
        let id = Uid::new(id);
        let object = data["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == id.get());
        if object || renderer_owner(state, id).is_ok() {
            return Ok((id, object));
        }
    }
    let (object_name, renderer_name) = path
        .split_once('/')
        .map_or((path, None), |(a, b)| (a, Some(b)));
    let objects = data["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["name"] == object_name)
        .collect::<Vec<_>>();
    if objects.len() != 1 {
        return Err(CueMolError::invalid_argument(format!(
            "object name is missing or ambiguous: {object_name}"
        )));
    }
    let object = objects[0];
    if let Some(name) = renderer_name {
        let renderers = object["renderers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["name"] == name)
            .collect::<Vec<_>>();
        if renderers.len() != 1 {
            return Err(CueMolError::invalid_argument(format!(
                "renderer name is missing or ambiguous: {name}"
            )));
        }
        return Ok((Uid::new(renderers[0]["id"].as_u64().unwrap()), false));
    }
    Ok((Uid::new(object["id"].as_u64().unwrap()), true))
}

fn atom_indices(state: &DesktopAppState, id: Uid, args: &Value) -> Result<Vec<usize>> {
    let object = state
        .scene_manager()
        .object(id)
        .ok_or_else(|| CueMolError::not_found("molecule"))?;
    let specs = args["atoms"]
        .as_array()
        .ok_or_else(|| CueMolError::invalid_argument("atoms must be an array"))?;
    specs
        .iter()
        .map(|spec| {
            let chain = text(spec, "chain")?;
            let residue = text(spec, "resid")?;
            let name = text(spec, "atomName")?;
            object
                .mol()
                .atoms()
                .iter()
                .position(|a| {
                    a.chain_id == chain
                        && format!(
                            "{}{}",
                            a.res_id,
                            a.ins_code.map(|c| c.to_string()).unwrap_or_default()
                        ) == residue
                        && a.name.trim() == name
                        && spec.get("altConf").is_none_or(|value| {
                            a.alt_loc == value.as_str().and_then(|s| s.chars().next())
                        })
                })
                .ok_or_else(|| CueMolError::not_found(format!("atom {chain}.{residue}.{name}")))
        })
        .collect()
}

fn molecule_annotations(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let id = uid(args, "objId")?;
    if name == "measure_geometry" || name == "measure_selections" {
        return measure(state, name, args);
    }
    let indices = if name == "set_atom_labels" {
        state
            .scene_manager()
            .object(id)
            .ok_or_else(|| CueMolError::not_found("molecule"))?
            .mol()
            .select_indices(&state.parse_object_selector(id, selection(args))?)
    } else {
        atom_indices(state, id, args)?
    };
    if name == "add_bond" || name == "remove_bond" {
        if indices.len() != 2 || indices[0] == indices[1] {
            return Err(CueMolError::invalid_argument(
                "exactly two distinct atoms are required",
            ));
        }
        let object = state.scene_manager().object(id).unwrap();
        let a = object.mol().atoms()[indices[0]].serial;
        let b = object.mol().atoms()[indices[1]].serial;
        let changed = if name == "add_bond" {
            state.make_bond_in_object(id, a, b)?
        } else {
            state.remove_bond_in_object(id, a, b)?
        };
        return Ok(json!({"changed":changed}));
    }
    if name == "set_atom_label" && indices.len() != 1 {
        return Err(CueMolError::invalid_argument(
            "exactly one atom is required",
        ));
    }
    let format = args["format"].as_str().unwrap_or("");
    cuemol_molstr::label_format::LabelFormat::new(format)?;
    if name == "set_atom_labels" && !args["format"].is_null() && indices.is_empty() {
        return Err(CueMolError::not_found("atoms matching label selection"));
    }
    let label = args["rendererName"].as_str().unwrap_or("");
    let object = state.scene_manager_mut().object_mut(id).unwrap();
    let show = if name == "set_atom_label" {
        args["on"] == true
    } else {
        !args["format"].is_null()
    };
    let renderer = object
        .renderers()
        .iter()
        .find(|r| r.renderer_type() == "*namelabel" && r.name() == label)
        .map(|r| r.uid());
    let renderer = match renderer {
        Some(id) => id,
        None if show => {
            let id = object.add_renderer("*namelabel", label);
            object
                .renderer_mut(id)
                .unwrap()
                .set_qsc_attr("style", "DefaultLabel");
            id
        }
        None => return Ok(json!({"labels":0})),
    };
    let labels = indices
        .iter()
        .map(|index| crate::navigation::atom_reference(&object.mol().atoms()[*index]))
        .collect::<Vec<_>>();
    let r = object.renderer_mut(renderer).unwrap();
    let mut max_labels = r
        .qsc_attr("maxlabel")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50);
    if show && name == "set_atom_labels" {
        max_labels = max_labels.saturating_add(labels.len());
        r.set_qsc_attr("maxlabel", max_labels.to_string());
    }
    let mut nodes = r.qsc_children().to_vec();
    nodes.retain(|n| {
        n.tag_name() != "label" || !labels.iter().any(|aid| n.str_attr("aid").as_str() == aid)
    });
    if show {
        for aid in &labels {
            let mut node = LDom2Node::new();
            node.set_tag_name("label");
            node.set_str_attr("aid", aid.as_str());
            if !format.is_empty() {
                node.set_str_attr("format", format);
            }
            nodes.push(node);
        }
    }
    if show {
        while nodes
            .iter()
            .filter(|node| node.tag_name() == "label")
            .count()
            > max_labels
        {
            if let Some(index) = nodes.iter().position(|node| node.tag_name() == "label") {
                nodes.remove(index);
            }
        }
    }
    r.set_qsc_children(nodes);
    object.set_modified(true);
    Ok(json!({"rendererId":renderer.get(),"labels":labels.len()}))
}

fn measure(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let id = uid(args, "objId")?;
    let mut references = Vec::new();
    let mut objects = Vec::new();
    let (positions, labels) = if name == "measure_selections" {
        let sels = text(args, "selections")?
            .split(';')
            .map(str::trim)
            .collect::<Vec<_>>();
        let positions = sels
            .iter()
            .map(|s| {
                state
                    .molecule_selection_center_for_text(id, s)?
                    .ok_or_else(|| CueMolError::not_found("selection atoms"))
            })
            .collect::<Result<Vec<_>>>()?;
        (
            positions,
            sels.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    } else {
        let specs = args["atoms"].as_array().unwrap();
        let owners = if let Some(s) = args["atomObjects"].as_str() {
            std::iter::once(Ok(id))
                .chain(s.split_whitespace().map(|part| {
                    if let Ok(n) = part.parse::<u64>() {
                        Ok(Uid::new(n))
                    } else {
                        resolve_node(state, part).and_then(|(id, obj)| {
                            if obj {
                                Ok(id)
                            } else {
                                Err(CueMolError::invalid_argument("expected molecule"))
                            }
                        })
                    }
                }))
                .collect::<Result<Vec<_>>>()?
        } else {
            vec![id; specs.len()]
        };
        if owners.len() != specs.len() {
            return Err(CueMolError::invalid_argument(
                "atomObjects must name one object per atom",
            ));
        }
        let mut positions = Vec::new();
        let mut labels = Vec::new();
        for (spec, owner) in specs.iter().zip(owners) {
            let index = atom_indices(state, owner, &json!({"atoms":[spec]}))?[0];
            let atom = &state.scene_manager().object(owner).unwrap().mol().atoms()[index];
            let mut position = atom.position;
            state
                .scene_manager()
                .object(owner)
                .unwrap()
                .xform_matrix()
                .xform3d(&mut position);
            positions.push(position);
            let aid = crate::navigation::atom_reference(atom);
            references.push(aid.clone());
            objects.push((
                owner,
                state
                    .scene_manager()
                    .object(owner)
                    .unwrap()
                    .name()
                    .to_string(),
            ));
            labels.push(aid);
        }
        (positions, labels)
    };
    let (kind, value, unit) = match positions.len() {
        2 => (
            AtomIntrKind::Distance,
            (positions[0] - positions[1]).length3d(),
            "angstrom",
        ),
        3 => {
            let a = positions[0] - positions[1];
            let b = positions[2] - positions[1];
            let den = a.length3d() * b.length3d();
            if den < 1e-12 {
                return Err(CueMolError::invalid_argument("coincident atoms"));
            }
            (
                AtomIntrKind::Angle,
                (a.dot3d(b) / den).clamp(-1., 1.).acos().to_degrees(),
                "degree",
            )
        }
        4 => {
            let a = positions[1] - positions[0];
            let b = positions[2] - positions[1];
            let c = positions[3] - positions[2];
            let n = a.cross(b);
            let m = b.cross(c);
            if n.length3d() * m.length3d() < 1e-12 {
                return Err(CueMolError::invalid_argument("collinear atoms"));
            }
            (
                AtomIntrKind::Torsion,
                (n.cross(m).dot3d(b) / b.length3d())
                    .atan2(n.dot3d(m))
                    .to_degrees(),
                "degree",
            )
        }
        _ => {
            return Err(CueMolError::invalid_argument(
                "measurements need two, three, or four atoms or selections",
            ))
        }
    };
    let mut renderer = None;
    if name == "measure_selections" || args["draw"] == true {
        let object = state.scene_manager_mut().object_mut(id).unwrap();
        let label = args["label"].as_str().unwrap_or("measure");
        let r = object
            .renderers()
            .iter()
            .find(|r| r.renderer_type() == "atomintr" && r.name() == label)
            .map(|r| r.uid())
            .unwrap_or_else(|| {
                let id = object.add_renderer("atomintr", label);
                object
                    .renderer_mut(id)
                    .unwrap()
                    .set_qsc_attr("showlabel", "true");
                id
            });
        let line = if name == "measure_selections" {
            AtomIntrLine::from_selections(kind, labels)
        } else {
            AtomIntrLine::from_atom_references(kind, references, labels).with_objects(id, objects)
        };
        object
            .renderer_mut(r)
            .unwrap()
            .append_atom_intr_line(line)?;
        object.set_modified(true);
        renderer = Some(r.get());
    }
    Ok(json!({"value":value,"unit":unit,"rendererId":renderer}))
}

fn coloring(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let id = uid(args, "nodeId")?;
    let is_object = args["nodeType"] == "object";
    let owner = if is_object {
        id
    } else {
        renderer_owner(state, id)?
    };
    let (kind, mut attrs, mut paint) = if is_object {
        let o = state
            .scene_manager()
            .object(owner)
            .ok_or_else(|| CueMolError::not_found("molecule"))?;
        (
            o.coloring_type().unwrap_or("CPKColoring").to_string(),
            o.coloring_attrs().clone(),
            o.paint_coloring().to_vec(),
        )
    } else {
        state
            .renderer_coloring_state(owner, id)
            .ok_or_else(|| CueMolError::not_found("renderer coloring"))?
    };
    if name == "list_paint" {
        return Ok(
            json!({"entries":paint.iter().enumerate().map(|(i,(s,c))|json!({"number":i+1,"selection":s,"color":c})).collect::<Vec<_>>()}),
        );
    }
    let entry_index = || {
        integer(args, "number", 0)?
            .checked_sub(1)
            .filter(|i| *i < paint.len())
            .ok_or_else(|| CueMolError::invalid_argument("paint entry number is out of range"))
    };
    match name {
        "add_paint" | "add_paint_at" => {
            state.parse_scene_selector(scene(state)?, selection(args))?;
            let i = if name == "add_paint" {
                0
            } else if args["at"].is_null() {
                paint.len()
            } else {
                integer(args, "at", 1)?
                    .checked_sub(1)
                    .filter(|i| *i <= paint.len())
                    .ok_or_else(|| {
                        CueMolError::invalid_argument("paint insertion is out of range")
                    })?
            };
            paint.insert(i, (selection(args).into(), text(args, "color")?.into()));
        }
        "set_paint" => {
            let i = entry_index()?;
            if let Some(s) = args["selection"].as_str() {
                state.parse_scene_selector(scene(state)?, s)?;
                paint[i].0 = s.into();
            }
            if let Some(c) = args["color"].as_str() {
                paint[i].1 = c.into();
            }
        }
        "remove_paint" => {
            let i = entry_index()?;
            paint.remove(i);
        }
        "move_paint" => {
            let from = entry_index()?;
            let to = nodes::move_target(args, from, paint.len())?;
            let item = paint.remove(from);
            paint.insert(to, item);
        }
        "clear_paint" => paint.clear(),
        "set_coloring_prop" => {
            attrs.insert(text(args, "prop")?.into(), text(args, "value")?.into());
        }
        _ => {}
    }
    let default = (name == "set_default_color")
        .then(|| text(args, "color").map(str::to_owned))
        .transpose()?;
    let kind = if name.contains("paint") {
        "PaintColoring".into()
    } else {
        kind
    };
    if is_object {
        state.set_object_coloring_details(owner, Some(kind), attrs, paint, default)?;
    } else {
        state.set_renderer_coloring_details(owner, id, Some(kind), attrs, paint, default)?;
    }
    if name == "add_paint" && state.scene_manager().object(owner).is_some() {
        state.set_molecule_selection_text(owner, selection(args))?;
    }
    Ok(json!({"nodeId":id.get()}))
}

pub fn surface_coloring(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let id = uid(args, "rendId")?;
    let owner = renderer_owner(state, id)?;
    let mut attrs = BTreeMap::new();
    if name == "set_elepot_coloring" {
        for (arg, prop) in [("map", "elepot"), ("low", "lowpar"), ("high", "highpar")] {
            if !args[arg].is_null() {
                attrs.insert(
                    prop.into(),
                    args[arg]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or(args[arg].to_string()),
                );
            }
        }
        state.set_renderer_electrostatic_coloring(owner, id, &attrs)?;
    } else {
        let nodes = text(args, "nodes")?
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|part| {
                let (value, color) = part.split_once(char::is_whitespace).ok_or_else(|| {
                    CueMolError::invalid_argument("gradient node requires a value and color")
                })?;
                Ok((
                    value
                        .parse::<f64>()
                        .map_err(|_| CueMolError::invalid_argument("invalid gradient value"))?,
                    color.trim().into(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        if state.density_map_summary(owner).is_some() {
            state.set_density_renderer_coloring(owner, id, "multigrad", &attrs, &nodes)?;
        } else {
            state.set_surface_renderer_coloring(owner, id, "multigrad", &attrs, &nodes)?;
        }
    }
    Ok(json!({"rendererId":id.get()}))
}
