//! Voice Module - Real-Time Conversational Audio
//!
//! SAFETY: This module is disabled by default and requires explicit opt-in
//! via the `voice` feature flag: `--features voice`
//!
//! This enables Ganesha to "hear" and "speak" in real-time without the
//! clunky transcribe -> prompt -> wait -> TTS loop. Instead, it uses
//! streaming WebSocket connections for true conversational AI.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │                    REAL-TIME VOICE PIPELINE                       │
//! ├──────────────────────────────────────────────────────────────────┤
//! │                                                                   │
//! │   Microphone ──► VAD ──► Audio Chunks ──┐                        │
//! │                                          │                        │
//! │                                          ▼                        │
//! │                                    ┌──────────┐                   │
//! │                                    │ WebSocket│ ◄── Streaming     │
//! │                                    │   API    │     Responses     │
//! │                                    └──────────┘                   │
//! │                                          │                        │
//! │   Speaker ◄── Audio Playback ◄── Audio Chunks                    │
//! │                                                                   │
//! │   ┌─────────────────────────────────────────────────────────┐    │
//! │   │ Turn-Taking: VAD detects silence, signals turn complete │    │
//! │   │ Barge-In: User can interrupt AI response at any time    │    │
//! │   │ Low Latency: ~200ms round-trip target                   │    │
//! │   └─────────────────────────────────────────────────────────┘    │
//! └──────────────────────────────────────────────────────────────────┘
//! ```

#[cfg(feature = "voice")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(feature = "voice")]
use std::sync::mpsc;
#[cfg(feature = "voice")]
use tokio::sync::broadcast;
#[cfg(feature = "voice")]
use tokio_tungstenite::tungstenite::Message;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Global kill switch for voice capabilities
static VOICE_ENABLED: AtomicBool = AtomicBool::new(false);
static VOICE_KILL_SWITCH: AtomicBool = AtomicBool::new(false);

/// Audio configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Sample rate (default 24000 Hz for most voice APIs)
    pub sample_rate: u32,
    /// Channels (1 = mono, 2 = stereo)
    pub channels: u16,
    /// Buffer size in samples
    pub buffer_size: usize,
    /// VAD sensitivity (0.0 - 1.0, higher = more sensitive)
    pub vad_sensitivity: f32,
    /// Silence duration to end turn (milliseconds)
    pub silence_duration_ms: u32,
    /// Enable barge-in (interrupt AI while speaking)
    pub allow_barge_in: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 24000,
            channels: 1,
            buffer_size: 4096,
            vad_sensitivity: 0.5,
            silence_duration_ms: 500,
            allow_barge_in: true,
        }
    }
}

/// Voice stream state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceState {
    /// Idle, not listening or speaking
    Idle,
    /// Listening to user
    Listening,
    /// User is speaking (VAD detected voice)
    UserSpeaking,
    /// Processing/waiting for AI response
    Processing,
    /// AI is speaking
    AiSpeaking,
    /// User interrupted AI (barge-in)
    BargeIn,
}

/// Voice capability status
#[derive(Debug, Clone)]
pub struct VoiceStatus {
    pub feature_compiled: bool,
    pub enabled: bool,
    pub kill_switch_active: bool,
    pub state: VoiceState,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
}

/// Audio chunk for streaming
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// PCM audio data (16-bit signed integers)
    pub data: Vec<i16>,
    /// Sample rate
    pub sample_rate: u32,
    /// Channels
    pub channels: u16,
    /// Whether voice was detected (VAD result)
    pub voice_detected: bool,
    /// Timestamp
    pub timestamp: Instant,
}

/// Voice stream for bidirectional real-time audio
#[derive(Debug, Clone)]
pub struct VoiceStream {
    /// Unique stream ID
    pub id: String,
    /// Current state
    pub state: VoiceState,
    /// Configuration
    pub config: AudioConfig,
}

/// Voice controller with streaming capabilities
pub struct VoiceController {
    /// Whether voice was explicitly enabled by user
    enabled: Arc<AtomicBool>,
    /// Emergency kill switch
    kill_switch: Arc<AtomicBool>,
    /// Current state
    state: std::sync::Mutex<VoiceState>,
    /// Configuration
    config: AudioConfig,
    /// Last activity timestamp for timeout
    last_activity: std::sync::Mutex<Instant>,
    /// Auto-disable timeout (default 10 minutes of inactivity)
    inactivity_timeout: Duration,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
}

impl Default for VoiceController {
    fn default() -> Self {
        Self::new(AudioConfig::default())
    }
}

impl VoiceController {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            kill_switch: Arc::new(AtomicBool::new(false)),
            state: std::sync::Mutex::new(VoiceState::Idle),
            config,
            last_activity: std::sync::Mutex::new(Instant::now()),
            inactivity_timeout: Duration::from_secs(600), // 10 minutes
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Enable voice capabilities (requires user consent)
    pub fn enable(&self) -> Result<(), VoiceError> {
        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(VoiceError::KillSwitchActive);
        }

        #[cfg(not(feature = "voice"))]
        return Err(VoiceError::FeatureNotCompiled);

        #[cfg(feature = "voice")]
        {
            // Check for audio devices
            let host = cpal::default_host();

            if host.default_input_device().is_none() {
                return Err(VoiceError::NoInputDevice);
            }

            if host.default_output_device().is_none() {
                return Err(VoiceError::NoOutputDevice);
            }

            VOICE_ENABLED.store(true, Ordering::SeqCst);
            self.enabled.store(true, Ordering::SeqCst);
            *self.last_activity.lock().unwrap() = Instant::now();
            *self.state.lock().unwrap() = VoiceState::Idle;
            Ok(())
        }
    }

    /// Disable voice capabilities
    pub fn disable(&self) {
        VOICE_ENABLED.store(false, Ordering::SeqCst);
        self.enabled.store(false, Ordering::SeqCst);
        self.shutdown.store(true, Ordering::SeqCst);
        *self.state.lock().unwrap() = VoiceState::Idle;
    }

    /// Activate emergency kill switch
    pub fn activate_kill_switch(&self) {
        VOICE_KILL_SWITCH.store(true, Ordering::SeqCst);
        self.kill_switch.store(true, Ordering::SeqCst);
        self.disable();
    }

    /// Check if voice is currently available
    pub fn is_available(&self) -> bool {
        #[cfg(not(feature = "voice"))]
        return false;

        #[cfg(feature = "voice")]
        {
            self.enabled.load(Ordering::SeqCst)
                && !self.kill_switch.load(Ordering::SeqCst)
                && !self.is_inactive_timeout()
        }
    }

    /// Check if inactive timeout has expired
    fn is_inactive_timeout(&self) -> bool {
        let last = self.last_activity.lock().unwrap();
        last.elapsed() > self.inactivity_timeout
    }

    /// Update activity timestamp
    fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    /// Get current state
    pub fn get_state(&self) -> VoiceState {
        *self.state.lock().unwrap()
    }

    /// Set state
    fn set_state(&self, state: VoiceState) {
        *self.state.lock().unwrap() = state;
    }

    /// Get current status
    pub fn status(&self) -> VoiceStatus {
        #[cfg(feature = "voice")]
        let (input_device, output_device) = {
            let host = cpal::default_host();
            (
                host.default_input_device().map(|d| d.name().unwrap_or_default()),
                host.default_output_device().map(|d| d.name().unwrap_or_default()),
            )
        };

        #[cfg(not(feature = "voice"))]
        let (input_device, output_device) = (None, None);

        VoiceStatus {
            feature_compiled: cfg!(feature = "voice"),
            enabled: self.enabled.load(Ordering::SeqCst),
            kill_switch_active: self.kill_switch.load(Ordering::SeqCst),
            state: self.get_state(),
            input_device,
            output_device,
        }
    }

    /// List available input devices
    #[cfg(feature = "voice")]
    pub fn list_input_devices(&self) -> Result<Vec<String>, VoiceError> {
        let host = cpal::default_host();
        let devices = host
            .input_devices()
            .map_err(|e| VoiceError::DeviceError(e.to_string()))?;

        Ok(devices
            .filter_map(|d| d.name().ok())
            .collect())
    }

    /// List available output devices
    #[cfg(feature = "voice")]
    pub fn list_output_devices(&self) -> Result<Vec<String>, VoiceError> {
        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|e| VoiceError::DeviceError(e.to_string()))?;

        Ok(devices
            .filter_map(|d| d.name().ok())
            .collect())
    }

    /// Start real-time voice conversation
    ///
    /// This creates bidirectional audio streams and connects to the
    /// streaming API for real-time conversation.
    #[cfg(feature = "voice")]
    pub async fn start_conversation(
        &self,
        api_url: &str,
        api_key: &str,
    ) -> Result<ConversationHandle, VoiceError> {
        if !self.is_available() {
            return Err(VoiceError::NotEnabled);
        }

        self.touch();
        self.set_state(VoiceState::Listening);

        // Create channels for audio data
        let (audio_tx, _audio_rx) = broadcast::channel::<AudioChunk>(100);
        let (response_tx, _response_rx) = broadcast::channel::<AudioChunk>(100);

        // Create WebSocket connection to streaming API
        let url = format!("{}?api_key={}", api_url, api_key);
        let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| VoiceError::ConnectionError(e.to_string()))?;

        let (_write, _read) = ws_stream.split();

        // Start audio input stream
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or(VoiceError::NoInputDevice)?;

        let input_config = cpal::StreamConfig {
            channels: self.config.channels,
            sample_rate: cpal::SampleRate(self.config.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(self.config.buffer_size as u32),
        };

        let audio_tx_clone = audio_tx.clone();
        let vad_sensitivity = self.config.vad_sensitivity;

        let input_stream = input_device
            .build_input_stream(
                &input_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Convert f32 to i16
                    let samples: Vec<i16> = data
                        .iter()
                        .map(|&s| (s * 32767.0) as i16)
                        .collect();

                    // Simple VAD: check if any sample exceeds threshold
                    let threshold = (vad_sensitivity * 1000.0) as i16;
                    let voice_detected = samples.iter().any(|&s| s.abs() > threshold);

                    let chunk = AudioChunk {
                        data: samples,
                        sample_rate: 24000,
                        channels: 1,
                        voice_detected,
                        timestamp: Instant::now(),
                    };

                    let _ = audio_tx_clone.send(chunk);
                },
                |err| eprintln!("Audio input error: {}", err),
                None,
            )
            .map_err(|e| VoiceError::StreamError(e.to_string()))?;

        input_stream.play().map_err(|e| VoiceError::StreamError(e.to_string()))?;

        // Start audio output stream
        let output_device = host
            .default_output_device()
            .ok_or(VoiceError::NoOutputDevice)?;

        let output_config = cpal::StreamConfig {
            channels: self.config.channels,
            sample_rate: cpal::SampleRate(self.config.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(self.config.buffer_size as u32),
        };

        let output_stream = output_device
            .build_output_stream(
                &output_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Fill with silence for now (actual implementation would
                    // pull from response channel)
                    for sample in data.iter_mut() {
                        *sample = 0.0;
                    }
                },
                |err| eprintln!("Audio output error: {}", err),
                None,
            )
            .map_err(|e| VoiceError::StreamError(e.to_string()))?;

        output_stream.play().map_err(|e| VoiceError::StreamError(e.to_string()))?;

        Ok(ConversationHandle {
            id: uuid::Uuid::new_v4().to_string(),
            audio_tx,
            response_tx,
            shutdown: self.shutdown.clone(),
        })
    }

    /// Push-to-talk: start listening
    #[cfg(feature = "voice")]
    pub fn push_to_talk_start(&self) -> Result<(), VoiceError> {
        if !self.is_available() {
            return Err(VoiceError::NotEnabled);
        }
        self.touch();
        self.set_state(VoiceState::UserSpeaking);
        Ok(())
    }

    /// Push-to-talk: stop listening
    #[cfg(feature = "voice")]
    pub fn push_to_talk_stop(&self) -> Result<(), VoiceError> {
        if !self.is_available() {
            return Err(VoiceError::NotEnabled);
        }
        self.set_state(VoiceState::Processing);
        Ok(())
    }

    /// Interrupt AI response (barge-in)
    #[cfg(feature = "voice")]
    pub fn interrupt(&self) -> Result<(), VoiceError> {
        if !self.config.allow_barge_in {
            return Err(VoiceError::BargeInDisabled);
        }

        if self.get_state() == VoiceState::AiSpeaking {
            self.set_state(VoiceState::BargeIn);
        }
        Ok(())
    }

    // Stub implementations when feature not compiled
    #[cfg(not(feature = "voice"))]
    pub fn list_input_devices(&self) -> Result<Vec<String>, VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "voice"))]
    pub fn list_output_devices(&self) -> Result<Vec<String>, VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "voice"))]
    pub async fn start_conversation(
        &self,
        _api_url: &str,
        _api_key: &str,
    ) -> Result<ConversationHandle, VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "voice"))]
    pub fn push_to_talk_start(&self) -> Result<(), VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "voice"))]
    pub fn push_to_talk_stop(&self) -> Result<(), VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "voice"))]
    pub fn interrupt(&self) -> Result<(), VoiceError> {
        Err(VoiceError::FeatureNotCompiled)
    }
}

/// Handle to an active conversation
pub struct ConversationHandle {
    /// Unique conversation ID
    pub id: String,
    /// Channel to send audio to API
    #[cfg(feature = "voice")]
    pub audio_tx: broadcast::Sender<AudioChunk>,
    #[cfg(not(feature = "voice"))]
    pub audio_tx: (),
    /// Channel to receive audio from API
    #[cfg(feature = "voice")]
    pub response_tx: broadcast::Sender<AudioChunk>,
    #[cfg(not(feature = "voice"))]
    pub response_tx: (),
    /// Shutdown signal
    pub shutdown: Arc<AtomicBool>,
}

impl ConversationHandle {
    /// Stop the conversation
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// Voice errors
#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    #[error("Voice feature not compiled. Rebuild with --features voice")]
    FeatureNotCompiled,

    #[error("Voice not enabled. Call enable() with user consent first")]
    NotEnabled,

    #[error("Emergency kill switch is active. Restart required")]
    KillSwitchActive,

    #[error("No audio input device found")]
    NoInputDevice,

    #[error("No audio output device found")]
    NoOutputDevice,

    #[error("Audio device error: {0}")]
    DeviceError(String),

    #[error("Audio stream error: {0}")]
    StreamError(String),

    #[error("WebSocket connection error: {0}")]
    ConnectionError(String),

    #[error("Barge-in is disabled in configuration")]
    BargeInDisabled,

    #[error("Inactivity timeout - voice auto-disabled")]
    InactivityTimeout,

    #[error("VAD error: {0}")]
    VadError(String),
}

/// Real-time voice API providers
#[derive(Debug, Clone)]
pub enum VoiceApiProvider {
    /// OpenAI Realtime API
    OpenAiRealtime {
        api_key: String,
        model: String,
    },
    /// Anthropic (when available)
    AnthropicRealtime {
        api_key: String,
    },
    /// Local Whisper + TTS
    LocalWhisperTts {
        whisper_model: String,
        tts_model: String,
    },
    /// Custom WebSocket endpoint
    Custom {
        url: String,
        auth_header: Option<String>,
    },
}

impl VoiceApiProvider {
    /// Get WebSocket URL for provider
    pub fn get_ws_url(&self) -> String {
        match self {
            VoiceApiProvider::OpenAiRealtime { .. } => {
                "wss://api.openai.com/v1/realtime".into()
            }
            VoiceApiProvider::AnthropicRealtime { .. } => {
                // Placeholder for when Anthropic launches realtime API
                "wss://api.anthropic.com/v1/realtime".into()
            }
            VoiceApiProvider::LocalWhisperTts { .. } => {
                "ws://localhost:8765/voice".into()
            }
            VoiceApiProvider::Custom { url, .. } => url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_disabled_by_default() {
        let controller = VoiceController::default();
        assert!(!controller.is_available());
    }

    #[test]
    fn test_kill_switch() {
        let controller = VoiceController::default();
        controller.activate_kill_switch();
        assert!(controller.kill_switch.load(Ordering::SeqCst));
        assert!(controller.enable().is_err());
    }

    #[test]
    fn test_voice_state_transitions() {
        let controller = VoiceController::default();
        assert_eq!(controller.get_state(), VoiceState::Idle);

        controller.set_state(VoiceState::Listening);
        assert_eq!(controller.get_state(), VoiceState::Listening);

        controller.set_state(VoiceState::UserSpeaking);
        assert_eq!(controller.get_state(), VoiceState::UserSpeaking);
    }
}
