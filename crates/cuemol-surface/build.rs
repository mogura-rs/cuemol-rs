fn main() {
    println!("cargo:rerun-if-changed=vendor");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut sources = Vec::new();
    for directory in [
        "vendor/meshms/src",
        "vendor/BALL/COMMON",
        "vendor/BALL/DATATYPE",
        "vendor/BALL/STRUCTURE",
    ] {
        sources.extend(
            std::fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "cpp")),
        );
    }
    sources.sort();
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .define("_USE_MATH_DEFINES", None)
        .opt_level(2)
        .warnings(false)
        .flag_if_supported("/EHsc")
        .flag_if_supported("-ffp-contract=off")
        .include("vendor")
        .include("vendor/meshms/include")
        .files(sources)
        .file("vendor/edtsurf/ProteinSurface.cpp")
        .file("vendor/surface_bridge.cpp")
        .compile("cuemol_surfaces");
}
