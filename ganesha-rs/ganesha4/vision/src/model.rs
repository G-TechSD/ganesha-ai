//! Vision model integration for local LLM endpoints.
//!
//! This module provides integration with local vision models via OpenAI-compatible APIs
//! (like LM Studio, Ollama, or other local inference servers).
//!
//! # Features
//!
//! - Multiple model endpoint support (planning + vision models)
//! - Base64 image transmission for screenshots
//! - Structured JSON response parsing
//! - Configurable timeouts and retry logic
//!
//! # Example
//!
//! ```no_run
//! use ganesha_vision::model::{VisionClient, VisionModelConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = VisionModelConfig::default();
//!     let client = VisionClient::new(config)?;
//!
//!     // Analyze a screenshot
//!     // let analysis = client.analyze_screen(&screenshot).await?;
//!     Ok(())
//! }
//! ```

use base64::{engine::general_purpose::STANDARD, Engine};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during vision model operations.
#[derive(Error, Debug)]
pub enum VisionModelError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// Invalid response from the model
    #[error("Invalid model response: {0}")]
    InvalidResponse(String),

    /// JSON parsing error
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Image encoding error
    #[error("Image encoding error: {0}")]
    ImageError(String),

    /// Request timed out
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    /// Model returned an error
    #[error("Model error: {0}")]
    ModelError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Rate limited
    #[error("Rate limited: {0}")]
    RateLimited(String),
}

/// Result type for vision model operations.
pub type Result<T> = std::result::Result<T, VisionModelError>;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for a vision model endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionModelConfig {
    /// API endpoint URL (OpenAI-compatible)
    pub endpoint: String,

    /// Model name/identifier
    pub model_name: String,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Optional API key for authenticated endpoints
    pub api_key: Option<String>,

    /// Maximum tokens in response
    pub max_tokens: u32,

    /// Temperature for response generation (0.0-2.0)
    pub temperature: f32,

    /// Image format for encoding (png, jpeg, webp)
    pub image_format: ImageFormat,

    /// JPEG quality (1-100) if using JPEG format
    pub jpeg_quality: u8,

    /// Maximum image dimension (images larger than this are scaled down)
    pub max_image_dimension: u32,
}

impl Default for VisionModelConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:1234/v1/chat/completions".to_string(),
            model_name: "local-model".to_string(),
            timeout: Duration::from_secs(60),
            api_key: None,
            max_tokens: 4096,
            temperature: 0.1,
            image_format: ImageFormat::Png,
            jpeg_quality: 85,
            max_image_dimension: 1920,
        }
    }
}

impl VisionModelConfig {
    /// Create a new configuration with custom endpoint.
    pub fn new(endpoint: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model_name: model_name.into(),
            ..Default::default()
        }
    }

    /// Create configuration for LM Studio defaults.
    pub fn lm_studio() -> Self {
        Self {
            endpoint: "http://localhost:1234/v1/chat/completions".to_string(),
            model_name: "local-model".to_string(),
            ..Default::default()
        }
    }

    /// Create configuration for Ollama defaults.
    pub fn ollama(model_name: impl Into<String>) -> Self {
        Self {
            endpoint: "http://localhost:11434/v1/chat/completions".to_string(),
            model_name: model_name.into(),
            ..Default::default()
        }
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.0, 2.0);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set image format.
    pub fn with_image_format(mut self, format: ImageFormat) -> Self {
        self.image_format = format;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            return Err(VisionModelError::ConfigError(
                "Endpoint cannot be empty".to_string(),
            ));
        }

        if self.model_name.is_empty() {
            return Err(VisionModelError::ConfigError(
                "Model name cannot be empty".to_string(),
            ));
        }

        if self.max_tokens == 0 {
            return Err(VisionModelError::ConfigError(
                "Max tokens must be greater than 0".to_string(),
            ));
        }

        if self.jpeg_quality == 0 || self.jpeg_quality > 100 {
            return Err(VisionModelError::ConfigError(
                "JPEG quality must be between 1 and 100".to_string(),
            ));
        }

        Ok(())
    }
}

/// Image format for encoding screenshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
    WebP,
}

impl ImageFormat {
    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }

    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }
}

// ============================================================================
// Screenshot Type
// ============================================================================

/// A screenshot captured from the screen.
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// The image data
    pub image: DynamicImage,

    /// Timestamp when captured (Unix milliseconds)
    pub timestamp: i64,

    /// Source description (e.g., "Monitor 1", "Firefox window")
    pub source: String,

    /// Optional region bounds (x, y, width, height)
    pub bounds: Option<(i32, i32, u32, u32)>,
}

impl Screenshot {
    /// Create a new screenshot from an image.
    pub fn new(image: DynamicImage, source: impl Into<String>) -> Self {
        Self {
            image,
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: source.into(),
            bounds: None,
        }
    }

    /// Set the bounds of the screenshot.
    pub fn with_bounds(mut self, x: i32, y: i32, width: u32, height: u32) -> Self {
        self.bounds = Some((x, y, width, height));
        self
    }

    /// Get the width of the image.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Get the height of the image.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Encode the screenshot to base64.
    pub fn to_base64(&self, config: &VisionModelConfig) -> Result<String> {
        let mut buffer = Cursor::new(Vec::new());

        // Scale down if necessary
        let image = if self.image.width() > config.max_image_dimension
            || self.image.height() > config.max_image_dimension
        {
            let scale = config.max_image_dimension as f64
                / self.image.width().max(self.image.height()) as f64;
            let new_width = (self.image.width() as f64 * scale) as u32;
            let new_height = (self.image.height() as f64 * scale) as u32;
            self.image
                .resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
        } else {
            self.image.clone()
        };

        match config.image_format {
            ImageFormat::Png => {
                image
                    .write_to(&mut buffer, image::ImageFormat::Png)
                    .map_err(|e| VisionModelError::ImageError(e.to_string()))?;
            }
            ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut buffer,
                    config.jpeg_quality,
                );
                image
                    .write_with_encoder(encoder)
                    .map_err(|e| VisionModelError::ImageError(e.to_string()))?;
            }
            ImageFormat::WebP => {
                image
                    .write_to(&mut buffer, image::ImageFormat::WebP)
                    .map_err(|e| VisionModelError::ImageError(e.to_string()))?;
            }
        }

        Ok(STANDARD.encode(buffer.into_inner()))
    }
}

// ============================================================================
// Screen Analysis Types
// ============================================================================

/// A UI element detected in the screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    /// Type of element (button, input, menu, icon, text, etc.)
    pub element_type: String,

    /// Label or text content of the element
    pub label: String,

    /// Bounding box: (x, y, width, height)
    /// None if the model couldn't determine precise bounds
    pub bounds: Option<(i32, i32, u32, u32)>,

    /// Whether this element is interactive (clickable, editable, etc.)
    pub interactive: bool,
}

impl UIElement {
    /// Create a new UI element.
    pub fn new(element_type: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            element_type: element_type.into(),
            label: label.into(),
            bounds: None,
            interactive: false,
        }
    }

    /// Set the bounds.
    pub fn with_bounds(mut self, x: i32, y: i32, width: u32, height: u32) -> Self {
        self.bounds = Some((x, y, width, height));
        self
    }

    /// Set as interactive.
    pub fn with_interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Get the center point of this element (if bounds are known).
    pub fn center(&self) -> Option<(i32, i32)> {
        self.bounds.map(|(x, y, w, h)| {
            (x + (w as i32 / 2), y + (h as i32 / 2))
        })
    }

    /// Check if a point is within this element's bounds.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        if let Some((x, y, w, h)) = self.bounds {
            px >= x && px < x + w as i32 && py >= y && py < y + h as i32
        } else {
            false
        }
    }
}

/// Complete analysis result from a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenAnalysis {
    /// Detected application name
    pub app_name: String,

    /// Window title
    pub window_title: String,

    /// Detected UI elements
    pub ui_elements: Vec<UIElement>,

    /// All visible text found in the screenshot
    pub visible_text: Vec<String>,

    /// Suggested actions the user could take
    pub suggested_actions: Vec<String>,

    /// Overall confidence score (0.0 to 1.0)
    pub confidence: f32,
}

impl Default for ScreenAnalysis {
    fn default() -> Self {
        Self {
            app_name: String::new(),
            window_title: String::new(),
            ui_elements: Vec::new(),
            visible_text: Vec::new(),
            suggested_actions: Vec::new(),
            confidence: 0.0,
        }
    }
}

impl ScreenAnalysis {
    /// Find elements by type.
    pub fn find_by_type(&self, element_type: &str) -> Vec<&UIElement> {
        self.ui_elements
            .iter()
            .filter(|e| e.element_type.eq_ignore_ascii_case(element_type))
            .collect()
    }

    /// Find elements by label (partial match, case-insensitive).
    pub fn find_by_label(&self, label: &str) -> Vec<&UIElement> {
        let label_lower = label.to_lowercase();
        self.ui_elements
            .iter()
            .filter(|e| e.label.to_lowercase().contains(&label_lower))
            .collect()
    }

    /// Find interactive elements only.
    pub fn find_interactive(&self) -> Vec<&UIElement> {
        self.ui_elements.iter().filter(|e| e.interactive).collect()
    }

    /// Find element at a specific point.
    pub fn find_at(&self, x: i32, y: i32) -> Option<&UIElement> {
        self.ui_elements.iter().find(|e| e.contains(x, y))
    }

    /// Get all text as a single string.
    pub fn all_text(&self) -> String {
        self.visible_text.join(" ")
    }
}

// ============================================================================
// Multi-Endpoint Configuration
// ============================================================================

/// Configuration for a dual-model setup (planning + vision).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualModelConfig {
    /// Primary model for planning and reasoning
    pub planning_model: VisionModelConfig,

    /// Secondary model for vision/image analysis
    pub vision_model: VisionModelConfig,
}

impl Default for DualModelConfig {
    fn default() -> Self {
        Self {
            planning_model: VisionModelConfig {
                model_name: "planning-model".to_string(),
                ..VisionModelConfig::default()
            },
            vision_model: VisionModelConfig {
                model_name: "vision-model".to_string(),
                ..VisionModelConfig::default()
            },
        }
    }
}

// ============================================================================
// OpenAI-Compatible API Types
// ============================================================================

/// Message content for the OpenAI API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum MessageContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// Image URL structure for the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Chat message for the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: Vec<MessageContent>,
}

/// Chat completion request.
#[derive(Debug, Clone, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

/// Chat completion response.
#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    error: Option<ApiError>,
}

/// Response choice.
#[derive(Debug, Clone, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

/// Response message.
#[derive(Debug, Clone, Deserialize)]
struct ResponseMessage {
    content: String,
}

/// API error.
#[derive(Debug, Clone, Deserialize)]
struct ApiError {
    message: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    error_type: Option<String>,
}

// ============================================================================
// Vision Client
// ============================================================================

/// Client for making vision model API calls.
pub struct VisionClient {
    /// HTTP client
    client: reqwest::Client,

    /// Primary configuration (or vision model in dual setup)
    config: VisionModelConfig,

    /// Optional secondary configuration for planning
    planning_config: Option<VisionModelConfig>,
}

impl VisionClient {
    /// Create a new vision client with a single model configuration.
    pub fn new(config: VisionModelConfig) -> Result<Self> {
        config.validate()?;

        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(VisionModelError::RequestFailed)?;

        Ok(Self {
            client,
            config,
            planning_config: None,
        })
    }

    /// Create a new vision client with dual model configuration.
    pub fn with_dual_models(dual_config: DualModelConfig) -> Result<Self> {
        dual_config.vision_model.validate()?;
        dual_config.planning_model.validate()?;

        let timeout = dual_config
            .vision_model
            .timeout
            .max(dual_config.planning_model.timeout);

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(VisionModelError::RequestFailed)?;

        Ok(Self {
            client,
            config: dual_config.vision_model,
            planning_config: Some(dual_config.planning_model),
        })
    }

    /// Get the vision model configuration.
    pub fn config(&self) -> &VisionModelConfig {
        &self.config
    }

    /// Get the planning model configuration (if using dual models).
    pub fn planning_config(&self) -> Option<&VisionModelConfig> {
        self.planning_config.as_ref()
    }

    /// Make a raw API call to the vision model.
    async fn call_api(
        &self,
        config: &VisionModelConfig,
        messages: Vec<ChatMessage>,
    ) -> Result<String> {
        let request = ChatCompletionRequest {
            model: config.model_name.clone(),
            messages,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        };

        debug!("Sending request to {}", config.endpoint);

        let mut req = self
            .client
            .post(&config.endpoint)
            .header("Content-Type", "application/json");

        if let Some(ref api_key) = config.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    VisionModelError::Timeout(config.timeout)
                } else {
                    VisionModelError::RequestFailed(e)
                }
            })?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(VisionModelError::RateLimited(
                "Rate limit exceeded".to_string(),
            ));
        }

        let body = response.text().await.map_err(VisionModelError::RequestFailed)?;

        if !status.is_success() {
            error!("API error ({}): {}", status, body);
            return Err(VisionModelError::ModelError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let completion: ChatCompletionResponse =
            serde_json::from_str(&body).map_err(|e| {
                warn!("Failed to parse response: {}", body);
                VisionModelError::InvalidResponse(format!("JSON parse error: {}", e))
            })?;

        if let Some(error) = completion.error {
            return Err(VisionModelError::ModelError(error.message));
        }

        completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| VisionModelError::InvalidResponse("No choices in response".to_string()))
    }

    /// Create a message with an image.
    fn create_image_message(
        &self,
        screenshot: &Screenshot,
        prompt: &str,
    ) -> Result<ChatMessage> {
        let base64_image = screenshot.to_base64(&self.config)?;
        let data_url = format!(
            "data:{};base64,{}",
            self.config.image_format.mime_type(),
            base64_image
        );

        Ok(ChatMessage {
            role: "user".to_string(),
            content: vec![
                MessageContent::Text {
                    text: prompt.to_string(),
                },
                MessageContent::ImageUrl {
                    image_url: ImageUrl {
                        url: data_url,
                        detail: Some("high".to_string()),
                    },
                },
            ],
        })
    }

    /// Analyze a screenshot and return structured information.
    pub async fn analyze_screen(&self, screenshot: &Screenshot) -> Result<ScreenAnalysis> {
        let prompt = r#"Analyze this screenshot and provide a JSON response with the following structure:
{
    "app_name": "Name of the application",
    "window_title": "Window title text",
    "ui_elements": [
        {
            "element_type": "button|input|menu|icon|text|checkbox|dropdown|tab|link|image|toolbar|statusbar|dialog|panel|other",
            "label": "Text or description of the element",
            "bounds": [x, y, width, height] or null if unknown,
            "interactive": true or false
        }
    ],
    "visible_text": ["All", "visible", "text", "strings"],
    "suggested_actions": ["Click X button", "Type in search field", "etc"],
    "confidence": 0.0 to 1.0
}

Focus on identifying:
1. The application and its current state
2. All interactive elements (buttons, inputs, menus)
3. Text content visible on screen
4. What actions are currently possible

Return ONLY valid JSON, no additional text."#;

        let message = self.create_image_message(screenshot, prompt)?;
        let response = self.call_api(&self.config, vec![message]).await?;

        self.parse_screen_analysis(&response)
    }

    /// Parse the screen analysis response.
    fn parse_screen_analysis(&self, response: &str) -> Result<ScreenAnalysis> {
        // Try to extract JSON from the response (in case there's extra text)
        let json_str = self.extract_json(response)?;

        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        let app_name = parsed["app_name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let window_title = parsed["window_title"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let ui_elements: Vec<UIElement> = parsed["ui_elements"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let element_type = e["element_type"].as_str()?.to_string();
                        let label = e["label"].as_str().unwrap_or("").to_string();
                        let interactive = e["interactive"].as_bool().unwrap_or(false);

                        let bounds = e["bounds"].as_array().and_then(|b| {
                            if b.len() == 4 {
                                Some((
                                    b[0].as_i64()? as i32,
                                    b[1].as_i64()? as i32,
                                    b[2].as_u64()? as u32,
                                    b[3].as_u64()? as u32,
                                ))
                            } else {
                                None
                            }
                        });

                        Some(UIElement {
                            element_type,
                            label,
                            bounds,
                            interactive,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let visible_text: Vec<String> = parsed["visible_text"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let suggested_actions: Vec<String> = parsed["suggested_actions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let confidence = parsed["confidence"].as_f64().unwrap_or(0.5) as f32;

        Ok(ScreenAnalysis {
            app_name,
            window_title,
            ui_elements,
            visible_text,
            suggested_actions,
            confidence,
        })
    }

    /// Ask a question about the screenshot.
    pub async fn ask_about_screen(
        &self,
        screenshot: &Screenshot,
        question: &str,
    ) -> Result<String> {
        let prompt = format!(
            "Look at this screenshot and answer the following question:\n\n{}\n\nProvide a clear, concise answer.",
            question
        );

        let message = self.create_image_message(screenshot, &prompt)?;
        self.call_api(&self.config, vec![message]).await
    }

    /// Find a specific element by description.
    pub async fn find_element(
        &self,
        screenshot: &Screenshot,
        description: &str,
    ) -> Result<Option<UIElement>> {
        let prompt = format!(
            r#"Find the UI element matching this description: "{}"

If found, return JSON:
{{
    "found": true,
    "element": {{
        "element_type": "button|input|menu|icon|text|etc",
        "label": "Element text or description",
        "bounds": [x, y, width, height] or null,
        "interactive": true or false
    }}
}}

If not found, return:
{{
    "found": false,
    "element": null
}}

Return ONLY valid JSON."#,
            description
        );

        let message = self.create_image_message(screenshot, &prompt)?;
        let response = self.call_api(&self.config, vec![message]).await?;

        let json_str = self.extract_json(&response)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if !parsed["found"].as_bool().unwrap_or(false) {
            return Ok(None);
        }

        let e = &parsed["element"];
        if e.is_null() {
            return Ok(None);
        }

        let element_type = e["element_type"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let label = e["label"].as_str().unwrap_or("").to_string();
        let interactive = e["interactive"].as_bool().unwrap_or(false);

        let bounds = e["bounds"].as_array().and_then(|b| {
            if b.len() == 4 {
                Some((
                    b[0].as_i64()? as i32,
                    b[1].as_i64()? as i32,
                    b[2].as_u64()? as u32,
                    b[3].as_u64()? as u32,
                ))
            } else {
                None
            }
        });

        Ok(Some(UIElement {
            element_type,
            label,
            bounds,
            interactive,
        }))
    }

    /// Use the planning model (if configured) for reasoning tasks.
    pub async fn plan_actions(
        &self,
        screenshot: &Screenshot,
        goal: &str,
    ) -> Result<Vec<String>> {
        let config = self.planning_config.as_ref().unwrap_or(&self.config);

        let prompt = format!(
            r#"Given this screenshot, plan the steps needed to achieve this goal: "{}"

Return a JSON array of action steps:
["Step 1: Click on...", "Step 2: Type...", "Step 3: ..."]

Be specific about which elements to interact with.
Return ONLY the JSON array."#,
            goal
        );

        let message = self.create_image_message(screenshot, &prompt)?;
        let response = self.call_api(config, vec![message]).await?;

        let json_str = self.extract_json(&response)?;
        let actions: Vec<String> = serde_json::from_str(&json_str)?;

        Ok(actions)
    }

    /// Extract text from the screenshot (OCR-like functionality).
    pub async fn extract_text(&self, screenshot: &Screenshot) -> Result<Vec<String>> {
        let prompt = r#"Extract ALL visible text from this screenshot.
Return a JSON array of text strings found:
["Text 1", "Text 2", "Text 3"]

Include button labels, menu items, titles, body text, etc.
Return ONLY the JSON array."#;

        let message = self.create_image_message(screenshot, prompt)?;
        let response = self.call_api(&self.config, vec![message]).await?;

        let json_str = self.extract_json(&response)?;
        let texts: Vec<String> = serde_json::from_str(&json_str)?;

        Ok(texts)
    }

    /// Extract JSON from a response that might contain extra text.
    fn extract_json(&self, response: &str) -> Result<String> {
        let trimmed = response.trim();

        // If it starts with { or [, assume it's pure JSON
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            // Find the matching closing bracket
            let (open, close) = if trimmed.starts_with('{') {
                ('{', '}')
            } else {
                ('[', ']')
            };

            let mut depth = 0;
            let mut end_idx = 0;

            for (i, c) in trimmed.chars().enumerate() {
                if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = i + 1;
                        break;
                    }
                }
            }

            if end_idx > 0 {
                return Ok(trimmed[..end_idx].to_string());
            }
        }

        // Try to find JSON within the response
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return Ok(trimmed[start..=end].to_string());
                }
            }
        }

        if let Some(start) = trimmed.find('[') {
            if let Some(end) = trimmed.rfind(']') {
                if end > start {
                    return Ok(trimmed[start..=end].to_string());
                }
            }
        }

        Err(VisionModelError::InvalidResponse(
            "No JSON found in response".to_string(),
        ))
    }

    /// Check if the endpoint is reachable.
    pub async fn health_check(&self) -> Result<bool> {
        // Try a simple request to check connectivity
        let message = ChatMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "Say 'ok'".to_string(),
            }],
        };

        match self.call_api(&self.config, vec![message]).await {
            Ok(_) => Ok(true),
            Err(VisionModelError::Timeout(_)) => Ok(false),
            Err(VisionModelError::RequestFailed(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

// ============================================================================
// Humantime Serde Module
// ============================================================================

mod humantime_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Analyze a screenshot using default LM Studio configuration.
pub async fn analyze_screen(screenshot: &Screenshot) -> Result<ScreenAnalysis> {
    let client = VisionClient::new(VisionModelConfig::lm_studio())?;
    client.analyze_screen(screenshot).await
}

/// Ask a question about a screenshot using default configuration.
pub async fn ask_about_screen(screenshot: &Screenshot, question: &str) -> Result<String> {
    let client = VisionClient::new(VisionModelConfig::lm_studio())?;
    client.ask_about_screen(screenshot, question).await
}

/// Find an element in a screenshot using default configuration.
pub async fn find_element(
    screenshot: &Screenshot,
    description: &str,
) -> Result<Option<UIElement>> {
    let client = VisionClient::new(VisionModelConfig::lm_studio())?;
    client.find_element(screenshot, description).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VisionModelConfig::default();
        assert_eq!(config.endpoint, "http://localhost:1234/v1/chat/completions");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_lm_studio_config() {
        let config = VisionModelConfig::lm_studio();
        assert_eq!(config.endpoint, "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn test_ollama_config() {
        let config = VisionModelConfig::ollama("llava");
        assert_eq!(config.endpoint, "http://localhost:11434/v1/chat/completions");
        assert_eq!(config.model_name, "llava");
    }

    #[test]
    fn test_config_validation() {
        let mut config = VisionModelConfig::default();
        assert!(config.validate().is_ok());

        config.endpoint = String::new();
        assert!(config.validate().is_err());

        config.endpoint = "http://localhost:1234".to_string();
        config.model_name = String::new();
        assert!(config.validate().is_err());

        config.model_name = "model".to_string();
        config.max_tokens = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_image_format() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }

    #[test]
    fn test_ui_element() {
        let elem = UIElement::new("button", "Click me")
            .with_bounds(100, 200, 80, 30)
            .with_interactive(true);

        assert_eq!(elem.center(), Some((140, 215)));
        assert!(elem.contains(120, 210));
        assert!(!elem.contains(0, 0));
    }

    #[test]
    fn test_screen_analysis() {
        let analysis = ScreenAnalysis {
            app_name: "Firefox".to_string(),
            window_title: "Test Page".to_string(),
            ui_elements: vec![
                UIElement::new("button", "Submit").with_interactive(true),
                UIElement::new("input", "Search").with_interactive(true),
                UIElement::new("text", "Welcome"),
            ],
            visible_text: vec!["Welcome".to_string(), "Submit".to_string()],
            suggested_actions: vec!["Click Submit".to_string()],
            confidence: 0.9,
        };

        assert_eq!(analysis.find_by_type("button").len(), 1);
        assert_eq!(analysis.find_by_label("sub").len(), 1);
        assert_eq!(analysis.find_interactive().len(), 2);
    }

    #[test]
    fn test_extract_json() {
        let client = VisionClient::new(VisionModelConfig::default()).unwrap();

        // Pure JSON
        let json = r#"{"key": "value"}"#;
        assert_eq!(client.extract_json(json).unwrap(), json);

        // JSON with surrounding text
        let response = "Here is the result: {\"key\": \"value\"} That's all.";
        assert_eq!(
            client.extract_json(response).unwrap(),
            r#"{"key": "value"}"#
        );

        // JSON array
        let array = r#"["a", "b", "c"]"#;
        assert_eq!(client.extract_json(array).unwrap(), array);

        // No JSON
        let no_json = "No JSON here";
        assert!(client.extract_json(no_json).is_err());
    }
}
