use cuemol_agent::{DownloadJob, Downloaded, Event, History, Job, ModelJob, Settings, ToolReply};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
pub type Wake = Arc<dyn Fn() + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type Wake = Arc<dyn Fn()>;
use zeroize::Zeroize;

pub(super) struct Message {
    pub role: String,
    pub text: String,
    pub image: Option<String>,
}

pub(super) struct TranscriptEntry<'a> {
    pub role: &'a str,
    pub text: &'a str,
    pub image: Option<&'a str>,
    pub tool: Option<TranscriptTool<'a>>,
}
pub(super) struct TranscriptTool<'a> {
    pub name: &'a str,
    pub input: &'a str,
    pub result: Option<(&'a str, bool)>,
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

pub struct AgentSession {
    pub settings: Settings,
    pub revision: u64,
    pub change_id: u64,
    pub changed_scene: bool,
    initial_camera: Camera,
    operation_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    job: Option<Job>,
    download: Option<PendingDownload>,
    pub(super) model_job: Option<ModelJob>,
    pub(super) models: Vec<String>,
    pub(super) history: History,
    pub(super) messages: Vec<Message>,
    pub(super) key: zeroize::Zeroizing<String>,
    pub(super) status: String,
    pub(super) settings_dirty: bool,
}
impl std::fmt::Debug for AgentSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentSession")
            .field("running", &self.running())
            .finish_non_exhaustive()
    }
}
impl Default for AgentSession {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            revision: 0,
            change_id: 0,
            changed_scene: false,
            initial_camera: Camera::new(),
            operation_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            job: None,
            download: None,
            model_job: None,
            models: Vec::new(),
            history: History::default(),
            messages: Vec::new(),
            key: cuemol_agent::secret(String::new()),
            status: String::new(),
            settings_dirty: false,
        }
    }
}
impl AgentSession {
    fn tool_result(&self, index: usize) -> Option<(&Message, &str)> {
        let message = self.messages.get(index)?;
        if message.role != "Tool" {
            return None;
        }
        let name = message.text.split_once(' ')?.0;
        let next = self.messages.get(index + 1)?;
        if !matches!(next.role.as_str(), "Result" | "Error") {
            return None;
        }
        let result = next.text.strip_prefix(name)?.strip_prefix(": ")?;
        Some((next, result))
    }
    pub(super) fn transcript(&self) -> impl Iterator<Item = (usize, TranscriptEntry<'_>)> {
        self.messages
            .iter()
            .enumerate()
            .filter_map(|(index, message)| {
                if index > 0 && self.tool_result(index - 1).is_some() {
                    return None;
                }
                let paired = self.tool_result(index);
                let tool = (message.role == "Tool")
                    .then(|| message.text.split_once(' '))
                    .flatten()
                    .map(|(name, input)| TranscriptTool {
                        name,
                        input,
                        result: paired.map(|(result, text)| (text, result.role == "Result")),
                    });
                Some((
                    index,
                    TranscriptEntry {
                        role: &message.role,
                        text: &message.text,
                        image: paired
                            .and_then(|(message, _)| message.image.as_deref())
                            .or(message.image.as_deref()),
                        tool,
                    },
                ))
            })
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "running":self.running(), "message":self.status, "settings":self.settings,
            "models":self.models, "refreshingModels":self.model_job.is_some(),
            "messages":self.transcript().map(|(_,message)|serde_json::json!({
                "role":message.role,"text":message.text,"image":message.image,
                "tool":message.tool.map(|tool|serde_json::json!({"name":tool.name,"input":tool.input,"result":tool.result.map(|(text,_)|text),"ok":tool.result.map(|(_,ok)|ok)}))
            })).collect::<Vec<_>>()
        })
    }
    pub fn configure(&mut self, settings: Settings) -> Result<(), String> {
        if self.running() {
            return Err("Cancel the current turn before changing Agent settings.".into());
        }
        let provider = settings.model_spec()?.0;
        if self
            .settings
            .model_spec()
            .ok()
            .map(|(provider, _)| provider)
            != Some(provider)
        {
            self.key.zeroize();
            self.models.clear();
            self.model_job = None;
        }
        self.settings = settings;
        self.settings_dirty = true;
        Ok(())
    }
    fn provider_key(&self) -> Result<zeroize::Zeroizing<String>, String> {
        if !self.key.is_empty() {
            return Ok(self.key.clone());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.settings
                .model_spec()
                .and_then(|(provider, _)| cuemol_agent::credentials::load(provider))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err("Enter the provider API key in Agent settings.".into())
        }
    }
    pub fn refresh_models(&mut self, wake: Wake) -> Result<(), String> {
        if self.model_job.is_some() {
            return Ok(());
        }
        let key = self.provider_key()?;
        self.model_job = Some(ModelJob::start(
            self.settings.clone(),
            key,
            Box::new(move || wake()),
        ));
        Ok(())
    }
    pub fn poll_models(&mut self) {
        if let Some(result) = self.model_job.as_ref().and_then(ModelJob::poll) {
            self.model_job = None;
            self.change_id = self.change_id.wrapping_add(1);
            match result {
                Ok(models) => {
                    self.status = format!("{} models available", models.len());
                    self.models = models;
                }
                Err(error) => self.status = error,
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_key(&mut self) -> Result<(), String> {
        self.settings
            .model_spec()
            .and_then(|(provider, _)| cuemol_agent::credentials::save(provider, &self.key))?;
        self.key.zeroize();
        self.status = "API key saved in the OS keychain.".into();
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn delete_key(&mut self) -> Result<(), String> {
        self.settings
            .model_spec()
            .and_then(|(provider, _)| cuemol_agent::credentials::delete(provider))?;
        self.status = "Stored key deleted.".into();
        Ok(())
    }
    pub fn new_conversation(&mut self) -> Result<(), String> {
        if self.running() {
            return Err("Cancel the current turn first.".into());
        }
        self.history = History::default();
        self.messages.clear();
        self.status.clear();
        Ok(())
    }
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
            self.change_id = self.change_id.wrapping_add(1);
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
        wake: Wake,
        revision: u64,
    ) {
        if !state.plugin_enabled("agent") {
            self.status = "Enable AI Agent in Settings > Plugins first.".into();
            return;
        }
        if self.running() {
            return;
        }
        let key = match self.provider_key() {
            Ok(key) => key,
            Err(error) => {
                self.status = error;
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
        let _ = state.set_active_view_camera(camera);
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
        self.job = Some(cuemol_agent::start(
            snapshot,
            self.settings.clone(),
            self.history.clone(),
            key,
            prompt,
            Box::new(move || wake()),
        ));
    }
    pub fn poll(&mut self, wake: Wake) -> Option<AgentCall> {
        self.poll_models();
        if let Some(result) = self.download.as_ref().and_then(|d| d.job.poll()) {
            let pending = self.download.take().unwrap();
            self.change_id = self.change_id.wrapping_add(1);
            return Some(AgentCall {
                name: pending.name,
                arguments: pending.arguments,
                reply: pending.reply,
                download: Some(result),
            });
        }
        while let Some(event) = self.job.as_ref().and_then(Job::poll) {
            self.change_id = self.change_id.wrapping_add(1);
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
                    if matches!(
                        name.as_str(),
                        "fetch_pdb" | "fetch_url" | "fetch_map" | "fetch_emdb"
                    ) {
                        let wake = wake.clone();
                        let job = DownloadJob::start(
                            name.clone(),
                            arguments.clone(),
                            Box::new(move || wake()),
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
                    self.messages.push(Message {
                        role: if ok { "Result" } else { "Error" }.into(),
                        text: format!("{name}: {summary}"),
                        image,
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
}
