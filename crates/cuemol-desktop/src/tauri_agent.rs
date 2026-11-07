use super::*;
use serde_json::{json, Value};

impl AppModel {
    pub(super) fn load_agent_settings(&mut self) {
        if let Some(path) = self.agent_settings_path() {
            if let Ok(source) = fs::read_to_string(path) {
                self.agent.load_settings(&source);
                if self
                    .desktop
                    .config_value("plugins:agent:enabled")
                    .is_empty()
                    && self.agent.settings.enabled
                {
                    let _ = self.desktop.set_plugin_enabled("agent", true);
                }
            }
        }
        self.agent.settings.enabled = self.desktop.plugin_enabled("agent");
    }

    fn agent_settings_path(&self) -> Option<PathBuf> {
        self.config_path
            .as_ref()?
            .parent()
            .map(|parent| parent.join("agent.json"))
    }

    fn save_agent_settings(&mut self) -> Result<(), String> {
        if let Some(path) = self.agent_settings_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(path, self.agent.settings_json()).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn agent_request(
        &mut self,
        operation: &str,
        mut args: Value,
    ) -> Result<Value, String> {
        self.sync_agent_plugin();
        match operation {
            "status" => {}
            "configure" => {
                let mut settings: cuemol_agent::Settings =
                    serde_json::from_value(args["settings"].take())
                        .map_err(|error| error.to_string())?;
                settings.enabled = self.desktop.plugin_enabled("agent");
                self.agent.configure(settings)?;
                self.save_agent_settings()?;
            }
            "key" => {
                let Value::String(key) = args["key"].take() else {
                    return Err("API key is required".into());
                };
                self.agent.set_session_key(key);
                if args["save"] == true {
                    self.agent.save_key()?;
                }
            }
            "delete_key" => self.agent.delete_key()?,
            "models" => self.agent.refresh_models(self.agent_wake.clone())?,
            "start" => {
                if !self.desktop.plugin_enabled("agent") {
                    return Err("Enable AI Agent in Settings > Plugins first.".into());
                }
                let prompt = args["prompt"].as_str().unwrap_or("").trim();
                if prompt.is_empty() {
                    return Err("Enter a message first.".into());
                }
                if self.agent.running() {
                    return Err("An Agent turn is already running.".into());
                }
                self.begin_remote_edit()?;
                self.stop_console();
                self.sync_view_camera()?;
                self.agent_history =
                    Some(history::Group::new(self.capture_script_undo(), "AI Agent"));
                self.agent.start(
                    &mut self.desktop,
                    &mut self.camera,
                    prompt.into(),
                    self.agent_wake.clone(),
                    self.snapshot_revision,
                );
            }
            "cancel" => self.agent.cancel("Cancelled; completed edits are kept."),
            "clear" => self.agent.new_conversation()?,
            _ => return Err("Unknown Agent operation".into()),
        }
        Ok(json!({"agent": self.agent.snapshot()}))
    }

    fn sync_agent_plugin(&mut self) {
        self.agent.settings.enabled = self.desktop.plugin_enabled("agent");
        if !self.agent.settings.enabled {
            self.agent.cancel("AI Agent plugin disabled.");
        }
    }

    pub(super) fn record_agent_history(&mut self, name: &str) {
        let Some(mut history) = self.agent_history.take() else {
            return;
        };
        cuemol_command::retain_animation_seed_in_undo(
            &mut history.before.desktop,
            &self.desktop,
            name,
        );
        self.agent.changed_scene |=
            self.update_grouped_undo(history.before.clone(), history.record.clone());
        self.agent_history = Some(history);
    }

    pub(super) fn poll_agent(&mut self) -> (bool, bool) {
        let before = self.agent.change_id;
        let revision = self.snapshot_revision;
        self.sync_agent_plugin();
        if self.agent.running() && self.agent.revision != self.snapshot_revision {
            self.agent
                .cancel("Scene changed outside the agent; turn cancelled.");
        }
        while let Some(cuemol_ui::AgentCall {
            name,
            arguments,
            reply,
            download,
        }) = self.agent.poll(self.agent_wake.clone())
        {
            if reply.is_closed() {
                continue;
            }
            if let Err(error) = cuemol_command::validate_args(&name, &arguments) {
                let _ = reply.try_send(Err(error));
                continue;
            }
            if matches!(
                name.as_str(),
                "render_image" | "get_screenshot" | "capture_view"
            ) {
                match cuemol_command::prepare_render(&self.desktop, &self.camera, &name, &arguments)
                {
                    Ok(render) => {
                        let cancel = self.agent.operation_cancel();
                        self.render_tasks.spawn(cancel, move |cancel| {
                            let result = render
                                .run_with_cancel(&cancel)
                                .map_err(|error| error.to_string());
                            let _ = reply.send_blocking(result);
                        });
                    }
                    Err(error) => {
                        let _ = reply.try_send(Err(error.to_string()));
                    }
                }
                continue;
            }
            if name == "calc_elepot" {
                if self.apbs_job.is_some() {
                    let _ = reply.try_send(Err("An APBS calculation is already running.".into()));
                    continue;
                }
                let (sender, receiver) = mpsc::sync_channel(1);
                self.start_mcp_apbs(cuemol_command::transport::ApiRequest { endpoint: cuemol_command::transport::ApiEndpoint::Mcp, message: json!({"jsonrpc":"2.0","id":0,"method":"tools/call","params":{"name":name,"arguments":arguments}}), reply:sender });
                if let Some(job) = &mut self.apbs_job {
                    job.command = None;
                    job.agent_reply = Some(reply);
                    job.agent_cancel = Some(self.agent.operation_cancel());
                    self.agent.revision = self.snapshot_revision;
                } else {
                    let error = receiver
                        .try_recv()
                        .ok()
                        .flatten()
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| self.status.clone());
                    let _ = reply.try_send(Err(error));
                }
                continue;
            }
            let changes_scene = cuemol_command::tool_changes_scene(&name);
            let changes_view = cuemol_command::tool_is_mutating(&name);
            if changes_view {
                if let Err(error) = self.begin_remote_edit() {
                    let _ = reply.try_send(Err(error));
                    continue;
                }
            }
            let result = if let Some(download) = download {
                download.and_then(|data| {
                    cuemol_command::load_fetched(
                        &mut self.desktop,
                        &mut self.camera,
                        &data.plan,
                        &data.bytes,
                    )
                    .map_err(|error| error.to_string())
                })
            } else {
                cuemol_command::call_tool(&mut self.desktop, &mut self.camera, &name, &arguments)
                    .map_err(|error| error.to_string())
            };
            if result.is_ok() && changes_view {
                if name == "set_mol_selection" {
                    self.selection = arguments["selection"].as_str().unwrap_or("").into();
                } else if name == "execute_script" {
                    if let Some(selection) = result
                        .as_ref()
                        .ok()
                        .and_then(|value| value["selection"].as_str())
                    {
                        self.selection = selection.into();
                    }
                    self.restore_script_view();
                }
                let _ = self.desktop.set_active_view_camera(&self.camera);
                self.sync_active_scene_properties();
                self.invalidate_snapshot(false);
                if changes_scene {
                    self.record_agent_history(&name);
                }
                self.agent.revision = self.snapshot_revision;
            }
            let _ = reply.try_send(result);
        }
        if !self.agent.running() {
            self.agent_history = None;
        }
        (
            before != self.agent.change_id,
            revision != self.snapshot_revision,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    const PDB: &[u8] = b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nATOM      2  CB  ALA A   1       2.000   3.000   4.000  1.00 10.00           C  \nEND\n";
    fn tool(name: &str, arguments: Value, id: &str) -> Value {
        json!({"type":"function_call","call_id":id,"name":name,"arguments":arguments.to_string()})
    }
    fn mock(outputs: Vec<Value>) -> (String, std::thread::JoinHandle<()>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let worker = std::thread::spawn(move || {
            for output in outputs {
                let request = server
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .expect("provider request");
                let event = json!({"type":"response.completed","response":{"status":"completed","output":output}});
                request
                    .respond(
                        tiny_http::Response::from_string(format!("data: {event}\n\n")).with_header(
                            tiny_http::Header::from_bytes("Content-Type", "text/event-stream")
                                .unwrap(),
                        ),
                    )
                    .unwrap();
            }
        });
        (endpoint, worker)
    }
    fn model() -> (AppModel, Uid) {
        let mut model = super::super::tests::model_without_gpu();
        model.desktop.set_plugin_enabled("agent", true).unwrap();
        model.desktop.open_bytes("agent.pdb", PDB).unwrap();
        let object = model.desktop.active_object_summaries()[0].uid;
        (model, object)
    }
    fn start(model: &mut AppModel, endpoint: String) {
        model.agent_request("configure", json!({"settings":{"model":"openai:test-model","reasoning":"default","endpoint":endpoint}})).unwrap();
        model
            .agent_request("key", json!({"key":"dummy-agent-key"}))
            .unwrap();
        let status = model
            .agent_request("start", json!({"prompt":"Update the molecule"}))
            .unwrap();
        assert_eq!(status["agent"]["running"], true, "{status}");
        assert!(!status.to_string().contains("dummy-agent-key"));
    }
    fn finish(model: &mut AppModel) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while model.agent.running() || model.apbs_job.is_some() {
            assert!(Instant::now() < deadline, "{}", model.agent.status());
            model.poll_agent();
            if model.apbs_job.is_some() {
                handle_model_request(model, ModelRequest::PollApbsCalculation).unwrap();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn agent_settings_migrate_only_without_an_explicit_plugin_preference() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("agent.json"),
            r#"{"model":"anthropic:test-model","enabled":true}"#,
        )
        .unwrap();
        let mut model = super::super::tests::model_without_gpu();
        model.config_path = Some(directory.path().join("config.tsv"));
        model.desktop.set_plugin_enabled("agent", false).unwrap();
        model.load_agent_settings();
        assert_eq!(model.agent.settings.model, "anthropic:test-model");
        assert!(!model.agent.settings.enabled);
        let mut model = super::super::tests::model_without_gpu();
        model.config_path = Some(directory.path().join("config.tsv"));
        model.load_agent_settings();
        assert!(model.desktop.plugin_enabled("agent"));
        assert!(model.agent.settings.enabled);
    }

    #[test]
    fn agent_turn_groups_edits_and_keeps_keys_out_of_saved_settings() {
        let (mut model, object) = model();
        let temp = tempfile::tempdir().unwrap();
        model.config_path = Some(temp.path().join("config.tsv"));
        let original = model
            .desktop
            .scene_manager()
            .object(object)
            .unwrap()
            .name()
            .to_owned();
        let renderers = model.desktop.renderer_summaries(object).len();
        let (endpoint, worker) = mock(vec![
            json!([
                tool(
                    "set_node_prop",
                    json!({"nodeType":"object","nodeId":object.get(),"prop":"name","value":"Agent edit"}),
                    "rename"
                ),
                tool(
                    "create_renderer",
                    json!({"objId":object.get(),"rendererType":"cpk"}),
                    "renderer"
                ),
                tool("capture_view", json!({"longSide":256}), "image")
            ]),
            json!([]),
        ]);
        start(&mut model, endpoint);
        finish(&mut model);
        worker.join().unwrap();
        assert_eq!(
            model.desktop.scene_manager().object(object).unwrap().name(),
            "Agent edit"
        );
        assert_eq!(
            model.desktop.renderer_summaries(object).len(),
            renderers + 1
        );
        assert_eq!(model.undo_stack.len(), 1);
        assert!(model.agent.snapshot()["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["image"]
                .as_str()
                .is_some_and(|image| image.starts_with("iVBOR"))));
        assert!(!fs::read_to_string(temp.path().join("agent.json"))
            .unwrap()
            .contains("dummy-agent-key"));
        model.undo(0).unwrap();
        assert_eq!(
            model.desktop.scene_manager().object(object).unwrap().name(),
            original
        );
        assert_eq!(model.desktop.renderer_summaries(object).len(), renderers);
        let status = model.agent_request("clear", json!({})).unwrap();
        assert!(status["agent"]["messages"].as_array().unwrap().is_empty());
    }

    #[test]
    fn agent_cancel_manual_edit_and_disable_reject_pending_downloads() {
        for action in ["cancel", "edit", "disable"] {
            let (mut model, object) = model();
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/late.pdb", server.server_addr());
            let (received, downloading) = mpsc::channel();
            let (release, wait) = mpsc::channel();
            let download = std::thread::spawn(move || {
                let request = server
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .unwrap();
                received.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                let _ = request.respond(tiny_http::Response::from_data(PDB));
            });
            let (endpoint, worker) = mock(vec![json!([
                tool(
                    "set_node_prop",
                    json!({"nodeType":"object","nodeId":object.get(),"prop":"name","value":"Completed edit"}),
                    "rename"
                ),
                tool("enable_toolsets", json!({"toolsets":["files"]}), "enable"),
                tool("fetch_url", json!({"url":url,"reader":"pdb"}), "download")
            ])]);
            start(&mut model, endpoint);
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                model.poll_agent();
                if downloading.try_recv().is_ok() {
                    break;
                }
                assert!(Instant::now() < deadline, "{}", model.agent.status());
                std::thread::sleep(Duration::from_millis(5));
            }
            match action {
                "cancel" => {
                    model.agent_request("cancel", json!({})).unwrap();
                }
                "edit" => {
                    model.inspector_request("set_node_prop", &json!({"nodeType":"object","nodeId":object.get(),"prop":"name","value":"Manual edit"})).unwrap();
                    model.poll_agent();
                }
                _ => {
                    model.desktop.set_plugin_enabled("agent", false).unwrap();
                    model.poll_agent();
                }
            }
            assert!(!model.agent.running(), "{action}");
            release.send(()).unwrap();
            download.join().unwrap();
            worker.join().unwrap();
            model.poll_agent();
            assert_eq!(model.desktop.active_object_summaries().len(), 1);
            assert_eq!(
                model.desktop.scene_manager().object(object).unwrap().name(),
                if action == "edit" {
                    "Manual edit"
                } else {
                    "Completed edit"
                }
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn agent_apbs_uses_background_worker_and_the_turns_undo() {
        use std::os::unix::fs::PermissionsExt;
        let (mut model, object) = model();
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("apbs");
        fs::write(&script, "#!/bin/sh\nbase=$(awk '/write pot dx/{print $4}' \"$1\")\nsleep 0.1\nprintf 'object 1 class gridpositions counts 1 1 2\\norigin 0 0 0\\ndelta 1 0 0\\ndelta 0 1 0\\ndelta 0 0 1\\nobject 2 class gridconnections counts 1 1 2\\nobject 3 class array type double rank 0 items 2 data follows\\n-1 1\\n' > \"${base}.dx\"\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        model
            .desktop
            .put_config_value("cuemol2.ui.apbs-exe-path", script.to_string_lossy());
        let renderers = model.desktop.renderer_summaries(object).len();
        let (endpoint, worker) = mock(vec![
            json!([
                tool("enable_toolsets", json!({"toolsets":["molops"]}), "enable"),
                tool(
                    "create_renderer",
                    json!({"objId":object.get(),"rendererType":"cpk"}),
                    "renderer"
                ),
                tool(
                    "calc_elepot",
                    json!({"objId":object.get(),"method":"internal","name":"Agent potential"}),
                    "apbs"
                )
            ]),
            json!([]),
        ]);
        start(&mut model, endpoint);
        let deadline = Instant::now() + Duration::from_secs(5);
        while model.apbs_job.is_none() {
            model.poll_agent();
            assert!(model.agent.running());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(model.apbs_job.as_ref().unwrap().agent_reply.is_some());
        finish(&mut model);
        worker.join().unwrap();
        assert_eq!(
            model.desktop.active_density_map_summaries().len(),
            1,
            "{}",
            model.agent.status()
        );
        assert_eq!(model.undo_stack.len(), 1);
        model.undo(0).unwrap();
        assert!(model.desktop.active_density_map_summaries().is_empty());
        assert_eq!(model.desktop.renderer_summaries(object).len(), renderers);
        let (endpoint, worker) = mock(vec![json!([
            tool("enable_toolsets", json!({"toolsets":["molops"]}), "enable"),
            tool(
                "calc_elepot",
                json!({"objId":object.get(),"method":"internal","name":"Cancelled potential"}),
                "apbs"
            )
        ])]);
        start(&mut model, endpoint);
        let deadline = Instant::now() + Duration::from_secs(5);
        while model.apbs_job.is_none() {
            model.poll_agent();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        model.agent_request("cancel", json!({})).unwrap();
        worker.join().unwrap();
        let (endpoint, worker) = mock(vec![json!([])]);
        start(&mut model, endpoint);
        finish(&mut model);
        worker.join().unwrap();
        assert!(model.desktop.active_density_map_summaries().is_empty());
        assert!(model.undo_stack.is_empty());
    }
}
