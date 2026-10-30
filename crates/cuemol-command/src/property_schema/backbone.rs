use super::{number, row, section, Properties};
use serde_json::{json, Value};

const CAPS: &[(&str, &str)] = &[("flat", "Flat"), ("none", "None"), ("sphere", "Round")];
const SHAPES: &[(&str, &str)] = &[
    ("elliptical", "Elliptical"),
    ("fancy1", "Fancy"),
    ("rectangle", "Rectangle"),
    ("roundsquare", "Round square"),
];
const SHAPES_NO_FANCY: &[(&str, &str)] = &[
    ("elliptical", "Elliptical"),
    ("roundsquare", "Round square"),
    ("rectangle", "Rectangle"),
];

impl Properties<'_> {
    fn targets(&self, mut row: Value, keys: &[String]) -> Value {
        let keys: Vec<_> = keys
            .iter()
            .filter(|key| self.0.contains_key(key.as_str()))
            .collect();
        row["key"] = json!(keys.first().map(|key| key.as_str()).unwrap_or(""));
        row["targets"] = json!(keys);
        row
    }

    fn junction(&self, prefixes: &[&str], side: Option<&str>) -> Vec<Value> {
        let keys = |field: &str| {
            prefixes
                .iter()
                .map(|prefix| format!("{prefix}.{field}"))
                .collect::<Vec<_>>()
        };
        let label = |field: &str| {
            side.map_or_else(
                || field.to_string(),
                |side| format!("{side} {}", field.to_lowercase()),
            )
        };
        let types = keys("type");
        let mut rows = vec![
            self.targets(
                self.mapped(
                    &types[0],
                    &label(if side.is_some() { "Type" } else { "Cap type" }),
                    &[("smooth", "Round"), ("flat", "Flat"), ("arrow", "Arrow")],
                ),
                &types,
            ),
            self.targets(
                number(
                    &format!("{}.gamma", prefixes[0]),
                    &label(if side.is_some() { "Power" } else { "Cap power" }),
                    0.1,
                    10.,
                    0.1,
                    "",
                ),
                &keys("gamma"),
            ),
        ];
        for (key, name, scale, offset, display_scale, display_offset) in [
            ("basw", "Arrow height", -0.01, 1., -100., 100.),
            ("arrow", "Arrow width", 0.02, 1., 50., -50.),
        ] {
            let targets = keys(key);
            let mut item = self.targets(
                number(&targets[0], &label(name), 0., 100., 10., "%"),
                &targets,
            );
            item["display"] =
                json!((self.number(&targets[0]) * display_scale + display_offset).to_string());
            item["writes"] = json!(targets
                .iter()
                .filter(|key| self.0.contains_key(key.as_str()))
                .map(|key| json!({"prop":key,"scale":scale,"offset":offset}))
                .collect::<Vec<_>>());
            item["disabled"] = json!(self.value(&types[0]) != "arrow");
            item["decimals"] = json!(0);
            rows.push(item);
        }
        rows
    }

    fn shape(
        &self,
        prefix: &str,
        fancy: bool,
        detail: Option<f64>,
        width: Option<(f64, &str)>,
        sharp_types: Option<&[&str]>,
    ) -> Vec<Value> {
        let name = |field| format!("{prefix}.{field}");
        let mut rows = vec![self.mapped(
            &name("type"),
            "Section type",
            if fancy { SHAPES } else { SHAPES_NO_FANCY },
        )];
        if let Some(min) = detail {
            rows.push(self.detail(&[&name("detail")], "Section detail", min));
        }
        if let Some((max, label)) = width {
            rows.push(number(&name("width"), label, 0., max, 0.05, "Å"));
        }
        rows.push(number(&name("tuber"), "Tuber", 0.2, 10., 0.1, ""));
        let mut sharp = number(&name("sharp"), "Sharpness", 0., 1., 0.05, "");
        sharp["disabled"] = json!(sharp_types
            .is_some_and(|types| self.0.contains_key(name("type").as_str())
                && !types.contains(&self.value(&name("type")))));
        rows.push(sharp);
        rows
    }

    pub(super) fn ribbon(&self) -> Vec<Value> {
        let caps = self.targets(
            self.mapped(
                "start_captype",
                "Cap type",
                &[("flat", "Flat"), ("sphere", "Round"), ("none", "None")],
            ),
            &["start_captype".into(), "end_captype".into()],
        );
        let mut sections = vec![section(
            "ribbon-main",
            "Ribbon",
            vec![
                self.detail(
                    &["coil.detail", "helix.detail", "sheet.detail"],
                    "Section detail",
                    2.,
                ),
                self.detail(&["axialdetail"], "Axial detail", 2.),
                row("smoothcolor", "Smooth color"),
                json!({"key":"pivotatom","label":"Pivot atom name","placeholder":"(default)"}),
                caps,
                row("segend_fade", "Segment-end fade out"),
            ],
        )];
        for (prefix, title, color) in [
            (
                "helix",
                "Helix",
                Some(("helix_usebackcol", "helix_backcol", "Back color")),
            ),
            (
                "sheet",
                "Sheet",
                Some(("sheet_usesidecol", "sheet_sidecol", "Side color")),
            ),
            ("coil", "Coil", None),
        ] {
            let mut rows = self.shape(
                prefix,
                prefix != "coil",
                None,
                Some((5., "Width")),
                Some(&["roundsquare", "fancy1"]),
            );
            if let Some((use_key, key, label)) = color {
                rows.splice(
                    1..1,
                    [
                        row(use_key, &format!("Use {}", label.to_lowercase())),
                        self.gated(row(key, label), use_key, "true"),
                    ],
                );
            }
            rows.push(number(
                &format!("{prefix}_smooth"),
                "Smoothness",
                0.,
                0.5,
                0.01,
                "",
            ));
            if prefix != "coil" {
                rows.extend(self.junction(&[&format!("{prefix}head")], Some("Head")));
                rows.extend(self.junction(&[&format!("{prefix}tail")], Some("Tail")));
            }
            sections.push(section(&format!("ribbon-{prefix}"), title, rows));
        }
        sections
    }

    pub(super) fn cartoon(&self) -> Vec<Value> {
        let mut weight = number("anchor_weight", "Anchor weight", 0., 20., 1., "");
        weight["disabled"] = json!(["", "none"].contains(&self.value("anchor_sel")));
        let main = section(
            "cartoon-main",
            "Cartoon",
            vec![
                self.detail(&["axialdetail"], "Axial detail", 2.),
                row("smoothcolor", "Smooth color"),
                json!({"key":"pivotatom","label":"Pivot atom name","placeholder":"(default)"}),
                self.mapped("start_captype", "Start cap", CAPS),
                self.mapped("end_captype", "End cap", CAPS),
                row("anchor_sel", "Anchor selection"),
                weight,
            ],
        );
        let mut helix = vec![self.mapped(
            "helix_ribbon",
            "Type",
            &[("false", "Cylinder"), ("true", "Ribbon")],
        )];
        if self.value("helix_ribbon") == "true" {
            helix.extend(self.shape(
                "ribhelix",
                true,
                Some(4.),
                Some((5., "Section width")),
                None,
            ));
            helix.extend(self.junction(&["ribhelix_head", "ribhelix_tail"], None));
        } else {
            helix.extend([
                number("helix_smooth", "Smoothing", -5., 5., 0.1, ""),
                number("helix_extend", "Extend", 0., 3., 0.05, "Å"),
            ]);
            helix.extend(self.shape("helix", false, Some(4.), None, Some(&["roundsquare"])));
            helix.extend([
                self.mapped(
                    "helix_width_mode",
                    "Width mode",
                    &[
                        ("average", "Average"),
                        ("const", "Constant"),
                        ("wavy", "Wavy"),
                    ],
                ),
                number("helix_wplus", "Add width", -2., 3., 0.05, "Å"),
                self.gated(
                    number("helix_wsmooth", "Width smooth", -5., 5., 0.1, ""),
                    "helix_width_mode",
                    "wavy",
                ),
            ]);
        }
        let mut sections = vec![main, section("cartoon-helix", "Helix", helix)];
        for (prefix, title, min) in [("sheet", "Sheet", 2.), ("coil", "Coil", 4.)] {
            let mut rows = vec![number(
                &format!("{prefix}_smooth"),
                "Smoothing",
                -5.,
                5.,
                0.1,
                "",
            )];
            rows.extend(self.shape(
                prefix,
                false,
                Some(min),
                Some((3., "Section width")),
                Some(&["roundsquare"]),
            ));
            if prefix == "sheet" {
                rows.push(number("sheet_wsmooth", "Width smooth", -5., 5., 0.1, ""));
                rows.extend(self.junction(&["sheethead"], None));
            }
            sections.push(section(&format!("cartoon-{prefix}"), title, rows));
        }
        sections
    }
}
