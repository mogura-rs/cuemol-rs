use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorTarget {
    pub kind: String,
    pub id: Option<u64>,
}
#[derive(Debug, Default)]
pub struct Inspector {
    pub visible: bool,
    pub rects: std::collections::BTreeMap<String, [f32; 4]>,
    pub target: Option<InspectorTarget>,
    revision: u64,
    rows: Vec<(String, String)>,
    committed: std::collections::BTreeMap<String, String>,
    filter: String,
    tab: usize,
    error: String,
}
impl Inspector {
    pub fn inspect(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        target: Option<InspectorTarget>,
        revision: u64,
    ) {
        if target == self.target && revision == self.revision {
            return;
        }
        if target != self.target {
            self.visible = target.is_some();
            self.filter.clear();
        }
        self.target = target;
        self.revision = revision;
        let Some(target) = &self.target else {
            self.rows.clear();
            return;
        };
        match cuemol_command::call_tool(
            state,
            camera,
            "list_node_props",
            &json!({"nodeType":target.kind,"nodeId":target.id}),
        ) {
            Ok(data) => {
                self.rows = data["properties"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|row| {
                        (
                            row["key"].as_str().unwrap_or("").into(),
                            row["value"]
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| row["value"].to_string()),
                        )
                    })
                    .collect();
                self.committed = self.rows.iter().cloned().collect();
                self.error.clear();
            }
            Err(error) => {
                self.rows.clear();
                self.error = error.to_string();
            }
        }
    }
    pub fn panel(&mut self, ctx: &egui::Context) -> Vec<serde_json::Value> {
        let mut changes = Vec::new();
        if !self.visible || self.target.is_none() {
            return changes;
        }
        let target = self.target.clone().unwrap();
        egui::SidePanel::right("inspector").default_width(300.).width_range(240.0..=600.).resizable(true).show(ctx,|ui| {
            ui.horizontal(|ui|{ui.strong("Inspector");ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.small_button("×").on_hover_text("Close Inspector").clicked(){self.visible=false;}});});
            ui.separator();
            let name=self.rows.iter().find(|(key,_)|key=="name").map(|(_,value)|value.as_str()).unwrap_or(&target.kind);
            ui.label(egui::RichText::new(name).strong());
            ui.weak(&target.kind);
            ui.horizontal(|ui|{ui.selectable_value(&mut self.tab,0,"Common");ui.selectable_value(&mut self.tab,1,"Properties");});
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter properties").desired_width(f32::INFINITY));
            if !self.error.is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,&self.error);}
            egui::ScrollArea::vertical().id_salt("inspector-fields").show(ui,|ui| {
                let common=["name","visible","locked","sel","selection","alpha","material","style","defaultcolor","color"];
                for (key,value) in &mut self.rows {
                    if (self.tab==0 && !common.contains(&key.as_str())) || (!self.filter.is_empty() && !key.to_lowercase().contains(&self.filter.to_lowercase())) {continue;}
                    ui.push_id(key.as_str(),|ui| {
                        let mut changed=false;
                        ui.horizontal(|ui| {
                            let label=ui.add_sized([100.,22.],egui::Label::new(key.as_str()).wrap());
                            if !matches!(key.as_str(), "name" | "sel" | "selection" | "style" | "material") && matches!(value.as_str(),"true"|"false") {let mut flag=value=="true";changed=ui.checkbox(&mut flag,"").changed();if changed{*value=flag.to_string();}}
                            else {
                                let response=ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width())).labelled_by(label.id);
                                self.rects.insert(key.clone(),[response.rect.min.x,response.rect.min.y,response.rect.width(),response.rect.height()]);
                                changed=response.lost_focus() || (response.has_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)));
                            }
                        });
                        if changed && self.committed.get(key)!=Some(value) {changes.push(json!({"nodeType":target.kind,"nodeId":target.id,"prop":key,"value":value}));}
                    });
                }
            });
        });
        changes
    }
}
