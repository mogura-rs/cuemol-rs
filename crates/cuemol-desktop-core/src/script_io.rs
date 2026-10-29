use super::*;

impl DesktopAppState {
    pub(super) fn set_script_object_io_metadata(
        &mut self,
        uid: Uid,
        source: String,
        source_type: String,
        options: LDom2Node,
    ) -> Result<()> {
        if let Some(object) = self.scene_manager_mut().object_mut(uid) {
            let modified = object.modified();
            object.set_source(source);
            object.set_source_type(source_type);
            let mut nodes = object.qsc_extension_nodes().to_vec();
            nodes.retain(|node| node.tag_name() != "ropts");
            nodes.push(options);
            object.replace_shared_qsc_extensions(nodes, object.qsc_extra_data_map().clone());
            object.set_modified(modified);
            return Ok(());
        }
        if let Some(map) = self.density_maps.get_mut(&uid) {
            map.source = source.clone();
            map.format = source_type.clone();
            let scene_uid = map.scene_uid;
            let index = map.storage_index;
            let stored = self
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .loaded_density_maps_mut()
                .get_mut(index)
                .ok_or_else(|| CueMolError::NotFound("density map storage".into()))?;
            set_node_io_metadata(
                &mut stored.qsc_node,
                &stored.name,
                &stored.object_type,
                source,
                source_type,
                options,
            );
            return Ok(());
        }
        let scene_uid = self
            .scene_manager()
            .scenes()
            .iter()
            .find_map(|scene| {
                (scene
                    .loaded_surfaces()
                    .iter()
                    .any(|object| object.uid == uid)
                    || scene
                        .loaded_lw_objects()
                        .iter()
                        .any(|object| object.uid == uid))
                .then_some(scene.uid())
            })
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", uid.get())))?;
        let scene = self.scene_manager_mut().scene_mut(scene_uid).unwrap();
        if let Some(surface) = scene
            .loaded_surfaces_mut()
            .iter_mut()
            .find(|object| object.uid == uid)
        {
            set_node_io_metadata(
                &mut surface.qsc_node,
                &surface.name,
                &surface.object_type,
                source,
                source_type,
                options,
            );
        } else if let Some(object) = scene
            .loaded_lw_objects_mut()
            .iter_mut()
            .find(|object| object.uid == uid)
        {
            set_node_io_metadata(
                &mut object.qsc_node,
                &object.name,
                "LWObject",
                source,
                source_type,
                options,
            );
        }
        Ok(())
    }
}

fn set_node_io_metadata(
    node: &mut Option<LDom2Node>,
    name: &str,
    class: &str,
    source: String,
    source_type: String,
    options: LDom2Node,
) {
    let node = node.get_or_insert_with(|| {
        let mut node = LDom2Node::new();
        node.set_tag_name("object");
        node.set_type_name(class);
        node.set_str_attr("name", name);
        node
    });
    node.set_str_attr("src", source);
    node.set_str_attr("srctype", source_type);
    node.retain_children(|child| child.is_attr() || child.tag_name() != "ropts");
    node.append_child_node(options);
}
