use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[path = "property_schema/backbone.rs"]
mod backbone;
#[path = "property_schema/specialized.rs"]
mod specialized;

pub(super) fn sections(state: &DesktopAppState, kind: &str, rows: &[Value]) -> Value {
    if matches!(kind, "object" | "renderer" | "rendGroup") {
        return node_sections(state, kind, rows);
    }
    if kind != "scene" {
        return json!([]);
    }
    json!([
        {"key":"scene-basic", "title":"Basic settings", "expanded":true, "rows":[
            {"key":"name", "label":"Name", "editable":true}
        ]},
        {"key":"scene-ao", "title":"Ambient occlusion", "expanded":true, "rows":[
            {"key":"aoEnabled", "label":"Enabled"},
            {"key":"ao-preset", "label":"Preset", "enabledWhen":"aoEnabled", "options":[
                {"label":"Low", "patch":{"aoRadius":"4", "aoSteps":"3", "aoIntensity":"2.2", "aoHalfRes":"false"}},
                {"label":"Medium", "patch":{"aoRadius":"8", "aoSteps":"4", "aoIntensity":"1.9", "aoHalfRes":"true"}},
                {"label":"High", "patch":{"aoRadius":"12", "aoSteps":"5", "aoIntensity":"1.7", "aoHalfRes":"true"}}
            ]}
        ]},
        {"key":"scene-aa", "title":"Anti-aliasing", "expanded":false, "rows":[
            {"key":"aa-preset", "label":"Quality", "options":[
                {"label":"Off", "patch":{"aa_method":"none", "aaJitterLevel":"0"}},
                {"label":"Standard (FXAA)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"0"}},
                {"label":"High (FXAA + 8x SS)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"3"}},
                {"label":"Ultra (FXAA + 32x SS)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"5"}}
            ]}
        ]},
        {"key":"scene-bg", "title":"Background", "expanded":false, "rows":[
            {"key":"bgcolor", "label":"Color"}
        ]},
        {"key":"scene-proof", "title":"Color proofing", "expanded":false, "rows":[
            {"key":"use_colproof", "label":"Enabled", "seedWhenEnabled":{"icc_filename":"GenericCMYK.icm"}},
            {"key":"icc_filename", "label":"Profile"},
            {"key":"icc_intent", "label":"Intent", "labels":{
                "perceptual":"Perceptual", "relative_colorimetric":"Relative colorimetric",
                "saturation":"Saturation", "absolute_colorimetric":"Absolute colorimetric"
            }}
        ]}
    ])
}

fn row(key: &str, label: &str) -> Value {
    json!({"key":key,"label":label})
}

fn number(key: &str, label: &str, min: f64, max: f64, step: f64, unit: &str) -> Value {
    json!({"key":key,"label":label,"min":min,"max":max,"step":step,"unit":unit})
}

fn section(key: &str, title: &str, rows: Vec<Value>) -> Value {
    json!({"key":key,"title":title,"expanded":true,"rows":rows})
}

struct Properties<'a>(BTreeMap<&'a str, &'a Value>);

impl<'a> Properties<'a> {
    fn value(&self, key: &str) -> &str {
        self.0
            .get(key)
            .and_then(|row| row["value"].as_str())
            .unwrap_or("")
    }

    fn choices(&self, key: &str, label: &str, values: &[&str]) -> Value {
        let mut choices: Vec<_> = values.iter().map(|value| value.to_string()).collect();
        let current = self.value(key);
        if !choices.iter().any(|value| value == current) {
            choices.push(current.into());
        }
        json!({"key":key,"label":label,"choices":choices})
    }

    fn detail(&self, keys: &[&str], label: &str, min: f64) -> Value {
        let keys: Vec<_> = keys
            .iter()
            .copied()
            .filter(|key| self.0.contains_key(key))
            .collect();
        let mut choices: Vec<f64> = [1., 2., 4., 8., 16., 32.]
            .into_iter()
            .filter(|value| *value >= min)
            .collect();
        for key in &keys {
            for field in ["value", "defaultValue"] {
                if let Some(value) = self.0[key][field]
                    .as_str()
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
                {
                    choices.push(value);
                }
            }
        }
        choices.sort_by(f64::total_cmp);
        choices.dedup();
        json!({"key":keys.first().copied().unwrap_or(""),"label":label,"targets":keys,"choices":choices.into_iter().map(|value|value.to_string()).collect::<Vec<_>>()})
    }

    fn gated(&self, mut row: Value, key: &str, value: &str) -> Value {
        row["disabled"] = json!(self.value(key) != value);
        row
    }

    fn radii(&self) -> Vec<Value> {
        [
            ("C", "Carbon"),
            ("N", "Nitrogen"),
            ("O", "Oxygen"),
            ("S", "Sulfur"),
            ("P", "Phosphorus"),
            ("H", "Hydrogen"),
            ("X", "Others"),
        ]
        .into_iter()
        .map(|(symbol, label)| {
            let mut row = number(&format!("vdwr_{symbol}"), label, 0., 3., 0.05, "Å");
            row["decimals"] = json!(2);
            row["fineStep"] = json!(0.01);
            row["coarseStep"] = json!(0.5);
            row
        })
        .collect()
    }

    fn sphere_stick(&self, rings: bool) -> Vec<Value> {
        let mut rows = vec![
            self.detail(&["detail"], "Detail", 2.),
            number("bondw", "Bond width", 0., 3., 0.01, "Å"),
            number("sphr", "Atom radius", 0., 3., 0.01, "Å"),
        ];
        if rings {
            rows.extend([
                row("ring", "Show ring"),
                self.gated(
                    number("thickness", "Thickness", 0., 3., 0.01, "Å"),
                    "ring",
                    "true",
                ),
                self.gated(row("ringcolor", "Ring color"), "ring", "true"),
            ]);
        }
        rows
    }

    fn number(&self, key: &str) -> f64 {
        self.value(key).parse::<f64>().unwrap_or(0.)
    }

    fn mapped(&self, key: &str, label: &str, values: &[(&str, &str)]) -> Value {
        let mut row = self.choices(
            key,
            label,
            &values.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
        );
        row["labels"] = Value::Object(
            values
                .iter()
                .map(|(value, label)| (value.to_string(), json!(label)))
                .collect(),
        );
        row
    }

    fn tube(&self, disabled: bool) -> Vec<Value> {
        let caps = [("flat", "Flat"), ("none", "None"), ("sphere", "Round")];
        let types = [
            ("elliptical", "Elliptical"),
            ("fancy1", "Fancy"),
            ("rectangle", "Rectangle"),
            ("roundsquare", "Round square"),
        ];
        let width = self.number("section.width");
        let minor = self.number("section.tuber") * width;
        let mut major = number("section.width", "Width1", 0.01, 3., 0.01, "Å");
        let mut writes = vec![json!({"prop":"section.width"})];
        if self.0.contains_key("section.tuber") {
            writes.push(json!({"prop":"section.tuber","inverse":minor}));
        }
        major["writes"] = json!(writes);
        let mut minor_row = number("section.tuber", "Width2", 0.01, 3., 0.01, "Å");
        minor_row["display"] = json!(minor.to_string());
        minor_row["writes"] =
            json!([{"prop":"section.tuber","scale":if width > 0. {1./width} else {0.}}]);
        minor_row["disabled"] = json!(width <= 0.);
        let mut sharp = number("section.sharp", "Sharpness", 0., 1., 0.05, "");
        sharp["disabled"] = json!(
            self.0.contains_key("section.type")
                && !["roundsquare", "fancy1"].contains(&self.value("section.type"))
        );
        let mut sections = vec![
            section(
                "tube-main",
                "Tube",
                vec![
                    self.detail(&["axialdetail"], "Axial detail", 2.),
                    number("smooth", "Smoothness", 0., 0.5, 0.01, ""),
                    row("smoothcolor", "Smooth color"),
                    self.mapped("start_captype", "Start cap", &caps),
                    self.mapped("end_captype", "End cap", &caps),
                    row("segend_fade", "Segment-end fade out"),
                    json!({"key":"pivotatom","label":"Pivot atom name","placeholder":"(default)"}),
                ],
            ),
            section(
                "tube-section",
                "Section",
                vec![
                    self.mapped("section.type", "Type", &types),
                    self.detail(&["section.detail"], "Detail", 2.),
                    major,
                    minor_row,
                    sharp,
                ],
            ),
            section(
                "tube-putty",
                "Putty",
                vec![
                    self.mapped(
                        "putty_mode",
                        "Mode",
                        &[("none", "None"), ("linear1", "Linear"), ("scale1", "Scale")],
                    ),
                    self.mapped(
                        "putty_tgt",
                        "Target",
                        &[("bfac", "B-factor"), ("occ", "Occupancy")],
                    ),
                    number("putty_loscl", "Low scale", 0.1, 10., 0.1, ""),
                    number("putty_hiscl", "High scale", 0.1, 10., 0.1, ""),
                ],
            ),
        ];
        for section in &mut sections {
            let putty = section["key"] == "tube-putty";
            for row in section["rows"].as_array_mut().unwrap() {
                row["disabled"] = json!(
                    disabled
                        || row["disabled"] == true
                        || putty
                            && row["key"] != "putty_mode"
                            && self.value("putty_mode") == "none"
                );
            }
        }
        sections
    }

    fn nucleic(&self) -> Vec<Value> {
        let size = self.number("base_size");
        let thick = self.number("base_thick");
        let mut size_row = number("base_size", "Base size", 0.1, 3., 0.1, "Å");
        let mut writes = vec![json!({"prop":"base_size"})];
        if self.0.contains_key("base_thick") && size > 0. {
            writes.push(json!({"prop":"base_thick","scale":thick/size}));
        }
        size_row["writes"] = json!(writes);
        let mut thick_row = number("base_thick", "Base thick", 0., 100., 10., "%");
        thick_row["display"] = json!(if size > 0. { thick * 100. / size } else { 0. }.to_string());
        thick_row["writes"] = json!([{"prop":"base_thick","scale":size/100.}]);
        thick_row["disabled"] = json!(size <= 0.);
        let mut sections = vec![section(
            "nucl-base",
            "Nucleic acid",
            vec![
                row("show_tube", "Show tube"),
                row("show_basepair", "Connect base pair"),
                row("base_type", "Base type"),
                self.detail(&["base_detail"], "Detail", 2.),
                size_row,
                thick_row,
            ],
        )];
        sections.extend(self.tube(self.value("show_tube") == "false"));
        sections
    }
}

fn node_sections(state: &DesktopAppState, kind: &str, rows: &[Value]) -> Value {
    let props = Properties(
        rows.iter()
            .filter_map(|row| row["key"].as_str().map(|key| (key, row)))
            .collect(),
    );
    let renderer_type = props.value("type_name");
    let group = kind == "rendGroup" || renderer_type == "*group";
    let mut basic = vec![row("name", "Name")];
    if !group {
        basic.push(row("sel", "Selection"));
    }
    basic.extend([row("visible", "Visible"), row("locked", "Locked")]);
    if kind == "object" {
        basic.push(row("src", "Linked"));
    } else if !group {
        let mut materials = state
            .active_scene()
            .map(|id| state.scene_material_names(id))
            .unwrap_or_default();
        materials.push(String::new());
        materials.push(props.value("material").into());
        materials.sort();
        materials.dedup();
        basic.extend([
            json!({"key":"material","label":"Material","choices":materials,"labels":{"":"(none)"}}),
            number("alpha", "Opacity", 0., 1., 0.1, ""),
        ]);
    }
    let mut sections = vec![section("common-basic", "Basic settings", basic)];
    if kind == "object" && props.0.contains_key("map_type") {
        sections.extend(props.density_map());
    }
    if kind != "object" && !group {
        if ![
            "simple",
            "trace",
            "spline",
            "contour",
            "gpu_mapmesh",
            "*selection",
            "*unitcell",
            "*namelabel",
        ]
        .contains(&renderer_type)
        {
            let mut width = number("eglinew", "Width", 0., 0.5, 0.01, "Å");
            let mut color = row("egcolor", "Color");
            width["disabled"] = json!(props.value("egtype") == "none");
            color["disabled"] = width["disabled"].clone();
            sections.push(section(
                "common-edge",
                "Edge lines",
                vec![
                    props.choices("egtype", "Edge type", &["none", "edges", "silhouette"]),
                    width,
                    color,
                ],
            ));
        }
        match renderer_type {
            "simple" | "trace" => sections.push(section(
                renderer_type,
                if renderer_type == "simple" {
                    "Simple"
                } else {
                    "Trace"
                },
                vec![number("width", "Line width", 0., 10., 0.2, "px")],
            )),
            "cpk" => sections.extend([
                section("cpk-radii", "Atom radii", props.radii()),
                section(
                    "cpk-detail",
                    "Detail",
                    vec![props.detail(&["detail"], "Detail", 2.)],
                ),
            ]),
            "ballstick" | "anisou" | "tracestick" => {
                let title = match renderer_type {
                    "anisou" => "Atoms and bonds",
                    "tracestick" => "Trace stick",
                    _ => "Ball and stick",
                };
                sections.push(section(
                    renderer_type,
                    title,
                    props.sphere_stick(renderer_type != "tracestick"),
                ));
                if renderer_type == "anisou" {
                    let mut rows = vec![row("drawdisc", "Draw disc")];
                    for (key, label) in
                        [("discscale", "Disc scale"), ("discthick", "Disc thickness")]
                    {
                        let mut row = number(key, label, 0., 3., 0.05, "");
                        row["decimals"] = json!(2);
                        row["fineStep"] = json!(0.01);
                        row["coarseStep"] = json!(0.5);
                        rows.push(props.gated(row, "drawdisc", "true"));
                    }
                    sections.push(section("anisou-disc", "Anisotropic displacement", rows));
                }
            }
            "tube" => sections.extend(props.tube(false)),
            "nucl" => sections.extend(props.nucleic()),
            "ribbon" => sections.extend(props.ribbon()),
            "cartoon" => sections.extend(props.cartoon()),
            "contour" | "gpu_mapmesh" | "isosurf" => {
                sections.extend(props.map(state, renderer_type))
            }
            "dsurface" | "dsurf2" | "directsurf" | "molsurf" => {
                sections.extend(props.surface(state, renderer_type))
            }
            "disorder" => sections.extend(props.disorder(state)),
            "atomintr" => sections.extend(props.interaction()),
            "*namelabel" => sections.extend(props.name_label()),
            "spline" => sections.push(section(
                "spline",
                "Spline",
                vec![
                    props.detail(&["axialdetail"], "Axial detail", 1.),
                    number("smooth", "Smoothness", 0., 0.5, 0.01, ""),
                    row("smoothcolor", "Smooth color"),
                    number("line_width", "Line width", 0., 10., 0.2, "px"),
                    row("segend_fade", "Segment-end fade out"),
                    json!({"key":"pivotatom","label":"Pivot atom name","placeholder":"(default)"}),
                ],
            )),
            _ => {}
        }
    }
    for section in &mut sections {
        section["rows"].as_array_mut().unwrap().retain(|row| {
            row["kind"] == "toggle"
                || row["options"].is_array()
                || row["key"]
                    .as_str()
                    .is_some_and(|key| props.0.contains_key(key))
        });
    }
    sections.retain(|section| !section["rows"].as_array().unwrap().is_empty());
    json!(sections)
}
