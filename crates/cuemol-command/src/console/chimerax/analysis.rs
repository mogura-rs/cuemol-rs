use super::{
    display,
    execute::{tool, Context, Deferred, Target},
    Command,
};
use crate::console::Action;
use cuemol_core::{Uid, Vector4};
use serde_json::json;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
const DISTANCE: &str = "chx:distance";

pub(super) fn positions(
    context: &Context<'_>,
    object: Uid,
    selection: &str,
) -> Result<Vec<Vector4>> {
    let mol = context
        .state
        .scene_manager()
        .object(object)
        .ok_or("Molecule no longer exists")?;
    let selector = context
        .state
        .parse_object_selector(object, selection)
        .map_err(|e| e.to_string())?;
    Ok(mol
        .mol()
        .select_indices(&selector)
        .into_iter()
        .map(|i| {
            let mut pos = mol.mol().atoms()[i].position;
            mol.xform_matrix().xform3d(&mut pos);
            pos
        })
        .collect())
}
pub(super) fn report(
    context: &Context<'_>,
    mov: Uid,
    reference: Uid,
    mov_sel: &str,
    ref_sel: &str,
) -> Result<Vec<Action>> {
    let a = positions(context, mov, mov_sel)?;
    let b = positions(context, reference, ref_sel)?;
    if a.len() != b.len() || a.is_empty() {
        return Err("Error: align: atom counts changed during fitting".into());
    }
    let sum = a
        .iter()
        .zip(&b)
        .map(|(a, b)| {
            let d = *a - *b;
            d.dot3d(d)
        })
        .sum::<f64>();
    Ok(vec![Action::Output(format!(
        "RMSD between {} atom pairs is {:.3} angstroms",
        a.len(),
        (sum / a.len() as f64).sqrt()
    ))])
}
fn one(context: &Context<'_>, spec: &str, name: &str) -> Result<Target> {
    let targets = context.targets(spec, true)?;
    if targets.len() != 1 {
        return Err(format!(
            "Error: {name}: \"{spec}\" must be in one model (it is in {})",
            targets.len()
        ));
    }
    Ok(targets.into_iter().next().unwrap())
}
pub(super) fn plan(
    context: &Context<'_>,
    command: &Command,
    args: &BTreeMap<String, String>,
) -> Result<Vec<Action>> {
    let name = command.name.as_str();
    if name.starts_with("distance") || name == "~distance" {
        return distance(context, name, args);
    }
    let align = name == "align";
    if align {
        for refused in [
            "each",
            "matchChainIds",
            "matchNumbering",
            "matchAtomNames",
            "cutoffDistance",
            "reportMatrix",
        ] {
            if !args[refused].is_empty() {
                return Err(format!("Error: align: {refused} is not supported"));
            }
        }
        if !args["move"].is_empty() && !"structures".starts_with(&args["move"]) {
            return Err("Error: align: only move structures is supported".into());
        }
    }
    let ref_key = if align { "toAtoms" } else { "to" };
    if args[ref_key].is_empty() {
        return Err(format!("Error: {name}: give the reference with {ref_key}"));
    }
    let reference = one(context, &args[ref_key], name)?;
    let movers = if align {
        let mov = one(context, &args["atoms"], name)?;
        if mov.model.id == reference.model.id {
            return Err("Error: align: the two sets of atoms are in the same model".into());
        }
        let n = context.indices(&mov)?.len();
        let n_ref = context.indices(&reference)?.len();
        if n != n_ref {
            return Err(format!(
                "Error: align: different numbers of atoms ({n} and {n_ref})"
            ));
        }
        if n < 3 {
            return Err(format!(
                "Error: align: fitting needs at least 3 atom pairs (got {n})"
            ));
        }
        vec![mov]
    } else {
        let movers = context
            .targets(&args["atoms"], true)?
            .into_iter()
            .filter(|t| t.model.id != reference.model.id)
            .collect::<Vec<_>>();
        if movers.is_empty() {
            return Err(format!(
                "Error: {name}: no model to move apart from the reference"
            ));
        }
        movers
    };
    let mut out = Vec::new();
    for mov in movers {
        if !align {
            out.push(Action::Output(format!("Warning: {name}: #{} is superposed by US-align (a structural alignment), not ChimeraX's sequence alignment; the fit can differ",mov.model.number)));
        }
        out.push(tool("superpose",json!({"movId":mov.model.id.get(),"refId":reference.model.id.get(),"movSel":mov.selection,"refSel":reference.selection,"algo":if align{"LSQ"}else{"USalign"},"pivot":null,"keepAsProperty":null}))?);
        if align {
            out.push(Action::ChimeraX(Box::new(Deferred::Rmsd {
                mov: mov.model.id,
                reference: reference.model.id,
                mov_sel: mov.selection,
                ref_sel: reference.selection.clone(),
            })));
        } else {
            out.push(Action::Output(format!(
                "Matched {} #{} to {} #{}",
                mov.model.name, mov.model.number, reference.model.name, reference.model.number
            )));
        }
    }
    Ok(out)
}
fn distance(
    context: &Context<'_>,
    name: &str,
    args: &BTreeMap<String, String>,
) -> Result<Vec<Action>> {
    let deleting = name != "distance";
    if deleting && args["objects"].trim().is_empty() {
        let mut out = Vec::new();
        for model in context.models.iter().filter(|m| m.molecule) {
            if let Some(&id) = display::owned(context, model.id).get(DISTANCE) {
                out.push(tool(
                    "delete_node",
                    json!({"nodeId":id.get(),"nodeType":"renderer"}),
                )?);
            }
        }
        return Ok(out);
    }
    let mut atoms = Vec::new();
    for target in context.targets(&args["objects"], true)? {
        for index in context.indices(&target)?.into_iter().take(3 - atoms.len()) {
            atoms.push((target.clone(), index));
        }
        if atoms.len() == 3 {
            break;
        }
    }
    if atoms.len() != 2 {
        return Err(format!(
            "Error: {name}: \"{}\" must be exactly two atoms (it is {})",
            args["objects"],
            if atoms.len() == 3 {
                "more".into()
            } else {
                atoms.len().to_string()
            }
        ));
    }
    let mut names = Vec::new();
    let mut labels = Vec::new();
    let mut specs = Vec::new();
    let mut points = Vec::new();
    let many = context.models.iter().filter(|m| m.molecule).count() > 1;
    for (target, index) in &atoms {
        let object = context
            .state
            .scene_manager()
            .object(target.model.id)
            .ok_or("No molecule")?;
        let atom = &object.mol().atoms()[*index];
        let residue = format!(
            "{}{}",
            atom.res_id,
            atom.ins_code
                .filter(|c| !c.is_whitespace())
                .map(|c| c.to_string())
                .unwrap_or_default()
        );
        names.push(crate::navigation::atom_reference(atom));
        labels.push(format!(
            "{}/{} {} {residue} {}",
            if many {
                format!("#{}", target.model.number)
            } else {
                String::new()
            },
            atom.chain_id,
            atom.res_name,
            atom.name
        ));
        specs.push(json!({"chain":atom.chain_id,"resid":residue,"atomName":atom.name}));
        let mut position = atom.position;
        object.xform_matrix().xform3d(&mut position);
        points.push(position);
    }
    let object = atoms[0].0.model.id;
    if deleting {
        let id = display::owned(context, object)
            .get(DISTANCE)
            .copied()
            .ok_or("Error: ~distance: there is no distance between those atoms")?;
        let entries = context
            .state
            .atom_intr_def_summaries(object, id)
            .map_err(|e| e.to_string())?;
        let entry = entries
            .iter()
            .position(|e| {
                e.mode == 1
                    && e.atoms.len() == 2
                    && e.atoms.iter().all(|atom| {
                        let atom = re!(r":.$").replace(atom, "");
                        names.iter().any(|name| name == &atom)
                    })
            })
            .ok_or("Error: ~distance: there is no distance between those atoms")?;
        return Ok(vec![tool(
            "remove_interaction",
            json!({"rendId":id.get(),"number":entry+1}),
        )?]);
    }
    let precision = args["decimalPlaces"]
        .parse::<usize>()
        .ok()
        .filter(|n| *n <= 100)
        .ok_or("Error: distance: decimalPlaces must be between 0 and 100")?;
    let value = (points[0] - points[1]).length3d();
    Ok(vec![
        tool(
            "measure_geometry",
            json!({"objId":object.get(),"atoms":specs,"draw":true,"label":DISTANCE,"atomObjects":if object==atoms[1].0.model.id {None}else{Some(atoms[1].0.model.id.get().to_string())}}),
        )?,
        Action::Output(format!(
            "Distance between {} and {}: {value:.precision$}Å",
            labels[0], labels[1]
        )),
    ])
}
