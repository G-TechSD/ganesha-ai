//! Robust Reactive Agent - Handles Activities/Mission Control properly
//!
//! Uses Super key instead of clicking, adds escape recovery
//!
//! Run with: cargo run --example reactive_robust --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

/// How often to take a high-res screenshot (every N low-res)
const HIGH_RES_INTERVAL: u64 = 10;

#[derive(Debug, Clone)]
struct ScreenState {
    description: String,
    screenshot_num: u64,
    is_high_res: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA REACTIVE AGENT (Robust)                     ║");
    println!("║           With Activities escape and verification             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());

    vision.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    input.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    // State tracking
    let current_state: Arc<RwLock<Option<ScreenState>>> = Arc::new(RwLock::new(None));
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
            let num = poll_count.fetch_add(1, Ordering::SeqCst) + 1;

            // Every HIGH_RES_INTERVAL frames, take a full-res screenshot
            let is_high_res = num % HIGH_RES_INTERVAL == 0;

            let screenshot_result = if is_high_res {
                poll_vision.capture_screen()  // Full 1920x1080
            } else {
                poll_vision.capture_screen_fast()  // 640x360
            };

            if let Ok(screenshot) = screenshot_result {
                let res_label = if is_high_res { "HIGH" } else { "low" };

                // Quick vision analysis
                let desc = quick_vision(&client, &screenshot.data, is_high_res).await
                    .unwrap_or_else(|_| "Analysis failed".into());

                // Log high-res captures
                if is_high_res {
                    println!("    [📸 HIGH-RES #{}] {}", num, &desc[..desc.len().min(50)]);
                }

                *poll_state.write().await = Some(ScreenState {
                    description: desc,
                    screenshot_num: num,
                    is_high_res,
                });
            }
            sleep(Duration::from_millis(300)).await;
        }
    });

    sleep(Duration::from_millis(800)).await;
    println!("[✓] Vision polling active\n");

    // Helper to get current state
    let get_state = || async {
        current_state.read().await.clone()
    };

    // Helper to check UI mode from vision response
    let is_in_activities = |desc: &str| -> bool {
        let d = desc.to_lowercase();
        d.contains("activities") || d.contains("overview") ||
        d.contains("mode:activities") || d.contains("mode: activities") ||
        d.contains("taskbar:hidden") || d.contains("taskbar: hidden") ||
        d.contains("app grid") || d.contains("workspace")
    };

    let is_normal_desktop = |desc: &str| -> bool {
        let d = desc.to_lowercase();
        (d.contains("mode:normal") || d.contains("mode: normal")) &&
        (d.contains("taskbar:visible") || d.contains("taskbar: visible"))
    };

    // STEP 0: Make sure we're not stuck in Activities
    println!("[Step 0] Ensuring clean desktop state...");
    if let Some(state) = get_state().await {
        println!("  Current: {}", &state.description[..state.description.len().min(60)]);
        if is_in_activities(&state.description) {
            println!("  [!] Stuck in Activities - pressing Escape...");
            input.key_press("Escape").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            sleep(Duration::from_millis(500)).await;
        }
    }

    // STEP 1: Close any Firefox/browser windows
    println!("\n[Step 1] Closing browser windows...");
    // First, let's see what's open
    sleep(Duration::from_millis(500)).await;
    if let Some(state) = get_state().await {
        println!("  Visible: {}", &state.description[..state.description.len().min(60)]);

        if state.description.to_lowercase().contains("firefox") ||
           state.description.to_lowercase().contains("ebay") ||
           state.description.to_lowercase().contains("browser") {
            // Click the close button area - but first we need to focus the window
            // Try clicking center of screen to focus, then close
            println!("  [*] Clicking window to focus...");
            input.mouse_move(960, 540).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            input.mouse_click(MouseButton::Left).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            sleep(Duration::from_millis(300)).await;

            println!("  [*] Clicking close button (top-right)...");
            input.mouse_move(1895, 12).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            sleep(Duration::from_millis(100)).await;
            input.mouse_click(MouseButton::Left).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            sleep(Duration::from_millis(800)).await;
        }
    }

    // Verify browser closed
    if let Some(state) = get_state().await {
        println!("  After: {}", &state.description[..state.description.len().min(60)]);
    }

    // STEP 2: Open LibreOffice Writer
    println!("\n[Step 2] Opening LibreOffice Writer...");

    // First escape any existing state and click desktop to clear focus
    println!("  [*] Clearing focus (Escape + click desktop)...");
    input.key_press("Escape").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_millis(200)).await;

    // Use xdotool for Activities - more reliable on Linux
    println!("  [*] Clicking Activities (using xdotool for reliability)...");
    std::process::Command::new("xdotool")
        .args(["mousemove", "50", "14"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(100)).await;
    std::process::Command::new("xdotool")
        .args(["click", "1"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(1200)).await;

    // Verify we're in Activities
    if let Some(state) = get_state().await {
        println!("  State: {}", &state.description[..state.description.len().min(60)]);

        if is_in_activities(&state.description) {
            println!("  [✓] Activities mode detected!");
        } else {
            println!("  [!] Not in Activities - trying again...");
            std::process::Command::new("xdotool")
                .args(["mousemove", "50", "14", "click", "1"])
                .env("DISPLAY", ":1")
                .status()
                .ok();
            sleep(Duration::from_millis(1000)).await;
        }
    }

    // Type to search (use xdotool for Activities search)
    println!("  [*] Typing 'writer'...");
    std::process::Command::new("xdotool")
        .args(["type", "--delay", "50", "writer"])
        .env("DISPLAY", ":1")
        .status()
        .ok();
    sleep(Duration::from_millis(1000)).await;

    // Check search results
    if let Some(state) = get_state().await {
        println!("  Search: {}", &state.description[..state.description.len().min(60)]);
    }

    // Press Enter to launch
    println!("  [*] Pressing Enter...");
    std::process::Command::new("xdotool")
        .args(["key", "Return"])
        .env("DISPLAY", ":1")
        .status()
        .ok();

    // Wait for Writer to open - poll vision for confirmation
    println!("  [*] Waiting for Writer to appear...");
    let start = Instant::now();
    let mut writer_opened = false;
    while start.elapsed() < Duration::from_secs(10) {
        sleep(Duration::from_millis(500)).await;
        if let Some(state) = get_state().await {
            let desc = state.description.to_lowercase();
            if desc.contains("writer") || desc.contains("libreoffice") || desc.contains("document") {
                println!("  [✓] Writer detected: {}", &state.description[..state.description.len().min(50)]);
                writer_opened = true;
                break;
            }
            // If stuck in Activities, escape
            if is_in_activities(&state.description) && start.elapsed() > Duration::from_secs(3) {
                println!("  [!] Still in Activities - pressing Escape...");
                input.key_press("Escape").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            }
        }
    }

    if !writer_opened {
        println!("  [!] Writer may not have opened - continuing anyway");
        if let Some(state) = get_state().await {
            println!("  Current state: {}", state.description);
        }
    }

    // Extra wait for Writer to fully load
    sleep(Duration::from_secs(2)).await;

    // STEP 3: Type the document
    println!("\n[Step 3] Writing about cats...");

    let content = [
        ("Title", "The Wonderful World of Cats\n\n"),
        ("Intro", "Cats have been human companions for thousands of years. "),
        ("History", "Ancient Egyptians revered cats as sacred animals. "),
        ("Breeds", "Today there are over 70 recognized cat breeds. "),
        ("Behavior", "Cats communicate through meows, purrs, and body language. "),
        ("Care", "Proper nutrition and regular vet visits keep cats healthy.\n\n"),
        ("Conclusion", "Cats bring joy and companionship to millions of homes worldwide."),
    ];

    for (label, text) in content.iter() {
        print!("  [*] {}... ", label);
        input.type_text(text).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        sleep(Duration::from_millis(200)).await;
        println!("✓ (screenshot #{})", screenshot_count.load(Ordering::SeqCst));
    }

    // STEP 4: Save
    println!("\n[Step 4] Saving document...");
    input.key_combination("ctrl+s").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_secs(1)).await;

    if let Some(state) = get_state().await {
        println!("  Dialog: {}", &state.description[..state.description.len().min(60)]);
    }

    input.type_text("cats_document").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_millis(300)).await;
    input.key_press("Return").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_secs(1)).await;

    // Stop polling
    running.store(false, Ordering::SeqCst);
    sleep(Duration::from_millis(500)).await;

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("COMPLETE - {} screenshots analyzed", screenshot_count.load(Ordering::SeqCst));
    println!("═══════════════════════════════════════════════════════════════");

    if let Some(state) = get_state().await {
        println!("Final: {}", state.description);
    }

    vision.disable();
    input.disable();

    Ok(())
}

async fn quick_vision(client: &reqwest::Client, base64_image: &str, is_high_res: bool) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Different prompts for low-res (quick) vs high-res (detailed)
    let prompt = if is_high_res {
        "Analyze this HIGH RESOLUTION screenshot in detail:\n\
         1. MODE: normal_desktop | activities_overview | fullscreen | login_screen | other\n\
         2. TASKBAR: visible | hidden\n\
         3. APPS: list ALL visible windows with positions (left/center/right)\n\
         4. DIALOGS: any popups/dialogs? describe them\n\
         5. MOUSE: where is the mouse cursor? (approximate x,y)\n\
         6. FOCUS: which window has focus?\n\
         Format: MODE:x TASKBAR:x APPS:x DIALOGS:x MOUSE:x FOCUS:x"
    } else {
        "Quick analysis:\n\
         MODE: normal_desktop | activities_overview | fullscreen | login_screen | other\n\
         TASKBAR: visible | hidden\n\
         APPS: main visible apps\n\
         DIALOGS: any popups?\n\
         Format: MODE:x TASKBAR:x APPS:x DIALOGS:x"
    };

    let max_tokens = if is_high_res { 200 } else { 80 };

    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_image)}}
            ]
        }],
        "max_tokens": max_tokens,
        "temperature": 0.1
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string())
}

/// Request specific coordinates from a high-res screenshot
async fn find_coordinates(
    client: &reqwest::Client,
    vision: &VisionController,
    target: &str
) -> Result<Option<(i32, i32)>, Box<dyn std::error::Error + Send + Sync>> {
    // Capture high-res for precise coordinate detection
    let screenshot = vision.capture_screen()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    let prompt = format!(
        "Find the {} on this screenshot. \
         The screen is 1920x1080 pixels. \
         Give the EXACT center coordinates as: COORDS:x,y \
         If not found, say: COORDS:none",
        target
    );

    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", screenshot.data)}}
            ]
        }],
        "max_tokens": 50,
        "temperature": 0.1
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    // Parse COORDS:x,y
    if let Some(coords_start) = content.find("COORDS:") {
        let coords_str = &content[coords_start + 7..];
        if coords_str.starts_with("none") {
            return Ok(None);
        }
        // Parse x,y
        let parts: Vec<&str> = coords_str.split(',').collect();
        if parts.len() >= 2 {
            let x: i32 = parts[0].trim().parse().unwrap_or(0);
            let y: i32 = parts[1].split_whitespace().next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if x > 0 && y > 0 && x < 1920 && y < 1080 {
                return Ok(Some((x, y)));
            }
        }
    }

    Ok(None)
}
