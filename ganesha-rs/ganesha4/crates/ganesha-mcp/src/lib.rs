//! # Ganesha MCP
//!
//! Model Context Protocol (MCP) management for Ganesha.
//!
//! ## Overview
//!
//! MCP is a standardized protocol for connecting AI assistants to external
//! tools and data sources. This crate provides:
//!
//! - Server discovery and management
//! - Hot-loading of servers (add/remove without restart)
//! - Multiple transport support (stdio, SSE, HTTP)
//! - Automatic credential prompting
//! - Tool/resource/prompt exposure to the LLM
//!
//! ## Transport Types
//!
//! - **Stdio**: Local process communication via stdin/stdout
//! - **SSE**: Server-Sent Events for streaming from remote servers
//! - **HTTP**: Standard HTTP for stateless remote servers
//!
//! ## Configuration
//!
//! Servers can be configured via:
//! - Global config: `~/.config/ganesha/mcp.toml`
//! - Project config: `.ganesha/mcp.toml`
//! - Runtime API

pub mod types;
pub mod transport;
pub mod server;
pub mod manager;
pub mod config;
pub mod registry;

pub use types::{
    Tool, ToolSchema, Resource, Prompt, PromptArgument,
    ToolCallRequest, ToolCallResponse,
    McpError, Result,
};
pub use transport::{Transport, StdioTransport, SseTransport, HttpTransport};
pub use server::{McpServer, ServerStatus, ServerCapabilities};
pub use manager::McpManager;
pub use config::{McpConfig, ServerConfig};
pub use registry::ServerRegistry;

use thiserror::Error;

/// MCP protocol errors
#[derive(Error, Debug)]
pub enum McpProtocolError {
    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Server not connected: {0}")]
    ServerNotConnected(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Timeout waiting for server: {0}")]
    Timeout(String),

    #[error("Server process error: {0}")]
    ProcessError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Authentication required: {0}")]
    AuthRequired(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
