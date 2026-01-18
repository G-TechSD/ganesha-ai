//! Voice input module for speech-to-text functionality.
//!
//! Provides traits and implementations for capturing audio and converting
//! speech to text using various backends.

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};
use hound::{WavSpec, WavWriter};
use parking_lot::Mutex;
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{Result, VoiceError};

/// Voice input event emitted during recording
#[derive(Debug, Clone)]
pub enum VoiceInputEvent {
    /// Recording has started
    RecordingStarted,
    /// Recording has stopped
    RecordingStopped,
    /// Voice activity detected
    VoiceActivityDetected,
    /// Silence detected (potential end of speech)
    SilenceDetected { duration: Duration },
    /// Audio level update
    AudioLevel { level: f32 },
    /// Transcription result
    Transcription { text: String, is_final: bool },
    /// Error occurred
    Error { message: String },
}

/// Configuration for voice activity detection
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Minimum audio level to consider as voice (0.0 to 1.0)
    pub voice_threshold: f32,
    /// Duration of silence before considering end of speech
    pub silence_duration: Duration,
    /// Minimum speech duration to trigger transcription
    pub min_speech_duration: Duration,
    /// Maximum recording duration
    pub max_recording_duration: Duration,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            voice_threshold: 0.02,
            silence_duration: Duration::from_millis(1500),
            min_speech_duration: Duration::from_millis(500),
            max_recording_duration: Duration::from_secs(60),
        }
    }
}

/// Audio data captured from microphone
#[derive(Debug, Clone)]
pub struct AudioData {
    /// Raw audio samples (f32, mono)
    pub samples: Vec<f32>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Duration of the audio
    pub duration: Duration,
}

impl AudioData {
    /// Create a new AudioData instance
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let duration = Duration::from_secs_f64(samples.len() as f64 / sample_rate as f64);
        Self {
            samples,
            sample_rate,
            channels,
            duration,
        }
    }

    /// Convert audio data to WAV bytes
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        let spec = WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::new(&mut cursor, spec)
            .map_err(|e| VoiceError::AudioError(format!("Failed to create WAV writer: {}", e)))?;

        for sample in &self.samples {
            // Convert f32 to i16
            let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| VoiceError::AudioError(format!("Failed to write sample: {}", e)))?;
        }

        writer
            .finalize()
            .map_err(|e| VoiceError::AudioError(format!("Failed to finalize WAV: {}", e)))?;

        Ok(cursor.into_inner())
    }

    /// Save audio data to a WAV file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let spec = WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(path, spec)
            .map_err(|e| VoiceError::AudioError(format!("Failed to create WAV file: {}", e)))?;

        for sample in &self.samples {
            let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| VoiceError::AudioError(format!("Failed to write sample: {}", e)))?;
        }

        writer
            .finalize()
            .map_err(|e| VoiceError::AudioError(format!("Failed to finalize WAV file: {}", e)))?;

        Ok(())
    }
}

/// Transcription result from speech-to-text
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Confidence score (0.0 to 1.0), if available
    pub confidence: Option<f32>,
    /// Detected language, if available
    pub language: Option<String>,
    /// Duration of the audio that was transcribed
    pub duration: Duration,
    /// Individual word segments with timestamps, if available
    pub segments: Vec<TranscriptionSegment>,
}

/// A segment of transcription with timing information
#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    /// The text of this segment
    pub text: String,
    /// Start time in seconds
    pub start: f32,
    /// End time in seconds
    pub end: f32,
}

/// Trait for voice input (speech-to-text) implementations
#[async_trait]
pub trait VoiceInput: Send + Sync {
    /// Get the name of this voice input implementation
    fn name(&self) -> &str;

    /// Transcribe audio data to text
    async fn transcribe(&self, audio: &AudioData) -> Result<TranscriptionResult>;

    /// Check if this implementation is available (e.g., API key configured)
    async fn is_available(&self) -> bool;
}

/// Audio recorder for capturing microphone input
pub struct AudioRecorder {
    device: Device,
    config: StreamConfig,
    is_recording: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    vad_config: VadConfig,
}

impl AudioRecorder {
    /// Create a new audio recorder with the default input device
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| VoiceError::AudioError("No input device available".to_string()))?;

        let supported_config = device
            .default_input_config()
            .map_err(|e| VoiceError::AudioError(format!("Failed to get input config: {}", e)))?;

        // Use the default sample rate from the device
        // Note: SupportedStreamConfig doesn't expose min/max_sample_rate in cpal 0.15
        let sample_rate = supported_config.sample_rate();

        let config = StreamConfig {
            channels: 1, // Mono for speech recognition
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        info!(
            "Audio recorder initialized: {} @ {}Hz",
            device.name().unwrap_or_default(),
            sample_rate.0
        );

        Ok(Self {
            device,
            config,
            is_recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            vad_config: VadConfig::default(),
        })
    }

    /// Create a new audio recorder with a specific device
    pub fn with_device(device_name: &str) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .input_devices()
            .map_err(|e| VoiceError::AudioError(format!("Failed to enumerate devices: {}", e)))?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            .ok_or_else(|| VoiceError::AudioError(format!("Device not found: {}", device_name)))?;

        let supported_config = device
            .default_input_config()
            .map_err(|e| VoiceError::AudioError(format!("Failed to get input config: {}", e)))?;

        let sample_rate = supported_config.sample_rate();

        let config = StreamConfig {
            channels: 1,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        Ok(Self {
            device,
            config,
            is_recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            vad_config: VadConfig::default(),
        })
    }

    /// Set voice activity detection configuration
    pub fn set_vad_config(&mut self, config: VadConfig) {
        self.vad_config = config;
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Start recording with push-to-talk (manual control)
    pub fn start_recording(&self, event_tx: Option<mpsc::Sender<VoiceInputEvent>>) -> Result<()> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.samples.lock().clear();
        self.is_recording.store(true, Ordering::SeqCst);

        let is_recording = self.is_recording.clone();
        let samples = self.samples.clone();
        let sample_format = self
            .device
            .default_input_config()
            .map_err(|e| VoiceError::AudioError(format!("Failed to get input config: {}", e)))?
            .sample_format();

        let err_fn = |err| error!("Audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => self.device.build_input_stream(
                &self.config,
                move |data: &[f32], _: &_| {
                    if is_recording.load(Ordering::SeqCst) {
                        samples.lock().extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => self.device.build_input_stream(
                &self.config,
                move |data: &[i16], _: &_| {
                    if is_recording.load(Ordering::SeqCst) {
                        let converted: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        samples.lock().extend(converted);
                    }
                },
                err_fn,
                None,
            ),
            SampleFormat::U16 => self.device.build_input_stream(
                &self.config,
                move |data: &[u16], _: &_| {
                    if is_recording.load(Ordering::SeqCst) {
                        let converted: Vec<f32> =
                            data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                        samples.lock().extend(converted);
                    }
                },
                err_fn,
                None,
            ),
            _ => {
                return Err(VoiceError::AudioError(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )))
            }
        }
        .map_err(|e| VoiceError::AudioError(format!("Failed to build stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| VoiceError::AudioError(format!("Failed to start stream: {}", e)))?;

        if let Some(tx) = event_tx {
            let _ = tx.try_send(VoiceInputEvent::RecordingStarted);
        }

        // Keep stream alive - in a real implementation, we'd store this
        std::mem::forget(stream);

        debug!("Recording started");
        Ok(())
    }

    /// Stop recording and return the captured audio
    pub fn stop_recording(&self) -> Result<AudioData> {
        self.is_recording.store(false, Ordering::SeqCst);

        let samples = {
            let mut lock = self.samples.lock();
            std::mem::take(&mut *lock)
        };

        debug!("Recording stopped, captured {} samples", samples.len());

        Ok(AudioData::new(samples, self.config.sample_rate.0, 1))
    }

    /// Record with voice activity detection
    pub async fn record_with_vad(
        &self,
        event_tx: mpsc::Sender<VoiceInputEvent>,
    ) -> Result<AudioData> {
        self.samples.lock().clear();
        self.is_recording.store(true, Ordering::SeqCst);

        let is_recording = self.is_recording.clone();
        let samples = self.samples.clone();
        let vad_config = self.vad_config.clone();
        let event_tx_clone = event_tx.clone();

        let sample_format = self
            .device
            .default_input_config()
            .map_err(|e| VoiceError::AudioError(format!("Failed to get input config: {}", e)))?
            .sample_format();

        let voice_detected = Arc::new(AtomicBool::new(false));
        let voice_detected_clone = voice_detected.clone();
        let last_voice_time = Arc::new(Mutex::new(Instant::now()));
        let last_voice_time_clone = last_voice_time.clone();
        let speech_start_time = Arc::new(Mutex::new(None::<Instant>));
        let speech_start_time_clone = speech_start_time.clone();

        let err_fn = |err| error!("Audio stream error: {}", err);

        let process_audio = move |data: &[f32]| {
            if !is_recording.load(Ordering::SeqCst) {
                return;
            }

            samples.lock().extend_from_slice(data);

            // Calculate RMS level
            let rms: f32 = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();

            // Send audio level event periodically
            let _ = event_tx_clone.try_send(VoiceInputEvent::AudioLevel { level: rms });

            if rms > vad_config.voice_threshold {
                if !voice_detected_clone.load(Ordering::SeqCst) {
                    voice_detected_clone.store(true, Ordering::SeqCst);
                    *speech_start_time_clone.lock() = Some(Instant::now());
                    let _ = event_tx_clone.try_send(VoiceInputEvent::VoiceActivityDetected);
                }
                *last_voice_time_clone.lock() = Instant::now();
            }
        };

        let config = self.config.clone();
        let stream = match sample_format {
            SampleFormat::F32 => self.device.build_input_stream(
                &config,
                move |data: &[f32], _: &_| process_audio(data),
                err_fn,
                None,
            ),
            SampleFormat::I16 => {
                let process = move |data: &[i16], _: &_| {
                    let converted: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    process_audio(&converted);
                };
                self.device
                    .build_input_stream(&config, process, err_fn, None)
            }
            _ => {
                return Err(VoiceError::AudioError(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )))
            }
        }
        .map_err(|e| VoiceError::AudioError(format!("Failed to build stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| VoiceError::AudioError(format!("Failed to start stream: {}", e)))?;

        let _ = event_tx.send(VoiceInputEvent::RecordingStarted).await;

        let start_time = Instant::now();

        // Wait for voice activity and then silence
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let elapsed = start_time.elapsed();
            if elapsed > self.vad_config.max_recording_duration {
                warn!("Max recording duration reached");
                break;
            }

            if voice_detected.load(Ordering::SeqCst) {
                let silence_duration = last_voice_time.lock().elapsed();
                if silence_duration > self.vad_config.silence_duration {
                    // Check if we have enough speech
                    if let Some(speech_start) = *speech_start_time.lock() {
                        let speech_duration = speech_start.elapsed() - silence_duration;
                        if speech_duration >= self.vad_config.min_speech_duration {
                            let _ = event_tx
                                .send(VoiceInputEvent::SilenceDetected {
                                    duration: silence_duration,
                                })
                                .await;
                            break;
                        }
                    }
                }
            }

            if !self.is_recording.load(Ordering::SeqCst) {
                break;
            }
        }

        drop(stream);
        self.is_recording.store(false, Ordering::SeqCst);

        let samples = {
            let mut lock = self.samples.lock();
            std::mem::take(&mut *lock)
        };

        let _ = event_tx.send(VoiceInputEvent::RecordingStopped).await;

        Ok(AudioData::new(samples, self.config.sample_rate.0, 1))
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new().expect("Failed to create default audio recorder")
    }
}

/// List available input devices
pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| VoiceError::AudioError(format!("Failed to enumerate devices: {}", e)))?;

    Ok(devices.filter_map(|d| d.name().ok()).collect())
}

/// OpenAI Whisper API implementation
pub struct WhisperInput {
    api_key: String,
    model: String,
    language: Option<String>,
    client: reqwest::Client,
}

impl WhisperInput {
    /// Create a new Whisper input with API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "whisper-1".to_string(),
            language: None,
            client: reqwest::Client::new(),
        }
    }

    /// Set the model to use
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Set the language hint
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }
}

#[async_trait]
impl VoiceInput for WhisperInput {
    fn name(&self) -> &str {
        "OpenAI Whisper"
    }

    async fn transcribe(&self, audio: &AudioData) -> Result<TranscriptionResult> {
        let wav_bytes = audio.to_wav_bytes()?;

        let part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| VoiceError::ApiError(format!("Failed to create multipart: {}", e)))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        if let Some(ref lang) = self.language {
            form = form.text("language", lang.clone());
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::ApiError(format!(
                "Whisper API error: {}",
                error_text
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VoiceError::ApiError(format!("Failed to parse response: {}", e)))?;

        let text = result["text"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        let language = result["language"].as_str().map(|s| s.to_string());
        let duration_secs = result["duration"].as_f64().unwrap_or(0.0);

        let segments: Vec<TranscriptionSegment> = result["segments"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|seg| {
                        Some(TranscriptionSegment {
                            text: seg["text"].as_str()?.to_string(),
                            start: seg["start"].as_f64()? as f32,
                            end: seg["end"].as_f64()? as f32,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(TranscriptionResult {
            text,
            confidence: None,
            language,
            duration: Duration::from_secs_f64(duration_secs),
            segments,
        })
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

/// Local Whisper implementation using whisper.cpp (optional feature)
#[cfg(feature = "local-whisper")]
pub struct LocalWhisperInput {
    model_path: std::path::PathBuf,
    language: Option<String>,
}

#[cfg(feature = "local-whisper")]
impl LocalWhisperInput {
    /// Create a new local Whisper input
    pub fn new(model_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            language: None,
        }
    }

    /// Set the language hint
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }
}

#[cfg(feature = "local-whisper")]
#[async_trait]
impl VoiceInput for LocalWhisperInput {
    fn name(&self) -> &str {
        "Local Whisper"
    }

    async fn transcribe(&self, audio: &AudioData) -> Result<TranscriptionResult> {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        let model_path = self.model_path.clone();
        let samples = audio.samples.clone();
        let language = self.language.clone();

        // Run whisper in blocking task
        let result = tokio::task::spawn_blocking(move || {
            let ctx = WhisperContext::new_with_params(
                model_path.to_str().unwrap(),
                WhisperContextParameters::default(),
            )
            .map_err(|e| VoiceError::TranscriptionError(format!("Failed to load model: {}", e)))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            if let Some(lang) = language {
                params.set_language(Some(&lang));
            }

            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            let mut state = ctx.create_state().map_err(|e| {
                VoiceError::TranscriptionError(format!("Failed to create state: {}", e))
            })?;

            state.full(params, &samples).map_err(|e| {
                VoiceError::TranscriptionError(format!("Transcription failed: {}", e))
            })?;

            let num_segments = state.full_n_segments().map_err(|e| {
                VoiceError::TranscriptionError(format!("Failed to get segments: {}", e))
            })?;

            let mut text = String::new();
            let mut segments = Vec::new();

            for i in 0..num_segments {
                if let Ok(segment_text) = state.full_get_segment_text(i) {
                    text.push_str(&segment_text);

                    if let (Ok(start), Ok(end)) = (
                        state.full_get_segment_t0(i),
                        state.full_get_segment_t1(i),
                    ) {
                        segments.push(TranscriptionSegment {
                            text: segment_text,
                            start: start as f32 / 100.0,
                            end: end as f32 / 100.0,
                        });
                    }
                }
            }

            Ok::<_, VoiceError>(TranscriptionResult {
                text: text.trim().to_string(),
                confidence: None,
                language: None,
                duration: Duration::from_secs_f64(samples.len() as f64 / 16000.0),
                segments,
            })
        })
        .await
        .map_err(|e| VoiceError::TranscriptionError(format!("Task failed: {}", e)))??;

        Ok(result)
    }

    async fn is_available(&self) -> bool {
        self.model_path.exists()
    }
}

/// Stub for local whisper when feature is not enabled
#[cfg(not(feature = "local-whisper"))]
pub struct LocalWhisperInput {
    #[allow(dead_code)]
    model_path: std::path::PathBuf,
}

#[cfg(not(feature = "local-whisper"))]
impl LocalWhisperInput {
    pub fn new(model_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }
}

#[cfg(not(feature = "local-whisper"))]
#[async_trait]
impl VoiceInput for LocalWhisperInput {
    fn name(&self) -> &str {
        "Local Whisper (disabled)"
    }

    async fn transcribe(&self, _audio: &AudioData) -> Result<TranscriptionResult> {
        Err(VoiceError::FeatureDisabled(
            "local-whisper feature not enabled".to_string(),
        ))
    }

    async fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_data_creation() {
        let samples = vec![0.0f32; 16000]; // 1 second at 16kHz
        let audio = AudioData::new(samples, 16000, 1);
        assert_eq!(audio.sample_rate, 16000);
        assert_eq!(audio.channels, 1);
        assert!(audio.duration >= Duration::from_millis(900));
    }

    #[test]
    fn test_vad_config_default() {
        let config = VadConfig::default();
        assert!(config.voice_threshold > 0.0);
        assert!(config.silence_duration > Duration::ZERO);
    }
}
