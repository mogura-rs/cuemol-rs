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
            .unwrap_or("mogura-rs/cuemol-rs")
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

pub type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub message: String,
    pub checking: bool,
    pub installing: bool,
    pub release: Option<Release>,
    pub notify: bool,
}

#[derive(Debug, Default)]
pub struct UpdateRuntime {
    status: String,
    release: Option<Release>,
    checking: Option<std::sync::mpsc::Receiver<Result<Option<Release>, String>>>,
    installing: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    startup: Option<std::time::Instant>,
    quiet: bool,
    notify: bool,
}
impl UpdateRuntime {
    pub fn schedule_startup(&mut self) {
        if std::env::var_os("CUEMOL_DISABLE_UPDATE_CHECK").is_none() {
            self.startup = Some(std::time::Instant::now() + Duration::from_secs(5));
        }
    }
    pub fn pending_startup(&self) -> bool {
        self.startup.is_some()
    }
    pub fn snapshot(&self) -> UpdateStatus {
        UpdateStatus {
            current_version: current_version().into(),
            message: self.status.clone(),
            checking: self.checking.is_some(),
            installing: self.installing.is_some(),
            release: self.release.clone(),
            notify: self.notify,
        }
    }
    pub fn acknowledge(&mut self) {
        self.notify = false;
    }
    pub fn check(&mut self, quiet: bool, wake: Wake) -> bool {
        self.check_with(quiet, wake, check)
    }
    fn check_with(
        &mut self,
        quiet: bool,
        wake: Wake,
        task: impl FnOnce() -> Result<Option<Release>, String> + Send + 'static,
    ) -> bool {
        self.startup = None;
        if self.checking.is_some() || self.installing.is_some() {
            return false;
        }
        self.status = "Checking for updates…".into();
        self.release = None;
        self.quiet = quiet;
        self.notify = false;
        let (sender, receiver) = std::sync::mpsc::channel();
        self.checking = Some(receiver);
        std::thread::spawn(move || {
            let _ = sender.send(task());
            wake();
        });
        true
    }
    pub fn install(&mut self, wake: Wake) -> Result<(), String> {
        if self.checking.is_some() || self.installing.is_some() {
            return Err("An update operation is already running.".into());
        }
        let release = self
            .release
            .as_ref()
            .filter(|r| r.newer)
            .ok_or("Check for a newer release before installing.")?;
        let version = release.version.clone();
        self.status = "Downloading and installing the verified release…".into();
        self.notify = false;
        let (sender, receiver) = std::sync::mpsc::channel();
        self.installing = Some(receiver);
        std::thread::spawn(move || {
            let _ = sender.send(install_release(&version));
            wake();
        });
        Ok(())
    }
    pub fn poll(&mut self, startup_enabled: bool, wake: Wake) -> bool {
        let mut changed = false;
        if self
            .startup
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.startup = None;
            if startup_enabled {
                changed |= self.check(true, wake);
            }
        }
        if let Some(result) = self.checking.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.checking = None;
            self.status = match result {
                Ok(Some(release)) => {
                    let status = if release.newer {
                        format!("CueMol {} is available.", release.version)
                    } else {
                        "You are using the latest version.".into()
                    };
                    self.notify = self.quiet && release.newer;
                    self.release = Some(release);
                    status
                }
                Ok(None) => "No published release is available yet.".into(),
                Err(error) => error,
            };
            changed = true;
        }
        if let Some(result) = self.installing.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.installing = None;
            self.status = result
                .map(|()| "Update installed. Restart CueMol to use it.".into())
                .unwrap_or_else(|error| error);
            self.release = None;
            changed = true;
        }
        changed
    }
}

fn install_release(version: &str) -> Result<(), String> {
    semver::Version::parse(version.trim_start_matches('v'))
        .map_err(|_| "Release version is invalid")?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    if executable.file_stem().and_then(|name| name.to_str()) != Some("cuemol") {
        return Err(
            "Run cuemol self-upgrade from the installed command to update this development build."
                .into(),
        );
    }
    let output = std::process::Command::new(executable)
        .arg("self-upgrade")
        .env("CUEMOL_VERSION", version)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Update failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    fn finish(runtime: &mut UpdateRuntime) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while runtime.snapshot().checking {
            runtime.poll(true, std::sync::Arc::new(|| {}));
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn startup_checks_are_silent_without_a_new_release_and_manual_checks_are_exclusive() {
        let wake: Wake = std::sync::Arc::new(|| {});
        let mut runtime = UpdateRuntime::default();
        runtime.startup = Some(std::time::Instant::now());
        assert!(!runtime.poll(false, wake.clone()));
        assert!(!runtime.pending_startup());
        assert!(!runtime.snapshot().checking);
        for (quiet, newer, notify) in [
            (true, false, false),
            (true, true, true),
            (false, true, false),
        ] {
            assert!(
                runtime.check_with(quiet, wake.clone(), move || Ok(Some(Release {
                    version: "v9.0.0".into(),
                    url: "https://github.com/example/viewer/releases/tag/v9.0.0".into(),
                    newer
                })))
            );
            assert!(!runtime.check_with(false, wake.clone(), || panic!(
                "a second request must not run"
            )));
            finish(&mut runtime);
            assert_eq!(runtime.snapshot().notify, notify);
            assert_eq!(runtime.snapshot().release.as_ref().unwrap().newer, newer);
            runtime.acknowledge();
            assert!(!runtime.snapshot().notify);
        }
        assert!(runtime.check_with(true, wake, || Err("offline".into())));
        finish(&mut runtime);
        assert_eq!(runtime.snapshot().message, "offline");
        assert!(!runtime.snapshot().notify);
        assert!(runtime.install(std::sync::Arc::new(|| {})).is_err());
    }
}
