use serde_json::{json, Value};
use std::collections::BTreeSet;

const GROUPS: &[(&str, &str)] = &[
    (
        "analysis",
        "Distances, angles, torsions, residue lists and interactions",
    ),
    (
        "files",
        "Open and save files, URLs, scenes and exported images",
    ),
    (
        "coloring",
        "Painted regions, gradients, named colors and electrostatic coloring",
    ),
    ("view", "Named cameras, projection, panning and focus"),
    ("map", "Density maps, contours and map surfaces"),
    (
        "molops",
        "Superposition, molecular surfaces, atom/bond and chain edits",
    ),
    ("xtal", "Crystal symmetry and unit cells"),
    ("style", "Renderer styles and reusable style sets"),
    ("selection", "Named selection expressions"),
    (
        "render",
        "High quality ray tracing, illustration and render settings",
    ),
    ("anim", "Animation and molecular morphs"),
    ("trajectory", "Load and play indexed MD trajectories"),
    ("advanced", "Additional scene operations and scripts"),
];
fn group(name: &str) -> &str {
    match name {
        "get_scene_state"
        | "create_scene"
        | "list_scenes"
        | "switch_scene"
        | "close_scene"
        | "capture_view"
        | "fetch_pdb"
        | "create_renderer"
        | "delete_node"
        | "rename_node"
        | "list_node_props"
        | "set_node_prop"
        | "set_node_props"
        | "set_visible"
        | "list_renderer_types"
        | "set_renderer_coloring"
        | "set_renderer_selection"
        | "set_renderer_type"
        | "set_mol_selection"
        | "count_selection"
        | "list_chains"
        | "add_paint"
        | "center_view"
        | "rotate_view"
        | "set_view" => "core",
        "list_residues"
        | "analyze_interactions"
        | "measure_geometry"
        | "measure_selections"
        | "list_interactions"
        | "remove_interaction"
        | "set_atom_label"
        | "set_atom_labels" => "analysis",
        "load_file" | "fetch_url" | "save_scene" | "save_object" | "export_image"
        | "export_scene" | "load_pdb" | "load_mmcif" | "load_mtz" | "load_ccp4" | "load_msms"
        | "load_namd" | "load_amber" => "files",
        "list_coloring_styles"
        | "clear_paint"
        | "set_default_color"
        | "set_coloring_prop"
        | "list_paint"
        | "set_paint"
        | "add_paint_at"
        | "set_multigrad_nodes"
        | "remove_paint"
        | "move_paint"
        | "set_elepot_coloring"
        | "define_color"
        | "remove_color" => "coloring",
        "list_cameras"
        | "apply_camera"
        | "delete_camera"
        | "save_camera"
        | "rename_camera"
        | "move_camera"
        | "set_camera_visibility"
        | "clear_camera_visibility"
        | "load_camera_file"
        | "save_camera_file"
        | "reload_camera"
        | "set_projection"
        | "pan_view"
        | "get_view"
        | "focus_node" => "view",
        "fetch_map"
        | "list_map_contours"
        | "set_map_contour"
        | "recenter_map"
        | "create_surface_from_map" => "map",
        "superpose"
        | "create_surface"
        | "recalc_surface"
        | "cut_surface"
        | "delete_atoms"
        | "rename_chain"
        | "renumber_residues"
        | "merge_molecules"
        | "set_secondary_structure"
        | "add_bond"
        | "remove_bond"
        | "calc_elepot" => "molops",
        "show_symmetry" | "show_unit_cell" | "set_symmetry" | "create_symmetry_copy" => "xtal",
        "list_renderer_styles"
        | "apply_renderer_style"
        | "set_renderer_styles"
        | "create_preset_renderer"
        | "create_style_set"
        | "delete_style_set"
        | "set_style_readonly"
        | "load_style_file"
        | "save_style_file"
        | "remove_style" => "style",
        "define_selection" | "remove_selection" => "selection",
        "render_image" | "get_screenshot" | "set_render_settings" | "get_hatch_style" => "render",
        "animate"
        | "list_anims"
        | "add_anim"
        | "remove_anim"
        | "move_anim"
        | "set_anim_time"
        | "set_anim_prop"
        | "set_anim_generic_prop"
        | "reset_anim_prop"
        | "set_anim_options"
        | "list_morph_frames"
        | "add_morph_frame"
        | "remove_morph_frame"
        | "convert_to_morph" => "anim",
        "load_trajectory" | "append_trajectory" | "list_trajectories" | "set_frame" => "trajectory",
        _ => "advanced",
    }
}
pub(crate) fn available(name: &str, enabled: &BTreeSet<String>) -> bool {
    name == "enable_toolsets"
        || (!matches!(name, "undo" | "redo" | "clear_undo")
            && cuemol_command::tools_list()
                .iter()
                .any(|t| t["name"] == name)
            && (group(name) == "core" || enabled.contains(group(name))))
}
pub(crate) fn tools(enabled: &BTreeSet<String>) -> Vec<Value> {
    let mut tools: Vec<_> = cuemol_command::tools_list()
        .iter()
        .filter(|t| available(t["name"].as_str().unwrap(), enabled))
        .cloned()
        .collect();
    let description = GROUPS
        .iter()
        .map(|(id, description)| format!("{id}: {description}"))
        .collect::<Vec<_>>()
        .join(". ");
    tools.push(json!({"name":"enable_toolsets","description":format!("Enable additional tools for this conversation. {description}. Enable groups only as needed."),"inputSchema":{"type":"object","properties":{"toolsets":{"type":"array","items":{"type":"string","enum":GROUPS.iter().map(|(id,_)|id).collect::<Vec<_>>()}}},"required":["toolsets"],"additionalProperties":false}}));
    tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    tools
}
pub(crate) fn enable(args: &Value, enabled: &mut BTreeSet<String>) -> Result<Value, String> {
    let values = args["toolsets"]
        .as_array()
        .ok_or("toolsets must be an array")?;
    let names = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|id| GROUPS.iter().any(|(name, _)| name == id))
                .ok_or("Unknown toolset")
        })
        .collect::<Result<Vec<_>, _>>()?;
    enabled.extend(names.into_iter().map(str::to_string));
    Ok(json!({"enabled":enabled}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolsets_keep_core_small_and_enable_every_non_history_tool() {
        let mut enabled = BTreeSet::new();
        assert!(tools(&enabled).len() <= 26);
        assert!(!available("calc_elepot", &enabled));
        enable(
            &json!({"toolsets":GROUPS.iter().map(|(id,_)|id).collect::<Vec<_>>()}),
            &mut enabled,
        )
        .unwrap();
        for tool in cuemol_command::tools_list() {
            let name = tool["name"].as_str().unwrap();
            assert_eq!(
                available(name, &enabled),
                !matches!(name, "undo" | "redo" | "clear_undo"),
                "{name}"
            );
        }
    }
}
