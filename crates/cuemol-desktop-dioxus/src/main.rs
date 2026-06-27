//! Entry point for the Dioxus-based desktop GUI (`cuemol-desktop-dioxus`).
//! Any command-line arguments are treated as molecule files to open on launch.

fn main() {
    let open_paths = std::env::args_os().skip(1).map(Into::into).collect();
    cuemol_desktop_dioxus::run(open_paths);
}
