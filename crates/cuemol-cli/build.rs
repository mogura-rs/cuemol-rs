use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

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
        Some("svg") => "image/svg+xml",
        Some("tza") => "application/octet-stream",
        _ => "text/plain; charset=utf-8",
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=CUEMOL_WEB_DIR");
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
    if manifest.is_file() {
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap())
            .expect("Invalid browser distribution manifest");
        assets.extend(
            data["files"]
                .as_object()
                .expect("Manifest files are missing")
                .keys()
                .cloned(),
        );
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
