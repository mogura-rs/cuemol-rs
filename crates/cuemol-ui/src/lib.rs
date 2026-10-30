#[cfg(feature = "egui")]
mod hatch;
#[cfg(feature = "egui")]
pub use hatch::hatch_controls;

#[cfg(feature = "egui")]
mod lighting;
#[cfg(feature = "egui")]
pub use lighting::lighting_controls;

#[cfg(feature = "egui")]
mod agent;
#[cfg(feature = "egui")]
pub use agent::{AgentCall, AgentPanel};

#[cfg(feature = "egui")]
pub mod clipboard;
#[cfg(feature = "egui")]
pub mod shell;

#[cfg(feature = "egui")]
pub mod navigation;

#[cfg(feature = "egui")]
mod inspector;
#[cfg(feature = "egui")]
pub use inspector::{Inspector, InspectorTarget};

#[cfg(feature = "egui")]
mod render;
#[cfg(feature = "egui")]
pub use render::{RenderAction, RenderControls, RenderPanel};

#[cfg(feature = "egui")]
pub mod sequence;

#[cfg(feature = "egui")]
pub mod actions;
#[cfg(feature = "egui")]
pub mod console;
#[cfg(feature = "console-runtime")]
pub mod console_runtime;
