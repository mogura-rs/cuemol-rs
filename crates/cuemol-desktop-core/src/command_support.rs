use super::*;
#[derive(Debug, Clone)]
pub enum GuiPastedNode {
    Camera(String),
    Style { index: usize, name: String },
    Object(Uid),
    Renderers { uids: Vec<Uid>, group: Option<Uid> },
}

impl DesktopAppState {
    pub fn set_density_map_sampling(&mut self, id: Uid, stride: usize, kind: &str) -> Result<()> {
        if !matches!(kind, "auto" | "xtal" | "em") {
            return Err(CueMolError::invalid_argument("invalid map type"));
        }
        let map = self
            .density_maps
            .get_mut(&id)
            .ok_or_else(|| CueMolError::not_found("density map"))?;
        if stride > 1 {
            map.map.subsample(stride)?;
            map.modified = true;
        }
        map.map.set_map_type(kind)?;
        map.modified = true;
        Ok(())
    }

    pub fn density_map_level_unit(&self, id: Uid) -> Result<&str> {
        let map = self
            .density_maps
            .get(&id)
            .ok_or_else(|| CueMolError::not_found("map"))?;
        Ok(if map.map.is_em() { "percent" } else { "sigma" })
    }

    pub fn paste_command_xml(
        &mut self,
        kind: &str,
        form: &str,
        xml: &str,
        target: Option<Uid>,
        target_type: Option<&str>,
    ) -> Result<GuiPastedNode> {
        let sid = self.ensure_active_scene();
        if kind == "camera" {
            let camera = Camera::from_xml_string(xml)?;
            let name = self.paste_workspace_camera(sid, &WorkspaceCameraClipboard { camera })?;
            return Ok(GuiPastedNode::Camera(name));
        }
        let tree = parse_ldom2_xml(xml)?;
        if kind == "style" {
            if tree.top().tag_name() != "styles" {
                return Err(CueMolError::parse("expected styles XML"));
            }
            let mut node = tree.top().clone();
            let original = node.str_attr("id").to_string();
            let mut name = original.clone();
            for n in 1.. {
                if !self.workspace_style_exists(sid, &name) {
                    break;
                }
                name = format!("copy{n}_{original}");
            }
            node.set_str_attr("id", &name);
            let data = write_ldom2_xml(&LDom2Tree::from(node.clone()));
            let index = self.paste_workspace_style(
                sid,
                &WorkspaceStyleClipboard {
                    style: LoadedStyle { node, data },
                },
                false,
            )?;
            return Ok(GuiPastedNode::Style { index, name });
        }
        if kind == "object" {
            if tree.top().tag_name() != "object" {
                return Err(CueMolError::parse("expected object XML"));
            }
            let mut root = LDom2Node::new();
            root.set_tag_name("scene");
            root.append_child_node(tree.top().clone());
            let wrapped = write_ldom2_xml(&LDom2Tree::from(root));
            let mut temporary = DesktopAppState::new();
            temporary.open_bytes("clipboard.qsc", wrapped.as_bytes())?;
            let tmp_scene = temporary
                .active_scene()
                .ok_or_else(|| CueMolError::not_found("clipboard scene"))?;
            let object = temporary
                .object_summaries(tmp_scene)
                .first()
                .map(|o| o.uid)
                .or_else(|| {
                    temporary
                        .density_map_summaries(tmp_scene)
                        .first()
                        .map(|o| o.uid)
                })
                .or_else(|| {
                    temporary
                        .surface_summaries(tmp_scene)
                        .first()
                        .map(|o| o.uid)
                })
                .or_else(|| {
                    temporary
                        .lw_object_summaries(tmp_scene)
                        .first()
                        .map(|o| o.uid)
                })
                .ok_or_else(|| CueMolError::not_found("clipboard object"))?;
            let clipboard = temporary.copy_workspace_object(object)?;
            let id = self.paste_workspace_object(sid, &clipboard)?;
            return Ok(GuiPastedNode::Object(id));
        }
        if kind != "renderer" {
            return Err(CueMolError::invalid_argument("unknown clipboard kind"));
        }
        let target =
            target.ok_or_else(|| CueMolError::invalid_argument("renderer target is required"))?;
        let (object, group) = if target_type == Some("rendGroup") {
            let object = self
                .object_summaries(sid)
                .iter()
                .find(|o| {
                    self.renderer_summaries(o.uid)
                        .iter()
                        .any(|r| r.uid == target)
                })
                .map(|o| o.uid)
                .or_else(|| {
                    self.density_map_summaries(sid)
                        .iter()
                        .find(|o| o.renderer_groups.iter().any(|r| r.uid == target))
                        .map(|o| o.uid)
                })
                .or_else(|| {
                    self.surface_summaries(sid)
                        .iter()
                        .find(|o| o.renderer_groups.iter().any(|r| r.uid == target))
                        .map(|o| o.uid)
                })
                .or_else(|| {
                    self.lw_object_summaries(sid)
                        .iter()
                        .find(|o| o.renderer_groups.iter().any(|r| r.uid == target))
                        .map(|o| o.uid)
                })
                .ok_or_else(|| CueMolError::not_found("renderer group"))?;
            (object, Some(target))
        } else {
            (target, None)
        };
        let mut bytes = cuemol_core::ByteArray::new();
        bytes.init(cuemol_core::ElementType::Uint8, xml.len());
        bytes.as_mut_bytes().copy_from_slice(xml.as_bytes());
        let (group_name, renderers) = if form == "rendArray" {
            self.stream_manager()
                .renderer_array_from_xml(&bytes, sid.get() as i32)?
        } else {
            (
                String::new(),
                vec![self
                    .stream_manager()
                    .renderer_from_xml(&bytes, sid.get() as i32)?],
            )
        };
        let mut ids = Vec::new();
        let mut target_group = group;
        if !group_name.is_empty() {
            target_group = Some(self.create_renderer_group(object, &group_name)?);
        }
        for source in renderers {
            let renderer_type = source.type_name();
            let label = self.suggest_renderer_name(sid, source.name())?;
            let id = if self.scene_manager().object(object).is_some() {
                self.new_renderer(sid, object, renderer_type, &label)?
                    .result_renderer
            } else if self.density_maps.contains_key(&object) {
                let id = self.add_density_renderer(object, renderer_type)?;
                self.rename_density_renderer(object, id, &label)?;
                id
            } else if self.surface_summary(object).is_some() {
                let id = self.add_surface_renderer(object)?;
                self.rename_surface_renderer(object, id, &label)?;
                id
            } else {
                let id = self.add_lw_renderer(object)?;
                self.rename_lw_renderer(object, id, &label)?;
                id
            };
            if let Some(obj) = self.scene_manager_mut().object_mut(object) {
                let r = obj.renderer_mut(id).unwrap();
                for (k, v) in source.properties() {
                    r.set_qsc_attr(k, v);
                }
                r.set_visible(source.is_visible());
                r.set_qsc_attr("style", source.style_names());
            }
            if let Some(group) = target_group {
                self.move_renderer_in_object(object, id, group, "into")?;
            }
            ids.push(id);
        }
        Ok(GuiPastedNode::Renderers {
            uids: ids,
            group: target_group,
        })
    }
}
