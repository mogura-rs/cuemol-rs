//! Desktop front ends (egui and Tauri) over the shared, GUI-agnostic
//! [`cuemol_desktop_core`] logic.
//!
//! The headless `DesktopAppState` facade, the `Gui*Summary` view-models, and the
//! offscreen wgpu renderer live in `cuemol-desktop-core` and are shared by both
//! front ends.
//!
//! Everything from the core crate is re-exported at this crate's root, so the
//! existing GUI modules keep referring to `crate::DesktopAppState`,
//! `crate::GuiObjectSummary`, `crate::offscreen::…` unchanged.

pub use cuemol_desktop_core::*;

// The offscreen renderer is compiled into core only under its `offscreen`
// feature, which both GUIs turn on; re-export it so `crate::offscreen::…` keeps
// resolving in the GUI modules.
#[cfg(any(feature = "egui-gui", feature = "tauri-gui"))]
pub use cuemol_desktop_core::offscreen;

#[cfg(feature = "egui-gui")]
pub mod gui;

#[cfg(feature = "tauri-gui")]
pub mod tauri_app;
