use crate::{DesktopAppState, MolObject, MolRenderer, MolScene, Selector, StyleManager};
use std::borrow::Cow;

pub const MAX_SELECTION_HISTORY: usize = 100;

fn has_named_references(selector: &Selector) -> bool {
    match selector {
        Selector::Ref(_) => true,
        Selector::Not(inner) | Selector::Unary { inner, .. } | Selector::Distance { inner, .. } => {
            has_named_references(inner)
        }
        Selector::And(left, right) | Selector::Or(left, right) => {
            has_named_references(left) || has_named_references(right)
        }
        _ => false,
    }
}

pub(crate) fn resolve<'a, 's>(
    selector: &'a Selector,
    styles: impl FnOnce() -> &'s StyleManager,
    scope: u64,
) -> cuemol_core::Result<Cow<'a, Selector>> {
    if has_named_references(selector) {
        let registry = styles()
            .str_data_entries("sel", scope)
            .into_iter()
            .filter_map(|(name, expression)| {
                crate::parse_selector(&expression)
                    .ok()
                    .map(|selector| (name, selector))
            })
            .collect();
        selector.resolve_named(&registry).map(Cow::Owned)
    } else {
        Ok(Cow::Borrowed(selector))
    }
}

pub(crate) fn indices<'s>(
    object: &MolObject,
    fallback: Option<&Selector>,
    styles: impl FnOnce() -> &'s StyleManager,
    scene: &MolScene,
) -> Vec<usize> {
    let selector = if object.mol().selection_text().is_empty() {
        fallback
    } else {
        Some(object.mol().selection())
    };
    selector
        .and_then(|selector| resolve(selector, styles, scene.uid().get()).ok())
        .map(|selector| {
            object
                .mol()
                .select_indices(&selector.bind_to_scene(object, scene))
        })
        .unwrap_or_default()
}

pub(crate) fn renderer_selector(
    object: &MolObject,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scene: &MolScene,
) -> cuemol_core::Result<Selector> {
    let parsed = renderer
        .qsc_attr("sel")
        .map(|text| crate::parse_selector(if text.trim().is_empty() { "*" } else { text }))
        .transpose()?;
    let selector = resolve(
        parsed.as_ref().unwrap_or_else(|| renderer.selection()),
        || styles,
        scene.uid().get(),
    )?;
    Ok(selector.bind_to_scene(object, scene).into_owned())
}

impl DesktopAppState {
    pub fn selection_definitions(&self) -> (Vec<String>, Vec<String>) {
        let scene = self
            .active_scene()
            .and_then(|id| self.scene_manager().scene(id));
        let mut styles = cuemol_molvis::default_style_manager();
        if let Some(scene) = scene {
            for (index, loaded) in scene.loaded_styles().iter().enumerate() {
                styles.load_style_set_from_xml(
                    scene.uid().get(),
                    format!("selection-{index}"),
                    &loaded.data,
                    true,
                );
            }
        }
        let names = |scope| styles.str_data_names_in_scope("sel", scope);
        (
            self.active_scene()
                .map_or_else(Vec::new, |id| names(id.get())),
            names(0),
        )
    }

    pub fn selection_history(&self) -> Vec<String> {
        let length = self
            .config_value("selection_history:length")
            .parse::<usize>()
            .unwrap_or(0)
            .min(MAX_SELECTION_HISTORY);
        let mut values = Vec::with_capacity(length);
        for index in 0..length {
            let value = self.config_value(&format!("selection_history:{index}"));
            let value = value.trim();
            if !matches!(value, "" | "*" | "none") && !values.iter().any(|v| v == value) {
                values.push(value.to_string());
            }
        }
        values
    }

    pub fn remember_selection(&mut self, expression: &str) {
        let expression = expression.trim();
        if matches!(expression, "" | "*" | "none") {
            return;
        }
        let mut values = self.selection_history();
        values.retain(|value| value != expression);
        values.insert(0, expression.into());
        self.restore_selection_history(&values);
    }

    pub fn restore_selection_history(&mut self, values: &[String]) {
        let old_length = self
            .config_value("selection_history:length")
            .parse::<usize>()
            .unwrap_or(0)
            .min(MAX_SELECTION_HISTORY);
        let length = values.len().min(MAX_SELECTION_HISTORY);
        self.put_config_value("selection_history:length", length.to_string());
        for index in 0..old_length.max(length) {
            self.put_config_value(
                &format!("selection_history:{index}"),
                values.get(index).cloned().unwrap_or_default(),
            );
        }
    }
}
