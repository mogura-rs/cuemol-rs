//! Entry point for the gpui-based desktop GUI (`cuemol-desktop-gpui`).
//! Any command-line arguments are treated as molecule files to open on launch.

fn main() {
    let open_paths = std::env::args_os().skip(1).map(Into::into).collect();
    cuemol_desktop::gpui_app::run(open_paths);
}
