//! Element Locator
//!
//! Finds exact screen coordinates for UI elements using vision models.
//! Inspired by OmniParser approach - use vision to locate clickable targets.

use serde_json::json;
use std::time::Duration;

/// Locates UI elements on screen using vision models
pub struct ElementLocator {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl ElementLocator {
    pub fn new(endpoint: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            endpoint,
            model,
            client,
        }
    }

    /// Locate an element on screen by description
    ///
    /// Returns: (x, y, confidence)
    pub async fn locate_element(
        &self,
        base64_image: &str,
        element_description: &str,
    ) -> Result<(i32, i32, f32), Box<dyn std::error::Error + Send + Sync>> {
        let system_prompt = r#"You are a UI element locator. Given an image and element description, find the element's center coordinates.

The image is 1280x720 pixels. Respond ONLY with JSON:
{"x": <center_x>, "y": <center_y>, "confidence": <0.0-1.0>, "found": true}

If element not found:
{"found": false, "reason": "why not found"}

Be precise. Consider:
- Buttons are usually rectangular, click center
- Text fields: click left side for cursor
- Checkboxes/radios: click the box itself
- Menu items: click center of text
- Icons: click center of icon"#;

        let user_content = json!([
            {
                "type": "text",
                "text": format!("Find the element: {}\n\nReturn coordinates in 1280x720 space.", element_description)
            },
            {
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", base64_image)
                }
            }
        ]);

        let request = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "temperature": 0.1,
            "max_tokens": 100
        });

        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Locator API error {}: {}", status, body).into());
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");

        self.parse_location(content, element_description)
    }

    /// Locate multiple elements at once
    pub async fn locate_elements(
        &self,
        base64_image: &str,
        descriptions: &[&str],
    ) -> Vec<Result<(i32, i32, f32), String>> {
        let mut results = Vec::with_capacity(descriptions.len());

        // Could be optimized with a single prompt, but this is clearer
        for desc in descriptions {
            match self.locate_element(base64_image, desc).await {
                Ok((x, y, conf)) => results.push(Ok((x, y, conf))),
                Err(e) => results.push(Err(e.to_string())),
            }
        }

        results
    }

    /// Find all interactive elements on screen
    pub async fn find_interactive_elements(
        &self,
        base64_image: &str,
    ) -> Result<Vec<InteractiveElement>, Box<dyn std::error::Error + Send + Sync>> {
        let system_prompt = r#"You are a UI analyzer. List all interactive elements visible on screen.

Respond ONLY with JSON array:
[
  {"type": "button|input|link|checkbox|menu|icon|tab", "label": "text", "x": 100, "y": 200, "w": 80, "h": 30},
  ...
]

Rules:
- Coordinates in 1280x720 space
- Include clickable buttons, links, inputs, checkboxes, tabs, icons
- x,y is top-left corner; w,h is size
- Skip decorative/non-interactive elements
- Max 20 elements, prioritize by visibility/importance"#;

        let user_content = json!([
            {
                "type": "text",
                "text": "List all interactive elements on this screen."
            },
            {
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", base64_image)
                }
            }
        ]);

        let request = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "temperature": 0.1,
            "max_tokens": 1000
        });

        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Interactive scan API error {}: {}", status, body).into());
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("[]");

        self.parse_interactive_elements(content)
    }

    /// Parse location response
    fn parse_location(
        &self,
        content: &str,
        description: &str,
    ) -> Result<(i32, i32, f32), Box<dyn std::error::Error + Send + Sync>> {
        // Extract JSON
        let json_str = if content.starts_with('{') || content.starts_with('[') {
            content.to_string()
        } else if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                content[start..=end].to_string()
            } else {
                return Err("Invalid JSON in response".into());
            }
        } else {
            return Err("No JSON found".into());
        };

        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if !parsed.get("found").and_then(|v| v.as_bool()).unwrap_or(true) {
            let reason = parsed["reason"].as_str().unwrap_or("Element not found");
            return Err(format!("Could not find '{}': {}", description, reason).into());
        }

        let x = parsed["x"].as_i64().ok_or("Missing x coordinate")? as i32;
        let y = parsed["y"].as_i64().ok_or("Missing y coordinate")? as i32;
        let confidence = parsed["confidence"].as_f64().unwrap_or(0.5) as f32;

        // Sanity check coordinates
        if x < 0 || x > 2560 || y < 0 || y > 1440 {
            return Err(format!("Coordinates out of bounds: ({}, {})", x, y).into());
        }

        Ok((x, y, confidence))
    }

    /// Parse interactive elements list
    fn parse_interactive_elements(
        &self,
        content: &str,
    ) -> Result<Vec<InteractiveElement>, Box<dyn std::error::Error + Send + Sync>> {
        // Extract JSON array
        let json_str = if content.starts_with('[') {
            content.to_string()
        } else if let Some(start) = content.find('[') {
            if let Some(end) = content.rfind(']') {
                content[start..=end].to_string()
            } else {
                return Ok(vec![]);
            }
        } else {
            return Ok(vec![]);
        };

        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str)?;

        let elements = parsed
            .into_iter()
            .filter_map(|v| {
                Some(InteractiveElement {
                    element_type: v["type"].as_str()?.to_string(),
                    label: v["label"].as_str()?.to_string(),
                    x: v["x"].as_i64()? as i32,
                    y: v["y"].as_i64()? as i32,
                    width: v["w"].as_i64().unwrap_or(50) as i32,
                    height: v["h"].as_i64().unwrap_or(20) as i32,
                })
            })
            .collect();

        Ok(elements)
    }

    /// Get the clickable center of a bounding box
    pub fn bbox_center(x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
        (x + w / 2, y + h / 2)
    }
}

/// An interactive UI element with location
#[derive(Debug, Clone)]
pub struct InteractiveElement {
    pub element_type: String,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl InteractiveElement {
    /// Get clickable center point
    pub fn center(&self) -> (i32, i32) {
        ElementLocator::bbox_center(self.x, self.y, self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_center() {
        let (cx, cy) = ElementLocator::bbox_center(100, 200, 80, 30);
        assert_eq!(cx, 140);
        assert_eq!(cy, 215);
    }

    #[test]
    fn test_parse_location() {
        let locator = ElementLocator::new("http://test".into(), "test".into());

        let result = locator
            .parse_location(r#"{"x": 500, "y": 300, "confidence": 0.9, "found": true}"#, "button")
            .unwrap();

        assert_eq!(result.0, 500);
        assert_eq!(result.1, 300);
        assert!((result.2 - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_parse_not_found() {
        let locator = ElementLocator::new("http://test".into(), "test".into());

        let result = locator.parse_location(
            r#"{"found": false, "reason": "No such button visible"}"#,
            "save button",
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("save button"));
    }
}
