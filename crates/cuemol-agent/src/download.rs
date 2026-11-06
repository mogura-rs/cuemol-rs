use crate::Wake;
use cuemol_command::FetchPlan;
use futures_util::{
    future::{select, AbortHandle, Abortable, Either},
    pin_mut, StreamExt,
};
use serde_json::Value;

#[derive(Debug)]
pub struct Downloaded {
    pub plan: FetchPlan,
    pub bytes: Vec<u8>,
}
#[derive(Debug)]
pub struct DownloadJob {
    receiver: async_channel::Receiver<Result<Downloaded, String>>,
    abort: AbortHandle,
}
impl DownloadJob {
    pub fn start(name: String, args: Value, wake: Wake) -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        let (abort, registration) = AbortHandle::new_pair();
        let work = async move {
            let result = Abortable::new(download(&name, &args), registration)
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
                .expect("download runtime")
                .block_on(work)
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(work);
        Self { receiver, abort }
    }
    pub fn poll(&self) -> Option<Result<Downloaded, String>> {
        self.receiver.try_recv().ok()
    }
}
impl Drop for DownloadJob {
    fn drop(&mut self) {
        self.abort.abort();
    }
}
async fn download(name: &str, args: &Value) -> Result<Downloaded, String> {
    let mut plan = cuemol_command::prepare_fetch(name, args).map_err(|e| e.to_string())?;
    let task = async {
        let client = reqwest::Client::new();
        loop {
            let request = if let Some(body) = &plan.request_body {
                client.post(&plan.url).json(body)
            } else {
                client.get(&plan.url)
            };
            let response = match request.send().await {
                Ok(response) => response,
                Err(_) if plan.optional_metadata() => {
                    plan = cuemol_command::advance_fetch(&plan, 0, &[])
                        .map_err(|e| e.to_string())?
                        .unwrap();
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            };
            let status = response.status().as_u16();
            let mut bytes = Vec::new();
            let limit = if plan.is_metadata() {
                4 * 1024 * 1024
            } else {
                256 * 1024 * 1024
            };
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if bytes.len() + chunk.len() > limit {
                    return Err(format!(
                        "Download exceeds {} MiB; use a local file for this data.",
                        limit / 1024 / 1024
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
            if let Some(next) =
                cuemol_command::advance_fetch(&plan, status, &bytes).map_err(|e| e.to_string())?
            {
                plan = next;
            } else {
                return Ok(Downloaded { plan, bytes });
            }
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(120));
    #[cfg(target_arch = "wasm32")]
    let timeout = gloo_timers::future::TimeoutFuture::new(120_000);
    pin_mut!(task, timeout);
    match select(task, timeout).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err("Download timed out".into()),
    }
}
