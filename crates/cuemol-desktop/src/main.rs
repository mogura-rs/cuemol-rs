fn main() {
    if let Err(error) = cuemol_desktop::gui::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
