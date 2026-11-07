//! Shared native, browser, and MCP command dispatch.
pub mod about;
mod advanced;
mod animation;
pub mod animation_graph;
mod animation_value;
pub mod clipboard;
mod commands;
mod downloads;
mod electrostatics;
#[cfg(not(target_arch = "wasm32"))]
pub mod external;
mod file_commands;
#[cfg(not(target_arch = "wasm32"))]
pub mod pov;
pub use file_commands::scene_save_options;
mod view_commands;
pub use downloads::{advance_fetch, load_fetched, prepare_fetch, FetchPlan};
pub mod navigation;
mod nodes;
mod object_export;
pub mod pdb;
pub mod render_history;
pub use object_export::{prepare_save_object, PreparedObjectSave};
mod properties;
pub mod selection;
pub use animation::{
    advance_animation, animation_element_detail, animation_timeline,
    initialize_animation_start_camera, prepare_animation_edit, retain_animation_seed_in_undo,
};
pub use properties::{focus_nodes, render_settings};
pub mod config;
pub mod console;
mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;
#[cfg(not(target_arch = "wasm32"))]
pub mod update;
pub use commands::{
    call_tool, hatch_params, hatch_settings, prepare_render, rendering_params, rendering_settings,
    view_json, PreparedRender,
};
pub use properties::{is_view_property_write, prepare_property_edit};
pub use protocol::{
    tool_changes_scene, tool_is_mutating, tool_records_history, tool_response, tools_list,
    validate_args, Protocol,
};
