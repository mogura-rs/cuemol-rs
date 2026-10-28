use cuemol_agent::{DownloadJob, Downloaded, Event, History, Job, ModelJob, Settings, ToolReply};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use egui::{Context, TextureHandle};
use zeroize::Zeroize;

struct Message {
    role: String,
    text: String,
    image: Option<TextureHandle>,
}

pub struct AgentCall {
    pub name: String,
    pub arguments: serde_json::Value,
    pub reply: ToolReply,
    pub download: Option<Result<Downloaded, String>>,
}
struct PendingDownload {
    name: String,
    arguments: serde_json::Value,
    reply: ToolReply,
    job: DownloadJob,
}

pub struct AgentPanel {
    pub visible: bool,
    pub docked: bool,
    pub settings: Settings,
    pub revision: u64,
    pub changed_scene: bool,
    initial_camera: Camera,
    operation_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    job: Option<Job>,
    download: Option<PendingDownload>,
    model_job: Option<ModelJob>,
    models: Vec<String>,
    history: History,
    messages: Vec<Message>,
    draft: String,
    key: zeroize::Zeroizing<String>,
    status: String,
    settings_dirty: bool,
}
impl std::fmt::Debug for AgentPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPanel")
            .field("visible", &self.visible)
            .field("running", &self.running())
            .finish_non_exhaustive()
    }
}
impl Default for AgentPanel {
    fn default() -> Self {
        Self {
            visible: false,
            docked: false,
            settings: Settings::default(),
            revision: 0,
            changed_scene: false,
            initial_camera: Camera::new(),
            operation_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            job: None,
            download: None,
            model_job: None,
            models: Vec::new(),
            history: History::default(),
            messages: Vec::new(),
            draft: String::new(),
            key: cuemol_agent::secret(String::new()),
            status: String::new(),
            settings_dirty: false,
        }
    }
}
impl AgentPanel {
    pub fn set_session_key(&mut self, key: String) {
        self.key = cuemol_agent::secret(key);
    }
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"running":self.running(),"message":self.status,"messages":self.messages.iter().map(|m|serde_json::json!({"role":m.role,"text":m.text,"image":m.image.is_some()})).collect::<Vec<_>>()})
    }
    pub fn initial_camera(&self) -> &Camera {
        &self.initial_camera
    }
    pub fn operation_cancel(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.operation_cancel.clone()
    }
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    pub fn cancel(&mut self, reason: &str) {
        self.operation_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.download = None;
        if let Some(job) = self.job.take() {
            job.cancel();
            self.status = reason.into();
        }
    }
    pub fn settings_json(&self) -> String {
        serde_json::to_string_pretty(&self.settings).unwrap()
    }
    pub fn load_settings(&mut self, text: &str) {
        if let Ok(settings) = serde_json::from_str(text) {
            self.settings = settings;
        }
    }
    pub fn take_settings_changed(&mut self) -> bool {
        std::mem::take(&mut self.settings_dirty)
    }
    pub fn start(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        prompt: String,
        ctx: &Context,
        revision: u64,
    ) {
        if self.running() {
            return;
        }
        let key = if !self.key.is_empty() {
            self.key.clone()
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            match self
                .settings
                .model_spec()
                .and_then(|(provider, _)| cuemol_agent::credentials::load(provider))
            {
                Ok(key) => key,
                Err(error) => {
                    self.status = error;
                    return;
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.status = "Enter the provider API key in Agent settings.".into();
                return;
            }
        };
        if let Err(error) = self.settings.model_spec() {
            self.status = error;
            return;
        }
        if !self.settings.enabled {
            self.status = "Enable AI Agent first.".into();
            return;
        }
        let snapshot = match cuemol_command::call_tool(
            state,
            camera,
            "get_scene_state",
            &serde_json::json!({}),
        ) {
            Ok(value) => value,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        self.messages.push(Message {
            role: "You".into(),
            text: prompt.clone(),
            image: None,
        });
        self.messages.push(Message {
            role: "Assistant".into(),
            text: String::new(),
            image: None,
        });
        self.status = "Thinking…".into();
        self.changed_scene = false;
        self.initial_camera = camera.clone();
        self.operation_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.revision = revision;
        let ctx = ctx.clone();
        self.job = Some(cuemol_agent::start(
            snapshot,
            self.settings.clone(),
            self.history.clone(),
            key,
            prompt,
            Box::new(move || ctx.request_repaint()),
        ));
    }
    pub fn poll(&mut self, ctx: &Context) -> Option<AgentCall> {
        if let Some(result) = self.download.as_ref().and_then(|d| d.job.poll()) {
            let pending = self.download.take().unwrap();
            return Some(AgentCall {
                name: pending.name,
                arguments: pending.arguments,
                reply: pending.reply,
                download: Some(result),
            });
        }
        while let Some(event) = self.job.as_ref().and_then(Job::poll) {
            match event {
                Event::Call {
                    mut name,
                    mut arguments,
                    reply,
                } => {
                    if name == "export_image"
                        && cuemol_command::validate_args(&name, &arguments).is_ok()
                    {
                        name = "render_image".into();
                        let map = arguments.as_object_mut().unwrap();
                        let path = map.remove("path").unwrap();
                        map.insert("fileName".into(), path);
                    }
                    if matches!(name.as_str(), "fetch_pdb" | "fetch_url" | "fetch_map") {
                        let ctx = ctx.clone();
                        let job = DownloadJob::start(
                            name.clone(),
                            arguments.clone(),
                            Box::new(move || ctx.request_repaint()),
                        );
                        self.download = Some(PendingDownload {
                            name,
                            arguments,
                            reply,
                            job,
                        });
                    } else {
                        return Some(AgentCall {
                            name,
                            arguments,
                            reply,
                            download: None,
                        });
                    }
                }
                Event::Text(text) => {
                    if self.messages.last().is_none_or(|m| m.role != "Assistant") {
                        self.messages.push(Message {
                            role: "Assistant".into(),
                            text: String::new(),
                            image: None,
                        });
                    }
                    self.messages.last_mut().unwrap().text.push_str(&text);
                    self.status = "Writing…".into();
                }
                Event::ToolStarted { name, arguments } => {
                    self.status = format!("Running {name}…");
                    self.messages.push(Message {
                        role: "Tool".into(),
                        text: format!("{name} {arguments}"),
                        image: None,
                    });
                }
                Event::ToolResult {
                    name,
                    summary,
                    image,
                    ok,
                } => {
                    let texture = image.and_then(|data| {
                        use base64::Engine;
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .ok()?;
                        let rgba = image::load_from_memory(&bytes).ok()?.into_rgba8();
                        Some(ctx.load_texture(
                            format!("agent-image-{}", self.messages.len()),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [rgba.width() as usize, rgba.height() as usize],
                                &rgba,
                            ),
                            Default::default(),
                        ))
                    });
                    self.messages.push(Message {
                        role: if ok { "Result" } else { "Error" }.into(),
                        text: format!("{name}: {summary}"),
                        image: texture,
                    });
                    self.status = "Thinking…".into();
                }
                Event::Finished(result) => {
                    self.job = None;
                    match result {
                        Ok(outcome) => {
                            self.history = outcome.history;
                            self.status = format!(
                                "{} tool calls · {} input / {} output tokens{}",
                                outcome.tool_calls,
                                outcome.usage.input,
                                outcome.usage.output,
                                if outcome.round_limit {
                                    " · Round limit reached; send another message to continue."
                                } else {
                                    ""
                                }
                            );
                        }
                        Err(error) => self.status = error,
                    }
                }
            }
        }
        None
    }
    pub fn show(&mut self, ctx: &Context) -> Option<String> {
        if !self.visible || self.docked {
            return None;
        }
        let mut open = self.visible;
        let mut prompt = None;
        egui::Window::new("AI Agent")
            .open(&mut open)
            .default_size([440., 560.])
            .show(ctx, |ui| prompt = self.show_docked(ui));
        self.visible = open;
        prompt
    }
    pub fn show_docked(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let ctx = ui.ctx().clone();
        if let Some(result) = self.model_job.as_ref().and_then(ModelJob::poll) {
            self.model_job = None;
            match result {
                Ok(models) => {
                    self.status = format!("{} models available", models.len());
                    self.models = models;
                }
                Err(error) => self.status = error,
            }
        }
        let before = self.settings_json();
        let previous_provider = self
            .settings
            .model_spec()
            .ok()
            .map(|(provider, _)| provider);
        ui.collapsing("Agent settings", |ui| {
            ui.checkbox(&mut self.settings.enabled, "Enable AI Agent");
            ui.label(
                "Messages and scene data are sent to the selected provider when you press Send.",
            );
            ui.add_enabled_ui(!self.running(), |ui| {
                ui.label("Model (provider:model)");
                ui.text_edit_singleline(&mut self.settings.model);
                if ui
                    .add_enabled(
                        self.model_job.is_none(),
                        egui::Button::new("Refresh available models"),
                    )
                    .clicked()
                {
                    let key = if !self.key.is_empty() {
                        Ok(self.key.clone())
                    } else {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            self.settings
                                .model_spec()
                                .and_then(|(p, _)| cuemol_agent::credentials::load(p))
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            Err("Enter the provider API key first.".to_string())
                        }
                    };
                    match key {
                        Ok(key) => {
                            let ctx = ctx.clone();
                            self.model_job = Some(ModelJob::start(
                                self.settings.clone(),
                                key,
                                Box::new(move || ctx.request_repaint()),
                            ));
                        }
                        Err(error) => self.status = error,
                    }
                }
                if !self.models.is_empty() {
                    egui::ComboBox::from_label("Available models")
                        .selected_text(&self.settings.model)
                        .show_ui(ui, |ui| {
                            for model in &self.models {
                                ui.selectable_value(&mut self.settings.model, model.clone(), model);
                            }
                        });
                }
                egui::ComboBox::from_label("Reasoning")
                    .selected_text(&self.settings.reasoning)
                    .show_ui(ui, |ui| {
                        for value in ["default", "low", "medium", "high"] {
                            ui.selectable_value(&mut self.settings.reasoning, value.into(), value);
                        }
                    });
                ui.checkbox(
                    &mut self.settings.enter_sends,
                    "Enter sends; Shift+Enter adds a line",
                );
                ui.label("API key");
                ui.add(egui::TextEdit::singleline(&mut *self.key).password(true));
                #[cfg(not(target_arch = "wasm32"))]
                ui.horizontal(|ui| {
                    if ui.button("Save in OS keychain").clicked() {
                        match self
                            .settings
                            .model_spec()
                            .and_then(|(p, _)| cuemol_agent::credentials::save(p, &self.key))
                        {
                            Ok(()) => {
                                self.key.zeroize();
                                self.status = "API key saved in the OS keychain.".into();
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    if ui.button("Delete stored key").clicked() {
                        self.status = self
                            .settings
                            .model_spec()
                            .and_then(|(p, _)| cuemol_agent::credentials::delete(p))
                            .map(|()| "Stored key deleted.".into())
                            .unwrap_or_else(|e| e);
                    }
                });
                #[cfg(target_arch = "wasm32")]
                ui.label("The key is kept only in this tab's memory.");
                ui.collapsing("Custom API endpoint", |ui| {
                    ui.text_edit_singleline(&mut self.settings.endpoint);
                });
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(330.)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for message in &self.messages {
                    ui.strong(&message.role);
                    ui.add(egui::Label::new(&message.text).wrap().selectable(true));
                    if let Some(image) = &message.image {
                        ui.add(
                            egui::Image::new((image.id(), image.size_vec2()))
                                .max_size(egui::vec2(360., 270.)),
                        );
                    }
                }
            });
        let composer_id = ui.make_persistent_id("agent-composer");
        let enter = self.settings.enter_sends
            && ui.memory(|m| m.has_focus(composer_id))
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let response = ui.add(
            egui::TextEdit::multiline(&mut self.draft)
                .id(composer_id)
                .hint_text("Describe the scene change…")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        let mut send = enter && response.has_focus();
        ui.horizontal(|ui| {
            send |= ui
                .add_enabled(
                    !self.running() && self.settings.enabled && !self.draft.trim().is_empty(),
                    egui::Button::new("Send"),
                )
                .clicked();
            if ui
                .add_enabled(self.running(), egui::Button::new("Cancel"))
                .clicked()
            {
                self.cancel("Cancelled; completed edits are kept.");
            }
            if ui
                .add_enabled(!self.running(), egui::Button::new("New conversation"))
                .clicked()
            {
                self.history = History::default();
                self.messages.clear();
                self.status.clear();
            }
        });
        ui.label(&self.status);
        if previous_provider
            != self
                .settings
                .model_spec()
                .ok()
                .map(|(provider, _)| provider)
        {
            self.key.zeroize();
            self.models.clear();
            self.model_job = None;
        }
        self.settings_dirty |= before != self.settings_json();
        if !self.settings.enabled && self.running() {
            self.cancel("AI Agent disabled.");
        }
        (send && !self.running() && self.settings.enabled && !self.draft.trim().is_empty())
            .then(|| std::mem::take(&mut self.draft))
    }
}
