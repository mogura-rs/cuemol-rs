use cuemol_command::transport::{info_path, read_info, GuiApiServer};
use cuemol_desktop_core::DesktopAppState;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

#[test]
fn gui_plugin_server_lifecycle() {
    if std::env::var_os("CUEMOL_PLUGIN_LIFECYCLE_CHILD").is_none() {
        let directory = tempfile::tempdir().unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "gui_plugin_server_lifecycle", "--nocapture"])
            .env("CUEMOL_PLUGIN_LIFECYCLE_CHILD", "1")
            .env("CUEMOL_CONFIG_DIR", directory.path())
            .env_remove("CUEMOL_DISABLE_MCP")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let mut state = DesktopAppState::new();
    let mut server = GuiApiServer::default();
    assert!(!server.sync(&state, || {}).unwrap());
    assert!(!info_path().exists());
    state.set_plugin_enabled("mcp", true).unwrap();
    let occupied = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    state.put_config_value(
        "mcp:port",
        occupied.local_addr().unwrap().port().to_string(),
    );
    assert!(server.sync(&state, || {}).is_err());
    assert!(state.mcp_server_enabled());
    assert!(!server.sync(&state, || {}).unwrap());
    drop(occupied);
    state.put_config_value("mcp:port", "0");
    assert!(server.sync(&state, || {}).unwrap());
    let info = read_info(&info_path()).unwrap();
    assert_eq!(server.port(), Some(info.port));
    assert!(!server.sync(&state, || {}).unwrap());
    assert_eq!(read_info(&info_path()).unwrap().token, info.token);
    let connection = info.clone();
    let request = std::thread::spawn(move || {
        cuemol_command::transport::post(
            &connection,
            &serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
        )
    });
    let pending = server
        .server()
        .unwrap()
        .requests
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    pending
        .reply
        .send(Some(
            serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}}),
        ))
        .unwrap();
    assert_eq!(request.join().unwrap().unwrap().unwrap()["id"], 1);
    state.put_config_value("mcp:accept-connections", "false");
    assert!(server.sync(&state, || {}).unwrap());
    assert!(!info_path().exists());
    assert!(server.port().is_none());
    let address: SocketAddr = ([127, 0, 0, 1], info.port).into();
    let deadline = Instant::now() + Duration::from_secs(2);
    while TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
        assert!(
            Instant::now() < deadline,
            "disabled MCP listener remained open"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    state.put_config_value("mcp:accept-connections", "true");
    state.put_config_value("mcp:port", info.port.to_string());
    assert!(server.sync(&state, || {}).unwrap());
    assert_eq!(server.port(), Some(info.port));
    assert_ne!(read_info(&info_path()).unwrap().token, info.token);
    let connection = read_info(&info_path()).unwrap();
    let request = std::thread::spawn(move || {
        cuemol_command::transport::post(
            &connection,
            &serde_json::json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
        )
    });
    let pending = server
        .server()
        .unwrap()
        .requests
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    state.set_plugin_enabled("mcp", false).unwrap();
    assert!(server.sync(&state, || {}).unwrap());
    assert!(!info_path().exists());
    let deadline = Instant::now() + Duration::from_secs(2);
    while TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
        assert!(
            Instant::now() < deadline,
            "disabled MCP listener waited for an outstanding result"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(pending.reply.send(None).is_err());
    assert!(request.join().unwrap().is_err());
}
