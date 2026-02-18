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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_defaults() {
        let state = AppState::new();
        assert!(state.window_visible);
        assert!(!state.border_visible);
        assert!(!state.is_listening);
        assert!(!state.is_processing);
        assert!(!state.is_connected);
        assert_eq!(state.risk_level, "Normal");
        assert_eq!(state.personality, "Professional");
        assert!(state.session_id.is_none());
        assert_eq!(state.message_count, 0);
    }

    #[test]
    fn test_connection_state() {
        let mut state = AppState::new();
        assert!(!state.is_connected);
        state.set_connected(true);
        assert!(state.is_connected);
        state.set_connected(false);
        assert!(!state.is_connected);
    }

    #[test]
    fn test_processing_state() {
        let mut state = AppState::new();
        state.set_processing(true);
        assert!(state.is_processing);
        state.set_processing(false);
        assert!(!state.is_processing);
    }

    #[test]
    fn test_session_lifecycle() {
        let mut state = AppState::new();

        // Start session
        state.start_session("session-123".to_string());
        assert_eq!(state.session_id, Some("session-123".to_string()));
        assert_eq!(state.message_count, 0);

        // Add messages
        state.add_message(100);
        state.add_message(200);
        assert_eq!(state.message_count, 2);
        assert_eq!(state.token_usage.total, 300);

        // End session
        state.end_session();
        assert!(state.session_id.is_none());
    }

    #[test]
    fn test_token_usage_tracking() {
        let mut state = AppState::new();
        state.start_session("test".to_string());
        state.add_message(50);
        state.add_message(75);
        state.add_message(125);
        assert_eq!(state.message_count, 3);
        assert_eq!(state.token_usage.total, 250);
    }

    #[test]
    fn test_state_serialization() {
        let state = AppState::new();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AppState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.risk_level, "Normal");
        assert_eq!(deserialized.window_visible, true);
    }

    #[test]
    fn test_atomic_flags() {
        let flags = AtomicFlags::new();
        assert!(!flags.is_push_to_talk());
        assert!(!flags.is_emergency_stop());
        assert!(flags.is_running());

        flags.set_push_to_talk(true);
        assert!(flags.is_push_to_talk());
        flags.set_push_to_talk(false);
        assert!(!flags.is_push_to_talk());
    }

    #[test]
    fn test_emergency_stop() {
        let flags = AtomicFlags::new();
        assert!(!flags.is_emergency_stop());
        flags.trigger_emergency_stop();
        assert!(flags.is_emergency_stop());
        flags.clear_emergency_stop();
        assert!(!flags.is_emergency_stop());
    }

    #[test]
    fn test_running_flag() {
        let flags = AtomicFlags::new();
        assert!(flags.is_running());
        flags.stop();
        assert!(!flags.is_running());
    }

    #[test]
    fn test_atomic_flags_thread_safe() {
        let flags = AtomicFlags::new();
        let clone = flags.clone();

        // Both should see same state
        flags.set_push_to_talk(true);
        assert!(clone.is_push_to_talk());
    }
}
