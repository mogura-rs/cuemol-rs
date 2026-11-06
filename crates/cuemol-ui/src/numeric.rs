use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) struct NumericDrag {
    pub(crate) original: String,
    pub(crate) was_default: bool,
    pub(crate) realtime: bool,
}

#[derive(Debug, Default)]
pub(crate) struct NumericEdit {
    pub(crate) id: Option<egui::Id>,
    pub(crate) value: Option<f64>,
    pub(crate) dirty: bool,
    pub(crate) optional_value: Option<f64>,
    pub(crate) drag: Option<NumericDrag>,
    pub(crate) action: Option<(&'static str, NumericDrag, String)>,
    pub(crate) cancelled: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn field(
    ui: &mut egui::Ui,
    key: &str,
    kind: &str,
    value_text: &mut String,
    schema: Option<&Value>,
    original: &str,
    was_default: bool,
    edit: &mut NumericEdit,
    rects: &mut BTreeMap<String, [f32; 4]>,
) -> (egui::Response, bool) {
    let mut value = value_text.parse::<f64>().unwrap();
    let optional = schema.and_then(|schema| schema.get("optional"));
    let mut optional_on = value >= 0.;
    let mut toggled = false;
    if let Some(optional) = optional {
        if optional_on {
            edit.optional_value = Some(value);
        }
        let gate = ui
            .checkbox(&mut optional_on, "")
            .on_hover_text(optional["label"].as_str().unwrap_or("Enabled"));
        record(rects, &format!("Enable {}", key), gate.rect);
        if gate.changed() {
            value = if optional_on {
                edit.optional_value
                    .unwrap_or(optional["on"].as_f64().unwrap_or(0.))
            } else {
                optional["off"].as_f64().unwrap_or(-1.)
            };
            *value_text = value.to_string();
            toggled = true;
        }
        if !optional_on {
            value = edit
                .optional_value
                .unwrap_or(optional["on"].as_f64().unwrap_or(0.));
        }
    }
    if edit.value != Some(value) && !edit.dirty {
        if let Some(id) = edit.id {
            ui.data_mut(|data| data.remove::<String>(id));
        }
    }
    let previous = value;
    let step = schema
        .and_then(|schema| schema["step"].as_f64())
        .unwrap_or(if kind == "integer" { 1. } else { 0.01 });
    let speed = ui.input(|input| {
        if input.modifiers.ctrl || input.modifiers.command {
            schema
                .and_then(|schema| schema["coarseStep"].as_f64())
                .unwrap_or(step * 10.)
        } else if input.modifiers.shift_only() {
            schema
                .and_then(|schema| schema["fineStep"].as_f64())
                .map_or(step, |fine| fine * 10.)
        } else {
            step
        }
    });
    let mut editor = egui::DragValue::new(&mut value)
        .speed(speed)
        .update_while_editing(false);
    if let Some(schema) = schema {
        if let Some((min, max)) = schema["min"].as_f64().zip(schema["max"].as_f64()) {
            editor = editor.range(min..=max).clamp_existing_to_range(false);
        }
        if let Some(decimals) = schema["decimals"].as_u64() {
            editor = editor.max_decimals(decimals as usize);
        }
        if let Some(unit) = schema["unit"].as_str().filter(|unit| !unit.is_empty()) {
            editor = editor.suffix(format!(" {unit}"));
        }
    }
    if kind == "integer" {
        editor = editor.max_decimals(0);
    }
    let response = ui
        .add_enabled_ui(optional.is_none() || optional_on, |ui| {
            ui.add_sized([ui.available_width(), 22.], editor)
        })
        .inner;
    if value != previous && schema.is_some() {
        let fine = schema
            .and_then(|schema| schema["fineStep"].as_f64())
            .unwrap_or(step / 10.);
        if fine.is_finite() && fine > 0. {
            let scale = 10_f64.powf((-fine.log10().floor()).max(0.));
            value = (value * scale).round() / scale;
        }
    }
    if response.drag_started() && !edit.cancelled {
        edit.drag = Some(NumericDrag {
            original: original.to_string(),
            was_default: was_default,
            realtime: schema.is_some_and(|schema| schema["realtime"] == true),
        });
    }
    if edit.cancelled {
        value = original.parse().ok().unwrap_or(previous);
        *value_text = value.to_string();
        if !ui.input(|input| input.pointer.primary_down()) {
            edit.cancelled = false;
        }
        edit.id = Some(response.id);
        edit.value = Some(value);
        edit.dirty = false;
        return (response, false);
    }
    edit.id = Some(response.id);
    edit.value = Some(value);
    edit.dirty = response.has_focus() && (edit.dirty || response.changed() && value == previous);
    if response.changed() && (optional.is_none() || optional_on) {
        *value_text = value.to_string();
    }
    let mut changed =
        toggled || (response.changed() && !response.dragged()) || response.drag_stopped();
    if let Some(drag) = &edit.drag {
        if drag.realtime {
            changed = toggled;
            if response.drag_stopped() {
                edit.action = Some(("commit", drag.clone(), value_text.clone()));
            } else if response.changed() {
                edit.action = Some(("preview", drag.clone(), value_text.clone()));
            }
        }
    }
    if response.drag_stopped() {
        edit.drag = None;
    }
    (response, changed)
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, rect: egui::Rect) {
    rects.insert(
        name.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}
