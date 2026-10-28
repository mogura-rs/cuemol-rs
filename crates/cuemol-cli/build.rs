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
    let mut assets = vec![
        ("index.html", "text/html; charset=utf-8"),
        ("pkg/cuemol_web.js", "text/javascript; charset=utf-8"),
        ("pkg/cuemol_web_bg.wasm", "application/wasm"),
        ("kernels.js", "text/javascript; charset=utf-8"),
        ("oidn.js", "text/javascript; charset=utf-8"),
        ("pkg/oidn/oidn.js", "text/javascript; charset=utf-8"),
        ("pkg/oidn/rt_hdr_calb_cnrm.tza", "application/octet-stream"),
        ("pkg/oidn/OIDN-WebGPU-LICENSE", "text/plain; charset=utf-8"),
        ("pkg/oidn/OIDN-Weights-LICENSE", "text/plain; charset=utf-8"),
        ("pkg/oidn/SOURCES.json", "application/json"),
        ("pkg/kernels/usalign.js", "text/javascript; charset=utf-8"),
        ("pkg/kernels/usalign.wasm", "application/wasm"),
        ("pkg/kernels/lcms.js", "text/javascript; charset=utf-8"),
        ("pkg/kernels/lcms.wasm", "application/wasm"),
        ("pkg/kernels/surface.js", "text/javascript; charset=utf-8"),
        ("pkg/kernels/surface.wasm", "application/wasm"),
        ("pkg/kernels/BALL-LICENSE", "text/plain; charset=utf-8"),
        ("pkg/kernels/Boost-LICENSE", "text/plain; charset=utf-8"),
        ("pkg/kernels/MeshMS-SOURCE.json", "application/json"),
        ("pkg/kernels/BALL-SOURCE.json", "application/json"),
        ("pkg/kernels/LittleCMS-LICENSE", "text/plain; charset=utf-8"),
        ("pkg/kernels/USalign-LICENSE", "text/plain; charset=utf-8"),
    ];
    println!(
        "cargo:rerun-if-changed={}",
        root.join("build-info.json").display()
    );
    if root.join("build-info.json").is_file() {
        assets.push(("build-info.json", "application/json"));
    }
    assets.sort_unstable();
    for (index, (name, mime)) in assets.into_iter().enumerate() {
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
