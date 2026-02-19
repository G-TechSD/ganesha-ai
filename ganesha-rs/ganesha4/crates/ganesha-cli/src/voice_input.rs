//! Voice input handling with keyboard controls.
//!
//! Provides push-to-talk (PTT) and conversation mode voice input
//! using CTRL key for safety - prevents accidental triggers from
//! ambient noise, TV, phone calls, etc.
//!
//! # Modes
//! - **Push-to-talk**: Hold CTRL to record, release to send
//! - **Conversation**: Double-tap CTRL to toggle continuous listening

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, KeyEventKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use colored::Colorize;

/// Voice input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMode {
    /// Push-to-talk: Hold CTRL to record
    PushToTalk,
    /// Conversation: Double-tap CTRL to toggle continuous listening
    Conversation,
}

/// Voice input events
#[derive(Debug, Clone)]
pub enum VoiceInputEvent {
    /// Start recording (CTRL pressed in PTT mode)
    StartRecording,
    /// Stop recording and transcribe (CTRL released in PTT mode)
    StopAndTranscribe,
    /// Toggle conversation mode (double-tap CTRL)
    ToggleConversation,
    /// Conversation mode enabled
    ConversationEnabled,
    /// Conversation mode disabled
    ConversationDisabled,
    /// Cancel current operation (ESC)
    Cancel,
    /// Exit voice mode (Ctrl+C)
    Exit,
}

/// Push-to-talk voice input handler
pub struct VoicePTT {
    is_recording: Arc<AtomicBool>,
    last_ctrl_press: Option<Instant>,
    double_tap_threshold: Duration,
    conversation_active: Arc<AtomicBool>,
}

impl VoicePTT {
    /// Create a new push-to-talk handler
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            last_ctrl_press: None,
            double_tap_threshold: Duration::from_millis(400),
            conversation_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Check if conversation mode is active
    pub fn is_conversation_active(&self) -> bool {
        self.conversation_active.load(Ordering::SeqCst)
    }

    /// Print instructions for voice mode
    pub fn print_instructions(&self) {
        println!();
        println!("{}", "Voice Input Mode".bright_cyan().bold());
        println!("{}", "─".repeat(40).dimmed());
        println!("  {} Hold to record, release to send", "CTRL".bright_yellow());
        println!("  {} Toggle conversation mode", "Double-tap CTRL".bright_yellow());
        println!("  {} Cancel current recording", "ESC".bright_yellow());
        println!("  {} Exit voice mode", "Ctrl+C".bright_yellow());
        println!("{}", "─".repeat(40).dimmed());
        println!();
    }

    /// Run the voice input loop
    /// Returns events through the channel
    pub async fn run(&mut self, event_tx: mpsc::Sender<VoiceInputEvent>) -> anyhow::Result<()> {
        use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

        // Enable raw mode for key detection
        enable_raw_mode()?;

        let result = self.event_loop(event_tx).await;

        // Always restore terminal
        disable_raw_mode()?;

        result
    }

    async fn event_loop(&mut self, event_tx: mpsc::Sender<VoiceInputEvent>) -> anyhow::Result<()> {
        loop {
            // Poll for events with timeout
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key_event) = event::read()? {
                    match self.handle_key_event(key_event) {
                        Some(VoiceInputEvent::Exit) => {
                            let _ = event_tx.send(VoiceInputEvent::Exit).await;
                            break;
                        }
                        Some(event) => {
                            let _ = event_tx.send(event).await;
                        }
                        None => {}
                    }
                }
            }

            // Small yield to prevent busy loop
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Option<VoiceInputEvent> {
        // Handle Ctrl+C for exit
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
            return Some(VoiceInputEvent::Exit);
        }

        // Handle ESC for cancel
        if event.code == KeyCode::Esc {
            if self.is_recording.load(Ordering::SeqCst) {
                self.is_recording.store(false, Ordering::SeqCst);
            }
            return Some(VoiceInputEvent::Cancel);
        }

        // Handle CTRL key for push-to-talk and double-tap
        match event.code {
            KeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftControl) |
            KeyCode::Modifier(crossterm::event::ModifierKeyCode::RightControl) => {
                match event.kind {
                    KeyEventKind::Press => {
                        // Check for double-tap
                        let now = Instant::now();
                        if let Some(last) = self.last_ctrl_press {
                            if now.duration_since(last) < self.double_tap_threshold {
                                // Double-tap detected - toggle conversation mode
                                let was_active = self.conversation_active.load(Ordering::SeqCst);
                                self.conversation_active.store(!was_active, Ordering::SeqCst);
                                self.last_ctrl_press = None;

                                if was_active {
                                    return Some(VoiceInputEvent::ConversationDisabled);
                                } else {
                                    return Some(VoiceInputEvent::ConversationEnabled);
                                }
                            }
                        }
                        self.last_ctrl_press = Some(now);

                        // Start recording (PTT mode)
                        if !self.conversation_active.load(Ordering::SeqCst) {
                            self.is_recording.store(true, Ordering::SeqCst);
                            return Some(VoiceInputEvent::StartRecording);
                        }
                    }
                    KeyEventKind::Release => {
                        // Stop recording (PTT mode)
                        if self.is_recording.load(Ordering::SeqCst) {
                            self.is_recording.store(false, Ordering::SeqCst);
                            return Some(VoiceInputEvent::StopAndTranscribe);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        None
    }
}

impl Default for VoicePTT {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple key press detection without raw mode
/// Used for checking if CTRL is currently held
pub fn is_ctrl_held() -> bool {
    if event::poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(key)) = event::read() {
            return key.modifiers.contains(KeyModifiers::CONTROL);
        }
    }
    false
}

/// Wait for CTRL key press (blocking)
/// Returns true if CTRL was pressed, false if cancelled/timeout
pub fn wait_for_ctrl_press(timeout: Duration) -> bool {
    let start = Instant::now();

    while start.elapsed() < timeout {
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.modifiers.contains(KeyModifiers::CONTROL) ||
                   matches!(key.code, KeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftControl) |
                                     KeyCode::Modifier(crossterm::event::ModifierKeyCode::RightControl)) {
                    return true;
                }
                // ESC cancels
                if key.code == KeyCode::Esc {
                    return false;
                }
            }
        }
    }

    false
}

/// Wait for CTRL key release
pub fn wait_for_ctrl_release(timeout: Duration) -> bool {
    let start = Instant::now();

    while start.elapsed() < timeout {
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.kind == KeyEventKind::Release {
                    if matches!(key.code, KeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftControl) |
                                         KeyCode::Modifier(crossterm::event::ModifierKeyCode::RightControl)) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_ptt_creation() {
        let ptt = VoicePTT::new();
        assert!(!ptt.is_recording());
        assert!(!ptt.is_conversation_active());
    }

    #[test]
    fn test_voice_mode_enum() {
        assert_ne!(VoiceMode::PushToTalk, VoiceMode::Conversation);
    }

    #[test]
    fn test_voice_ptt_default() {
        let ptt = VoicePTT::default();
        assert!(!ptt.is_recording());
        assert!(!ptt.is_conversation_active());
    }

    #[test]
    fn test_voice_ptt_initial_state() {
        let ptt = VoicePTT::new();
        assert!(ptt.last_ctrl_press.is_none());
        assert_eq!(ptt.double_tap_threshold, Duration::from_millis(400));
    }

    #[test]
    fn test_voice_input_event_variants() {
        // Ensure all event variants can be constructed
        let events = vec![
            VoiceInputEvent::StartRecording,
            VoiceInputEvent::StopAndTranscribe,
            VoiceInputEvent::ToggleConversation,
            VoiceInputEvent::ConversationEnabled,
            VoiceInputEvent::ConversationDisabled,
            VoiceInputEvent::Cancel,
            VoiceInputEvent::Exit,
        ];
        assert_eq!(events.len(), 7);
    }

    #[test]
    fn test_voice_mode_equality() {
        assert_eq!(VoiceMode::PushToTalk, VoiceMode::PushToTalk);
        assert_eq!(VoiceMode::Conversation, VoiceMode::Conversation);
        assert_ne!(VoiceMode::PushToTalk, VoiceMode::Conversation);
    }

    #[test]
    fn test_voice_mode_clone() {
        let mode = VoiceMode::PushToTalk;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_voice_mode_copy() {
        let mode = VoiceMode::Conversation;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_recording_flag_atomic() {
        let ptt = VoicePTT::new();
        // Directly test atomic behavior
        ptt.is_recording.store(true, Ordering::SeqCst);
        assert!(ptt.is_recording());
        ptt.is_recording.store(false, Ordering::SeqCst);
        assert!(!ptt.is_recording());
    }

    #[test]
    fn test_conversation_flag_atomic() {
        let ptt = VoicePTT::new();
        ptt.conversation_active.store(true, Ordering::SeqCst);
        assert!(ptt.is_conversation_active());
        ptt.conversation_active.store(false, Ordering::SeqCst);
        assert!(!ptt.is_conversation_active());
    }

    #[test]
    fn test_voice_input_event_debug() {
        let event = VoiceInputEvent::StartRecording;
        let debug = format!("{:?}", event);
        assert!(debug.contains("StartRecording"));
    }

    #[test]
    fn test_voice_input_event_clone() {
        let event = VoiceInputEvent::Cancel;
        let cloned = event.clone();
        assert!(format!("{:?}", cloned).contains("Cancel"));
    }

}
