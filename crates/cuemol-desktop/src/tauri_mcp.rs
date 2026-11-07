use super::*;
use serde_json::{json, Value};

use cuemol_command::tool_response;

impl AppModel {
    fn start_mcp_apbs(&mut self, request: cuemol_command::transport::ApiRequest) {
        let args = &request.message["params"]["arguments"];
        let Some(object_uid) = args["objId"].as_u64().filter(|id| *id > 0) else {
            let _ = request.reply.send(Some(tool_response(
                request.message["id"].clone(),
                Err("Invalid object ID".into()),
            )));
            return;
        };
        let input = ApbsCalculationInput {
            object_uid,
            selection: args["selection"].as_str().unwrap_or("*").into(),
            potential_name: args["name"].as_str().unwrap_or("").into(),
            charge_method: args["method"].as_str().unwrap_or("pdb2pqr").into(),
            force_field: args["forceField"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    let configured = self.desktop.config_value("apbs:force_field");
                    if configured.is_empty() {
                        "CHARMM".into()
                    } else {
                        configured
                    }
                }),
            use_hydrogen: args["useHydrogen"] == true,
            use_npbe: args["nonLinear"] == true,
            temperature: args["temperature"].as_f64().unwrap_or(298.15),
            grid_spacing: args["gridSpacing"].as_f64().unwrap_or(1.),
            water_dielectric: args["waterDielec"].as_f64().unwrap_or(78.54),
            protein_dielectric: args["protDielec"].as_f64().unwrap_or(2.),
            apbs_executable: String::new(),
            pdb2pqr_executable: String::new(),
        };
        if let Err(error) = self.begin_remote_edit() {
            let _ = request.reply.send(Some(tool_response(
                request.message["id"].clone(),
                Err(error),
            )));
            return;
        }
        self.stop_console();
        match handle_model_request(self, ModelRequest::StartApbsCalculation(input)) {
            Ok(_) => self.apbs_job.as_mut().unwrap().command = Some(request),
            Err(error) => {
                let _ = request.reply.send(Some(tool_response(
                    request.message["id"].clone(),
                    Err(error),
                )));
            }
        }
    }

    pub(super) fn mcp_request(
        &mut self,
        protocol: &mut cuemol_command::Protocol,
        message: Value,
    ) -> Option<Value> {
        if !protocol.is_tool_call(&message) {
            return protocol.handle(message, &mut self.desktop, &mut self.camera);
        }
        let name = message["params"]["name"].as_str().unwrap().to_owned();
        let args = &message["params"]["arguments"];
        if cuemol_command::tool_is_mutating(&name) {
            if let Err(error) = self.begin_remote_edit() {
                return Some(tool_response(message["id"].clone(), Err(error)));
            }
        }
        if matches!(name.as_str(), "undo" | "redo" | "clear_undo") {
            self.stop_console();
            let result = match name.as_str() {
                "undo" if self.undo_stack.is_empty() => Err("Nothing to undo".into()),
                "redo" if self.redo_stack.is_empty() => Err("Nothing to redo".into()),
                "undo" => self.undo(0),
                "redo" => self.redo(0),
                _ => {
                    self.clear_undo();
                    Ok(())
                }
            };
            return Some(tool_response(
                message["id"].clone(),
                result.map(|()| json!({"changed":true})),
            ));
        }
        let changes_scene = cuemol_command::tool_changes_scene(&name);
        let changes_view = cuemol_command::tool_is_mutating(&name);
        if changes_scene {
            self.stop_console();
        }
        if let Err(error) = self.sync_view_camera() {
            return Some(tool_response(message["id"].clone(), Err(error)));
        }
        let mut before =
            cuemol_command::tool_records_history(&name).then(|| self.capture_script_undo());
        let record =
            cuemol_command::console::actions::Record::operation(&self.desktop, &name, args);
        let response = protocol.handle(message, &mut self.desktop, &mut self.camera);
        let success = response.as_ref().is_some_and(|reply| {
            reply.get("result").is_some() && reply["result"]["isError"] != true
        });
        if let Some(mut before) = before.take() {
            cuemol_command::retain_animation_seed_in_undo(
                &mut before.desktop,
                &self.desktop,
                &name,
            );
            if !before.desktop.same_scene_undo_state(&self.desktop) {
                self.status = format!("MCP: {name}");
                if self.push_script_undo(before) {
                    if let Some(entry) = self.undo_stack.last_mut() {
                        entry.record = record;
                    }
                }
            }
        }
        if success && changes_view {
            let _ = self.desktop.set_active_view_camera(&self.camera);
            self.restore_script_view();
            self.sync_active_scene_properties();
            if changes_scene {
                self.invalidate_snapshot(false);
            }
        }
        response
    }
}

pub(super) fn poll(
    model: &mut AppModel,
    api: &cuemol_command::transport::GuiApiServer,
    protocol: &mut cuemol_command::Protocol,
    changed: impl Fn(),
) {
    let Some(api) = api.server() else {
        return;
    };
    while let Ok(request) = api.requests.try_recv() {
        if !model.desktop.mcp_server_enabled() {
            break;
        }
        let name = request.message["params"]["name"].as_str().unwrap_or("");
        if protocol.is_tool_call(&request.message) && name == "calc_elepot" {
            model.start_mcp_apbs(request);
            continue;
        }
        if protocol.is_tool_call(&request.message)
            && matches!(name, "render_image" | "get_screenshot" | "capture_view")
        {
            let prepared = cuemol_command::prepare_render(
                &model.desktop,
                &model.camera,
                name,
                &request.message["params"]["arguments"],
            );
            match prepared {
                Ok(prepared) => {
                    std::thread::spawn(move || {
                        let response = tool_response(
                            request.message["id"].clone(),
                            prepared.run().map_err(|error| error.to_string()),
                        );
                        let _ = request.reply.send(Some(response));
                    });
                }
                Err(error) => {
                    let _ = request.reply.send(Some(tool_response(
                        request.message["id"].clone(),
                        Err(error.to_string()),
                    )));
                }
            }
            continue;
        }
        let notify =
            protocol.is_tool_call(&request.message) && cuemol_command::tool_is_mutating(name);
        let response = model.mcp_request(protocol, request.message);
        let _ = request.reply.send(response);
        if notify {
            changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn mcp_apbs_remains_asynchronous_and_targets_its_original_scene() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("apbs");
        fs::write(&script, "#!/bin/sh\nbase=$(awk '/write pot dx/{print $4}' \"$1\")\nsleep 0.1\nprintf 'object 1 class gridpositions counts 1 1 2\\norigin 0 0 0\\ndelta 1 0 0\\ndelta 0 1 0\\ndelta 0 0 1\\nobject 2 class gridconnections counts 1 1 2\\nobject 3 class array type double rank 0 items 2 data follows\\n-1 1\\n' > \"${base}.dx\"\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let mut model = super::super::tests::model_without_gpu();
        model.desktop.set_plugin_enabled("mcp", true).unwrap();
        model
            .desktop
            .put_config_value("cuemol2.ui.apbs-exe-path", script.to_string_lossy());
        let scene = model.desktop.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nATOM      2  CB  ALA A   1       2.000   3.000   4.000  1.00 10.00           C  \nEND\n").unwrap();
        let object = model
            .desktop
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("source", molecule);
        let start = |model: &mut AppModel, id, args| {
            let (reply, response) = mpsc::sync_channel(1);
            model.start_mcp_apbs(cuemol_command::transport::ApiRequest { message:json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"calc_elepot","arguments":args}}), reply });
            response
        };
        let invalid = start(&mut model, 0, json!({"objId":-1,"method":"internal"}));
        assert_eq!(invalid.recv().unwrap().unwrap()["result"]["isError"], true);
        assert!(model.apbs_job.is_none());
        let response = start(
            &mut model,
            1,
            json!({"objId":object.get(),"method":"internal","name":"potential"}),
        );
        assert!(response.try_recv().is_err());
        assert!(model.apbs_job.is_some());
        let other = model.desktop.new_scene("other", true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let response = loop {
            handle_model_request(&mut model, ModelRequest::PollApbsCalculation).unwrap();
            if let Ok(response) = response.try_recv() {
                break response.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(response["result"]["isError"], false);
        let body: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        let map = Uid::new(body["result"]["objectId"].as_u64().unwrap());
        assert_eq!(
            model.desktop.density_map_summary(map).unwrap().scene_uid,
            scene
        );
        assert_eq!(model.desktop.active_scene(), Some(other.result_scene));
        assert_eq!(model.undo_stack.len(), 1);
        model.undo(0).unwrap();
        assert!(model.desktop.density_map_summary(map).is_none());
        let response = start(
            &mut model,
            2,
            json!({"objId":object.get(),"method":"internal","name":"cancelled"}),
        );
        model.desktop.set_plugin_enabled("mcp", false).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let response = loop {
            handle_model_request(&mut model, ModelRequest::PollApbsCalculation).unwrap();
            if let Ok(response) = response.try_recv() {
                break response.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(response["result"]["isError"], true);
        assert!(model.desktop.density_map_summaries(scene).is_empty());
        assert!(model.undo_stack.is_empty());
    }

    #[test]
    fn mcp_edits_restore_active_previews_and_reject_late_drag_events() {
        let mut model = super::super::tests::model_without_gpu();
        model.desktop.open_bytes("preview.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = model.desktop.active_object_summaries()[0].uid;
        let renderer = model
            .desktop
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer("cpk", "preview");
        let target = json!({"nodeType":"renderer","nodeId":renderer.get()});
        let read = |model: &mut AppModel| {
            model.inspector_request("list_node_props", &target).unwrap()["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["key"] == "vdwr_C")
                .unwrap()
                .clone()
        };
        let original = read(&mut model);
        let mut args = json!({"nodeType":"renderer","nodeId":renderer.get(),"sceneId":model.desktop.active_scene().unwrap().get(),"prop":"vdwr_C","mode":"preview","value":"2.1","originalValue":original["value"],"originalWasDefault":true,"remoteRevision":0});
        model
            .inspector_request("edit_node_property", &args)
            .unwrap();
        assert_eq!(read(&mut model)["value"], "2.1");
        let mut protocol = cuemol_command::Protocol::without_history();
        model.mcp_request(
            &mut protocol,
            json!({"jsonrpc":"2.0","id":0,"method":"initialize"}),
        );
        let result = model.mcp_request(&mut protocol, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"set_node_prop","arguments":{"nodeType":"renderer","nodeId":renderer.get(),"prop":"vdwr_C","value":"3.2"}}})).unwrap();
        assert_eq!(result["result"]["isError"], false);
        assert_eq!(read(&mut model)["value"], "3.2");
        assert_eq!(model.undo_stack.len(), 1);
        for mode in ["preview", "commit", "abort"] {
            args["mode"] = json!(mode);
            assert!(model
                .inspector_request("edit_node_property", &args)
                .is_err());
            assert_eq!(read(&mut model)["value"], "3.2");
        }
        model.undo(0).unwrap();
        assert_eq!(read(&mut model), original);
        assert!(model.numeric_preview.is_none());
        assert_eq!(model.snapshot().remote_revision, 1);
        args["mode"] = json!("preview");
        args["remoteRevision"] = json!(1);
        model
            .inspector_request("edit_node_property", &args)
            .unwrap();
        cuemol_command::call_tool(
            &mut model.desktop,
            &mut model.camera,
            "delete_node",
            &target,
        )
        .unwrap();
        let result = model.mcp_request(&mut protocol, json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"set_node_prop","arguments":{"nodeType":"object","nodeId":object.get(),"prop":"name","value":"After removal"}}})).unwrap();
        assert_eq!(result["result"]["isError"], false);
        assert_eq!(
            model.desktop.scene_manager().object(object).unwrap().name(),
            "After removal"
        );
        assert_eq!(model.snapshot().remote_revision, 2);
    }

    #[test]
    fn mcp_uses_tauri_history_and_preserves_preferences_and_view_navigation() {
        let mut model = super::super::tests::model_without_gpu();
        let mut protocol = cuemol_command::Protocol::without_history();
        let call = |model: &mut AppModel,
                    protocol: &mut cuemol_command::Protocol,
                    name: &str,
                    args| {
            model.mcp_request(protocol, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap()
        };
        assert!(call(&mut model, &mut protocol, "undo", json!({}))
            .get("error")
            .is_some());
        model
            .mcp_request(
                &mut protocol,
                json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}),
            )
            .unwrap();
        let scene = model.desktop.active_scene().unwrap();
        let original = model
            .desktop
            .scene_manager()
            .scene(scene)
            .unwrap()
            .name()
            .to_string();
        assert_eq!(
            call(
                &mut model,
                &mut protocol,
                "set_node_prop",
                json!({"nodeType":"scene","prop":"name","value":"remote"})
            )["result"]["isError"],
            false
        );
        assert_eq!(model.undo_stack.len(), 1);
        model.desktop.set_plugin_enabled("mdtools", true).unwrap();
        assert_eq!(
            call(&mut model, &mut protocol, "undo", json!({}))["result"]["isError"],
            false
        );
        assert_eq!(
            model.desktop.scene_manager().scene(scene).unwrap().name(),
            original
        );
        assert!(model.desktop.plugin_enabled("mdtools"));
        assert_eq!(model.redo_stack.len(), 1);
        assert_eq!(
            call(
                &mut model,
                &mut protocol,
                "set_view",
                json!({"center":[3,4,5]})
            )["result"]["isError"],
            false
        );
        assert_eq!(model.redo_stack.len(), 1);
        assert_eq!(
            call(&mut model, &mut protocol, "redo", json!({}))["result"]["isError"],
            false
        );
        assert_eq!(
            model.desktop.scene_manager().scene(scene).unwrap().name(),
            "remote"
        );
        assert_eq!(model.camera.center().x, 3.);
        assert_eq!(
            call(
                &mut model,
                &mut protocol,
                "set_node_prop",
                json!({"nodeType":"scene","prop":"name"})
            )["result"]["isError"],
            true
        );
        assert_eq!(model.undo_stack.len(), 1);
    }
}
