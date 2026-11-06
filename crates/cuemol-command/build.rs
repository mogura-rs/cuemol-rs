use std::{collections::BTreeMap, env, fs, path::PathBuf, process::Command};

fn output(command: &mut Command) -> Option<String> {
    let result = command.output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let git = |args: &[&str]| output(Command::new("git").current_dir(&root).args(args));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    for name in ["SOURCE_DATE_EPOCH", "CUEMOL_RELEASE_VERSION", "RUSTC"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let mut git_paths = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
    if let Some(reference) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        git_paths.push(reference);
    }
    for name in git_paths {
        if let Some(path) = git(&["rev-parse", "--git-path", &name]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    let lock: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.lock")).unwrap())
        .expect("valid Cargo.lock");
    let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for package in lock["package"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        if ["wgpu", "egui", "tauri", "boa_engine"].contains(&name) {
            dependencies
                .entry(name.to_owned())
                .or_default()
                .push(package["version"].as_str().unwrap().to_owned());
        }
    }
    let date = match env::var("SOURCE_DATE_EPOCH") {
        Ok(epoch) => time::OffsetDateTime::from_unix_timestamp(
            epoch.parse().expect("Invalid SOURCE_DATE_EPOCH"),
        )
        .expect("Invalid SOURCE_DATE_EPOCH"),
        Err(_) => time::OffsetDateTime::now_utc(),
    }
    .format(&time::format_description::well_known::Rfc3339)
    .unwrap();
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let info = serde_json::json!({
        "version": env::var("CUEMOL_RELEASE_VERSION").unwrap_or_else(|_| env::var("CARGO_PKG_VERSION").unwrap()),
        "commit": git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unavailable".into()),
        "date": date,
        "compiler": output(Command::new(rustc).arg("--version")).unwrap_or_else(|| "unavailable".into()),
        "target": env::var("TARGET").unwrap(),
        "dependencies": dependencies,
    });
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("about-build.json"),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
}
