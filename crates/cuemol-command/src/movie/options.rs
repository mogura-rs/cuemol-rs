use cuemol_desktop_core::DesktopAppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const FORMATS: [(&str, &str); 7] = [
    ("mov_h264", "QuickTime (H.264)"),
    ("mov_h265", "QuickTime (H.265)"),
    ("mov_raw", "QuickTime (raw)"),
    ("mp4_h264", "MP4 (H.264)"),
    ("mp4_h265", "MP4 (H.265)"),
    ("wmv2", "Windows Media"),
    ("gifanim", "Animated GIF"),
];
const PREF_KEY: &str = "movie_render:preferences";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MovieOptions {
    pub use_temp_dir: bool,
    pub output_dir: String,
    pub base_name: String,
    pub fps: f64,
    pub make_movie: bool,
    pub movie_format: String,
    pub dup_last_frame: bool,
    pub bitrate_kbps: u32,
    #[serde(skip)]
    last_custom_dir: String,
}

impl Default for MovieOptions {
    fn default() -> Self {
        Self {
            use_temp_dir: true,
            output_dir: String::new(),
            base_name: "movie".into(),
            fps: 30.,
            make_movie: true,
            movie_format: "mp4_h264".into(),
            dup_last_frame: true,
            bitrate_kbps: 1024,
            last_custom_dir: String::new(),
        }
    }
}

impl MovieOptions {
    pub fn load(state: &DesktopAppState, temporary_dir: &str) -> Self {
        let value =
            serde_json::from_str::<Value>(&state.config_value(PREF_KEY)).unwrap_or_else(|_| {
                let mut value = json!({});
                for (key, old) in [
                    ("outputDir", "output_dir"),
                    ("baseName", "base_name"),
                    ("fps", "fps"),
                    ("makeMovie", "make_movie"),
                    ("movieFormat", "movie_format"),
                    ("dupLastFrame", "duplicate_last_frame"),
                    ("bitrateKbps", "bitrate_kbps"),
                ] {
                    let text = state.config_value(&format!("animation_render:{old}"));
                    if !text.is_empty() {
                        value[key] = if matches!(key, "outputDir" | "baseName" | "movieFormat") {
                            json!(text)
                        } else {
                            serde_json::from_str(&text).unwrap_or(Value::Null)
                        };
                    }
                }
                if value["outputDir"]
                    .as_str()
                    .is_some_and(|v| !v.trim().is_empty())
                {
                    value["useTempDir"] = json!(false);
                }
                value
            });
        Self::from_preferences(&value, temporary_dir)
    }

    pub fn from_preferences(value: &Value, temporary_dir: &str) -> Self {
        let mut options = Self::default();
        options.patch(value);
        if !options.output_dir.trim().is_empty() && !options.use_temp_dir {
            options.last_custom_dir = options.output_dir.clone();
        } else {
            options.use_temp_dir = true;
        }
        if options.use_temp_dir {
            options.output_dir = temporary_dir.into();
        }
        options
    }

    pub fn patch(&mut self, value: &Value) {
        if let Some(v) = value["useTempDir"].as_bool() {
            self.use_temp_dir = v;
        }
        if let Some(v) = value["outputDir"].as_str() {
            self.output_dir = v.into();
        }
        if let Some(v) = value["baseName"].as_str() {
            self.base_name = v.into();
        }
        if let Some(v) = value["fps"].as_f64().filter(|v| v.is_finite() && *v > 0.) {
            self.fps = v;
        }
        if let Some(v) = value["makeMovie"].as_bool() {
            self.make_movie = v;
        }
        if let Some(v) = value["movieFormat"]
            .as_str()
            .filter(|v| FORMATS.iter().any(|(id, _)| id == v))
        {
            self.movie_format = v.into();
        }
        if let Some(v) = value["dupLastFrame"].as_bool() {
            self.dup_last_frame = v;
        }
        if let Some(v) = value["bitrateKbps"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= u32::MAX as u64)
        {
            self.bitrate_kbps = v as u32;
        }
    }

    pub fn preferences(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap();
        if self.use_temp_dir {
            value.as_object_mut().unwrap().remove("outputDir");
        }
        value
    }

    pub fn persist(&self, state: &mut DesktopAppState) {
        state.put_config_value(PREF_KEY, self.preferences().to_string());
    }

    pub fn select_temporary(&mut self, directory: &str) {
        if !self.use_temp_dir {
            self.last_custom_dir = self.output_dir.clone();
        }
        self.use_temp_dir = true;
        self.output_dir = directory.into();
    }

    pub fn select_custom(&mut self) {
        if self.use_temp_dir {
            self.output_dir = self.last_custom_dir.clone();
        }
        self.use_temp_dir = false;
    }

    pub fn set_custom(&mut self, directory: String) {
        self.last_custom_dir = directory.clone();
        self.output_dir = directory;
        self.use_temp_dir = false;
    }

    pub fn validate(&self) -> Result<(), String> {
        base_name(&self.base_name)?;
        if !self.fps.is_finite() || self.fps <= 0. {
            return Err("Frame rate must be positive".into());
        }
        if self.bitrate_kbps == 0 {
            return Err("Movie bitrate must be positive".into());
        }
        if !FORMATS.iter().any(|(id, _)| *id == self.movie_format) {
            return Err("Unknown movie format".into());
        }
        Ok(())
    }
}

pub fn base_name(value: &str) -> Result<&str, String> {
    if value.chars().any(char::is_control) {
        return Err("Movie base name cannot contain control characters".into());
    }
    let value = value.trim();
    let value = if value.is_empty() { "movie" } else { value };
    if value
        .chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|%".contains(c))
        || matches!(value, "." | "..")
    {
        return Err("Movie base name must be a simple file name without percent signs".into());
    }
    Ok(value)
}

pub fn frame_count(duration: f64, fps: f64, duplicate_last_frame: bool) -> Result<u32, String> {
    let last = (duration * fps).floor();
    if !duration.is_finite()
        || duration < 0.
        || !fps.is_finite()
        || fps <= 0.
        || !last.is_finite()
        || last >= i32::MAX as f64
    {
        return Err("Movie duration or frame rate is outside the supported range".into());
    }
    let count = last as u32 + u32::from(duplicate_last_frame);
    if count == 0 {
        return Err("The animation has no frames to render".into());
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_validate_individual_fields_and_do_not_persist_temporary_paths() {
        let mut value = MovieOptions::from_preferences(
            &json!({"fps":29.97,"movieFormat":"missing","bitrateKbps":0,"baseName":"","outputDir":"old-temp"}),
            "this-session",
        );
        assert_eq!(value.fps, 29.97);
        assert_eq!(value.movie_format, "mp4_h264");
        assert_eq!(value.bitrate_kbps, 1024);
        assert_eq!(base_name(&value.base_name).unwrap(), "movie");
        assert_eq!(value.output_dir, "this-session");
        assert!(value.preferences().get("outputDir").is_none());
        value.set_custom("my frames".into());
        value.select_temporary("temporary");
        value.select_custom();
        assert_eq!(value.output_dir, "my frames");
        let mut state = DesktopAppState::new();
        value.persist(&mut state);
        assert_eq!(MovieOptions::load(&state, "new session"), value);
        state.put_config_value(PREF_KEY, "invalid");
        state.put_config_value("animation_render:output_dir", "old folder");
        state.put_config_value("animation_render:fps", "12.5");
        let migrated = MovieOptions::load(&state, "new session");
        assert!(!migrated.use_temp_dir);
        assert_eq!(migrated.output_dir, "old folder");
        assert_eq!(migrated.fps, 12.5);
    }

    #[test]
    fn movie_frame_count_matches_animation_manager_for_fractional_end_times() {
        for (duration, fps) in [(2., 30.), (0.09, 30.), (0.4, 29.97), (0.01, 24.)] {
            let mut animation = cuemol_anim::AnimMgr::default();
            let expected = animation.setup_render(0, cuemol_core::time_from_seconds(duration), fps);
            assert_eq!(frame_count(duration, fps, true).unwrap(), expected as u32);
            let actual = frame_count(duration, fps, false);
            if expected == 1 {
                assert!(actual.is_err());
            } else {
                assert_eq!(actual.unwrap(), expected as u32 - 1);
            }
        }
        for fps in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(frame_count(1., fps, true).is_err());
        }
        assert!(frame_count(1e15, 30., true).is_err());
        for name in ["../x", "x%04d", "x\n", "x:y"] {
            assert!(base_name(name).is_err());
        }
    }
}
