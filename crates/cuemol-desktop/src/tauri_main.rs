fn main() {
    if let Some(result) = cuemol_command::pov::try_cli_request() {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
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
