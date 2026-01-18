//! # Ganesha Voice
//!
//! Voice interface for Ganesha using OpenAI Realtime API.
//!
//! ## Features
//!
//! - Push-to-talk activation
//! - Multiple voice personalities
//! - Real-time transcription and response
//! - Voice activity detection
//!
//! ## Coming Soon
//!
//! This crate is a placeholder for the voice subsystem.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VoiceError {
    #[error("Audio capture error: {0}")]
    CaptureError(String),

    #[error("Voice API error: {0}")]
    ApiError(String),

    #[error("Transcription error: {0}")]
    TranscriptionError(String),
}

pub type Result<T> = std::result::Result<T, VoiceError>;

/// Voice personality
#[derive(Debug, Clone)]
pub enum Personality {
    /// Default neutral assistant
    Default,
    /// Friendly and casual
    Friendly,
    /// Technical and precise
    Technical,
    /// Brief and to the point
    Terse,
}

/// Voice system placeholder
pub struct VoiceSystem {
    enabled: bool,
    personality: Personality,
}

impl VoiceSystem {
    pub fn new() -> Self {
        Self {
            enabled: false,
            personality: Personality::Default,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_personality(&mut self, personality: Personality) {
        self.personality = personality;
    }
}

impl Default for VoiceSystem {
    fn default() -> Self {
        Self::new()
    }
}
