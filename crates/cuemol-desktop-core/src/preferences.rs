use crate::DesktopAppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MOUSE_PRESETS: &[&str] = &[
    "DefaultViewInConf",
    "CootViewInConf",
    "PymolViewInConf",
    "IphoneViewInConf",
    "TrackpadViewInConf",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub id: String,
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub dark_mode: bool,
    pub update_check: bool,
    pub console_remote: bool,
    pub tools: ToolPreferences,
    pub plugins: BTreeMap<String, bool>,
    pub mcp_accept_connections: bool,
    pub mcp_port: u16,
    pub drop_target: crate::FileOpenTarget,
    pub shell_target: crate::FileOpenTarget,
    pub atom_label_font: String,
    pub atom_label_size: f64,
    pub atom_label_color: String,
    pub atom_label_italic: bool,
    pub atom_label_bold: bool,
    pub ui_language: String,
    pub use_hidpi: bool,
    pub mouse_preset: String,
    #[serde(default)]
    pub input_device: String,
    pub multitouch_pad: bool,
    pub trackball_radius: f64,
    pub hit_precision: f64,
    pub momentum_scroll: bool,
    pub emulate_right_button: bool,
    pub hover_info: bool,
    pub hover_highlight: bool,
    pub gpu_picking: bool,
    pub shortcuts: Vec<Shortcut>,
}

impl Preferences {
    pub fn from_state(state: &DesktopAppState) -> Self {
        let input = state.view_input_config();
        Self {
            dark_mode: state.config_value("layout:theme") != "light",
            update_check: config_bool(state, "updates:check", true),
            console_remote: config_bool(state, "console:remote-access", false),
            tools: ToolPreferences::from_state(state),
            mcp_accept_connections: state.config_value("mcp:accept-connections") != "false",
            mcp_port: state.mcp_server_port().unwrap_or(27182),
            plugins: state.plugin_states(),
            drop_target: crate::FileOpenTarget::from_config(
                &state.config_value("files:drop-target"),
            ),
            shell_target: crate::FileOpenTarget::from_config(
                &state.config_value("files:shell-target"),
            ),
            atom_label_font: nonempty_or(
                state.style_value(0, "", "DefaultLabel.font_name"),
                "sans-serif",
            ),
            atom_label_size: state
                .style_value(0, "", "DefaultLabel.font_size")
                .parse()
                .unwrap_or(12.0),
            atom_label_color: nonempty_or(state.style_value(0, "", "DefaultLabel.color"), "Yellow"),
            atom_label_italic: matches!(
                state.style_value(0, "", "DefaultLabel.font_style").as_str(),
                "italic" | "oblique"
            ),
            atom_label_bold: state.style_value(0, "", "DefaultLabel.font_weight") == "bold",
            ui_language: nonempty_or(state.config_value("general.useragent.locale"), "en-US"),
            use_hidpi: config_bool(state, "cuemol2.ui.view.use_hidpi", false),
            multitouch_pad: config_bool(state, "cuemol2.ui.mouse-multitouch-pad", false),
            input_device: nonempty_or(
                state.config_value("view_input:device"),
                if config_bool(state, "cuemol2.ui.mouse-multitouch-pad", false) {
                    "trackpad"
                } else {
                    "auto"
                },
            ),
            mouse_preset: nonempty_or(
                state.config_value("cuemol2.ui.viewinconf"),
                "DefaultViewInConf",
            ),
            trackball_radius: input.tbrad(),
            hit_precision: input.hitprec(),
            momentum_scroll: config_bool(state, "cuemol2.ui.mouse-momentum-scroll", false),
            emulate_right_button: config_bool(state, "cuemol2.ui.mouse-emulate-rbutton", false),
            hover_info: config_bool(state, "view_input:hover-info", true),
            hover_highlight: config_bool(state, "view_input:hover-highlight", true),
            gpu_picking: state.resolved_view_input(false).gpu_pick(),
            shortcuts: ["view-trans-plusx", "view-trans-minusx"]
                .into_iter()
                .map(|id| Shortcut {
                    id: id.to_string(),
                    key: state.config_value(&format!("shortcut:{id}:key")),
                    ctrl: config_bool(state, &format!("shortcut:{id}:ctrl"), true),
                    alt: config_bool(state, &format!("shortcut:{id}:alt"), false),
                    shift: config_bool(state, &format!("shortcut:{id}:shift"), false),
                })
                .collect(),
        }
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self::from_state(&DesktopAppState::new())
    }
}

impl Preferences {
    pub fn apply(&self, state: &mut DesktopAppState) -> Result<(), String> {
        let preferences = self;
        validate(preferences)?;
        state
            .set_plugin_states(&preferences.plugins)
            .map_err(|error| error.to_string())?;
        state.put_config_value(
            "layout:theme",
            if self.dark_mode { "dark" } else { "light" },
        );
        state.put_config_value("updates:check", self.update_check.to_string());
        state.put_config_value("console:remote-access", self.console_remote.to_string());
        self.tools.apply(state);
        state.put_config_value(
            "mcp:accept-connections",
            preferences.mcp_accept_connections.to_string(),
        );
        state.put_config_value("mcp:port", preferences.mcp_port.to_string());
        state.put_config_value("files:drop-target", preferences.drop_target.as_str());
        state.put_config_value("files:shell-target", preferences.shell_target.as_str());
        state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_name",
            preferences.atom_label_font.trim(),
        );
        state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_size",
            preferences.atom_label_size.to_string(),
        );
        state.set_style_value(
            0,
            "user",
            "DefaultLabel.color",
            preferences.atom_label_color.trim(),
        );
        state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_style",
            if preferences.atom_label_italic {
                "italic"
            } else {
                "normal"
            },
        );
        state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_weight",
            if preferences.atom_label_bold {
                "bold"
            } else {
                "normal"
            },
        );
        {
            let input = state.view_input_config_mut();
            input.set_style(preferences.mouse_preset.clone());
            input.set_tbrad(preferences.trackball_radius);
            input.set_hitprec(preferences.hit_precision);
        }
        for (key, value) in [
            (
                "style:DefaultLabel.font_name",
                preferences.atom_label_font.trim().to_string(),
            ),
            (
                "style:DefaultLabel.font_size",
                preferences.atom_label_size.to_string(),
            ),
            (
                "style:DefaultLabel.color",
                preferences.atom_label_color.trim().to_string(),
            ),
            (
                "style:DefaultLabel.font_style",
                if preferences.atom_label_italic {
                    "italic".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            (
                "style:DefaultLabel.font_weight",
                if preferences.atom_label_bold {
                    "bold".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            ("general.useragent.locale", preferences.ui_language.clone()),
            (
                "cuemol2.ui.view.use_hidpi",
                preferences.use_hidpi.to_string(),
            ),
            ("cuemol2.ui.viewinconf", preferences.mouse_preset.clone()),
            (
                "view_input:device",
                if preferences.input_device.is_empty() {
                    if preferences.multitouch_pad {
                        "trackpad"
                    } else {
                        "auto"
                    }
                    .into()
                } else {
                    preferences.input_device.clone()
                },
            ),
            (
                "cuemol2.ui.mouse-multitouch-pad",
                preferences.multitouch_pad.to_string(),
            ),
            ("view_input:tbrad", preferences.trackball_radius.to_string()),
            ("view_input:hitprec", preferences.hit_precision.to_string()),
            ("view_input:gpu-pick", preferences.gpu_picking.to_string()),
            ("view_input:hover-info", preferences.hover_info.to_string()),
            (
                "view_input:hover-highlight",
                preferences.hover_highlight.to_string(),
            ),
            (
                "cuemol2.ui.mouse-momentum-scroll",
                preferences.momentum_scroll.to_string(),
            ),
            (
                "cuemol2.ui.mouse-emulate-rbutton",
                preferences.emulate_right_button.to_string(),
            ),
        ] {
            state.put_config_value(key, value);
        }
        for shortcut in &preferences.shortcuts {
            for (field, value) in [
                ("key", shortcut.key.clone()),
                ("ctrl", shortcut.ctrl.to_string()),
                ("alt", shortcut.alt.to_string()),
                ("shift", shortcut.shift.to_string()),
            ] {
                state.put_config_value(&format!("shortcut:{}:{field}", shortcut.id), value);
            }
        }
        if !preferences.hover_info {
            state.set_active_hover_hit(None);
        }
        Ok(())
    }
}

pub fn validate(preferences: &Preferences) -> std::result::Result<(), String> {
    preferences.tools.validate()?;
    if preferences.atom_label_font.trim().is_empty() {
        return Err("an atom-label font is required".to_string());
    }
    if !preferences.atom_label_size.is_finite()
        || !(4.0..=144.0).contains(&preferences.atom_label_size)
    {
        return Err("atom-label size must be between 4 and 144".to_string());
    }
    if preferences.atom_label_color.trim().is_empty() {
        return Err("an atom-label color is required".to_string());
    }
    if !matches!(preferences.ui_language.as_str(), "en-US" | "ja") {
        return Err(format!(
            "unsupported UI language: {}",
            preferences.ui_language
        ));
    }
    if !MOUSE_PRESETS.contains(&preferences.mouse_preset.as_str()) {
        return Err(format!(
            "unknown mouse preset: {}",
            preferences.mouse_preset
        ));
    }
    if !preferences.trackball_radius.is_finite()
        || !(0.1..=10.0).contains(&preferences.trackball_radius)
    {
        return Err("trackball radius must be between 0.1 and 10".to_string());
    }
    if !preferences.hit_precision.is_finite() || !(0.0..=100.0).contains(&preferences.hit_precision)
    {
        return Err("pick precision must be between 0 and 100".to_string());
    }
    if !matches!(
        preferences.input_device.as_str(),
        "" | "auto" | "mouse" | "trackpad"
    ) {
        return Err(format!(
            "unknown input device: {}",
            preferences.input_device
        ));
    }
    const IDS: [&str; 2] = ["view-trans-plusx", "view-trans-minusx"];
    const KEYS: [&str; 4] = ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"];
    if preferences.shortcuts.len() != IDS.len()
        || !IDS.iter().all(|id| {
            preferences
                .shortcuts
                .iter()
                .any(|shortcut| shortcut.id == *id)
        })
    {
        return Err("the complete reference shortcut set is required".to_string());
    }
    for shortcut in &preferences.shortcuts {
        if !KEYS.contains(&shortcut.key.as_str()) {
            return Err(format!("unsupported shortcut key: {}", shortcut.key));
        }
    }
    for (index, shortcut) in preferences.shortcuts.iter().enumerate() {
        if preferences.shortcuts.iter().skip(index + 1).any(|other| {
            shortcut.key == other.key
                && shortcut.ctrl == other.ctrl
                && shortcut.alt == other.alt
                && shortcut.shift == other.shift
        }) {
            return Err(format!("duplicate shortcut: {}", shortcut.key));
        }
    }
    Ok(())
}

fn nonempty_or(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.into()
    } else {
        value
    }
}
fn config_bool(state: &DesktopAppState, key: &str, fallback: bool) -> bool {
    match state.config_value(key).as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_preferences_do_not_partially_change_the_application() {
        let mut state = DesktopAppState::new();
        let original = state.config_text();
        let mut preferences = Preferences::from_state(&state);
        preferences.plugins.insert("catalog".into(), true);
        preferences.atom_label_size = f64::NAN;
        assert!(preferences.apply(&mut state).is_err());
        assert_eq!(state.config_text(), original);
        preferences.atom_label_size = 18.0;
        preferences.plugins.insert("getpdb".into(), false);
        assert!(preferences.apply(&mut state).is_err());
        assert_eq!(state.config_text(), original);
        preferences.plugins.insert("getpdb".into(), true);
        preferences.input_device = "unknown".into();
        assert!(preferences.apply(&mut state).is_err());
        assert_eq!(state.config_text(), original);
    }

    #[test]
    fn preferences_round_trip_across_config_and_live_style_values() {
        let mut state = DesktopAppState::new();
        let mut preferences = Preferences::from_state(&state);
        preferences.dark_mode = false;
        preferences.update_check = false;
        preferences.console_remote = true;
        preferences.tools = ToolPreferences {
            povray: "tools/povray".into(),
            povray_include: "tools/include".into(),
            ffmpeg: "tools/ffmpeg".into(),
            apbs: "tools/apbs".into(),
            pdb2pqr: "tools/pdb2pqr".into(),
            force_field: "amber".into(),
        };
        preferences.atom_label_font = "monospace".into();
        preferences.atom_label_bold = true;
        preferences.atom_label_size = 24.0;
        preferences.input_device = "trackpad".into();
        preferences.trackball_radius = 1.7;
        preferences.hit_precision = 15.0;
        preferences.gpu_picking = false;
        preferences.hover_info = false;
        preferences.plugins.insert("catalog".into(), true);
        preferences.drop_target = crate::FileOpenTarget::New;
        preferences.apply(&mut state).unwrap();
        assert_eq!(state.style_value(0, "", "DefaultLabel.font_size"), "24");
        assert_eq!(state.resolved_view_input(false).tbrad(), 1.7);
        assert!(!state.resolved_view_input(false).gpu_pick());
        let mut reloaded = DesktopAppState::new();
        reloaded.load_config_text(&state.config_text()).unwrap();
        assert_eq!(Preferences::from_state(&reloaded), preferences);
    }
}

pub const FORCE_FIELDS: &[&str] = &["charmm", "amber", "tyl06", "peoepb", "swanson"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolPreferences {
    pub povray: String,
    pub povray_include: String,
    pub ffmpeg: String,
    pub apbs: String,
    pub pdb2pqr: String,
    pub force_field: String,
}
impl Default for ToolPreferences {
    fn default() -> Self {
        Self {
            povray: String::new(),
            povray_include: String::new(),
            ffmpeg: String::new(),
            apbs: String::new(),
            pdb2pqr: String::new(),
            force_field: "charmm".into(),
        }
    }
}
impl ToolPreferences {
    pub fn from_state(state: &DesktopAppState) -> Self {
        Self {
            povray: state.config_value("cuemol2.ui.render.pov-exe-path"),
            povray_include: state.config_value("cuemol2.ui.render.pov-inc-path"),
            ffmpeg: state.config_value("cuemol2.ui.ffmpeg-path"),
            apbs: state.config_value("cuemol2.ui.apbs-exe-path"),
            pdb2pqr: state.config_value("cuemol2.ui.pdb2pqr-py-path"),
            force_field: nonempty_or(state.config_value("apbs:force_field"), "charmm")
                .to_ascii_lowercase(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        if !FORCE_FIELDS.contains(&self.force_field.as_str()) {
            return Err(format!("unknown pdb2pqr force field: {}", self.force_field));
        }
        for path in [
            &self.povray,
            &self.povray_include,
            &self.ffmpeg,
            &self.apbs,
            &self.pdb2pqr,
        ] {
            if path.contains('\0') {
                return Err("tool paths cannot contain a NUL character".into());
            }
        }
        Ok(())
    }
    fn apply(&self, state: &mut DesktopAppState) {
        for (key, value) in [
            ("cuemol2.ui.render.pov-exe-path", &self.povray),
            ("cuemol2.ui.render.pov-inc-path", &self.povray_include),
            ("cuemol2.ui.ffmpeg-path", &self.ffmpeg),
            ("cuemol2.ui.apbs-exe-path", &self.apbs),
            ("cuemol2.ui.pdb2pqr-py-path", &self.pdb2pqr),
            ("apbs:force_field", &self.force_field),
        ] {
            state.put_config_value(key, value.trim());
        }
    }
}

pub(crate) fn restore_runtime(state: &mut DesktopAppState) {
    for key in [
        "font_name",
        "font_size",
        "color",
        "font_style",
        "font_weight",
    ] {
        let name = format!("DefaultLabel.{key}");
        let value = state.config_value(&format!("style:{name}"));
        if !value.is_empty() {
            state.set_style_value(0, "user", &name, value);
        }
    }
    let style = state.config_value("cuemol2.ui.viewinconf");
    if !style.is_empty() {
        state.view_input_config_mut().set_style(style);
    }
    if let Ok(value) = state.config_value("view_input:tbrad").parse::<f64>() {
        if value.is_finite() && value > 0. {
            state.view_input_config_mut().set_tbrad(value);
        }
    }
    if let Ok(value) = state.config_value("view_input:hitprec").parse::<f64>() {
        if value.is_finite() && value >= 0. {
            state.view_input_config_mut().set_hitprec(value);
        }
    }
}
