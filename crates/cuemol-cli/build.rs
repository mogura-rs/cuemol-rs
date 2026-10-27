use std::io::Write;
use std::path::PathBuf;
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
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let mut source = String::from("static BUNDLED_WEB: &[(&str, &str, &[u8])] = &[\n");
    for (index, (name, mime)) in [
        ("index.html", "text/html; charset=utf-8"),
        ("pkg/cuemol_web.js", "text/javascript; charset=utf-8"),
        ("pkg/cuemol_web_bg.wasm", "application/wasm"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = root.join(name);
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
        source.push_str(&format!(
            "({name:?},{mime:?},include_bytes!({output:?})),\n"
        ));
    }
    source.push_str("];\n");
    std::fs::write(out.join("web_assets.rs"), source).unwrap();
}
