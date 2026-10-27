use crate::{config, Protocol};
use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

const MAX_MESSAGE: u64 = 4 * 1024 * 1024;

pub fn stdio(mut handle: impl FnMut(Value) -> Result<Option<Value>>) -> Result<()> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut line = Vec::new();
    loop {
        line.clear();
        if (&mut input)
            .take(MAX_MESSAGE + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() as u64 > MAX_MESSAGE {
            return Err(CueMolError::invalid_argument("MCP message exceeds 4 MiB"));
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(request) => handle(request)?,
            Err(_) => Some(crate::protocol::rpc_error(
                Value::Null,
                -32700,
                "Invalid JSON",
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)
                .map_err(|e| CueMolError::Io(e.to_string()))?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

pub fn serve_stdio() -> Result<()> {
    let mut state = DesktopAppState::new();
    state.new_scene("Untitled", true);
    config::load(&mut state)?;
    let mut camera = state.active_view_camera().unwrap_or_default();
    let mut protocol = Protocol::new();
    stdio(|request| Ok(protocol.handle(request, &mut state, &mut camera)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
    #[serde(default)]
    pub endpoints: Vec<String>,
}

pub fn info_path() -> PathBuf {
    config::directory().join("local-api.json")
}

pub fn read_info(path: &Path) -> Result<ConnectionInfo> {
    let data = std::fs::read(path)?;
    let info: ConnectionInfo =
        serde_json::from_slice(&data).map_err(|e| CueMolError::parse(e.to_string()))?;
    if info.port == 0
        || info.token.is_empty()
        || !info.endpoints.iter().any(|endpoint| endpoint == "mcp")
    {
        return Err(CueMolError::invalid_argument(
            "MCP is not enabled in this connection file",
        ));
    }
    Ok(info)
}

pub fn post(info: &ConnectionInfo, request: &Value) -> Result<Option<Value>> {
    let url = format!("http://127.0.0.1:{}/mcp", info.port);
    let mut response = ureq::post(&url)
        .header("Authorization", format!("Bearer {}", info.token))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-11-25")
        .send_json(request)
        .map_err(|e| CueMolError::Io(format!("MCP connection failed: {e}")))?;
    if response.status().as_u16() == 202 {
        return Ok(None);
    }
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| CueMolError::Io(e.to_string()))?;
    if content_type.starts_with("text/event-stream") {
        for line in body.lines().filter_map(|line| line.strip_prefix("data:")) {
            if let Ok(value) = serde_json::from_str::<Value>(line.trim()) {
                if value.get("id") == request.get("id") {
                    return Ok(Some(value));
                }
            }
        }
        return Err(CueMolError::parse(
            "MCP response stream contained no matching response",
        ));
    }
    serde_json::from_str(&body)
        .map(Some)
        .map_err(|e| CueMolError::parse(e.to_string()))
}

pub fn connect_stdio(path: &Path) -> Result<()> {
    let info = read_info(path)?;
    stdio(|request| post(&info, &request))
}

pub struct ApiRequest {
    pub message: Value,
    pub reply: mpsc::SyncSender<Option<Value>>,
}

pub struct ApiServer {
    pub requests: mpsc::Receiver<ApiRequest>,
    stop: Arc<AtomicBool>,
    info: ConnectionInfo,
    path: PathBuf,
}

impl ApiServer {
    pub fn start(address: SocketAddr, wake: impl Fn() + Send + 'static) -> Result<Self> {
        if !address.ip().is_loopback() {
            return Err(CueMolError::invalid_argument(
                "MCP must listen on a loopback address",
            ));
        }
        let server =
            tiny_http::Server::http(address).map_err(|e| CueMolError::Io(e.to_string()))?;
        let address = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| CueMolError::Io("MCP TCP address unavailable".into()))?;
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|e| CueMolError::Io(e.to_string()))?;
        let token = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let info = ConnectionInfo {
            port: address.port(),
            token: token.clone(),
            pid: std::process::id(),
            endpoints: vec!["mcp".into()],
        };
        let directory = config::directory();
        std::fs::create_dir_all(&directory)?;
        let path = info_path();
        let mut staged = tempfile::NamedTempFile::new_in(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            staged
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        serde_json::to_writer(&mut staged, &info).map_err(|e| CueMolError::Io(e.to_string()))?;
        staged.flush()?;
        staged
            .persist(&path)
            .map_err(|e| CueMolError::Io(e.to_string()))?;
        let stop = Arc::new(AtomicBool::new(false));
        let quitting = stop.clone();
        let (send, requests) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            while !quitting.load(Ordering::Relaxed) {
                let mut request = match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(r)) => r,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                let header = |key: &'static str| {
                    request
                        .headers()
                        .iter()
                        .find(|h| h.field.equiv(key))
                        .map(|h| h.value.as_str())
                };
                let origin_ok =
                    header("Origin").is_none_or(|origin| origin == format!("http://{address}"));
                let host_ok = header("Host").is_some_and(|host| {
                    host == address.to_string() || host == format!("localhost:{}", address.port())
                });
                let authenticated =
                    header("Authorization").is_some_and(|value| value == format!("Bearer {token}"));
                if !origin_ok || !host_ok {
                    let _ = request.respond(tiny_http::Response::empty(403));
                    continue;
                }
                if !authenticated {
                    let _ = request.respond(tiny_http::Response::empty(401));
                    continue;
                }
                if request.url() != "/mcp" {
                    let _ = request.respond(tiny_http::Response::empty(404));
                    continue;
                }
                if request.method() != &tiny_http::Method::Post {
                    let _ = request.respond(tiny_http::Response::empty(405));
                    continue;
                }
                if request
                    .body_length()
                    .is_some_and(|size| size as u64 > MAX_MESSAGE)
                {
                    let _ = request.respond(tiny_http::Response::empty(413));
                    continue;
                }
                let mut body = Vec::new();
                if request
                    .as_reader()
                    .take(MAX_MESSAGE + 1)
                    .read_to_end(&mut body)
                    .is_err()
                    || body.len() as u64 > MAX_MESSAGE
                {
                    let _ = request.respond(tiny_http::Response::empty(413));
                    continue;
                }
                let message = match serde_json::from_slice(&body) {
                    Ok(message) => message,
                    Err(_) => {
                        let _ = request.respond(tiny_http::Response::empty(400));
                        continue;
                    }
                };
                let (reply, answer) = mpsc::sync_channel(1);
                if send.try_send(ApiRequest { message, reply }).is_err() {
                    let _ = request.respond(tiny_http::Response::empty(503));
                    continue;
                }
                wake();
                match answer.recv_timeout(Duration::from_secs(120)) {
                    Ok(Some(response)) => {
                        let response = tiny_http::Response::from_string(response.to_string())
                            .with_header(
                                tiny_http::Header::from_bytes("Content-Type", "application/json")
                                    .unwrap(),
                            );
                        let _ = request.respond(response);
                    }
                    Ok(None) => {
                        let _ = request.respond(tiny_http::Response::empty(202));
                    }
                    Err(_) => {
                        let _ = request.respond(tiny_http::Response::empty(504));
                    }
                }
            }
        });
        Ok(Self {
            requests,
            stop,
            info,
            path,
        })
    }

    pub fn port(&self) -> u16 {
        self.info.port
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if read_info(&self.path).is_ok_and(|info| info.token == self.info.token) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn serve_http(address: SocketAddr) -> Result<()> {
    let mut state = DesktopAppState::new();
    state.new_scene("Untitled", true);
    config::load(&mut state)?;
    let mut camera = state.active_view_camera().unwrap_or_default();
    let mut protocol = Protocol::new();
    let api = ApiServer::start(address, || {})?;
    eprintln!(
        "CueMol MCP listening on http://127.0.0.1:{}/mcp",
        api.port()
    );
    while let Ok(request) = api.requests.recv() {
        let _ = request
            .reply
            .send(protocol.handle(request.message, &mut state, &mut camera));
    }
    Ok(())
}

pub(crate) fn fetch_structure(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    let (url, format, label) = if name == "fetch_pdb" {
        let id = args["pdbId"].as_str().unwrap().to_ascii_uppercase();
        if id.len() != 4
            || !id.bytes().next().is_some_and(|c| c.is_ascii_digit())
            || !id.bytes().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(CueMolError::invalid_argument("invalid PDB accession"));
        }
        let format = if args["format"] == "pdb" {
            "pdb"
        } else {
            "cif"
        };
        (
            format!("https://files.rcsb.org/download/{id}.{format}"),
            format.to_string(),
            id,
        )
    } else {
        let url = args["url"].as_str().unwrap().to_string();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(CueMolError::invalid_argument("URL must use http or https"));
        }
        let format = args["reader"].as_str().unwrap().to_string();
        if !format.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CueMolError::invalid_argument("invalid reader name"));
        }
        (
            url,
            format,
            args["name"].as_str().unwrap_or("download").to_string(),
        )
    };
    let directory = config::directory().join("downloads");
    std::fs::create_dir_all(&directory)?;
    let mut file = tempfile::Builder::new()
        .suffix(&format!(".{format}"))
        .tempfile_in(&directory)?;
    let mut response = ureq::get(&url)
        .call()
        .map_err(|e| CueMolError::Io(e.to_string()))?;
    std::io::copy(&mut response.body_mut().as_reader(), &mut file)?;
    file.flush()?;
    let (_file, path) = file.keep().map_err(|e| CueMolError::Io(e.to_string()))?;
    let mut input = args.clone();
    let map = input.as_object_mut().unwrap();
    map.remove("pdbId");
    map.remove("format");
    map.remove("url");
    map.remove("reader");
    map.insert("path".into(), json!(path));
    map.insert(
        "name".into(),
        args["name"]
            .as_str()
            .map(|n| json!(n))
            .unwrap_or(json!(label)),
    );
    let result = crate::call_tool(state, camera, "load_file", &input);
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}
