use super::*;
use serde_json::{json, Value};

impl AppModel {
    pub(super) fn inspector_request(&mut self, tool: &str, args: &Value) -> Result<Value, String> {
        if !matches!(
            tool,
            "list_node_props" | "set_node_prop" | "reset_node_props"
        ) {
            return Err("Unsupported inspector operation".into());
        }
        if tool == "list_node_props" {
            return cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, args)
                .map_err(|error| error.to_string());
        }
        self.stop_console();
        let before = self.capture_undo();
        let result = cuemol_command::call_tool(&mut self.desktop, &mut self.camera, tool, args)
            .map_err(|error| error.to_string())?;
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
            }
            self.invalidate_snapshot(false);
        }
        Ok(json!({"result":result,"snapshot":self.snapshot()}))
    }
}
