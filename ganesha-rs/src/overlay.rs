//! AI Activity Overlay - Visible timer and status for human + AI awareness
//!
//! Displays on screen:
//! - Time since last AI action
//! - Current action/status
//! - Goal progress
//!
//! Both human observer and vision model can see this, creating shared awareness.

use std::process::{Command, Child};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::thread;

/// Overlay position on screen
#[derive(Debug, Clone, Copy)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Overlay state
#[derive(Debug, Clone)]
pub struct OverlayState {
    /// Time of last action
    pub last_action_time: Instant,
    /// Current action being performed
    pub current_action: String,
    /// Current goal
    pub goal: String,
    /// Progress 0-100
    pub progress: u8,
    /// Status: "working", "waiting", "stuck", "done"
    pub status: String,
    /// Whether AI is in control
    pub ai_in_control: bool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            last_action_time: Instant::now(),
            current_action: "Idle".into(),
            goal: String::new(),
            progress: 0,
            status: "waiting".into(),
            ai_in_control: false,
        }
    }
}

/// AI Activity Overlay using yad/zenity or native X11
pub struct ActivityOverlay {
    state: Arc<RwLock<OverlayState>>,
    position: OverlayPosition,
    process: Option<Child>,
    update_thread: Option<thread::JoinHandle<()>>,
    running: Arc<RwLock<bool>>,
}

impl ActivityOverlay {
    pub fn new(position: OverlayPosition) -> Self {
        Self {
            state: Arc::new(RwLock::new(OverlayState::default())),
            position,
            process: None,
            update_thread: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the overlay display
    #[cfg(target_os = "linux")]
    pub fn start(&mut self) -> Result<(), String> {
        // Check if yad is available (more flexible than zenity)
        let has_yad = Command::new("which")
            .arg("yad")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_yad {
            return Err("yad not found. Install with: sudo apt install yad".into());
        }

        *self.running.write().unwrap() = true;

        // Start the overlay process
        let state = self.state.clone();
        let running = self.running.clone();
        let position = self.position;

        self.update_thread = Some(thread::spawn(move || {
            Self::overlay_loop(state, running, position);
        }));

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn overlay_loop(
        state: Arc<RwLock<OverlayState>>,
        running: Arc<RwLock<bool>>,
        position: OverlayPosition,
    ) {
        // Position coordinates
        let (x, y) = match position {
            OverlayPosition::TopLeft => (10, 40),
            OverlayPosition::TopRight => (1600, 40),
            OverlayPosition::BottomLeft => (10, 1000),
            OverlayPosition::BottomRight => (1600, 1000),
        };

        let mut last_text = String::new();
        let mut process: Option<Child> = None;

        while *running.read().unwrap() {
            let state = state.read().unwrap();
            let elapsed = state.last_action_time.elapsed();

            // Format elapsed time
            let elapsed_str = if elapsed.as_secs() < 60 {
                format!("{}s", elapsed.as_secs())
            } else {
                format!("{}m {}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
            };

            // Color based on elapsed time
            let color = if elapsed.as_secs() < 10 {
                "#00ff00" // Green - recent activity
            } else if elapsed.as_secs() < 30 {
                "#ffff00" // Yellow - getting stale
            } else {
                "#ff0000" // Red - possibly stuck
            };

            // Status icon
            let icon = match state.status.as_str() {
                "working" => "🔄",
                "waiting" => "⏳",
                "stuck" => "🔴",
                "done" => "✅",
                _ => "🤖",
            };

            // Build display text
            let text = format!(
                "{} {} | {} ago\n{}\nProgress: {}%",
                icon,
                state.status.to_uppercase(),
                elapsed_str,
                if state.current_action.len() > 30 {
                    format!("{}...", &state.current_action[..30])
                } else {
                    state.current_action.clone()
                },
                state.progress
            );

            drop(state);

            // Only update if text changed (reduces flicker)
            if text != last_text {
                // Kill old process
                if let Some(mut p) = process.take() {
                    let _ = p.kill();
                }

                // Launch new overlay
                process = Command::new("yad")
                    .args([
                        "--text", &text,
                        "--no-buttons",
                        "--undecorated",
                        "--on-top",
                        "--skip-taskbar",
                        "--sticky",
                        "--geometry", &format!("250x80+{}+{}", x, y),
                        "--text-align", "center",
                        "--fore", color,
                        "--back", "#1a1a1a",
                        "--timeout", "10",
                    ])
                    .env("DISPLAY", std::env::var("DISPLAY").unwrap_or(":1".into()))
                    .spawn()
                    .ok();

                last_text = text;
            }

            thread::sleep(Duration::from_millis(500));
        }

        // Cleanup
        if let Some(mut p) = process {
            let _ = p.kill();
        }
    }

    /// Update the overlay state
    pub fn update(&self, action: &str, status: &str, progress: u8) {
        let mut state = self.state.write().unwrap();
        state.current_action = action.to_string();
        state.status = status.to_string();
        state.progress = progress;
    }

    /// Mark an action as just completed (resets timer)
    pub fn action_completed(&self, action: &str) {
        let mut state = self.state.write().unwrap();
        state.last_action_time = Instant::now();
        state.current_action = action.to_string();
        state.status = "working".into();
    }

    /// Set the current goal
    pub fn set_goal(&self, goal: &str) {
        let mut state = self.state.write().unwrap();
        state.goal = goal.to_string();
    }

    /// Set AI control status
    pub fn set_ai_control(&self, in_control: bool) {
        let mut state = self.state.write().unwrap();
        state.ai_in_control = in_control;
    }

    /// Get time since last action
    pub fn time_since_action(&self) -> Duration {
        self.state.read().unwrap().last_action_time.elapsed()
    }

    /// Check if possibly stuck (no action for too long)
    pub fn is_possibly_stuck(&self, threshold: Duration) -> bool {
        self.time_since_action() > threshold
    }

    /// Stop the overlay
    pub fn stop(&mut self) {
        *self.running.write().unwrap() = false;
        if let Some(handle) = self.update_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ActivityOverlay {
    fn drop(&mut self) {
        self.stop();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SIMPLER OVERLAY: Just update a small window periodically
// ═══════════════════════════════════════════════════════════════════════════════

/// Minimal overlay using notify-send (works without yad)
pub struct NotifyOverlay {
    state: Arc<RwLock<OverlayState>>,
}

impl NotifyOverlay {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(OverlayState::default())),
        }
    }

    /// Show current status via notification
    pub fn show_status(&self) {
        let state = self.state.read().unwrap();
        let elapsed = state.last_action_time.elapsed();

        let urgency = if elapsed.as_secs() < 10 {
            "low"
        } else if elapsed.as_secs() < 30 {
            "normal"
        } else {
            "critical"
        };

        let _ = Command::new("notify-send")
            .args([
                "-u", urgency,
                "-t", "3000",
                "-h", "string:x-canonical-private-synchronous:ganesha",
                "Ganesha AI",
                &format!(
                    "{} | {} ago\n{}",
                    state.status.to_uppercase(),
                    elapsed.as_secs(),
                    state.current_action
                ),
            ])
            .spawn();
    }

    /// Update state
    pub fn update(&self, action: &str, status: &str) {
        let mut state = self.state.write().unwrap();
        state.current_action = action.to_string();
        state.status = status.to_string();
    }

    /// Reset timer
    pub fn action_completed(&self) {
        let mut state = self.state.write().unwrap();
        state.last_action_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires display
    fn test_overlay() {
        let mut overlay = ActivityOverlay::new(OverlayPosition::TopRight);
        overlay.start().unwrap();
        overlay.set_goal("Search eBay for vintage synth");
        overlay.action_completed("SEARCH_EBAY");
        overlay.update("Searching eBay", "working", 25);

        thread::sleep(Duration::from_secs(5));
        overlay.stop();
    }
}
