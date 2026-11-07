#[cfg(feature = "egui")]
pub mod about;

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
pub use agent::AgentPanel;
#[cfg(feature = "console-runtime")]
pub mod agent_runtime;
#[cfg(feature = "console-runtime")]
pub use agent_runtime::AgentCall;

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
pub mod selection;

#[cfg(feature = "egui")]
pub mod selection_pane;

#[cfg(feature = "egui")]
pub mod style_editor;

#[cfg(feature = "egui")]
pub mod actions;
#[cfg(feature = "egui")]
pub mod catalog;
#[cfg(feature = "egui")]
pub mod console;
#[cfg(feature = "console-runtime")]
pub mod console_runtime;

#[cfg(feature = "egui")]
pub mod pdb_dialog;

#[cfg(feature = "egui")]
mod numeric;

#[cfg(feature = "egui")]
pub mod density;

#[cfg(feature = "egui")]
pub mod symmetry;

#[cfg(feature = "egui")]
pub mod animation;

#[cfg(feature = "egui")]
pub mod camera;

#[cfg(feature = "egui")]
pub mod view;

#[cfg(feature = "egui")]
pub mod settings;

#[cfg(feature = "egui")]
pub mod file_options;

#[cfg(feature = "egui")]
pub mod interaction;
#[cfg(feature = "egui")]
pub mod secondary;

#[cfg(feature = "egui")]
pub mod molecule_edit;

#[cfg(feature = "egui")]
pub mod morph;

#[cfg(feature = "egui")]
pub mod superposition;
#[cfg(feature = "egui")]
pub mod surface;

#[cfg(feature = "egui")]
pub mod renderer;
