use std::sync::mpsc::{self, Receiver};

#[derive(Debug, Default)]
pub(super) struct Updates {
    visible: bool,
    status: String,
    release: Option<cuemol_command::update::Release>,
    checking: Option<Receiver<Result<Option<cuemol_command::update::Release>, String>>>,
    installing: Option<Receiver<Result<(), String>>>,
}
impl Updates {
    pub fn check(&mut self, ctx: &egui::Context) {
        self.visible = true;
        if self.checking.is_some() || self.installing.is_some() {
            return;
        }
        self.status = "Checking for updates…".into();
        self.release = None;
        let (sender, receiver) = mpsc::channel();
        self.checking = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(cuemol_command::update::check());
            ctx.request_repaint();
        });
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if let Some(result) = self
            .checking
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.checking = None;
            self.status = match result {
                Ok(Some(release)) => {
                    let text = if release.newer {
                        format!("CueMol {} is available.", release.version)
                    } else {
                        "You are using the latest version.".into()
                    };
                    self.release = Some(release);
                    text
                }
                Ok(None) => "No published release is available yet.".into(),
                Err(error) => error,
            };
        }
        if let Some(result) = self
            .installing
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.installing = None;
            self.status = result
                .map(|()| "Update installed. Restart CueMol to use it.".into())
                .unwrap_or_else(|error| error);
            self.release = None;
        }
        if !self.visible {
            return;
        }
        let mut open = self.visible;
        egui::Window::new("CueMol updates").open(&mut open).resizable(false).show(ctx,|ui| {
            ui.label(format!("Installed: {}",cuemol_command::update::current_version()));
            ui.label(&self.status);
            if self.checking.is_some() || self.installing.is_some() {ui.spinner();}
            if let Some(release)=&self.release {
                ui.hyperlink_to("Release notes",&release.url);
                if release.newer && ui.add_enabled(self.installing.is_none(),egui::Button::new("Install update")).clicked() {
                    let version=release.version.clone();
                    let (sender,receiver)=mpsc::channel();self.installing=Some(receiver);
                    self.status="Downloading and installing the verified release…".into();
                    let ctx=ctx.clone();
                    std::thread::spawn(move || {
                        let result=(|| {
                            let exe=std::env::current_exe().map_err(|e|e.to_string())?;
                            if !matches!(exe.file_stem().and_then(|s|s.to_str()),Some("cuemol")) {return Err("Run cuemol self-upgrade from the installed command to update this development build.".into());}
                            let output=std::process::Command::new(exe).arg("self-upgrade").env("CUEMOL_VERSION",version).output().map_err(|e|e.to_string())?;
                            if output.status.success() {Ok(())} else {Err(format!("Update failed: {}",String::from_utf8_lossy(&output.stderr)))}
                        })();
                        let _=sender.send(result);ctx.request_repaint();
                    });
                }
            }
        });
        self.visible = open;
    }
}
