//! Vision Integration for Ganesha
//!
//! Screen analysis using vision models with strict JSON output.
//! The vision model returns lightweight JSON only.
//!
//! Used for:
//! - Confirming UI state
//! - Reading screen content
//! - Verifying action results
//! - Detecting errors/dialogs

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Screen analysis result - strict JSON format from vision model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenAnalysis {
    /// Active application name
    pub app: String,
    /// Window title
    pub title: String,
    /// Key UI elements visible
    pub elements: Vec<UiElement>,
    /// Any dialogs or popups
    pub dialogs: Vec<Dialog>,
    /// Text content visible (key snippets)
    pub text: Vec<String>,
    /// Screen state summary
    pub state: ScreenState,
    /// Confidence (0.0-1.0)
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    /// Type of element
    #[serde(rename = "type")]
    pub element_type: String,
    /// Label or identifier
    pub label: String,
    /// Approximate position (quadrant: tl, tr, bl, br, center)
    pub position: String,
    /// Is it interactive?
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dialog {
    /// Dialog type
    #[serde(rename = "type")]
    pub dialog_type: String,
    /// Title
    pub title: String,
    /// Message content
    pub message: String,
    /// Available buttons
    pub buttons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenState {
    Ready,
    Loading,
    Error,
    Dialog,
    Busy,
    Unknown,
}

/// Vision configuration
#[derive(Debug, Clone)]
pub struct VisionConfig {
    pub endpoint: String,
    pub model: String,
    pub timeout: Duration,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:1234/v1/chat/completions".into(),
            model: "default".into(),
            timeout: Duration::from_secs(30),
        }
    }
}

/// The vision analyzer
pub struct VisionAnalyzer {
    config: VisionConfig,
    client: reqwest::Client,
    is_anthropic: bool,
    api_key: Option<String>,
}

impl VisionAnalyzer {
    pub fn new(config: VisionConfig) -> Self {
        let is_anthropic = config.endpoint.contains("anthropic.com");
        let api_key = if is_anthropic {
            std::env::var("ANTHROPIC_API_KEY").ok()
        } else {
            None
        };

        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap();

        Self { config, client, is_anthropic, api_key }
    }

    pub fn with_defaults() -> Self {
        Self::new(VisionConfig::default())
    }

    /// Capture and analyze the current screen
    #[cfg(feature = "vision")]
    pub async fn analyze_screen(&self) -> Result<ScreenAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        use crate::vision::VisionController;

        let vision = VisionController::new();
        vision.enable()?;

        let screenshot = vision.capture_screen()?;
        let analysis = self.analyze_image(&screenshot.data).await?;

        vision.disable();
        Ok(analysis)
    }

    /// Analyze an image (base64 encoded)
    pub async fn analyze_image(&self, base64_image: &str) -> Result<ScreenAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let system_prompt = r#"OUTPUT ONLY RAW JSON. NO MARKDOWN. NO EXPLANATION. NO CODE BLOCKS.

Schema:
{"app":"name","title":"window title","elements":[{"type":"button","label":"text","position":"center","interactive":true}],"dialogs":[],"text":["visible text"],"state":"ready","confidence":0.9}

CRITICAL: Your entire response must be a single JSON object starting with { and ending with }. Nothing else."#;

        let response = if self.is_anthropic {
            // Anthropic API format with vision
            let user_content = serde_json::json!([
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/jpeg",
                        "data": base64_image
                    }
                },
                {
                    "type": "text",
                    "text": "Analyze this screen. Return JSON only."
                }
            ]);

            let request = serde_json::json!({
                "model": self.config.model,
                "max_tokens": 2000,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": user_content}
                ]
            });

            let mut req = self.client.post(&self.config.endpoint).json(&request);
            if let Some(ref key) = self.api_key {
                req = req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
            }
            req.send().await?
        } else {
            // OpenAI-compatible format
            let user_content = serde_json::json!([
                {
                    "type": "text",
                    "text": "Analyze this screen. Return JSON only."
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", base64_image)
                    }
                }
            ]);

            let request = serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content}
                ],
                "temperature": 0.1,
                "max_tokens": 2000
            });

            self.client
                .post(&self.config.endpoint)
                .json(&request)
                .send()
                .await?
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Vision API error {}: {}", status, body).into());
        }

        let json: serde_json::Value = response.json().await?;
        let content = if self.is_anthropic {
            // Anthropic response format
            json["content"][0]["text"].as_str().unwrap_or("{}")
        } else {
            // OpenAI response format - check both content and reasoning_content
            // Reasoning models (e.g. ministral-3-14b-reasoning) put output in reasoning_content
            let msg = &json["choices"][0]["message"];
            let c = msg["content"].as_str().unwrap_or("");
            if c.is_empty() {
                msg["reasoning_content"].as_str().unwrap_or("{}")
            } else {
                c
            }
        };

        // Parse the JSON response
        self.parse_analysis(content)
    }

    /// Analyze screen with a specific query
    pub async fn query_screen(&self, base64_image: &str, query: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let system_prompt = r#"You are a screen analyzer. Answer the user's question about the screen briefly and precisely. Keep response under 50 words."#;

        let response = if self.is_anthropic {
            // Anthropic API format with vision
            let user_content = serde_json::json!([
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/jpeg",
                        "data": base64_image
                    }
                },
                {
                    "type": "text",
                    "text": query
                }
            ]);

            let request = serde_json::json!({
                "model": self.config.model,
                "max_tokens": 100,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": user_content}
                ]
            });

            let mut req = self.client.post(&self.config.endpoint).json(&request);
            if let Some(ref key) = self.api_key {
                req = req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
            }
            req.send().await?
        } else {
            // OpenAI-compatible format
            let user_content = serde_json::json!([
                {
                    "type": "text",
                    "text": query
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", base64_image)
                    }
                }
            ]);

            let request = serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content}
                ],
                "temperature": 0.1,
                "max_tokens": 100
            });

            self.client
                .post(&self.config.endpoint)
                .json(&request)
                .send()
                .await?
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Vision API error {}: {}", status, body).into());
        }

        let json: serde_json::Value = response.json().await?;
        let content = if self.is_anthropic {
            json["content"][0]["text"].as_str().unwrap_or("")
        } else {
            // Check both content and reasoning_content for reasoning models
            let msg = &json["choices"][0]["message"];
            let c = msg["content"].as_str().unwrap_or("");
            if c.is_empty() {
                msg["reasoning_content"].as_str().unwrap_or("")
            } else {
                c
            }
        };
        Ok(content.to_string())
    }

    /// Check if a specific element is visible
    pub async fn is_visible(&self, base64_image: &str, element: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let query = format!("Is '{}' visible on screen? Answer YES or NO only.", element);
        let response = self.query_screen(base64_image, &query).await?;
        Ok(response.to_uppercase().contains("YES"))
    }

    /// Wait for an element to appear
    #[cfg(feature = "vision")]
    pub async fn wait_for_element(
        &self,
        element: &str,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        use crate::vision::VisionController;
        use std::time::Instant;

        let start = Instant::now();
        let vision = VisionController::new();
        vision.enable()?;

        while start.elapsed() < timeout {
            if let Ok(screenshot) = vision.capture_screen() {
                if self.is_visible(&screenshot.data, element).await? {
                    vision.disable();
                    return Ok(true);
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        vision.disable();
        Ok(false)
    }

    /// Detect screen state changes
    #[cfg(feature = "vision")]
    pub async fn detect_change(
        &self,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        use crate::vision::VisionController;
        use std::time::Instant;

        let vision = VisionController::new();
        vision.enable()?;

        let initial = vision.capture_screen()?.data;
        let start = Instant::now();

        while start.elapsed() < timeout {
            tokio::time::sleep(Duration::from_millis(200)).await;

            if let Ok(current) = vision.capture_screen() {
                // Simple change detection: compare lengths (real impl would hash)
                if current.data.len() != initial.len() {
                    vision.disable();
                    return Ok(true);
                }
            }
        }

        vision.disable();
        Ok(false)
    }

    /// Parse the vision model's JSON response
    fn parse_analysis(&self, content: &str) -> Result<ScreenAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        // Strip markdown code blocks if present
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        
        // Try to find JSON in the response
        let json_str = if cleaned.starts_with('{') {
            cleaned.to_string()
        } else if let Some(start) = cleaned.find('{') {
            if let Some(end) = cleaned.rfind('}') {
                cleaned[start..=end].to_string()
            } else {
                return Err("No valid JSON found in response".into());
            }
        } else {
            return Err("No JSON found in response".into());
        };

        // Parse with defaults for missing fields
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        Ok(ScreenAnalysis {
            app: parsed["app"].as_str().unwrap_or("Unknown").to_string(),
            title: parsed["title"].as_str().unwrap_or("").to_string(),
            elements: parsed["elements"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
            dialogs: parsed["dialogs"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|d| serde_json::from_value(d.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
            text: parsed["text"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            state: serde_json::from_value(
                parsed["state"].clone()
            ).unwrap_or(ScreenState::Unknown),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.5) as f32,
        })
    }
}

impl Default for VisionAnalyzer {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Quick screen check without full analysis
pub async fn quick_check(query: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(feature = "vision")]
    {
        use crate::vision::VisionController;

        let vision = VisionController::new();
        vision.enable()?;
        let screenshot = vision.capture_screen()?;
        vision.disable();

        let analyzer = VisionAnalyzer::with_defaults();
        analyzer.query_screen(&screenshot.data, query).await
    }

    #[cfg(not(feature = "vision"))]
    {
        Err("Vision feature not enabled. Compile with --features vision".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_config_default() {
        let config = VisionConfig::default();
        assert!(config.endpoint.contains("localhost"));
    }

    #[test]
    fn test_parse_analysis() {
        let analyzer = VisionAnalyzer::with_defaults();

        let json = r#"{
            "app": "Firefox",
            "title": "Google",
            "elements": [{"type": "button", "label": "Search", "position": "center", "interactive": true}],
            "dialogs": [],
            "text": ["Google Search"],
            "state": "ready",
            "confidence": 0.95
        }"#;

        let result = analyzer.parse_analysis(json).unwrap();
        assert_eq!(result.app, "Firefox");
        assert_eq!(result.elements.len(), 1);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_screen_state_parsing() {
        let states = ["ready", "loading", "error", "dialog", "busy", "unknown"];
        for state in states {
            let json = format!(r#"{{"app": "test", "title": "", "elements": [], "dialogs": [], "text": [], "state": "{}", "confidence": 0.5}}"#, state);
            let analyzer = VisionAnalyzer::with_defaults();
            let result = analyzer.parse_analysis(&json).unwrap();
            assert!(matches!(result.state, ScreenState::Ready | ScreenState::Loading | ScreenState::Error | ScreenState::Dialog | ScreenState::Busy | ScreenState::Unknown));
        }
    }
}
