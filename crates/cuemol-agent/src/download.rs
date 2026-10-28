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
    let plan = cuemol_command::prepare_fetch(name, args).map_err(|e| e.to_string())?;
    let task = async {
        let response = reqwest::Client::new()
            .get(&plan.url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Download returned HTTP {}", response.status()));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            if bytes.len() + chunk.len() > 256 * 1024 * 1024 {
                return Err(
                    "Download exceeds 256 MiB; use a local file for this structure.".into(),
                );
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Downloaded { plan, bytes })
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
