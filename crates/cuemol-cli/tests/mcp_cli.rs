use std::process::{Command, Stdio};

#[test]
fn bare_mcp_matches_explicit_connect_without_panicking() {
    let work = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .args(args)
            .env(
                "CUEMOL_LOCAL_API_INFO",
                work.path().join("missing-api.json"),
            )
            .env("CUEMOL_CONFIG_DIR", work.path().join("config"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let explicit = run(&["mcp", "connect"]);
    let default = run(&["mcp"]);
    assert_eq!(explicit.status.code(), Some(1));
    assert_eq!(default.status.code(), explicit.status.code());
    assert_eq!(default.stderr, explicit.stderr);
    assert!(default.stdout.is_empty());
}
