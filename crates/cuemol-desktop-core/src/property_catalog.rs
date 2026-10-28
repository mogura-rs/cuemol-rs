#[derive(Clone, Copy)]
pub enum RendererAttrKind {
    Boolean,
    Integer,
    Real,
    String,
    Color,
    Selection,
    Vector,
}
#[derive(Clone, Copy)]
pub struct RendererAttrProperty {
    pub name: &'static str,
    pub key: &'static str,
    pub kind: RendererAttrKind,
    pub default: &'static str,
    pub writable: bool,
}

pub fn validate_renderer_attr(
    renderer_type: &str,
    name: &str,
    value: &str,
) -> cuemol_core::Result<()> {
    if matches!(renderer_type, "*namelabel" | "name_label") {
        match name {
            "format" => {
                cuemol_molstr::label_format::LabelFormat::new(value)?;
            }
            "halign" if !matches!(value, "left" | "center" | "right") => {
                return Err(cuemol_core::CueMolError::invalid_argument(
                    "halign must be left, center or right",
                ));
            }
            "valign" if !matches!(value, "bottom" | "middle" | "top") => {
                return Err(cuemol_core::CueMolError::invalid_argument(
                    "valign must be bottom, middle or top",
                ));
            }
            _ => {}
        }
    }
    if matches!(renderer_type, "isosurf" | "MapSurfRenderer")
        && name == "cap_mode"
        && !matches!(value, "auto" | "on" | "off")
    {
        return Err(cuemol_core::CueMolError::invalid_argument(
            "cap_mode must be auto, on or off",
        ));
    }
    Ok(())
}
pub fn renderer_attr_properties(renderer_type: &str) -> Vec<RendererAttrProperty> {
    use RendererAttrKind::{Boolean, Color, Integer, Real, Selection, String as Text, Vector};

    let mut properties = Vec::new();
    let mut add = |name, kind, default| {
        properties.push(RendererAttrProperty {
            name,
            key: name,
            kind,
            default,
            writable: true,
        });
    };
    add("egroup", Text, "");
    if matches!(
        renderer_type,
        "simple"
            | "ballstick"
            | "cpk"
            | "trace"
            | "tracestick"
            | "spline"
            | "tube"
            | "ribbon"
            | "cartoon"
            | "nucl"
            | "anisou"
            | "disorder"
            | "*selection"
            | "selection"
            | "dsurface"
            | "directsurf"
            | "dsurf2"
            | "ms2test"
    ) {
        add("sel", Selection, "");
        add("defaultcolor", Color, "#ffffff");
    }
    match renderer_type {
        "isosurf" | "MapSurfRenderer" => {
            add("cap_mode", Text, "auto");
            add("target", Text, "");
            add("sel", Selection, "");
        }
        "test" => add("color1", Color, "#00ffff"),
        "simple" => {
            add("width", Real, "1.2");
            add("valbond", Boolean, "true");
            add("vb_scl1", Real, "0.1");
            add("vb_scl2", Real, "0");
        }
        "trace" => {
            add("pivotatom", Text, "");
            add("width", Real, "1.2");
        }
        "tracestick" => {
            add("pivotatom", Text, "");
            add("bondw", Real, "0.25");
            add("sphr", Real, "0.25");
            add("detail", Integer, "3");
        }
        "*namelabel" | "name_label" => {
            add("maxlabel", Integer, "50");
            add("format", Text, "");
            add("halign", Text, "left");
            add("valign", Text, "bottom");
            add("label_on_top", Boolean, "false");
            add("dispx", Real, "0");
            add("dispy", Real, "0");
            add("color", Color, "#ffff00");
            add("font_size", Real, "12");
            add("font_name", Text, "sans-serif");
            add("font_style", Text, "normal");
            add("font_weight", Text, "normal");
        }
        "*selection" | "selection" => {
            add("linew", Real, "3");
            add("dispx", Real, "3");
            add("dispy", Real, "3");
            add("color", Color, "rgba(0,255,255,0.5)");
            add("mode", Text, "stick");
            if let Some(selection) = properties
                .iter_mut()
                .find(|property| property.name == "sel")
            {
                selection.writable = false;
            }
        }
        "ballstick" | "anisou" => {
            add("bondw", Real, "0.2");
            add("sphr", Real, "0.3");
            add("detail", Integer, "3");
            add("ring", Boolean, "false");
            add("thickness", Real, "0.2");
            add("ringcolor", Color, "#ffffbf");
            add("valbond_mode", Text, "off");
            add("glrender_mode", Text, "default");
            if renderer_type == "anisou" {
                add("drawdisc", Boolean, "true");
                add("discscale", Real, "1.1");
                add("discthick", Real, "0.1");
                add("maxverts", Integer, "1000000");
            }
        }
        "cpk" => {
            for (name, default) in [
                ("vdwr_H", "1.2"),
                ("vdwr_C", "1.7"),
                ("vdwr_N", "1.55"),
                ("vdwr_O", "1.52"),
                ("vdwr_S", "1.8"),
                ("vdwr_P", "1.8"),
                ("vdwr_X", "1.7"),
            ] {
                add(name, Real, default);
            }
            add("detail", Integer, "10");
            add("glrender_mode", Text, "default");
        }
        "spline" | "tube" | "ribbon" | "nucl" => {
            add("pivotatom", Text, "");
            add("axialdetail", Integer, "6");
            add("smooth", Real, "0");
            add("smoothcolor", Boolean, "true");
            add("line_width", Real, "1.2");
            add("start_captype", Text, "sphere");
            add("end_captype", Text, "sphere");
            add("segend_fade", Boolean, "false");
            if matches!(renderer_type, "tube" | "nucl") {
                add("putty_mode", Text, "none");
                add("putty_tgt", Text, "bfac");
                add("putty_hiscl", Real, "3");
                add("putty_loscl", Real, "3");
            }
            if renderer_type == "ribbon" {
                add("helix_smooth", Real, "0");
                add("sheet_smooth", Real, "0.5");
                add("coil_smooth", Real, "0");
                add("helix_usebackcol", Boolean, "false");
                add("helix_backcol", Color, "#ffffff");
                add("sheet_usesidecol", Boolean, "false");
                add("sheet_sidecol", Color, "#ffffff");
            }
            if renderer_type == "nucl" {
                add("show_tube", Boolean, "true");
                add("show_basepair", Boolean, "true");
                add("base_size", Real, "0.5");
                add("base_detail", Integer, "3");
                add("base_thick", Real, "0.5");
                add("base_type", Text, "basepair");
            }
        }
        "cartoon" => {
            add("pivotatom", Text, "");
            add("axialdetail", Integer, "8");
            add("smoothcolor", Boolean, "false");
            add("anchor_sel", Selection, "");
            add("anchor_weight", Real, "10");
            add("helix_ribbon", Boolean, "false");
            add("helix_smooth", Real, "3");
            add("helix_width_mode", Text, "average");
            add("helix_width", Real, "2.3");
            add("helix_extend", Real, "0.5");
            add("helix_wsmooth", Real, "5");
            add("helix_wplus", Real, "0.2");
            add("helix_waver", Boolean, "false");
            add("sheet_smooth", Real, "2");
            add("sheet_wsmooth", Real, "5");
            add("coil_smooth", Real, "-1");
            add("start_captype", Text, "flat");
            add("end_captype", Text, "flat");
            add("segend_fade", Boolean, "false");
            add("dump_curvature", Boolean, "false");
        }
        "atomintr" => {
            add("mode", Text, "fancy");
            add("showlabel", Boolean, "false");
            add("label_on_top", Boolean, "false");
            add("detail", Integer, "5");
            for (name, default) in [
                ("stipple0", "1"),
                ("stipple1", "1"),
                ("stipple2", "-1"),
                ("stipple3", "-1"),
                ("stipple4", "-1"),
                ("stipple5", "-1"),
            ] {
                add(name, Real, default);
            }
            add("width", Real, "0.1");
            add("color", Color, "#ffff00");
            add("captype_start", Text, "sphere");
            add("captype_end", Text, "sphere");
            add("arrowheight", Real, "1");
            add("arrowwidth", Real, "2");
            add("font_size", Real, "12");
            add("font_name", Text, "sans-serif");
            add("font_style", Text, "normal");
            add("font_weight", Text, "normal");
        }
        "disorder" => {
            add("detail", Integer, "5");
            add("width", Real, "0.3");
            add("dotsep", Real, "1");
            add("loopsize", Real, "2");
            add("loopsize2", Real, "-1");
            add("target", Text, "");
        }
        "dsurface" | "directsurf" | "dsurf2" => {
            add("colormode", Text, "molecule");
            add("target", Text, "");
            add("detail", Integer, "6");
            add("proberad", Real, "1.4");
            add("surftype", Text, "ses");
            if renderer_type != "dsurf2" {
                add("surfalgor", Text, "distfield");
            }
            add("cullface", Boolean, "false");
            add("drawmode", Text, "fill");
            add("width", Real, "1.2");
            add("showsel", Selection, "");
            for (name, default) in [
                ("vdwr_H", "1.2"),
                ("vdwr_C", "1.7"),
                ("vdwr_N", "1.55"),
                ("vdwr_O", "1.52"),
                ("vdwr_S", "1.8"),
                ("vdwr_P", "1.8"),
                ("vdwr_X", "1.7"),
            ] {
                add(name, Real, default);
            }
            add("elepot", Text, "");
            add("color_mapname", Text, "");
            add("lowpar", Real, "-10");
            add("midpar", Real, "0");
            add("highpar", Real, "10");
            add("highcol", Color, "#0000ff");
            add("midcol", Color, "#ffffff");
            add("lowcol", Color, "#ff0000");
            add("ramp_above", Boolean, "false");
            add("ramp_value", Real, "1.4");
        }
        "molsurf" => {
            add("defaultcolor", Color, "#c0c0ff");
            add("cullface", Boolean, "false");
            add("drawmode", Text, "fill");
            add("colormode", Text, "solid");
            add("target", Text, "");
            add("width", Real, "1.2");
            add("elepot", Text, "");
            add("lowpar", Real, "-10");
            add("midpar", Real, "0");
            add("highpar", Real, "10");
            add("highcol", Color, "#0000ff");
            add("midcol", Color, "#ffffff");
            add("lowcol", Color, "#ff0000");
            add("color_mapname", Text, "");
            add("ramp_above", Boolean, "false");
            add("ramp_value", Real, "1.4");
            add("sel", Selection, "");
            add("showsel", Selection, "");
        }
        "symm" => {
            add("maxop", Integer, "1000");
            add("extent", Real, "50");
            add("unitcell", Boolean, "true");
            add("autoupdate", Boolean, "false");
            add("center", Vector, "(0,0,0,0)");
        }
        "unitcell" => {
            add("width", Real, "1");
            add("color", Color, "#ffff00");
            add("labelcolor", Color, "#ff0000");
        }
        "lwrend" => add("data_id", Integer, "0"),
        "*group" => add("ui_collapsed", Boolean, "false"),
        _ => {}
    }
    properties
}

impl crate::DesktopAppState {
    pub fn parse_scene_selector(
        &self,
        scene_uid: cuemol_core::Uid,
        selection: &str,
    ) -> cuemol_core::Result<cuemol_molstr::Selector> {
        let scene = self
            .scene_manager()
            .scene(scene_uid)
            .ok_or_else(|| cuemol_core::CueMolError::not_found("scene"))?;
        let styles = crate::scene_style_manager(scene);
        crate::parse_selector_with_styles(
            if selection.trim().is_empty() {
                "*"
            } else {
                selection
            },
            &styles,
            scene_uid.get(),
        )
    }

    pub fn molecule_renderer_properties(
        &self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) -> cuemol_core::Result<std::collections::BTreeMap<String, String>> {
        let scene_uid = self
            .scene_manager()
            .scene_uid_for_object(object_uid)
            .ok_or_else(|| cuemol_core::CueMolError::not_found("molecule"))?;
        let scene = self.scene_manager().scene(scene_uid).unwrap();
        let styles = crate::scene_style_manager(scene);
        let renderer = self
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .ok_or_else(|| cuemol_core::CueMolError::not_found("renderer"))?;
        let mut values = renderer_attr_properties(renderer.renderer_type())
            .into_iter()
            .map(|p| {
                (
                    p.key.to_string(),
                    renderer
                        .resolved_attr(p.key, &styles, scene_uid.get())
                        .unwrap_or_else(|| p.default.to_string()),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        values.extend(renderer.qsc_attrs().clone());
        Ok(values)
    }

    pub fn workspace_object_render_snapshot(
        &self,
        object_uid: cuemol_core::Uid,
    ) -> cuemol_core::Result<cuemol_render::RenderSceneSnapshot> {
        let sid = self
            .active_scene()
            .ok_or_else(|| cuemol_core::CueMolError::not_found("scene"))?;
        let scene = self.scene_manager().scene(sid).unwrap();
        let styles = crate::scene_style_manager(scene);
        let mut snapshot = cuemol_render::RenderSceneSnapshot::empty();
        if let Some(object) = scene.object(object_uid) {
            crate::append_object_snapshot(
                &mut snapshot,
                object,
                None,
                &styles,
                sid.get(),
                self.surface_potential.as_ref(),
                &self.density_maps,
                scene,
            );
        } else if let Some(surface) = scene.loaded_surface(object_uid) {
            snapshot.meshes =
                self.loaded_surface_meshes(scene, std::slice::from_ref(surface), true);
        } else if let Some(object) = scene
            .loaded_lw_objects()
            .iter()
            .find(|o| o.uid == object_uid)
        {
            crate::append_lw_object_snapshot(&mut snapshot, object, &styles)?;
        } else {
            return Err(cuemol_core::CueMolError::not_found("object geometry"));
        }
        snapshot.recompute_bounds();
        Ok(snapshot)
    }
}
