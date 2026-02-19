//! Voice output module for text-to-speech functionality.
//!
//! Provides traits and implementations for converting text to speech
//! and playing audio using various TTS backends.

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::Mutex;
use cpal::traits::DeviceTrait;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::debug;

use crate::{Result, VoiceError};

/// OpenAI TTS voice options
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAIVoice {
    Alloy,
    Echo,
    Fable,
    Onyx,
    Nova,
    Shimmer,
}

impl Default for OpenAIVoice {
    fn default() -> Self {
        Self::Nova
    }
}

impl std::fmt::Display for OpenAIVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Alloy => "alloy",
            Self::Echo => "echo",
            Self::Fable => "fable",
            Self::Onyx => "onyx",
            Self::Nova => "nova",
            Self::Shimmer => "shimmer",
        };
        write!(f, "{}", name)
    }
}

/// ElevenLabs voice model
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ElevenLabsVoice {
    /// Voice ID from ElevenLabs
    pub voice_id: String,
    /// Voice name for display
    pub name: String,
    /// Model ID (e.g., "eleven_monolingual_v1")
    pub model_id: String,
}

impl Default for ElevenLabsVoice {
    fn default() -> Self {
        Self {
            voice_id: "21m00Tcm4TlvDq8ikWAM".to_string(), // Rachel
            name: "Rachel".to_string(),
            model_id: "eleven_monolingual_v1".to_string(),
        }
    }
}

/// Voice output settings
#[derive(Debug, Clone)]
pub struct VoiceOutputSettings {
    /// Playback speed (0.5 to 2.0)
    pub speed: f32,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
    /// Whether to enable audio playback
    pub playback_enabled: bool,
}

impl Default for VoiceOutputSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            volume: 1.0,
            playback_enabled: true,
        }
    }
}

/// Voice output event
#[derive(Debug, Clone)]
pub enum VoiceOutputEvent {
    /// Speech generation started
    GenerationStarted,
    /// Speech generation completed
    GenerationCompleted { duration: Duration },
    /// Playback started
    PlaybackStarted,
    /// Playback progress
    PlaybackProgress { elapsed: Duration, total: Duration },
    /// Playback completed
    PlaybackCompleted,
    /// Playback interrupted
    PlaybackInterrupted,
    /// Error occurred
    Error { message: String },
}

/// Generated speech audio
#[derive(Debug, Clone)]
pub struct SpeechAudio {
    /// Raw audio bytes (MP3 or WAV format)
    pub data: Bytes,
    /// Audio format
    pub format: AudioFormat,
    /// Duration of the audio
    pub duration: Option<Duration>,
    /// The text that was synthesized
    pub text: String,
}

/// Audio format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Wav,
    Opus,
    Pcm,
}

impl SpeechAudio {
    /// Save audio to a file
    pub async fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        tokio::fs::write(path, &self.data)
            .await
            .map_err(|e| VoiceError::AudioError(format!("Failed to save audio: {}", e)))
    }
}

/// Trait for voice output (text-to-speech) implementations
#[async_trait]
pub trait VoiceOutput: Send + Sync {
    /// Get the name of this voice output implementation
    fn name(&self) -> &str;

    /// Generate speech from text
    async fn synthesize(&self, text: &str) -> Result<SpeechAudio>;

    /// Check if this implementation is available
    async fn is_available(&self) -> bool;

    /// Get the current voice name/ID
    fn current_voice(&self) -> String;

    /// List available voices
    async fn list_voices(&self) -> Result<Vec<String>>;
}

/// Audio playback manager
pub struct AudioPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Arc<Mutex<Option<Sink>>>,
    is_playing: Arc<AtomicBool>,
    settings: VoiceOutputSettings,
}

impl AudioPlayer {
    /// Create a new audio player
    pub fn new() -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| VoiceError::AudioError(format!("Failed to create output stream: {}", e)))?;

        Ok(Self {
            _stream: stream,
            handle,
            sink: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(AtomicBool::new(false)),
            settings: VoiceOutputSettings::default(),
        })
    }

    /// Set playback settings
    pub fn set_settings(&mut self, settings: VoiceOutputSettings) {
        self.settings = settings;
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    /// Play audio data
    pub fn play(&self, audio: &SpeechAudio) -> Result<()> {
        if !self.settings.playback_enabled {
            return Ok(());
        }

        self.stop();

        let cursor = Cursor::new(audio.data.to_vec());
        let source = Decoder::new(cursor)
            .map_err(|e| VoiceError::AudioError(format!("Failed to decode audio: {}", e)))?;

        let sink = Sink::try_new(&self.handle)
            .map_err(|e| VoiceError::AudioError(format!("Failed to create sink: {}", e)))?;

        sink.set_volume(self.settings.volume);
        sink.set_speed(self.settings.speed);
        sink.append(source);

        self.is_playing.store(true, Ordering::SeqCst);
        *self.sink.lock() = Some(sink);

        debug!("Audio playback started");
        Ok(())
    }

    /// Play audio and wait for completion
    pub async fn play_and_wait(
        &self,
        audio: &SpeechAudio,
        event_tx: Option<mpsc::Sender<VoiceOutputEvent>>,
    ) -> Result<()> {
        if !self.settings.playback_enabled {
            return Ok(());
        }

        self.play(audio)?;

        if let Some(ref tx) = event_tx {
            let _ = tx.send(VoiceOutputEvent::PlaybackStarted).await;
        }

        // Wait for playback to complete
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let sink_empty = {
                let lock = self.sink.lock();
                lock.as_ref().map(|s| s.empty()).unwrap_or(true)
            };

            if sink_empty {
                break;
            }

            if !self.is_playing.load(Ordering::SeqCst) {
                if let Some(ref tx) = event_tx {
                    let _ = tx.send(VoiceOutputEvent::PlaybackInterrupted).await;
                }
                return Ok(());
            }
        }

        self.is_playing.store(false, Ordering::SeqCst);

        if let Some(ref tx) = event_tx {
            let _ = tx.send(VoiceOutputEvent::PlaybackCompleted).await;
        }

        debug!("Audio playback completed");
        Ok(())
    }

    /// Stop current playback
    pub fn stop(&self) {
        if let Some(sink) = self.sink.lock().take() {
            sink.stop();
        }
        self.is_playing.store(false, Ordering::SeqCst);
        debug!("Audio playback stopped");
    }

    /// Pause current playback
    pub fn pause(&self) {
        if let Some(ref sink) = *self.sink.lock() {
            sink.pause();
        }
    }

    /// Resume paused playback
    pub fn resume(&self) {
        if let Some(ref sink) = *self.sink.lock() {
            sink.play();
        }
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        self.settings.volume = volume.clamp(0.0, 1.0);
        if let Some(ref sink) = *self.sink.lock() {
            sink.set_volume(self.settings.volume);
        }
    }

    /// Set playback speed (0.5 to 2.0)
    pub fn set_speed(&mut self, speed: f32) {
        self.settings.speed = speed.clamp(0.5, 2.0);
        if let Some(ref sink) = *self.sink.lock() {
            sink.set_speed(self.settings.speed);
        }
    }
}

/// OpenAI TTS implementation
pub struct OpenAITTS {
    api_key: String,
    voice: OpenAIVoice,
    model: String,
    speed: f32,
    client: reqwest::Client,
}

impl OpenAITTS {
    /// Create a new OpenAI TTS instance
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice: OpenAIVoice::default(),
            model: "tts-1".to_string(),
            speed: 1.0,
            client: reqwest::Client::new(),
        }
    }

    /// Set the voice to use
    pub fn with_voice(mut self, voice: OpenAIVoice) -> Self {
        self.voice = voice;
        self
    }

    /// Set the model (tts-1 or tts-1-hd)
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Set the speed (0.25 to 4.0)
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.clamp(0.25, 4.0);
        self
    }

    /// Change the voice
    pub fn set_voice(&mut self, voice: OpenAIVoice) {
        self.voice = voice;
    }

    /// Get available voices
    pub fn available_voices() -> Vec<OpenAIVoice> {
        vec![
            OpenAIVoice::Alloy,
            OpenAIVoice::Echo,
            OpenAIVoice::Fable,
            OpenAIVoice::Onyx,
            OpenAIVoice::Nova,
            OpenAIVoice::Shimmer,
        ]
    }
}

#[async_trait]
impl VoiceOutput for OpenAITTS {
    fn name(&self) -> &str {
        "OpenAI TTS"
    }

    async fn synthesize(&self, text: &str) -> Result<SpeechAudio> {
        let request_body = serde_json::json!({
            "model": self.model,
            "input": text,
            "voice": self.voice.to_string(),
            "speed": self.speed,
            "response_format": "mp3"
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::ApiError(format!("TTS API error: {}", error_text)));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Failed to read response: {}", e)))?;

        Ok(SpeechAudio {
            data: bytes,
            format: AudioFormat::Mp3,
            duration: None,
            text: text.to_string(),
        })
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn current_voice(&self) -> String {
        self.voice.to_string()
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        Ok(Self::available_voices()
            .iter()
            .map(|v| v.to_string())
            .collect())
    }
}

/// ElevenLabs TTS implementation
pub struct ElevenLabsTTS {
    api_key: String,
    voice: ElevenLabsVoice,
    stability: f32,
    similarity_boost: f32,
    client: reqwest::Client,
}

impl ElevenLabsTTS {
    /// Create a new ElevenLabs TTS instance
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice: ElevenLabsVoice::default(),
            stability: 0.5,
            similarity_boost: 0.75,
            client: reqwest::Client::new(),
        }
    }

    /// Set the voice to use
    pub fn with_voice(mut self, voice: ElevenLabsVoice) -> Self {
        self.voice = voice;
        self
    }

    /// Set voice settings
    pub fn with_settings(mut self, stability: f32, similarity_boost: f32) -> Self {
        self.stability = stability.clamp(0.0, 1.0);
        self.similarity_boost = similarity_boost.clamp(0.0, 1.0);
        self
    }

    /// Set voice by ID
    pub fn set_voice_id(&mut self, voice_id: &str) {
        self.voice.voice_id = voice_id.to_string();
    }

    /// Fetch available voices from the API
    pub async fn fetch_voices(&self) -> Result<Vec<ElevenLabsVoice>> {
        let response = self
            .client
            .get("https://api.elevenlabs.io/v1/voices")
            .header("xi-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::ApiError(format!(
                "ElevenLabs API error: {}",
                error_text
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Failed to parse response: {}", e)))?;

        let voices = result["voices"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        Some(ElevenLabsVoice {
                            voice_id: v["voice_id"].as_str()?.to_string(),
                            name: v["name"].as_str()?.to_string(),
                            model_id: "eleven_monolingual_v1".to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(voices)
    }
}

#[async_trait]
impl VoiceOutput for ElevenLabsTTS {
    fn name(&self) -> &str {
        "ElevenLabs TTS"
    }

    async fn synthesize(&self, text: &str) -> Result<SpeechAudio> {
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            self.voice.voice_id
        );

        let request_body = serde_json::json!({
            "text": text,
            "model_id": self.voice.model_id,
            "voice_settings": {
                "stability": self.stability,
                "similarity_boost": self.similarity_boost
            }
        });

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "audio/mpeg")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::ApiError(format!(
                "ElevenLabs API error: {}",
                error_text
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Failed to read response: {}", e)))?;

        Ok(SpeechAudio {
            data: bytes,
            format: AudioFormat::Mp3,
            duration: None,
            text: text.to_string(),
        })
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn current_voice(&self) -> String {
        self.voice.name.clone()
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        let voices = self.fetch_voices().await?;
        Ok(voices.into_iter().map(|v| v.name).collect())
    }
}

/// Piper TTS implementation (local, free)
/// Uses the piper command-line tool for neural TTS
pub struct PiperTTS {
    model_path: std::path::PathBuf,
    config_path: Option<std::path::PathBuf>,
    speaker_id: Option<i32>,
    length_scale: f32, // Speed: < 1.0 = faster, > 1.0 = slower
}

impl PiperTTS {
    /// Create a new Piper TTS instance with a model path
    pub fn new(model_path: impl Into<std::path::PathBuf>) -> Self {
        let model_path = model_path.into();
        let config_path = {
            let mut p = model_path.clone();
            p.set_extension("onnx.json");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        };

        Self {
            model_path,
            config_path,
            speaker_id: None,
            length_scale: 1.0,
        }
    }

    /// Set the config file path
    pub fn with_config(mut self, config_path: impl Into<std::path::PathBuf>) -> Self {
        self.config_path = Some(config_path.into());
        self
    }

    /// Set the speaker ID (for multi-speaker models)
    pub fn with_speaker(mut self, speaker_id: i32) -> Self {
        self.speaker_id = Some(speaker_id);
        self
    }

    /// Set the length scale (speed adjustment)
    pub fn with_speed(mut self, length_scale: f32) -> Self {
        self.length_scale = length_scale.clamp(0.5, 2.0);
        self
    }

    /// Check if piper command is available
    pub fn is_piper_installed() -> bool {
        std::process::Command::new("piper")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl VoiceOutput for PiperTTS {
    fn name(&self) -> &str {
        "Piper TTS (Local)"
    }

    async fn synthesize(&self, text: &str) -> Result<SpeechAudio> {
        use std::process::{Command, Stdio};
        use std::io::Write;

        // Check if piper is available
        if !Self::is_piper_installed() {
            return Err(VoiceError::FeatureDisabled(
                "Piper TTS not installed. Install with: pip install piper-tts".to_string(),
            ));
        }

        // Check if model exists
        if !self.model_path.exists() {
            return Err(VoiceError::ConfigError(format!(
                "Piper model not found: {}",
                self.model_path.display()
            )));
        }

        // Create a temp file for output
        let temp_dir = std::env::temp_dir();
        let output_file = temp_dir.join(format!("piper_output_{}.wav", std::process::id()));

        // Build piper command
        let mut cmd = Command::new("piper");
        cmd.arg("--model")
            .arg(&self.model_path)
            .arg("--output_file")
            .arg(&output_file)
            .arg("--length_scale")
            .arg(self.length_scale.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        if let Some(ref config) = self.config_path {
            cmd.arg("--config").arg(config);
        }

        if let Some(speaker) = self.speaker_id {
            cmd.arg("--speaker").arg(speaker.to_string());
        }

        // Spawn process and write text
        let mut child = cmd.spawn().map_err(|e| {
            VoiceError::AudioError(format!("Failed to start piper: {}", e))
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).map_err(|e| {
                VoiceError::AudioError(format!("Failed to write to piper: {}", e))
            })?;
        }

        // Wait for completion
        let output = child.wait_with_output().map_err(|e| {
            VoiceError::AudioError(format!("Piper process failed: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::AudioError(format!(
                "Piper TTS failed: {}",
                stderr
            )));
        }

        // Read the output file
        let audio_data = std::fs::read(&output_file).map_err(|e| {
            VoiceError::AudioError(format!("Failed to read piper output: {}", e))
        })?;

        // Clean up temp file
        let _ = std::fs::remove_file(&output_file);

        Ok(SpeechAudio {
            data: Bytes::from(audio_data),
            format: AudioFormat::Wav,
            duration: None,
            text: text.to_string(),
        })
    }

    async fn is_available(&self) -> bool {
        Self::is_piper_installed() && self.model_path.exists()
    }

    fn current_voice(&self) -> String {
        self.model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    async fn list_voices(&self) -> Result<Vec<String>> {
        // For Piper, voices are model files - list available models in the directory
        let parent = self.model_path.parent().unwrap_or(std::path::Path::new("."));
        let mut voices = Vec::new();

        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "onnx").unwrap_or(false) {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        voices.push(name.to_string());
                    }
                }
            }
        }

        Ok(voices)
    }
}

/// List available output devices
pub fn list_output_devices() -> Result<Vec<String>> {
    use cpal::traits::HostTrait;

    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|e| VoiceError::AudioError(format!("Failed to enumerate devices: {}", e)))?;

    Ok(devices.filter_map(|d| d.name().ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_voice_display() {
        assert_eq!(OpenAIVoice::Alloy.to_string(), "alloy");
        assert_eq!(OpenAIVoice::Nova.to_string(), "nova");
    }

    #[test]
    fn test_voice_output_settings_default() {
        let settings = VoiceOutputSettings::default();
        assert_eq!(settings.speed, 1.0);
        assert_eq!(settings.volume, 1.0);
        assert!(settings.playback_enabled);
    }

    #[test]
    fn test_elevenlabs_voice_default() {
        let voice = ElevenLabsVoice::default();
        assert!(!voice.voice_id.is_empty());
        assert!(!voice.name.is_empty());
    }

    #[test]
    fn test_openai_voice_all_variants() {
        let voices = vec![
            OpenAIVoice::Alloy,
            OpenAIVoice::Echo,
            OpenAIVoice::Fable,
            OpenAIVoice::Onyx,
            OpenAIVoice::Nova,
            OpenAIVoice::Shimmer,
        ];
        for voice in &voices {
            let s = voice.to_string();
            assert!(!s.is_empty());
        }
        assert_eq!(voices.len(), 6);
    }

    #[test]
    fn test_openai_voice_default_is_nova() {
        let voice = OpenAIVoice::default();
        assert_eq!(voice.to_string(), "nova");
    }

    #[test]
    fn test_voice_output_settings_custom() {
        let mut settings = VoiceOutputSettings::default();
        settings.speed = 1.5;
        settings.volume = 0.8;
        settings.playback_enabled = false;
        assert_eq!(settings.speed, 1.5);
        assert_eq!(settings.volume, 0.8);
        assert!(!settings.playback_enabled);
    }

    #[test]
    fn test_speech_audio_creation() {
        let audio = SpeechAudio {
            data: bytes::Bytes::from(vec![1u8, 2, 3, 4]),
            format: AudioFormat::Mp3,
            duration: Some(std::time::Duration::from_secs(1)),
            text: "hello".to_string(),
        };
        assert_eq!(audio.data.len(), 4);
        assert_eq!(audio.text, "hello");
    }

    #[test]
    fn test_audio_formats() {
        let formats = vec![
            AudioFormat::Mp3,
            AudioFormat::Wav,
            AudioFormat::Opus,
            AudioFormat::Pcm,
        ];
        assert_eq!(formats.len(), 4);
    }

    #[test]
    fn test_speech_audio_empty() {
        let audio = SpeechAudio {
            data: bytes::Bytes::new(),
            format: AudioFormat::Wav,
            duration: None,
            text: String::new(),
        };
        assert!(audio.data.is_empty());
        assert!(audio.duration.is_none());
    }

    #[test]
    fn test_elevenlabs_voice_custom() {
        let voice = ElevenLabsVoice {
            voice_id: "custom123".to_string(),
            name: "Custom Voice".to_string(),
            ..Default::default()
        };
        assert_eq!(voice.voice_id, "custom123");
        assert_eq!(voice.name, "Custom Voice");
    }

    #[test]
    fn test_openai_voice_display_all() {
        assert_eq!(OpenAIVoice::Echo.to_string(), "echo");
        assert_eq!(OpenAIVoice::Fable.to_string(), "fable");
        assert_eq!(OpenAIVoice::Onyx.to_string(), "onyx");
        assert_eq!(OpenAIVoice::Shimmer.to_string(), "shimmer");
    }
}
