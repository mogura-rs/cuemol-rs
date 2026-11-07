use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Merge,
    Delete,
    Chain,
    Residue,
}

impl Kind {
    pub fn from_command(name: &str) -> Option<Self> {
        [Self::Merge, Self::Delete, Self::Chain, Self::Residue]
            .into_iter()
            .find(|kind| kind.command() == name)
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Merge => "merge_molecules",
            Self::Delete => "delete_atoms",
            Self::Chain => "rename_chain",
            Self::Residue => "renumber_residues",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Merge => "Merge molecule",
            Self::Delete => "Delete mol atoms",
            Self::Chain => "Change chain ID",
            Self::Residue => "Change residue index",
        }
    }

    pub fn no_result(self) -> &'static str {
        match self {
            Self::Residue => "Residue index change was rejected",
            _ => "No atoms matched the selection",
        }
    }

    pub fn result_key(self) -> &'static str {
        match self {
            Self::Merge => "atoms",
            Self::Delete => "deleted",
            _ => "changed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Cancel,
}

#[derive(Debug)]
pub struct Dialog {
    pub kind: Kind,
    pub scene: Option<Uid>,
    pub object: Option<Uid>,
    pub target: Option<Uid>,
    pub selection: String,
    pub copy: bool,
    pub chain: String,
    pub shift: bool,
    pub value: String,
    pub renumber: bool,
    pub error: String,
    pub pending_confirmation: Option<&'static str>,
    pub rects: BTreeMap<String, [f32; 4]>,
    builder: crate::selection::Builder,
    choosing: bool,
}

impl Dialog {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            scene: None,
            object: None,
            target: None,
            selection: String::new(),
            copy: true,
            chain: String::new(),
            shift: true,
            value: "1".into(),
            renumber: false,
            error: String::new(),
            pending_confirmation: None,
            rects: BTreeMap::new(),
            builder: crate::selection::Builder::for_molecule(),
            choosing: false,
        }
    }

    fn opened(&mut self, state: &DesktopAppState, selected: Option<Uid>) {
        let objects = state.active_object_summaries();
        let valid = |id: Option<Uid>| id.filter(|id| objects.iter().any(|o| o.uid == *id));
        *self = Self {
            scene: state.active_scene(),
            object: valid(self.object).or_else(|| {
                (self.kind != Kind::Merge)
                    .then(|| valid(selected).or_else(|| objects.first().map(|o| o.uid)))
                    .flatten()
            }),
            target: valid(self.target),
            ..Self::new(self.kind)
        };
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() {
            return Some("The active scene changed. Reopen the molecule edit dialog.");
        }
        let objects = state.active_object_summaries();
        if !objects.iter().any(|o| Some(o.uid) == self.object) {
            return Some("Select a molecule.");
        }
        if self.kind == Kind::Merge {
            if !objects.iter().any(|o| Some(o.uid) == self.target) {
                return Some("Select a destination molecule.");
            }
            if self.object == self.target {
                return Some("Source and destination must be different molecules.");
            }
        }
        if self.kind == Kind::Delete && self.selection.trim().is_empty() {
            return Some("Enter an explicit selection to delete atoms.");
        }
        if self.kind == Kind::Chain && self.chain.is_empty() {
            return Some("New chain ID is empty.");
        }
        None
    }

    pub fn arguments(&self) -> Result<Value, &'static str> {
        let id = self.object.map(Uid::get);
        Ok(match self.kind {
            Kind::Merge => {
                json!({"fromId":id,"toId":self.target.map(Uid::get),"selection":self.selection,"copy":self.copy})
            }
            Kind::Delete => json!({"objId":id,"selection":self.selection}),
            Kind::Chain => {
                json!({"objId":id,"selection":self.selection,"chain":resolve_chain(&self.chain)?.0})
            }
            Kind::Residue => {
                json!({"objId":id,"selection":self.selection,"mode":if self.shift {"shift"} else {"start"},"value":resolve_residue(self.shift, &self.value)?.0,"renumber":self.renumber})
            }
        })
    }

    pub fn request_accept(&mut self, state: &DesktopAppState) -> Option<Action> {
        self.error.clear();
        if let Some(error) = self.validation_error(state) {
            self.error = error.into();
            return None;
        }
        let confirmation = match self.kind {
            Kind::Chain => resolve_chain(&self.chain).map(|r| r.1),
            Kind::Residue => resolve_residue(self.shift, &self.value).map(|r| r.1),
            _ => Ok(None),
        };
        match confirmation {
            Ok(Some(message)) => self.pending_confirmation = Some(message),
            Ok(None) => return Some(Action::Accept),
            Err(error) => self.error = error.into(),
        }
        None
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
    ) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        let objects = state.active_object_summaries();
        ui.add_enabled_ui(self.pending_confirmation.is_none(), |ui| {
            egui::ScrollArea::vertical().id_salt("molecule-edit-fields")
                .max_height((ui.ctx().content_rect().height() - 240.).max(120.))
                .show(ui, |ui| {
                    molecule(ui, &objects, &mut self.object, if self.kind == Kind::Merge {"From molecule"} else {"Molecule"}, &mut self.rects);
                    ui.horizontal(|ui| {
                        ui.label("Selection");
                        let field = ui.add(egui::TextEdit::singleline(&mut self.selection).desired_width((ui.available_width() - 75.).max(120.)));
                        if field.changed() { self.error.clear(); }
                        record(&mut self.rects, "Selection", field);
                        let choose = ui.button("Choose…");
                        if choose.clicked() { self.choosing = !self.choosing; self.builder.opened(); }
                        record(&mut self.rects, "Choose selection", choose);
                    });
                    if self.choosing {
                        if let Some(applied) = self.builder.show(ui, state, self.object, revision, &self.selection) {
                            self.selection = applied.expression;
                            self.choosing = false;
                            self.error.clear();
                        }
                        self.rects.extend(self.builder.rects.clone());
                    }
                    match self.kind {
                        Kind::Merge => {
                            ui.separator();
                            molecule(ui, &objects, &mut self.target, "To molecule", &mut self.rects);
                            record(&mut self.rects, "Copy", ui.checkbox(&mut self.copy, "Copy (keep source atoms)"));
                        }
                        Kind::Delete => { ui.weak("Use * to select all atoms."); }
                        Kind::Chain => {
                            ui.separator();
                            ui.label("New chain ID");
                            let field = ui.add(egui::TextEdit::singleline(&mut self.chain).desired_width(300.));
                            if field.changed() { self.error.clear(); }
                            record(&mut self.rects, "New chain ID", field);
                            if self.chain.trim().encode_utf16().count() > 1 {
                                ui.weak("Chain ID longer than 1 character does not conform to the PDB format.");
                            }
                        }
                        Kind::Residue => {
                            ui.separator();
                            ui.horizontal(|ui| {
                                record(&mut self.rects, "Shift by", ui.selectable_value(&mut self.shift, true, "Shift by"));
                                record(&mut self.rects, "Start from", ui.selectable_value(&mut self.shift, false, "Start from"));
                            });
                            ui.label(if self.shift {"Shift by"} else {"Start from"});
                            let field = ui.add(egui::TextEdit::singleline(&mut self.value).desired_width(300.));
                            if field.changed() { self.error.clear(); }
                            record(&mut self.rects, "Residue value", field);
                            record(&mut self.rects, "Renumber", ui.checkbox(&mut self.renumber, "Renumber"));
                        }
                    }
                });
            ui.separator();
            let invalid = self.validation_error(state);
            if let Some(error) = invalid { ui.colored_label(ui.visuals().error_fg_color, error); }
            else if !self.error.is_empty() { ui.colored_label(ui.visuals().error_fg_color, &self.error); }
            ui.horizontal(|ui| {
                let accept = ui.add_enabled(invalid.is_none(), egui::Button::new(if self.kind == Kind::Delete {"Delete"} else {"OK"}));
                if accept.clicked() { action = self.request_accept(state); }
                record(&mut self.rects, "OK", accept);
                let cancel = ui.button("Cancel");
                if cancel.clicked() { action = Some(Action::Cancel); }
                record(&mut self.rects, "Cancel", cancel);
            });
        });
        if let Some(message) = self.pending_confirmation {
            ui.separator();
            ui.label(message);
            ui.horizontal(|ui| {
                let yes = ui.button("Yes");
                if yes.clicked() {
                    self.pending_confirmation = None;
                    action = Some(Action::Accept);
                }
                record(&mut self.rects, "Yes", yes);
                let no = ui.button("No");
                if no.clicked() {
                    self.pending_confirmation = None;
                }
                record(&mut self.rects, "No", no);
            });
        }
        action
    }
}

#[derive(Debug)]
pub struct Dialogs {
    pub active: Option<Kind>,
    drafts: [Dialog; 4],
}

impl Default for Dialogs {
    fn default() -> Self {
        Self {
            active: None,
            drafts: [Kind::Merge, Kind::Delete, Kind::Chain, Kind::Residue].map(Dialog::new),
        }
    }
}

impl Dialogs {
    pub fn open(&mut self, kind: Kind, state: &DesktopAppState, selected: Option<Uid>) {
        self.drafts[kind as usize].opened(state, selected);
        self.active = Some(kind);
    }
    pub fn draft(&self) -> &Dialog {
        &self.drafts[self.active.expect("active molecule edit") as usize]
    }
    pub fn draft_mut(&mut self) -> &mut Dialog {
        &mut self.drafts[self.active.expect("active molecule edit") as usize]
    }
}

fn resolve_chain(raw: &str) -> Result<(&str, Option<&'static str>), &'static str> {
    if raw.is_empty() {
        return Err("New chain ID is empty.");
    }
    let value = raw.trim_matches(js_whitespace);
    if value.is_empty() {
        Ok((
            "_",
            Some("Chain ID < > will be converted to <_>. Change the chain ID?"),
        ))
    } else if value.encode_utf16().count() > 1 {
        Ok((value, Some("Chain ID longer than 1 character does not conform to the PDB format. Change the chain ID?")))
    } else {
        Ok((value, None))
    }
}

fn js_whitespace(c: char) -> bool {
    c.is_whitespace() && c != '\u{85}' || c == '\u{feff}'
}

fn resolve_residue(shift: bool, raw: &str) -> Result<(i32, Option<&'static str>), &'static str> {
    let raw = raw.trim_start_matches(js_whitespace);
    let end = raw
        .char_indices()
        .take_while(|(i, c)| c.is_ascii_digit() || *i == 0 && matches!(c, '+' | '-'))
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    let prefix = &raw[..end];
    if !prefix.chars().any(|c| c.is_ascii_digit()) {
        return Err(if shift {
            "Invalid residue shift value."
        } else {
            "Invalid start residue number."
        });
    }
    let value = prefix
        .parse::<i32>()
        .map_err(|_| "Residue number is outside the supported integer range.")?;
    if shift && value == 0 {
        return Err("Residue shift value must not be zero.");
    }
    Ok((value, (!shift && !(-999..=9999).contains(&value)).then_some("Residue number larger than 4 digits does not conform to the PDB format. Change the residue index?")))
}

fn molecule(
    ui: &mut egui::Ui,
    objects: &[cuemol_desktop_core::GuiObjectSummary],
    selected: &mut Option<Uid>,
    label: &str,
    rects: &mut BTreeMap<String, [f32; 4]>,
) {
    ui.label(label);
    let text = objects
        .iter()
        .find(|o| Some(o.uid) == *selected)
        .map_or("Select molecule", |o| o.name.as_str());
    let combo = egui::ComboBox::from_id_salt(label)
        .selected_text(text)
        .width(300.)
        .show_ui(ui, |ui| {
            for object in objects {
                let response = ui.selectable_value(selected, Some(object.uid), &object.name);
                record(rects, &format!("{label} {}", object.name), response);
            }
        });
    record(rects, label, combo.response);
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, label: &str, response: egui::Response) {
    let r = response.rect;
    rects.insert(
        format!("Molecule edit {label}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chain_inputs_preserve_reference_blank_and_utf16_rules() {
        assert!(resolve_chain("").is_err());
        assert_eq!(resolve_chain(" \t").unwrap().0, "_");
        assert!(resolve_chain(" \t").unwrap().1.is_some());
        assert_eq!(resolve_chain(" A "), Ok(("A", None)));
        assert!(resolve_chain("AB").unwrap().1.is_some());
        assert!(resolve_chain("🧬").unwrap().1.is_some());
        assert_eq!(resolve_chain("\u{feff}A"), Ok(("A", None)));
    }
    #[test]
    fn residue_inputs_use_decimal_prefix_and_checked_integer_range() {
        for (raw, value) in [("1.5", 1), ("1e2", 1), (" +12x", 12), ("-0009", -9)] {
            assert_eq!(resolve_residue(true, raw), Ok((value, None)));
        }
        for raw in ["", "word", "+", "-0", "2147483648", "-2147483649"] {
            assert!(resolve_residue(true, raw).is_err());
        }
        assert_eq!(resolve_residue(false, "0x20"), Ok((0, None)));
        assert_eq!(resolve_residue(false, "9999"), Ok((9999, None)));
        assert_eq!(resolve_residue(false, "-999"), Ok((-999, None)));
        assert!(resolve_residue(false, "10000").unwrap().1.is_some());
        assert!(resolve_residue(false, "-1000").unwrap().1.is_some());
    }
}
