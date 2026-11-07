#[derive(Debug, Default)]
pub(super) struct Updates {
    runtime: cuemol_command::update::UpdateRuntime,
    visible: bool,
}
fn wake(ctx: &egui::Context) -> cuemol_command::update::Wake {
    let ctx = ctx.clone();
    std::sync::Arc::new(move || ctx.request_repaint())
}
impl Updates {
    pub fn is_open(&self) -> bool {
        self.visible
    }

    pub fn schedule_startup(&mut self) {
        self.runtime.schedule_startup();
    }
    pub fn check(&mut self, ctx: &egui::Context) {
        self.visible = true;
        self.runtime.acknowledge();
        self.runtime.check(false, wake(ctx));
    }
    pub fn show(&mut self, ctx: &egui::Context, startup_enabled: bool) {
        self.runtime.poll(startup_enabled, wake(ctx));
        if self.runtime.pending_startup() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let status = self.runtime.snapshot();
        if status.notify {
            self.visible = true;
            self.runtime.acknowledge();
        }
        if !self.visible {
            return;
        }
        let mut open = true;
        egui::Window::new("CueMol updates")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Installed: {}", status.current_version));
                ui.label(&status.message);
                if status.checking || status.installing {
                    ui.spinner();
                }
                if let Some(release) = &status.release {
                    ui.hyperlink_to("Release notes", &release.url);
                    if release.newer
                        && ui
                            .add_enabled(!status.installing, egui::Button::new("Install update"))
                            .clicked()
                    {
                        let _ = self.runtime.install(wake(ctx));
                    }
                }
            });
        self.visible = open;
    }
}
