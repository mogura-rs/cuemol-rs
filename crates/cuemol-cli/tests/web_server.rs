use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(command: &mut Command, directory: &Path) -> (Server, String) {
    let mut server = Server(
        command
            .args(["web", "serve", "--port", "0", "--no-open", "--dir"])
            .arg(directory)
            .env("CUEMOL_CONFIG_DIR", directory.join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let address = line
        .trim()
        .strip_prefix("cuemol-rs: http://")
        .unwrap()
        .trim_end_matches('/')
        .to_owned();
    (server, address)
}

#[test]
fn a_slow_asset_download_does_not_block_other_requests() {
    let work = tempfile::tempdir().unwrap();
    std::fs::File::create(work.path().join("large.wasm"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    std::fs::write(work.path().join("input.js"), "export const ready = true;").unwrap();
    let (_server, address) =
        start_server(&mut Command::new(env!("CARGO_BIN_EXE_cuemol")), work.path());
    let mut slow = TcpStream::connect(&address).unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    slow.write_all(b"GET /large.wasm HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        slow.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 8192);
    }
    assert!(header.starts_with(b"HTTP/1.1 200"));

    let mut other = TcpStream::connect(&address).unwrap();
    other
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    other
        .write_all(b"GET /input.js HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    other
        .read_to_string(&mut response)
        .expect("a stalled download blocked an unrelated asset");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with("export const ready = true;"));
}

#[cfg(target_os = "linux")]
#[test]
fn head_requests_do_not_load_large_assets_into_memory() {
    let work = tempfile::tempdir().unwrap();
    std::fs::File::create(work.path().join("large.wasm"))
        .unwrap()
        .set_len(128 * 1024 * 1024)
        .unwrap();
    let (server, address) =
        start_server(&mut Command::new(env!("CARGO_BIN_EXE_cuemol")), work.path());
    let peak_rss = || {
        std::fs::read_to_string(format!("/proc/{}/status", server.0.id()))
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let before = peak_rss();
    let mut client = TcpStream::connect(&address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    client
        .write_all(b"HEAD /large.wasm HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with("\r\n\r\n"));
    assert!(
        peak_rss().saturating_sub(before) < 32 * 1024,
        "HEAD allocated a large asset before sending its headers"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_connection_burst_is_served_before_keep_alive_clients_disconnect() {
    let work = tempfile::tempdir().unwrap();
    let body = b"export const ready = true;";
    std::fs::write(work.path().join("input.js"), body).unwrap();
    let process = std::fs::read_to_string("/proc/self/status").unwrap();
    let cpu = process
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .unwrap()
        .trim()
        .split([',', '-'])
        .next()
        .unwrap();
    let mut command = Command::new("taskset");
    command.args(["-c", cpu, env!("CARGO_BIN_EXE_cuemol")]);
    let (server, address) = start_server(&mut command, work.path());
    std::thread::sleep(Duration::from_millis(100));
    let signal = |name| {
        assert!(Command::new("kill")
            .arg(name)
            .arg(server.0.id().to_string())
            .status()
            .unwrap()
            .success());
    };
    signal("-STOP");
    let mut clients: Vec<_> = (0..12)
        .map(|_| TcpStream::connect(&address).unwrap())
        .collect();
    for client in &mut clients {
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        client
            .write_all(
                b"GET /input.js HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
            )
            .unwrap();
    }
    signal("-CONT");
    for (index, client) in clients.iter_mut().enumerate() {
        let mut reader = BufReader::new(client);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap_or_else(|error| {
            panic!("connection {index} remained queued: {error}");
        });
        assert!(line.starts_with("HTTP/1.1 200"));
        loop {
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
        }
        let mut response = vec![0; body.len()];
        reader.read_exact(&mut response).unwrap();
        assert_eq!(response, body);
    }
}
