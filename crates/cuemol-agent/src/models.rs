use crate::{Provider, Settings, Wake};
use futures_util::{
    future::{select, AbortHandle, Abortable, Either},
    pin_mut, StreamExt,
};
use serde_json::Value;
use zeroize::Zeroizing;

#[derive(Debug)]
pub struct ModelJob {
    receiver: async_channel::Receiver<Result<Vec<String>, String>>,
    abort: AbortHandle,
}
impl ModelJob {
    pub fn start(settings: Settings, key: Zeroizing<String>, wake: Wake) -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        let (abort, registration) = AbortHandle::new_pair();
        let work = async move {
            let result = Abortable::new(list_models(&settings, &key), registration)
                .await
                .unwrap_or_else(|_| Err("Cancelled".into()));
            let _ = sender.send(result).await;
            wake();
        };
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("model runtime")
                .block_on(work)
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(work);
        Self { receiver, abort }
    }
    pub fn poll(&self) -> Option<Result<Vec<String>, String>> {
        self.receiver.try_recv().ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait(&self) -> Result<Vec<String>, String> {
        self.receiver.recv_blocking().map_err(|e| e.to_string())?
    }
}
impl Drop for ModelJob {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

pub async fn list_models(settings: &Settings, key: &str) -> Result<Vec<String>, String> {
    let (provider, _) = settings.model_spec()?;
    if key.trim().is_empty() {
        return Err("Set the provider API key first.".into());
    }
    let base = crate::stream::base_url(provider, settings)?;
    let client = crate::stream::client()?;
    let task = async {
        let mut models = Vec::new();
        let mut cursor = String::new();
        for _ in 0..50 {
            let mut request = client.get(format!("{base}/models"));
            match provider {
                Provider::Anthropic => {
                    request = request.query(&[("limit", "1000")]);
                    if !cursor.is_empty() {
                        request = request.query(&[("after_id", &cursor)]);
                    }
                }
                Provider::Google => {
                    request = request.query(&[("pageSize", "1000")]);
                    if !cursor.is_empty() {
                        request = request.query(&[("pageToken", &cursor)]);
                    }
                }
                Provider::OpenAi => {}
            }
            let response = crate::stream::authenticate(request, provider, key)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err(format!("Model list returned HTTP {}", response.status()));
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                    return Err("Model list is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let body: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let items = body[if provider == Provider::Google {
                "models"
            } else {
                "data"
            }]
            .as_array()
            .ok_or("Missing model list")?;
            for item in items {
                if provider == Provider::Google
                    && !item["supportedGenerationMethods"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|v| v == "generateContent"))
                {
                    continue;
                }
                if let Some(id) = item[if provider == Provider::Google {
                    "name"
                } else {
                    "id"
                }]
                .as_str()
                {
                    models.push(format!(
                        "{}:{}",
                        provider.name(),
                        id.trim_start_matches("models/")
                    ));
                }
            }
            let next = match provider {
                Provider::Google => body["nextPageToken"].as_str().unwrap_or(""),
                Provider::Anthropic if body["has_more"] == true => body["last_id"]
                    .as_str()
                    .ok_or("Missing pagination cursor")?,
                _ => "",
            };
            if next.is_empty() {
                models.sort();
                models.dedup();
                return Ok(models);
            }
            if next == cursor {
                return Err("Model pagination did not advance".into());
            }
            cursor = next.into();
        }
        Err("Model list exceeded the page limit".into())
    };
    #[cfg(not(target_arch = "wasm32"))]
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(30));
    #[cfg(target_arch = "wasm32")]
    let timeout = gloo_timers::future::TimeoutFuture::new(30_000);
    pin_mut!(task, timeout);
    match select(task, timeout).await {
        Either::Left((result, _)) => result.map_err(|s: String| s.replace(key, "[redacted]")),
        Either::Right(_) => Err("Model list timed out".into()),
    }
}
