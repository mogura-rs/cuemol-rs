//! Release discovery shared by the command-line and native applications.
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub fn current_version() -> &'static str {
    option_env!("CUEMOL_RELEASE_VERSION")
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}
pub fn repository() -> String {
    std::env::var("CUEMOL_RELEASE_REPOSITORY").unwrap_or_else(|_| {
        option_env!("CUEMOL_RELEASE_REPOSITORY")
            .unwrap_or("mogura-rs/cuemol2-rs")
            .into()
    })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub newer: bool,
}
pub fn check() -> Result<Option<Release>, String> {
    let repository = repository();
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
    {
        return Err("Invalid release repository".into());
    }
    check_at(
        &format!("https://api.github.com/repos/{repository}/releases/latest"),
        current_version(),
    )
}
fn check_at(url: &str, current: &str) -> Result<Option<Release>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .new_agent();
    let response = agent
        .get(url)
        .header("User-Agent", "cuemol-update-check")
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut response = match response {
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        result => result.map_err(|error| format!("Could not check releases: {error}"))?,
    };
    let value: serde_json::Value = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json()
        .map_err(|e| e.to_string())?;
    let tag = value["tag_name"].as_str().ok_or("Release has no version")?;
    let version = semver::Version::parse(tag.trim_start_matches('v'))
        .map_err(|_| "Release version is invalid")?;
    let current = semver::Version::parse(current.trim_start_matches('v'))
        .map_err(|_| "Installed version is invalid")?;
    let url = value["html_url"]
        .as_str()
        .filter(|url| url.starts_with("https://github.com/"))
        .ok_or("Release has no GitHub page")?;
    Ok(Some(Release {
        version: tag.into(),
        url: url.into(),
        newer: version > current,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_check_compares_versions_and_handles_unpublished_repositories() {
        for (status, tag, expected) in [
            (200, "v0.2.0", Some(true)),
            (200, "v0.1.0", Some(false)),
            (200, "v0.1.1-rc.1", Some(true)),
            (404, "", None),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/latest", server.server_addr());
            let worker = std::thread::spawn(move || {
                let request = server.recv().unwrap();
                request.respond(tiny_http::Response::from_string(serde_json::json!({"tag_name":tag,"html_url":"https://github.com/example/viewer/releases/tag/test"}).to_string()).with_status_code(status)).unwrap();
            });
            assert_eq!(check_at(&url, "0.1.0").unwrap().map(|r| r.newer), expected);
            worker.join().unwrap();
        }
    }
}
