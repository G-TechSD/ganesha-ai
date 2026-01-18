//! Action planning for the Vision/VLA system.
//!
//! This module provides:
//! - VisionTask describing what to accomplish
//! - ActionPlan with sequence of UI interactions
//! - Plan verification after each step
//! - Error recovery (retry, alternative actions)
//! - Human confirmation for destructive actions

use crate::analysis::{ScreenAnalysis, VisionAnalyzer};
use crate::apps::{AppAction, AppController};
use crate::capture::{ScreenCapture, Screenshot};
use crate::config::VisionConfig;
use crate::input::InputSimulator;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors that can occur during planning and execution.
#[derive(Error, Debug)]
pub enum PlannerError {
    #[error("Failed to analyze screen: {0}")]
    AnalysisFailed(String),

    #[error("Failed to execute action: {0}")]
    ExecutionFailed(String),

    #[error("Plan verification failed: {0}")]
    VerificationFailed(String),

    #[error("No viable plan found: {0}")]
    NoPlanFound(String),

    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    #[error("User confirmation required")]
    ConfirmationRequired(ConfirmationRequest),

    #[error("User cancelled operation")]
    UserCancelled,

    #[error("Emergency stop triggered")]
    EmergencyStop,

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Result type for planner operations.
pub type PlannerResult<T> = Result<T, PlannerError>;

/// A task to be accomplished via vision-based automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionTask {
    /// Unique identifier for this task
    pub id: String,
    /// Human-readable description of what to accomplish
    pub description: String,
    /// Target application (if specific)
    pub target_app: Option<String>,
    /// Expected outcome description
    pub expected_outcome: String,
    /// Maximum time allowed for task
    pub timeout: Duration,
    /// Whether to require confirmation before destructive actions
    pub require_confirmation: bool,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Priority (higher = more important)
    pub priority: i32,
}

impl VisionTask {
    /// Create a new vision task.
    pub fn new(description: impl Into<String>, expected_outcome: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.into(),
            target_app: None,
            expected_outcome: expected_outcome.into(),
            timeout: Duration::from_secs(60),
            require_confirmation: true,
            max_retries: 3,
            priority: 0,
        }
    }

    /// Set the target application.
    pub fn with_target_app(mut self, app: impl Into<String>) -> Self {
        self.target_app = Some(app.into());
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set whether confirmation is required.
    pub fn with_confirmation(mut self, require: bool) -> Self {
        self.require_confirmation = require;
        self
    }

    /// Set maximum retries.
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// A single step in an action plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step number
    pub step_number: u32,
    /// Description of this step
    pub description: String,
    /// The action to perform
    pub action: PlannedAction,
    /// Expected state after this step
    pub expected_state: Option<String>,
    /// Whether this step is destructive/irreversible
    pub is_destructive: bool,
    /// Retry count for this step
    pub retries: u32,
}

/// Actions that can be planned.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannedAction {
    /// Click on an element
    ClickElement {
        element_description: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        element_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        coordinates: Option<(i32, i32)>,
    },
    /// Type text
    TypeText {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_element: Option<String>,
    },
    /// Press a keyboard shortcut
    Shortcut { shortcut: String },
    /// Wait for a condition
    WaitFor {
        condition: String,
        timeout_ms: u64,
    },
    /// Scroll in a direction
    Scroll {
        direction: ScrollDirection,
        amount: i32,
    },
    /// Drag from one location to another
    DragDrop {
        from_element: String,
        to_element: String,
    },
    /// Focus an application
    FocusApp { app_name: String },
    /// Verify a condition
    Verify { condition: String },
    /// Execute an app-specific action
    AppAction {
        app_name: String,
        action: AppAction,
    },
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// A complete action plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    /// The task this plan is for
    pub task: VisionTask,
    /// The steps to execute
    pub steps: Vec<PlanStep>,
    /// Current step index
    pub current_step: usize,
    /// Plan generation timestamp
    pub created_at: i64,
    /// Overall confidence in the plan (0.0 to 1.0)
    pub confidence: f32,
    /// Alternative plans if this one fails
    pub alternatives: Vec<ActionPlan>,
}

impl ActionPlan {
    /// Create a new action plan.
    pub fn new(task: VisionTask, steps: Vec<PlanStep>) -> Self {
        Self {
            task,
            steps,
            current_step: 0,
            created_at: chrono::Utc::now().timestamp_millis(),
            confidence: 0.5,
            alternatives: Vec::new(),
        }
    }

    /// Get the next step to execute.
    pub fn next_step(&self) -> Option<&PlanStep> {
        self.steps.get(self.current_step)
    }

    /// Mark current step as complete and move to next.
    pub fn advance(&mut self) -> bool {
        if self.current_step < self.steps.len() {
            self.current_step += 1;
            true
        } else {
            false
        }
    }

    /// Check if the plan is complete.
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Get progress as a percentage.
    pub fn progress(&self) -> f32 {
        if self.steps.is_empty() {
            1.0
        } else {
            self.current_step as f32 / self.steps.len() as f32
        }
    }

    /// Add an alternative plan.
    pub fn with_alternative(mut self, plan: ActionPlan) -> Self {
        self.alternatives.push(plan);
        self
    }
}

/// A request for user confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// Unique request ID
    pub id: String,
    /// Description of the action requiring confirmation
    pub action_description: String,
    /// Why confirmation is needed
    pub reason: String,
    /// The step that requires confirmation
    pub step: PlanStep,
    /// Screenshot at time of request (base64 encoded)
    pub screenshot: Option<String>,
}

/// Status of plan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Not started
    Pending,
    /// Currently executing
    Running,
    /// Waiting for user confirmation
    WaitingConfirmation,
    /// Paused
    Paused,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
}

/// Execution context for a plan.
#[derive(Debug)]
pub struct ExecutionContext {
    /// Current status
    pub status: ExecutionStatus,
    /// Current plan
    pub plan: ActionPlan,
    /// Error message if failed
    pub error: Option<String>,
    /// Screenshots taken during execution
    pub screenshots: Vec<Screenshot>,
    /// Execution history
    pub history: Vec<ExecutionEvent>,
    /// Start time
    pub started_at: Option<i64>,
    /// End time
    pub ended_at: Option<i64>,
}

/// An event during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// Event timestamp
    pub timestamp: i64,
    /// Event type
    pub event_type: ExecutionEventType,
    /// Event description
    pub description: String,
}

/// Types of execution events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventType {
    PlanStarted,
    StepStarted,
    StepCompleted,
    StepFailed,
    StepRetried,
    VerificationPassed,
    VerificationFailed,
    ConfirmationRequested,
    ConfirmationReceived,
    PlanCompleted,
    PlanFailed,
    EmergencyStop,
}

/// The action planner that creates and executes plans.
pub struct ActionPlanner<C, I, A, V>
where
    C: ScreenCapture,
    I: InputSimulator,
    A: AppController,
    V: VisionAnalyzer,
{
    capture: Arc<C>,
    input: Arc<I>,
    app_controller: Arc<A>,
    analyzer: Arc<V>,
    config: VisionConfig,
    emergency_stop: Arc<RwLock<bool>>,
    confirmation_handler: Option<Box<dyn ConfirmationHandler + Send + Sync>>,
}

/// Trait for handling confirmation requests.
#[async_trait::async_trait]
pub trait ConfirmationHandler: Send + Sync {
    /// Request confirmation from user.
    async fn request_confirmation(&self, request: &ConfirmationRequest) -> bool;
}

impl<C, I, A, V> ActionPlanner<C, I, A, V>
where
    C: ScreenCapture + Send + Sync + 'static,
    I: InputSimulator + Send + Sync + 'static,
    A: AppController + Send + Sync + 'static,
    V: VisionAnalyzer + Send + Sync + 'static,
{
    /// Create a new action planner.
    pub fn new(
        capture: C,
        input: I,
        app_controller: A,
        analyzer: V,
        config: VisionConfig,
    ) -> Self {
        Self {
            capture: Arc::new(capture),
            input: Arc::new(input),
            app_controller: Arc::new(app_controller),
            analyzer: Arc::new(analyzer),
            config,
            emergency_stop: Arc::new(RwLock::new(false)),
            confirmation_handler: None,
        }
    }

    /// Set the confirmation handler.
    pub fn with_confirmation_handler(
        mut self,
        handler: Box<dyn ConfirmationHandler + Send + Sync>,
    ) -> Self {
        self.confirmation_handler = Some(handler);
        self
    }

    /// Trigger emergency stop.
    pub async fn emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = true;
    }

    /// Reset emergency stop.
    pub async fn reset_emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = false;
    }

    /// Check if emergency stop is active.
    async fn check_emergency_stop(&self) -> bool {
        *self.emergency_stop.read().await
    }

    /// Analyze the current screen state.
    async fn analyze_screen(&self) -> PlannerResult<(Screenshot, ScreenAnalysis)> {
        let screenshot = self
            .capture
            .capture_all()
            .await
            .map_err(|e| PlannerError::AnalysisFailed(e.to_string()))?;

        let analysis = self
            .analyzer
            .analyze(&screenshot, None)
            .await
            .map_err(|e| PlannerError::AnalysisFailed(e.to_string()))?;

        Ok((screenshot, analysis))
    }

    /// Create an action plan for a task.
    pub async fn create_plan(&self, task: VisionTask) -> PlannerResult<ActionPlan> {
        // Analyze current screen state
        let (screenshot, analysis) = self.analyze_screen().await?;

        // Ask the vision model to create a plan
        let plan_prompt = format!(
            r#"Given the current screen state and task, create a step-by-step plan.

Task: {}
Expected Outcome: {}
Target App: {}

Current Screen Description: {}
Available Elements: {:?}

Create a JSON plan with this structure:
{{
    "steps": [
        {{
            "step_number": 1,
            "description": "Step description",
            "action": {{"type": "click_element", "element_description": "Button text"}},
            "expected_state": "What should happen after this step",
            "is_destructive": false
        }}
    ],
    "confidence": 0.8
}}

Available action types:
- click_element: {{"type": "click_element", "element_description": "..."}}
- type_text: {{"type": "type_text", "text": "...", "target_element": "..."}}
- shortcut: {{"type": "shortcut", "shortcut": "Ctrl+S"}}
- wait_for: {{"type": "wait_for", "condition": "...", "timeout_ms": 5000}}
- scroll: {{"type": "scroll", "direction": "down", "amount": 3}}
- focus_app: {{"type": "focus_app", "app_name": "..."}}
- verify: {{"type": "verify", "condition": "..."}}"#,
            task.description,
            task.expected_outcome,
            task.target_app.as_deref().unwrap_or("Any"),
            analysis.description,
            analysis.elements.iter().take(10).collect::<Vec<_>>()
        );

        let plan_response = self
            .analyzer
            .ask(&screenshot, &plan_prompt)
            .await
            .map_err(|e| PlannerError::AnalysisFailed(e.to_string()))?;

        // Parse the plan
        let plan_json: serde_json::Value = serde_json::from_str(&plan_response)
            .or_else(|_| {
                // Try to extract JSON from response
                if let Some(start) = plan_response.find('{') {
                    if let Some(end) = plan_response.rfind('}') {
                        return serde_json::from_str(&plan_response[start..=end]);
                    }
                }
                Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No JSON found",
                )))
            })
            .map_err(|e| PlannerError::NoPlanFound(e.to_string()))?;

        let steps: Vec<PlanStep> = plan_json["steps"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        Some(PlanStep {
                            step_number: s["step_number"].as_u64()? as u32,
                            description: s["description"].as_str()?.to_string(),
                            action: serde_json::from_value(s["action"].clone()).ok()?,
                            expected_state: s["expected_state"].as_str().map(|s| s.to_string()),
                            is_destructive: s["is_destructive"].as_bool().unwrap_or(false),
                            retries: 0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if steps.is_empty() {
            return Err(PlannerError::NoPlanFound(
                "No valid steps could be parsed from the plan".to_string(),
            ));
        }

        let confidence = plan_json["confidence"].as_f64().unwrap_or(0.5) as f32;

        let mut plan = ActionPlan::new(task, steps);
        plan.confidence = confidence;

        Ok(plan)
    }

    /// Execute a single step of a plan.
    async fn execute_step(&self, step: &PlanStep) -> PlannerResult<()> {
        if self.check_emergency_stop().await {
            return Err(PlannerError::EmergencyStop);
        }

        // Check if confirmation is needed
        if step.is_destructive {
            if let Some(ref handler) = self.confirmation_handler {
                let request = ConfirmationRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    action_description: step.description.clone(),
                    reason: "This action is marked as destructive and may not be reversible"
                        .to_string(),
                    step: step.clone(),
                    screenshot: None,
                };

                if !handler.request_confirmation(&request).await {
                    return Err(PlannerError::UserCancelled);
                }
            }
        }

        match &step.action {
            PlannedAction::ClickElement {
                element_description,
                coordinates,
                ..
            } => {
                // If we have coordinates, use them directly
                if let Some((x, y)) = coordinates {
                    self.input
                        .click(*x, *y)
                        .await
                        .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
                } else {
                    // Find the element using vision
                    let (screenshot, _) = self.analyze_screen().await?;
                    let element = self
                        .analyzer
                        .find_element(&screenshot, element_description)
                        .await
                        .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;

                    if let Some(elem) = element {
                        let (cx, cy) = elem.center();
                        self.input
                            .click(cx, cy)
                            .await
                            .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
                    } else {
                        return Err(PlannerError::ExecutionFailed(format!(
                            "Could not find element: {}",
                            element_description
                        )));
                    }
                }
            }

            PlannedAction::TypeText { text, .. } => {
                self.input
                    .type_text(text)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }

            PlannedAction::Shortcut { shortcut } => {
                let parsed = crate::input::KeyboardShortcut::parse(shortcut)
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
                self.input
                    .shortcut(&parsed)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }

            PlannedAction::WaitFor {
                condition,
                timeout_ms,
            } => {
                let start = std::time::Instant::now();
                let timeout = Duration::from_millis(*timeout_ms);

                while start.elapsed() < timeout {
                    if self.check_emergency_stop().await {
                        return Err(PlannerError::EmergencyStop);
                    }

                    let (screenshot, _) = self.analyze_screen().await?;
                    let response = self
                        .analyzer
                        .ask(
                            &screenshot,
                            &format!(
                                "Is this condition satisfied? Answer only 'yes' or 'no': {}",
                                condition
                            ),
                        )
                        .await
                        .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;

                    if response.to_lowercase().contains("yes") {
                        return Ok(());
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                return Err(PlannerError::Timeout(format!(
                    "Condition not met: {}",
                    condition
                )));
            }

            PlannedAction::Scroll { direction, amount } => {
                let (x, y) = self
                    .input
                    .mouse_position()
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;

                let scroll = match direction {
                    ScrollDirection::Up => crate::input::ScrollAction::vertical(x, y, -*amount),
                    ScrollDirection::Down => crate::input::ScrollAction::vertical(x, y, *amount),
                    ScrollDirection::Left => crate::input::ScrollAction::horizontal(x, y, -*amount),
                    ScrollDirection::Right => crate::input::ScrollAction::horizontal(x, y, *amount),
                };

                self.input
                    .mouse_scroll(&scroll)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }

            PlannedAction::DragDrop {
                from_element,
                to_element,
            } => {
                let (screenshot, _) = self.analyze_screen().await?;

                let from = self
                    .analyzer
                    .find_element(&screenshot, from_element)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?
                    .ok_or_else(|| {
                        PlannerError::ExecutionFailed(format!(
                            "Could not find source element: {}",
                            from_element
                        ))
                    })?;

                let to = self
                    .analyzer
                    .find_element(&screenshot, to_element)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?
                    .ok_or_else(|| {
                        PlannerError::ExecutionFailed(format!(
                            "Could not find target element: {}",
                            to_element
                        ))
                    })?;

                let (fx, fy) = from.center();
                let (tx, ty) = to.center();

                let drag =
                    crate::input::DragOperation::new(fx, fy, tx, ty).with_duration(Duration::from_millis(500));

                self.input
                    .mouse_drag(&drag)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }

            PlannedAction::FocusApp { app_name } => {
                let app = self
                    .app_controller
                    .find_app(app_name)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?
                    .ok_or_else(|| {
                        PlannerError::ExecutionFailed(format!("App not found: {}", app_name))
                    })?;

                self.app_controller
                    .focus(&app)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }

            PlannedAction::Verify { condition } => {
                let (screenshot, _) = self.analyze_screen().await?;
                let response = self
                    .analyzer
                    .ask(
                        &screenshot,
                        &format!(
                            "Verify this condition. Answer 'VERIFIED' if true, 'FAILED' if false: {}",
                            condition
                        ),
                    )
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;

                if response.to_uppercase().contains("FAILED") {
                    return Err(PlannerError::VerificationFailed(condition.clone()));
                }
            }

            PlannedAction::AppAction { app_name, action } => {
                let app = self
                    .app_controller
                    .find_app(app_name)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?
                    .ok_or_else(|| {
                        PlannerError::ExecutionFailed(format!("App not found: {}", app_name))
                    })?;

                self.app_controller
                    .execute_action(&app, action)
                    .await
                    .map_err(|e| PlannerError::ExecutionFailed(e.to_string()))?;
            }
        }

        // Delay after action
        tokio::time::sleep(Duration::from_millis(
            self.config.safety.action_delay_ms,
        ))
        .await;

        Ok(())
    }

    /// Execute a complete plan.
    pub async fn execute_plan(&self, mut plan: ActionPlan) -> PlannerResult<ExecutionContext> {
        let mut context = ExecutionContext {
            status: ExecutionStatus::Running,
            plan: plan.clone(),
            error: None,
            screenshots: Vec::new(),
            history: vec![ExecutionEvent {
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: ExecutionEventType::PlanStarted,
                description: format!("Started plan for: {}", plan.task.description),
            }],
            started_at: Some(chrono::Utc::now().timestamp_millis()),
            ended_at: None,
        };

        let start_time = std::time::Instant::now();

        while let Some(step) = plan.next_step() {
            // Check timeout
            if start_time.elapsed() > plan.task.timeout {
                context.status = ExecutionStatus::Failed;
                context.error = Some("Task timeout exceeded".to_string());
                context.history.push(ExecutionEvent {
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    event_type: ExecutionEventType::PlanFailed,
                    description: "Task timeout exceeded".to_string(),
                });
                break;
            }

            // Check emergency stop
            if self.check_emergency_stop().await {
                context.status = ExecutionStatus::Failed;
                context.error = Some("Emergency stop triggered".to_string());
                context.history.push(ExecutionEvent {
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    event_type: ExecutionEventType::EmergencyStop,
                    description: "Emergency stop triggered".to_string(),
                });
                break;
            }

            context.history.push(ExecutionEvent {
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: ExecutionEventType::StepStarted,
                description: format!("Step {}: {}", step.step_number, step.description),
            });

            // Execute the step with retry logic
            let mut retries = 0;
            let step_clone = step.clone();
            loop {
                match self.execute_step(&step_clone).await {
                    Ok(()) => {
                        context.history.push(ExecutionEvent {
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            event_type: ExecutionEventType::StepCompleted,
                            description: format!("Step {} completed", step_clone.step_number),
                        });
                        break;
                    }
                    Err(PlannerError::EmergencyStop) => {
                        context.status = ExecutionStatus::Failed;
                        context.error = Some("Emergency stop".to_string());
                        context.history.push(ExecutionEvent {
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            event_type: ExecutionEventType::EmergencyStop,
                            description: "Emergency stop triggered".to_string(),
                        });
                        return Ok(context);
                    }
                    Err(PlannerError::UserCancelled) => {
                        context.status = ExecutionStatus::Cancelled;
                        context.error = Some("User cancelled".to_string());
                        return Ok(context);
                    }
                    Err(e) => {
                        retries += 1;
                        if retries > plan.task.max_retries {
                            context.status = ExecutionStatus::Failed;
                            context.error = Some(e.to_string());
                            context.history.push(ExecutionEvent {
                                timestamp: chrono::Utc::now().timestamp_millis(),
                                event_type: ExecutionEventType::StepFailed,
                                description: format!(
                                    "Step {} failed after {} retries: {}",
                                    step_clone.step_number, retries, e
                                ),
                            });
                            return Ok(context);
                        }

                        context.history.push(ExecutionEvent {
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            event_type: ExecutionEventType::StepRetried,
                            description: format!(
                                "Step {} retry {}: {}",
                                step_clone.step_number, retries, e
                            ),
                        });

                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }

            plan.advance();
        }

        if context.status == ExecutionStatus::Running {
            context.status = ExecutionStatus::Completed;
            context.history.push(ExecutionEvent {
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: ExecutionEventType::PlanCompleted,
                description: "Plan completed successfully".to_string(),
            });
        }

        context.ended_at = Some(chrono::Utc::now().timestamp_millis());
        context.plan = plan;

        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_task_creation() {
        let task = VisionTask::new("Click save button", "File should be saved")
            .with_target_app("Notepad")
            .with_timeout(Duration::from_secs(30))
            .with_max_retries(5);

        assert_eq!(task.description, "Click save button");
        assert_eq!(task.target_app, Some("Notepad".to_string()));
        assert_eq!(task.timeout, Duration::from_secs(30));
        assert_eq!(task.max_retries, 5);
    }

    #[test]
    fn test_action_plan_progress() {
        let task = VisionTask::new("Test", "Test outcome");
        let steps = vec![
            PlanStep {
                step_number: 1,
                description: "Step 1".to_string(),
                action: PlannedAction::TypeText {
                    text: "test".to_string(),
                    target_element: None,
                },
                expected_state: None,
                is_destructive: false,
                retries: 0,
            },
            PlanStep {
                step_number: 2,
                description: "Step 2".to_string(),
                action: PlannedAction::Shortcut {
                    shortcut: "Ctrl+S".to_string(),
                },
                expected_state: None,
                is_destructive: false,
                retries: 0,
            },
        ];

        let mut plan = ActionPlan::new(task, steps);

        assert_eq!(plan.progress(), 0.0);
        assert!(!plan.is_complete());

        plan.advance();
        assert_eq!(plan.progress(), 0.5);

        plan.advance();
        assert_eq!(plan.progress(), 1.0);
        assert!(plan.is_complete());
    }

    #[test]
    fn test_planned_action_serialization() {
        let action = PlannedAction::ClickElement {
            element_description: "Save button".to_string(),
            element_id: None,
            coordinates: Some((100, 200)),
        };

        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("click_element"));
        assert!(json.contains("Save button"));
    }
}
