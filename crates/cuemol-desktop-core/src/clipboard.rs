use super::*;

#[derive(Debug, Clone)]
pub struct GuiClipboardPayload {
    pub kind: &'static str,
    pub form: &'static str,
    pub bytes: Vec<u8>,
}

impl WorkspaceCameraClipboard {
    pub fn to_payload(&self) -> Result<GuiClipboardPayload> {
        Ok(GuiClipboardPayload {
            kind: "camera",
            form: "single",
            bytes: self
                .xml
                .clone()
                .unwrap_or_else(|| self.camera.to_xml_string())
                .into_bytes(),
        })
    }
}

impl WorkspaceStyleClipboard {
    pub fn to_payload(&self) -> Result<GuiClipboardPayload> {
        Ok(xml_payload("style", "single", self.style.node.clone()))
    }
}

impl WorkspaceObjectClipboard {
    pub fn to_payload(&self) -> Result<GuiClipboardPayload> {
        let mut temporary = DesktopAppState::new();
        let scene = temporary.ensure_active_scene();
        let id = temporary.paste_workspace_object(scene, self)?;
        if let Some(object) = temporary.scene_manager_mut().object_mut(id) {
            object.set_source_type("qdfmol");
        }
        let bytes = temporary.active_scene_bytes()?;
        let (xml, suffix) = split_qsc_document(&bytes)?;
        let tree = parse_ldom2_xml(xml)?;
        let node = tree
            .top()
            .children()
            .iter()
            .find(|node| !node.is_attr() && node.tag_name() == "object")
            .ok_or_else(|| CueMolError::not_found("copied object"))?;
        let mut payload = xml_payload("object", "single", node.clone());
        payload
            .bytes
            .extend_from_slice(QSC_XML_END_MARKER.as_bytes());
        payload.bytes.extend_from_slice(suffix);
        Ok(payload)
    }
}

fn xml_payload(kind: &'static str, form: &'static str, mut node: LDom2Node) -> GuiClipboardPayload {
    clear_xml_formatting_whitespace(&mut node);
    GuiClipboardPayload {
        kind,
        form,
        bytes: write_ldom2_xml(&LDom2Tree::from(node)).into_bytes(),
    }
}

impl WorkspaceRendererClipboard {
    pub fn to_payload(&self) -> Result<GuiClipboardPayload> {
        let (group, nodes) = renderer_nodes(&self.renderer)?;
        if group.is_none() && nodes.len() == 1 {
            return Ok(xml_payload(
                "renderer",
                "single",
                nodes.into_iter().next().unwrap(),
            ));
        }
        let mut root = LDom2Node::new();
        root.set_tag_name("renderers");
        if let Some(group) = group {
            root.set_str_attr("group", group);
        }
        for mut node in nodes {
            node.remove_child("group");
            root.append_child_node(node);
        }
        Ok(xml_payload("renderer", "rendArray", root))
    }
}

fn renderer_nodes(
    data: &WorkspaceRendererClipboardData,
) -> Result<(Option<String>, Vec<LDom2Node>)> {
    use WorkspaceRendererClipboardData as Data;
    let nodes = match data {
        Data::Molecule {
            group_name,
            renderers,
        } => {
            let mut manager = MolSceneManager::new();
            let sid = manager.new_scene("clipboard");
            let scene = manager.scene_mut(sid).unwrap();
            let id = scene.add_molecule("clipboard", MolCoord::new());
            let object = scene.object_mut(id).unwrap();
            for renderer in renderers {
                object.attach_renderer(renderer.clone());
            }
            let bytes = scene.write_qsc_bytes();
            let (xml, _) = split_qsc_document(&bytes)?;
            let tree = parse_ldom2_xml(xml)?;
            let object = tree
                .top()
                .children()
                .iter()
                .find(|node| node.tag_name() == "object")
                .unwrap();
            return Ok((
                group_name.clone(),
                object
                    .children()
                    .iter()
                    .filter(|node| !node.is_attr() && node.tag_name() == "renderer")
                    .cloned()
                    .collect(),
            ));
        }
        Data::DensityMap(renderer) => vec![density_renderer_qsc_node(renderer)],
        Data::DensityMapList(renderers) => {
            renderers.iter().map(density_renderer_qsc_node).collect()
        }
        Data::DensityMapGroup { group, renderers } => {
            return Ok((
                Some(group.str_attr("name").to_string()),
                renderers.iter().map(density_renderer_qsc_node).collect(),
            ))
        }
        Data::Surface(node) | Data::Lightweight { node, .. } => vec![node.clone()],
        Data::SurfaceList(nodes) => nodes.clone(),
        Data::SurfaceGroup { group, renderers } => {
            return Ok((Some(group.str_attr("name").to_string()), renderers.clone()))
        }
        Data::LightweightList(renderers) => {
            renderers.iter().map(|(_, node)| node.clone()).collect()
        }
        Data::LightweightGroup { group, renderers } => {
            return Ok((
                Some(group.str_attr("name").to_string()),
                renderers.iter().map(|(_, node)| node.clone()).collect(),
            ))
        }
        Data::Multiple { items, .. } => {
            let mut nodes = Vec::new();
            for item in items {
                let (group, renderers) = renderer_nodes(item)?;
                if let Some(name) = group {
                    nodes.push(renderer_group_qsc_node(&name));
                }
                nodes.extend(renderers);
            }
            nodes
        }
    };
    Ok((None, nodes))
}

fn clipboard_document(bytes: &[u8]) -> Result<(&str, &[u8])> {
    if byte_index(bytes, QSC_XML_END_MARKER.as_bytes()).is_some() {
        split_qsc_document(bytes)
    } else {
        Ok((
            std::str::from_utf8(bytes)
                .map_err(|_| CueMolError::parse("clipboard XML is not UTF-8"))?,
            &[],
        ))
    }
}

fn wrap_object(node: LDom2Node, suffix: &[u8]) -> Vec<u8> {
    let mut root = LDom2Node::new();
    root.set_tag_name("scene");
    root.append_child_node(node);
    let mut bytes = write_ldom2_xml(&LDom2Tree::from(root)).into_bytes();
    bytes.extend_from_slice(QSC_XML_END_MARKER.as_bytes());
    bytes.extend_from_slice(suffix);
    bytes
}

impl DesktopAppState {
    pub fn paste_clipboard_bytes(
        &mut self,
        kind: &str,
        form: &str,
        bytes: &[u8],
        target: Option<Uid>,
        target_type: Option<&str>,
    ) -> Result<GuiPastedNode> {
        let (xml, suffix) = clipboard_document(bytes)?;
        let tree = parse_ldom2_xml(xml)?;
        let sid = self.ensure_active_scene();
        if kind == "camera" {
            let camera = self.parse_scene_camera_xml(sid, xml)?;
            return self
                .paste_workspace_camera(sid, &WorkspaceCameraClipboard { camera, xml: None })
                .map(GuiPastedNode::Camera);
        }
        if kind == "style" {
            if tree.top().tag_name() != "styles" {
                return Err(CueMolError::parse("expected styles XML"));
            }
            let mut node = tree.top().clone();
            let original = node.str_attr("id").to_string();
            let original = if original.is_empty() {
                "style"
            } else {
                &original
            };
            let name = unique_name(original, |name| self.workspace_style_exists(sid, name));
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
            let mut temporary = DesktopAppState::new();
            temporary.open_bytes("clipboard.qsc", &wrap_object(tree.top().clone(), suffix))?;
            let scene = temporary
                .active_scene()
                .ok_or_else(|| CueMolError::not_found("clipboard scene"))?;
            let object = object_ids(&temporary, scene)
                .first()
                .copied()
                .ok_or_else(|| CueMolError::not_found("clipboard object"))?;
            let clipboard = temporary.copy_workspace_object(object)?;
            return self
                .paste_workspace_object(sid, &clipboard)
                .map(GuiPastedNode::Object);
        }
        if kind != "renderer" {
            return Err(CueMolError::invalid_argument("unknown clipboard kind"));
        }
        let target =
            target.ok_or_else(|| CueMolError::invalid_argument("renderer target is required"))?;
        let (object, group) = if target_type == Some("rendGroup") {
            let object = object_ids(self, sid)
                .into_iter()
                .find(|object| {
                    self.renderer_summaries(*object)
                        .iter()
                        .any(|renderer| renderer.uid == target)
                        || self.density_map_summary(*object).is_some_and(|item| {
                            item.renderer_groups
                                .iter()
                                .any(|renderer| renderer.uid == target)
                        })
                        || self.surface_summary(*object).is_some_and(|item| {
                            item.renderer_groups
                                .iter()
                                .any(|renderer| renderer.uid == target)
                        })
                        || self.lw_object_summary(*object).is_some_and(|item| {
                            item.renderer_groups
                                .iter()
                                .any(|renderer| renderer.uid == target)
                        })
                })
                .ok_or_else(|| CueMolError::not_found("renderer group"))?;
            (object, Some(target))
        } else {
            (target, None)
        };
        let (group_name, nodes) = if form == "rendArray" {
            if tree.top().tag_name() != "renderers" {
                return Err(CueMolError::parse("expected renderer array"));
            }
            let group = tree.top().str_attr("group").to_string();
            (
                (!group.is_empty()).then_some(group),
                tree.top()
                    .children()
                    .iter()
                    .filter(|node| !node.is_attr() && node.tag_name() == "renderer")
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        } else {
            if tree.top().tag_name() != "renderer" {
                return Err(CueMolError::parse("expected renderer XML"));
            }
            (None, vec![tree.top().clone()])
        };
        let clipboard = self.renderer_clipboard_from_nodes(object, group_name, nodes)?;
        let result = self.paste_workspace_renderers(object, group, &clipboard)?;
        Ok(GuiPastedNode::Renderers {
            uids: result.renderer_uids,
            group: result.group_uid,
        })
    }

    fn renderer_clipboard_from_nodes(
        &self,
        object: Uid,
        group_name: Option<String>,
        nodes: Vec<LDom2Node>,
    ) -> Result<WorkspaceRendererClipboard> {
        use WorkspaceRendererClipboardData as Data;
        if group_name.is_none() && nodes.iter().any(|node| node.type_name() == "*group") {
            let mut items = Vec::with_capacity(nodes.len());
            for node in nodes {
                let clipboard = if node.type_name() == "*group" {
                    self.renderer_clipboard_from_nodes(
                        object,
                        Some(node.str_attr("name").to_string()),
                        Vec::new(),
                    )?
                } else {
                    self.renderer_clipboard_from_nodes(object, None, vec![node])?
                };
                items.push(clipboard.renderer);
            }
            let kind = items
                .first()
                .map(workspace_renderer_clipboard_kind)
                .ok_or_else(|| CueMolError::invalid_argument("empty renderer array"))?;
            return Ok(WorkspaceRendererClipboard {
                renderer: Data::Multiple { kind, items },
            });
        }
        let renderer = if self.scene_manager().object(object).is_some() {
            let mut root = LDom2Node::new();
            root.set_tag_name("object");
            root.set_type_name("MolCoord");
            root.set_str_attr("name", "clipboard");
            root.set_str_attr("src", "datachunk:00000");
            root.set_str_attr("srctype", "pdb");
            for node in nodes {
                if MOLECULE_RENDERER_TYPES.contains(&node.type_name())
                    || node.type_name() == "*group"
                {
                    root.append_child_node(node);
                }
            }
            let bytes = wrap_object(root, b"\n==========START datachunk:00000==========\nEND\n==========END datachunk:00000==========\n");
            let mut manager = MolSceneManager::new();
            let sid = manager.new_scene("clipboard");
            let scene = manager.scene_mut(sid).unwrap();
            scene.load_qsc_bytes_with_molecule_decoder(&bytes, None, &|_, _, _, _| {
                Ok(MolCoord::new())
            })?;
            Data::Molecule {
                group_name,
                renderers: scene
                    .objects()
                    .first()
                    .ok_or_else(|| CueMolError::not_found("clipboard renderers"))?
                    .renderers()
                    .to_vec(),
            }
        } else if self.density_maps.contains_key(&object) {
            let mut root = LDom2Node::new();
            for node in nodes {
                root.append_child_node(node);
            }
            let scene = self
                .scene_manager()
                .scene(self.density_maps[&object].scene_uid)
                .unwrap();
            let renderers = density_renderer_configs(Some(&root), &scene_style_manager(scene));
            if let Some(name) = group_name {
                Data::DensityMapGroup {
                    group: renderer_group_qsc_node(&name),
                    renderers,
                }
            } else {
                Data::DensityMapList(renderers)
            }
        } else if self.surface_summary(object).is_some() {
            let renderers = nodes
                .into_iter()
                .filter(|node| node.type_name() == "molsurf")
                .collect();
            if let Some(name) = group_name {
                Data::SurfaceGroup {
                    group: renderer_group_qsc_node(&name),
                    renderers,
                }
            } else {
                Data::SurfaceList(renderers)
            }
        } else if self
            .scene_manager()
            .scene_uid_for_lw_object(object)
            .is_some()
        {
            let renderers = nodes
                .into_iter()
                .filter(|node| node.type_name() == "lwrend")
                .map(|node| {
                    let renderer = LoadedLWRenderer {
                        name: node.str_attr("name").to_string(),
                        data_id: node.str_attr("data_id").as_str().parse().unwrap_or(0),
                        visible: node.str_attr("visible").is_empty()
                            || qsc_bool(node.str_attr("visible").as_str()),
                        xform: Matrix4::from_script_string(node.str_attr("xformMat").as_str())
                            .unwrap_or(Matrix4::IDENTITY),
                    };
                    (renderer, node)
                })
                .collect();
            if let Some(name) = group_name {
                Data::LightweightGroup {
                    group: renderer_group_qsc_node(&name),
                    renderers,
                }
            } else {
                Data::LightweightList(renderers)
            }
        } else {
            return Err(CueMolError::not_found("clipboard target object"));
        };
        Ok(WorkspaceRendererClipboard { renderer })
    }
}

fn object_ids(state: &DesktopAppState, scene: Uid) -> Vec<Uid> {
    state
        .object_summaries(scene)
        .into_iter()
        .map(|item| item.uid)
        .chain(
            state
                .density_map_summaries(scene)
                .into_iter()
                .map(|item| item.uid),
        )
        .chain(
            state
                .surface_summaries(scene)
                .into_iter()
                .map(|item| item.uid),
        )
        .chain(
            state
                .lw_object_summaries(scene)
                .into_iter()
                .map(|item| item.uid),
        )
        .collect()
}

pub(super) fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (1_u64..)
        .map(|index| format!("{base}_{index}"))
        .find(|name| !taken(name))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    const PDB: &[u8] = b"ATOM      1  CA  GLY A   1       1.234   2.345   3.456  1.00 10.00           C\nATOM      2  CA  GLY A   2       4.321   5.432   6.543  1.00 20.00           C\nEND\n";

    fn molecule(state: &mut DesktopAppState, name: &str) -> Uid {
        let sid = state.ensure_active_scene();
        state
            .scene_manager_mut()
            .scene_mut(sid)
            .unwrap()
            .add_molecule(
                name,
                read_pdb_str(std::str::from_utf8(PDB).unwrap()).unwrap(),
            )
    }

    fn paste(
        state: &mut DesktopAppState,
        payload: &GuiClipboardPayload,
        target: Option<Uid>,
        kind: Option<&str>,
    ) -> GuiPastedNode {
        state
            .paste_clipboard_bytes(payload.kind, payload.form, &payload.bytes, target, kind)
            .unwrap()
    }

    #[test]
    fn clipboard_empty_molecule_keeps_its_renderer() {
        let mut state = DesktopAppState::new();
        let scene = state.ensure_active_scene();
        let object = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("empty", MolCoord::new());
        state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer("simple", "lines");
        let payload = state
            .copy_workspace_object(object)
            .unwrap()
            .to_payload()
            .unwrap();
        let GuiPastedNode::Object(pasted) = paste(&mut state, &payload, None, None) else {
            panic!()
        };
        let object = state.scene_manager().object(pasted).unwrap();
        assert_eq!(object.name(), "empty_1");
        assert_eq!(object.mol().atom_count(), 0);
        assert_eq!(object.renderers().len(), 1);
        assert_eq!(object.renderers()[0].name(), "lines");
    }

    #[test]
    fn clipboard_object_is_embedded_and_survives_source_deletion() {
        let mut source = DesktopAppState::new();
        let id = molecule(&mut source, "分子");
        let mut expected = source
            .scene_manager()
            .object(id)
            .unwrap()
            .mol()
            .atoms()
            .to_vec();
        for atom in &mut expected {
            atom.position.x = f64::from(atom.position.x as f32);
            atom.position.y = f64::from(atom.position.y as f32);
            atom.position.z = f64::from(atom.position.z as f32);
        }
        let payload = source
            .copy_workspace_object(id)
            .unwrap()
            .to_payload()
            .unwrap();
        assert!(payload
            .bytes
            .windows(10)
            .any(|bytes| bytes == b"datachunk:"));
        source.destroy_object(id).unwrap();
        let mut destination = DesktopAppState::new();
        let GuiPastedNode::Object(first) = paste(&mut destination, &payload, None, None) else {
            panic!()
        };
        let GuiPastedNode::Object(second) = paste(&mut destination, &payload, None, None) else {
            panic!()
        };
        assert_eq!(
            destination.scene_manager().object(first).unwrap().name(),
            "分子"
        );
        assert_eq!(
            destination.scene_manager().object(second).unwrap().name(),
            "分子_1"
        );
        assert_eq!(
            destination
                .scene_manager()
                .object(first)
                .unwrap()
                .mol()
                .atoms(),
            expected
        );
        assert_eq!(destination.active_object_summaries().len(), 2);
    }

    #[test]
    fn clipboard_renderer_keeps_children_properties_and_destination_group() {
        let mut state = DesktopAppState::new();
        let id = molecule(&mut state, "molecule");
        let sid = state.active_scene().unwrap();
        let group = state.create_renderer_group(id, "source group").unwrap();
        let rid = state
            .new_renderer(sid, id, "cartoon", "ribbon")
            .unwrap()
            .result_renderer;
        state
            .move_renderer_in_object(id, rid, group, "inside")
            .unwrap();
        let children = parse_ldom2_xml(
            "<coloring type=\"PaintColoring\"><paint sel=\"resid 1\" color=\"red\"/></coloring>",
        )
        .unwrap();
        let renderer = state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .renderer_mut(rid)
            .unwrap();
        renderer.set_qsc_attr("width", "2.5");
        renderer.set_qsc_attr("default_alpha", "0.4");
        renderer.set_qsc_children(vec![children.top().clone()]);
        let payload = state
            .copy_workspace_renderer(id, group)
            .unwrap()
            .to_payload()
            .unwrap();
        assert_eq!(payload.form, "rendArray");
        let destination = state
            .create_renderer_group(id, "destination group")
            .unwrap();
        let GuiPastedNode::Renderers { uids, group } =
            paste(&mut state, &payload, Some(destination), Some("rendGroup"))
        else {
            panic!()
        };
        assert_eq!(group, Some(destination));
        assert_eq!(uids.len(), 1);
        let renderer = state
            .scene_manager()
            .object(id)
            .unwrap()
            .renderer(uids[0])
            .unwrap();
        assert_eq!(renderer.name(), "ribbon_1");
        assert_eq!(renderer.qsc_attr("width"), Some("2.5"));
        assert_eq!(renderer.qsc_attr("default_alpha"), Some("0.4"));
        assert_eq!(renderer.group(), Some("destination group"));
        assert_eq!(renderer.qsc_children()[0].type_name(), "PaintColoring");
        assert_eq!(
            renderer.qsc_children()[0]
                .find_child("paint")
                .unwrap()
                .str_attr("color")
                .as_str(),
            "red"
        );
        assert_eq!(
            state
                .renderer_summaries(id)
                .iter()
                .filter(|renderer| renderer.renderer_type == "*group")
                .count(),
            2
        );
    }

    #[test]
    fn clipboard_multiple_group_members_are_deduplicated() {
        let mut state = DesktopAppState::new();
        let object = molecule(&mut state, "source");
        let scene = state.active_scene().unwrap();
        let group = state.create_renderer_group(object, "group").unwrap();
        let first = state
            .new_renderer(scene, object, "simple", "first")
            .unwrap()
            .result_renderer;
        let second = state
            .new_renderer(scene, object, "cpk", "second")
            .unwrap()
            .result_renderer;
        for id in [first, second] {
            state
                .move_renderer_in_object(object, id, group, "inside")
                .unwrap();
        }
        let payload = state
            .copy_workspace_renderers(&[(object, group), (object, first)])
            .unwrap()
            .to_payload()
            .unwrap();
        let target = molecule(&mut state, "target");
        let GuiPastedNode::Renderers { uids, group } =
            paste(&mut state, &payload, Some(target), Some("object"))
        else {
            panic!()
        };
        assert!(group.is_none());
        assert_eq!(uids.len(), 2);
        let renderers = state.scene_manager().object(target).unwrap().renderers();
        assert_eq!(
            renderers
                .iter()
                .map(|renderer| renderer.name())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert!(renderers.iter().all(|renderer| renderer.group().is_none()));
    }

    #[test]
    fn clipboard_style_and_camera_names_follow_reference() {
        let mut state = DesktopAppState::new();
        let sid = state.ensure_active_scene();
        let style = state.create_scene_style(sid, "appearance").unwrap();
        let payload = state
            .copy_workspace_style(sid, style)
            .unwrap()
            .to_payload()
            .unwrap();
        let GuiPastedNode::Style { name, .. } = paste(&mut state, &payload, None, None) else {
            panic!()
        };
        assert_eq!(name, "appearance_1");
        let mut camera = Camera::new();
        camera.set_name("camera");
        camera.set_zoom(31.5);
        state.set_active_scene_camera(camera).unwrap();
        let payload = state
            .copy_workspace_camera(sid, "camera")
            .unwrap()
            .to_payload()
            .unwrap();
        let GuiPastedNode::Camera(name) = paste(&mut state, &payload, None, None) else {
            panic!()
        };
        assert_eq!(name, "copy1_camera");
        assert_eq!(
            state
                .scene_manager()
                .scene(sid)
                .unwrap()
                .camera_by_name(&name)
                .unwrap()
                .zoom(),
            31.5
        );
    }

    #[test]
    fn clipboard_rejects_wrong_xml_root_without_scene_edits() {
        let mut state = DesktopAppState::new();
        molecule(&mut state, "molecule");
        let before = state.clone();
        assert!(state
            .paste_clipboard_bytes("object", "single", b"<scene/>", None, None)
            .is_err());
        assert!(state
            .paste_clipboard_bytes(
                "renderer",
                "rendArray",
                b"<renderer type=\"simple\"/>",
                state
                    .active_object_summaries()
                    .first()
                    .map(|object| object.uid),
                None
            )
            .is_err());
        assert_eq!(state.active_object_summaries().len(), 1);
        assert!(state.same_undo_state(&before));
    }
}
