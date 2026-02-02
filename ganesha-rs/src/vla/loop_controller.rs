//! VLA Loop Controller
//!
//! The closed-loop system that runs: capture → analyze → plan → act → verify

use super::*;
use crate::input::{InputController, MouseButton};
use crate::vision::VisionController;
use crate::orchestrator::vision::{VisionAnalyzer, VisionConfig};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// The VLA closed-loop controller
pub struct VlaLoop {
    config: VlaConfig,
    vision: VisionController,
    input: InputController,
    analyzer: VisionAnalyzer,
    planner: ActionPlanner,
    locator: ElementLocator,
    /// Emergency stop flag
    stop_flag: Arc<AtomicBool>,
    /// Current status
    status: Arc<RwLock<Option<VlaStatus>>>,
}

impl VlaLoop {
    pub fn new(config: VlaConfig) -> Self {
        let vision_config = VisionConfig {
            endpoint: config.vision_endpoint.clone(),
            model: config.vision_model.clone(),
            timeout: Duration::from_secs(30),
        };

        Self {
            vision: VisionController::new(),
            input: InputController::new(),
            analyzer: VisionAnalyzer::new(vision_config),
            planner: ActionPlanner::new(
                config.planner_endpoint.clone(),
                config.planner_model.clone(),
            ),
            locator: ElementLocator::new(
                config.vision_endpoint.clone(),
                config.vision_model.clone(),
            ),
            config,
            stop_flag: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(VlaConfig::default())
    }

    /// Emergency stop the VLA loop
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Check if stopped
    fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    /// Get current status
    pub async fn status(&self) -> Option<VlaStatus> {
        self.status.read().await.clone()
    }

    /// Execute a goal using the VLA loop
    #[cfg(feature = "vision")]
    #[cfg(feature = "input")]
    pub async fn execute_goal(&self, goal: VlaGoal) -> Result<VlaStatus, VlaError> {
        // Reset stop flag
        self.stop_flag.store(false, Ordering::SeqCst);

        // Initialize status
        let mut status = VlaStatus {
            goal: goal.clone(),
            actions_taken: 0,
            current_state: "Starting".into(),
            success: false,
            completed: false,
            error: None,
            action_history: vec![],
        };
        *self.status.write().await = Some(status.clone());

        // Enable vision and input
        self.vision.enable().map_err(|e| VlaError::VisionError(e.to_string()))?;
        self.input.enable().map_err(|e| VlaError::InputError(e.to_string()))?;

        let start_time = Instant::now();

        // Main VLA loop
        while status.actions_taken < goal.max_actions {
            // Check timeout
            if start_time.elapsed() > goal.timeout {
                status.error = Some("Timeout exceeded".into());
                break;
            }

            // Check stop flag
            if self.is_stopped() {
                status.error = Some("Stopped by user".into());
                break;
            }

            // 1. CAPTURE - Get current screen state
            let screenshot = self.vision
                .capture_screen_scaled(self.config.capture_width, self.config.capture_height)
                .map_err(|e| VlaError::VisionError(e.to_string()))?;

            // Save screenshot if configured
            if self.config.save_screenshots {
                self.save_screenshot(&screenshot.data, status.actions_taken).await;
            }

            // 2. ANALYZE - Understand what's on screen
            let analysis = self.analyzer
                .analyze_image(&screenshot.data)
                .await
                .map_err(|e| VlaError::AnalysisError(e.to_string()))?;

            status.current_state = format!(
                "App: {}, State: {:?}, Elements: {}",
                analysis.app,
                analysis.state,
                analysis.elements.len()
            );

            // 3. CHECK - Are we done?
            let goal_achieved = self.check_success_criteria(&goal, &analysis).await?;
            if goal_achieved {
                status.success = true;
                status.completed = true;
                break;
            }

            // 4. PLAN - Decide what action to take
            let action = self.planner
                .plan_next_action(&goal, &analysis, &status.action_history)
                .await
                .map_err(|e| VlaError::PlanningError(e.to_string()))?;

            // If planner says we're done or stuck
            if action.is_none() {
                status.current_state = "Planner indicates goal achieved or no viable actions".into();
                status.completed = true;
                break;
            }

            let mut action = action.unwrap();

            // 5. LOCATE - Find exact coordinates for the target
            if let Some(ref mut target) = action.target {
                let located = self.locator
                    .locate_element(&screenshot.data, &target.description)
                    .await;

                if let Ok((x, y, conf)) = located {
                    target.x = x;
                    target.y = y;
                    target.location_confidence = conf;
                }
            }

            // 6. ACT - Execute the action
            let action_start = Instant::now();
            let exec_result = self.execute_action(&action).await;
            let action_duration = action_start.elapsed().as_millis() as u64;

            // 7. WAIT - Give UI time to respond
            tokio::time::sleep(self.config.action_delay).await;

            // 8. VERIFY - Check if action had expected effect
            let verify_screenshot = self.vision
                .capture_screen_scaled(self.config.capture_width, self.config.capture_height)
                .map_err(|e| VlaError::VisionError(e.to_string()))?;

            let verify_analysis = self.analyzer
                .analyze_image(&verify_screenshot.data)
                .await
                .ok();

            let expected_achieved = if let Some(ref analysis) = verify_analysis {
                // Simple verification: check if state changed as expected
                self.verify_expected_result(&action, analysis).await
            } else {
                false
            };

            // Record action result
            let result = ActionResult {
                action: action.clone(),
                success: exec_result.is_ok(),
                error: exec_result.err().map(|e| e.to_string()),
                screen_state: verify_analysis.map(|a| format!("{:?}", a.state)),
                expected_achieved,
                duration_ms: action_duration,
            };

            status.action_history.push(result);
            status.actions_taken += 1;

            // Update shared status
            *self.status.write().await = Some(status.clone());
        }

        // Cleanup
        self.vision.disable();
        self.input.disable();

        if !status.completed {
            status.completed = true;
            if status.error.is_none() && !status.success {
                status.error = Some("Max actions reached without achieving goal".into());
            }
        }

        *self.status.write().await = Some(status.clone());
        Ok(status)
    }

    /// Execute a single action
    #[cfg(feature = "input")]
    async fn execute_action(&self, action: &PlannedAction) -> Result<(), VlaError> {
        match action.action_type {
            ActionType::Click => {
                if let Some(ref target) = action.target {
                    self.input
                        .mouse_move(target.x, target.y)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.input
                        .mouse_click(MouseButton::Left)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
            ActionType::DoubleClick => {
                if let Some(ref target) = action.target {
                    self.input
                        .mouse_move(target.x, target.y)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.input
                        .mouse_double_click(MouseButton::Left)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
            ActionType::RightClick => {
                if let Some(ref target) = action.target {
                    self.input
                        .mouse_move(target.x, target.y)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.input
                        .mouse_click(MouseButton::Right)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
            ActionType::Type => {
                if let Some(ref text) = action.text {
                    // Click target first if specified
                    if let Some(ref target) = action.target {
                        self.input
                            .mouse_move(target.x, target.y)
                            .map_err(|e| VlaError::InputError(e.to_string()))?;
                        self.input
                            .mouse_click(MouseButton::Left)
                            .map_err(|e| VlaError::InputError(e.to_string()))?;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    self.input
                        .type_text(text)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
            ActionType::KeyPress => {
                if let Some(ref keys) = action.keys {
                    if keys.contains('+') {
                        self.input
                            .key_combination(keys)
                            .map_err(|e| VlaError::InputError(e.to_string()))?;
                    } else {
                        self.input
                            .key_press(keys)
                            .map_err(|e| VlaError::InputError(e.to_string()))?;
                    }
                }
            }
            ActionType::Scroll => {
                // Default scroll down, could enhance with direction in action
                self.input
                    .scroll(0, -3)
                    .map_err(|e| VlaError::InputError(e.to_string()))?;
            }
            ActionType::Wait => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            ActionType::Hover => {
                if let Some(ref target) = action.target {
                    self.input
                        .mouse_move(target.x, target.y)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
            ActionType::Drag => {
                // Would need start and end points, simplified for now
                if let Some(ref target) = action.target {
                    self.input
                        .mouse_move(target.x, target.y)
                        .map_err(|e| VlaError::InputError(e.to_string()))?;
                }
            }
        }

        Ok(())
    }

    /// Check if success criteria are met
    async fn check_success_criteria(
        &self,
        goal: &VlaGoal,
        analysis: &crate::orchestrator::vision::ScreenAnalysis,
    ) -> Result<bool, VlaError> {
        // Check each success criterion
        for criterion in &goal.success_criteria {
            let criterion_lower = criterion.to_lowercase();
            
            // Check in visible text
            let text_match = analysis.text.iter().any(|t| {
                t.to_lowercase().contains(&criterion_lower)
            });

            // Check in elements
            let element_match = analysis.elements.iter().any(|e| {
                e.label.to_lowercase().contains(&criterion_lower)
            });

            // Check app/title
            let title_match = analysis.title.to_lowercase().contains(&criterion_lower)
                || analysis.app.to_lowercase().contains(&criterion_lower);

            if text_match || element_match || title_match {
                continue; // This criterion is met
            } else {
                return Ok(false); // This criterion is not met
            }
        }

        // All criteria met
        Ok(true)
    }

    /// Verify expected result of an action
    async fn verify_expected_result(
        &self,
        action: &PlannedAction,
        analysis: &crate::orchestrator::vision::ScreenAnalysis,
    ) -> bool {
        let expected = action.expected_result.to_lowercase();

        // Check if expected result appears in screen state
        analysis.text.iter().any(|t| t.to_lowercase().contains(&expected))
            || analysis.elements.iter().any(|e| e.label.to_lowercase().contains(&expected))
            || analysis.title.to_lowercase().contains(&expected)
            || format!("{:?}", analysis.state).to_lowercase().contains(&expected)
    }

    /// Save screenshot for debugging
    async fn save_screenshot(&self, base64_data: &str, action_num: usize) {
        use std::fs;
        use base64_lib::{engine::general_purpose::STANDARD as BASE64, Engine};

        let _ = fs::create_dir_all(&self.config.screenshot_dir);
        let path = format!("{}/action_{:03}.png", self.config.screenshot_dir, action_num);

        if let Ok(data) = BASE64.decode(base64_data) {
            let _ = fs::write(path, data);
        }
    }

    // Stub implementations when features not compiled
    #[cfg(not(all(feature = "vision", feature = "input")))]
    pub async fn execute_goal(&self, _goal: VlaGoal) -> Result<VlaStatus, VlaError> {
        Err(VlaError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    async fn execute_action(&self, _action: &PlannedAction) -> Result<(), VlaError> {
        Err(VlaError::FeatureNotCompiled)
    }
}

/// VLA errors
#[derive(Debug, thiserror::Error)]
pub enum VlaError {
    #[error("VLA features not compiled. Rebuild with --features vision,input")]
    FeatureNotCompiled,

    #[error("Vision error: {0}")]
    VisionError(String),

    #[error("Input error: {0}")]
    InputError(String),

    #[error("Analysis error: {0}")]
    AnalysisError(String),

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Locator error: {0}")]
    LocatorError(String),

    #[error("Goal failed: {0}")]
    GoalFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vla_config_default() {
        let config = VlaConfig::default();
        assert_eq!(config.capture_width, 1280);
        assert_eq!(config.capture_height, 720);
    }

    #[tokio::test]
    async fn test_vla_stop() {
        let vla = VlaLoop::with_defaults();
        assert!(!vla.is_stopped());
        vla.stop();
        assert!(vla.is_stopped());
    }
}
