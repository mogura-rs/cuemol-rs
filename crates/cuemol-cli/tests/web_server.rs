use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_slow_asset_download_does_not_block_other_requests() {
    let work = tempfile::tempdir().unwrap();
    std::fs::File::create(work.path().join("large.wasm"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    std::fs::write(work.path().join("input.js"), "export const ready = true;").unwrap();
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .args(["web", "serve", "--port", "0", "--no-open", "--dir"])
            .arg(work.path())
            .env("CUEMOL_CONFIG_DIR", work.path().join("config"))
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
        .trim_end_matches('/');
    let mut slow = TcpStream::connect(address).unwrap();
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

    let mut other = TcpStream::connect(address).unwrap();
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
