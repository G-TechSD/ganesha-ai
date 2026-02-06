//! VLA Loop Controller
//!
//! The closed-loop system that runs: capture → analyze → plan → act → verify

use super::*;
use super::task_db::VlaTaskDb;
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
    /// SQLite task tracker for long-horizon context
    task_db: Option<VlaTaskDb>,
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
            timeout: Duration::from_secs(90),
        };

        // Open task DB - non-fatal if it fails
        let task_db = VlaTaskDb::open().ok();

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
            task_db,
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

        // Start task in DB
        let task_id = self.task_db.as_ref()
            .and_then(|db| db.start_task(&goal.objective, &goal.success_criteria).ok());

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
        let mut prev_screen_summary = String::new();

        // Main VLA loop
        // Architecture: CAPTURE → ANALYZE → PLAN (with context from DB) → ACT → VERIFY → RECORD
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

            // 1. CAPTURE
            let screenshot = self.vision
                .capture_screen_scaled(self.config.capture_width, self.config.capture_height)
                .map_err(|e| VlaError::VisionError(e.to_string()))?;

            if self.config.save_screenshots {
                self.save_screenshot(&screenshot.data, status.actions_taken).await;
            }

            // 2. ANALYZE - Get screen state
            let analysis = self.analyzer
                .analyze_image(&screenshot.data)
                .await
                .ok();

            let screen_summary = if let Some(ref a) = analysis {
                let summary = format!("App: {}, Title: {}, State: {:?}, Elements: {}, Text: {}",
                    a.app, a.title, a.state, a.elements.len(),
                    a.text.join("; "));
                status.current_state = summary.clone();
                summary
            } else {
                "Unknown".into()
            };

            // 3. CHECK - Are we done?
            if let Some(ref a) = analysis {
                let goal_achieved = self.check_success_criteria(&goal, a).await?;
                if goal_achieved {
                    status.success = true;
                    status.completed = true;
                    break;
                }
            }

            let dummy_analysis = crate::orchestrator::vision::ScreenAnalysis {
                app: "Unknown".into(),
                title: "".into(),
                elements: vec![],
                dialogs: vec![],
                text: vec![],
                state: crate::orchestrator::vision::ScreenState::Unknown,
                confidence: 0.0,
            };
            let analysis_ref = analysis.as_ref().unwrap_or(&dummy_analysis);

            // 4. GET DB CONTEXT - past actions and known failures for the planner
            let db_context = if let (Some(db), Some(ref tid)) = (&self.task_db, &task_id) {
                db.get_planner_context(tid, &analysis_ref.app).unwrap_or_default()
            } else {
                String::new()
            };

            // 5. PLAN - with image + DB context (past actions, known failures)
            let action = self.planner
                .plan_next_action(&goal, analysis_ref, &status.action_history, Some(&screenshot.data), &db_context)
                .await
                .map_err(|e| VlaError::PlanningError(e.to_string()))?;

            if action.is_none() {
                status.current_state = "Planner indicates goal achieved or no viable actions".into();
                status.completed = true;
                break;
            }

            let mut action = action.unwrap();

            // 6. SCALE coordinates
            if let Some(ref mut target) = action.target {
                let (screen_w, screen_h) = self.vision.get_screen_size().unwrap_or((1920, 1080));
                let scale_x = screen_w as f32 / self.config.capture_width as f32;
                let scale_y = screen_h as f32 / self.config.capture_height as f32;
                target.x = (target.x as f32 * scale_x) as i32;
                target.y = (target.y as f32 * scale_y) as i32;
            }

            // 7. RECORD action start in DB
            let db_action_id = if let (Some(db), Some(ref tid)) = (&self.task_db, &task_id) {
                db.record_action_start(
                    tid,
                    status.actions_taken,
                    &action.intent,
                    &format!("{:?}", action.action_type),
                    action.target.as_ref().map(|t| t.description.as_str()),
                    action.target.as_ref().map(|t| t.x),
                    action.target.as_ref().map(|t| t.y),
                    action.text.as_deref(),
                    action.keys.as_deref(),
                    action.confidence,
                    &action.expected_result,
                    &screen_summary,
                ).ok()
            } else {
                None
            };

            // 8. ACT
            let action_start = Instant::now();
            let exec_result = self.execute_action(&action).await;
            let action_duration = action_start.elapsed().as_millis() as u64;

            // 9. WAIT for UI
            tokio::time::sleep(self.config.action_delay).await;

            // 10. VERIFY - capture after state
            let verify_screenshot = self.vision
                .capture_screen_scaled(self.config.capture_width, self.config.capture_height)
                .ok();

            let verify_analysis = if let Some(ref vs) = verify_screenshot {
                self.analyzer.analyze_image(&vs.data).await.ok()
            } else {
                None
            };

            let after_summary = if let Some(ref a) = verify_analysis {
                format!("App: {}, Title: {}, State: {:?}", a.app, a.title, a.state)
            } else {
                "Unknown".into()
            };

            let screen_changed = after_summary != screen_summary;

            let expected_achieved = if let Some(ref a) = verify_analysis {
                self.verify_expected_result(&action, a).await
            } else {
                false
            };

            // 11. RECORD action result in DB
            if let (Some(db), Some(aid)) = (&self.task_db, db_action_id) {
                let _ = db.record_action_result(
                    aid,
                    exec_result.is_ok(),
                    exec_result.as_ref().err().map(|e| e.to_string()).as_deref(),
                    action_duration,
                    &after_summary,
                    screen_changed,
                    expected_achieved,
                );

                // 12. DETECT AND RECORD FAILURES
                if exec_result.is_ok() && !screen_changed {
                    let action_desc = format!("{:?} {}", action.action_type,
                        action.target.as_ref().map(|t| t.description.as_str()).unwrap_or(""));
                    if let Some(ref tid) = task_id {
                        let _ = db.record_failure(
                            Some(tid),
                            &screen_summary,
                            &action_desc,
                            "Screen did not change - action had no visible effect",
                            &format!("Try a different approach to: {}", action.intent),
                        );
                    }
                } else if exec_result.is_ok() && screen_changed && !expected_achieved {
                    // Screen changed but not to expected state - wrong result
                    let action_desc = format!("{:?} {}", action.action_type,
                        action.keys.as_deref().or(action.text.as_deref())
                            .unwrap_or(action.target.as_ref().map(|t| t.description.as_str()).unwrap_or("")));
                    if let Some(ref tid) = task_id {
                        let _ = db.record_failure(
                            Some(tid),
                            &screen_summary,
                            &action_desc,
                            &format!("Got: {} (not expected: {})", after_summary, action.expected_result),
                            &format!("Avoid this approach when in: {}", screen_summary),
                        );
                    }
                }
            }

            // Record in status history
            let result = ActionResult {
                action: action.clone(),
                success: exec_result.is_ok(),
                error: exec_result.err().map(|e| e.to_string()),
                screen_state: Some(after_summary.clone()),
                expected_achieved,
                duration_ms: action_duration,
            };

            status.action_history.push(result);
            status.actions_taken += 1;
            prev_screen_summary = after_summary;

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

        // End task in DB
        if let (Some(db), Some(ref tid)) = (&self.task_db, &task_id) {
            let db_status = if status.success { "success" }
                else if status.error.as_deref() == Some("Timeout exceeded") { "timeout" }
                else if status.error.as_deref() == Some("Stopped by user") { "stopped" }
                else { "failed" };
            let _ = db.end_task(tid, db_status, status.error.as_deref(), Some(&status.current_state));
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
                // Drag from drag_start to target (or just click-drag at target if no start)
                let (start_x, start_y) = if let Some(ref start) = action.drag_start {
                    (start.x, start.y)
                } else if let Some(ref target) = action.target {
                    // If no explicit start, start slightly to the left of target
                    (target.x - 50, target.y)
                } else {
                    return Ok(());
                };

                let (end_x, end_y) = if let Some(ref target) = action.target {
                    (target.x, target.y)
                } else {
                    return Ok(());
                };

                // Move to start position
                self.input
                    .mouse_move(start_x, start_y)
                    .map_err(|e| VlaError::InputError(e.to_string()))?;
                tokio::time::sleep(Duration::from_millis(50)).await;

                // Mouse button down
                self.input
                    .key_down("--mousebutton-left--")
                    .ok(); // Fallback below via xdotool

                // Use xdotool for reliable mouse drag on Linux
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdotool")
                        .arg("mousedown")
                        .arg("1")
                        .status();
                    tokio::time::sleep(Duration::from_millis(50)).await;

                    // Move in steps for smoother painting
                    let steps = 10;
                    for i in 1..=steps {
                        let frac = i as f64 / steps as f64;
                        let ix = start_x as f64 + (end_x as f64 - start_x as f64) * frac;
                        let iy = start_y as f64 + (end_y as f64 - start_y as f64) * frac;
                        let _ = std::process::Command::new("xdotool")
                            .arg("mousemove")
                            .arg(format!("{}", ix as i32))
                            .arg(format!("{}", iy as i32))
                            .status();
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }

                    let _ = std::process::Command::new("xdotool")
                        .arg("mouseup")
                        .arg("1")
                        .status();
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
        let path = format!("{}/action_{:03}.jpg", self.config.screenshot_dir, action_num);

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
