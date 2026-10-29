fn main() {
    #[cfg(feature = "tauri-gui")]
    {
        println!("cargo:rerun-if-changed=../../cuemol-rs.png");
        std::fs::copy("../../cuemol-rs.png", "tauri-ui/logo.png").expect("copy application logo");
        image::load_from_memory(include_bytes!("../../cuemol-rs.png"))
            .expect("application logo")
            .into_rgba8()
            .save("icons/app.png")
            .expect("write RGBA application icon");
        tauri_build::build();
    }
}
