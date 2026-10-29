fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("cuemol-rs {}", cuemol_command::update::current_version());
            return;
        }
        Some("--help" | "-h") => {
            println!("Usage: cuemol-rs [FILE ...]");
            return;
        }
        _ => {}
    }
    if let Err(error) = cuemol_desktop::tauri_app::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
