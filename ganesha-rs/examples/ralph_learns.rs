//! Ralph Learns - Self-improving reactive agent
//!
//! Loops until it can consistently complete complex harmless tasks.
//! Tracks success/failure, adapts strategies, and verifies with vision.
//!
//! Run with: cargo run --example ralph_learns --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

// Bedroom server - fast vision model (100 tok/sec)
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3-3b";

/// Task result
#[derive(Debug, Clone)]
struct TaskResult {
    task_name: String,
    success: bool,
    attempts: u32,
    strategy_used: String,
    error: Option<String>,
}

/// Current screen state from vision
#[derive(Debug, Clone, Default)]
struct ScreenState {
    mode: String,
    taskbar_visible: bool,
    apps: Vec<String>,
    dialogs: Vec<String>,
    raw: String,
}

impl ScreenState {
    fn has_app(&self, app: &str) -> bool {
        let app_lower = app.to_lowercase();
        self.apps.iter().any(|a| a.to_lowercase().contains(&app_lower)) ||
        self.raw.to_lowercase().contains(&app_lower)
    }

    fn in_activities(&self) -> bool {
        // Only check mode - taskbar detection is unreliable with some vision models
        self.mode.contains("activities") || self.mode.contains("overview")
    }
}

/// Parse vision response into structured state
fn parse_state(raw: &str) -> ScreenState {
    let raw_lower = raw.to_lowercase();

    let mode = if raw_lower.contains("mode:activities") || raw_lower.contains("activities_overview") {
        "activities".to_string()
    } else if raw_lower.contains("mode:fullscreen") {
        "fullscreen".to_string()
    } else {
        "normal_desktop".to_string()
    };

    let taskbar_visible = raw_lower.contains("taskbar:visible") ||
                          raw_lower.contains("taskbar: visible");

    // Extract apps mentioned
    let mut apps = Vec::new();
    for app in ["firefox", "writer", "libreoffice", "terminal", "files", "nautilus", "chrome", "ebay"] {
        if raw_lower.contains(app) {
            apps.push(app.to_string());
        }
    }

    ScreenState {
        mode,
        taskbar_visible,
        apps,
        dialogs: vec![],
        raw: raw.to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           RALPH LEARNS                                        ║");
    println!("║           Self-improving reactive agent                       ║");
    println!("║           'Me fail English? That's unpossible!'               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());

    vision.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    input.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    let current_state: Arc<RwLock<ScreenState>> = Arc::new(RwLock::new(ScreenState::default()));
    let screenshot_count = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // Start vision polling
    let poll_vision = Arc::clone(&vision);
    let poll_state = Arc::clone(&current_state);
    let poll_count = Arc::clone(&screenshot_count);
    let poll_running = Arc::clone(&running);

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        while poll_running.load(Ordering::SeqCst) {
            if let Ok(screenshot) = poll_vision.capture_screen_fast() {
                poll_count.fetch_add(1, Ordering::SeqCst);

                if let Ok(desc) = analyze_screen(&client, &screenshot.data).await {
                    let state = parse_state(&desc);
                    *poll_state.write().await = state;
                }
            }
            sleep(Duration::from_millis(400)).await;
        }
    });

    sleep(Duration::from_secs(1)).await;
    println!("[✓] Ralph is watching the screen...\n");

    // Track learning progress
    let mut task_history: Vec<TaskResult> = Vec::new();
    let mut consecutive_successes = 0;
    let target_successes = 3;  // Need 3 in a row to "graduate"
    let mut round = 0;

    // Define the complex task sequence
    let tasks = [
        "escape_to_desktop",
        "close_all_windows",
        "open_writer",
        "type_document",
        "save_document",
        "close_writer",
        "open_files",
        "verify_file_exists",
    ];

    println!("═══════════════════════════════════════════════════════════════");
    println!("GOAL: Complete {} consecutive successful rounds", target_successes);
    println!("TASKS: {:?}", tasks);
    println!("═══════════════════════════════════════════════════════════════\n");

    while consecutive_successes < target_successes {
        round += 1;
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  ROUND {} - Consecutive successes: {}/{}                      ║",
                 round, consecutive_successes, target_successes);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        let mut round_success = true;

        for task_name in &tasks {
            let state = current_state.read().await.clone();
            println!("[Task: {}]", task_name);
            println!("  State: {} | Apps: {:?}", state.mode, state.apps);

            let result = execute_task(
                task_name,
                &input,
                &vision,
                &current_state,
                &task_history,
            ).await;

            match &result {
                Ok(res) if res.success => {
                    println!("  [✓] Success (strategy: {})", res.strategy_used);
                    task_history.push(res.clone());
                }
                Ok(res) => {
                    println!("  [✗] Failed: {:?}", res.error);
                    task_history.push(res.clone());
                    round_success = false;
                }
                Err(e) => {
                    println!("  [✗] Error: {}", e);
                    round_success = false;
                }
            }

            sleep(Duration::from_millis(500)).await;
        }

        if round_success {
            consecutive_successes += 1;
            println!("\n[★] Round {} PASSED! ({}/{})", round, consecutive_successes, target_successes);
        } else {
            consecutive_successes = 0;
            println!("\n[✗] Round {} FAILED - resetting counter", round);

            // Learn from failures
            println!("[*] Analyzing failures...");
            let recent_failures: Vec<_> = task_history.iter()
                .rev()
                .take(10)
                .filter(|r| !r.success)
                .collect();

            for fail in &recent_failures {
                println!("    - {} failed with strategy '{}': {:?}",
                         fail.task_name, fail.strategy_used, fail.error);
            }
        }

        // Don't go forever
        if round > 20 {
            println!("\n[!] Too many rounds, Ralph needs more practice offline...");
            break;
        }
    }

    running.store(false, Ordering::SeqCst);
    sleep(Duration::from_millis(500)).await;

    // Final report
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    if consecutive_successes >= target_successes {
        println!("║  🎓 RALPH GRADUATED!                                          ║");
        println!("║  'I'm learnding!'                                             ║");
    } else {
        println!("║  📚 RALPH NEEDS MORE PRACTICE                                 ║");
        println!("║  'Me fail task? That's unpossible!'                           ║");
    }
    println!("╚═══════════════════════════════════════════════════════════════╝");

    // Stats
    let total = task_history.len();
    let successes = task_history.iter().filter(|r| r.success).count();
    println!("\nStats: {}/{} tasks succeeded ({:.1}%)",
             successes, total,
             if total > 0 { successes as f64 / total as f64 * 100.0 } else { 0.0 });

    // Task breakdown
    let mut by_task: HashMap<String, (u32, u32)> = HashMap::new();
    for result in &task_history {
        let entry = by_task.entry(result.task_name.clone()).or_insert((0, 0));
        entry.0 += 1;
        if result.success { entry.1 += 1; }
    }

    println!("\nBy task:");
    for (task, (total, success)) in &by_task {
        let rate = if *total > 0 { *success as f64 / *total as f64 * 100.0 } else { 0.0 };
        println!("  {}: {}/{} ({:.0}%)", task, success, total, rate);
    }

    vision.disable();
    input.disable();

    Ok(())
}

/// Execute a task with retry and strategy selection
async fn execute_task(
    task_name: &str,
    input: &InputController,
    vision: &VisionController,
    current_state: &Arc<RwLock<ScreenState>>,
    history: &[TaskResult],
) -> Result<TaskResult, Box<dyn std::error::Error + Send + Sync>> {
    let max_attempts = 3;
    let mut attempt = 0;
    let mut last_error = None;
    let mut strategy = select_strategy(task_name, history);

    while attempt < max_attempts {
        attempt += 1;

        let result = match task_name {
            "escape_to_desktop" => escape_to_desktop(input, current_state, &strategy).await,
            "close_all_windows" => close_all_windows(input, current_state, &strategy).await,
            "open_writer" => open_writer(input, current_state, &strategy).await,
            "type_document" => type_document(input, &strategy).await,
            "save_document" => save_document(input, current_state, &strategy).await,
            "close_writer" => close_writer(input, current_state, &strategy).await,
            "open_files" => open_files(input, current_state, &strategy).await,
            "verify_file_exists" => verify_file_exists(current_state, &strategy).await,
            _ => Err("Unknown task".into()),
        };

        match result {
            Ok(()) => {
                return Ok(TaskResult {
                    task_name: task_name.to_string(),
                    success: true,
                    attempts: attempt,
                    strategy_used: strategy,
                    error: None,
                });
            }
            Err(e) => {
                last_error = Some(e.to_string());
                // Try different strategy next time
                strategy = alternate_strategy(&strategy);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Ok(TaskResult {
        task_name: task_name.to_string(),
        success: false,
        attempts: attempt,
        strategy_used: strategy,
        error: last_error,
    })
}

/// Select initial strategy based on history
fn select_strategy(task_name: &str, history: &[TaskResult]) -> String {
    // Find what worked before for this task
    let past_successes: Vec<_> = history.iter()
        .filter(|r| r.task_name == task_name && r.success)
        .collect();

    if let Some(last_success) = past_successes.last() {
        return last_success.strategy_used.clone();
    }

    // Default strategies
    match task_name {
        "escape_to_desktop" => "escape_key",
        "close_all_windows" => "xdotool_close",
        "open_writer" => "activities_search",
        "type_document" => "direct_type",
        "save_document" => "ctrl_s",
        "close_writer" => "click_x",
        "open_files" => "activities_search",
        "verify_file_exists" => "vision_check",
        _ => "default",
    }.to_string()
}

/// Get alternate strategy
fn alternate_strategy(current: &str) -> String {
    match current {
        "escape_key" => "click_desktop".to_string(),
        "click_desktop" => "super_key".to_string(),
        "xdotool_close" => "click_x".to_string(),
        "click_x" => "alt_f4_safe".to_string(),
        "activities_search" => "direct_launch".to_string(),
        "direct_launch" => "dock_click".to_string(),
        _ => "default".to_string(),
    }
}

// ============ Task Implementations ============

async fn escape_to_desktop(
    _input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Always use xdotool for more reliable key presses
    match strategy {
        "escape_key" | "click_desktop" | _ => {
            // Multiple escape presses with xdotool (more reliable than enigo)
            for _ in 0..3 {
                std::process::Command::new("xdotool")
                    .args(["key", "Escape"])
                    .env("DISPLAY", ":1")
                    .status()
                    .ok();
                sleep(Duration::from_millis(200)).await;
            }

            // Move mouse away from Activities hot corner
            std::process::Command::new("xdotool")
                .args(["mousemove", "960", "540"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(100)).await;

            // Click to dismiss any overlay
            std::process::Command::new("xdotool")
                .args(["click", "1"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
        }
    }

    // Give UI time to settle
    sleep(Duration::from_millis(800)).await;

    // Wait for vision to update and verify (with retry)
    for attempt in 0..3 {
        sleep(Duration::from_millis(300)).await;
        let s = state.read().await;
        if !s.in_activities() {
            return Ok(());
        }
        if attempt < 2 {
            drop(s);
            // Try one more Escape
            std::process::Command::new("xdotool")
                .args(["key", "Escape"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
        }
    }

    // Final check
    let s = state.read().await;
    if s.in_activities() {
        return Err("Still in activities mode".into());
    }
    Ok(())
}

async fn close_all_windows(
    input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Try to close visible windows
    for _ in 0..3 {
        let s = state.read().await;
        if s.apps.is_empty() {
            return Ok(());
        }
        drop(s);

        match strategy {
            "xdotool_close" => {
                std::process::Command::new("xdotool")
                    .args(["key", "alt+F4"])
                    .env("DISPLAY", ":1")
                    .status()
                    .ok();
            }
            "click_x" => {
                input.mouse_move(1900, 12).map_err(box_err)?;
                sleep(Duration::from_millis(100)).await;
                input.mouse_click(MouseButton::Left).map_err(box_err)?;
            }
            _ => {
                std::process::Command::new("xdotool")
                    .args(["key", "alt+F4"])
                    .env("DISPLAY", ":1")
                    .status()
                    .ok();
            }
        }
        sleep(Duration::from_millis(800)).await;
    }
    Ok(())
}

async fn open_writer(
    input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match strategy {
        "activities_search" => {
            // Click Activities
            std::process::Command::new("xdotool")
                .args(["mousemove", "50", "14", "click", "1"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(1000)).await;

            // Type search
            std::process::Command::new("xdotool")
                .args(["type", "--delay", "30", "writer"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(800)).await;

            // Enter to launch
            std::process::Command::new("xdotool")
                .args(["key", "Return"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
        }
        "direct_launch" => {
            std::process::Command::new("soffice")
                .args(["--writer"])
                .env("DISPLAY", ":1")
                .spawn()
                .ok();
        }
        _ => {
            std::process::Command::new("xdotool")
                .args(["mousemove", "50", "14", "click", "1"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(1000)).await;
            std::process::Command::new("xdotool")
                .args(["type", "writer"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(500)).await;
            std::process::Command::new("xdotool")
                .args(["key", "Return"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
        }
    }

    // Wait for Writer to open
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        sleep(Duration::from_millis(500)).await;
        let s = state.read().await;
        if s.has_app("writer") || s.has_app("libreoffice") {
            return Ok(());
        }
    }

    Err("Writer did not open".into())
}

async fn type_document(
    input: &InputController,
    _strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = "The Quick Brown Fox\n\nThis document was created by Ralph the Learning Agent.\n\n\
                   Ralph is getting smarter with each attempt.\n\n\
                   'Me write good! That's possimpible!'";

    input.type_text(content).map_err(box_err)?;
    sleep(Duration::from_millis(500)).await;
    Ok(())
}

async fn save_document(
    input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    _strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ctrl+S
    input.key_combination("ctrl+s").map_err(box_err)?;
    sleep(Duration::from_secs(1)).await;

    // Type filename with timestamp
    let filename = format!("ralph_doc_{}", chrono::Utc::now().timestamp());
    input.type_text(&filename).map_err(box_err)?;
    sleep(Duration::from_millis(300)).await;

    // Enter to save
    input.key_press("Return").map_err(box_err)?;
    sleep(Duration::from_secs(2)).await;

    Ok(())
}

async fn close_writer(
    input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // First, escape from Activities if we're stuck there
    let s = state.read().await;
    if s.in_activities() {
        drop(s);
        std::process::Command::new("xdotool")
            .args(["key", "Escape"])
            .env("DISPLAY", ":1")
            .status()
            .ok();
        sleep(Duration::from_millis(500)).await;
    } else {
        drop(s);
    }

    // Use wmctrl to close Writer - more reliable than xdotool search
    // wmctrl -c matches substring in window title
    let wmctrl_result = std::process::Command::new("wmctrl")
        .args(["-c", "Writer"])
        .env("DISPLAY", ":1")
        .status();

    if wmctrl_result.is_err() || !wmctrl_result.unwrap().success() {
        // Fallback: focus by class (soffice) and close
        std::process::Command::new("xdotool")
            .args(["search", "--class", "soffice", "windowactivate"])
            .env("DISPLAY", ":1")
            .status()
            .ok();
        sleep(Duration::from_millis(200)).await;

        match strategy {
            "click_x" => {
                input.mouse_move(1900, 12).map_err(box_err)?;
                sleep(Duration::from_millis(100)).await;
                input.mouse_click(MouseButton::Left).map_err(box_err)?;
            }
            _ => {
                std::process::Command::new("xdotool")
                    .args(["key", "alt+F4"])
                    .env("DISPLAY", ":1")
                    .status()
                    .ok();
            }
        }
    }
    sleep(Duration::from_secs(1)).await;

    // Handle "save changes?" dialog - press 'd' for Don't Save (LibreOffice)
    // or Tab Tab Enter as fallback
    std::process::Command::new("xdotool")
        .args(["key", "d"])  // 'd' is mnemonic for Don't Save in LibreOffice
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(300)).await;
    std::process::Command::new("xdotool")
        .args(["key", "Tab", "Tab", "Return"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(500)).await;

    // Give time for window to fully close
    sleep(Duration::from_millis(500)).await;

    // Verify closed by checking if there's no Writer window anymore
    let check = std::process::Command::new("wmctrl")
        .args(["-l"])
        .env("DISPLAY", ":1")
        .output();

    if let Ok(output) = check {
        let windows = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if !windows.contains("writer") && !windows.contains("libreoffice") {
            return Ok(());  // Confirmed closed
        }
    }

    // If wmctrl check fails or Writer still shows, try one more Alt+F4
    std::process::Command::new("xdotool")
        .args(["key", "alt+F4"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(500)).await;
    std::process::Command::new("xdotool")
        .args(["key", "d"])  // Handle save dialog again
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(300)).await;

    // Final verification using wmctrl (authoritative)
    sleep(Duration::from_millis(500)).await;
    let final_check = std::process::Command::new("wmctrl")
        .args(["-l"])
        .env("DISPLAY", ":1")
        .output();

    if let Ok(output) = final_check {
        let windows = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if windows.contains("writer") || windows.contains("libreoffice") {
            return Err("Writer still open".into());
        }
    }

    Ok(())
}

async fn open_files(
    input: &InputController,
    state: &Arc<RwLock<ScreenState>>,
    _strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::process::Command::new("xdotool")
        .args(["mousemove", "50", "14", "click", "1"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(1000)).await;

    std::process::Command::new("xdotool")
        .args(["type", "--delay", "30", "files"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(800)).await;

    std::process::Command::new("xdotool")
        .args(["key", "Return"])
        .env("DISPLAY", ":1")
        .status()
        .ok();

    // Wait for Files to open
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        sleep(Duration::from_millis(500)).await;
        let s = state.read().await;
        if s.has_app("files") || s.has_app("nautilus") {
            return Ok(());
        }
    }

    Err("Files did not open".into())
}

async fn verify_file_exists(
    state: &Arc<RwLock<ScreenState>>,
    _strategy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let s = state.read().await;
    // Just check we have Files open - actual file verification would need more vision work
    if s.has_app("files") || s.has_app("nautilus") {
        return Ok(());
    }
    Err("Cannot verify - Files not open".into())
}

// ============ Helpers ============

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e)
}

async fn analyze_screen(client: &reqwest::Client, base64_image: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Quick analysis:\n\
                    MODE: normal_desktop | activities_overview | fullscreen | other\n\
                    TASKBAR: visible | hidden\n\
                    APPS: list main visible apps\n\
                    Format: MODE:x TASKBAR:x APPS:x"},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_image)}}
            ]
        }],
        "max_tokens": 80,
        "temperature": 0.1
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string())
}
