//! Reactive Computer-Use Agent
//!
//! A vision-in-the-loop agent that maintains situational awareness through
//! continuous screenshot polling and analysis.
//!
//! Key principles:
//! - Never execute blind - always verify with vision
//! - Continuous screenshot polling (configurable rate)
//! - Parallel vision analysis (doesn't block execution)
//! - Adaptive timing - react to what you see, not fixed delays
//! - Dual-model architecture: fast vision + smart planner
//!
//! ## Safety Features
//! - Red frame overlay when agent is in control
//! - Optional input grab (user can still interrupt)
//! - Foolproof interrupt: Both Shift keys + Escape
//! - Vision-based verification of all actions

pub mod control;
pub mod reactive_vision;
pub mod knowledge;

pub use control::{AgentControl, ControlError};
pub use reactive_vision::{ReactiveVision, ElementLocation, ScreenAnalysis, ActionVerification};
pub use knowledge::{UIKnowledgeBase, AppKnowledge, LaunchMethod, CloseMethod};

use crate::input::{InputController, MouseButton};
use crate::vision::VisionController;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

/// Screen state from vision analysis
#[derive(Debug, Clone)]
pub struct ScreenState {
    pub timestamp: Instant,
    pub width: u32,
    pub height: u32,
    pub description: String,
    pub detected_elements: Vec<DetectedElement>,
    pub raw_image: String, // base64
}

/// A UI element detected on screen
#[derive(Debug, Clone)]
pub struct DetectedElement {
    pub element_type: ElementType,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementType {
    Button,
    TextInput,
    Window,
    Icon,
    Menu,
    Dialog,
    Text,
    Unknown,
}

/// Action the agent can take
#[derive(Debug, Clone)]
pub enum AgentAction {
    Click { x: i32, y: i32 },
    DoubleClick { x: i32, y: i32 },
    RightClick { x: i32, y: i32 },
    Type { text: String },
    KeyPress { key: String },
    KeyCombo { combo: String },
    Scroll { dx: i32, dy: i32 },
    Wait { condition: WaitCondition },
    MoveMouse { x: i32, y: i32 },
}

/// Conditions to wait for
#[derive(Debug, Clone)]
pub enum WaitCondition {
    /// Wait until we see specific text on screen
    TextVisible(String),
    /// Wait until a window with this title appears
    WindowVisible(String),
    /// Wait until screen changes significantly
    ScreenChanged,
    /// Wait until screen stabilizes (stops changing)
    ScreenStable { duration_ms: u64 },
    /// Maximum wait time (fallback)
    MaxTime { ms: u64 },
}

/// Configuration for the reactive agent
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// How often to capture screenshots (ms)
    pub screenshot_interval_ms: u64,
    /// Vision model endpoint
    pub vision_endpoint: String,
    /// Vision model name
    pub vision_model: String,
    /// Planner model endpoint
    pub planner_endpoint: String,
    /// Planner model name
    pub planner_model: String,
    /// Maximum actions per task
    pub max_actions: u32,
    /// Timeout for waiting conditions (ms)
    pub wait_timeout_ms: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            screenshot_interval_ms: 300,  // ~3 FPS
            vision_endpoint: "http://localhost:1234/v1/chat/completions".into(),
            vision_model: "vision-model".into(),
            planner_endpoint: "http://localhost:1234/v1/chat/completions".into(),
            planner_model: "planner-model".into(),
            max_actions: 100,
            wait_timeout_ms: 30000,
        }
    }
}

/// The reactive computer-use agent
pub struct ReactiveAgent {
    config: AgentConfig,
    vision: Arc<VisionController>,
    input: Arc<InputController>,

    /// Latest screen state (updated by polling task)
    current_state: Arc<RwLock<Option<ScreenState>>>,

    /// Running flag for polling task
    running: Arc<AtomicBool>,

    /// Screenshot counter
    screenshot_count: Arc<AtomicU64>,

    /// Channel to receive screen updates
    state_rx: Option<mpsc::Receiver<ScreenState>>,
}

impl ReactiveAgent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            vision: Arc::new(VisionController::new()),
            input: Arc::new(InputController::new()),
            current_state: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            screenshot_count: Arc::new(AtomicU64::new(0)),
            state_rx: None,
        }
    }

    /// Start the agent (enables vision, input, starts polling)
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.vision.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        self.input.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        self.running.store(true, Ordering::SeqCst);

        // Create channel for screen state updates
        let (tx, rx) = mpsc::channel(10);
        self.state_rx = Some(rx);

        // Start the screenshot polling task
        let vision = Arc::clone(&self.vision);
        let current_state = Arc::clone(&self.current_state);
        let running = Arc::clone(&self.running);
        let screenshot_count = Arc::clone(&self.screenshot_count);
        let interval = self.config.screenshot_interval_ms;
        let vision_endpoint = self.config.vision_endpoint.clone();
        let vision_model = self.config.vision_model.clone();

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap();

            while running.load(Ordering::SeqCst) {
                let start = Instant::now();

                // Capture screenshot
                if let Ok(screenshot) = vision.capture_screen() {
                    screenshot_count.fetch_add(1, Ordering::SeqCst);

                    // Quick vision analysis (async, non-blocking)
                    let description = quick_analyze(
                        &client,
                        &vision_endpoint,
                        &vision_model,
                        &screenshot.data
                    ).await.unwrap_or_else(|_| "Analysis unavailable".into());

                    let state = ScreenState {
                        timestamp: Instant::now(),
                        width: screenshot.width,
                        height: screenshot.height,
                        description,
                        detected_elements: vec![], // TODO: element detection
                        raw_image: screenshot.data,
                    };

                    // Update shared state
                    *current_state.write().await = Some(state.clone());

                    // Send to channel (non-blocking)
                    let _ = tx.try_send(state);
                }

                // Maintain target interval
                let elapsed = start.elapsed();
                if elapsed < Duration::from_millis(interval) {
                    tokio::time::sleep(Duration::from_millis(interval) - elapsed).await;
                }
            }
        });

        Ok(())
    }

    /// Stop the agent
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.vision.disable();
        self.input.disable();
    }

    /// Get current screen state
    pub async fn get_state(&self) -> Option<ScreenState> {
        self.current_state.read().await.clone()
    }

    /// Get screenshot count
    pub fn screenshot_count(&self) -> u64 {
        self.screenshot_count.load(Ordering::SeqCst)
    }

    /// Execute an action
    pub async fn execute(&self, action: AgentAction) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match action {
            AgentAction::Click { x, y } => {
                self.input.mouse_move(x, y)?;
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.input.mouse_click(MouseButton::Left)?;
            }
            AgentAction::DoubleClick { x, y } => {
                self.input.mouse_move(x, y)?;
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.input.mouse_double_click(MouseButton::Left)?;
            }
            AgentAction::RightClick { x, y } => {
                self.input.mouse_move(x, y)?;
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.input.mouse_click(MouseButton::Right)?;
            }
            AgentAction::Type { text } => {
                self.input.type_text(&text)?;
            }
            AgentAction::KeyPress { key } => {
                self.input.key_press(&key)?;
            }
            AgentAction::KeyCombo { combo } => {
                self.input.key_combination(&combo)?;
            }
            AgentAction::Scroll { dx, dy } => {
                self.input.scroll(dx, dy)?;
            }
            AgentAction::MoveMouse { x, y } => {
                self.input.mouse_move(x, y)?;
            }
            AgentAction::Wait { condition } => {
                self.wait_for(condition).await?;
            }
        }
        Ok(())
    }

    /// Wait for a condition to be met
    pub async fn wait_for(&self, condition: WaitCondition) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timeout = Duration::from_millis(self.config.wait_timeout_ms);
        let start = Instant::now();
        let poll_interval = Duration::from_millis(100);

        match condition {
            WaitCondition::TextVisible(text) => {
                while start.elapsed() < timeout {
                    if let Some(state) = self.get_state().await {
                        if state.description.to_lowercase().contains(&text.to_lowercase()) {
                            return Ok(());
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            WaitCondition::WindowVisible(title) => {
                while start.elapsed() < timeout {
                    if let Some(state) = self.get_state().await {
                        if state.description.to_lowercase().contains(&title.to_lowercase()) {
                            return Ok(());
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            WaitCondition::ScreenChanged => {
                let initial = self.get_state().await;
                while start.elapsed() < timeout {
                    if let (Some(initial_state), Some(current)) = (&initial, self.get_state().await) {
                        // Simple change detection: compare image sizes or use hash
                        if current.raw_image.len() != initial_state.raw_image.len() {
                            return Ok(());
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            WaitCondition::ScreenStable { duration_ms } => {
                let stable_duration = Duration::from_millis(duration_ms);
                let mut last_change = Instant::now();
                let mut last_size = 0usize;

                while start.elapsed() < timeout {
                    if let Some(state) = self.get_state().await {
                        if state.raw_image.len() != last_size {
                            last_size = state.raw_image.len();
                            last_change = Instant::now();
                        } else if last_change.elapsed() >= stable_duration {
                            return Ok(());
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            WaitCondition::MaxTime { ms } => {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                return Ok(());
            }
        }

        // Timeout reached
        Ok(())
    }

    /// Execute a task with full situational awareness
    pub async fn run_task(&mut self, task: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("[Agent] Starting task: {}", task);
        println!("[Agent] Screenshot polling active ({} ms interval)", self.config.screenshot_interval_ms);

        // Get initial state
        tokio::time::sleep(Duration::from_millis(500)).await; // Let polling catch up

        if let Some(state) = self.get_state().await {
            println!("[Agent] Initial state: {}", state.description);
        }

        // TODO: Use planner model to generate action sequence
        // TODO: Execute actions with vision feedback
        // For now, this is a framework for the reactive approach

        Ok(())
    }
}

/// Quick vision analysis - optimized for speed
async fn quick_analyze(
    client: &reqwest::Client,
    endpoint: &str,
    model: &str,
    base64_image: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let request = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "Quick: What app is open? Any dialogs? Just key details, 20 words max."
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", base64_image)
                    }
                }
            ]
        }],
        "max_tokens": 50,
        "temperature": 0.1
    });

    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.screenshot_interval_ms, 300);
    }
}
