use super::*;
use cuemol_command::transport::{reject_console, ApiRequest, ConsoleReply};

impl GuiApplication {
    pub(super) fn start_remote_console(&mut self, request: ApiRequest, ctx: &egui::Context) {
        let parsed = (|| {
            if !self.state.console_remote_enabled() {
                return Err("Command line access is disabled.".into());
            }
            if self.console_run.is_some() || self.remote_console.is_some() {
                return Err("The console is already running.".into());
            }
            let source = request.message["source"]
                .as_str()
                .filter(|source| !source.trim().is_empty())
                .ok_or("A console source is required.")?
                .to_owned();
            let dialect = cuemol_command::transport::console_dialect(&request.message)?;
            if !self.state.console_dialect_enabled(dialect.label()) {
                return Err("Console dialect is disabled.".into());
            }
            if let Some(directory) = request.message["cwd"].as_str() {
                self.console_session
                    .set_working_directory(Path::new(directory))?;
            }
            Ok::<_, String>((source, dialect))
        })();
        let (source, dialect) = match parsed {
            Ok(value) => value,
            Err(error) => {
                reject_console(request, error);
                return;
            }
        };
        self.agent.cancel("Command line submission started.");
        let pending = ConsoleReply::new(request, &self.log_lines);
        let previous_source = std::mem::replace(&mut self.console.source, source);
        let previous_dialect = std::mem::replace(&mut self.console.dialect, dialect);
        self.execute_console_input();
        self.console.source = previous_source;
        self.console.dialect = previous_dialect;
        self.remote_console = Some(pending);
        self.poll_console(ctx);
    }
    pub(super) fn poll_console(&mut self, ctx: &egui::Context) {
        if self.remote_console.is_some() && !self.state.console_remote_enabled() {
            self.cancel_console("Command line access disabled.");
            return;
        }
        self.advance_console(ctx);
        if let Some(pending) = &mut self.remote_console {
            pending.collect(&self.log_lines);
        }
        if self.console_run.is_none() {
            if let Some(pending) = self.remote_console.take() {
                pending.finish(
                    &self.log_lines,
                    matches!(self.status.as_str(), "Console executed" | "Script executed"),
                    &self.status,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(
        source: &str,
    ) -> (
        ApiRequest,
        std::sync::mpsc::Receiver<Option<serde_json::Value>>,
    ) {
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        (
            ApiRequest {
                endpoint: cuemol_command::transport::ApiEndpoint::Console,
                message: serde_json::json!({"source":source,"dialect":"cuemol"}),
                reply,
            },
            receiver,
        )
    }
    #[test]
    fn remote_console_checks_access_preserves_drafts_and_groups_undo() {
        let mut app = GuiApplication::new();
        let ctx = egui::Context::default();
        app.config_path.clear();
        app.state.open_bytes("probe.pdb",b"ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let uid = app.state.active_object_summaries()[0].uid;
        app.state
            .scene_manager_mut()
            .object_mut(uid)
            .unwrap()
            .set_name("probe");
        let (req, rx) = request("rename probe, changed");
        app.start_remote_console(req, &ctx);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], false);
        assert_eq!(
            app.state.scene_manager().object(uid).unwrap().name(),
            "probe"
        );
        app.state.put_config_value("console:remote-access", "true");
        app.console.source = "unfinished local draft".into();
        let (req, rx) = request("hide probe; rename probe, changed");
        app.start_remote_console(req, &ctx);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], true);
        assert_eq!(app.console.source, "unfinished local draft");
        assert_eq!(app.undo_stack.len(), 1);
        let (req, rx) = request("undo");
        app.start_remote_console(req, &ctx);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], true);
        assert_eq!(
            app.state.scene_manager().object(uid).unwrap().name(),
            "probe"
        );
        assert!(app.state.scene_manager().object(uid).unwrap().is_visible());
        let source = std::iter::repeat_n("pwd", 200)
            .chain(["rename probe, late"])
            .collect::<Vec<_>>()
            .join(";");
        let (req, rx) = request(&source);
        app.start_remote_console(req, &ctx);
        assert!(rx.try_recv().is_err());
        assert!(app.console_run.is_some());
        app.state.put_config_value("console:remote-access", "false");
        app.poll_console(&ctx);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], false);
        assert!(app.console_run.is_none());
        assert_eq!(
            app.state.scene_manager().object(uid).unwrap().name(),
            "probe"
        );
    }
}
