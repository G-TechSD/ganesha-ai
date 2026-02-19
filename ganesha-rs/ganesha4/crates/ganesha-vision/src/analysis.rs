//! Image analysis for the Vision/VLA system.
//!
//! This module provides:
//! - Vision model integration (GPT-4V, Claude, Gemini)
//! - UI element detection (buttons, text fields, menus)
//! - OCR/text extraction from screenshots
//! - Element location with bounding boxes
//! - State detection (enabled/disabled, checked/unchecked)

use crate::capture::{Region, Screenshot};
use crate::config::{CaptureSettings, VisionConfig, VisionModel};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during image analysis.
#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Vision model error: {0}")]
    ModelError(String),

    #[error("API request failed: {0}")]
    ApiError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("No API key found for {0}")]
    MissingApiKey(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Analysis timeout")]
    Timeout,
}

/// Result type for analysis operations.
pub type AnalysisResult<T> = Result<T, AnalysisError>;

/// Type of UI element detected in a screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Button,
    TextField,
    TextArea,
    Checkbox,
    RadioButton,
    Dropdown,
    Menu,
    MenuItem,
    Tab,
    Slider,
    ScrollBar,
    Link,
    Image,
    Icon,
    Label,
    Dialog,
    Window,
    Toolbar,
    StatusBar,
    TreeView,
    ListView,
    Table,
    Panel,
    Unknown,
}

impl ElementType {
    /// Check if this element type is interactive.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::TextField
                | Self::TextArea
                | Self::Checkbox
                | Self::RadioButton
                | Self::Dropdown
                | Self::Menu
                | Self::MenuItem
                | Self::Tab
                | Self::Slider
                | Self::Link
        )
    }

    /// Check if this element type can contain text input.
    pub fn accepts_text_input(&self) -> bool {
        matches!(self, Self::TextField | Self::TextArea | Self::Dropdown)
    }
}

/// State of a UI element.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ElementState {
    /// Whether the element is enabled
    pub enabled: bool,
    /// Whether the element is visible
    pub visible: bool,
    /// Whether the element has focus
    pub focused: bool,
    /// Whether the element is selected (for checkboxes, radio buttons)
    pub selected: Option<bool>,
    /// Whether the element is expanded (for dropdowns, trees)
    pub expanded: Option<bool>,
    /// Current value (for sliders, text fields)
    pub value: Option<String>,
}

/// A detected UI element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    /// Unique identifier for this element
    pub id: String,
    /// Type of UI element
    pub element_type: ElementType,
    /// Bounding box in screen coordinates
    pub bounds: Region,
    /// Text content or label
    pub text: Option<String>,
    /// Current state
    pub state: ElementState,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Additional attributes
    pub attributes: HashMap<String, String>,
}

impl UIElement {
    /// Get the center point of this element.
    pub fn center(&self) -> (i32, i32) {
        self.bounds.center()
    }

    /// Check if this element can be clicked.
    pub fn is_clickable(&self) -> bool {
        self.element_type.is_interactive() && self.state.enabled && self.state.visible
    }
}

/// Text extracted from a screenshot (OCR result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedText {
    /// The extracted text content
    pub text: String,
    /// Bounding box of the text
    pub bounds: Region,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Whether this is a single word or a text block
    pub is_word: bool,
}

/// Complete analysis result from a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenAnalysis {
    /// Detected UI elements
    pub elements: Vec<UIElement>,
    /// Extracted text blocks
    pub text_blocks: Vec<ExtractedText>,
    /// Overall description of the screen
    pub description: String,
    /// Detected application context
    pub app_context: Option<AppContext>,
    /// Raw response from the vision model
    pub raw_response: Option<String>,
    /// Analysis timestamp
    pub timestamp: i64,
}

impl ScreenAnalysis {
    /// Find an element by its text content.
    pub fn find_by_text(&self, text: &str) -> Option<&UIElement> {
        let text_lower = text.to_lowercase();
        self.elements.iter().find(|e| {
            e.text
                .as_ref()
                .map(|t| t.to_lowercase().contains(&text_lower))
                .unwrap_or(false)
        })
    }

    /// Find all elements of a specific type.
    pub fn find_by_type(&self, element_type: ElementType) -> Vec<&UIElement> {
        self.elements
            .iter()
            .filter(|e| e.element_type == element_type)
            .collect()
    }

    /// Find clickable elements.
    pub fn find_clickable(&self) -> Vec<&UIElement> {
        self.elements.iter().filter(|e| e.is_clickable()).collect()
    }

    /// Find an element at a specific location.
    pub fn find_at(&self, x: i32, y: i32) -> Option<&UIElement> {
        self.elements.iter().find(|e| e.bounds.contains(x, y))
    }

    /// Get all text content as a single string.
    pub fn all_text(&self) -> String {
        self.text_blocks
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Detected application context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    /// Application name
    pub app_name: String,
    /// Current view or screen
    pub current_view: Option<String>,
    /// Available actions in this context
    pub available_actions: Vec<String>,
    /// Any detected errors or warnings
    pub alerts: Vec<String>,
}

/// Trait for vision model analyzers.
#[async_trait]
pub trait VisionAnalyzer: Send + Sync {
    /// Analyze a screenshot and detect UI elements.
    async fn analyze(&self, screenshot: &Screenshot, prompt: Option<&str>)
        -> AnalysisResult<ScreenAnalysis>;

    /// Extract text from a screenshot (OCR).
    async fn extract_text(&self, screenshot: &Screenshot) -> AnalysisResult<Vec<ExtractedText>>;

    /// Find a specific element by description.
    async fn find_element(
        &self,
        screenshot: &Screenshot,
        description: &str,
    ) -> AnalysisResult<Option<UIElement>>;

    /// Answer a question about the screenshot.
    async fn ask(&self, screenshot: &Screenshot, question: &str) -> AnalysisResult<String>;
}

/// Vision analyzer using OpenAI GPT-4 Vision.
pub struct Gpt4VisionAnalyzer {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
    model: String,
    capture_settings: CaptureSettings,
}

impl Gpt4VisionAnalyzer {
    /// Create a new GPT-4 Vision analyzer.
    pub fn new(config: &VisionConfig) -> AnalysisResult<Self> {
        let api_key = std::env::var(&config.api_key_env)
            .map_err(|_| AnalysisError::MissingApiKey(config.api_key_env.clone()))?;

        let endpoint = config
            .api_endpoint
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            endpoint,
            model: config.model.model_id().to_string(),
            capture_settings: config.capture.clone(),
        })
    }

    async fn call_api(&self, messages: Vec<serde_json::Value>) -> AnalysisResult<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": 4096
        });

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        if response.status() == 429 {
            return Err(AnalysisError::RateLimited(
                "API rate limit exceeded".to_string(),
            ));
        }

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AnalysisError::ApiError(error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AnalysisError::InvalidResponse("Missing content in response".to_string()))
    }
}

#[async_trait]
impl VisionAnalyzer for Gpt4VisionAnalyzer {
    async fn analyze(
        &self,
        screenshot: &Screenshot,
        prompt: Option<&str>,
    ) -> AnalysisResult<ScreenAnalysis> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let default_prompt = r#"Analyze this screenshot and identify all UI elements.
For each element, provide:
- Type (button, text_field, checkbox, etc.)
- Bounding box coordinates (x, y, width, height)
- Text content if any
- State (enabled, visible, selected, etc.)

Also provide a brief description of the overall screen and detected application context.

Return the analysis as JSON with this structure:
{
    "description": "Brief description of the screen",
    "app_context": {
        "app_name": "Application name",
        "current_view": "Current view or screen",
        "available_actions": ["action1", "action2"],
        "alerts": []
    },
    "elements": [
        {
            "id": "unique_id",
            "element_type": "button",
            "bounds": {"x": 0, "y": 0, "width": 100, "height": 30},
            "text": "Button text",
            "state": {"enabled": true, "visible": true, "focused": false},
            "confidence": 0.95
        }
    ],
    "text_blocks": [
        {
            "text": "Extracted text",
            "bounds": {"x": 0, "y": 0, "width": 100, "height": 20},
            "confidence": 0.9,
            "is_word": false
        }
    ]
}"#;

        let system_prompt = prompt.unwrap_or(default_prompt);

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": system_prompt
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}",
                            self.capture_settings.format.mime_type(),
                            base64_image
                        )
                    }
                }
            ]
        })];

        let response = self.call_api(messages).await?;

        // Try to parse as JSON
        let parsed: serde_json::Value = serde_json::from_str(&response)
            .or_else(|_| {
                // Try to extract JSON from the response
                if let Some(start) = response.find('{') {
                    if let Some(end) = response.rfind('}') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No JSON found in response",
                )))
            })
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        // Parse elements
        let elements: Vec<UIElement> = parsed["elements"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(UIElement {
                            id: e["id"].as_str()?.to_string(),
                            element_type: serde_json::from_value(e["element_type"].clone())
                                .unwrap_or(ElementType::Unknown),
                            bounds: Region::new(
                                e["bounds"]["x"].as_i64()? as i32,
                                e["bounds"]["y"].as_i64()? as i32,
                                e["bounds"]["width"].as_u64()? as u32,
                                e["bounds"]["height"].as_u64()? as u32,
                            ),
                            text: e["text"].as_str().map(|s| s.to_string()),
                            state: ElementState {
                                enabled: e["state"]["enabled"].as_bool().unwrap_or(true),
                                visible: e["state"]["visible"].as_bool().unwrap_or(true),
                                focused: e["state"]["focused"].as_bool().unwrap_or(false),
                                selected: e["state"]["selected"].as_bool(),
                                expanded: e["state"]["expanded"].as_bool(),
                                value: e["state"]["value"].as_str().map(|s| s.to_string()),
                            },
                            confidence: e["confidence"].as_f64().unwrap_or(0.5) as f32,
                            attributes: HashMap::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse text blocks
        let text_blocks: Vec<ExtractedText> = parsed["text_blocks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        Some(ExtractedText {
                            text: t["text"].as_str()?.to_string(),
                            bounds: Region::new(
                                t["bounds"]["x"].as_i64()? as i32,
                                t["bounds"]["y"].as_i64()? as i32,
                                t["bounds"]["width"].as_u64()? as u32,
                                t["bounds"]["height"].as_u64()? as u32,
                            ),
                            confidence: t["confidence"].as_f64().unwrap_or(0.5) as f32,
                            is_word: t["is_word"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse app context
        let app_context = parsed["app_context"].as_object().map(|ctx| AppContext {
            app_name: ctx
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            current_view: ctx.get("current_view").and_then(|v| v.as_str()).map(|s| s.to_string()),
            available_actions: ctx
                .get("available_actions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            alerts: ctx
                .get("alerts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        });

        Ok(ScreenAnalysis {
            elements,
            text_blocks,
            description: parsed["description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            app_context,
            raw_response: Some(response),
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    async fn extract_text(&self, screenshot: &Screenshot) -> AnalysisResult<Vec<ExtractedText>> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "Extract all text from this screenshot. Return as JSON array with format: [{\"text\": \"...\", \"bounds\": {\"x\": 0, \"y\": 0, \"width\": 100, \"height\": 20}, \"confidence\": 0.95, \"is_word\": true}]"
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}",
                            self.capture_settings.format.mime_type(),
                            base64_image
                        )
                    }
                }
            ]
        })];

        let response = self.call_api(messages).await?;

        // Parse response
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&response)
            .or_else(|_| {
                if let Some(start) = response.find('[') {
                    if let Some(end) = response.rfind(']') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Ok(Vec::new())
            })
            .unwrap_or_default();

        Ok(parsed
            .into_iter()
            .filter_map(|t| {
                Some(ExtractedText {
                    text: t["text"].as_str()?.to_string(),
                    bounds: Region::new(
                        t["bounds"]["x"].as_i64().unwrap_or(0) as i32,
                        t["bounds"]["y"].as_i64().unwrap_or(0) as i32,
                        t["bounds"]["width"].as_u64().unwrap_or(100) as u32,
                        t["bounds"]["height"].as_u64().unwrap_or(20) as u32,
                    ),
                    confidence: t["confidence"].as_f64().unwrap_or(0.5) as f32,
                    is_word: t["is_word"].as_bool().unwrap_or(false),
                })
            })
            .collect())
    }

    async fn find_element(
        &self,
        screenshot: &Screenshot,
        description: &str,
    ) -> AnalysisResult<Option<UIElement>> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": format!(
                        "Find the UI element matching this description: \"{}\"\n\nReturn as JSON: {{\"found\": true/false, \"element\": {{\"id\": \"...\", \"element_type\": \"button\", \"bounds\": {{\"x\": 0, \"y\": 0, \"width\": 100, \"height\": 30}}, \"text\": \"...\", \"confidence\": 0.95}}}}",
                        description
                    )
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}",
                            self.capture_settings.format.mime_type(),
                            base64_image
                        )
                    }
                }
            ]
        })];

        let response = self.call_api(messages).await?;

        // Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response)
            .or_else(|_| {
                if let Some(start) = response.find('{') {
                    if let Some(end) = response.rfind('}') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No JSON found",
                )))
            })
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        if !parsed["found"].as_bool().unwrap_or(false) {
            return Ok(None);
        }

        let e = &parsed["element"];
        Ok(Some(UIElement {
            id: e["id"]
                .as_str()
                .unwrap_or("found_element")
                .to_string(),
            element_type: serde_json::from_value(e["element_type"].clone())
                .unwrap_or(ElementType::Unknown),
            bounds: Region::new(
                e["bounds"]["x"].as_i64().unwrap_or(0) as i32,
                e["bounds"]["y"].as_i64().unwrap_or(0) as i32,
                e["bounds"]["width"].as_u64().unwrap_or(100) as u32,
                e["bounds"]["height"].as_u64().unwrap_or(30) as u32,
            ),
            text: e["text"].as_str().map(|s| s.to_string()),
            state: ElementState {
                enabled: true,
                visible: true,
                ..Default::default()
            },
            confidence: e["confidence"].as_f64().unwrap_or(0.5) as f32,
            attributes: HashMap::new(),
        }))
    }

    async fn ask(&self, screenshot: &Screenshot, question: &str) -> AnalysisResult<String> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": question
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}",
                            self.capture_settings.format.mime_type(),
                            base64_image
                        )
                    }
                }
            ]
        })];

        self.call_api(messages).await
    }
}

/// Vision analyzer using Anthropic Claude.
pub struct ClaudeVisionAnalyzer {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
    model: String,
    capture_settings: CaptureSettings,
}

impl ClaudeVisionAnalyzer {
    /// Create a new Claude Vision analyzer.
    pub fn new(config: &VisionConfig) -> AnalysisResult<Self> {
        let api_key = std::env::var(&config.api_key_env)
            .map_err(|_| AnalysisError::MissingApiKey(config.api_key_env.clone()))?;

        let endpoint = config
            .api_endpoint
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            endpoint,
            model: config.model.model_id().to_string(),
            capture_settings: config.capture.clone(),
        })
    }

    async fn call_api(&self, content: Vec<serde_json::Value>) -> AnalysisResult<String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [{
                "role": "user",
                "content": content
            }]
        });

        let response = self
            .client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        if response.status() == 429 {
            return Err(AnalysisError::RateLimited(
                "API rate limit exceeded".to_string(),
            ));
        }

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AnalysisError::ApiError(error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AnalysisError::InvalidResponse("Missing text in response".to_string()))
    }
}

#[async_trait]
impl VisionAnalyzer for ClaudeVisionAnalyzer {
    async fn analyze(
        &self,
        screenshot: &Screenshot,
        prompt: Option<&str>,
    ) -> AnalysisResult<ScreenAnalysis> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let default_prompt = r#"Analyze this screenshot and identify all UI elements.
Return the analysis as JSON with this structure:
{
    "description": "Brief description of the screen",
    "app_context": {"app_name": "...", "current_view": "...", "available_actions": [], "alerts": []},
    "elements": [{"id": "...", "element_type": "button", "bounds": {"x": 0, "y": 0, "width": 100, "height": 30}, "text": "...", "state": {"enabled": true, "visible": true}, "confidence": 0.95}],
    "text_blocks": [{"text": "...", "bounds": {"x": 0, "y": 0, "width": 100, "height": 20}, "confidence": 0.9, "is_word": false}]
}"#;

        let content = vec![
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": self.capture_settings.format.mime_type(),
                    "data": base64_image
                }
            }),
            serde_json::json!({
                "type": "text",
                "text": prompt.unwrap_or(default_prompt)
            }),
        ];

        let response = self.call_api(content).await?;

        // Parse the JSON response (same parsing logic as GPT-4V)
        let parsed: serde_json::Value = serde_json::from_str(&response)
            .or_else(|_| {
                if let Some(start) = response.find('{') {
                    if let Some(end) = response.rfind('}') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No JSON found in response",
                )))
            })
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        // Parse elements (same as GPT-4V)
        let elements: Vec<UIElement> = parsed["elements"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(UIElement {
                            id: e["id"].as_str()?.to_string(),
                            element_type: serde_json::from_value(e["element_type"].clone())
                                .unwrap_or(ElementType::Unknown),
                            bounds: Region::new(
                                e["bounds"]["x"].as_i64()? as i32,
                                e["bounds"]["y"].as_i64()? as i32,
                                e["bounds"]["width"].as_u64()? as u32,
                                e["bounds"]["height"].as_u64()? as u32,
                            ),
                            text: e["text"].as_str().map(|s| s.to_string()),
                            state: ElementState {
                                enabled: e["state"]["enabled"].as_bool().unwrap_or(true),
                                visible: e["state"]["visible"].as_bool().unwrap_or(true),
                                focused: e["state"]["focused"].as_bool().unwrap_or(false),
                                selected: e["state"]["selected"].as_bool(),
                                expanded: e["state"]["expanded"].as_bool(),
                                value: e["state"]["value"].as_str().map(|s| s.to_string()),
                            },
                            confidence: e["confidence"].as_f64().unwrap_or(0.5) as f32,
                            attributes: HashMap::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let text_blocks: Vec<ExtractedText> = parsed["text_blocks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        Some(ExtractedText {
                            text: t["text"].as_str()?.to_string(),
                            bounds: Region::new(
                                t["bounds"]["x"].as_i64()? as i32,
                                t["bounds"]["y"].as_i64()? as i32,
                                t["bounds"]["width"].as_u64()? as u32,
                                t["bounds"]["height"].as_u64()? as u32,
                            ),
                            confidence: t["confidence"].as_f64().unwrap_or(0.5) as f32,
                            is_word: t["is_word"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let app_context = parsed["app_context"].as_object().map(|ctx| AppContext {
            app_name: ctx
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            current_view: ctx.get("current_view").and_then(|v| v.as_str()).map(|s| s.to_string()),
            available_actions: ctx
                .get("available_actions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            alerts: ctx
                .get("alerts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        });

        Ok(ScreenAnalysis {
            elements,
            text_blocks,
            description: parsed["description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            app_context,
            raw_response: Some(response),
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    async fn extract_text(&self, screenshot: &Screenshot) -> AnalysisResult<Vec<ExtractedText>> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let content = vec![
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": self.capture_settings.format.mime_type(),
                    "data": base64_image
                }
            }),
            serde_json::json!({
                "type": "text",
                "text": "Extract all text from this screenshot. Return as JSON array: [{\"text\": \"...\", \"bounds\": {\"x\": 0, \"y\": 0, \"width\": 100, \"height\": 20}, \"confidence\": 0.95, \"is_word\": true}]"
            }),
        ];

        let response = self.call_api(content).await?;

        let parsed: Vec<serde_json::Value> = serde_json::from_str(&response)
            .or_else(|_| {
                if let Some(start) = response.find('[') {
                    if let Some(end) = response.rfind(']') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Ok(Vec::new())
            })
            .unwrap_or_default();

        Ok(parsed
            .into_iter()
            .filter_map(|t| {
                Some(ExtractedText {
                    text: t["text"].as_str()?.to_string(),
                    bounds: Region::new(
                        t["bounds"]["x"].as_i64().unwrap_or(0) as i32,
                        t["bounds"]["y"].as_i64().unwrap_or(0) as i32,
                        t["bounds"]["width"].as_u64().unwrap_or(100) as u32,
                        t["bounds"]["height"].as_u64().unwrap_or(20) as u32,
                    ),
                    confidence: t["confidence"].as_f64().unwrap_or(0.5) as f32,
                    is_word: t["is_word"].as_bool().unwrap_or(false),
                })
            })
            .collect())
    }

    async fn find_element(
        &self,
        screenshot: &Screenshot,
        description: &str,
    ) -> AnalysisResult<Option<UIElement>> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let content = vec![
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": self.capture_settings.format.mime_type(),
                    "data": base64_image
                }
            }),
            serde_json::json!({
                "type": "text",
                "text": format!(
                    "Find the UI element matching: \"{}\"\nReturn JSON: {{\"found\": true/false, \"element\": {{\"id\": \"...\", \"element_type\": \"button\", \"bounds\": {{\"x\": 0, \"y\": 0, \"width\": 100, \"height\": 30}}, \"text\": \"...\", \"confidence\": 0.95}}}}",
                    description
                )
            }),
        ];

        let response = self.call_api(content).await?;

        let parsed: serde_json::Value = serde_json::from_str(&response)
            .or_else(|_| {
                if let Some(start) = response.find('{') {
                    if let Some(end) = response.rfind('}') {
                        return serde_json::from_str(&response[start..=end]);
                    }
                }
                Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No JSON found",
                )))
            })
            .map_err(|e| AnalysisError::InvalidResponse(e.to_string()))?;

        if !parsed["found"].as_bool().unwrap_or(false) {
            return Ok(None);
        }

        let e = &parsed["element"];
        Ok(Some(UIElement {
            id: e["id"]
                .as_str()
                .unwrap_or("found_element")
                .to_string(),
            element_type: serde_json::from_value(e["element_type"].clone())
                .unwrap_or(ElementType::Unknown),
            bounds: Region::new(
                e["bounds"]["x"].as_i64().unwrap_or(0) as i32,
                e["bounds"]["y"].as_i64().unwrap_or(0) as i32,
                e["bounds"]["width"].as_u64().unwrap_or(100) as u32,
                e["bounds"]["height"].as_u64().unwrap_or(30) as u32,
            ),
            text: e["text"].as_str().map(|s| s.to_string()),
            state: ElementState {
                enabled: true,
                visible: true,
                ..Default::default()
            },
            confidence: e["confidence"].as_f64().unwrap_or(0.5) as f32,
            attributes: HashMap::new(),
        }))
    }

    async fn ask(&self, screenshot: &Screenshot, question: &str) -> AnalysisResult<String> {
        let base64_image = screenshot
            .to_base64(&self.capture_settings)
            .map_err(|e| AnalysisError::ModelError(e.to_string()))?;

        let content = vec![
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": self.capture_settings.format.mime_type(),
                    "data": base64_image
                }
            }),
            serde_json::json!({
                "type": "text",
                "text": question
            }),
        ];

        self.call_api(content).await
    }
}

/// Create a vision analyzer based on configuration.
pub fn create_analyzer(config: &VisionConfig) -> AnalysisResult<Box<dyn VisionAnalyzer>> {
    match config.model {
        VisionModel::Gpt4Vision => Ok(Box::new(Gpt4VisionAnalyzer::new(config)?)),
        VisionModel::ClaudeVision => Ok(Box::new(ClaudeVisionAnalyzer::new(config)?)),
        VisionModel::GeminiVision => {
            // TODO: Implement Gemini analyzer
            Err(AnalysisError::ModelError(
                "Gemini Vision not yet implemented".to_string(),
            ))
        }
        VisionModel::Local => {
            // TODO: Implement local model analyzer
            Err(AnalysisError::ModelError(
                "Local vision model not yet implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_element(id: &str, etype: ElementType, x: i32, y: i32, w: u32, h: u32, text: Option<&str>) -> UIElement {
        UIElement {
            id: id.to_string(),
            element_type: etype,
            bounds: Region::new(x, y, w, h),
            text: text.map(|s| s.to_string()),
            state: ElementState { enabled: true, visible: true, ..Default::default() },
            confidence: 0.95,
            attributes: HashMap::new(),
        }
    }

    fn make_analysis(elements: Vec<UIElement>) -> ScreenAnalysis {
        ScreenAnalysis {
            elements,
            text_blocks: vec![],
            description: String::new(),
            app_context: None,
            raw_response: None,
            timestamp: 0,
        }
    }

    #[test]
    fn test_find_by_text() {
        let analysis = make_analysis(vec![
            make_element("btn1", ElementType::Button, 0, 0, 100, 30, Some("Save File")),
        ]);
        assert!(analysis.find_by_text("save").is_some());
        assert!(analysis.find_by_text("delete").is_none());
    }

    #[test]
    fn test_element_type_is_interactive() {
        assert!(ElementType::Button.is_interactive());
        assert!(ElementType::TextField.is_interactive());
        assert!(!ElementType::Label.is_interactive());
        assert!(!ElementType::Image.is_interactive());
    }

    #[test]
    fn test_element_type_accepts_text() {
        assert!(ElementType::TextField.accepts_text_input());
        assert!(ElementType::TextArea.accepts_text_input());
        assert!(!ElementType::Button.accepts_text_input());
    }

    #[test]
    fn test_ui_element_center() {
        let elem = make_element("a", ElementType::Button, 100, 200, 50, 30, None);
        let (cx, cy) = elem.center();
        assert_eq!(cx, 125);
        assert_eq!(cy, 215);
    }

    #[test]
    fn test_ui_element_is_clickable() {
        let btn = make_element("b", ElementType::Button, 0, 0, 10, 10, None);
        assert!(btn.is_clickable());
        let lbl = make_element("l", ElementType::Label, 0, 0, 10, 10, None);
        assert!(!lbl.is_clickable());
    }

    #[test]
    fn test_find_by_type() {
        let analysis = make_analysis(vec![
            make_element("b1", ElementType::Button, 0, 0, 10, 10, Some("OK")),
            make_element("t1", ElementType::TextField, 0, 20, 100, 10, None),
            make_element("b2", ElementType::Button, 0, 40, 10, 10, Some("Cancel")),
        ]);
        let buttons = analysis.find_by_type(ElementType::Button);
        assert_eq!(buttons.len(), 2);
    }

    #[test]
    fn test_find_clickable() {
        let analysis = make_analysis(vec![
            make_element("b1", ElementType::Button, 0, 0, 10, 10, Some("OK")),
            make_element("l1", ElementType::Label, 0, 20, 100, 10, Some("Title")),
            make_element("lk", ElementType::Link, 0, 40, 100, 10, Some("Click me")),
        ]);
        let clickable = analysis.find_clickable();
        assert!(clickable.len() >= 2);
    }

    #[test]
    fn test_find_at() {
        let analysis = make_analysis(vec![
            make_element("b1", ElementType::Button, 10, 10, 50, 30, Some("OK")),
        ]);
        assert!(analysis.find_at(20, 20).is_some());
        assert!(analysis.find_at(200, 200).is_none());
    }

    #[test]
    fn test_all_text() {
        let analysis = ScreenAnalysis {
            elements: vec![],
            text_blocks: vec![
                ExtractedText { text: "Hello".to_string(), bounds: Region::new(0,0,10,10), confidence: 0.9, is_word: true },
                ExtractedText { text: "World".to_string(), bounds: Region::new(0,20,10,10), confidence: 0.9, is_word: true },
            ],
            description: String::new(),
            app_context: None,
            raw_response: None,
            timestamp: 0,
        };
        let text = analysis.all_text();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_analysis_error_variants() {
        let _ = AnalysisError::ModelError("test".to_string());
        let _ = AnalysisError::ApiError("test".to_string());
        let _ = AnalysisError::InvalidResponse("test".to_string());
        let _ = AnalysisError::MissingApiKey("gpt4v".to_string());
    }

    #[test]
    fn test_empty_analysis() {
        let analysis = make_analysis(vec![]);
        assert!(analysis.find_by_text("anything").is_none());
        assert!(analysis.find_clickable().is_empty());
        assert!(analysis.all_text().is_empty());
    }
}
