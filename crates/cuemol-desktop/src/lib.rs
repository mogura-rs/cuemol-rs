//! Desktop front ends (egui + gpui) over the shared, GUI-agnostic
//! [`cuemol_desktop_core`] logic.
//!
//! The headless `DesktopAppState` facade, the `Gui*Summary` view-models, and the
//! offscreen wgpu renderer now live in `cuemol-desktop-core` so they can also be
//! reused by the Dioxus front end (`cuemol-desktop-dioxus`), which must sit
//! outside this workspace — gpui 0.2.2 hard-pins `cocoa =0.26.0` while Dioxus
//! needs `>=0.26.1`, an irreconcilable single-lockfile conflict.
//!
//! Everything from the core crate is re-exported at this crate's root, so the
//! existing GUI modules keep referring to `crate::DesktopAppState`,
//! `crate::GuiObjectSummary`, `crate::offscreen::…` unchanged.

pub use cuemol_desktop_core::*;

// The offscreen renderer is compiled into core only under its `offscreen`
// feature, which both GUIs turn on; re-export it so `crate::offscreen::…` keeps
// resolving in the GUI modules.
#[cfg(any(feature = "native-gui", feature = "gpui-gui"))]
pub use cuemol_desktop_core::offscreen;

#[cfg(feature = "native-gui")]
pub mod gui;

#[cfg(feature = "gpui-gui")]
pub mod gpui_widgets;

#[cfg(feature = "gpui-gui")]
pub mod gpui_app;
