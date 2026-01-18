//! Application state management

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Application runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// Window visibility
    pub window_visible: bool,
    /// Border overlay visibility
    pub border_visible: bool,
    /// Voice listening active
    pub is_listening: bool,
    /// Currently processing a request
    pub is_processing: bool,
    /// Connected to backend
    pub is_connected: bool,
    /// Current model name
    pub current_model: String,
    /// Current risk level
    pub risk_level: String,
    /// Active personality
    pub personality: String,
    /// Session ID
    pub session_id: Option<String>,
    /// Message count
    pub message_count: usize,
    /// Token usage
    pub token_usage: TokenUsage,
}

impl AppState {
    /// Create new default state
    pub fn new() -> Self {
        Self::default()
    }

    /// Update connection status
    pub fn set_connected(&mut self, connected: bool) {
        self.is_connected = connected;
    }

    /// Update processing status
    pub fn set_processing(&mut self, processing: bool) {
        self.is_processing = processing;
    }

    /// Start a new session
    pub fn start_session(&mut self, session_id: String) {
        self.session_id = Some(session_id);
        self.message_count = 0;
        self.token_usage = TokenUsage::default();
    }

    /// End current session
    pub fn end_session(&mut self) {
        self.session_id = None;
    }

    /// Increment message count
    pub fn add_message(&mut self, tokens: usize) {
        self.message_count += 1;
        self.token_usage.total += tokens;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            window_visible: true,
            border_visible: false,
            is_listening: false,
            is_processing: false,
            is_connected: false,
            current_model: "unknown".to_string(),
            risk_level: "Normal".to_string(),
            personality: "Professional".to_string(),
            session_id: None,
            message_count: 0,
            token_usage: TokenUsage::default(),
        }
    }
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Total tokens used
    pub total: usize,
    /// Input tokens
    pub input: usize,
    /// Output tokens
    pub output: usize,
    /// Cached tokens
    pub cached: usize,
}

/// Thread-safe state flags for hotkey handlers
#[derive(Debug, Clone)]
pub struct AtomicFlags {
    /// Push-to-talk active
    pub push_to_talk: Arc<AtomicBool>,
    /// Emergency stop requested
    pub emergency_stop: Arc<AtomicBool>,
    /// Application running
    pub running: Arc<AtomicBool>,
}

impl AtomicFlags {
    pub fn new() -> Self {
        Self {
            push_to_talk: Arc::new(AtomicBool::new(false)),
            emergency_stop: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_push_to_talk(&self) -> bool {
        self.push_to_talk.load(Ordering::SeqCst)
    }

    pub fn set_push_to_talk(&self, value: bool) {
        self.push_to_talk.store(value, Ordering::SeqCst);
    }

    pub fn is_emergency_stop(&self) -> bool {
        self.emergency_stop.load(Ordering::SeqCst)
    }

    pub fn trigger_emergency_stop(&self) {
        self.emergency_stop.store(true, Ordering::SeqCst);
    }

    pub fn clear_emergency_stop(&self) {
        self.emergency_stop.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for AtomicFlags {
    fn default() -> Self {
        Self::new()
    }
}
