use cuemol_core::{CueMolError, Result, Uid, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::Atom;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Navigate,
    RectSelect,
    LassoSelect,
    Distance,
    Angle,
    Torsion,
    Bond,
}
impl Tool {
    pub const ALL: [Self; 7] = [
        Self::Navigate,
        Self::RectSelect,
        Self::LassoSelect,
        Self::Distance,
        Self::Angle,
        Self::Torsion,
        Self::Bond,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Navigate => "Navigate",
            Self::RectSelect => "Rect Select",
            Self::LassoSelect => "Lasso",
            Self::Distance => "Distance",
            Self::Angle => "Angle",
            Self::Torsion => "Torsion",
            Self::Bond => "Add Bond",
        }
    }
    pub fn is_selection(self) -> bool {
        matches!(self, Self::RectSelect | Self::LassoSelect)
    }
    fn count(self) -> usize {
        match self {
            Self::Distance | Self::Bond => 2,
            Self::Angle => 3,
            Self::Torsion => 4,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomHit {
    pub object: Uid,
    pub index: usize,
}
impl From<(u64, usize)> for AtomHit {
    fn from((object, index): (u64, usize)) -> Self {
        Self {
            object: Uid::new(object),
            index,
        }
    }
}
impl AtomHit {
    pub fn atom(self, state: &DesktopAppState) -> Result<&Atom> {
        state
            .scene_manager()
            .object(self.object)
            .and_then(|o| o.mol().atoms().get(self.index))
            .ok_or_else(|| CueMolError::not_found("picked atom"))
    }
    pub fn position(self, state: &DesktopAppState) -> Option<Vector4> {
        let o = state.scene_manager().object(self.object)?;
        let mut position = o.mol().atoms().get(self.index)?.position;
        o.xform_matrix().xform3d(&mut position);
        Some(position)
    }
    fn spec(self, state: &DesktopAppState) -> Result<Value> {
        let a = self.atom(state)?;
        Ok(
            json!({"chain":a.chain_id,"resid":residue(a),"atomName":a.name.trim(),"altConf":a.alt_loc.map(|c|c.to_string())}),
        )
    }
    pub fn description(self, state: &DesktopAppState) -> Result<String> {
        let a = self.atom(state)?;
        let object = state.scene_manager().object(self.object).unwrap();
        Ok(format!(
            "Molecule [{}], {}, O: {:.2} B: {:.2} Pos: ({:.3}, {:.3}, {:.3})",
            object.name(),
            a.format_msg(),
            a.occupancy,
            a.b_factor,
            a.position.x,
            a.position.y,
            a.position.z
        ))
    }
}
fn residue(a: &Atom) -> String {
    format!(
        "{}{}",
        a.res_id,
        a.ins_code.map(|c| c.to_string()).unwrap_or_default()
    )
}
pub(crate) fn atom_reference(a: &Atom) -> String {
    format!(
        "{}.{}.{}{}",
        a.chain_id,
        residue(a),
        a.name.trim(),
        a.alt_loc.map(|c| format!(":{c}")).unwrap_or_default()
    )
}

pub struct Command {
    pub name: &'static str,
    pub args: Value,
}
impl Command {
    pub fn selection(&self) -> Option<(Uid, String)> {
        (self.name == "set_mol_selection")
            .then(|| {
                Some((
                    Uid::new(self.args["objId"].as_u64()?),
                    self.args["selection"].as_str()?.to_string(),
                ))
            })
            .flatten()
    }
}

#[derive(Debug, Default)]
pub struct Navigation {
    pub tool: Tool,
    pub picks: Vec<AtomHit>,
    pub measure_target: String,
    pub message: String,
    anchor: Option<AtomHit>,
    scene: Option<Uid>,
}
impl Navigation {
    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.cancel();
            self.tool = tool;
        }
    }
    pub fn sync_scene(&mut self, scene: Option<Uid>) {
        if self.scene != scene {
            self.cancel();
            self.anchor = None;
            self.scene = scene;
        }
    }
    pub fn cancel(&mut self) {
        self.picks.clear();
    }
    pub fn click(
        &mut self,
        state: &DesktopAppState,
        hit: AtomHit,
        double: bool,
        shift: bool,
    ) -> Result<Option<Command>> {
        self.sync_scene(state.active_scene());
        let atom = hit.atom(state)?;
        self.message = hit.description(state)?;
        if self.tool.count() == 0 {
            if double {
                let selection = if shift {
                    let Some(anchor) = self.anchor.filter(|a| a.object == hit.object) else {
                        return Ok(None);
                    };
                    let previous = anchor.atom(state)?;
                    if previous.chain_id != atom.chain_id {
                        return Ok(None);
                    }
                    let lo = (previous.res_id, previous.ins_code).min((atom.res_id, atom.ins_code));
                    let hi = (previous.res_id, previous.ins_code).max((atom.res_id, atom.ins_code));
                    let mol = state.scene_manager().object(hit.object).unwrap().mol();
                    let serials = mol
                        .atoms()
                        .iter()
                        .filter(|a| {
                            a.chain_id == atom.chain_id
                                && (lo..=hi).contains(&(a.res_id, a.ins_code))
                        })
                        .map(|a| a.serial)
                        .collect::<Vec<_>>();
                    state.molecule_residue_range_selection_text(
                        hit.object,
                        "",
                        &format!("aid {}", atom.serial),
                        &atom_id_selection(&serials),
                        false,
                    )?
                } else {
                    self.anchor = Some(hit);
                    let selection = format!("aid {}", atom.serial);
                    state.molecule_residue_range_selection_text(
                        hit.object, "", &selection, &selection, true,
                    )?
                };
                return Ok(Some(selection_command(hit.object, selection)));
            }
            let reference = atom_reference(atom);
            let on = !state
                .scene_manager()
                .object(hit.object)
                .unwrap()
                .renderers()
                .iter()
                .filter(|r| r.renderer_type() == "*namelabel" && r.name().is_empty())
                .any(|r| {
                    r.qsc_children()
                        .iter()
                        .any(|n| n.tag_name() == "label" && n.str_attr("aid").as_str() == reference)
                });
            return Ok(Some(Command {
                name: "set_atom_label",
                args: json!({"objId":hit.object.get(),"atoms":[hit.spec(state)?],"on":on}),
            }));
        }
        if double {
            return Ok(None);
        }
        if self.tool == Tool::Bond {
            if let Some(first) = self.picks.first() {
                if first.object != hit.object {
                    self.message = "A bond must connect two atoms in the same molecule.".into();
                    return Ok(None);
                }
                if *first == hit {
                    self.message =
                        "Cannot bond an atom to itself; pick a different second atom.".into();
                    return Ok(None);
                }
            }
        }
        self.picks.push(hit);
        if self.picks.len() < self.tool.count() {
            self.message = format!(
                "{} atom(s) picked; pick {} more",
                self.picks.len(),
                self.tool.count() - self.picks.len()
            );
            return Ok(None);
        }
        let picks = std::mem::take(&mut self.picks);
        if picks
            .iter()
            .enumerate()
            .any(|(i, a)| picks[..i].contains(a))
        {
            self.message = "Atom pick canceled (same atom).".into();
            return Ok(None);
        }
        let atoms = picks
            .iter()
            .map(|p| p.spec(state))
            .collect::<Result<Vec<_>>>()?;
        let command = if self.tool == Tool::Bond {
            Command {
                name: "add_bond",
                args: json!({"objId":picks[0].object.get(),"atoms":atoms}),
            }
        } else {
            Command {
                name: "measure_geometry",
                args: json!({"objId":picks[0].object.get(),"atoms":atoms,
                "atomObjects":picks.iter().skip(1).map(|p|p.object.get().to_string()).collect::<Vec<_>>().join(" "),
                "draw":true,"label":if self.measure_target.is_empty(){"measure"}else{&self.measure_target}}),
            }
        };
        Ok(Some(command))
    }
}

pub fn atom_id_selection(serials: &[i32]) -> String {
    if serials.is_empty() {
        return "none".into();
    }
    let mut serials = serials.to_vec();
    serials.sort_unstable();
    serials.dedup();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < serials.len() {
        let start = serials[index];
        let mut end = start;
        while index + 1 < serials.len() && end.checked_add(1) == Some(serials[index + 1]) {
            index += 1;
            end = serials[index];
        }
        if start == end {
            ranges.push(start.to_string());
        } else {
            ranges.push(format!("{start}:{end}"));
        }
        index += 1;
    }
    format!("aid {}", ranges.join(","))
}

pub fn add_selection_expression(current: &str, addition: &str) -> String {
    let current = current.trim();
    if current.is_empty() {
        addition.trim().to_string()
    } else {
        format!("({current}) or ({})", addition.trim())
    }
}

pub fn selection_command(object: Uid, selection: String) -> Command {
    Command {
        name: "set_mol_selection",
        args: json!({"objId":object.get(),"selection":selection}),
    }
}
pub fn selection_for_hit(state: &DesktopAppState, hit: AtomHit, level: &str) -> Result<String> {
    let atom = hit.atom(state)?;
    if level == "Molecule" {
        return Ok("*".into());
    }
    let mol = state.scene_manager().object(hit.object).unwrap().mol();
    let serials = mol
        .atoms()
        .iter()
        .filter(|a| match level {
            "Atom" => a.serial == atom.serial,
            "Residue" => {
                a.chain_id == atom.chain_id
                    && a.res_id == atom.res_id
                    && a.ins_code == atom.ins_code
            }
            "Chain" => a.chain_id == atom.chain_id,
            _ => false,
        })
        .map(|a| a.serial)
        .collect::<Vec<_>>();
    Ok(atom_id_selection(&serials))
}
pub fn selection_commands(
    state: &DesktopAppState,
    hits: &[AtomHit],
    add: bool,
) -> Result<Vec<Command>> {
    let mut grouped = BTreeMap::<Uid, Vec<i32>>::new();
    for hit in hits {
        grouped
            .entry(hit.object)
            .or_default()
            .push(hit.atom(state)?.serial);
    }
    grouped
        .into_iter()
        .map(|(id, mut serials)| {
            serials.sort_unstable();
            serials.dedup();
            let mut selection = atom_id_selection(&serials);
            let current = state.molecule_selection_text(id)?;
            if add && !current.is_empty() {
                selection = add_selection_expression(current, &selection);
            }
            Ok(selection_command(id, selection))
        })
        .collect()
}

pub fn project(mvp: [[f32; 4]; 4], point: Vector4, size: [f64; 2]) -> Option<[f64; 3]> {
    let v = [point.x as f32, point.y as f32, point.z as f32, 1.];
    let c: [f32; 4] = std::array::from_fn(|i| (0..4).map(|j| mvp[j][i] * v[j]).sum());
    if c[3] <= 1e-6 {
        return None;
    }
    let z = (c[2] / c[3]) as f64;
    if !(0. ..=1.).contains(&z) {
        return None;
    }
    Some([
        (c[0] / c[3] + 1.) as f64 * 0.5 * size[0],
        (1. - c[1] / c[3]) as f64 * 0.5 * size[1],
        z,
    ])
}
pub fn polygon_contains(points: &[[f64; 2]], point: [f64; 2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut a = points[points.len() - 1];
    for &b in points {
        let cross = (point[0] - a[0]) * (b[1] - a[1]) - (point[1] - a[1]) * (b[0] - a[0]);
        if cross.abs() < 1e-7
            && (a[0].min(b[0])..=a[0].max(b[0])).contains(&point[0])
            && (a[1].min(b[1])..=a[1].max(b[1])).contains(&point[1])
        {
            return true;
        }
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        a = b;
    }
    inside
}
pub fn polygon_hits(
    state: &DesktopAppState,
    mvp: [[f32; 4]; 4],
    size: [f64; 2],
    points: &[[f64; 2]],
) -> Vec<AtomHit> {
    state
        .active_pick_atoms("")
        .unwrap_or_default()
        .iter()
        .filter_map(|atom| {
            let screen = project(mvp, atom.center, size)?;
            polygon_contains(points, [screen[0], screen[1]]).then_some(AtomHit {
                object: Uid::new(atom.object_uid?),
                index: atom.atom_index?,
            })
        })
        .collect()
}
