//! Vision Controller - The main orchestrator for the vision-action loop.
//!
//! This module provides the `VisionController` which is the central coordinator
//! that ties together screen capture, vision analysis, learning, and action execution.
//!
//! # Architecture
//!
//! The controller runs a continuous loop:
//! 1. Capture screen
//! 2. Analyze with vision model
//! 3. Check for relevant learned skills
//! 4. Plan next action
//! 5. Safety check
//! 6. Execute action
//! 7. Verify result
//! 8. Loop or complete
//!
//! # Example
//!
//! ```rust,no_run
//! use ganesha_vision::controller::{VisionController, VisionControllerConfig, SpeedMode};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = VisionControllerConfig::default();
//!     let controller = VisionController::new(config, "ganesha_vision.db").await?;
//!
//!     // Execute a task
//!     controller.execute_task("Open the settings menu").await?;
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::capture::{CaptureConfig, DefaultCapture, ScreenCapture, create_capture};
use crate::db::{Action, ActionDetails, ActionType, Database, Demonstration, MouseButton, Modifier};
use crate::error::{Error, Result};
use crate::learning::{LearningEngine, Screenshot as LearningScreenshot, SkillMatch};
use crate::model::{
    DualModelConfig, ScreenAnalysis, Screenshot as ModelScreenshot, VisionClient, VisionModelConfig,
};

// ============================================================================
// Configuration Types
// ============================================================================

/// Speed mode for action execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeedMode {
    /// Slow and careful - good for learning and debugging
    Careful,
    /// Normal speed - balanced between safety and speed
    #[default]
    Normal,
    /// Fast mode - for experienced users and well-tested skills
    Fast,
    /// As fast as possible - minimal verification
    Turbo,
}

impl SpeedMode {
    /// Get the base delay between actions in milliseconds.
    pub fn base_delay_ms(&self) -> u64 {
        match self {
            SpeedMode::Careful => 2000,
            SpeedMode::Normal => 1000,
            SpeedMode::Fast => 500,
            SpeedMode::Turbo => 100,
        }
    }

    /// Get the verification delay in milliseconds.
    pub fn verification_delay_ms(&self) -> u64 {
        match self {
            SpeedMode::Careful => 500,
            SpeedMode::Normal => 250,
            SpeedMode::Fast => 100,
            SpeedMode::Turbo => 50,
        }
    }

    /// Whether to perform full verification after each action.
    pub fn full_verification(&self) -> bool {
        matches!(self, SpeedMode::Careful | SpeedMode::Normal)
    }
}

/// Model configuration - either single model or dual model setup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelConfig {
    /// Single model for both planning and vision
    Single(VisionModelConfig),
    /// Dual model setup (planning + vision)
    Dual(DualModelConfig),
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig::Single(VisionModelConfig::default())
    }
}

/// Configuration for the VisionController.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionControllerConfig {
    /// Model configuration (single or dual)
    pub model_config: ModelConfig,

    /// Screen capture configuration
    pub capture_config: CaptureConfig,

    /// Speed mode for action execution
    pub speed_mode: SpeedMode,

    /// Maximum actions per second (rate limiting)
    pub max_actions_per_second: f32,

    /// Action types that require user confirmation before execution
    pub require_confirmation_for: Vec<ActionType>,

    /// Whether the learning system is enabled
    pub learning_enabled: bool,

    /// Whether the overlay system is enabled (for visual feedback)
    pub overlay_enabled: bool,

    /// Maximum number of actions before giving up on a goal
    pub max_actions_per_goal: u32,

    /// Timeout for a single goal in seconds
    pub goal_timeout_secs: u64,

    /// Minimum confidence threshold for action execution
    pub min_action_confidence: f32,

    /// Whether to automatically retry failed actions
    pub auto_retry: bool,

    /// Maximum number of retries for a failed action
    pub max_retries: u32,
}

impl Default for VisionControllerConfig {
    fn default() -> Self {
        Self {
            model_config: ModelConfig::default(),
            capture_config: CaptureConfig::default(),
            speed_mode: SpeedMode::Normal,
            max_actions_per_second: 1.0,
            require_confirmation_for: vec![
                ActionType::KeyboardShortcut,
                ActionType::TextInput,
            ],
            learning_enabled: true,
            overlay_enabled: true,
            max_actions_per_goal: 100,
            goal_timeout_secs: 300, // 5 minutes
            min_action_confidence: 0.5,
            auto_retry: true,
            max_retries: 3,
        }
    }
}

impl VisionControllerConfig {
    /// Create a config optimized for careful/learning mode.
    pub fn careful() -> Self {
        Self {
            speed_mode: SpeedMode::Careful,
            max_actions_per_second: 0.5,
            learning_enabled: true,
            overlay_enabled: true,
            min_action_confidence: 0.7,
            ..Default::default()
        }
    }

    /// Create a config optimized for fast execution.
    pub fn fast() -> Self {
        Self {
            speed_mode: SpeedMode::Fast,
            max_actions_per_second: 2.0,
            require_confirmation_for: vec![],
            min_action_confidence: 0.3,
            ..Default::default()
        }
    }

    /// Set the model configuration.
    pub fn with_model(mut self, config: ModelConfig) -> Self {
        self.model_config = config;
        self
    }

    /// Set the speed mode.
    pub fn with_speed_mode(mut self, mode: SpeedMode) -> Self {
        self.speed_mode = mode;
        self
    }

    /// Set the capture configuration.
    pub fn with_capture_config(mut self, config: CaptureConfig) -> Self {
        self.capture_config = config;
        self
    }

    /// Enable or disable learning.
    pub fn with_learning(mut self, enabled: bool) -> Self {
        self.learning_enabled = enabled;
        self
    }

    /// Add action types that require confirmation.
    pub fn with_confirmation_for(mut self, action_types: Vec<ActionType>) -> Self {
        self.require_confirmation_for = action_types;
        self
    }
}

// ============================================================================
// Controller State
// ============================================================================

/// Current state of the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerState {
    /// Controller is idle, not executing any task
    Idle,
    /// Controller is running and executing a task
    Running,
    /// Controller is paused
    Paused,
    /// Controller is recording a demonstration
    Recording,
    /// Controller encountered an error
    Error,
    /// Controller is shutting down
    Stopping,
}

impl Default for ControllerState {
    fn default() -> Self {
        ControllerState::Idle
    }
}

/// Status information about the controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerStatus {
    /// Current state
    pub state: ControllerState,

    /// Current goal being executed
    pub current_goal: Option<String>,

    /// Current action being performed
    pub current_action: Option<String>,

    /// Number of actions taken for the current goal
    pub actions_taken: u32,

    /// Recent errors (last 10)
    pub errors: Vec<String>,

    /// Whether recording is active
    pub is_recording: bool,

    /// Name of the current recording if any
    pub recording_name: Option<String>,

    /// Timestamp when the current task started
    pub task_started_at: Option<i64>,

    /// Last action timestamp
    pub last_action_at: Option<i64>,

    /// Matched skills for the current context
    pub matched_skills: Vec<String>,
}

impl Default for ControllerStatus {
    fn default() -> Self {
        Self {
            state: ControllerState::Idle,
            current_goal: None,
            current_action: None,
            actions_taken: 0,
            errors: Vec::new(),
            is_recording: false,
            recording_name: None,
            task_started_at: None,
            last_action_at: None,
            matched_skills: Vec::new(),
        }
    }
}

// ============================================================================
// Safety System
// ============================================================================

/// Safety checker for actions before execution.
pub struct SafetyChecker {
    /// Action types that are always blocked
    blocked_actions: HashSet<ActionType>,

    /// Action types that require confirmation
    confirmation_required: HashSet<ActionType>,

    /// Dangerous keyboard shortcuts that require confirmation
    dangerous_shortcuts: Vec<Vec<String>>,

    /// Maximum allowed mouse movement per action
    max_mouse_movement: Option<u32>,
}

impl SafetyChecker {
    /// Create a new safety checker with the given confirmation requirements.
    pub fn new(confirmation_required: Vec<ActionType>) -> Self {
        Self {
            blocked_actions: HashSet::new(),
            confirmation_required: confirmation_required.into_iter().collect(),
            dangerous_shortcuts: vec![
                vec!["ctrl".to_string(), "alt".to_string(), "delete".to_string()],
                vec!["alt".to_string(), "f4".to_string()],
                vec!["ctrl".to_string(), "w".to_string()],
                vec!["ctrl".to_string(), "q".to_string()],
            ],
            max_mouse_movement: None,
        }
    }

    /// Check if an action is allowed to execute.
    pub fn is_allowed(&self, action: &Action) -> SafetyResult {
        // Check if action type is blocked
        if self.blocked_actions.contains(&action.action_type) {
            return SafetyResult::Blocked("Action type is blocked".to_string());
        }

        // Check dangerous shortcuts
        if let ActionDetails::KeyboardShortcut { ref keys, ref modifiers } = action.details {
            let all_keys: Vec<String> = modifiers
                .iter()
                .map(|m| format!("{:?}", m).to_lowercase())
                .chain(keys.iter().map(|k| k.to_lowercase()))
                .collect();

            for dangerous in &self.dangerous_shortcuts {
                if dangerous.iter().all(|k| all_keys.contains(k)) {
                    return SafetyResult::RequiresConfirmation(
                        format!("Dangerous shortcut detected: {:?}", keys)
                    );
                }
            }
        }

        // Check if confirmation is required for this action type
        if self.confirmation_required.contains(&action.action_type) {
            return SafetyResult::RequiresConfirmation(
                format!("Action type {:?} requires confirmation", action.action_type)
            );
        }

        SafetyResult::Allowed
    }

    /// Add an action type to the blocked list.
    pub fn block_action_type(&mut self, action_type: ActionType) {
        self.blocked_actions.insert(action_type);
    }

    /// Add an action type to the confirmation required list.
    pub fn require_confirmation(&mut self, action_type: ActionType) {
        self.confirmation_required.insert(action_type);
    }

    /// Set maximum allowed mouse movement.
    pub fn set_max_mouse_movement(&mut self, max: u32) {
        self.max_mouse_movement = Some(max);
    }
}

/// Result of a safety check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyResult {
    /// Action is allowed
    Allowed,
    /// Action requires user confirmation
    RequiresConfirmation(String),
    /// Action is blocked
    Blocked(String),
}

impl SafetyResult {
    /// Check if the action is allowed (without confirmation).
    pub fn is_allowed(&self) -> bool {
        matches!(self, SafetyResult::Allowed)
    }

    /// Check if the action requires confirmation.
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, SafetyResult::RequiresConfirmation(_))
    }

    /// Check if the action is blocked.
    pub fn is_blocked(&self) -> bool {
        matches!(self, SafetyResult::Blocked(_))
    }
}

// ============================================================================
// Action Planner
// ============================================================================

/// Plans the next action based on the current goal and screen analysis.
pub struct ActionPlanner {
    /// Minimum confidence for planning
    min_confidence: f32,
}

impl ActionPlanner {
    /// Create a new action planner.
    pub fn new(min_confidence: f32) -> Self {
        Self { min_confidence }
    }

    /// Plan the next action based on goal, analysis, and available skills.
    pub async fn plan_next_action(
        &self,
        vision_client: &VisionClient,
        goal: &str,
        screenshot: &ModelScreenshot,
        analysis: &ScreenAnalysis,
        skills: &[SkillMatch],
    ) -> Result<Option<PlannedAction>> {
        // First, check if we have a high-confidence skill match
        if let Some(skill_match) = skills.first() {
            if skill_match.confidence >= self.min_confidence {
                debug!(
                    "Using skill '{}' with confidence {:.2}",
                    skill_match.skill.name, skill_match.confidence
                );

                // Get the first action from the skill
                if let Some(template) = skill_match.skill.action_template.first() {
                    let action = Action::new(
                        template.action_type,
                        template.details_template.clone(),
                        format!("From skill: {}", skill_match.skill.name),
                    );

                    return Ok(Some(PlannedAction {
                        action,
                        confidence: skill_match.confidence,
                        reasoning: format!(
                            "Applying learned skill '{}': {}",
                            skill_match.skill.name, skill_match.reason
                        ),
                        from_skill: Some(skill_match.skill.id.clone()),
                    }));
                }
            }
        }

        // Otherwise, use the vision model to plan
        let steps = vision_client.plan_actions(screenshot, goal).await
            .map_err(|e| Error::invalid(format!("Vision planning failed: {}", e)))?;

        if steps.is_empty() {
            return Ok(None);
        }

        // Parse the first step into an action
        let first_step = &steps[0];
        let action = self.parse_step_to_action(first_step, analysis)?;

        Ok(Some(PlannedAction {
            action,
            confidence: analysis.confidence,
            reasoning: first_step.clone(),
            from_skill: None,
        }))
    }

    /// Check if the goal has been achieved based on the screen analysis.
    pub async fn is_goal_achieved(
        &self,
        vision_client: &VisionClient,
        goal: &str,
        screenshot: &ModelScreenshot,
        _analysis: &ScreenAnalysis,
    ) -> Result<bool> {
        // Ask the vision model to verify
        let question = format!(
            "Has the following goal been achieved? Goal: '{}'. Answer only 'yes' or 'no'.",
            goal
        );

        let answer = vision_client.ask_about_screen(screenshot, &question).await
            .map_err(|e| Error::invalid(format!("Vision query failed: {}", e)))?;

        let answer_lower = answer.to_lowercase();
        Ok(answer_lower.contains("yes") || answer_lower.contains("achieved") || answer_lower.contains("completed"))
    }

    /// Parse a step description into an action.
    fn parse_step_to_action(&self, step: &str, analysis: &ScreenAnalysis) -> Result<Action> {
        let step_lower = step.to_lowercase();

        // Try to parse click actions
        if step_lower.contains("click") {
            // Try to find the target element
            for element in &analysis.ui_elements {
                if step_lower.contains(&element.label.to_lowercase()) {
                    if let Some((x, y, w, h)) = element.bounds {
                        let center_x = x + (w as i32 / 2);
                        let center_y = y + (h as i32 / 2);

                        return Ok(Action::new(
                            ActionType::MouseClick,
                            ActionDetails::MouseClick {
                                x: center_x,
                                y: center_y,
                                button: MouseButton::Left,
                                modifiers: vec![],
                            },
                            step.to_string(),
                        ));
                    }
                }
            }

            // Default click at center if no element found
            return Ok(Action::new(
                ActionType::MouseClick,
                ActionDetails::MouseClick {
                    x: 960,
                    y: 540,
                    button: MouseButton::Left,
                    modifiers: vec![],
                },
                step.to_string(),
            ));
        }

        // Try to parse type/input actions
        if step_lower.contains("type") || step_lower.contains("enter") || step_lower.contains("input") {
            // Extract text to type (simplified - look for quoted text)
            let text = if let Some(start) = step.find('"') {
                if let Some(end) = step[start + 1..].find('"') {
                    step[start + 1..start + 1 + end].to_string()
                } else {
                    "text".to_string()
                }
            } else {
                "text".to_string()
            };

            return Ok(Action::new(
                ActionType::TextInput,
                ActionDetails::TextInput { text },
                step.to_string(),
            ));
        }

        // Try to parse keyboard shortcut actions
        if step_lower.contains("press") || step_lower.contains("shortcut") {
            let mut keys = Vec::new();
            let mut modifiers = Vec::new();

            if step_lower.contains("ctrl") || step_lower.contains("control") {
                modifiers.push(Modifier::Ctrl);
            }
            if step_lower.contains("alt") {
                modifiers.push(Modifier::Alt);
            }
            if step_lower.contains("shift") {
                modifiers.push(Modifier::Shift);
            }

            // Extract key names
            for word in step.split_whitespace() {
                let word_lower = word.to_lowercase();
                if word_lower.len() == 1 && word_lower.chars().next().unwrap().is_alphabetic() {
                    keys.push(word.to_string());
                } else if ["enter", "escape", "tab", "space", "backspace", "delete"]
                    .contains(&word_lower.as_str())
                {
                    keys.push(word.to_string());
                }
            }

            if !keys.is_empty() || !modifiers.is_empty() {
                return Ok(Action::new(
                    ActionType::KeyboardShortcut,
                    ActionDetails::KeyboardShortcut { keys, modifiers },
                    step.to_string(),
                ));
            }
        }

        // Default to a wait action if we can't parse
        Ok(Action::new(
            ActionType::Wait,
            ActionDetails::Wait { duration_ms: 1000 },
            format!("Could not parse action: {}", step),
        ))
    }
}

/// A planned action with metadata.
#[derive(Debug, Clone)]
pub struct PlannedAction {
    /// The action to execute
    pub action: Action,
    /// Confidence in this action (0.0 - 1.0)
    pub confidence: f32,
    /// Reasoning for this action
    pub reasoning: String,
    /// ID of the skill this action came from (if any)
    pub from_skill: Option<String>,
}

// ============================================================================
// Input Executor (stub - would integrate with actual input system)
// ============================================================================

/// Executes actions by simulating input.
pub struct InputExecutor {
    /// Delay between actions
    action_delay: Duration,
}

impl InputExecutor {
    /// Create a new input executor.
    pub fn new(action_delay: Duration) -> Self {
        Self { action_delay }
    }

    /// Execute an action.
    pub async fn execute(&self, action: &Action) -> Result<()> {
        // In a real implementation, this would use platform-specific input simulation
        // For now, we just log and wait

        info!("Executing action: {:?} - {}", action.action_type, action.description);

        // Wait for the action's delay
        if action.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(action.delay_ms)).await;
        }

        // Simulate the action based on type
        match &action.details {
            ActionDetails::MouseClick { x, y, button, .. } => {
                debug!("Mouse click at ({}, {}) with {:?} button", x, y, button);
                // Would call platform input API here
            }
            ActionDetails::MouseDoubleClick { x, y, button } => {
                debug!("Mouse double-click at ({}, {}) with {:?} button", x, y, button);
            }
            ActionDetails::MouseDrag { start_x, start_y, end_x, end_y, .. } => {
                debug!("Mouse drag from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y);
            }
            ActionDetails::MouseScroll { x, y, delta_x, delta_y } => {
                debug!("Mouse scroll at ({}, {}) delta ({}, {})", x, y, delta_x, delta_y);
            }
            ActionDetails::MouseMove { x, y } => {
                debug!("Mouse move to ({}, {})", x, y);
            }
            ActionDetails::KeyPress { key, modifiers } => {
                debug!("Key press: {} with modifiers {:?}", key, modifiers);
            }
            ActionDetails::KeyRelease { key } => {
                debug!("Key release: {}", key);
            }
            ActionDetails::TextInput { text } => {
                debug!("Text input: {}", text);
            }
            ActionDetails::KeyboardShortcut { keys, modifiers } => {
                debug!("Keyboard shortcut: {:?} with modifiers {:?}", keys, modifiers);
            }
            ActionDetails::Wait { duration_ms } => {
                debug!("Waiting for {} ms", duration_ms);
                tokio::time::sleep(Duration::from_millis(*duration_ms)).await;
            }
            ActionDetails::Screenshot { reason } => {
                debug!("Screenshot marker: {}", reason);
            }
        }

        // Wait after the action
        tokio::time::sleep(self.action_delay).await;

        Ok(())
    }
}

// ============================================================================
// Confirmation Handler (stub - would integrate with overlay)
// ============================================================================

/// Handles user confirmation requests.
pub struct ConfirmationHandler {
    /// Channel for sending confirmation requests
    request_tx: mpsc::Sender<ConfirmationRequest>,
    /// Channel for receiving responses
    response_rx: Mutex<mpsc::Receiver<bool>>,
}

/// A confirmation request.
#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    /// Action being confirmed
    pub action: Action,
    /// Reason for confirmation
    pub reason: String,
}

impl ConfirmationHandler {
    /// Create a new confirmation handler.
    pub fn new() -> (Self, mpsc::Receiver<ConfirmationRequest>, mpsc::Sender<bool>) {
        let (request_tx, request_rx) = mpsc::channel(10);
        let (response_tx, response_rx) = mpsc::channel(10);

        (
            Self {
                request_tx,
                response_rx: Mutex::new(response_rx),
            },
            request_rx,
            response_tx,
        )
    }

    /// Request confirmation for an action.
    pub async fn request_confirmation(&self, action: &Action, reason: &str) -> Result<bool> {
        // Send the request
        self.request_tx
            .send(ConfirmationRequest {
                action: action.clone(),
                reason: reason.to_string(),
            })
            .await
            .map_err(|_| Error::invalid("Confirmation channel closed"))?;

        // Wait for response
        let mut rx = self.response_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| Error::invalid("Confirmation response channel closed"))
    }
}

// ============================================================================
// Vision Controller
// ============================================================================

/// The main vision controller that orchestrates the vision-action loop.
pub struct VisionController {
    /// Configuration
    config: VisionControllerConfig,

    /// Vision model client
    vision_client: Arc<VisionClient>,

    /// Screen capture
    capture: Arc<DefaultCapture>,

    /// Learning engine
    learning: Arc<LearningEngine>,

    /// Action planner
    planner: ActionPlanner,

    /// Safety checker
    safety: Arc<RwLock<SafetyChecker>>,

    /// Input executor
    executor: InputExecutor,

    /// Confirmation handler
    confirmation: Option<Arc<ConfirmationHandler>>,

    /// Current status
    status: Arc<RwLock<ControllerStatus>>,

    /// State watch channel (for external monitoring)
    state_tx: watch::Sender<ControllerState>,
    state_rx: watch::Receiver<ControllerState>,

    /// Stop signal
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,

    /// Pause signal
    pause_tx: watch::Sender<bool>,
    pause_rx: watch::Receiver<bool>,

    /// Current recording session name
    recording_name: Arc<Mutex<Option<String>>>,
}

impl VisionController {
    /// Create a new vision controller.
    pub async fn new(config: VisionControllerConfig, db_path: impl AsRef<Path>) -> Result<Self> {
        // Create vision client
        let vision_client = match &config.model_config {
            ModelConfig::Single(c) => VisionClient::new(c.clone())
                .map_err(|e| Error::invalid(format!("Failed to create vision client: {}", e)))?,
            ModelConfig::Dual(c) => VisionClient::with_dual_models(c.clone())
                .map_err(|e| Error::invalid(format!("Failed to create dual vision client: {}", e)))?,
        };

        // Create screen capture
        let capture = create_capture(config.capture_config.clone());

        // Create learning engine
        let db = Database::open(db_path)?;
        let learning = LearningEngine::new(db);

        // Create planner
        let planner = ActionPlanner::new(config.min_action_confidence);

        // Create safety checker
        let safety = SafetyChecker::new(config.require_confirmation_for.clone());

        // Create executor with delay based on speed mode
        let action_delay = Duration::from_millis(config.speed_mode.base_delay_ms());
        let executor = InputExecutor::new(action_delay);

        // Create channels
        let (state_tx, state_rx) = watch::channel(ControllerState::Idle);
        let (stop_tx, stop_rx) = watch::channel(false);
        let (pause_tx, pause_rx) = watch::channel(false);

        Ok(Self {
            config,
            vision_client: Arc::new(vision_client),
            capture: Arc::new(capture),
            learning: Arc::new(learning),
            planner,
            safety: Arc::new(RwLock::new(safety)),
            executor,
            confirmation: None,
            status: Arc::new(RwLock::new(ControllerStatus::default())),
            state_tx,
            state_rx,
            stop_tx,
            stop_rx,
            pause_tx,
            pause_rx,
            recording_name: Arc::new(Mutex::new(None)),
        })
    }

    /// Set up confirmation handling.
    pub fn with_confirmation(
        mut self,
    ) -> (Self, mpsc::Receiver<ConfirmationRequest>, mpsc::Sender<bool>) {
        let (handler, request_rx, response_tx) = ConfirmationHandler::new();
        self.confirmation = Some(Arc::new(handler));
        (self, request_rx, response_tx)
    }

    /// Get the current configuration.
    pub fn config(&self) -> &VisionControllerConfig {
        &self.config
    }

    /// Get the current status.
    pub async fn get_status(&self) -> ControllerStatus {
        self.status.read().await.clone()
    }

    /// Check if the controller is running.
    pub fn is_running(&self) -> bool {
        *self.state_rx.borrow() == ControllerState::Running
    }

    /// Check if the controller is paused.
    pub fn is_paused(&self) -> bool {
        *self.pause_rx.borrow()
    }

    /// Get a watch receiver for state changes.
    pub fn state_watch(&self) -> watch::Receiver<ControllerState> {
        self.state_rx.clone()
    }

    /// Get access to the learning engine.
    pub fn learning_engine(&self) -> &LearningEngine {
        &self.learning
    }

    // ========================================================================
    // Control Methods
    // ========================================================================

    /// Start the control loop (non-blocking, runs in background).
    pub async fn start(&self) -> Result<()> {
        // Check if already running
        if self.is_running() {
            return Err(Error::invalid("Controller is already running"));
        }

        let _ = self.state_tx.send(ControllerState::Running);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Running;
        }

        info!("Vision controller started");
        Ok(())
    }

    /// Stop the control loop.
    pub async fn stop(&self) -> Result<()> {
        let _ = self.stop_tx.send(true);
        let _ = self.state_tx.send(ControllerState::Stopping);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Stopping;
        }

        info!("Vision controller stopping");
        Ok(())
    }

    /// Pause the control loop.
    pub async fn pause(&self) -> Result<()> {
        let _ = self.pause_tx.send(true);
        let _ = self.state_tx.send(ControllerState::Paused);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Paused;
        }

        info!("Vision controller paused");
        Ok(())
    }

    /// Resume the control loop.
    pub async fn resume(&self) -> Result<()> {
        let _ = self.pause_tx.send(false);
        let _ = self.state_tx.send(ControllerState::Running);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Running;
        }

        info!("Vision controller resumed");
        Ok(())
    }

    // ========================================================================
    // Task Execution
    // ========================================================================

    /// Execute a task with the given goal.
    pub async fn execute_task(&self, goal: &str) -> Result<TaskResult> {
        info!("Executing task: {}", goal);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Running;
            status.current_goal = Some(goal.to_string());
            status.actions_taken = 0;
            status.task_started_at = Some(chrono::Utc::now().timestamp_millis());
        }
        let _ = self.state_tx.send(ControllerState::Running);

        let start_time = Instant::now();
        let timeout = Duration::from_secs(self.config.goal_timeout_secs);
        let min_interval = Duration::from_secs_f32(1.0 / self.config.max_actions_per_second);

        let mut actions_taken = 0u32;
        let mut errors = Vec::new();
        let mut last_action_time = Instant::now() - min_interval;

        // Main control loop
        loop {
            // Check stop signal
            if *self.stop_rx.borrow() {
                info!("Task cancelled by stop signal");
                return Ok(TaskResult {
                    success: false,
                    actions_taken,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors,
                    final_state: "cancelled".to_string(),
                });
            }

            // Check pause signal
            while *self.pause_rx.borrow() {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if *self.stop_rx.borrow() {
                    return Ok(TaskResult {
                        success: false,
                        actions_taken,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        errors,
                        final_state: "cancelled".to_string(),
                    });
                }
            }

            // Check timeout
            if start_time.elapsed() > timeout {
                warn!("Task timed out after {:?}", timeout);
                return Ok(TaskResult {
                    success: false,
                    actions_taken,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors,
                    final_state: "timeout".to_string(),
                });
            }

            // Check max actions
            if actions_taken >= self.config.max_actions_per_goal {
                warn!("Task exceeded max actions ({})", self.config.max_actions_per_goal);
                return Ok(TaskResult {
                    success: false,
                    actions_taken,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors,
                    final_state: "max_actions_exceeded".to_string(),
                });
            }

            // Rate limiting
            let elapsed = last_action_time.elapsed();
            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }

            // Capture screen
            let screenshot = match self.capture_and_convert().await {
                Ok(s) => s,
                Err(e) => {
                    let err_msg = format!("Screen capture failed: {}", e);
                    error!("{}", err_msg);
                    errors.push(err_msg);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Analyze screen
            let analysis = match self.vision_client.analyze_screen(&screenshot.model).await {
                Ok(a) => a,
                Err(e) => {
                    let err_msg = format!("Screen analysis failed: {}", e);
                    error!("{}", err_msg);
                    errors.push(err_msg);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Check if goal achieved
            match self.planner.is_goal_achieved(&self.vision_client, goal, &screenshot.model, &analysis).await {
                Ok(true) => {
                    info!("Goal achieved: {}", goal);
                    return Ok(TaskResult {
                        success: true,
                        actions_taken,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        errors,
                        final_state: "completed".to_string(),
                    });
                }
                Ok(false) => {}
                Err(e) => {
                    debug!("Goal check failed (continuing): {}", e);
                }
            }

            // Find relevant skills
            let skills = if self.config.learning_enabled {
                match self.learning.find_relevant_skills(goal, &screenshot.learning) {
                    Ok(s) => {
                        // Update status with matched skills
                        let mut status = self.status.write().await;
                        status.matched_skills = s.iter().map(|m| m.skill.name.clone()).collect();
                        s
                    }
                    Err(e) => {
                        debug!("Skill search failed (continuing): {}", e);
                        vec![]
                    }
                }
            } else {
                vec![]
            };

            // Plan next action
            let planned = match self.planner.plan_next_action(
                &self.vision_client,
                goal,
                &screenshot.model,
                &analysis,
                &skills,
            ).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    info!("No action planned, checking if goal achieved");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => {
                    let err_msg = format!("Action planning failed: {}", e);
                    error!("{}", err_msg);
                    errors.push(err_msg);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Check confidence threshold
            if planned.confidence < self.config.min_action_confidence {
                debug!(
                    "Action confidence too low ({:.2} < {:.2}), skipping",
                    planned.confidence, self.config.min_action_confidence
                );
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            // Update status
            {
                let mut status = self.status.write().await;
                status.current_action = Some(planned.reasoning.clone());
            }

            // Safety check
            let safety_result = {
                let safety = self.safety.read().await;
                safety.is_allowed(&planned.action)
            };

            match safety_result {
                SafetyResult::Blocked(reason) => {
                    let err_msg = format!("Action blocked: {}", reason);
                    warn!("{}", err_msg);
                    errors.push(err_msg);
                    continue;
                }
                SafetyResult::RequiresConfirmation(reason) => {
                    // Request confirmation if handler is available
                    if let Some(ref handler) = self.confirmation {
                        match handler.request_confirmation(&planned.action, &reason).await {
                            Ok(true) => {
                                debug!("User confirmed action");
                            }
                            Ok(false) => {
                                info!("User rejected action");
                                continue;
                            }
                            Err(e) => {
                                warn!("Confirmation failed (skipping): {}", e);
                                continue;
                            }
                        }
                    } else {
                        warn!("Action requires confirmation but no handler: {}", reason);
                        // Continue anyway in absence of handler
                    }
                }
                SafetyResult::Allowed => {}
            }

            // Execute action
            if let Err(e) = self.executor.execute(&planned.action).await {
                let err_msg = format!("Action execution failed: {}", e);
                error!("{}", err_msg);
                errors.push(err_msg);

                // Record failure if from skill
                if let Some(ref skill_id) = planned.from_skill {
                    let _ = self.learning.report_outcome(skill_id, false);
                }

                continue;
            }

            actions_taken += 1;
            last_action_time = Instant::now();

            // Update status
            {
                let mut status = self.status.write().await;
                status.actions_taken = actions_taken;
                status.last_action_at = Some(chrono::Utc::now().timestamp_millis());
            }

            // Verify (if full verification is enabled)
            if self.config.speed_mode.full_verification() {
                tokio::time::sleep(Duration::from_millis(
                    self.config.speed_mode.verification_delay_ms()
                )).await;

                // Capture new screenshot for verification
                if let Ok(new_screenshot) = self.capture_and_convert().await {
                    if let Ok(new_analysis) = self.vision_client.analyze_screen(&new_screenshot.model).await {
                        // Check if goal achieved after action
                        if let Ok(true) = self.planner.is_goal_achieved(
                            &self.vision_client,
                            goal,
                            &new_screenshot.model,
                            &new_analysis,
                        ).await {
                            info!("Goal achieved after action: {}", goal);

                            // Record success if from skill
                            if let Some(ref skill_id) = planned.from_skill {
                                let _ = self.learning.report_outcome(skill_id, true);
                            }

                            return Ok(TaskResult {
                                success: true,
                                actions_taken,
                                duration_ms: start_time.elapsed().as_millis() as u64,
                                errors,
                                final_state: "completed".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // ========================================================================
    // Recording/Demonstration
    // ========================================================================

    /// Start recording a demonstration.
    pub async fn start_demonstration(&self, name: &str) -> Result<String> {
        info!("Starting demonstration recording: {}", name);

        // Get current app info from a screenshot
        let screenshot = self.capture_and_convert().await?;
        let analysis = self.vision_client.analyze_screen(&screenshot.model).await
            .map_err(|e| Error::invalid(format!("Screen analysis failed: {}", e)))?;

        let app_name = if analysis.app_name.is_empty() {
            "Unknown".to_string()
        } else {
            analysis.app_name
        };

        // Start recording in learning engine
        let session_id = self.learning.start_recording(&app_name, name)?;

        // Update state
        *self.recording_name.lock().await = Some(name.to_string());
        let _ = self.state_tx.send(ControllerState::Recording);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Recording;
            status.is_recording = true;
            status.recording_name = Some(name.to_string());
        }

        Ok(session_id)
    }

    /// Stop recording and return the demonstration.
    pub async fn stop_demonstration(&self) -> Result<Demonstration> {
        info!("Stopping demonstration recording");

        let demo = self.learning.stop_recording()?;

        // Update state
        *self.recording_name.lock().await = None;
        let _ = self.state_tx.send(ControllerState::Idle);

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ControllerState::Idle;
            status.is_recording = false;
            status.recording_name = None;
        }

        Ok(demo)
    }

    /// Check if recording is active.
    pub async fn is_recording(&self) -> bool {
        self.recording_name.lock().await.is_some()
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Capture a screenshot and convert to both model and learning formats.
    async fn capture_and_convert(&self) -> Result<CapturedScreenshots> {
        let mut screenshot = self.capture.capture_screen(None).await
            .map_err(|e| Error::invalid(format!("Screen capture failed: {}", e)))?;

        // Convert to model screenshot
        let model_screenshot = ModelScreenshot::new(
            screenshot.image().clone(),
            format!("{}", screenshot.metadata.source),
        );

        // Convert to learning screenshot
        let base64 = screenshot.to_base64()
            .map_err(|e| Error::invalid(format!("Base64 encoding failed: {}", e)))?;

        let learning_screenshot = LearningScreenshot::new(
            base64,
            screenshot.width(),
            screenshot.height(),
        );

        Ok(CapturedScreenshots {
            model: model_screenshot,
            learning: learning_screenshot,
        })
    }
}

/// Helper struct for captured screenshots in both formats.
struct CapturedScreenshots {
    model: ModelScreenshot,
    learning: LearningScreenshot,
}

/// Result of a task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether the task succeeded
    pub success: bool,
    /// Number of actions taken
    pub actions_taken: u32,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Errors encountered
    pub errors: Vec<String>,
    /// Final state description
    pub final_state: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_mode_defaults() {
        assert_eq!(SpeedMode::default(), SpeedMode::Normal);
        assert_eq!(SpeedMode::Careful.base_delay_ms(), 2000);
        assert_eq!(SpeedMode::Normal.base_delay_ms(), 1000);
        assert_eq!(SpeedMode::Fast.base_delay_ms(), 500);
        assert_eq!(SpeedMode::Turbo.base_delay_ms(), 100);
    }

    #[test]
    fn test_speed_mode_verification() {
        assert!(SpeedMode::Careful.full_verification());
        assert!(SpeedMode::Normal.full_verification());
        assert!(!SpeedMode::Fast.full_verification());
        assert!(!SpeedMode::Turbo.full_verification());
    }

    #[test]
    fn test_controller_config_defaults() {
        let config = VisionControllerConfig::default();
        assert_eq!(config.speed_mode, SpeedMode::Normal);
        assert_eq!(config.max_actions_per_second, 1.0);
        assert!(config.learning_enabled);
        assert!(config.overlay_enabled);
    }

    #[test]
    fn test_controller_config_builders() {
        let careful = VisionControllerConfig::careful();
        assert_eq!(careful.speed_mode, SpeedMode::Careful);
        assert_eq!(careful.max_actions_per_second, 0.5);

        let fast = VisionControllerConfig::fast();
        assert_eq!(fast.speed_mode, SpeedMode::Fast);
        assert_eq!(fast.max_actions_per_second, 2.0);
    }

    #[test]
    fn test_safety_checker() {
        let checker = SafetyChecker::new(vec![ActionType::TextInput]);

        // Text input should require confirmation
        let text_action = Action::new(
            ActionType::TextInput,
            ActionDetails::TextInput { text: "hello".to_string() },
            "Test",
        );
        assert!(checker.is_allowed(&text_action).requires_confirmation());

        // Mouse click should be allowed
        let click_action = Action::new(
            ActionType::MouseClick,
            ActionDetails::MouseClick {
                x: 100,
                y: 200,
                button: MouseButton::Left,
                modifiers: vec![],
            },
            "Test",
        );
        assert!(checker.is_allowed(&click_action).is_allowed());
    }

    #[test]
    fn test_safety_result() {
        let allowed = SafetyResult::Allowed;
        assert!(allowed.is_allowed());
        assert!(!allowed.requires_confirmation());
        assert!(!allowed.is_blocked());

        let confirmation = SafetyResult::RequiresConfirmation("reason".to_string());
        assert!(!confirmation.is_allowed());
        assert!(confirmation.requires_confirmation());
        assert!(!confirmation.is_blocked());

        let blocked = SafetyResult::Blocked("reason".to_string());
        assert!(!blocked.is_allowed());
        assert!(!blocked.requires_confirmation());
        assert!(blocked.is_blocked());
    }

    #[test]
    fn test_controller_status_default() {
        let status = ControllerStatus::default();
        assert_eq!(status.state, ControllerState::Idle);
        assert!(status.current_goal.is_none());
        assert_eq!(status.actions_taken, 0);
        assert!(status.errors.is_empty());
    }

    #[test]
    fn test_task_result() {
        let result = TaskResult {
            success: true,
            actions_taken: 5,
            duration_ms: 10000,
            errors: vec![],
            final_state: "completed".to_string(),
        };
        assert!(result.success);
        assert_eq!(result.actions_taken, 5);
    }
}
