use std::path::Path;
use std::process::Command;

pub fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn source_info(root: &Path) -> serde_json::Value {
    serde_json::json!({
        "commit": git(root, &["rev-parse", "HEAD"]),
        "dirty": git(root, &["status", "--porcelain", "--untracked-files=normal"])
            .map(|status| !status.is_empty()),
    })
}
