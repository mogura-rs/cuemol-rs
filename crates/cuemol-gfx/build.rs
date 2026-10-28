fn main() {
    println!("cargo:rerun-if-changed=vendor");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut sources: Vec<_> = std::fs::read_dir("vendor/lcms2/src")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    sources.sort();
    cc::Build::new()
        .std("c99")
        .warnings(false)
        .opt_level(2)
        .include("vendor/lcms2/include")
        .files(sources)
        .file("vendor/cms_bridge.c")
        .compile("cuemol_lcms2");
}
