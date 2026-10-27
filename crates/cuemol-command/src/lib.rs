//! Shared native, browser, and MCP command dispatch.
mod advanced;
mod animation;
mod commands;
mod electrostatics;
#[cfg(not(target_arch = "wasm32"))]
pub mod external;
mod file_commands;
mod nodes;
mod properties;
pub use animation::advance_animation;
pub mod config;
mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;
pub use commands::{call_tool, view_json};
pub use protocol::{tool_is_mutating, tool_records_history, tools_list, Protocol};
