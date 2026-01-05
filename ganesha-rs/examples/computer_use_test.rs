//! Computer Use Test - Vision + Mouse + Keyboard Only
//!
//! This test demonstrates Ganesha's computer-use capabilities:
//! - Takes screenshots and analyzes them with vision model
//! - Uses mouse clicks to interact with UI
//! - Uses keyboard to type text
//! - NO shell commands - pure GUI interaction
//!
//! Run with: cargo run --example computer_use_test --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::time::Duration;
use tokio::time::sleep;

const BEDROOM_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA COMPUTER USE TEST                           ║");
    println!("║           Vision + Mouse + Keyboard Only                      ║");
    println!("║           Task: Open Firefox -> Navigate to ebay.com          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize controllers
    let vision = VisionController::new();
    let input = InputController::new();

    // Enable both modules
    println!("[*] Enabling vision module...");
    vision.enable()?;
    println!("[✓] Vision enabled");

    println!("[*] Enabling input module...");
    input.enable()?;
    println!("[✓] Input enabled");
    println!();

    // Step 1: Take initial screenshot and analyze
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 1: Analyze current screen state");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot = vision.capture_screen()?;
    println!("[✓] Captured {}x{} screenshot", screenshot.width, screenshot.height);

    let analysis = analyze_for_action(
        &screenshot.data,
        "Look at this Ubuntu/GNOME desktop screenshot. I need to open Firefox browser. \
         Tell me EXACTLY what to do - either: \
         1) If Firefox is visible in a dock/taskbar, give me the X,Y pixel coordinates to click it \
         2) If I need to click 'Activities' in top-left corner, say 'click Activities at 50,14' \
         3) If there's a Firefox icon on the desktop, give coordinates \
         Be very specific with coordinates. Screen is 1920x1080."
    ).await?;

    println!("[Vision Analysis]:");
    println!("{}", analysis);
    println!();

    // Parse coordinates from vision response and click
    if let Some((x, y)) = extract_coordinates(&analysis) {
        println!("[*] Vision suggested clicking at ({}, {})", x, y);
        println!("[*] Moving mouse and clicking...");
        input.mouse_move(x, y)?;
        input.mouse_click(MouseButton::Left)?;
        sleep(Duration::from_millis(500)).await;
    } else {
        // Default: Click Activities corner (GNOME)
        println!("[*] No specific coordinates found, clicking Activities corner (50, 14)...");
        input.mouse_move(50, 14)?;
        input.mouse_click(MouseButton::Left)?;
        sleep(Duration::from_millis(800)).await;
    }

    // Step 2: After clicking Activities, search for Firefox
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 2: Search for Firefox in Activities");
    println!("═══════════════════════════════════════════════════════════════");

    // Take another screenshot to see Activities overview
    sleep(Duration::from_millis(500)).await;
    let screenshot2 = vision.capture_screen()?;

    let analysis2 = analyze_for_action(
        &screenshot2.data,
        "I'm in GNOME Activities overview (or similar). I need to find and launch Firefox. \
         Should I: 1) Type 'firefox' in the search bar, or 2) Click a Firefox icon I can see? \
         If typing, just say 'type firefox'. If clicking, give X,Y coordinates."
    ).await?;

    println!("[Vision Analysis]:");
    println!("{}", analysis2);
    println!();

    // Type "firefox" to search
    if analysis2.to_lowercase().contains("type") || extract_coordinates(&analysis2).is_none() {
        println!("[*] Typing 'firefox' to search...");
        input.type_text("firefox")?;
        sleep(Duration::from_millis(800)).await;
    }

    // Step 3: Press Enter or click Firefox result
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 3: Launch Firefox");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot3 = vision.capture_screen()?;

    let analysis3 = analyze_for_action(
        &screenshot3.data,
        "I typed 'firefox' in GNOME search. I should see Firefox in the results. \
         Should I press Enter to launch, or click somewhere specific? \
         If clicking, give X,Y coordinates of Firefox icon."
    ).await?;

    println!("[Vision Analysis]:");
    println!("{}", analysis3);
    println!();

    if let Some((x, y)) = extract_coordinates(&analysis3) {
        println!("[*] Clicking Firefox at ({}, {})", x, y);
        input.mouse_move(x, y)?;
        input.mouse_click(MouseButton::Left)?;
    } else {
        println!("[*] Pressing Enter to launch Firefox...");
        input.key_press("Return")?;
    }

    // Wait for Firefox to open
    println!("[*] Waiting for Firefox to open...");
    sleep(Duration::from_secs(3)).await;

    // Step 4: Navigate to ebay.com
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 4: Navigate to ebay.com");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot4 = vision.capture_screen()?;

    let analysis4 = analyze_for_action(
        &screenshot4.data,
        "Firefox should be open now. I need to navigate to ebay.com. \
         Is the browser visible? Is there an address bar I can type in? \
         Should I press Ctrl+L to focus the address bar first, or click somewhere?"
    ).await?;

    println!("[Vision Analysis]:");
    println!("{}", analysis4);
    println!();

    // Focus address bar with Ctrl+L
    println!("[*] Pressing Ctrl+L to focus address bar...");
    input.key_combination("ctrl+l")?;
    sleep(Duration::from_millis(300)).await;

    // Type the URL
    println!("[*] Typing 'ebay.com'...");
    input.type_text("ebay.com")?;
    sleep(Duration::from_millis(300)).await;

    // Press Enter
    println!("[*] Pressing Enter...");
    input.key_press("Return")?;

    // Wait for page to load
    println!("[*] Waiting for page to load...");
    sleep(Duration::from_secs(3)).await;

    // Step 5: Final verification
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 5: Verify result");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot5 = vision.capture_screen()?;

    let final_analysis = analyze_for_action(
        &screenshot5.data,
        "Describe what you see. Did we successfully navigate to ebay.com? \
         Is the eBay website visible? Describe any visible eBay branding, \
         search bars, or content that confirms we reached the site."
    ).await?;

    println!("[Final Vision Analysis]:");
    println!("{}", final_analysis);
    println!();

    // Cleanup
    vision.disable();
    input.disable();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           TEST COMPLETE                                       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Ask the vision model for guidance on what action to take
async fn analyze_for_action(base64_image: &str, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    let request_body = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {
                "role": "system",
                "content": "You are a computer vision assistant helping with GUI automation. \
                           Be concise and specific. When giving coordinates, format them as (X, Y). \
                           The screen resolution is 1920x1080. Top-left is (0,0)."
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", base64_image)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 500,
        "temperature": 0.2
    });

    println!("[*] Sending to Qwen3-VL for analysis...");

    let response = client
        .post(BEDROOM_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(content) = result["choices"][0]["message"]["content"].as_str() {
        Ok(content.to_string())
    } else if let Some(error) = result["error"].as_str() {
        Ok(format!("[Vision model error: {}]", error))
    } else {
        Ok(format!("[Unexpected response: {}]", result))
    }
}

/// Extract (X, Y) coordinates from vision model response
fn extract_coordinates(text: &str) -> Option<(i32, i32)> {
    // Look for patterns like (123, 456) or (123,456) or "click at 123, 456"
    let re = regex::Regex::new(r"\(?\s*(\d+)\s*,\s*(\d+)\s*\)?").ok()?;

    if let Some(caps) = re.captures(text) {
        let x: i32 = caps.get(1)?.as_str().parse().ok()?;
        let y: i32 = caps.get(2)?.as_str().parse().ok()?;

        // Sanity check for screen bounds
        if x >= 0 && x <= 1920 && y >= 0 && y <= 1080 {
            return Some((x, y));
        }
    }

    None
}
