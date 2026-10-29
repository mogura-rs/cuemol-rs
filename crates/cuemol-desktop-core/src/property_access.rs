use super::*;

const RENDERER_DEFAULTS: &[(&str, &str)] = &[
    ("name", ""),
    ("visible", "true"),
    ("locked", "false"),
    ("material", ""),
    ("alpha", "1"),
    ("egtype", "none"),
    ("eglinew", "0.01"),
    ("egcolor", "#000000"),
    ("egroup", ""),
];

fn node_attributes(node: &LDom2Node) -> BTreeMap<String, String> {
    node.children()
        .iter()
        .filter(|child| child.is_attr())
        .map(|child| (child.tag_name().to_string(), child.value().to_string()))
        .collect()
}

fn finite_number(key: &str, value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| CueMolError::invalid_argument(format!("{key} requires a finite number")))
}

pub(crate) fn property_matrix(value: &str) -> Result<Matrix4> {
    let matrix = Matrix4::from_script_string(value)?;
    if !matrix
        .values
        .iter()
        .flatten()
        .all(|value| value.is_finite())
    {
        return Err(CueMolError::invalid_argument(
            "matrix entries must be finite",
        ));
    }
    Ok(matrix)
}

impl DesktopAppState {
    pub fn workspace_object_transform(&self, uid: Uid) -> Result<Matrix4> {
        if let Some(object) = self.scene_manager().object(uid) {
            return Ok(object.xform_matrix());
        }
        if let Some(object) = self.density_maps.get(&uid) {
            return Ok(object.xform);
        }
        if let Some(scene) = self
            .scene_manager()
            .scene_uid_for_surface(uid)
            .and_then(|id| self.scene_manager().scene(id))
        {
            return Ok(scene
                .loaded_surface(uid)
                .and_then(|surface| surface.qsc_node.as_ref())
                .and_then(|node| {
                    Matrix4::from_script_string(node.str_attr("xformMat").as_str()).ok()
                })
                .unwrap_or(Matrix4::IDENTITY));
        }
        self.scene_manager()
            .scene_uid_for_lw_object(uid)
            .and_then(|id| self.scene_manager().scene(id))
            .and_then(|scene| scene.loaded_lw_object(uid))
            .map(|object| object.xform)
            .ok_or_else(|| CueMolError::not_found("object"))
    }

    pub fn set_workspace_object_transform(&mut self, uid: Uid, value: &str) -> Result<()> {
        let matrix = property_matrix(value)?;
        if let Some(object) = self.scene_manager_mut().object_mut(uid) {
            object.set_xform_matrix(matrix);
            object.set_modified(true);
        } else if let Some(object) = self.density_maps.get_mut(&uid) {
            object.xform = matrix;
            object.modified = true;
        } else if let Some(scene_uid) = self.scene_manager().scene_uid_for_surface(uid) {
            let object = self
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .loaded_surface_mut(uid)
                .unwrap();
            ensure_surface_qsc_node(object).set_str_attr("xformMat", matrix.to_script_string());
        } else if let Some(scene_uid) = self.scene_manager().scene_uid_for_lw_object(uid) {
            let object = self
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .loaded_lw_object_mut(uid)
                .unwrap();
            object.xform = matrix;
            ensure_lw_qsc_node(object).set_str_attr("xformMat", matrix.to_script_string());
        } else {
            return Err(CueMolError::not_found("object"));
        }
        Ok(())
    }

    fn auxiliary_renderer_node(&self, owner: Uid, uid: Uid) -> Result<(Uid, LDom2Node)> {
        if !self.density_maps.contains_key(&owner)
            && self.scene_manager().scene_uid_for_surface(owner).is_none()
            && self
                .scene_manager()
                .scene_uid_for_lw_object(owner)
                .is_none()
        {
            return Err(CueMolError::not_found("object"));
        }
        if let Some(object) = self.density_maps.get(&owner) {
            if let Some(renderer) = object.renderers.iter().find(|renderer| renderer.uid == uid) {
                return Ok((object.scene_uid, density_renderer_qsc_node(renderer)));
            }
            if let Some(node) = aux_renderer_group_index(owner, uid)
                .and_then(|index| object.renderer_groups.get(index))
            {
                return Ok((object.scene_uid, node.clone()));
            }
        }
        if let Some(scene_uid) = self.scene_manager().scene_uid_for_surface(owner) {
            let surface = self
                .scene_manager()
                .scene(scene_uid)
                .unwrap()
                .loaded_surface(owner)
                .unwrap();
            if let Some(node) = surface.qsc_node.as_ref().and_then(|node| {
                surface_renderer_nodes(node)
                    .enumerate()
                    .find_map(|(index, node)| {
                        (surface_renderer_uid(scene_uid, owner, index) == uid).then(|| node.clone())
                    })
                    .or_else(|| {
                        aux_renderer_group_index(owner, uid)
                            .and_then(|index| aux_renderer_group_nodes(node).nth(index).cloned())
                    })
            }) {
                return Ok((scene_uid, node));
            }
            if uid == surface_renderer_uid(scene_uid, owner, 0)
                && surface
                    .qsc_node
                    .as_ref()
                    .is_none_or(|node| surface_renderer_nodes(node).next().is_none())
            {
                return Ok((scene_uid, default_surface_renderer_node(1)));
            }
        }
        if let Some(scene_uid) = self.scene_manager().scene_uid_for_lw_object(owner) {
            let object = self
                .scene_manager()
                .scene(scene_uid)
                .unwrap()
                .loaded_lw_object(owner)
                .unwrap();
            if let Some(index) = lw_renderer_index(scene_uid, owner, uid)
                .filter(|index| *index < object.renderers.len())
            {
                let node = object
                    .qsc_node
                    .as_ref()
                    .and_then(|node| {
                        node.children()
                            .iter()
                            .filter(|node| {
                                !node.is_attr()
                                    && node.tag_name() == "renderer"
                                    && node.type_name() == "lwrend"
                            })
                            .nth(index)
                    })
                    .cloned()
                    .unwrap_or_else(|| lw_renderer_qsc_node(&object.renderers[index]));
                return Ok((scene_uid, node));
            }
            if let Some(node) = object.qsc_node.as_ref().and_then(|node| {
                aux_renderer_group_index(owner, uid)
                    .and_then(|index| aux_renderer_group_nodes(node).nth(index).cloned())
            }) {
                return Ok((scene_uid, node));
            }
        }
        Err(CueMolError::not_found("renderer"))
    }

    pub fn auxiliary_renderer_properties(
        &self,
        owner: Uid,
        uid: Uid,
    ) -> Result<BTreeMap<String, String>> {
        let (scene_uid, node) = self.auxiliary_renderer_node(owner, uid)?;
        let styles = scene_style_manager(self.scene_manager().scene(scene_uid).unwrap());
        let style_list = node.str_attr("style").to_string();
        let mut values = node_attributes(&node);
        if let Some(object) = self.density_maps.get(&owner) {
            if let Some(renderer) = object.renderers.iter().find(|renderer| renderer.uid == uid) {
                values.extend(
                    density_renderer_summary(owner, &object.map, renderer, 1, &styles).specific,
                );
            }
        } else if let Some(surface) = self.surface_summary(owner) {
            if let Some(renderer) = surface
                .renderers
                .iter()
                .find(|renderer| renderer.uid == uid)
            {
                values.extend(renderer.specific.clone());
                values.insert("sel".into(), renderer.selection.clone());
            }
        } else if node.type_name() == "lwrend" {
            values.entry("data_id".into()).or_insert_with(|| "0".into());
        }
        for (key, default) in RENDERER_DEFAULTS {
            values.insert(
                (*key).into(),
                node.find_child(key)
                    .filter(|child| child.is_attr())
                    .map(|child| child.value().to_string())
                    .or_else(|| styles.resolve_style_attr(DEFAULT_STYLE_SCOPE, &style_list, key))
                    .unwrap_or_else(|| (*default).into()),
            );
        }
        values
            .entry("xformMat".into())
            .or_insert_with(|| Matrix4::IDENTITY.to_script_string());
        values.insert("style".into(), style_list);
        values.entry("group".into()).or_default();
        values
            .entry("ui_order".into())
            .or_insert_with(|| "0".into());
        values.remove("type");
        Ok(values)
    }

    fn replace_auxiliary_renderer_node(
        &mut self,
        owner: Uid,
        uid: Uid,
        scene_uid: Uid,
        node: LDom2Node,
    ) -> Result<()> {
        if matches!(node.type_name(), "*group" | "RendGroup") {
            let (_, old) = self.auxiliary_renderer_node(owner, uid)?;
            if old.str_attr("name") != node.str_attr("name") {
                self.rename_renderer_group(owner, uid, node.str_attr("name").to_string())?;
            }
            let visible = |node: &LDom2Node| {
                node.find_child("visible")
                    .is_none_or(|child| qsc_bool(child.value()))
            };
            if visible(&old) != visible(&node) {
                self.set_renderer_group_visible(owner, uid, visible(&node))?;
            }
        }
        let styles = scene_style_manager(self.scene_manager().scene(scene_uid).unwrap());
        if let Some(object) = self.density_maps.get_mut(&owner) {
            if let Some(renderer) = object
                .renderers
                .iter_mut()
                .find(|renderer| renderer.uid == uid)
            {
                let mut root = LDom2Node::new();
                root.append_child_node(node);
                let mut replacement = density_renderer_configs(Some(&root), &styles)
                    .pop()
                    .ok_or_else(|| CueMolError::invalid_argument("invalid map renderer"))?;
                replacement.uid = uid;
                *renderer = replacement;
            } else if let Some(group) = aux_renderer_group_index(owner, uid)
                .and_then(|index| object.renderer_groups.get_mut(index))
            {
                *group = node;
            } else {
                return Err(CueMolError::not_found("renderer"));
            }
            object.modified = true;
        } else if self.scene_manager().scene_uid_for_surface(owner).is_some() {
            let object = self
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .loaded_surface_mut(owner)
                .unwrap();
            if let Some(index) = aux_renderer_group_index(owner, uid) {
                let group = ensure_surface_qsc_node(object)
                    .children_mut()
                    .iter_mut()
                    .filter(|node| {
                        !node.is_attr()
                            && node.tag_name() == "renderer"
                            && matches!(node.type_name(), "*group" | "RendGroup")
                    })
                    .nth(index)
                    .ok_or_else(|| CueMolError::not_found("renderer group"))?;
                *group = node;
            } else {
                *ensure_surface_renderer_node(object, scene_uid, uid)? = node;
            }
        } else if aux_renderer_group_index(owner, uid).is_some() {
            let index = aux_renderer_group_index(owner, uid).unwrap();
            let object = self
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .loaded_lw_object_mut(owner)
                .unwrap();
            let group = ensure_lw_qsc_node(object)
                .children_mut()
                .iter_mut()
                .filter(|node| {
                    !node.is_attr()
                        && node.tag_name() == "renderer"
                        && matches!(node.type_name(), "*group" | "RendGroup")
                })
                .nth(index)
                .ok_or_else(|| CueMolError::not_found("renderer group"))?;
            *group = node;
        } else {
            self.update_lw_renderer(owner, uid, |renderer, stored| {
                renderer.name = node.str_attr("name").to_string();
                renderer.visible = node
                    .find_child("visible")
                    .is_none_or(|v| qsc_bool(v.value()));
                renderer.data_id = node
                    .str_attr("data_id")
                    .as_str()
                    .parse()
                    .unwrap_or_default();
                renderer.xform = Matrix4::from_script_string(node.str_attr("xformMat").as_str())
                    .unwrap_or_default();
                *stored = node;
            })?;
        }
        Ok(())
    }

    #[cfg(feature = "scripting")]
    pub(crate) fn set_auxiliary_renderer_children(
        &mut self,
        owner: Uid,
        uid: Uid,
        children: Vec<LDom2Node>,
    ) -> Result<()> {
        let (scene_uid, mut node) = self.auxiliary_renderer_node(owner, uid)?;
        node.retain_children(LDom2Node::is_attr);
        for child in children {
            node.append_child_node(child);
        }
        self.replace_auxiliary_renderer_node(owner, uid, scene_uid, node)
    }

    pub fn set_auxiliary_renderer_property(
        &mut self,
        owner: Uid,
        uid: Uid,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let (scene_uid, mut node) = self.auxiliary_renderer_node(owner, uid)?;
        let values = self.auxiliary_renderer_properties(owner, uid)?;
        if !values.contains_key(key)
            && !property_catalog::renderer_attr_properties(node.type_name())
                .iter()
                .any(|p| p.key == key)
        {
            return Err(CueMolError::not_found(format!("renderer property {key}")));
        }
        validate_auxiliary_property(node.type_name(), key, value)?;
        if key == "level" && self.density_maps.contains_key(&owner) {
            let level =
                self.density_native_level_from_absolute(owner, finite_number(key, value)?)?;
            node.set_str_attr("siglevel", level.to_string());
        } else {
            node.set_str_attr(key, value);
        }
        self.replace_auxiliary_renderer_node(owner, uid, scene_uid, node)
    }

    pub fn reset_auxiliary_renderer_property(
        &mut self,
        owner: Uid,
        uid: Uid,
        key: &str,
    ) -> Result<()> {
        let (scene_uid, mut node) = self.auxiliary_renderer_node(owner, uid)?;
        if key == "*" {
            node.retain_children(|child| {
                child.is_attr()
                    && matches!(child.tag_name(), "name" | "ui_order" | "style" | "group")
            });
        } else {
            let key = if key == "level" { "siglevel" } else { key };
            let values = self.auxiliary_renderer_properties(owner, uid)?;
            if !values.contains_key(key) {
                return Err(CueMolError::not_found(format!("renderer property {key}")));
            }
            if auxiliary_property_readonly(key) || matches!(key, "style" | "ui_order" | "group") {
                return Err(CueMolError::invalid_argument(format!(
                    "{key} has no default"
                )));
            }
            node.remove_child(key);
        }
        self.replace_auxiliary_renderer_node(owner, uid, scene_uid, node)
    }
}

pub fn auxiliary_property_readonly(key: &str) -> bool {
    matches!(
        key,
        "minLevel"
            | "maxLevel"
            | "maxExtent"
            | "ui_abslevel_min"
            | "ui_abslevel_max"
            | "ui_level_min"
            | "ui_level_max"
            | "siglevel_effective"
            | "map_type_resolved"
            | "region_mode_resolved"
    )
}

pub(crate) fn validate_auxiliary_property(
    renderer_type: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    if auxiliary_property_readonly(key) {
        return Err(CueMolError::invalid_argument(format!("{key} is read-only")));
    }
    let invalid = || CueMolError::invalid_argument(format!("invalid {key}: {value}"));
    match key {
        "visible" | "locked" | "use_pbc" | "zoom_refine" | "autoupdate" | "dragupdate"
        | "cullface" | "use_abslevel" | "ramp_above" | "ui_collapsed" => {
            value.parse::<bool>().map_err(|_| invalid())?;
        }
        "data_id" | "ui_order" | "binning" | "max_grids" | "bufsize" | "lod_budget" => {
            value.parse::<i32>().map_err(|_| invalid())?;
        }
        "alpha" if !(0.0..=1.0).contains(&finite_number(key, value)?) => return Err(invalid()),
        "eglinew" if finite_number(key, value)? < 0.0 => return Err(invalid()),
        "alpha" | "eglinew" | "siglevel" | "level" | "extent" | "width" | "bndry_rng"
        | "lowpar" | "midpar" | "highpar" | "ramp_value" => {
            finite_number(key, value)?;
        }
        "xformMat" => {
            property_matrix(value)?;
        }
        "center" => {
            let vector = Vector4::from_script_string(value)?;
            if ![vector.x, vector.y, vector.z]
                .iter()
                .all(|value| value.is_finite())
            {
                return Err(invalid());
            }
        }
        "egcolor" | "color" | "defaultcolor" | "lowcol" | "midcol" | "highcol" => {
            cuemol_gfx::parse_color(value)?;
        }
        "sel" | "showsel" | "bndry_sel" => {
            parse_selector(value)?;
        }
        "egtype" if !matches!(value, "none" | "edges" | "silhouette") => return Err(invalid()),
        "drawmode" if !matches!(value, "fill" | "line" | "point") => return Err(invalid()),
        "colormode" if !matches!(value, "solid" | "molecule" | "multigrad" | "potential") => {
            return Err(invalid())
        }
        "region_mode" if !matches!(value, "auto" | "box" | "full") => return Err(invalid()),
        "lod" if !matches!(value, "auto" | "step1" | "step2" | "step4" | "step8") => {
            return Err(invalid())
        }
        _ => {}
    }
    property_catalog::validate_renderer_attr(renderer_type, key, value)
}
