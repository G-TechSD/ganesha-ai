//! IPC commands for Tauri frontend communication

use serde::{Deserialize, Serialize};

/// Commands that can be invoked from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Command {
    /// Send a message to the AI
    SendMessage { content: String },
    /// Cancel current operation
    Cancel,
    /// Clear conversation history
    ClearHistory,
    /// Change the current model
    SetModel { model: String },
    /// Change risk level
    SetRiskLevel { level: String },
    /// Change personality
    SetPersonality { personality: String },
    /// Start voice recording
    StartVoice,
    /// Stop voice recording
    StopVoice,
    /// Toggle voice mode
    ToggleVoice,
    /// Get current state
    GetState,
    /// Get available models
    GetModels,
    /// Get conversation history
    GetHistory { limit: Option<usize> },
    /// Load a session
    LoadSession { session_id: String },
    /// Save current session
    SaveSession,
    /// List saved sessions
    ListSessions,
    /// Open file in editor
    OpenFile { path: String },
    /// Execute shell command
    ExecuteCommand { command: String },
    /// Get file content
    ReadFile { path: String },
    /// Update settings
    UpdateSettings { settings: serde_json::Value },
    /// Connect to MCP server
    ConnectMcp { server: String },
    /// Disconnect from MCP server
    DisconnectMcp { server: String },
    /// List MCP servers
    ListMcpServers,
}

/// Responses sent back to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Response {
    /// Success with optional data
    Success { data: Option<serde_json::Value> },
    /// Error response
    Error { message: String, code: Option<String> },
    /// State update
    State { state: crate::state::AppState },
    /// AI message chunk (streaming)
    MessageChunk { content: String, done: bool },
    /// Full AI message
    Message { content: String, model: String },
    /// Available models
    Models { models: Vec<ModelInfo> },
    /// Conversation history
    History { messages: Vec<HistoryMessage> },
    /// Session list
    Sessions { sessions: Vec<SessionInfo> },
    /// File content
    FileContent { path: String, content: String },
    /// Command output
    CommandOutput { stdout: String, stderr: String, exit_code: i32 },
    /// MCP server status
    McpStatus { servers: Vec<McpServerInfo> },
}

/// Events emitted to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    /// State changed
    StateChanged { state: crate::state::AppState },
    /// Processing started
    ProcessingStarted,
    /// Processing completed
    ProcessingCompleted,
    /// Voice recording started
    VoiceStarted,
    /// Voice recording stopped
    VoiceStopped,
    /// Voice transcript ready
    VoiceTranscript { text: String },
    /// Message received from AI
    MessageReceived { content: String },
    /// Streaming chunk received
    StreamChunk { content: String },
    /// Error occurred
    Error { message: String },
    /// Tool was called
    ToolCalled { name: String, args: serde_json::Value },
    /// Tool completed
    ToolCompleted { name: String, result: serde_json::Value },
    /// File was modified
    FileModified { path: String },
    /// Session changed
    SessionChanged { session_id: String },
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub tier: String,
    pub context_window: Option<usize>,
}

/// History message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub tokens: Option<usize>,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub working_dir: String,
}

/// MCP server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub status: String,
    pub tools: Vec<String>,
}

/// Command handler trait
#[async_trait::async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle a command and return a response
    async fn handle(&self, command: Command) -> Response;
}

/// Default command handler implementation
pub struct DefaultCommandHandler {
    // Will hold references to the various managers
}

impl DefaultCommandHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl CommandHandler for DefaultCommandHandler {
    async fn handle(&self, command: Command) -> Response {
        match command {
            Command::GetState => {
                // Return current state
                Response::State {
                    state: crate::state::AppState::new(),
                }
            }
            Command::SendMessage { content } => {
                // Process message through AI
                Response::Success {
                    data: Some(serde_json::json!({"status": "processing", "message": content})),
                }
            }
            Command::Cancel => {
                // Cancel current operation
                Response::Success { data: None }
            }
            _ => {
                // Not implemented
                Response::Error {
                    message: "Command not implemented".to_string(),
                    code: Some("NOT_IMPLEMENTED".to_string()),
                }
            }
        }
    }
}

impl Default for DefaultCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}
