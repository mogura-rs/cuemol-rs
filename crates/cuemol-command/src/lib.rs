//! Shared native, browser, and MCP command dispatch.
mod commands;
pub mod config;
mod protocol;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;
pub use commands::{call_tool, view_json};
pub use protocol::{tool_is_mutating, tools_list, Protocol};
