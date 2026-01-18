//! # Ganesha Voice
//!
//! Voice interface for Ganesha - the Obstacle Remover AI coding assistant.
//!
//! This crate provides a complete voice interface including:
//! - Speech-to-text (STT) via OpenAI Whisper or local whisper.cpp
//! - Text-to-speech (TTS) via OpenAI TTS or ElevenLabs
//! - Voice personalities with different speaking styles
//! - Conversation management with turn-taking and interrupts
//! - Audio recording and playback
//! - Push-to-talk support
//!
//! ## Features
//!
//! - `openai-whisper` - OpenAI Whisper API for speech recognition (default)
//! - `openai-tts` - OpenAI TTS API for speech synthesis (default)
//! - `elevenlabs` - ElevenLabs API for high-quality speech synthesis
//! - `local-whisper` - Local Whisper via whisper.cpp (optional)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ganesha_voice::{VoiceManager, VoiceConfig, VoiceConfigBuilder};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a voice manager with default configuration
//!     let config = VoiceConfigBuilder::new()
//!         .openai_api_key("your-api-key")
//!         .default_personality("friendly")
//!         .build()?;
//!
//!     let mut manager = VoiceManager::new(config).await?;
//!
//!     // Start listening
//!     manager.start_listening()?;
//!
//!     // Speak with a personality
//!     manager.speak("Hello! How can I help you today?").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Personalities
//!
//! Built-in personalities include:
//! - **Professional** - Clear, concise, business-like
//! - **Friendly** - Casual, encouraging, uses appropriate humor
//! - **Mentor** - Patient, educational, explains concepts
//! - **Pirate** - Fun novelty voice (arr matey!)
//!
//! Custom personalities can be loaded from TOML configuration files.

pub mod config;
pub mod conversation;
pub mod input;
pub mod output;
pub mod personality;
pub mod setup;

pub use config::{VoiceConfig, VoiceConfigBuilder};
pub use conversation::{ConversationEvent, ConversationState, VoiceConversation};
pub use input::{AudioData, AudioRecorder, TranscriptionResult, VoiceInput, VoiceInputEvent, WhisperInput, LocalWhisperInput};
pub use output::{AudioPlayer, OpenAITTS, ElevenLabsTTS, PiperTTS, OpenAIVoice, SpeechAudio, VoiceOutput, VoiceOutputEvent};
pub use setup::{VoiceModels, VoiceSetupStatus, download_whisper_model, download_piper_voice, WHISPER_MODELS, PIPER_VOICES};
pub use personality::{BuiltInPersonalities, Personality, PersonalityManager};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

use thiserror::Error;

/// Voice system error types
#[derive(Error, Debug)]
pub enum VoiceError {
    /// Audio capture or playback error
    #[error("Audio error: {0}")]
    AudioError(String),

    /// Voice API error (Whisper, TTS, etc.)
    #[error("API error: {0}")]
    ApiError(String),

    /// Transcription error
    #[error("Transcription error: {0}")]
    TranscriptionError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Feature not enabled
    #[error("Feature disabled: {0}")]
    FeatureDisabled(String),

    /// Conversation error
    #[error("Conversation error: {0}")]
    ConversationError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for voice operations
pub type Result<T> = std::result::Result<T, VoiceError>;

/// Voice manager event
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// Voice system initialized
    Initialized,
    /// Listening started
    ListeningStarted,
    /// Listening stopped
    ListeningStopped,
    /// Voice activity detected
    VoiceActivityDetected,
    /// User finished speaking
    UserFinishedSpeaking { text: String },
    /// Assistant started speaking
    AssistantStartedSpeaking,
    /// Assistant finished speaking
    AssistantFinishedSpeaking,
    /// User interrupted
    UserInterrupted,
    /// Audio level update
    AudioLevel { level: f32 },
    /// Error occurred
    Error { message: String },
}

/// Main interface for the voice system
pub struct VoiceManager {
    config: VoiceConfig,
    recorder: Option<AudioRecorder>,
    player: Option<AudioPlayer>,
    whisper: Option<WhisperInput>,
    tts: Option<Box<dyn VoiceOutput>>,
    personality_manager: PersonalityManager,
    conversation: VoiceConversation,
    is_listening: Arc<AtomicBool>,
    is_speaking: Arc<AtomicBool>,
    event_tx: Option<mpsc::Sender<VoiceEvent>>,
}

impl VoiceManager {
    /// Create a new voice manager with the given configuration
    pub async fn new(config: VoiceConfig) -> Result<Self> {
        config.validate()?;

        // Initialize audio recorder
        let recorder = if config.enabled {
            match &config.input.device {
                Some(device) => AudioRecorder::with_device(device).ok(),
                None => AudioRecorder::new().ok(),
            }
        } else {
            None
        };

        // Initialize audio player
        let player = if config.enabled {
            AudioPlayer::new().ok()
        } else {
            None
        };

        // Initialize Whisper
        let whisper = if config.enabled {
            config.api_keys.get_openai_key().map(WhisperInput::new)
        } else {
            None
        };

        // Initialize TTS
        let tts: Option<Box<dyn VoiceOutput>> = if config.enabled {
            match config.output.tts_provider {
                personality::TTSProvider::OpenAI => {
                    config.api_keys.get_openai_key().map(|key| {
                        let tts = OpenAITTS::new(key)
                            .with_voice(config.output.openai_voice)
                            .with_model(&config.output.openai_model)
                            .with_speed(config.output.speed);
                        Box::new(tts) as Box<dyn VoiceOutput>
                    })
                }
                personality::TTSProvider::ElevenLabs => {
                    config.api_keys.get_elevenlabs_key().map(|key| {
                        let mut tts = ElevenLabsTTS::new(key);
                        if let Some(ref voice_id) = config.output.elevenlabs_voice_id {
                            tts.set_voice_id(voice_id);
                        }
                        Box::new(tts) as Box<dyn VoiceOutput>
                    })
                }
            }
        } else {
            None
        };

        // Initialize personality manager
        let mut personality_manager = PersonalityManager::new();
        personality_manager.set_current(&config.personality.default_personality)?;

        // Load custom personalities if configured
        if let Some(ref dir) = config.personality.custom_personalities_dir {
            if dir.exists() {
                match personality_manager.load_from_directory(dir).await {
                    Ok(count) => info!("Loaded {} custom personalities", count),
                    Err(e) => warn!("Failed to load custom personalities: {}", e),
                }
            }
        }

        // Initialize conversation
        let conversation_config = conversation::ConversationConfig {
            allow_interruptions: config.advanced.allow_interruptions,
            end_of_turn_silence: std::time::Duration::from_millis(
                config.input.vad.silence_duration_ms,
            ),
            max_turn_duration: std::time::Duration::from_secs(
                config.input.vad.max_recording_duration_secs,
            ),
            max_history_turns: config.advanced.max_history_turns,
            auto_listen: config.advanced.auto_listen,
            min_transcription_confidence: 0.0,
        };
        let conversation = VoiceConversation::new(conversation_config);

        info!(
            "Voice manager initialized (enabled: {}, TTS: {:?})",
            config.enabled, config.output.tts_provider
        );

        Ok(Self {
            config,
            recorder,
            player,
            whisper,
            tts,
            personality_manager,
            conversation,
            is_listening: Arc::new(AtomicBool::new(false)),
            is_speaking: Arc::new(AtomicBool::new(false)),
            event_tx: None,
        })
    }

    /// Set the event channel for receiving voice events
    pub fn set_event_channel(&mut self, tx: mpsc::Sender<VoiceEvent>) {
        self.event_tx = Some(tx);
    }

    /// Check if voice is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if currently listening
    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }

    /// Check if currently speaking
    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst)
    }

    /// Get the current personality
    pub fn current_personality(&self) -> &Personality {
        self.personality_manager.current()
    }

    /// Set the current personality
    pub fn set_personality(&mut self, id: &str) -> Result<()> {
        self.personality_manager.set_current(id)
    }

    /// Get the personality manager
    pub fn personality_manager(&self) -> &PersonalityManager {
        &self.personality_manager
    }

    /// Get a mutable reference to the personality manager
    pub fn personality_manager_mut(&mut self) -> &mut PersonalityManager {
        &mut self.personality_manager
    }

    /// Get the conversation
    pub fn conversation(&self) -> &VoiceConversation {
        &self.conversation
    }

    /// Start listening for voice input
    pub fn start_listening(&self) -> Result<()> {
        if !self.config.enabled {
            return Err(VoiceError::ConfigError("Voice is not enabled".to_string()));
        }

        let recorder = self.recorder.as_ref().ok_or_else(|| {
            VoiceError::AudioError("Audio recorder not initialized".to_string())
        })?;

        if self.is_listening.load(Ordering::SeqCst) {
            return Ok(()); // Already listening
        }

        recorder.start_recording(None)?;
        self.is_listening.store(true, Ordering::SeqCst);

        self.emit_event(VoiceEvent::ListeningStarted);
        info!("Started listening");

        Ok(())
    }

    /// Stop listening for voice input
    pub fn stop_listening(&self) -> Result<Option<AudioData>> {
        if !self.is_listening.load(Ordering::SeqCst) {
            return Ok(None);
        }

        let recorder = self.recorder.as_ref().ok_or_else(|| {
            VoiceError::AudioError("Audio recorder not initialized".to_string())
        })?;

        let audio = recorder.stop_recording()?;
        self.is_listening.store(false, Ordering::SeqCst);

        self.emit_event(VoiceEvent::ListeningStopped);
        info!("Stopped listening, captured {} samples", audio.samples.len());

        Ok(Some(audio))
    }

    /// Record audio with voice activity detection
    pub async fn record_with_vad(&self) -> Result<AudioData> {
        if !self.config.enabled {
            return Err(VoiceError::ConfigError("Voice is not enabled".to_string()));
        }

        let recorder = self.recorder.as_ref().ok_or_else(|| {
            VoiceError::AudioError("Audio recorder not initialized".to_string())
        })?;

        self.is_listening.store(true, Ordering::SeqCst);
        self.emit_event(VoiceEvent::ListeningStarted);

        let (tx, mut rx) = mpsc::channel(32);

        // Forward events
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Some(ref tx) = event_tx {
                    match event {
                        VoiceInputEvent::VoiceActivityDetected => {
                            let _ = tx.send(VoiceEvent::VoiceActivityDetected).await;
                        }
                        VoiceInputEvent::AudioLevel { level } => {
                            let _ = tx.send(VoiceEvent::AudioLevel { level }).await;
                        }
                        _ => {}
                    }
                }
            }
        });

        let audio = recorder.record_with_vad(tx).await?;

        self.is_listening.store(false, Ordering::SeqCst);
        self.emit_event(VoiceEvent::ListeningStopped);

        Ok(audio)
    }

    /// Transcribe audio to text
    pub async fn transcribe(&self, audio: &AudioData) -> Result<TranscriptionResult> {
        let whisper = self.whisper.as_ref().ok_or_else(|| {
            VoiceError::ConfigError("Whisper not configured".to_string())
        })?;

        whisper.transcribe(audio).await
    }

    /// Speak text using TTS
    pub async fn speak(&self, text: &str) -> Result<()> {
        self.speak_with_personality(text, self.personality_manager.current())
            .await
    }

    /// Speak text using a specific personality
    pub async fn speak_with_personality(&self, text: &str, personality: &Personality) -> Result<()> {
        if !self.config.enabled {
            return Err(VoiceError::ConfigError("Voice is not enabled".to_string()));
        }

        let tts = self.tts.as_ref().ok_or_else(|| {
            VoiceError::ConfigError("TTS not configured".to_string())
        })?;

        let player = self.player.as_ref().ok_or_else(|| {
            VoiceError::AudioError("Audio player not initialized".to_string())
        })?;

        self.is_speaking.store(true, Ordering::SeqCst);
        self.emit_event(VoiceEvent::AssistantStartedSpeaking);

        // Apply personality text modifications
        let modified_text = personality.apply_to_text(text);

        // Generate speech
        let audio = tts.synthesize(&modified_text).await?;

        // Play audio
        player.play_and_wait(&audio, None).await?;

        self.is_speaking.store(false, Ordering::SeqCst);
        self.emit_event(VoiceEvent::AssistantFinishedSpeaking);

        Ok(())
    }

    /// Stop current speech playback
    pub fn stop_speaking(&self) {
        if let Some(ref player) = self.player {
            player.stop();
            self.is_speaking.store(false, Ordering::SeqCst);
            self.emit_event(VoiceEvent::UserInterrupted);
        }
    }

    /// Generate speech audio without playing it
    pub async fn generate_speech(&self, text: &str) -> Result<SpeechAudio> {
        let tts = self.tts.as_ref().ok_or_else(|| {
            VoiceError::ConfigError("TTS not configured".to_string())
        })?;

        let personality = self.personality_manager.current();
        let modified_text = personality.apply_to_text(text);

        tts.synthesize(&modified_text).await
    }

    /// Set the OpenAI voice
    pub fn set_openai_voice(&mut self, voice: OpenAIVoice) {
        // Note: This only updates the config. For dynamic voice changes,
        // the TTS engine would need to be recreated or support dynamic updates.
        self.config.output.openai_voice = voice;
    }

    /// Set volume
    pub fn set_volume(&mut self, volume: f32) {
        self.config.output.volume = volume.clamp(0.0, 1.0);
        if let Some(ref mut player) = self.player {
            player.set_volume(self.config.output.volume);
        }
    }

    /// Set playback speed
    pub fn set_speed(&mut self, speed: f32) {
        self.config.output.speed = speed.clamp(0.25, 4.0);
        if let Some(ref mut player) = self.player {
            player.set_speed(self.config.output.speed);
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &VoiceConfig {
        &self.config
    }

    /// List available input devices
    pub fn list_input_devices() -> Result<Vec<String>> {
        input::list_input_devices()
    }

    /// List available output devices
    pub fn list_output_devices() -> Result<Vec<String>> {
        output::list_output_devices()
    }

    /// Emit a voice event
    fn emit_event(&self, event: VoiceEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(event);
        }
    }

    /// Run a complete voice interaction cycle
    ///
    /// 1. Listen for user input
    /// 2. Transcribe speech to text
    /// 3. Call the response generator
    /// 4. Speak the response
    pub async fn run_interaction<F, Fut>(
        &self,
        generate_response: F,
    ) -> Result<(String, String)>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        // Record audio with VAD
        let audio = self.record_with_vad().await?;

        // Transcribe
        let transcription = self.transcribe(&audio).await?;
        let user_text = transcription.text;

        self.emit_event(VoiceEvent::UserFinishedSpeaking {
            text: user_text.clone(),
        });

        // Generate response
        let response = generate_response(user_text.clone()).await?;

        // Speak response
        self.speak(&response).await?;

        Ok((user_text, response))
    }
}

// Implement Default for VoiceManager using async initialization would require
// a different approach, so we'll skip Default implementation

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_error_display() {
        let err = VoiceError::AudioError("test error".to_string());
        assert!(err.to_string().contains("Audio error"));

        let err = VoiceError::ApiError("api failed".to_string());
        assert!(err.to_string().contains("API error"));
    }

    #[tokio::test]
    async fn test_voice_manager_disabled() {
        let config = VoiceConfigBuilder::new()
            .enabled(false)
            .build()
            .unwrap();

        let manager = VoiceManager::new(config).await.unwrap();
        assert!(!manager.is_enabled());
        assert!(manager.start_listening().is_err());
    }

    #[test]
    fn test_voice_config_validation() {
        let config = VoiceConfig::default();
        assert!(config.validate().is_ok());
    }
}
