use super::{
    display,
    execute::{Context, Deferred, Target},
    paint, palette, Model,
};
use crate::console::Action;
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;
const POLYMER: &str = "(protein or nucleic)";
const METAL: &str="elem Li,Na,K,Rb,Cs,Be,Mg,Ca,Sr,Ba,Al,Ga,In,Tl,Sn,Pb,Bi,Sc,Ti,V,Cr,Mn,Fe,Co,Ni,Cu,Zn,Y,Zr,Mo,Ru,Rh,Pd,Ag,Cd,W,Re,Os,Ir,Pt,Au,Hg,La,Ce,Eu,Gd,Tb,Yb,Lu,U";
const HALIDE: &str = "ligand and elem F,Cl,Br,I";
pub(super) fn choice(chains: usize, atoms: usize, residues: usize) -> &'static str {
    if chains == 0 {
        "non-polymer"
    } else if chains < 5 && atoms < 200_000 && residues < 5000 {
        "small polymer"
    } else if chains < 250 && atoms < 1_000_000 {
        "medium polymer"
    } else {
        "large polymer"
    }
}
pub(super) fn apply(context: &Context<'_>, model: &Model) -> Result<Vec<Action>> {
    let target = Target {
        model: model.clone(),
        selection: "*".into(),
    };
    let mut out = display::ensure(context, std::slice::from_ref(&target))?;
    if !out.is_empty() {
        out.push(Action::ChimeraX(Box::new(Deferred::AutoStyle {
            object: model.id,
        })));
        return Ok(out);
    }
    let object = context
        .state
        .scene_manager()
        .object(model.id)
        .ok_or("No molecule")?;
    let polymer = Target {
        selection: POLYMER.into(),
        ..target.clone()
    };
    let mut chains: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut previous = None;
    for index in context.indices(&polymer)? {
        let atom = &object.mol().atoms()[index];
        let key = (&atom.chain_id, atom.res_id, atom.ins_code);
        if previous == Some(key) {
            continue;
        }
        previous = Some(key);
        chains
            .entry(atom.chain_id.clone())
            .or_default()
            .push(atom.res_name.clone());
    }
    let residues = chains.values().map(Vec::len).sum();
    let one_sequence = chains.values().collect::<BTreeSet<_>>().len() <= 1;
    let style = choice(chains.len(), object.mol().atoms().len(), residues);
    let (color, exact) = palette::model(model.number);
    if !exact {
        out.push(Action::Output(format!("Warning: open: #{} is past ChimeraX's model colours; its colour differs from ChimeraX's",model.number)));
    }
    let mut entries = vec![("*".into(), color)];
    let ligand = format!("ligand and not ({HALIDE})");
    let mut hetero = None;
    let shown: [String; 5] = if style == "non-polymer" || style == "small polymer" && residues <= 10
    {
        hetero = Some("*".into());
        ["*", "none", "none", "none", "none"].map(str::to_owned)
    } else if style == "small polymer" {
        let polar = [
            "name H,H1,H2,H3,HN",
            "resn ARG and name HE,HH11,HH12,HH21,HH22",
            "resn ASN and name HD21,HD22",
            "resn GLN and name HE21,HE22",
            "resn HIS and name HD1,HE2",
            "resn TRP and name HE1",
            "resn LYS and name HZ1,HZ2,HZ3",
            "resn SER,CYS and name HG",
            "resn THR and name HG1",
            "resn TYR and name HH",
        ]
        .map(|s| format!("({s})"))
        .join(" or ");
        let nearby = display::minus(
            &format!("byres (({ligand}) expand 3.6)"),
            &format!("protein and hydrogen and not ({polar})"),
        );
        let spheres = format!("({nearby}) and ({METAL} or {HALIDE})");
        hetero = Some("*".into());
        [
            display::minus(&nearby, &spheres),
            "none".into(),
            spheres,
            "protein".into(),
            "nucleic".into(),
        ]
    } else if style == "medium polymer" {
        if one_sequence {
            entries.extend(
                chains
                    .keys()
                    .map(|chain| (format!("chain {chain}"), palette::chain(chain).0)),
            );
        }
        hetero = Some(format!("({ligand}) or water"));
        [
            ligand.clone(),
            "water".into(),
            display::minus("*", &format!("({ligand}) or water")),
            "none".into(),
            "none".into(),
        ]
    } else {
        ["none", "none", "*", "none", "none"].map(str::to_owned)
    };
    if !one_sequence && matches!(style, "medium polymer" | "large polymer") {
        out.push(Action::Output("Warning: open: chains with different sequences keep the model colour (ChimeraX colours each sequence)".into()));
    }
    let ids = display::owned(context, model.id);
    for (kind, selection) in display::KINDS.iter().zip(shown) {
        if let Some(&id) = ids.get(kind.0) {
            display::write(context, model.id, id, &selection, &mut out)?;
        }
    }
    paint::apply(context, &target, "ac", &entries, false, &mut out)?;
    if let Some(selection) = hetero {
        paint::apply(
            context,
            &target,
            "a",
            &paint::element_entries(&selection, false),
            false,
            &mut out,
        )?;
    }
    Ok(out)
}
