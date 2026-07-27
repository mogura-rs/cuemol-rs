fn main() {
    if let Err(error) = cuemol_desktop::tauri_app::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
