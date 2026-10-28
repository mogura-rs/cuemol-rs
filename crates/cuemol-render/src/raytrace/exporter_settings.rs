use super::*;
use cuemol_qsys::{RenderSettings, SettingsClass};

#[derive(Debug, Clone)]
pub struct UmbreonExportOptions {
    pub dpi: f64,
    pub gi_ground_color: String,
    pub hatch_enable: bool,
    pub hatch: HatchRenderParams,
    pub hatch_ink_color: String,
    pub hatch_paper_color: String,
}

impl Default for UmbreonExportOptions {
    fn default() -> Self {
        Self {
            dpi: 0.,
            gi_ground_color: "#666666".into(),
            hatch_enable: false,
            hatch: HatchRenderParams::default(),
            hatch_ink_color: String::new(),
            hatch_paper_color: String::new(),
        }
    }
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    let rgb = (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()?;
    Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}

impl UmbreonSceneExporter {
    pub fn params_mut(&mut self) -> &mut UmbreonRenderParams {
        &mut self.params
    }

    pub fn options(&self) -> &UmbreonExportOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut UmbreonExportOptions {
        &mut self.options
    }

    pub fn set_gi_ground_color(&mut self, value: String) {
        self.params.gi_ground_color = parse_hex(&value)
            .unwrap_or([255; 3])
            .map(|v| v as f64 / 255.);
        self.options.gi_ground_color = value;
    }

    pub fn get_hatch_style_spec(&self, name: &str) -> String {
        hatch_style_spec(name).unwrap_or_default()
    }

    pub fn get_render_log(&self) -> String {
        std::mem::take(&mut self.render_log.lock().unwrap())
    }

    fn log(&self, message: &str) {
        let mut log = self.render_log.lock().unwrap();
        log.push_str(message);
        log.push('\n');
    }

    pub(super) fn prepared_hatch(&self) -> Option<HatchRenderParams> {
        if !self.options.hatch_enable {
            return None;
        }
        let mut hatch = self.options.hatch.clone();
        if hatch_style_spec(&hatch.style).is_err() {
            self.log(&format!(
                "Unknown hatch style '{}'; using richardson",
                hatch.style
            ));
            hatch.style = "richardson".into();
        }
        let mut look = HatchLook::style(&hatch.style).expect("resolved hatch style");
        if let Err(error) = look.apply_spec(&hatch.layers_spec, 1) {
            self.log(&format!("Hatch layers spec ignored: {error}"));
            hatch.layers_spec.clear();
        }
        if let Err(error) = look.apply_spec(&hatch.tone_spec, 6) {
            self.log(&format!("Hatch tone spec ignored: {error}"));
            hatch.tone_spec.clear();
        }
        for scale in [
            &mut hatch.density,
            &mut hatch.width_scale,
            &mut hatch.tone_strength,
            &mut hatch.tone_curve,
        ] {
            if !scale.is_finite() || *scale <= 0. {
                *scale = 1.;
            }
        }
        hatch.custom_ink = false;
        if let Some(color) = parse_hex(&self.options.hatch_ink_color) {
            hatch.ink = color;
            hatch.custom_ink = true;
        }
        hatch.custom_paper = false;
        if let Some(color) = parse_hex(&self.options.hatch_paper_color) {
            hatch.paper = color;
            hatch.custom_paper = true;
        }
        Some(hatch)
    }

    pub(super) fn log_start(&self, params: &UmbreonRenderParams) {
        self.log(&format!(
            "Rendering {}x{} with {}x sampling{}",
            params.width,
            params.height,
            params.supersample.max(1),
            if self.options.hatch_enable {
                " and hatching"
            } else {
                ""
            }
        ));
        if self.options.hatch_enable && params.use_gi {
            self.log("GI is disabled while hatching is enabled");
        }
        if params.aa_mode > 0 && params.use_gi && !self.options.hatch_enable {
            self.log("Adaptive AA uses grid sampling while GI is enabled");
        }
        if params.ao_samples > 0
            && (params.ao_res_div < 0 || params.ao_res_div > 1)
            && params.use_gi
            && !self.options.hatch_enable
        {
            self.log("Coarse AO uses per-hit gathering while GI is enabled");
        }
    }

    pub(super) fn log_result(log: &Mutex<String>, result: &Result<UmbreonFrame>, cancelled: bool) {
        let message = if cancelled {
            "Render cancelled".to_string()
        } else {
            match result {
                Ok(frame) => format!(
                    "Rendered {}x{} in {:.3} seconds",
                    frame.width, frame.height, frame.render_seconds
                ),
                Err(error) => format!("Render failed: {error}"),
            }
        };
        let mut log = log.lock().unwrap();
        log.push_str(&message);
        log.push('\n');
    }

    pub fn apply_render_settings(
        &mut self,
        settings: &RenderSettings,
        backend: &str,
    ) -> Result<&'static str> {
        if settings.class() != SettingsClass::Render {
            return Err(CueMolError::invalid_argument(
                "applyRenderSettings requires RenderSettings",
            ));
        }
        let backend = match backend {
            "" => {
                if settings.get("backend") == Some("umbreon_npr") {
                    "umbreon_npr"
                } else {
                    "umbreon"
                }
            }
            "umbreon" => "umbreon",
            "umbreon_npr" => "umbreon_npr",
            _ => {
                return Err(CueMolError::invalid_argument(format!(
                    "applyRenderSettings: unknown backend '{backend}'"
                )))
            }
        };
        let npr = backend == "umbreon_npr";
        self.params.perspective = settings.get("projection") == Some("perspective");
        self.params.use_clip_z = settings.get("clipPlane") == Some("true");
        self.params.show_edge_lines = settings.get("edgeLines") == Some("true");
        self.params.transparent_background = settings.get("transparentBg") == Some("true");
        let dpi = settings
            .get("dpi")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(600.);
        let factor = match settings.get("unit") {
            Some("in") => dpi,
            Some("mm") => dpi / 25.4,
            Some("cm") => dpi / 2.54,
            _ => 1.,
        };
        let pixels = |key| {
            let value = settings
                .get(key)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.)
                * factor;
            if value.is_finite() {
                value.round().clamp(1., i32::MAX as f64) as i32
            } else {
                1
            }
        };
        self.options.dpi = dpi;
        self.set_width(pixels("width"));
        self.set_height(pixels("height"));
        let get = |key: &str| settings.get(&format!("{backend}.{key}")).unwrap_or("");
        macro_rules! set {
            ($field:ident, $name:literal) => {
                if let Ok(value) = get($name).parse() {
                    self.params.$field = value;
                }
            };
        }
        set!(supersample, "supersample");
        if get("aoEnabled") == "true" {
            set!(ao_samples, "aoSamples");
            set!(ao_distance, "aoDistance");
            set!(ao_intensity, "aoIntensity");
            set!(ao_diffuse_factor, "aoDiffuseFactor");
            set!(ao_multi_scale, "aoMultiScale");
            set!(ao_bent_normal, "aoBentNormal");
            set!(ao_low_discrepancy, "aoLowDiscrepancy");
            self.params.ao_res_div = if get("aoGather") == "Per shading hit" {
                0
            } else {
                -1
            };
        } else {
            self.params.ao_samples = 0;
        }
        set!(shadows, "shadows");
        set!(shadow_samples, "shadowSamples");
        set!(light_radius, "lightRadius");
        set!(light_intensity, "lightIntensity");
        set!(flash_fraction, "flashFraction");
        self.params.use_gi = !npr && get("useGI") == "true";
        if self.params.use_gi {
            set!(ambient_fraction, "ambientFraction");
        } else {
            self.params.ambient_fraction = 0.16;
        }
        set!(crease_limit, "creaseLimit");
        set!(edge_rise, "edgeRise");
        set!(contact_edges, "contactEdges");
        set!(outline_far_depth, "outlineFarDepth");
        set!(per_pixel_blend, "perPixelBlend");
        self.options.hatch_enable = npr;
        if npr {
            let hatch = &mut self.options.hatch;
            hatch.style = get("hatchStyle").into();
            hatch.density = get("hatchDensity").parse().unwrap_or(hatch.density);
            hatch.width_scale = get("hatchWidthScale").parse().unwrap_or(hatch.width_scale);
            let (base, ink) = match get("hatchColoring") {
                "Ink on paper" => ("paper", "fixed"),
                "Colored ink on paper" => ("paper", "albedo"),
                "Ink on color fill" => ("albedo", "fixed"),
                "Colored ink on color fill" => ("albedo", "albedo"),
                _ => ("", ""),
            };
            hatch.base = base.into();
            hatch.ink_model = ink.into();
            hatch.default_edges = get("hatchDefaultEdges") == "true";
            hatch.layers_spec = get("hatchLayersSpec").into();
            hatch.tone_spec = get("hatchToneSpec").into();
            self.options.hatch_ink_color = if get("hatchCustomInk") == "true" {
                get("hatchInkColor").into()
            } else {
                String::new()
            };
            self.options.hatch_paper_color = if get("hatchCustomPaper") == "true" {
                get("hatchPaperColor").into()
            } else {
                String::new()
            };
        } else {
            set!(gi_samples, "giSamples");
            if self.params.gi_samples <= 0 {
                self.params.gi_samples = 32;
            }
            self.set_denoise_mode(UmbreonDenoiseMode::from_label_or_default(get("denoise")));
            set!(gi_sky_gradient, "giSkyGradient");
            self.set_gi_ground_color(get("giGroundColor").into());
        }
        Ok(backend)
    }
}

pub(super) fn render(
    input: &UmbreonRenderInput,
    params: &UmbreonRenderParams,
    hatch: Option<&HatchRenderParams>,
    async_state: Option<(&AtomicU32, &AtomicBool)>,
) -> Result<UmbreonFrame> {
    let hatch = hatch
        .map(|h| npr::ResolvedHatch::new(h, params.supersample))
        .transpose()?;
    let (mut params, background) = hatch
        .as_ref()
        .map(|h| h.render_settings(params, input.background))
        .unwrap_or_else(|| (params.clone(), input.background));
    if hatch.is_some() {
        params.use_gi = false;
    }
    render_with_hatch(
        &input.snapshot,
        &input.camera,
        background,
        &params,
        async_state,
        hatch.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_switches_preserve_ao_and_pin_direct_lighting() {
        let mut exporter = UmbreonSceneExporter::new();
        exporter.scene_mut().set_camera_name("saved");
        let mut settings = RenderSettings::default();
        for (name, value) in [
            ("width", "25.4"),
            ("height", "12.7"),
            ("unit", "mm"),
            ("dpi", "100"),
            ("umbreon.aoEnabled", "true"),
            ("umbreon.aoSamples", "17"),
            ("umbreon.aoDistance", "4"),
            ("umbreon.useGI", "false"),
            ("umbreon.ambientFraction", "0.9"),
        ] {
            settings.set(name, value).unwrap();
        }
        assert_eq!(
            exporter.apply_render_settings(&settings, "").unwrap(),
            "umbreon"
        );
        assert_eq!((exporter.width(), exporter.height()), (100, 50));
        assert_eq!(exporter.scene().camera_name(), "saved");
        assert_eq!(exporter.params.ao_samples, 17);
        assert_eq!(exporter.params.ao_res_div, -1);
        assert_eq!(exporter.params.ambient_fraction, 0.16);
        settings.set("umbreon.aoEnabled", "false").unwrap();
        settings.set("umbreon.aoDistance", "99").unwrap();
        settings.set("umbreon.useGI", "true").unwrap();
        exporter
            .apply_render_settings(&settings, "umbreon")
            .unwrap();
        assert_eq!(exporter.params.ao_samples, 0);
        assert_eq!(exporter.params.ao_distance, 4.);
        assert_eq!(exporter.params.ambient_fraction, 0.9);
        let previous = exporter.params.clone();
        assert!(exporter.apply_render_settings(&settings, "povray").is_err());
        assert_eq!(exporter.params, previous);
        settings.set("backend", "umbreon_npr").unwrap();
        settings.set("umbreon_npr.useGI", "true").unwrap();
        settings
            .set("umbreon_npr.hatchColoring", "Colored ink on color fill")
            .unwrap();
        assert_eq!(
            exporter.apply_render_settings(&settings, "").unwrap(),
            "umbreon_npr"
        );
        assert!(!exporter.params.use_gi);
        assert!(exporter.options.hatch_enable);
        assert_eq!(exporter.options.hatch.base, "albedo");
        assert_eq!(exporter.options.hatch.ink_model, "albedo");
        assert_eq!(exporter.params.ambient_fraction, 0.16);
    }

    #[test]
    fn hatch_fallbacks_log_once_and_explicit_models_control_the_image() {
        let mut exporter = UmbreonSceneExporter::new();
        exporter.set_gi_ground_color("invalid".into());
        assert_eq!(exporter.params.gi_ground_color, [1.; 3]);
        assert_eq!(exporter.options.gi_ground_color, "invalid");
        exporter.options.hatch_enable = true;
        exporter.options.hatch.style = "unknown-look".into();
        exporter.options.hatch.layers_spec = "layer: no-such-key=12".into();
        exporter.options.hatch_ink_color = "#ff0000".into();
        exporter.options.hatch_paper_color = "#00ff00".into();
        exporter.options.hatch.ink_model = "albedo".into();
        let hatch = exporter.prepared_hatch().unwrap();
        assert_eq!(hatch.style, "richardson");
        assert!(hatch.layers_spec.is_empty());
        assert_eq!(hatch.ink_model, "albedo");
        assert_eq!(hatch.paper, [0, 255, 0]);
        let log = exporter.get_render_log();
        assert!(log.contains("Unknown hatch style"));
        assert!(log.contains("layers spec ignored"));
        assert!(exporter.get_render_log().is_empty());
        assert!(exporter.get_hatch_style_spec("unknown-look").is_empty());
        assert!(exporter.get_hatch_style_spec("manga").contains("layer:"));
        let mut camera = Camera::new();
        camera.set_zoom(5.);
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.atoms.push(crate::RenderAtom::new(
            Vector4::ZERO,
            1.,
            Some(cuemol_gfx::parse_color("blue").unwrap()),
        ));
        let params = UmbreonRenderParams {
            width: 24,
            height: 24,
            supersample: 1,
            use_gi: true,
            ..Default::default()
        };
        let input = UmbreonRenderInput {
            snapshot,
            camera,
            background: [0., 0., 0., 1.],
        };
        let frame = render(&input, &params, Some(&hatch), None).unwrap();
        assert_eq!(&frame.pixels[..3], &[0, 255, 0]);
        let mut fixed = hatch.clone();
        fixed.ink_model = "fixed".into();
        let other = render(&input, &params, Some(&fixed), None).unwrap();
        assert_ne!(frame.pixels, other.pixels);
        let bytes = encode_umbreon_png_with_dpi(&frame, 100.).unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert_eq!(reader.info().pixel_dims.unwrap().xppu, 3937);
    }
}
