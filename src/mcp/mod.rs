//! Model Context Protocol (MCP) server for gdeye.
//!
//! This module provides an MCP server that exposes gdeye's linting and
//! formatting capabilities to AI assistants.
//!
//! # Available Tools
//!
//! - `format_snippet` - Format GDScript source code
//! - `lint_snippet` - Lint GDScript code and return diagnostics
//! - `list_rules` - List all available lint rules
//! - `get_rule_info` - Get details about a specific rule
//!
//! # Example
//!
//! ```ignore
//! use gdeye::mcp::run_server;
//!
//! #[tokio::main]
//! async fn main() {
//!     run_server().await;
//! }
//! ```

mod server;
mod types;

pub use server::GdeyeMcpServer;
pub use types::*;

use rmcp::transport::stdio;
use rmcp::ServiceExt;

/// Run the MCP server on stdin/stdout.
pub async fn run_server() {
    let service = GdeyeMcpServer::new();
    let server = service.serve(stdio()).await;

    match server {
        Ok(s) => {
            if let Err(e) = s.waiting().await {
                eprintln!("MCP server error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to start MCP server: {}", e);
        }
    }
}
