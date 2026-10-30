use super::*;

fn toggle(key: &str, label: &str, on: bool, enabled: Value, disabled: Value) -> Value {
    json!({"key":key,"kind":"toggle","label":label,"display":on.to_string(),"patches":{"true":enabled,"false":disabled}})
}

fn molecule_names(state: &DesktopAppState) -> Vec<&str> {
    state
        .active_scene()
        .and_then(|id| state.scene_manager().scene(id))
        .into_iter()
        .flat_map(|scene| scene.objects())
        .map(|object| object.name())
        .filter(|name| !name.is_empty())
        .collect()
}

impl Properties<'_> {
    pub(super) fn density_map(&self) -> Vec<Value> {
        vec![section(
            "densitymap-main",
            "Density map",
            vec![
                self.mapped(
                    "map_type",
                    "Map type",
                    &[
                        ("auto", "Auto (from header)"),
                        ("xtal", "Crystallographic (periodic)"),
                        ("em", "Cryo-EM (whole map, level of detail)"),
                    ],
                ),
                self.map_kind("Effective kind"),
            ],
        )]
    }

    fn map_kind(&self, label: &str) -> Value {
        let mut field = row("map_type_resolved", label);
        field["display"] = json!(match self.value("map_type_resolved") {
            "em" => "Cryo-EM",
            "xtal" => "Crystallographic",
            value => value,
        });
        field
    }

    fn molecule_choice(&self, state: &DesktopAppState, key: &str, label: &str) -> Value {
        let mut values = molecule_names(state);
        if self.value(key).is_empty() {
            values.insert(0, "");
        }
        self.choices(key, label, &values)
    }

    pub(super) fn map(&self, state: &DesktopAppState, kind: &str) -> Vec<Value> {
        let full = self.0.get("region_mode_resolved").map_or_else(
            || self.value("region_mode"),
            |_| self.value("region_mode_resolved"),
        ) == "full";
        let auto = self.value("autoupdate") == "true";
        let drag = self.value("dragupdate") == "true";
        let center = json!({"key":"center-update","label":"Center update","display":if !auto {"None"} else if drag {"Automatic (drag)"} else {"Automatic"},"reset":["autoupdate","dragupdate"],"options":[
            {"label":"None","patch":{"autoupdate":"false","dragupdate":"false"}},
            {"label":"Automatic","patch":{"autoupdate":"true","dragupdate":"false"}},
            {"label":"Automatic (drag)","patch":{"autoupdate":"true","dragupdate":"true"}}
        ]});
        let mut rows = vec![
            self.map_kind("Map kind"),
            center,
            self.mapped(
                "region_mode",
                "Region",
                &[
                    ("auto", "Auto"),
                    ("box", "Box around center"),
                    ("full", "Full map"),
                ],
            ),
            self.mapped(
                "lod",
                "Level of detail",
                &[
                    ("auto", "Auto"),
                    ("step1", "1 (full resolution)"),
                    ("step2", "2"),
                    ("step4", "4"),
                    ("step8", "8"),
                ],
            ),
        ];
        if full {
            rows.extend([
                number("lod_budget", "LoD budget", 1., 256., 1., "Mcell"),
                row("zoom_refine", "Refine on zoom"),
            ]);
        }
        if kind == "isosurf" {
            let mut width = number("width", "Line/Point size", 0., 10., 0.1, "px");
            width["disabled"] = json!(self.value("drawmode") == "fill");
            rows.extend([row("drawmode", "Drawing mode"), width]);
            if !full {
                rows.push(number("max_grids", "Max grid size", 50., 1000., 10., ""));
            }
            rows.extend([
                self.mapped(
                    "cap_mode",
                    "Cap mode",
                    &[("auto", "Auto"), ("on", "On"), ("off", "Off")],
                ),
                row("cullface", "Back-face culling"),
            ]);
        } else {
            rows.push(number("width", "Line width", 0., 10., 0.1, "px"));
            if !full {
                rows.push(number("bufsize", "Buffer size", 50., 200., 10., ""));
            }
        }
        if !full {
            rows.push(row("use_pbc", "Use periodic boundary"));
        }
        if self.0.contains_key("bndry_molname") {
            let names = molecule_names(state);
            let has_target = !self.value("bndry_molname").is_empty();
            let on = if has_target || names.is_empty() {
                json!({})
            } else {
                json!({"bndry_molname":names[0]})
            };
            let mut off = json!({"bndry_molname":""});
            if self.0.contains_key("bndry_sel") {
                off["bndry_sel"] = json!("");
            }
            let mut limit = toggle("limit-display", "Limit display by", has_target, on, off);
            limit["local"] = json!(true);
            rows.push(limit);
            for mut field in [
                self.molecule_choice(state, "bndry_molname", "Target"),
                row("bndry_sel", "Selection"),
                number("bndry_rng", "Distance", 0., 10., 0.1, "Å"),
            ] {
                field["enabledWhen"] = json!("limit-display");
                rows.push(field);
            }
        }
        vec![section(
            kind,
            match kind {
                "isosurf" => "Isosurf",
                "gpu_mapmesh" => "GPU contour",
                _ => "Contour",
            },
            rows,
        )]
    }

    pub(super) fn surface(&self, state: &DesktopAppState, kind: &str) -> Vec<Value> {
        let mut width = number("width", "Line/Point size", 0., 10., 0.1, "px");
        width["decimals"] = json!(1);
        width["disabled"] = json!(self.value("drawmode") == "fill");
        let mut rows = vec![
            self.mapped(
                "drawmode",
                "Drawing mode",
                &[("fill", "Fill"), ("line", "Wireframe"), ("point", "Dots")],
            ),
            width,
        ];
        if kind == "molsurf" {
            rows.extend([
                self.molecule_choice(state, "target", "Selection mol"),
                row("showsel", "Selection"),
            ]);
        } else {
            let mut detail = self.detail(&["detail"], "Detail", 1.);
            let values = detail["choices"].as_array_mut().unwrap();
            if !values.contains(&json!("24")) {
                values.push(json!("24"));
            }
            values.sort_by(|a, b| {
                a.as_str()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
                    .total_cmp(&b.as_str().unwrap().parse::<f64>().unwrap())
            });
            rows.extend([
                self.mapped(
                    "surfalgor",
                    "Algorithm",
                    &[
                        ("distfield", "Distance field"),
                        ("meshms", "MeshMS (analytic SES)"),
                        ("edtsurf", "EDTSurf (voxel)"),
                    ],
                ),
                self.mapped(
                    "surftype",
                    "Surface type",
                    &[
                        ("vdw", "van der Waals"),
                        ("sas", "Solvent accessible"),
                        ("ses", "Solvent excluded"),
                    ],
                ),
                detail,
            ]);
        }
        let mut sections = vec![section(
            "surface-main",
            if kind == "molsurf" {
                "MolSurf"
            } else {
                "Surface"
            },
            rows,
        )];
        if kind != "molsurf" {
            sections.push(section("surface-radii", "Atom radii", self.radii()));
        }
        sections
    }

    pub(super) fn disorder(&self, state: &DesktopAppState) -> Vec<Value> {
        let id = self.value("uid").parse::<u64>().unwrap_or_default();
        let mut names = vec![""];
        if let Some(scene) = state
            .active_scene()
            .and_then(|id| state.scene_manager().scene(id))
        {
            if let Some(object) = scene.objects().iter().find(|object| {
                object
                    .renderers()
                    .iter()
                    .any(|renderer| renderer.uid().get() == id)
            }) {
                names.extend(
                    object
                        .renderers()
                        .iter()
                        .filter(|renderer| {
                            ["tube", "ribbon", "cartoon", "nucl"]
                                .contains(&renderer.renderer_type())
                        })
                        .map(|renderer| renderer.name()),
                );
            }
        }
        let mut target = self.choices("target", "Target", &names);
        target["labels"] = json!({"":"(none)"});
        let mut optional = number("loopsize2", "Loop size 2", 0., 10., 0.1, "Å");
        optional["optional"] = json!({"off":-1.,"on":2.,"label":"Use a separate loop size 2"});
        vec![section(
            "disorder-main",
            "Disorder",
            vec![
                target,
                self.detail(&["detail"], "Detail", 2.),
                number("width", "Dot size", 0., 3., 0.1, "Å"),
                number("dotsep", "Dot separation", 0., 3., 0.1, "Å"),
                number("loopsize", "Loop size", 0., 10., 0.1, "Å"),
                optional,
                row("defaultcolor", "Color"),
            ],
        )]
    }

    fn font(&self) -> Vec<Value> {
        let mut size = number("font_size", "Font size", 1., 72., 1., "pt");
        size["decimals"] = json!(0);
        let mut families = cuemol_desktop_core::font_catalog::families();
        if !families
            .iter()
            .any(|family| family == self.value("font_name"))
        {
            families.insert(0, self.value("font_name").into());
        }
        let mut font = self.choices(
            "font_name",
            "Font name",
            &families.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        font["font"] = json!(true);
        vec![
            size,
            font,
            self.mapped(
                "font_style",
                "Font style",
                &[("normal", "Normal"), ("italic", "Italic")],
            ),
            self.mapped(
                "font_weight",
                "Font weight",
                &[("normal", "Normal"), ("bold", "Bold")],
            ),
        ]
    }

    pub(super) fn interaction(&self) -> Vec<Value> {
        let simple = self.0.contains_key("mode") && self.value("mode") != "fancy";
        let arrow = self.value("captype_start") == "arrow" || self.value("captype_end") == "arrow";
        let mut sections = vec![section(
            "atomintr-main",
            "Interaction",
            vec![
                self.mapped(
                    "mode",
                    "Mode",
                    &[("simple", "Simple line"), ("fancy", "3D tube")],
                ),
                number(
                    "width",
                    "Width",
                    0.,
                    5.,
                    0.05,
                    if simple { "px" } else { "Å" },
                ),
                row("color", "Color"),
                row("showlabel", "Show label"),
            ],
        )];
        let mut on = serde_json::Map::new();
        let mut off = serde_json::Map::new();
        let mut dashed = false;
        let mut cells = Vec::new();
        for index in 0..6 {
            let key = format!("stipple{index}");
            if !self.0.contains_key(key.as_str()) {
                continue;
            }
            dashed |= self.number(&key) >= 0.;
            on.insert(key.clone(), json!(if index < 2 { "1" } else { "-1" }));
            off.insert(key.clone(), json!("-1"));
            cells.push(json!({"key":key,"label":format!("{} {}", if index%2 == 0 {"Dash"} else {"Gap"}, index/2+1),"blankNegative":true,"enabledWhen":"dash-pattern"}));
        }
        if !cells.is_empty() {
            cells.insert(
                0,
                toggle("dash-pattern", "Dashed", dashed, json!(on), json!(off)),
            );
            sections.push(section("atomintr-dashed", "Dashed line", cells));
        }
        let caps = [("flat", "Flat"), ("sphere", "Round"), ("arrow", "Arrow")];
        let mut tube = vec![
            self.detail(&["detail"], "Detail", 2.),
            self.mapped("captype_start", "Start cap", &caps),
            self.mapped("captype_end", "End cap", &caps),
            number("arrowheight", "Arrow height", 0., 5., 0.1, "Å"),
            number("arrowwidth", "Arrow width", 0., 5., 0.1, ""),
        ];
        for field in &mut tube {
            field["disabled"] = json!(
                simple
                    || field["key"]
                        .as_str()
                        .is_some_and(|key| key.starts_with("arrow"))
                        && !arrow
            );
        }
        sections.push(section("atomintr-tube", "3D tube", tube));
        let mut font = self.font();
        font.push(row("label_on_top", "Label on top"));
        for field in &mut font {
            field["disabled"] = json!(self.value("showlabel") == "false");
        }
        sections.push(section("atomintr-label", "Value label", font));
        sections
    }

    pub(super) fn name_label(&self) -> Vec<Value> {
        let mut rows = vec![
            json!({"key":"format","label":"Format","placeholder":"{chain} {resn}{resi} {name}"}),
            row("color", "Color"),
            row("label_on_top", "Label on top"),
            number("maxlabel", "Max labels", 1., 1000., 1., ""),
            self.mapped(
                "halign",
                "Horizontal align",
                &[("left", "Left"), ("center", "Center"), ("right", "Right")],
            ),
            self.mapped(
                "valign",
                "Vertical align",
                &[("top", "Top"), ("middle", "Middle"), ("bottom", "Bottom")],
            ),
        ];
        for (key, label) in [("dispx", "Offset X"), ("dispy", "Offset Y")] {
            let mut field = number(key, label, -100., 100., 1., "px");
            field["decimals"] = json!(0);
            rows.push(field);
        }
        vec![
            section("namelabel-main", "Name label", rows),
            section("namelabel-font", "Font", self.font()),
        ]
    }
}
