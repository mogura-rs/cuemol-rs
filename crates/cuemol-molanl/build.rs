fn main() {
    println!("cargo:rerun-if-changed=vendor");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .flag_if_supported("/EHsc")
        .flag_if_supported("-Wno-deprecated-declarations")
        .file("vendor/USalignImpl.cpp")
        .file("vendor/usalign_bridge.cpp")
        .compile("cuemol_usalign");
}
