fn main() {
    if let Some(result) = cuemol_command::pov::try_cli_request() {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = cuemol_desktop::gui::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
