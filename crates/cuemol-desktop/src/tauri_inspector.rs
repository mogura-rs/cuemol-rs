use super::*;
use serde_json::{json, Value};

impl AppModel {
    pub(super) fn inspector_request(&mut self, tool: &str, args: &Value) -> Result<Value, String> {
        if tool == "animation_element_detail" {
            if args["sceneId"].as_u64() != self.desktop.active_scene().map(|sid| sid.get()) {
                return Err("The inspected scene changed".into());
            }
            let uid = args["uid"].as_u64().ok_or("Animation uid is required")?;
            return cuemol_command::animation_element_detail(&self.desktop, uid)
                .map_err(|error| error.to_string());
        }
        if tool == "animation_selection" {
            if args["sceneId"].as_u64() != self.desktop.active_scene().map(|sid| sid.get()) {
                return Err("The inspected scene changed".into());
            }
            let animation = self.active_animation_manager()?;
            let uids = args["uids"]
                .as_array()
                .ok_or("Animation uids are required")?;
            let indices = uids
                .iter()
                .map(|uid| {
                    animation
                        .motions()
                        .iter()
                        .position(|motion| Some(motion.base().uid().get()) == uid.as_u64())
                        .ok_or_else(|| "Animation element was not found".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            match args["action"].as_str() {
                Some("delete") => self.delete_animation_objects(&indices)?,
                Some("up") => self.move_animation_objects(&indices, -1)?,
                Some("down") => self.move_animation_objects(&indices, 1)?,
                _ => return Err("Unknown animation selection action".into()),
            }
            return Ok(json!({"snapshot":self.snapshot()}));
        }
        if tool == "edit_animation_property" {
            self.stop_console();
            let command =
                cuemol_command::prepare_animation_edit(&mut self.desktop, &mut self.camera, args)
                    .map_err(|error| error.to_string())?;
            if let Some((tool, mut command)) = command {
                command["sceneId"] = args["sceneId"].clone();
                return self.inspector_request(tool, &command);
            }
            self.reset_animation_playback();
            self.sync_active_scene_properties();
            self.invalidate_snapshot(false);
            return Ok(json!({"snapshot":self.snapshot()}));
        }
        let animation = matches!(
            tool,
            "add_anim"
                | "set_anim_time"
                | "set_anim_prop"
                | "set_anim_generic_prop"
                | "reset_anim_prop"
        );
        if animation && args["sceneId"].as_u64() != self.desktop.active_scene().map(|sid| sid.get())
        {
            return Err("The inspected scene changed".into());
        }
        let mut command_args = args.clone();
        if animation {
            command_args
                .as_object_mut()
                .ok_or("Inspector arguments must be an object")?
                .remove("sceneId");
        }
        let args = if animation { &command_args } else { args };
        if tool == "style_edit" {
            return self.style_edit_request(args);
        }
        if tool == "selection_history" {
            return self.selection_history_request(args);
        }
        if tool == "selection_edit" {
            return self.selection_edit_request(args);
        }
        if tool == "selection_info" {
            return cuemol_command::selection::request(&self.desktop, args)
                .map_err(|error| error.to_string());
        }
        if tool == "edit_node_property" {
            self.stop_console();
            let command =
                cuemol_command::prepare_property_edit(&mut self.desktop, &mut self.camera, args)
                    .map_err(|error| error.to_string())?;
            if let Some(command) = command {
                return self.inspector_request("set_node_prop", &command);
            }
            self.sync_active_scene_properties();
            self.invalidate_snapshot(false);
            return Ok(json!({"snapshot":self.snapshot()}));
        }
        if !animation
            && !matches!(
                tool,
                "list_node_props" | "set_node_prop" | "set_node_props" | "reset_node_props"
            )
        {
            return Err("Unsupported inspector operation".into());
        }
        if !args.is_object() {
            return Err("Inspector arguments must be an object".into());
        }
        if tool == "list_node_props" {
            return cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, args)
                .map_err(|error| error.to_string());
        }
        self.stop_console();
        let before = self.capture_undo();
        let record = cuemol_command::console::actions::Record::operation(&self.desktop, tool, args);
        let result = cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, args)
            .map_err(|error| error.to_string())?;
        if animation {
            self.reset_animation_playback();
        }
        self.sync_active_scene_properties();
        let changed = !before.desktop.same_undo_state(&self.desktop);
        if changed || !cuemol_command::console::same_camera_view(&before.camera, &self.camera) {
            self.status = if tool == "reset_node_props" {
                "Reset properties".into()
            } else {
                format!("Change property: {}", args["prop"].as_str().unwrap_or(""))
            };
            if args["nodeType"] != "view" && changed {
                self.push_undo(before);
                if let Some(entry) = self.undo_stack.last_mut() {
                    entry.record = record;
                }
            }
            self.invalidate_snapshot(false);
        }
        let remembered =
            cuemol_command::selection::remember_property_edit(&mut self.desktop, tool, args);
        let history = remembered.then(|| self.desktop.selection_history());
        let warning = if remembered {
            self.save_config().err()
        } else {
            None
        };
        Ok(json!({"result":result,"snapshot":self.snapshot(),"history":history,"warning":warning}))
    }

    fn style_edit_request(&mut self, args: &Value) -> Result<Value, String> {
        let input: StyleTargetInput =
            serde_json::from_value(args["target"].clone()).map_err(|error| error.to_string())?;
        let target = self.style_target(input)?;
        let value = match args.get("value") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.as_str()),
            Some(_) => return Err("Style entry value must be a string".into()),
        };
        let tool = match (args["kind"].as_str(), value) {
            (Some("color"), Some(_)) => "define_color",
            (Some("color"), None) => "remove_color",
            (Some("selection"), Some(_)) => "define_selection",
            (Some("selection"), None) => "remove_selection",
            (Some("style"), None) => "remove_style",
            _ => return Err("Unsupported style entry operation".into()),
        };
        let name = args["name"].as_str().ok_or("Missing style entry name")?;
        let mut command = json!({
            "sceneId":input.owner_scene_uid,
            "scope":if target.scope().get() == 0 { "global" } else { "scene" },
            "styleId":target.runtime_id(),
            "styleSet":self.desktop.style_name(target).map_err(|error|error.to_string())?,
            "name":name,
        });
        if let Some(value) = value {
            command[if tool == "define_color" {
                "color"
            } else {
                "selection"
            }] = json!(value);
        }
        self.stop_console();
        let before = self.capture_undo();
        let record =
            cuemol_command::console::actions::Record::operation(&self.desktop, tool, &command);
        let result =
            match cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, &command) {
                Ok(value) => value,
                Err(error) => {
                    self.restore_undo(before)?;
                    return Err(error.to_string());
                }
            };
        self.status = format!(
            "{} style entry {name}",
            if value.is_some() { "Edit" } else { "Remove" }
        );
        if input.scene_uid != 0 && !before.desktop.same_scene_undo_state(&self.desktop) {
            self.push_undo_for_scene(
                before,
                (input.scene_uid != 0).then_some(Uid::new(input.scene_uid)),
            );
            if let Some(entry) = self.undo_stack.last_mut() {
                entry.record = record;
            }
        }
        self.invalidate_snapshot(false);
        Ok(json!({"result":result,"snapshot":self.snapshot()}))
    }

    fn selection_history_request(&mut self, args: &Value) -> Result<Value, String> {
        match args["action"].as_str() {
            Some("import") => {
                let mut values = self.desktop.selection_history();
                let imported = args["entries"]
                    .as_array()
                    .ok_or("Missing history entries")?;
                for value in imported.iter().filter_map(Value::as_str) {
                    let value = value.trim();
                    if !matches!(value, "" | "*" | "none") && !values.iter().any(|v| v == value) {
                        values.push(value.into());
                    }
                    if values.len() >= cuemol_desktop_core::MAX_SELECTION_HISTORY {
                        break;
                    }
                }
                self.desktop.restore_selection_history(&values);
            }
            Some("remember") => self.desktop.remember_selection(
                args["expression"]
                    .as_str()
                    .ok_or("Missing selection expression")?,
            ),
            Some("list") => return Ok(json!({"history":self.desktop.selection_history()})),
            _ => return Err("Unknown selection history operation".into()),
        }
        let warning = self.save_config().err();
        Ok(json!({"history":self.desktop.selection_history(),"warning":warning}))
    }

    fn selection_edit_request(&mut self, args: &Value) -> Result<Value, String> {
        let scene = self.desktop.active_scene().ok_or("No active scene")?;
        if args["sceneId"].as_u64() != Some(scene.get()) {
            return Err("Selection scene changed".into());
        }
        let molecule = args["moleculeId"]
            .as_u64()
            .map(Uid::new)
            .ok_or("Missing molecule")?;
        if self.desktop.scene_manager().scene_uid_for_object(molecule) != Some(scene) {
            return Err("Selection molecule changed".into());
        }
        let expression = args["expression"]
            .as_str()
            .ok_or("Missing selection expression")?;
        if let Some(error) =
            cuemol_command::selection::evaluate(&self.desktop, Some(molecule), expression).error
        {
            return Err(error);
        }
        let (tool, command) = match args["action"].as_str() {
            Some("apply") => (
                "set_mol_selection",
                json!({"objId":molecule.get(),"selection":expression}),
            ),
            Some("center") => (
                "center_view",
                json!({"objId":molecule.get(),"selection":expression,"zoom":false}),
            ),
            Some("define") => (
                "define_selection",
                json!({"name":args["name"],"selection":expression,"styleSet":cuemol_command::selection::definition_style_set(&self.desktop)}),
            ),
            _ => return Err("Unknown selection edit".into()),
        };
        self.stop_console();
        let before = self.capture_undo();
        let record =
            cuemol_command::console::actions::Record::operation(&self.desktop, tool, &command);
        let result =
            match cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, &command) {
                Ok(result) => result,
                Err(error) => {
                    self.restore_undo(before)?;
                    return Err(error.to_string());
                }
            };
        if tool == "set_mol_selection" {
            self.selection = expression.into();
        }
        self.status = match tool {
            "define_selection" => "Define selection",
            "center_view" => "Center on selection",
            _ => "Apply selection",
        }
        .into();
        if !before.desktop.same_undo_state(&self.desktop)
            || !cuemol_command::console::same_camera_view(&before.camera, &self.camera)
            || before.selection != self.selection
        {
            self.push_undo(before);
            if let Some(entry) = self.undo_stack.last_mut() {
                entry.record = record;
            }
        }
        let warning = if tool == "define_selection"
            || tool == "set_mol_selection" && args["remember"] == true
        {
            self.desktop.remember_selection(expression);
            self.save_config().err()
        } else {
            None
        };
        self.invalidate_snapshot(false);
        Ok(
            json!({"result":result,"snapshot":self.snapshot(),"history":self.desktop.selection_history(),"warning":warning}),
        )
    }
}
