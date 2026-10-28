//! Optional conversational control over the shared scene commands.
mod catalog;
mod models;
mod provider;
mod stream;
pub use models::{list_models, ModelJob};
#[cfg(not(target_arch = "wasm32"))]
pub mod credentials;

use async_channel::{Receiver, Sender};
use futures_util::future::{AbortHandle, Abortable};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

pub use provider::Provider;
pub type ToolReply = async_channel::Sender<Result<Value, String>>;
mod download;
pub use download::{DownloadJob, Downloaded};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub model: String,
    pub reasoning: String,
    pub enabled: bool,
    pub enter_sends: bool,
    pub endpoint: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "openai:gpt-5.6".into(),
            reasoning: "low".into(),
            enabled: false,
            enter_sends: false,
            endpoint: String::new(),
        }
    }
}
impl Settings {
    pub fn model_spec(&self) -> Result<(Provider, &str), String> {
        if !matches!(
            self.reasoning.as_str(),
            "default" | "low" | "medium" | "high"
        ) {
            return Err("Reasoning must be default, low, medium or high.".into());
        }
        let (provider, model) = self
            .model
            .split_once(':')
            .unwrap_or(("openai", &self.model));
        if model.trim().is_empty() || model.contains(['/', '?', '#']) {
            return Err("Enter a valid model ID.".into());
        }
        Ok((provider.parse()?, model.trim()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct History {
    enabled: std::collections::BTreeSet<String>,
    pub(crate) provider: Option<Provider>,
    pub(crate) items: Vec<Value>,
    pub(crate) text: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}

#[derive(Debug)]
pub struct Outcome {
    pub history: History,
    pub usage: Usage,
    pub tool_calls: usize,
    pub round_limit: bool,
}

#[derive(Debug)]
pub enum Event {
    Text(String),
    ToolStarted {
        name: String,
        arguments: Value,
    },
    ToolResult {
        name: String,
        summary: String,
        image: Option<String>,
        ok: bool,
    },
    Call {
        name: String,
        arguments: Value,
        reply: Sender<Result<Value, String>>,
    },
    Finished(Result<Outcome, String>),
}

#[derive(Debug)]
pub struct Job {
    receiver: Receiver<Event>,
    abort: AbortHandle,
}
impl Job {
    pub fn cancel(&self) {
        self.abort.abort();
    }
    pub fn poll(&self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn receive(&self) -> Result<Event, String> {
        self.receiver
            .recv_blocking()
            .map_err(|_| "Agent stopped".into())
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(not(target_arch = "wasm32"))]
type Wake = Box<dyn Fn() + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type Wake = Box<dyn Fn()>;

struct Events {
    sender: Sender<Event>,
    wake: Wake,
}
impl Events {
    async fn send(&self, event: Event) -> Result<(), String> {
        self.sender
            .send(event)
            .await
            .map_err(|_| "Agent panel closed".to_string())?;
        (self.wake)();
        Ok(())
    }
}

pub fn start(
    scene: Value,
    settings: Settings,
    history: History,
    key: Zeroizing<String>,
    prompt: String,
    wake: Wake,
) -> Job {
    let (sender, receiver) = async_channel::bounded(16);
    let (abort, registration) = AbortHandle::new_pair();
    let work = async move {
        let events = Events { sender, wake };
        let result = Abortable::new(
            run(scene, settings, history, key, prompt, &events),
            registration,
        )
        .await;
        let result = result.unwrap_or_else(|_| Err("Cancelled".into()));
        let _ = events.send(Event::Finished(result)).await;
    };
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("agent runtime")
            .block_on(work);
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(work);
    Job { receiver, abort }
}

async fn run(
    snapshot: Value,
    settings: Settings,
    mut history: History,
    key: Zeroizing<String>,
    prompt: String,
    events: &Events,
) -> Result<Outcome, String> {
    if !settings.enabled {
        return Err("Enable AI Agent in its settings before sending a message.".into());
    }
    if key.trim().is_empty() {
        return Err("Set the provider API key before sending a message.".into());
    }
    let (provider, model) = settings.model_spec()?;
    if history.provider != Some(provider) {
        history.items.clear();
        for (user, assistant) in &history.text {
            history.items.push(provider.message("user", user));
            history.items.push(provider.message("assistant", assistant));
        }
        history.provider = Some(provider);
    }
    history.items.push(provider.message(
        "user",
        &format!("<scene_state>{snapshot}</scene_state>\n\n{prompt}"),
    ));
    let client = stream::client()?;
    let mut usage = Usage::default();
    let mut tool_calls = 0;
    let mut final_text = String::new();
    for round_index in 0..16 {
        let response = stream::request(
            &client,
            provider,
            model,
            &settings,
            &key,
            &history.items,
            &history.enabled,
            events,
        )
        .await?;
        usage.input += response.usage.input;
        usage.output += response.usage.output;
        usage.cached += response.usage.cached;
        final_text.push_str(&response.text);
        let calls = response.calls;
        history.items.extend(response.items);
        if calls.is_empty() {
            history.text.push((prompt, final_text));
            return Ok(Outcome {
                history,
                usage,
                tool_calls,
                round_limit: false,
            });
        }
        for call in calls {
            tool_calls += 1;
            events
                .send(Event::ToolStarted {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                })
                .await?;
            let result = if call.name == "enable_toolsets" {
                catalog::enable(&call.arguments, &mut history.enabled)
            } else if !catalog::available(&call.name, &history.enabled) {
                Err(format!(
                    "Unknown tool or disabled toolset: {}. Use enable_toolsets first.",
                    call.name
                ))
            } else {
                let (reply, receiver) = async_channel::bounded(1);
                events
                    .send(Event::Call {
                        name: call.name.clone(),
                        arguments: call.arguments,
                        reply,
                    })
                    .await?;
                receiver
                    .recv()
                    .await
                    .map_err(|_| "Tool execution was cancelled".to_string())?
            };
            let ok = result.is_ok();
            let mut value = result.unwrap_or_else(|error| json!({"error":error}));
            let image = value
                .as_object_mut()
                .and_then(|v| v.remove("_image"))
                .and_then(|v| v.as_str().map(str::to_string));
            let summary: String = value.to_string().chars().take(300).collect();
            events
                .send(Event::ToolResult {
                    name: call.name.clone(),
                    summary,
                    image: image.clone(),
                    ok,
                })
                .await?;
            history
                .items
                .extend(provider.tool_result(&call.id, &call.name, value, image, ok));
        }
        if round_index == 15 {
            history.text.push((prompt, final_text));
            return Ok(Outcome {
                history,
                usage,
                tool_calls,
                round_limit: true,
            });
        }
    }
    unreachable!()
}

pub fn secret(value: String) -> Zeroizing<String> {
    Zeroizing::new(value)
}
