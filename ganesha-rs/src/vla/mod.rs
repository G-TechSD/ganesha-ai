//! VLA (Vision-Language-Action) Module
//!
//! Closed-loop GUI automation: perceive → plan → act → verify → repeat
//!
//! This is Ganesha's computer-use capability - the ability to control
//! GUI applications through visual understanding and input simulation.

pub mod loop_controller;
pub mod action_planner;
pub mod element_locator;

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub use loop_controller::VlaLoop;
pub use action_planner::ActionPlanner;
pub use element_locator::ElementLocator;

/// A planned action to execute on the GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    /// What we're trying to do
    pub intent: String,
    /// Type of action
    pub action_type: ActionType,
    /// Target element (if applicable)
    pub target: Option<ActionTarget>,
    /// Text to type (if typing action)
    pub text: Option<String>,
    /// Key combination (if key action)
    pub keys: Option<String>,
    /// Confidence in this action (0.0-1.0)
    pub confidence: f32,
    /// Expected result description
    pub expected_result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Click,
    DoubleClick,
    RightClick,
    Type,
    KeyPress,
    Scroll,
    Wait,
    /// Move mouse without clicking
    Hover,
    /// Drag from one point to another
    Drag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTarget {
    /// Textual description of the element
    pub description: String,
    /// Estimated x coordinate
    pub x: i32,
    /// Estimated y coordinate  
    pub y: i32,
    /// Bounding box if available (x, y, width, height)
    pub bbox: Option<(i32, i32, i32, i32)>,
    /// Confidence in location
    pub location_confidence: f32,
}

/// Result of executing an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: PlannedAction,
    pub success: bool,
    pub error: Option<String>,
    /// Screen state after action
    pub screen_state: Option<String>,
    /// Whether the expected result was achieved
    pub expected_achieved: bool,
    pub duration_ms: u64,
}

/// Goal for the VLA loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlaGoal {
    /// What we're trying to accomplish
    pub objective: String,
    /// Success criteria
    pub success_criteria: Vec<String>,
    /// Maximum actions to try
    pub max_actions: usize,
    /// Timeout for the entire goal
    pub timeout: Duration,
    /// App/window to focus on (optional)
    pub target_app: Option<String>,
}

/// Status of the VLA loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlaStatus {
    pub goal: VlaGoal,
    pub actions_taken: usize,
    pub current_state: String,
    pub success: bool,
    pub completed: bool,
    pub error: Option<String>,
    pub action_history: Vec<ActionResult>,
}

/// VLA configuration
#[derive(Debug, Clone)]
pub struct VlaConfig {
    /// Vision model endpoint
    pub vision_endpoint: String,
    /// Vision model name
    pub vision_model: String,
    /// Planning model endpoint (can be different from vision)
    pub planner_endpoint: String,
    /// Planning model name
    pub planner_model: String,
    /// Minimum delay between actions (safety)
    pub action_delay: Duration,
    /// Screenshot resolution for analysis
    pub capture_width: u32,
    pub capture_height: u32,
    /// Maximum retries per action
    pub max_retries: usize,
    /// Whether to save screenshots for debugging
    pub save_screenshots: bool,
    /// Directory for screenshots
    pub screenshot_dir: String,
}

impl Default for VlaConfig {
    fn default() -> Self {
        Self {
            vision_endpoint: "http://192.168.245.155:1234/v1/chat/completions".into(),
            vision_model: "qwen/qwen2.5-vl-7b".into(),
            planner_endpoint: "http://192.168.245.155:1234/v1/chat/completions".into(),
            planner_model: "gpt-oss-20b".into(),
            action_delay: Duration::from_millis(200),
            capture_width: 640,
            capture_height: 360,
            max_retries: 3,
            save_screenshots: false,
            screenshot_dir: "/tmp/ganesha-vla".into(),
        }
    }
}
