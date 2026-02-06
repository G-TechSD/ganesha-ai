//! Action Planner
//!
//! Given a goal and current screen state, plans the next action.

use super::*;
use crate::orchestrator::vision::ScreenAnalysis;
use serde_json::json;
use std::time::Duration;

/// Plans actions based on goal and screen state
pub struct ActionPlanner {
    endpoint: String,
    model: String,
    client: reqwest::Client,
    is_anthropic: bool,
    api_key: Option<String>,
}

impl ActionPlanner {
    pub fn new(endpoint: String, model: String) -> Self {
        let is_anthropic = endpoint.contains("anthropic.com");
        let api_key = if is_anthropic {
            std::env::var("ANTHROPIC_API_KEY").ok()
        } else {
            None
        };
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            endpoint,
            model,
            client,
            is_anthropic,
            api_key,
        }
    }

    /// Plan the next action based on current state
    /// Plan next action with full context:
    /// - base64_image: screenshot for visual coordinate estimation
    /// - db_context: past actions, known failures from SQLite task tracker
    pub async fn plan_next_action(
        &self,
        goal: &VlaGoal,
        screen: &ScreenAnalysis,
        history: &[ActionResult],
        base64_image: Option<&str>,
        db_context: &str,
    ) -> Result<Option<PlannedAction>, Box<dyn std::error::Error + Send + Sync>> {
        let system_prompt = r#"You are a GUI automation planner. Given a goal, screenshot, and screen state, output the SINGLE next action to take.

SYSTEM CONTEXT:
- OS: Ubuntu 24.04 with GNOME desktop
- Screen: 1920x1080 actual, captured as 1280x720 (coordinates in 1280x720 space)
- Left dock occupies x=0-50. Main content starts at x~55.

KEYBOARD-FIRST STRATEGY (strongly prefer over mouse clicks):
- Web browser: Ctrl+L (address bar), Ctrl+F (find text), Tab (next element), Enter (activate), Ctrl+T (new tab)
- To click a link by name: Ctrl+F to find it, Escape to close find, Enter to follow
- To navigate to URL: Ctrl+L, type URL, Enter
- GNOME desktop: Super (activities), Alt+Tab (switch windows), Alt+F4 (close)
- File manager: Ctrl+L (path bar), Type path, Enter
- General: Ctrl+S (save), Ctrl+Z (undo), Ctrl+C/V (copy/paste)

ONLY use mouse clicks when keyboard navigation is impossible (e.g., clicking specific UI buttons, dock icons, or non-text elements).

Respond ONLY with valid JSON matching this schema:
{
  "intent": "what this action accomplishes",
  "action_type": "click|double_click|right_click|type|key_press|scroll|wait|hover|drag",
  "target": {"description": "element description", "x": 0, "y": 0},
  "text": "text to type if action_type is type",
  "keys": "key combo if action_type is key_press (e.g., 'ctrl+s')",
  "confidence": 0.9,
  "expected_result": "what should happen after this action"
}

If the goal appears achieved, respond with: {"done": true, "reason": "why goal is complete"}
If stuck with no viable action, respond with: {"stuck": true, "reason": "why we can't proceed"}

Rules:
- ONE action at a time
- ALWAYS prefer keyboard shortcuts and navigation over mouse clicks
- For mouse clicks: coordinates are in 1280x720 space. Safe area: x=55-1250, y=30-690
- Use 'wait' if expecting loading/transition"#;

        let history_summary = if history.is_empty() {
            "No actions taken yet.".to_string()
        } else {
            history.iter().rev().take(5).map(|h| {
                format!(
                    "- {} ({}): {}",
                    h.action.intent,
                    if h.success { "OK" } else { "FAILED" },
                    h.action.expected_result
                )
            }).collect::<Vec<_>>().join("\n")
        };

        // Build context section from DB (past actions + known failures)
        let context_section = if db_context.is_empty() {
            String::new()
        } else {
            format!("\n{}", db_context)
        };

        let user_content = format!(
            r#"GOAL: {}

SUCCESS CRITERIA:
{}

CURRENT SCREEN STATE:
- App: {}
- Title: {}
- State: {:?}
- Visible Elements: {}
- Visible Text: {}
- Dialogs: {}

RECENT ACTIONS:
{}
{}
What is the SINGLE next action to achieve the goal?"#,
            goal.objective,
            goal.success_criteria.join("\n- "),
            screen.app,
            screen.title,
            screen.state,
            screen.elements.iter().map(|e| {
                format!("{} '{}' at {} (interactive: {})", e.element_type, e.label, e.position, e.interactive)
            }).collect::<Vec<_>>().join("; "),
            screen.text.join("; "),
            if screen.dialogs.is_empty() {
                "None".to_string()
            } else {
                screen.dialogs.iter().map(|d| {
                    format!("{}: {} [{}]", d.dialog_type, d.message, d.buttons.join(", "))
                }).collect::<Vec<_>>().join("; ")
            },
            history_summary,
            context_section,
        );

        let response = if self.is_anthropic {
            // Anthropic API format - include image if available
            let msg_content = if let Some(img) = base64_image {
                json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/jpeg",
                            "data": img
                        }
                    },
                    {
                        "type": "text",
                        "text": user_content
                    }
                ])
            } else {
                json!(user_content)
            };

            let request = json!({
                "model": self.model,
                "max_tokens": 2000,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": msg_content}
                ]
            });

            let mut req = self.client.post(&self.endpoint).json(&request);
            if let Some(ref key) = self.api_key {
                req = req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
            }
            req.send().await?
        } else {
            // OpenAI-compatible format - include image if available
            let msg_content = if let Some(img) = base64_image {
                json!([
                    {
                        "type": "text",
                        "text": user_content
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/jpeg;base64,{}", img)
                        }
                    }
                ])
            } else {
                json!(user_content)
            };

            let request = json!({
                "model": self.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": msg_content}
                ],
                "temperature": 0.2,
                "max_tokens": 2000
            });

            self.client
                .post(&self.endpoint)
                .json(&request)
                .send()
                .await?
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Planner API error {}: {}", status, body).into());
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

        self.parse_action_response(content)
    }

    /// Parse the planner's response into an action
    fn parse_action_response(
        &self,
        content: &str,
    ) -> Result<Option<PlannedAction>, Box<dyn std::error::Error + Send + Sync>> {
        // Extract JSON from response
        let json_str = if content.starts_with('{') {
            content.to_string()
        } else if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                content[start..=end].to_string()
            } else {
                return Err("No valid JSON found".into());
            }
        } else {
            return Err("No JSON found in response".into());
        };

        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        // Check if done or stuck
        if parsed.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(None);
        }
        if parsed.get("stuck").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(None);
        }

        // Parse action
        let action_type = match parsed["action_type"].as_str().unwrap_or("click") {
            "click" => ActionType::Click,
            "double_click" => ActionType::DoubleClick,
            "right_click" => ActionType::RightClick,
            "type" => ActionType::Type,
            "key_press" => ActionType::KeyPress,
            "scroll" => ActionType::Scroll,
            "wait" => ActionType::Wait,
            "hover" => ActionType::Hover,
            "drag" => ActionType::Drag,
            _ => ActionType::Click,
        };

        let target = if let Some(t) = parsed.get("target") {
            Some(ActionTarget {
                description: t["description"].as_str().unwrap_or("").to_string(),
                x: t["x"].as_i64().unwrap_or(640) as i32,
                y: t["y"].as_i64().unwrap_or(360) as i32,
                bbox: None,
                location_confidence: 0.5,
            })
        } else {
            None
        };

        Ok(Some(PlannedAction {
            intent: parsed["intent"].as_str().unwrap_or("").to_string(),
            action_type,
            target,
            text: parsed["text"].as_str().map(|s| s.to_string()),
            keys: parsed["keys"].as_str().map(|s| s.to_string()),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.5) as f32,
            expected_result: parsed["expected_result"].as_str().unwrap_or("").to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_done_response() {
        let planner = ActionPlanner::new("http://test".into(), "test".into());
        let result = planner.parse_action_response(r#"{"done": true, "reason": "goal achieved"}"#).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_action_response() {
        let planner = ActionPlanner::new("http://test".into(), "test".into());
        let result = planner.parse_action_response(r#"{
            "intent": "Click save button",
            "action_type": "click",
            "target": {"description": "Save button", "x": 500, "y": 300},
            "confidence": 0.9,
            "expected_result": "File saved"
        }"#).unwrap();

        assert!(result.is_some());
        let action = result.unwrap();
        assert_eq!(action.intent, "Click save button");
        assert!(matches!(action.action_type, ActionType::Click));
    }
}
