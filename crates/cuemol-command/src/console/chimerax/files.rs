use super::{
    execute::{tool, Context, Deferred},
    Command,
};
use crate::console::{Action, Session};
use cuemol_qsys::Camera;
use serde_json::json;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;

pub(super) fn plan(
    session: &Session,
    context: &Context<'_>,
    camera: &Camera,
    command: &Command,
    args: &BTreeMap<String, String>,
) -> Result<Vec<Action>> {
    let mut out = Vec::new();
    match command.name.as_str() {
        "open" => {
            let name = &args["names"];
            if name.to_ascii_lowercase().ends_with(".cxc") {
                return session.read_script(name);
            }
            let path = session.resolve_path(name);
            let path_text = path.to_string_lossy();
            let pdb = re!(r"(?i)^(?:pdb:)?([0-9][A-Za-z0-9]{3})$")
                .captures(name)
                .filter(|_| !path.exists());
            if let Some(pdb) = pdb {
                out.push(tool("fetch_pdb",json!({"pdbId":&pdb[1],"format":"mmcif","rendererType":"ribbon","rendererName":"chx:cartoon","selection":"protein","name":if args["name"].is_empty(){None}else{Some(&args["name"])},"preset":null,"centerView":true}))?);
            } else if path_text.to_ascii_lowercase().ends_with(".qsc") {
                return Ok(vec![tool(
                    "load_file",
                    json!({"path":path_text,"rendererType":null,"selection":null,"name":null,"rendererName":null,"preset":null,"centerView":null,"mapCenterPolicy":null}),
                )?]);
            } else {
                let molecule = context
                    .state
                    .path_supports_molecule_renderer_setup(&path)
                    .map_err(|e| e.to_string())?;
                out.push(tool("load_file",json!({"path":path_text,"rendererType":if molecule{Some("ribbon")}else{None},"rendererName":if molecule{Some("chx:cartoon")}else{None},"selection":if molecule{Some("protein")}else{None},"name":if args["name"].is_empty(){None}else{Some(&args["name"])},"preset":null,"centerView":true,"mapCenterPolicy":null}))?);
            }
            out.push(Action::ChimeraX(Box::new(Deferred::Opened {
                before: context.models.iter().map(|m| m.id).collect(),
            })));
        }
        "close" | "delete" => {
            let close = command.name == "close";
            for target in context.targets(&args[if close { "models" } else { "atoms" }], !close)? {
                out.push(if close {
                    tool(
                        "delete_node",
                        json!({"nodeId":target.model.id.get(),"nodeType":"object"}),
                    )?
                } else {
                    tool(
                        "delete_atoms",
                        json!({"objId":target.model.id.get(),"selection":target.selection}),
                    )?
                });
            }
        }
        "save" => {
            for refused in ["selectedOnly", "displayedOnly", "relModel", "allCoordsets"] {
                if !args[refused].is_empty() {
                    return Err(format!("Error: save: {refused} is not supported"));
                }
            }
            let path = session.resolve_path(&args["filename"]);
            let ext = if !args["format"].is_empty() {
                args["format"].to_ascii_lowercase()
            } else {
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
            };
            match ext.as_str() {
                "cxs"|"session"=>return Err("Error: save: a ChimeraX session (.cxs) cannot be written; save the scene as .qsc".into()),
                "png"=> {
                    let mut width=args["width"].parse::<i64>().ok();
                    let mut height=args["height"].parse::<i64>().ok();
                    let view=super::view::current_view(context,camera);
                    if let (Some(w),None)=(width,height) {height=Some((w as f64*f64::from(view.height())/f64::from(view.width())).round() as i64);}
                    if let (None,Some(h))=(width,height) {width=Some((h as f64*f64::from(view.width())/f64::from(view.height())).round() as i64);}
                    out.push(tool("export_scene",json!({"path":path.to_string_lossy(),"format":"png","width":width,"height":height,"transparent":if args["transparentBackground"].is_empty(){None}else{Some(args["transparentBackground"]=="true")},"dpi":null}))?);
                }
                "qsc"=>out.push(tool("save_scene",json!({"path":path.to_string_lossy()}))?),
                "pdb"=> {
                    let targets=context.targets(&args["models"],true)?;
                    if targets.len()!=1 {return Err(format!("Error: save: a .pdb file holds one model here; name it with models ({} given)",targets.len()))}
                    out.push(tool("save_object",json!({"objId":targets[0].model.id.get(),"fileName":path.to_string_lossy(),"selection":null,"format":"pdb","link":false}))?);
                }
                _=>return Err(format!("Error: save: \"{ext}\" files are not supported (png, qsc, pdb)")),
            }
        }
        _ => unreachable!(),
    }
    Ok(out)
}
