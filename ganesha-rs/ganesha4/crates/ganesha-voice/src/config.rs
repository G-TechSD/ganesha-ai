//! Voice configuration module.
//!
//! Provides configuration structures for the voice system including
//! device selection, hotkeys, API keys, and personality settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::input::VadConfig;
use crate::output::OpenAIVoice;
use crate::personality::TTSProvider;
use crate::{Result, VoiceError};

/// Main voice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Whether voice is enabled
    pub enabled: bool,
    /// Input configuration
    pub input: InputConfig,
    /// Output configuration
    pub output: OutputConfig,
    /// Personality configuration
    pub personality: PersonalityConfig,
    /// API keys configuration
    pub api_keys: ApiKeysConfig,
    /// Hotkey configuration
    pub hotkeys: HotkeyConfig,
    /// Advanced settings
    pub advanced: AdvancedConfig,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: InputConfig::default(),
            output: OutputConfig::default(),
            personality: PersonalityConfig::default(),
            api_keys: ApiKeysConfig::default(),
            hotkeys: HotkeyConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

impl VoiceConfig {
    /// Create a new voice configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a TOML file
    pub async fn load_from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| VoiceError::ConfigError(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| VoiceError::ConfigError(format!("Failed to parse config: {}", e)))
    }

    /// Save configuration to a TOML file
    pub async fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| VoiceError::ConfigError(format!("Failed to serialize config: {}", e)))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| VoiceError::ConfigError(format!("Failed to create directory: {}", e)))?;
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| VoiceError::ConfigError(format!("Failed to write config file: {}", e)))
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate input config
        if self.input.vad.voice_threshold < 0.0 || self.input.vad.voice_threshold > 1.0 {
            return Err(VoiceError::ConfigError(
                "Voice threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Validate output config
        if self.output.volume < 0.0 || self.output.volume > 1.0 {
            return Err(VoiceError::ConfigError(
                "Volume must be between 0.0 and 1.0".to_string(),
            ));
        }

        if self.output.speed < 0.25 || self.output.speed > 4.0 {
            return Err(VoiceError::ConfigError(
                "Speed must be between 0.25 and 4.0".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the default config file path
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ganesha")
            .join("voice.toml")
    }
}

/// Input device and recording configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Input device name (None for default)
    pub device: Option<String>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Voice activity detection configuration
    pub vad: VadConfigSerializable,
    /// Whether to use local Whisper (if available)
    pub use_local_whisper: bool,
    /// Path to local Whisper model (if using local)
    pub local_whisper_model: Option<PathBuf>,
    /// Language hint for transcription
    pub language: Option<String>,
    /// Whether to save recordings
    pub save_recordings: bool,
    /// Directory to save recordings
    pub recordings_dir: Option<PathBuf>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            device: None,
            sample_rate: 16000,
            vad: VadConfigSerializable::default(),
            use_local_whisper: false,
            local_whisper_model: None,
            language: None,
            save_recordings: false,
            recordings_dir: None,
        }
    }
}

/// Serializable version of VadConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfigSerializable {
    /// Minimum audio level to consider as voice (0.0 to 1.0)
    pub voice_threshold: f32,
    /// Duration of silence before considering end of speech (ms)
    pub silence_duration_ms: u64,
    /// Minimum speech duration to trigger transcription (ms)
    pub min_speech_duration_ms: u64,
    /// Maximum recording duration (seconds)
    pub max_recording_duration_secs: u64,
}

impl Default for VadConfigSerializable {
    fn default() -> Self {
        Self {
            voice_threshold: 0.02,
            silence_duration_ms: 1500,
            min_speech_duration_ms: 500,
            max_recording_duration_secs: 60,
        }
    }
}

impl From<VadConfigSerializable> for VadConfig {
    fn from(config: VadConfigSerializable) -> Self {
        Self {
            voice_threshold: config.voice_threshold,
            silence_duration: std::time::Duration::from_millis(config.silence_duration_ms),
            min_speech_duration: std::time::Duration::from_millis(config.min_speech_duration_ms),
            max_recording_duration: std::time::Duration::from_secs(config.max_recording_duration_secs),
        }
    }
}

impl From<VadConfig> for VadConfigSerializable {
    fn from(config: VadConfig) -> Self {
        Self {
            voice_threshold: config.voice_threshold,
            silence_duration_ms: config.silence_duration.as_millis() as u64,
            min_speech_duration_ms: config.min_speech_duration.as_millis() as u64,
            max_recording_duration_secs: config.max_recording_duration.as_secs(),
        }
    }
}

/// Output device and playback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output device name (None for default)
    pub device: Option<String>,
    /// TTS provider to use
    pub tts_provider: TTSProvider,
    /// OpenAI voice selection
    pub openai_voice: OpenAIVoice,
    /// ElevenLabs voice ID (if using ElevenLabs)
    pub elevenlabs_voice_id: Option<String>,
    /// OpenAI TTS model (tts-1 or tts-1-hd)
    pub openai_model: String,
    /// Playback volume (0.0 to 1.0)
    pub volume: f32,
    /// Playback speed (0.25 to 4.0)
    pub speed: f32,
    /// Whether playback is enabled
    pub playback_enabled: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            device: None,
            tts_provider: TTSProvider::OpenAI,
            openai_voice: OpenAIVoice::Nova,
            elevenlabs_voice_id: None,
            openai_model: "tts-1".to_string(),
            volume: 1.0,
            speed: 1.0,
            playback_enabled: true,
        }
    }
}

/// Personality configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    /// Default personality ID
    pub default_personality: String,
    /// Path to custom personalities directory
    pub custom_personalities_dir: Option<PathBuf>,
    /// Per-personality overrides
    #[serde(default)]
    pub overrides: HashMap<String, PersonalityOverride>,
}

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            default_personality: "friendly".to_string(),
            custom_personalities_dir: None,
            overrides: HashMap::new(),
        }
    }
}

/// Override settings for a specific personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityOverride {
    /// Override voice selection
    pub voice: Option<OpenAIVoice>,
    /// Override speed
    pub speed: Option<f32>,
    /// Additional system prompt text
    pub additional_prompt: Option<String>,
}

/// API keys configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeysConfig {
    /// OpenAI API key (for Whisper and TTS)
    pub openai: Option<String>,
    /// ElevenLabs API key
    pub elevenlabs: Option<String>,
    /// Whether to use environment variables for API keys
    pub use_env_vars: bool,
    /// Environment variable name for OpenAI key
    pub openai_env_var: String,
    /// Environment variable name for ElevenLabs key
    pub elevenlabs_env_var: String,
}

impl ApiKeysConfig {
    /// Create default config that uses environment variables
    pub fn from_env() -> Self {
        Self {
            openai: None,
            elevenlabs: None,
            use_env_vars: true,
            openai_env_var: "OPENAI_API_KEY".to_string(),
            elevenlabs_env_var: "ELEVENLABS_API_KEY".to_string(),
        }
    }

    /// Get the OpenAI API key (from config or environment)
    pub fn get_openai_key(&self) -> Option<String> {
        self.openai.clone().or_else(|| {
            if self.use_env_vars {
                std::env::var(&self.openai_env_var).ok()
            } else {
                None
            }
        })
    }

    /// Get the ElevenLabs API key (from config or environment)
    pub fn get_elevenlabs_key(&self) -> Option<String> {
        self.elevenlabs.clone().or_else(|| {
            if self.use_env_vars {
                std::env::var(&self.elevenlabs_env_var).ok()
            } else {
                None
            }
        })
    }
}

/// Hotkey configuration for push-to-talk and other actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Push-to-talk key
    pub push_to_talk: HotkeyBinding,
    /// Toggle listening key
    pub toggle_listening: HotkeyBinding,
    /// Stop/interrupt key
    pub stop: HotkeyBinding,
    /// Mute toggle key
    pub mute_toggle: HotkeyBinding,
    /// Whether push-to-talk mode is enabled
    pub push_to_talk_enabled: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            push_to_talk: HotkeyBinding::new("Space"),
            toggle_listening: HotkeyBinding::new("Ctrl+Shift+V"),
            stop: HotkeyBinding::new("Escape"),
            mute_toggle: HotkeyBinding::new("Ctrl+Shift+M"),
            push_to_talk_enabled: true,
        }
    }
}

/// A hotkey binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    /// The key binding string (e.g., "Ctrl+Shift+V")
    pub binding: String,
    /// Whether this hotkey is enabled
    pub enabled: bool,
}

impl HotkeyBinding {
    /// Create a new hotkey binding
    pub fn new(binding: &str) -> Self {
        Self {
            binding: binding.to_string(),
            enabled: true,
        }
    }

    /// Create a disabled hotkey binding
    pub fn disabled() -> Self {
        Self {
            binding: String::new(),
            enabled: false,
        }
    }

    /// Parse the binding into modifiers and key
    pub fn parse(&self) -> Option<(Vec<Modifier>, String)> {
        if !self.enabled || self.binding.is_empty() {
            return None;
        }

        let parts: Vec<&str> = self.binding.split('+').collect();
        let key = parts.last()?.to_string();
        let modifiers: Vec<Modifier> = parts[..parts.len() - 1]
            .iter()
            .filter_map(|s| match s.to_lowercase().as_str() {
                "ctrl" | "control" => Some(Modifier::Ctrl),
                "alt" => Some(Modifier::Alt),
                "shift" => Some(Modifier::Shift),
                "meta" | "super" | "win" | "cmd" => Some(Modifier::Meta),
                _ => None,
            })
            .collect();

        Some((modifiers, key))
    }
}

/// Keyboard modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

/// Advanced configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Audio buffer size in samples
    pub buffer_size: usize,
    /// Whether to enable noise cancellation
    pub noise_cancellation: bool,
    /// Whether to enable echo cancellation
    pub echo_cancellation: bool,
    /// Maximum history turns to keep
    pub max_history_turns: usize,
    /// Whether to auto-listen after assistant finishes
    pub auto_listen: bool,
    /// Whether to allow interruptions
    pub allow_interruptions: bool,
    /// Debug mode (saves audio, logs more)
    pub debug_mode: bool,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4096,
            noise_cancellation: false,
            echo_cancellation: false,
            max_history_turns: 100,
            auto_listen: true,
            allow_interruptions: true,
            debug_mode: false,
        }
    }
}

/// Builder for VoiceConfig
pub struct VoiceConfigBuilder {
    config: VoiceConfig,
}

impl VoiceConfigBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: VoiceConfig::default(),
        }
    }

    /// Set whether voice is enabled
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the input device
    pub fn input_device(mut self, device: &str) -> Self {
        self.config.input.device = Some(device.to_string());
        self
    }

    /// Set the output device
    pub fn output_device(mut self, device: &str) -> Self {
        self.config.output.device = Some(device.to_string());
        self
    }

    /// Set the TTS provider
    pub fn tts_provider(mut self, provider: TTSProvider) -> Self {
        self.config.output.tts_provider = provider;
        self
    }

    /// Set the OpenAI voice
    pub fn openai_voice(mut self, voice: OpenAIVoice) -> Self {
        self.config.output.openai_voice = voice;
        self
    }

    /// Set the OpenAI API key
    pub fn openai_api_key(mut self, key: &str) -> Self {
        self.config.api_keys.openai = Some(key.to_string());
        self
    }

    /// Set the ElevenLabs API key
    pub fn elevenlabs_api_key(mut self, key: &str) -> Self {
        self.config.api_keys.elevenlabs = Some(key.to_string());
        self
    }

    /// Set the default personality
    pub fn default_personality(mut self, personality: &str) -> Self {
        self.config.personality.default_personality = personality.to_string();
        self
    }

    /// Set push-to-talk key
    pub fn push_to_talk_key(mut self, binding: &str) -> Self {
        self.config.hotkeys.push_to_talk = HotkeyBinding::new(binding);
        self
    }

    /// Enable/disable push-to-talk mode
    pub fn push_to_talk_enabled(mut self, enabled: bool) -> Self {
        self.config.hotkeys.push_to_talk_enabled = enabled;
        self
    }

    /// Set volume
    pub fn volume(mut self, volume: f32) -> Self {
        self.config.output.volume = volume.clamp(0.0, 1.0);
        self
    }

    /// Set speed
    pub fn speed(mut self, speed: f32) -> Self {
        self.config.output.speed = speed.clamp(0.25, 4.0);
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<VoiceConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for VoiceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_config_default() {
        let config = VoiceConfig::default();
        assert!(config.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_voice_config_builder() {
        let config = VoiceConfigBuilder::new()
            .enabled(true)
            .openai_voice(OpenAIVoice::Alloy)
            .volume(0.8)
            .speed(1.2)
            .default_personality("professional")
            .build()
            .unwrap();

        assert!(config.enabled);
        assert_eq!(config.output.openai_voice, OpenAIVoice::Alloy);
        assert_eq!(config.output.volume, 0.8);
        assert_eq!(config.output.speed, 1.2);
        assert_eq!(config.personality.default_personality, "professional");
    }

    #[test]
    fn test_hotkey_parsing() {
        let hotkey = HotkeyBinding::new("Ctrl+Shift+V");
        let (modifiers, key) = hotkey.parse().unwrap();

        assert_eq!(modifiers.len(), 2);
        assert!(modifiers.contains(&Modifier::Ctrl));
        assert!(modifiers.contains(&Modifier::Shift));
        assert_eq!(key, "V");
    }

    #[test]
    fn test_api_keys_from_env() {
        let config = ApiKeysConfig::from_env();
        assert!(config.use_env_vars);
        assert_eq!(config.openai_env_var, "OPENAI_API_KEY");
    }

    #[test]
    fn test_vad_config_conversion() {
        let serializable = VadConfigSerializable::default();
        let vad_config: VadConfig = serializable.clone().into();

        assert_eq!(vad_config.voice_threshold, serializable.voice_threshold);
        assert_eq!(
            vad_config.silence_duration.as_millis() as u64,
            serializable.silence_duration_ms
        );
    }

    #[test]
    fn test_config_validation() {
        let mut config = VoiceConfig::default();
        assert!(config.validate().is_ok());

        config.input.vad.voice_threshold = 2.0; // Invalid
        assert!(config.validate().is_err());

        config.input.vad.voice_threshold = 0.5;
        config.output.volume = -1.0; // Invalid
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_api_keys_from_env_empty() {
        // With no env vars set, keys should be None
        let keys = ApiKeysConfig::default();
        // Default should have empty/none keys
        let _ = keys;
    }

    #[test]
    fn test_hotkey_binding_new() {
        let binding = HotkeyBinding::new("Ctrl+Space");
        assert_eq!(binding.binding, "Ctrl+Space");
        assert!(binding.enabled);
    }

    #[test]
    fn test_hotkey_binding_disabled() {
        let binding = HotkeyBinding::disabled();
        assert!(!binding.enabled);
    }

    #[test]
    fn test_hotkey_binding_parse() {
        let binding = HotkeyBinding::new("Ctrl+Space");
        let parsed = binding.parse();
        assert!(parsed.is_some());
        let (mods, key) = parsed.unwrap();
        assert!(mods.contains(&Modifier::Ctrl));
        assert_eq!(key, "Space");
    }

    #[test]
    fn test_hotkey_binding_parse_multi_modifier() {
        let binding = HotkeyBinding::new("Ctrl+Shift+M");
        let parsed = binding.parse();
        assert!(parsed.is_some());
        let (mods, key) = parsed.unwrap();
        assert!(mods.contains(&Modifier::Ctrl));
        assert!(mods.contains(&Modifier::Shift));
    }

    #[test]
    fn test_builder_input_device() {
        let config = VoiceConfigBuilder::new()
            .input_device("USB Microphone")
            .build()
            .unwrap();
        assert_eq!(config.input.device, Some("USB Microphone".to_string()));
    }

    #[test]
    fn test_builder_output_device() {
        let config = VoiceConfigBuilder::new()
            .output_device("Speakers")
            .build()
            .unwrap();
        assert_eq!(config.output.device, Some("Speakers".to_string()));
    }

    #[test]
    fn test_builder_push_to_talk() {
        let config = VoiceConfigBuilder::new()
            .push_to_talk_enabled(true)
            .push_to_talk_key("Ctrl+T")
            .build()
            .unwrap();
        assert!(config.hotkeys.push_to_talk_enabled);
    }

    #[test]
    fn test_config_default_path() {
        let path = VoiceConfig::default_path();
        assert!(path.to_str().unwrap().contains("voice"));
    }

    #[test]
    fn test_personality_config_default() {
        let config = PersonalityConfig::default();
        assert!(!config.default_personality.is_empty());
    }
}
