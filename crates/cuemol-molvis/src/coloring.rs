use super::*;
use cuemol_molstr::{MolObject, Selector};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct MolecularColoring {
    renderer: Layer,
    object: Layer,
}

#[derive(Debug, Clone, PartialEq)]
struct Layer {
    kind: String,
    fallback: ColorSpec,
    paint: Vec<(Selector, ColorSpec)>,
    cpk: BTreeMap<String, ColorSpec>,
    rainbow: Option<RainbowColoring>,
    bfac: Option<BfacColoring>,
    script: Option<ScriptColoringOptions>,
}

const COLORING_ATTRS: &[&str] = &[
    "start_hue",
    "end_hue",
    "sat",
    "bri",
    "mode",
    "incr_mode",
    "auto",
    "lowcol",
    "highcol",
    "lowpar",
    "highpar",
    "col_H",
    "col_C",
    "col_N",
    "col_O",
    "col_P",
    "col_S",
    "col_X",
];

fn color(value: &str, styles: &StyleManager, scope: u64) -> ColorSpec {
    let spec = cuemol_gfx::parse_color(value).unwrap_or_else(|_| rgba_to_spec([1.0; 4]));
    if matches!(spec.base, ColorBase::MolColor(_)) {
        spec
    } else {
        solid_color_to_spec(&styles.compile_color(value, scope))
    }
}

impl Layer {
    fn new(
        kind: &str,
        fallback: ColorSpec,
        attrs: &BTreeMap<String, String>,
        paint: &[(String, String)],
        styles: &StyleManager,
        scope: u64,
    ) -> Self {
        let aliases = style_selection_aliases(styles, scope);
        let paint = paint
            .iter()
            .filter_map(|(selection, value)| {
                Some((
                    cuemol_molstr::parse_named_selector(selection, &aliases).ok()?,
                    color(value, styles, scope),
                ))
            })
            .collect();
        let mut attrs = attrs.clone();
        for name in ["lowcol", "highcol"] {
            if let Some(value) = attrs.get_mut(name) {
                *value = styles.compile_color(value, scope).script_string();
            }
        }
        let attr = |name: &str| attrs.get(name).map(String::as_str);
        let cpk = [
            ("H", "#00ffff"),
            ("C", "rgb(1,1,0.75)"),
            ("N", "#0000ff"),
            ("O", "#ff0000"),
            ("S", "#00ff00"),
            ("P", "#ffff00"),
            ("X", "rgb(0.7,0.7,0.7)"),
        ]
        .into_iter()
        .map(|(element, default)| {
            (
                element.to_string(),
                color(
                    attr(&format!("col_{element}")).unwrap_or(default),
                    styles,
                    scope,
                ),
            )
        })
        .collect();
        Self {
            kind: kind.to_string(),
            fallback,
            paint,
            cpk,
            rainbow: rainbow_coloring_from_attrs(Some(kind), attr),
            bfac: bfac_coloring_from_attrs(Some(kind), attr),
            script: (kind == "ScriptColoring")
                .then(|| attr("script"))
                .flatten()
                .map(|script| ScriptColoringOptions {
                    script: script.to_string(),
                    renderer_name: None,
                    renderer_type: None,
                    all_atoms: true,
                }),
        }
    }

    fn colors(&self, molecule: &MolCoord) -> Vec<ColorSpec> {
        let rainbow = self
            .rainbow
            .as_ref()
            .map(|r| rainbow_atom_colors(molecule, r));
        let bfac = self.bfac.clone().map(|mut b| {
            b.start_molecule(molecule);
            b
        });
        let script = self
            .script
            .as_ref()
            .map(|s| evaluate_script_colors(molecule, s));
        molecule
            .atoms()
            .iter()
            .enumerate()
            .map(|(i, atom)| {
                let value = match self.kind.as_str() {
                    "CPKColoring" => self
                        .cpk
                        .get(&atom.element.trim().to_ascii_uppercase())
                        .or_else(|| self.cpk.get("X"))
                        .cloned(),
                    "PaintColoring" => self
                        .paint
                        .iter()
                        .find(|(s, _)| s.matches_in_molecule(atom, molecule))
                        .map(|(_, c)| c.clone()),
                    "RainbowColoring" => rainbow.as_ref().and_then(|c| c[i]).map(rgba_to_spec),
                    "BfacColoring" => bfac
                        .as_ref()
                        .map(|b| solid_color_to_spec(&b.color_for_atom(atom))),
                    "ScriptColoring" => script.as_ref().and_then(|c| c[i].clone()),
                    "AtomPropColoring" => Some(solid_color_to_spec(
                        &atom
                            .int_prop("col")
                            .map(|c| SolidColor::from_code(c as u32))
                            .unwrap_or_else(|| SolidColor::from_rgb(0.5, 0.5, 0.5)),
                    )),
                    _ => None,
                };
                value.unwrap_or_else(|| self.fallback.clone())
            })
            .collect()
    }
}

impl MolecularColoring {
    pub fn bind_to_scene(&mut self, object: &MolObject, scene: &cuemol_molstr::MolScene) {
        for layer in [&mut self.renderer, &mut self.object] {
            for (selector, _) in &mut layer.paint {
                if let std::borrow::Cow::Owned(bound) = selector.bind_to_scene(object, scene) {
                    *selector = bound;
                }
            }
        }
    }

    pub(super) fn resolve(&self, molecule: &MolCoord) -> Vec<ColorSpec> {
        let mut colors = self.renderer.colors(molecule);
        if colors
            .iter()
            .any(|c| matches!(c.base, ColorBase::MolColor(_)))
        {
            let object = self.object.colors(molecule);
            for (value, parent) in colors.iter_mut().zip(object) {
                if matches!(value.base, ColorBase::MolColor(_)) {
                    let parent = parent
                        .resolve(|_| None, None)
                        .unwrap_or_else(|| molecule.default_color().clone());
                    *value = solid_color_to_spec(&value.resolve(|_| None, Some(&parent)).unwrap());
                }
            }
        }
        colors
    }
}

pub fn apply_molecular_object_coloring(
    options: &mut MoleculeRenderOptions,
    object: &MolObject,
    styles: &StyleManager,
    scope: u64,
) {
    let layer = Layer::new(
        "SolidColoring",
        color("$molcol", styles, scope),
        &BTreeMap::new(),
        &[],
        styles,
        scope,
    );
    set_coloring_layers(options, object, layer, styles, scope);
}

/// Resolve each renderer color through the object's scheme only for `$molcol`.
pub fn apply_molecular_coloring(
    options: &mut MoleculeRenderOptions,
    object: &MolObject,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope: u64,
) {
    let node = renderer
        .qsc_children()
        .iter()
        .find(|n| n.tag_name() == "coloring")
        .or_else(|| {
            renderer
                .style()
                .and_then(|s| styles.resolve_style_child_node(scope, s, "coloring"))
        });
    let mut attrs = COLORING_ATTRS
        .iter()
        .filter_map(|&name| {
            renderer
                .resolved_child_attr("coloring", name, styles, scope)
                .map(|value| (name.to_string(), value))
        })
        .collect::<BTreeMap<_, _>>();
    if let Some(script) = node.and_then(script_source_from_coloring_node) {
        attrs.insert("script".to_string(), script);
    }
    let paint = node
        .into_iter()
        .flat_map(|n| n.children())
        .filter(|n| !n.is_attr() && n.tag_name() == "paint")
        .map(|n| {
            (
                n.str_attr("sel").to_string(),
                n.str_attr("color").to_string(),
            )
        })
        .collect::<Vec<_>>();
    let kind = renderer
        .resolved_child_type("coloring", styles, scope)
        .unwrap_or_else(|| "SolidColoring".to_string());
    let fallback = color(
        &renderer
            .resolved_attr("defaultcolor", styles, scope)
            .unwrap_or_else(|| "#ffffff".to_string()),
        styles,
        scope,
    );
    let mut layer = Layer::new(&kind, fallback, &attrs, &paint, styles, scope);
    if let Some(script) = &mut layer.script {
        script.renderer_name = Some(renderer.name().to_string());
        script.renderer_type = Some(renderer.renderer_type().to_string());
    }
    set_coloring_layers(options, object, layer, styles, scope);
}

/// Resolve a serialized renderer's coloring, including scene styles and `$molcol`.
pub fn apply_molecular_node_coloring(
    options: &mut MoleculeRenderOptions,
    object: &MolObject,
    renderer: Option<&cuemol_core::LDom2Node>,
    styles: &StyleManager,
    scope: u64,
    default_color: &str,
) {
    let style_list = renderer
        .map(|r| r.str_attr("style").to_string())
        .unwrap_or_default();
    let inline = renderer.and_then(|r| r.children().iter().find(|n| n.tag_name() == "coloring"));
    let node = inline.or_else(|| styles.resolve_style_child_node(scope, &style_list, "coloring"));
    let mut attrs = COLORING_ATTRS
        .iter()
        .filter_map(|&name| {
            inline
                .map(|n| n.str_attr(name).to_string())
                .filter(|v| !v.is_empty())
                .or_else(|| styles.resolve_style_child_attr(scope, &style_list, "coloring", name))
                .map(|v| (name.to_string(), v))
        })
        .collect::<BTreeMap<_, _>>();
    if let Some(script) = node.and_then(script_source_from_coloring_node) {
        attrs.insert("script".to_string(), script);
    }
    let paint = node
        .into_iter()
        .flat_map(|n| n.children())
        .filter(|n| !n.is_attr() && n.tag_name() == "paint")
        .map(|n| {
            (
                n.str_attr("sel").to_string(),
                n.str_attr("color").to_string(),
            )
        })
        .collect::<Vec<_>>();
    let kind = inline
        .map(|n| {
            if n.type_name().is_empty() {
                n.str_attr("type").to_string()
            } else {
                n.type_name().to_string()
            }
        })
        .filter(|v| !v.is_empty())
        .or_else(|| styles.resolve_style_child_type(scope, &style_list, "coloring"))
        .unwrap_or_else(|| "SolidColoring".to_string());
    let fallback = renderer
        .map(|r| r.str_attr("defaultcolor").to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| styles.resolve_style_attr(scope, &style_list, "defaultcolor"))
        .unwrap_or_else(|| default_color.to_string());
    let mut layer = Layer::new(
        &kind,
        color(&fallback, styles, scope),
        &attrs,
        &paint,
        styles,
        scope,
    );
    if let (Some(script), Some(renderer)) = (&mut layer.script, renderer) {
        script.renderer_name = Some(renderer.str_attr("name").to_string());
        script.renderer_type = Some(renderer.type_name().to_string());
    }
    set_coloring_layers(options, object, layer, styles, scope);
}

fn set_coloring_layers(
    options: &mut MoleculeRenderOptions,
    object: &MolObject,
    layer: Layer,
    styles: &StyleManager,
    scope: u64,
) {
    let object_kind = object
        .coloring_type()
        .unwrap_or(if object.paint_coloring().is_empty() {
            "SolidColoring"
        } else {
            "PaintColoring"
        });
    options.molecular_coloring = Some(MolecularColoring {
        renderer: layer,
        object: Layer::new(
            object_kind,
            object
                .qsc_extra_attrs()
                .get("defaultcolor")
                .map(|value| color(value, styles, scope))
                .unwrap_or_else(|| solid_color_to_spec(object.mol().default_color())),
            object.coloring_attrs(),
            object.paint_coloring(),
            styles,
            scope,
        ),
    });
    options.backbone_atom_colors = true;
}
