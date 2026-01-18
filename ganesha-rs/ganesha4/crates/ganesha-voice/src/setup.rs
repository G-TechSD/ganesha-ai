//! Voice setup and model download utilities.
//!
//! Handles automatic download and setup of local voice models:
//! - Whisper models for STT (speech-to-text)
//! - Piper models for TTS (text-to-speech)

use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{info, warn};

use crate::{Result, VoiceError};

/// Voice models directory structure
pub struct VoiceModels {
    /// Base directory for voice models
    pub base_dir: PathBuf,
    /// Whisper model directory
    pub whisper_dir: PathBuf,
    /// Piper model directory
    pub piper_dir: PathBuf,
}

impl VoiceModels {
    /// Create a new VoiceModels instance with default paths
    pub fn new() -> Self {
        let base_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ganesha")
            .join("voice");

        Self {
            whisper_dir: base_dir.join("whisper"),
            piper_dir: base_dir.join("piper"),
            base_dir,
        }
    }

    /// Get the path to the default Whisper model
    pub fn whisper_model_path(&self) -> PathBuf {
        self.whisper_dir.join("ggml-base.en.bin")
    }

    /// Get the path to the default Piper model
    pub fn piper_model_path(&self) -> PathBuf {
        self.piper_dir.join("en_US-amy-medium.onnx")
    }

    /// Get the path to the Piper JSON config
    pub fn piper_config_path(&self) -> PathBuf {
        self.piper_dir.join("en_US-amy-medium.onnx.json")
    }

    /// Check if Whisper model is installed
    pub fn has_whisper_model(&self) -> bool {
        self.whisper_model_path().exists()
    }

    /// Check if Piper model is installed
    pub fn has_piper_model(&self) -> bool {
        self.piper_model_path().exists() && self.piper_config_path().exists()
    }

    /// Check if Piper binary is available
    pub fn has_piper_binary(&self) -> bool {
        // Check system piper
        if Command::new("piper").arg("--version").output().is_ok() {
            return true;
        }
        // Check local piper
        self.piper_dir.join("piper").exists()
    }

    /// Ensure directories exist
    pub async fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.whisper_dir)
            .await
            .map_err(|e| VoiceError::IoError(e))?;
        fs::create_dir_all(&self.piper_dir)
            .await
            .map_err(|e| VoiceError::IoError(e))?;
        Ok(())
    }
}

impl Default for VoiceModels {
    fn default() -> Self {
        Self::new()
    }
}

/// Download a file from URL to destination
pub async fn download_file(url: &str, dest: &Path, progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>) -> Result<()> {
    info!("Downloading {} to {}", url, dest.display());

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| VoiceError::ApiError(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(VoiceError::ApiError(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // Create parent directory if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| VoiceError::IoError(e))?;
    }

    let mut file = fs::File::create(dest)
        .await
        .map_err(|e| VoiceError::IoError(e))?;

    use tokio::io::AsyncWriteExt;
    let mut stream = response.bytes_stream();
    use futures::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| VoiceError::ApiError(format!("Download error: {}", e)))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| VoiceError::IoError(e))?;

        downloaded += chunk.len() as u64;
        if let Some(ref cb) = progress_callback {
            cb(downloaded, total_size);
        }
    }

    file.flush().await.map_err(|e| VoiceError::IoError(e))?;
    info!("Downloaded {} bytes to {}", downloaded, dest.display());

    Ok(())
}

/// Whisper model info
#[derive(Debug, Clone)]
pub struct WhisperModelInfo {
    pub name: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub description: &'static str,
}

/// Available Whisper models
pub const WHISPER_MODELS: &[WhisperModelInfo] = &[
    WhisperModelInfo {
        name: "tiny.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        size_mb: 75,
        description: "Fastest, English only, lower accuracy",
    },
    WhisperModelInfo {
        name: "base.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        size_mb: 142,
        description: "Good balance of speed and accuracy (recommended)",
    },
    WhisperModelInfo {
        name: "small.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
        size_mb: 466,
        description: "Better accuracy, slower",
    },
    WhisperModelInfo {
        name: "medium.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
        size_mb: 1500,
        description: "High accuracy, much slower",
    },
];

/// Get default Whisper model info
pub fn default_whisper_model() -> &'static WhisperModelInfo {
    &WHISPER_MODELS[1] // base.en
}

/// Download default Whisper model
pub async fn download_whisper_model(models: &VoiceModels, progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>) -> Result<PathBuf> {
    let model = default_whisper_model();
    let dest = models.whisper_dir.join(format!("ggml-{}.bin", model.name));

    if dest.exists() {
        info!("Whisper model already exists: {}", dest.display());
        return Ok(dest);
    }

    models.ensure_dirs().await?;
    download_file(model.url, &dest, progress_callback).await?;

    Ok(dest)
}

/// Piper voice info
#[derive(Debug, Clone)]
pub struct PiperVoiceInfo {
    pub name: &'static str,
    pub model_url: &'static str,
    pub config_url: &'static str,
    pub size_mb: u32,
    pub description: &'static str,
}

/// Available Piper voices
pub const PIPER_VOICES: &[PiperVoiceInfo] = &[
    PiperVoiceInfo {
        name: "amy-medium",
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/amy/medium/en_US-amy-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/amy/medium/en_US-amy-medium.onnx.json",
        size_mb: 63,
        description: "Female US English, medium quality (recommended)",
    },
    PiperVoiceInfo {
        name: "lessac-medium",
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
        size_mb: 63,
        description: "Male US English, medium quality",
    },
    PiperVoiceInfo {
        name: "ryan-medium",
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx.json",
        size_mb: 63,
        description: "Male US English, medium quality",
    },
];

/// Get default Piper voice info
pub fn default_piper_voice() -> &'static PiperVoiceInfo {
    &PIPER_VOICES[0] // amy-medium
}

/// Download default Piper voice model
pub async fn download_piper_voice(models: &VoiceModels, progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>) -> Result<PathBuf> {
    let voice = default_piper_voice();

    let model_dest = models.piper_dir.join(format!("en_US-{}.onnx", voice.name));
    let config_dest = models.piper_dir.join(format!("en_US-{}.onnx.json", voice.name));

    models.ensure_dirs().await?;

    if !model_dest.exists() {
        download_file(voice.model_url, &model_dest, progress_callback).await?;
    }

    if !config_dest.exists() {
        // Config file is small, no progress needed
        download_file(voice.config_url, &config_dest, None).await?;
    }

    Ok(model_dest)
}

/// Check if system has piper installed
pub fn check_piper_installed() -> bool {
    Command::new("piper")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get instructions for installing piper
pub fn piper_install_instructions() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "Install Piper TTS:\n\
         Ubuntu/Debian: pip install piper-tts\n\
         Or download from: https://github.com/rhasspy/piper/releases"
    }
    #[cfg(target_os = "macos")]
    {
        "Install Piper TTS:\n\
         brew install piper\n\
         Or: pip install piper-tts"
    }
    #[cfg(target_os = "windows")]
    {
        "Install Piper TTS:\n\
         pip install piper-tts\n\
         Or download from: https://github.com/rhasspy/piper/releases"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "Install Piper TTS from: https://github.com/rhasspy/piper/releases"
    }
}

/// Full voice setup status
#[derive(Debug, Clone)]
pub struct VoiceSetupStatus {
    pub whisper_model_installed: bool,
    pub piper_installed: bool,
    pub piper_voice_installed: bool,
    pub ready_for_local_voice: bool,
}

impl VoiceSetupStatus {
    /// Check current voice setup status
    pub fn check(models: &VoiceModels) -> Self {
        let whisper_model_installed = models.has_whisper_model();
        let piper_installed = check_piper_installed() || models.has_piper_binary();
        let piper_voice_installed = models.has_piper_model();

        Self {
            whisper_model_installed,
            piper_installed,
            piper_voice_installed,
            ready_for_local_voice: whisper_model_installed && piper_installed && piper_voice_installed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_models_paths() {
        let models = VoiceModels::new();
        assert!(models.whisper_model_path().to_string_lossy().contains("whisper"));
        assert!(models.piper_model_path().to_string_lossy().contains("piper"));
    }

    #[test]
    fn test_whisper_models_list() {
        assert!(!WHISPER_MODELS.is_empty());
        let default = default_whisper_model();
        assert!(default.name.contains("base"));
    }

    #[test]
    fn test_piper_voices_list() {
        assert!(!PIPER_VOICES.is_empty());
        let default = default_piper_voice();
        assert!(default.name.contains("amy"));
    }
}
