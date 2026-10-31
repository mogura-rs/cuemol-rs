use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
mod build_support;

fn record_build() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    for item in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = build_support::git(&root, &["rev-parse", "--git-path", item]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(reference) = build_support::git(&root, &["symbolic-ref", "--quiet", "HEAD"]) {
        if let Some(path) = build_support::git(&root, &["rev-parse", "--git-path", &reference]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(files) = build_support::git(&root, &["ls-files", "-z"]) {
        for name in files.split('\0').filter(|name| !name.is_empty()) {
            println!("cargo:rerun-if-changed={}", root.join(name).display());
        }
    }
    println!("cargo:rerun-if-changed=build_support.rs");
    let mut info = build_support::source_info(&root);
    info["target"] = std::env::var("TARGET").unwrap().into();
    info["features"] = ["gui", "bundled-web", "scripting"]
        .into_iter()
        .filter(|name| {
            std::env::var_os(format!(
                "CARGO_FEATURE_{}",
                name.replace('-', "_").to_uppercase()
            ))
            .is_some()
        })
        .collect::<Vec<_>>()
        .into();
    info["web_commit"] = serde_json::Value::Null;
    info["web_dirty"] = serde_json::Value::Null;
    info["web_manifest_sha256"] = serde_json::Value::Null;
    if std::env::var_os("CARGO_FEATURE_BUNDLED_WEB").is_some() {
        let web = std::env::var_os("CUEMOL_WEB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("web"));
        if let Ok(bytes) = std::fs::read(web.join("pkg/source-build.json")) {
            let source: serde_json::Value =
                serde_json::from_slice(&bytes).expect("Invalid browser source metadata");
            info["web_commit"] = source["commit"].clone();
            info["web_dirty"] = source["dirty"].clone();
        }
        if let Ok(bytes) = std::fs::read(web.join("build-info.json")) {
            info["web_manifest_sha256"] = format!("{:x}", Sha256::digest(bytes)).into();
        }
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(
        out.join("build-info.json"),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
}

fn collect_assets(root: &Path, directory: &Path, names: &mut BTreeSet<String>) {
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in std::fs::read_dir(directory).expect("Build browser assets before bundled-web") {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(!kind.is_symlink(), "Browser assets must not be symlinks");
        if kind.is_dir() {
            collect_assets(root, &entry.path(), names);
        } else if kind.is_file() {
            names.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/"),
            );
        }
    }
}

fn mime(name: &str) -> &'static str {
    match Path::new(name).extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("png") => "image/png",
        Some("ttf") => "font/ttf",
        Some("svg") => "image/svg+xml",
        Some("tza") => "application/octet-stream",
        _ => "text/plain; charset=utf-8",
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=CUEMOL_WEB_DIR");
    record_build();
    if std::env::var_os("CARGO_FEATURE_BUNDLED_WEB").is_none() {
        return;
    }
    let root = std::env::var_os("CUEMOL_WEB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../web")
        });
    let root = root
        .canonicalize()
        .expect("Browser distribution directory is missing");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let manifest = root.join("build-info.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let mut assets = BTreeSet::new();
    let mut checksums = serde_json::Map::new();
    if manifest.is_file() {
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap())
            .expect("Invalid browser distribution manifest");
        checksums = data["files"]
            .as_object()
            .expect("Manifest files are missing")
            .clone();
        assets.extend(checksums.keys().cloned());
        assets.insert("build-info.json".into());
    } else {
        assets.extend(
            [
                "index.html",
                "kernels.js",
                "oidn.js",
                "input.js",
                "loader.js",
                "loader-worker.js",
            ]
            .map(str::to_owned),
        );
        collect_assets(&root, &root.join("pkg"), &mut assets);
    }
    for required in [
        "index.html",
        "loader.js",
        "loader-worker.js",
        "pkg/cuemol_web.js",
        "pkg/cuemol_web_bg.wasm",
        "pkg/cuemol_web_loader.js",
        "pkg/cuemol_web_loader_bg.wasm",
        "pkg/fonts/NotoSansJP-Regular.ttf",
        "pkg/fonts/OFL.txt",
        "pkg/fonts/SOURCE.json",
    ] {
        assert!(
            assets.contains(required),
            "Missing browser entry point: {required}"
        );
    }
    let mut source = String::from("static BUNDLED_WEB: &[(&str, &str, &[u8])] = &[\n");
    for (index, name) in assets.into_iter().enumerate() {
        assert!(
            !name.is_empty()
                && !name.contains('\\')
                && Path::new(&name)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "Invalid browser asset path: {name}"
        );
        let path = root.join(&name);
        assert!(
            path.canonicalize().unwrap().starts_with(&root),
            "Browser asset is outside the distribution"
        );
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(path)
            .expect("Build the web assets with cuemol web build before enabling bundled-web");
        if let Some(expected) = checksums.get(&name) {
            assert_eq!(
                expected.as_str(),
                Some(format!("{:x}", Sha256::digest(&bytes)).as_str()),
                "Browser checksum mismatch: {name}"
            );
        }
        let output = out.join(format!("web-{index}.gz"));
        let mut gzip = flate2::write::GzEncoder::new(
            std::fs::File::create(&output).unwrap(),
            flate2::Compression::best(),
        );
        gzip.write_all(&bytes).unwrap();
        gzip.finish().unwrap();
        let mime = mime(&name);
        source.push_str(&format!(
            "({name:?},{mime:?},include_bytes!({output:?})),\n"
        ));
    }
    source.push_str("];\n");
    std::fs::write(out.join("web_assets.rs"), source).unwrap();
}
