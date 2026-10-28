//! Shared native, browser, and MCP command dispatch.
mod advanced;
mod animation;
mod commands;
mod downloads;
mod electrostatics;
#[cfg(not(target_arch = "wasm32"))]
pub mod external;
mod file_commands;
pub use downloads::{load_fetched, prepare_fetch, FetchPlan};
mod nodes;
mod properties;
pub use animation::advance_animation;
pub use properties::render_settings;
pub mod config;
mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;
#[cfg(not(target_arch = "wasm32"))]
pub mod update;
pub use commands::{
    call_tool, hatch_params, hatch_settings, prepare_render, rendering_params, rendering_settings,
    view_json, PreparedRender,
};
pub use protocol::{tool_is_mutating, tool_records_history, tools_list, validate_args, Protocol};
