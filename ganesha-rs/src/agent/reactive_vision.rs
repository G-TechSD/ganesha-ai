//! Reactive Vision System
//!
//! Actually looks at the screen and finds elements dynamically instead of
//! using hardcoded coordinates. Uses vision model for element detection
//! and OCR for text verification.

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::vision::VisionController;

/// Element location result from vision
#[derive(Debug, Clone)]
pub struct ElementLocation {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
    pub element_type: String,
    pub label: String,
}

impl ElementLocation {
    /// Get center coordinates for clicking
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width as i32 / 2),
            self.y + (self.height as i32 / 2)
        )
    }
}

/// Vision-based element finder
pub struct ReactiveVision {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    screen_width: u32,
    screen_height: u32,
}

impl ReactiveVision {
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            screen_width: 1920,
            screen_height: 1080,
        }
    }

    /// Find a specific UI element by description
    /// Returns coordinates if found
    pub async fn find_element(
        &self,
        vision: &VisionController,
        description: &str,
    ) -> Result<Option<ElementLocation>, Box<dyn std::error::Error + Send + Sync>> {
        // Take high-res screenshot for accurate detection
        let screenshot = vision.capture_screen()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let prompt = format!(
            r#"Find the UI element: "{}"

Screen is {}x{} pixels. Analyze the screenshot and locate this element.

RESPOND WITH EXACTLY THIS FORMAT (nothing else):
FOUND: yes/no
X: [left edge x coordinate, 0-{}]
Y: [top edge y coordinate, 0-{}]
WIDTH: [element width in pixels]
HEIGHT: [element height in pixels]
CONFIDENCE: [0.0-1.0]
TYPE: [button/text_field/menu/icon/link/window/dialog/label]

If the element is NOT visible, respond with:
FOUND: no

Be precise with coordinates. The element center will be used for clicking."#,
            description,
            self.screen_width, self.screen_height,
            self.screen_width - 1, self.screen_height - 1
        );

        let request = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }],
            "max_tokens": 150,
            "temperature": 0.1
        });

        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");

        self.parse_element_response(content, description)
    }

    /// Find multiple possible elements matching a description
    pub async fn find_elements(
        &self,
        vision: &VisionController,
        description: &str,
        max_results: usize,
    ) -> Result<Vec<ElementLocation>, Box<dyn std::error::Error + Send + Sync>> {
        let screenshot = vision.capture_screen()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let prompt = format!(
            r#"Find ALL UI elements matching: "{}"

Screen is {}x{} pixels. List up to {} matching elements.

For EACH element found, provide (one per line):
ELEMENT: [label] X:[x] Y:[y] W:[width] H:[height] CONF:[0.0-1.0] TYPE:[type]

If no elements match, respond with:
NONE FOUND

Example format:
ELEMENT: Save Button X:150 Y:400 W:80 H:30 CONF:0.9 TYPE:button
ELEMENT: Save menu item X:200 Y:250 W:100 H:25 CONF:0.7 TYPE:menu"#,
            description,
            self.screen_width, self.screen_height,
            max_results
        );

        let request = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }],
            "max_tokens": 300,
            "temperature": 0.1
        });

        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");

        self.parse_multi_element_response(content)
    }

    /// Analyze current screen state comprehensively
    pub async fn analyze_screen(
        &self,
        vision: &VisionController,
    ) -> Result<ScreenAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let screenshot = vision.capture_screen()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let prompt = r#"Analyze this screenshot comprehensively:

1. DESKTOP_STATE: normal | activities_overview | app_fullscreen | login | lock_screen | other
2. ACTIVE_WINDOW: [window title or "none"]
3. VISIBLE_WINDOWS: [comma-separated list of visible window titles]
4. DIALOGS: [any popup dialogs visible? describe them]
5. TASKBAR: visible | hidden
6. MOUSE_POSITION: [approximate x,y if visible]
7. FOCUS_ELEMENT: [what UI element appears to have focus?]
8. TEXT_FIELDS: [any text input fields visible? describe state]
9. BUTTONS: [list prominent buttons visible]
10. ERRORS: [any error messages visible?]
11. LOADING: [is anything loading/spinning?]

Format each on its own line as: FIELD: value"#;

        let request = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }],
            "max_tokens": 400,
            "temperature": 0.1
        });

        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");

        Ok(self.parse_screen_analysis(content))
    }

    /// Verify an action succeeded by comparing before/after
    pub async fn verify_action(
        &self,
        vision: &VisionController,
        expected_change: &str,
    ) -> Result<ActionVerification, Box<dyn std::error::Error + Send + Sync>> {
        let screenshot = vision.capture_screen()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let prompt = format!(
            r#"Verify if this expected change occurred: "{}"

Analyze the current screen state and determine:
1. Did the expected change happen?
2. What is the current state?
3. Are there any unexpected dialogs or errors?

Respond with:
SUCCESS: yes/no
CURRENT_STATE: [brief description]
UNEXPECTED: [any unexpected elements/dialogs/errors, or "none"]
CONFIDENCE: [0.0-1.0]
SUGGESTION: [if failed, what might help?]"#,
            expected_change
        );

        let request = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }],
            "max_tokens": 200,
            "temperature": 0.1
        });

        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");

        Ok(self.parse_verification(content))
    }

    /// Read text at a specific screen location
    pub async fn read_text_at(
        &self,
        vision: &VisionController,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Capture region if possible, or full screen with focus area
        let screenshot = vision.capture_screen()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let prompt = format!(
            r#"Read the text in the region from ({},{}) to ({},{}) on this {}x{} screen.

Return ONLY the text content, nothing else. If no readable text, respond with: [NO TEXT]"#,
            x, y, x + width as i32, y + height as i32,
            self.screen_width, self.screen_height
        );

        let request = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }],
            "max_tokens": 200,
            "temperature": 0.1
        });

        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        Ok(result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("[NO TEXT]")
            .to_string())
    }

    // ============ Parsing Helpers ============

    fn parse_element_response(
        &self,
        content: &str,
        description: &str,
    ) -> Result<Option<ElementLocation>, Box<dyn std::error::Error + Send + Sync>> {
        let content_lower = content.to_lowercase();

        if content_lower.contains("found: no") || content_lower.contains("not found") ||
           content_lower.contains("none found") || content_lower.contains("cannot find") {
            return Ok(None);
        }

        // Parse coordinates
        let x = self.extract_number(content, "X:");
        let y = self.extract_number(content, "Y:");
        let width = self.extract_number(content, "WIDTH:").unwrap_or(50) as u32;
        let height = self.extract_number(content, "HEIGHT:").unwrap_or(30) as u32;
        let confidence = self.extract_float(content, "CONFIDENCE:").unwrap_or(0.5);

        let element_type = self.extract_value(content, "TYPE:")
            .unwrap_or_else(|| "unknown".to_string());

        match (x, y) {
            (Some(x), Some(y)) if x >= 0 && y >= 0 &&
                                  x < self.screen_width as i32 &&
                                  y < self.screen_height as i32 => {
                Ok(Some(ElementLocation {
                    x,
                    y,
                    width,
                    height,
                    confidence,
                    element_type,
                    label: description.to_string(),
                }))
            }
            _ => Ok(None)
        }
    }

    fn parse_multi_element_response(
        &self,
        content: &str,
    ) -> Result<Vec<ElementLocation>, Box<dyn std::error::Error + Send + Sync>> {
        let mut elements = Vec::new();

        for line in content.lines() {
            if line.contains("ELEMENT:") {
                // Parse: ELEMENT: [label] X:[x] Y:[y] W:[width] H:[height] CONF:[conf] TYPE:[type]
                let label = self.extract_between(line, "ELEMENT:", "X:")
                    .unwrap_or_else(|| "element".to_string())
                    .trim()
                    .to_string();

                if let (Some(x), Some(y)) = (
                    self.extract_number(line, "X:"),
                    self.extract_number(line, "Y:")
                ) {
                    let width = self.extract_number(line, "W:").unwrap_or(50) as u32;
                    let height = self.extract_number(line, "H:").unwrap_or(30) as u32;
                    let confidence = self.extract_float(line, "CONF:").unwrap_or(0.5);
                    let element_type = self.extract_value(line, "TYPE:")
                        .unwrap_or_else(|| "unknown".to_string());

                    if x >= 0 && y >= 0 {
                        elements.push(ElementLocation {
                            x, y, width, height, confidence, element_type, label,
                        });
                    }
                }
            }
        }

        // Sort by confidence
        elements.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        Ok(elements)
    }

    fn parse_screen_analysis(&self, content: &str) -> ScreenAnalysis {
        ScreenAnalysis {
            desktop_state: self.extract_value(content, "DESKTOP_STATE:")
                .unwrap_or_else(|| "unknown".to_string()),
            active_window: self.extract_value(content, "ACTIVE_WINDOW:"),
            visible_windows: self.extract_value(content, "VISIBLE_WINDOWS:")
                .map(|s| s.split(',').map(|w| w.trim().to_string()).collect())
                .unwrap_or_default(),
            dialogs: self.extract_value(content, "DIALOGS:"),
            taskbar_visible: !content.to_lowercase().contains("taskbar: hidden"),
            focus_element: self.extract_value(content, "FOCUS_ELEMENT:"),
            has_errors: content.to_lowercase().contains("error"),
            is_loading: content.to_lowercase().contains("loading") ||
                        content.to_lowercase().contains("spinning"),
            raw: content.to_string(),
        }
    }

    fn parse_verification(&self, content: &str) -> ActionVerification {
        let success = content.to_lowercase().contains("success: yes");
        let confidence = self.extract_float(content, "CONFIDENCE:").unwrap_or(0.5);

        ActionVerification {
            success,
            confidence,
            current_state: self.extract_value(content, "CURRENT_STATE:")
                .unwrap_or_else(|| "unknown".to_string()),
            unexpected: self.extract_value(content, "UNEXPECTED:")
                .filter(|s| s.to_lowercase() != "none"),
            suggestion: self.extract_value(content, "SUGGESTION:"),
        }
    }

    fn extract_number(&self, text: &str, prefix: &str) -> Option<i32> {
        if let Some(idx) = text.find(prefix) {
            let after = &text[idx + prefix.len()..];
            let num_str: String = after.trim_start()
                .chars()
                .take_while(|c| c.is_numeric() || *c == '-')
                .collect();
            num_str.parse().ok()
        } else {
            None
        }
    }

    fn extract_float(&self, text: &str, prefix: &str) -> Option<f32> {
        if let Some(idx) = text.find(prefix) {
            let after = &text[idx + prefix.len()..];
            let num_str: String = after.trim_start()
                .chars()
                .take_while(|c| c.is_numeric() || *c == '.' || *c == '-')
                .collect();
            num_str.parse().ok()
        } else {
            None
        }
    }

    fn extract_value(&self, text: &str, prefix: &str) -> Option<String> {
        if let Some(idx) = text.find(prefix) {
            let after = &text[idx + prefix.len()..];
            let value: String = after.trim_start()
                .chars()
                .take_while(|c| *c != '\n' && *c != '\r')
                .collect();
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    fn extract_between(&self, text: &str, start: &str, end: &str) -> Option<String> {
        if let Some(start_idx) = text.find(start) {
            let after_start = &text[start_idx + start.len()..];
            if let Some(end_idx) = after_start.find(end) {
                return Some(after_start[..end_idx].to_string());
            }
        }
        None
    }
}

/// Comprehensive screen analysis result
#[derive(Debug, Clone)]
pub struct ScreenAnalysis {
    pub desktop_state: String,
    pub active_window: Option<String>,
    pub visible_windows: Vec<String>,
    pub dialogs: Option<String>,
    pub taskbar_visible: bool,
    pub focus_element: Option<String>,
    pub has_errors: bool,
    pub is_loading: bool,
    pub raw: String,
}

impl ScreenAnalysis {
    pub fn is_activities(&self) -> bool {
        self.desktop_state.contains("activities") || !self.taskbar_visible
    }

    pub fn has_dialog(&self) -> bool {
        self.dialogs.is_some() &&
        self.dialogs.as_ref().map(|d| !d.to_lowercase().contains("none")).unwrap_or(false)
    }

    pub fn has_window(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();
        self.visible_windows.iter().any(|w| w.to_lowercase().contains(&name_lower)) ||
        self.active_window.as_ref().map(|w| w.to_lowercase().contains(&name_lower)).unwrap_or(false)
    }
}

/// Action verification result
#[derive(Debug, Clone)]
pub struct ActionVerification {
    pub success: bool,
    pub confidence: f32,
    pub current_state: String,
    pub unexpected: Option<String>,
    pub suggestion: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_center() {
        let elem = ElementLocation {
            x: 100,
            y: 200,
            width: 80,
            height: 40,
            confidence: 0.9,
            element_type: "button".into(),
            label: "test".into(),
        };
        assert_eq!(elem.center(), (140, 220));
    }
}
