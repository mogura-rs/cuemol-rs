use crate::{
    provider::{Provider, Round},
    Event, Events, Settings,
};
use futures_util::{
    future::{select, Either},
    pin_mut, StreamExt,
};
use serde_json::Value;

const MAX_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn client() -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder();
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(20));
    builder.build().map_err(|e| e.to_string())
}

#[derive(Default)]
pub(crate) struct Sse {
    bytes: Vec<u8>,
    data: String,
}
impl Sse {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() + self.data.len() > MAX_BYTES {
            return Err("Model event exceeded the size limit".into());
        }
        let mut events = Vec::new();
        let mut consumed = 0;
        while let Some(relative) = self.bytes[consumed..].iter().position(|b| *b == b'\n') {
            let end = consumed + relative;
            let line = std::str::from_utf8(&self.bytes[consumed..end])
                .map_err(|_| "Invalid UTF-8 in event stream")?
                .trim_end_matches('\r');
            consumed = end + 1;
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(std::mem::take(&mut self.data));
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
            }
        }
        self.bytes.drain(..consumed);
        Ok(events)
    }
}

pub(crate) fn base_url(provider: Provider, settings: &Settings) -> Result<String, String> {
    let base = if settings.endpoint.trim().is_empty() {
        provider.base()
    } else {
        settings.endpoint.trim_end_matches('/')
    };
    let url = reqwest::Url::parse(base).map_err(|_| "Invalid API endpoint")?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use an HTTPS API endpoint, or HTTP on localhost.".into());
    }
    Ok(base.to_string())
}
pub(crate) fn authenticate(
    request: reqwest::RequestBuilder,
    provider: Provider,
    key: &str,
) -> reqwest::RequestBuilder {
    match provider {
        Provider::OpenAi => request.bearer_auth(key),
        Provider::Anthropic => request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-dangerous-direct-browser-access", "true"),
        Provider::Google => request.header("x-goog-api-key", key),
    }
}
pub(crate) async fn request(
    client: &reqwest::Client,
    provider: Provider,
    model: &str,
    settings: &Settings,
    key: &str,
    messages: &[Value],
    enabled: &std::collections::BTreeSet<String>,
    events: &Events,
) -> Result<Round, String> {
    let base = base_url(provider, settings)?;
    let suffix = match provider {
        Provider::OpenAi => "responses".into(),
        Provider::Anthropic => "messages".into(),
        Provider::Google => format!("models/{model}:streamGenerateContent?alt=sse"),
    };
    let request = authenticate(client.post(format!("{base}/{suffix}")), provider, key)
        .json(&provider.request(model, settings, messages, enabled));
    let task = async {
        let response = request
            .send()
            .await
            .map_err(|e| format!("Provider connection failed: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                let remaining = 8192usize.saturating_sub(bytes.len());
                bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                if bytes.len() == 8192 {
                    break;
                }
            }
            let detail = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or_else(|| "Check the model, API key, quota and endpoint.".into());
            return Err(format!(
                "{} API returned HTTP {}: {}",
                provider.name(),
                status,
                detail
            ));
        }
        let mut stream = response.bytes_stream();
        let mut parser = Sse::default();
        let mut round = Round::default();
        let mut total = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Response stream interrupted: {e}"))?;
            total += chunk.len();
            if total > MAX_BYTES {
                return Err("Model response exceeded the size limit".into());
            }
            for data in parser.push(&chunk)? {
                if data == "[DONE]" {
                    continue;
                }
                let value =
                    serde_json::from_str(&data).map_err(|e| format!("Invalid event JSON: {e}"))?;
                let text = round.event(provider, value)?;
                if !text.is_empty() {
                    events.send(Event::Text(text)).await?;
                }
            }
        }
        if !round.complete {
            return Err("Model stream ended before completion; retry the turn.".into());
        }
        Ok(round)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(300));
    #[cfg(target_arch = "wasm32")]
    let timeout = gloo_timers::future::TimeoutFuture::new(300_000);
    pin_mut!(task, timeout);
    match select(task, timeout).await {
        Either::Left((result, _)) => result.map_err(|s: String| s.replace(key, "[redacted]")),
        Either::Right(_) => Err("Provider timed out after five minutes.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_handles_split_utf8_crlf_comments_and_multiple_data_lines() {
        let input = ": keepalive\r\ndata: {\"text\":\"日本語\",\r\ndata: \"ok\":true}\r\n\r\n";
        let mut s = Sse::default();
        let events: Vec<_> = input
            .as_bytes()
            .iter()
            .flat_map(|b| s.push(&[*b]).unwrap())
            .collect();
        assert_eq!(events.len(), 1);
        let value: Value = serde_json::from_str(&events[0]).unwrap();
        assert_eq!(value["text"], "日本語");
        assert_eq!(value["ok"], true);
    }
}
