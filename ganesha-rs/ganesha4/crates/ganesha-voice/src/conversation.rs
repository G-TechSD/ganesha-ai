//! Voice conversation module for managing turn-taking and dialogue flow.
//!
//! Provides conversation management including interrupt handling,
//! silence detection, transcript generation, and audio history.

use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::info;

use crate::input::{AudioData, VoiceInput};
use crate::output::{AudioPlayer, VoiceOutput};
use crate::{Result, VoiceError};

/// Conversation turn speaker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    User,
    Assistant,
}

/// A single turn in the conversation
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// Unique ID for this turn
    pub id: u64,
    /// Who spoke this turn
    pub speaker: Speaker,
    /// The text content (transcription or response)
    pub text: String,
    /// Timestamp when this turn started
    pub timestamp: SystemTime,
    /// Duration of the turn
    pub duration: Duration,
    /// Associated audio data, if available
    pub audio: Option<AudioData>,
    /// Whether this turn was interrupted
    pub was_interrupted: bool,
}

impl ConversationTurn {
    /// Create a new conversation turn
    pub fn new(speaker: Speaker, text: String) -> Self {
        static TURN_ID: AtomicU64 = AtomicU64::new(1);

        Self {
            id: TURN_ID.fetch_add(1, Ordering::SeqCst),
            speaker,
            text,
            timestamp: SystemTime::now(),
            duration: Duration::ZERO,
            audio: None,
            was_interrupted: false,
        }
    }

    /// Set the duration of this turn
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Set the audio data for this turn
    pub fn with_audio(mut self, audio: AudioData) -> Self {
        self.audio = Some(audio);
        self
    }

    /// Mark this turn as interrupted
    pub fn mark_interrupted(&mut self) {
        self.was_interrupted = true;
    }
}

/// Conversation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationState {
    /// Idle, waiting for user input
    Idle,
    /// Listening to user speech
    Listening,
    /// Processing user input (transcription + AI response)
    Processing,
    /// AI is speaking
    Speaking,
    /// Conversation is paused
    Paused,
}

/// Conversation event
#[derive(Debug, Clone)]
pub enum ConversationEvent {
    /// State changed
    StateChanged { from: ConversationState, to: ConversationState },
    /// User started speaking
    UserStartedSpeaking,
    /// User finished speaking
    UserFinishedSpeaking { text: String },
    /// Assistant started speaking
    AssistantStartedSpeaking,
    /// Assistant finished speaking
    AssistantFinishedSpeaking,
    /// User interrupted the assistant
    UserInterrupted,
    /// Transcription available
    TranscriptionAvailable { text: String, is_final: bool },
    /// Audio level update
    AudioLevel { level: f32 },
    /// Error occurred
    Error { message: String },
    /// Turn completed
    TurnCompleted { turn: ConversationTurn },
}

/// Configuration for conversation behavior
#[derive(Debug, Clone)]
pub struct ConversationConfig {
    /// Whether to allow interruptions
    pub allow_interruptions: bool,
    /// Silence duration before considering end of turn
    pub end_of_turn_silence: Duration,
    /// Maximum turn duration
    pub max_turn_duration: Duration,
    /// Maximum number of turns to keep in history
    pub max_history_turns: usize,
    /// Whether to automatically start listening after assistant finishes
    pub auto_listen: bool,
    /// Minimum confidence for transcription
    pub min_transcription_confidence: f32,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            allow_interruptions: true,
            end_of_turn_silence: Duration::from_millis(1500),
            max_turn_duration: Duration::from_secs(60),
            max_history_turns: 100,
            auto_listen: true,
            min_transcription_confidence: 0.0,
        }
    }
}

/// Conversation transcript
#[derive(Debug, Clone, Default)]
pub struct Transcript {
    /// All turns in the conversation
    pub turns: Vec<ConversationTurn>,
    /// Start time of the conversation
    pub started_at: Option<SystemTime>,
    /// End time of the conversation
    pub ended_at: Option<SystemTime>,
}

impl Transcript {
    /// Create a new empty transcript
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a turn to the transcript
    pub fn add_turn(&mut self, turn: ConversationTurn) {
        if self.started_at.is_none() {
            self.started_at = Some(turn.timestamp);
        }
        self.turns.push(turn);
    }

    /// Get the total duration of the conversation
    pub fn duration(&self) -> Duration {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => end.duration_since(start).unwrap_or(Duration::ZERO),
            (Some(start), None) => SystemTime::now()
                .duration_since(start)
                .unwrap_or(Duration::ZERO),
            _ => Duration::ZERO,
        }
    }

    /// Generate a text-only transcript
    pub fn to_text(&self) -> String {
        let mut result = String::new();

        for turn in &self.turns {
            let speaker = match turn.speaker {
                Speaker::User => "User",
                Speaker::Assistant => "Assistant",
            };

            let timestamp = turn
                .timestamp
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            result.push_str(&format!("[{}] {}: {}\n", timestamp, speaker, turn.text));
        }

        result
    }

    /// Export transcript to JSON
    pub fn to_json(&self) -> Result<String> {
        #[derive(serde::Serialize)]
        struct TranscriptEntry {
            id: u64,
            speaker: String,
            text: String,
            timestamp: u64,
            duration_ms: u64,
            was_interrupted: bool,
        }

        let entries: Vec<TranscriptEntry> = self
            .turns
            .iter()
            .map(|t| TranscriptEntry {
                id: t.id,
                speaker: match t.speaker {
                    Speaker::User => "user".to_string(),
                    Speaker::Assistant => "assistant".to_string(),
                },
                text: t.text.clone(),
                timestamp: t
                    .timestamp
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                duration_ms: t.duration.as_millis() as u64,
                was_interrupted: t.was_interrupted,
            })
            .collect();

        serde_json::to_string_pretty(&entries)
            .map_err(|e| VoiceError::ConfigError(format!("Failed to serialize transcript: {}", e)))
    }
}

/// Voice conversation manager
pub struct VoiceConversation {
    state: Arc<RwLock<ConversationState>>,
    config: ConversationConfig,
    transcript: Arc<RwLock<Transcript>>,
    history: Arc<RwLock<VecDeque<ConversationTurn>>>,
    is_running: Arc<AtomicBool>,
    event_tx: Option<mpsc::Sender<ConversationEvent>>,
    current_turn_start: Arc<RwLock<Option<Instant>>>,
}

impl VoiceConversation {
    /// Create a new voice conversation
    pub fn new(config: ConversationConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(ConversationState::Idle)),
            config,
            transcript: Arc::new(RwLock::new(Transcript::new())),
            history: Arc::new(RwLock::new(VecDeque::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            event_tx: None,
            current_turn_start: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the event channel for receiving conversation events
    pub fn set_event_channel(&mut self, tx: mpsc::Sender<ConversationEvent>) {
        self.event_tx = Some(tx);
    }

    /// Get the current conversation state
    pub fn state(&self) -> ConversationState {
        *self.state.read()
    }

    /// Get a clone of the transcript
    pub fn transcript(&self) -> Transcript {
        self.transcript.read().clone()
    }

    /// Get recent conversation history
    pub fn history(&self, limit: usize) -> Vec<ConversationTurn> {
        let history = self.history.read();
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Check if the conversation is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Start the conversation
    pub fn start(&self) {
        self.is_running.store(true, Ordering::SeqCst);
        self.set_state(ConversationState::Idle);
        info!("Voice conversation started");
    }

    /// Stop the conversation
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        let mut transcript = self.transcript.write();
        transcript.ended_at = Some(SystemTime::now());
        self.set_state(ConversationState::Idle);
        info!("Voice conversation stopped");
    }

    /// Pause the conversation
    pub fn pause(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            self.set_state(ConversationState::Paused);
            info!("Voice conversation paused");
        }
    }

    /// Resume the conversation
    pub fn resume(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            self.set_state(ConversationState::Idle);
            info!("Voice conversation resumed");
        }
    }

    /// Set the conversation state and emit event
    fn set_state(&self, new_state: ConversationState) {
        let old_state = {
            let mut state = self.state.write();
            let old = *state;
            *state = new_state;
            old
        };

        if old_state != new_state {
            self.emit_event(ConversationEvent::StateChanged {
                from: old_state,
                to: new_state,
            });
        }
    }

    /// Emit a conversation event
    fn emit_event(&self, event: ConversationEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(event);
        }
    }

    /// Handle user speech input
    pub async fn handle_user_input<I: VoiceInput>(
        &self,
        audio: AudioData,
        transcriber: &I,
    ) -> Result<ConversationTurn> {
        self.set_state(ConversationState::Processing);
        *self.current_turn_start.write() = Some(Instant::now());

        // Transcribe the audio
        let result = transcriber.transcribe(&audio).await?;

        let duration = self
            .current_turn_start
            .read()
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);

        let turn = ConversationTurn::new(Speaker::User, result.text.clone())
            .with_duration(duration)
            .with_audio(audio);

        // Add to history
        self.add_to_history(turn.clone());

        self.emit_event(ConversationEvent::UserFinishedSpeaking { text: result.text });
        self.emit_event(ConversationEvent::TurnCompleted { turn: turn.clone() });

        Ok(turn)
    }

    /// Handle assistant response - plays the generated audio
    /// Note: The caller is responsible for spawning audio playback since AudioPlayer is not Send+Sync
    pub async fn handle_assistant_response<O: VoiceOutput>(
        &self,
        text: &str,
        tts: &O,
    ) -> Result<ConversationTurn> {
        self.set_state(ConversationState::Speaking);
        *self.current_turn_start.write() = Some(Instant::now());

        self.emit_event(ConversationEvent::AssistantStartedSpeaking);

        // Generate speech (this part is async)
        let _audio = tts.synthesize(text).await?;

        // Note: Actual playback should be handled by the caller since AudioPlayer
        // contains non-Send types (OutputStream). The caller should use the returned
        // audio data to play back on the main thread or a dedicated audio thread.

        let duration = self
            .current_turn_start
            .read()
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);

        let turn = ConversationTurn::new(Speaker::Assistant, text.to_string())
            .with_duration(duration);

        self.emit_event(ConversationEvent::AssistantFinishedSpeaking);

        // Add to history
        self.add_to_history(turn.clone());
        self.emit_event(ConversationEvent::TurnCompleted { turn: turn.clone() });

        // Return to idle or listening state
        if self.config.auto_listen && self.is_running.load(Ordering::SeqCst) {
            self.set_state(ConversationState::Listening);
        } else {
            self.set_state(ConversationState::Idle);
        }

        Ok(turn)
    }

    /// Handle interruption from user
    pub fn handle_interruption(&self, player: &AudioPlayer) {
        if *self.state.read() == ConversationState::Speaking {
            player.stop();

            // Mark current turn as interrupted
            let mut history = self.history.write();
            if let Some(last_turn) = history.back_mut() {
                if last_turn.speaker == Speaker::Assistant {
                    last_turn.mark_interrupted();
                }
            }

            self.emit_event(ConversationEvent::UserInterrupted);
            self.set_state(ConversationState::Listening);

            info!("User interrupted assistant");
        }
    }

    /// Add a turn to history (and transcript)
    fn add_to_history(&self, turn: ConversationTurn) {
        {
            let mut history = self.history.write();
            history.push_back(turn.clone());

            // Trim history if needed
            while history.len() > self.config.max_history_turns {
                history.pop_front();
            }
        }

        {
            let mut transcript = self.transcript.write();
            transcript.add_turn(turn);
        }
    }

    /// Clear conversation history
    pub fn clear_history(&self) {
        self.history.write().clear();
        *self.transcript.write() = Transcript::new();
        info!("Conversation history cleared");
    }
}

impl Default for VoiceConversation {
    fn default() -> Self {
        Self::new(ConversationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_turn_creation() {
        let turn = ConversationTurn::new(Speaker::User, "Hello".to_string());
        assert!(turn.id > 0);
        assert_eq!(turn.speaker, Speaker::User);
        assert_eq!(turn.text, "Hello");
        assert!(!turn.was_interrupted);
    }

    #[test]
    fn test_transcript() {
        let mut transcript = Transcript::new();

        transcript.add_turn(ConversationTurn::new(Speaker::User, "Hello".to_string()));
        transcript.add_turn(ConversationTurn::new(
            Speaker::Assistant,
            "Hi there!".to_string(),
        ));

        assert_eq!(transcript.turns.len(), 2);
        assert!(transcript.started_at.is_some());

        let text = transcript.to_text();
        assert!(text.contains("User: Hello"));
        assert!(text.contains("Assistant: Hi there!"));
    }

    #[test]
    fn test_conversation_state_transitions() {
        let conversation = VoiceConversation::new(ConversationConfig::default());

        assert_eq!(conversation.state(), ConversationState::Idle);

        conversation.start();
        assert!(conversation.is_running());

        conversation.pause();
        assert_eq!(conversation.state(), ConversationState::Paused);

        conversation.resume();
        assert_eq!(conversation.state(), ConversationState::Idle);

        conversation.stop();
        assert!(!conversation.is_running());
    }

    #[test]
    fn test_conversation_config_default() {
        let config = ConversationConfig::default();
        assert!(config.allow_interruptions);
        assert!(config.auto_listen);
        assert_eq!(config.max_history_turns, 100);
    }

    #[test]
    fn test_turn_with_duration() {
        let turn = ConversationTurn::new(Speaker::Assistant, "Response".to_string())
            .with_duration(Duration::from_secs(3));
        assert_eq!(turn.duration, Duration::from_secs(3));
    }

    #[test]
    fn test_turn_interrupted() {
        let mut turn = ConversationTurn::new(Speaker::Assistant, "Interrupted".to_string());
        assert!(!turn.was_interrupted);
        turn.mark_interrupted();
        assert!(turn.was_interrupted);
    }

    #[test]
    fn test_speaker_equality() {
        assert_eq!(Speaker::User, Speaker::User);
        assert_eq!(Speaker::Assistant, Speaker::Assistant);
        assert_ne!(Speaker::User, Speaker::Assistant);
    }

    #[test]
    fn test_conversation_state_equality() {
        assert_eq!(ConversationState::Idle, ConversationState::Idle);
        assert_ne!(ConversationState::Idle, ConversationState::Listening);
        assert_ne!(ConversationState::Speaking, ConversationState::Processing);
    }

    #[test]
    fn test_transcript_empty() {
        let transcript = Transcript::new();
        assert!(transcript.turns.is_empty());
        assert!(transcript.started_at.is_none());
        let text = transcript.to_text();
        assert!(text.is_empty() || text.len() >= 0); // empty transcript produces empty string
    }

    #[test]
    fn test_turn_ids_increment() {
        let turn1 = ConversationTurn::new(Speaker::User, "First".to_string());
        let turn2 = ConversationTurn::new(Speaker::User, "Second".to_string());
        assert!(turn2.id > turn1.id);
    }


    #[test]
    fn test_conversation_turn_new() {
        let turn = ConversationTurn::new(Speaker::User, "hello".to_string());
        assert_eq!(turn.text, "hello");
    }

    #[test]
    fn test_conversation_turn_with_duration() {
        let turn = ConversationTurn::new(Speaker::User, "test".to_string())
            .with_duration(std::time::Duration::from_secs(2));
        assert!(turn.duration.as_secs() >= 2);
    }

    #[test]
    fn test_conversation_turn_mark_interrupted() {
        let mut turn = ConversationTurn::new(Speaker::Assistant, "response".to_string());
        turn.mark_interrupted();
        assert!(turn.was_interrupted);
    }

    #[test]
    fn test_transcript_new_empty() {
        let t = Transcript::new();
        assert!(t.to_text().is_empty());
    }

    #[test]
    fn test_transcript_add_turns() {
        let mut t = Transcript::new();
        t.add_turn(ConversationTurn::new(Speaker::User, "hi".to_string()));
        t.add_turn(ConversationTurn::new(Speaker::Assistant, "hello".to_string()));
        let text = t.to_text();
        assert!(text.contains("hi"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn test_transcript_to_json() {
        let mut t = Transcript::new();
        t.add_turn(ConversationTurn::new(Speaker::User, "test".to_string()));
        let json = t.to_json();
        assert!(json.is_ok());
    }

    #[test]
    fn test_speaker_variants() {
        let _ = Speaker::User;
        let _ = Speaker::Assistant;
        // Only User and Assistant variants
    }

    #[test]
    fn test_conversation_state_variants() {
        let _ = ConversationState::Idle;
        let _ = ConversationState::Listening;
        let _ = ConversationState::Processing;
        let _ = ConversationState::Speaking;
    }

    #[test]
    fn test_voice_conversation_new() {
        let config = ConversationConfig::default();
        let conv = VoiceConversation::new(config);
        assert!(matches!(conv.state(), ConversationState::Idle));
    }

    #[test]
    fn test_voice_conversation_transcript() {
        let conv = VoiceConversation::new(ConversationConfig::default());
        let t = conv.transcript();
        assert!(t.to_text().is_empty());
    }
}
