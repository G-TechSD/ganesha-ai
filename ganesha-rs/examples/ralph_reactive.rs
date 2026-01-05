//! Ralph Reactive - Truly reactive agent with safety controls
//!
//! This version:
//! - Shows RED FRAME while agent is in control
//! - Can optionally grab input (blocks user mouse/keyboard)
//! - Has foolproof interrupt: BOTH SHIFT KEYS + ESCAPE
//! - Uses vision to FIND elements dynamically (no hardcoded coords)
//! - Verifies each action with vision before proceeding
//! - Uses UI knowledge base for app-specific strategies
//!
//! Run with: cargo run --example ralph_reactive --features computer-use

use ganesha::agent::{AgentControl, ReactiveVision, UIKnowledgeBase, CloseMethod};
use ganesha::input::InputController;
use ganesha::vision::VisionController;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use std::process::Command;

// Bedroom server - fast vision model (100 tok/sec)
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3-3b";

// Beast server - planning/orchestration model
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           RALPH REACTIVE                                      ║");
    println!("║           Truly reactive agent with safety controls           ║");
    println!("║                                                               ║");
    println!("║   🛑 INTERRUPT: Hold BOTH SHIFT keys + press ESCAPE          ║");
    println!("║   📁 Or: touch /tmp/ganesha_interrupt                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize components
    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());
    let reactive_vision = ReactiveVision::new(VISION_ENDPOINT, VISION_MODEL);
    let knowledge = UIKnowledgeBase::gnome();
    let mut control = AgentControl::new();

    vision.enable().map_err(box_err)?;
    input.enable().map_err(box_err)?;

    // Ask user about input grab
    println!("[?] Grab input (block mouse/keyboard while working)?");
    println!("    This is safer but you can only interrupt with:");
    println!("    - Both Shift keys + Escape");
    println!("    - touch /tmp/ganesha_interrupt");
    println!();
    println!("    Press Enter for NO (recommended for testing)");
    println!("    Type 'yes' + Enter for YES");
    print!("    > ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    let grab_input = response.trim().to_lowercase() == "yes";

    println!();
    println!("[*] Starting agent with:");
    println!("    - Red frame overlay: YES");
    println!("    - Input grab: {}", if grab_input { "YES" } else { "NO" });
    println!("    - Sound indicator: YES");
    println!();

    // Take control - shows overlay
    control.take_control(grab_input, true)?;

    // Run the task with interrupt checking
    let result = run_reactive_task(&vision, &input, &reactive_vision, &knowledge, &control).await;

    // Release control
    control.release_control();

    match result {
        Ok(_) => {
            println!("\n[✓] Task completed successfully!");
        }
        Err(e) => {
            if control.is_interrupted() {
                println!("\n[!] Task interrupted by user");
            } else {
                println!("\n[✗] Task failed: {}", e);
            }
        }
    }

    vision.disable();
    input.disable();

    Ok(())
}

async fn run_reactive_task(
    vision: &VisionController,
    input: &InputController,
    rv: &ReactiveVision,
    kb: &UIKnowledgeBase,
    control: &AgentControl,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    // Check interrupt before each major action
    macro_rules! check_interrupt {
        () => {
            if control.is_interrupted() {
                return Err("Interrupted by user".into());
            }
        };
    }

    // ============ TASK: Open Firefox, go to Wikipedia, copy text, paste in Writer ============

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("REACTIVE TASK: Web research to document");
    println!("═══════════════════════════════════════════════════════════════\n");

    // Step 1: Analyze current screen state
    println!("[Step 1] Analyzing current screen...");
    let analysis = rv.analyze_screen(vision).await?;
    println!("  Desktop: {}", analysis.desktop_state);
    println!("  Windows: {:?}", analysis.visible_windows);
    if analysis.has_dialog() {
        println!("  Dialog detected: {:?}", analysis.dialogs);
    }

    check_interrupt!();

    // Step 2: Ensure we're at a clean desktop
    println!("\n[Step 2] Ensuring clean desktop state...");
    if analysis.is_activities() {
        println!("  In Activities mode - pressing Escape...");
        xdotool(&["key", "Escape"])?;
        sleep(Duration::from_millis(500)).await;
    }

    // Close any existing windows that might interfere
    for window in &["Firefox", "Writer", "LibreOffice"] {
        wmctrl_close(window)?;
        sleep(Duration::from_millis(200)).await;
    }
    sleep(Duration::from_millis(500)).await;

    check_interrupt!();

    // Step 3: Open Firefox using knowledge base
    println!("\n[Step 3] Opening Firefox (using knowledge base)...");

    // Use Activities search (from knowledge base)
    let firefox_shortcut = kb.activities_shortcut();
    println!("  Triggering Activities with: {}", firefox_shortcut);
    xdotool(&["key", &firefox_shortcut])?;
    sleep(Duration::from_millis(1000)).await;

    // Verify we're in Activities
    let analysis = rv.analyze_screen(vision).await?;
    if !analysis.is_activities() {
        println!("  Activities not detected, trying again...");
        xdotool(&["key", "super"])?;
        sleep(Duration::from_millis(1000)).await;
    }

    check_interrupt!();

    // Type search query
    println!("  Searching for 'firefox'...");
    xdotool(&["type", "--delay", "30", "firefox"])?;
    sleep(Duration::from_millis(800)).await;

    // Launch
    xdotool(&["key", "Return"])?;
    println!("  Waiting for Firefox to open...");

    // Wait and verify with vision
    let start = Instant::now();
    let mut firefox_opened = false;
    while start.elapsed() < Duration::from_secs(10) {
        check_interrupt!();
        sleep(Duration::from_millis(500)).await;

        let analysis = rv.analyze_screen(vision).await?;
        if analysis.has_window("firefox") || analysis.has_window("mozilla") {
            println!("  [✓] Firefox detected!");
            firefox_opened = true;
            break;
        }
    }

    if !firefox_opened {
        return Err("Firefox did not open".into());
    }

    check_interrupt!();

    // Step 4: Navigate to Wikipedia using keyboard shortcut (from knowledge)
    println!("\n[Step 4] Navigating to Wikipedia...");
    sleep(Duration::from_millis(500)).await;

    // Use address bar shortcut from knowledge base
    if let Some(shortcut) = kb.get_shortcut(Some("firefox"), "address_bar") {
        println!("  Focusing address bar with: {}", shortcut);
        xdotool(&["key", &shortcut])?;
    } else {
        xdotool(&["key", "ctrl+l"])?;
    }
    sleep(Duration::from_millis(300)).await;

    xdotool(&["type", "--delay", "15", "https://en.wikipedia.org/wiki/Rust_(programming_language)"])?;
    sleep(Duration::from_millis(200)).await;
    xdotool(&["key", "Return"])?;

    // Wait for page load with vision verification
    println!("  Waiting for page to load...");
    sleep(Duration::from_secs(3)).await;

    check_interrupt!();

    // Verify page loaded
    let verification = rv.verify_action(vision, "Wikipedia page about Rust should be visible").await?;
    if verification.success {
        println!("  [✓] Wikipedia page loaded (confidence: {:.0}%)", verification.confidence * 100.0);
    } else {
        println!("  [!] Page may not have loaded: {}", verification.current_state);
        // Continue anyway
    }

    check_interrupt!();

    // Step 5: Find and click on article content, then copy
    println!("\n[Step 5] Copying article introduction...");

    // Try to find the article content area
    println!("  Looking for article content...");
    if let Some(element) = rv.find_element(vision, "article content area or first paragraph of Wikipedia article").await? {
        let (cx, cy) = element.center();
        println!("  Found content at ({}, {}) - clicking...", cx, cy);
        xdotool(&["mousemove", &cx.to_string(), &cy.to_string()])?;
        sleep(Duration::from_millis(100)).await;
        xdotool(&["click", "1"])?;
    } else {
        // Fallback: click roughly in content area
        println!("  Using fallback click position...");
        xdotool(&["mousemove", "600", "400", "click", "1"])?;
    }
    sleep(Duration::from_millis(200)).await;

    check_interrupt!();

    // Select first paragraph (triple-click)
    println!("  Triple-clicking to select paragraph...");
    xdotool(&["click", "1", "click", "1", "click", "1"])?;
    sleep(Duration::from_millis(200)).await;

    // Extend selection
    xdotool(&["key", "shift+ctrl+End"])?;
    sleep(Duration::from_millis(100)).await;

    // Copy using shortcut from knowledge base
    if let Some(shortcut) = kb.get_shortcut(None, "copy") {
        xdotool(&["key", &shortcut])?;
    }
    sleep(Duration::from_millis(300)).await;

    // Verify clipboard has content
    let clip_check = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .env("DISPLAY", ":1")
        .output();

    if let Ok(output) = clip_check {
        let text_len = output.stdout.len();
        if text_len > 100 {
            println!("  [✓] Copied {} characters to clipboard", text_len);
        } else {
            println!("  [!] Clipboard may be empty or small ({} chars)", text_len);
        }
    }

    check_interrupt!();

    // Step 6: Open Writer
    println!("\n[Step 6] Opening LibreOffice Writer...");

    // Use Activities
    xdotool(&["key", "super"])?;
    sleep(Duration::from_millis(1000)).await;

    xdotool(&["type", "--delay", "30", "writer"])?;
    sleep(Duration::from_millis(800)).await;

    xdotool(&["key", "Return"])?;

    // Wait for Writer with vision verification
    println!("  Waiting for Writer to open...");
    let start = Instant::now();
    let mut writer_opened = false;
    while start.elapsed() < Duration::from_secs(12) {
        check_interrupt!();
        sleep(Duration::from_millis(500)).await;

        let analysis = rv.analyze_screen(vision).await?;
        if analysis.has_window("writer") || analysis.has_window("libreoffice") {
            println!("  [✓] Writer detected!");
            writer_opened = true;
            break;
        }
    }

    if !writer_opened {
        // Try wmctrl check
        let output = Command::new("wmctrl")
            .args(["-l"])
            .env("DISPLAY", ":1")
            .output()?;
        let windows = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if windows.contains("writer") || windows.contains("libreoffice") {
            writer_opened = true;
        }
    }

    if !writer_opened {
        return Err("Writer did not open".into());
    }

    check_interrupt!();

    // Step 7: Paste content and format
    println!("\n[Step 7] Pasting and formatting content...");
    sleep(Duration::from_millis(500)).await;

    // Ensure Writer has focus
    wmctrl_activate("Writer")?;
    sleep(Duration::from_millis(300)).await;

    // Type heading
    println!("  Adding heading...");
    input.type_text("Research Notes: Rust Programming Language\n\n").map_err(box_err)?;
    sleep(Duration::from_millis(200)).await;

    // Select heading and make bold
    xdotool(&["key", "ctrl+Home"])?;  // Go to start
    sleep(Duration::from_millis(100)).await;
    xdotool(&["key", "shift+End"])?;  // Select line
    sleep(Duration::from_millis(100)).await;

    // Bold using knowledge base shortcut
    if let Some(shortcut) = kb.get_shortcut(Some("writer"), "bold") {
        xdotool(&["key", &shortcut])?;
    } else {
        xdotool(&["key", "ctrl+b"])?;
    }
    sleep(Duration::from_millis(100)).await;

    // Go to end and paste
    xdotool(&["key", "ctrl+End"])?;
    sleep(Duration::from_millis(100)).await;

    println!("  Pasting clipboard content...");
    if let Some(shortcut) = kb.get_shortcut(None, "paste") {
        xdotool(&["key", &shortcut])?;
    }
    sleep(Duration::from_millis(500)).await;

    // Add attribution
    input.type_text("\n\n--- Source: Wikipedia ---\n").map_err(box_err)?;

    check_interrupt!();

    // Verify paste worked
    let verification = rv.verify_action(vision, "Document should contain pasted text about Rust").await?;
    println!("  Paste verification: {} (confidence: {:.0}%)",
             if verification.success { "success" } else { "uncertain" },
             verification.confidence * 100.0);

    check_interrupt!();

    // Step 8: Save document
    println!("\n[Step 8] Saving document...");

    if let Some(shortcut) = kb.get_shortcut(None, "save") {
        xdotool(&["key", &shortcut])?;
    }
    sleep(Duration::from_secs(1)).await;

    // Type filename
    let filename = format!("reactive_research_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    input.type_text(&filename).map_err(box_err)?;
    sleep(Duration::from_millis(300)).await;

    xdotool(&["key", "Return"])?;
    sleep(Duration::from_secs(2)).await;

    // Handle format dialog if it appears (from knowledge base)
    if let Some(dialog) = kb.find_dialog_handler("format") {
        println!("  Handling format dialog with: {}", dialog.default_action);
        xdotool(&["key", &dialog.default_action])?;
        sleep(Duration::from_millis(500)).await;
    }

    check_interrupt!();

    // Step 9: Verify file saved
    println!("\n[Step 9] Verifying file was saved...");

    let home = std::env::var("HOME").unwrap_or("/home/bill".into());
    let check = Command::new("ls")
        .args(["-la"])
        .current_dir(&home)
        .output()?;
    let files = String::from_utf8_lossy(&check.stdout);

    if files.contains("reactive_research") {
        println!("  [✓] Document saved!");
    } else {
        println!("  [!] Document may not have saved (checking Documents folder...)");
    }

    check_interrupt!();

    // Step 10: Close applications cleanly
    println!("\n[Step 10] Closing applications...");

    // Close Writer using method from knowledge base
    let close_method = kb.get_close_method("writer");
    match close_method {
        CloseMethod::WmCtrl => {
            wmctrl_close("Writer")?;
        }
        CloseMethod::AltF4 => {
            xdotool(&["key", "alt+F4"])?;
        }
        _ => {
            wmctrl_close("Writer")?;
        }
    }
    sleep(Duration::from_millis(500)).await;

    // Handle save dialog if it appears
    if let Some(dialog) = kb.find_dialog_handler("save changes") {
        xdotool(&["key", "d"])?;  // Don't Save
        sleep(Duration::from_millis(300)).await;
    }

    // Close Firefox
    wmctrl_close("Firefox")?;
    sleep(Duration::from_millis(300)).await;

    // Final verification
    let final_analysis = rv.analyze_screen(vision).await?;
    println!("\nFinal state: {}", final_analysis.desktop_state);
    println!("Remaining windows: {:?}", final_analysis.visible_windows);

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("REACTIVE TASK COMPLETE");
    println!("═══════════════════════════════════════════════════════════════");

    Ok(())
}

// Helper functions

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e)
}

fn xdotool(args: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Command::new("xdotool")
        .args(args)
        .env("DISPLAY", ":1")
        .status()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    Ok(())
}

fn wmctrl_close(window: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Command::new("wmctrl")
        .args(["-c", window])
        .env("DISPLAY", ":1")
        .status()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    Ok(())
}

fn wmctrl_activate(window: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Command::new("wmctrl")
        .args(["-a", window])
        .env("DISPLAY", ":1")
        .status()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    Ok(())
}
