//! Simple Firefox Test - Opens Firefox and navigates to ebay.com
//!
//! Uses mouse and keyboard with minimal vision (just verification)
//! Assumes GNOME desktop with standard layout
//!
//! Run with: cargo run --example simple_firefox_test --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::time::Duration;
use tokio::time::sleep;

const BEDROOM_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA SIMPLE FIREFOX TEST                         ║");
    println!("║           Task: Open Firefox -> Navigate to ebay.com          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize controllers
    let vision = VisionController::new();
    let input = InputController::new();

    // Enable modules
    println!("[*] Enabling vision and input modules...");
    vision.enable()?;
    input.enable()?;
    println!("[✓] Modules enabled");
    println!();

    // Step 1: Take initial screenshot for reference
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 1: Capture initial state");
    println!("═══════════════════════════════════════════════════════════════");
    let initial = vision.capture_screen()?;
    println!("[✓] Initial screenshot: {}x{}", initial.width, initial.height);

    // Step 2: Click Activities (top-left hot corner for GNOME)
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 2: Open Activities Overview (GNOME)");
    println!("═══════════════════════════════════════════════════════════════");
    println!("[*] Moving to Activities corner and clicking...");

    // GNOME Activities button is in top-left
    input.mouse_move(50, 14)?;
    sleep(Duration::from_millis(100)).await;
    input.mouse_click(MouseButton::Left)?;

    println!("[✓] Clicked Activities");
    sleep(Duration::from_millis(800)).await;

    // Step 3: Type "firefox" to search
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 3: Search for Firefox");
    println!("═══════════════════════════════════════════════════════════════");
    println!("[*] Typing 'firefox'...");

    input.type_text("firefox")?;
    println!("[✓] Typed search query");
    sleep(Duration::from_millis(1000)).await;

    // Step 4: Press Enter to launch (first result)
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 4: Launch Firefox");
    println!("═══════════════════════════════════════════════════════════════");
    println!("[*] Pressing Enter...");

    input.key_press("Return")?;
    println!("[✓] Pressed Enter");
    println!("[*] Waiting for Firefox to open (3 seconds)...");
    sleep(Duration::from_secs(3)).await;

    // Step 5: Focus address bar and navigate
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 5: Navigate to ebay.com");
    println!("═══════════════════════════════════════════════════════════════");

    // Ctrl+L focuses the address bar
    println!("[*] Pressing Ctrl+L to focus address bar...");
    input.key_combination("ctrl+l")?;
    sleep(Duration::from_millis(300)).await;

    // Type URL
    println!("[*] Typing 'ebay.com'...");
    input.type_text("ebay.com")?;
    sleep(Duration::from_millis(200)).await;

    // Press Enter
    println!("[*] Pressing Enter to navigate...");
    input.key_press("Return")?;
    println!("[✓] Navigation initiated");

    // Wait for page load
    println!("[*] Waiting for page to load (4 seconds)...");
    sleep(Duration::from_secs(4)).await;

    // Step 6: Final verification with vision
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 6: Verify with Vision");
    println!("═══════════════════════════════════════════════════════════════");

    let final_screenshot = vision.capture_screen()?;
    println!("[✓] Captured final screenshot: {}x{}", final_screenshot.width, final_screenshot.height);

    println!("[*] Sending to vision model for verification...");
    match verify_ebay(&final_screenshot.data).await {
        Ok(analysis) => {
            println!();
            println!("[Vision Verification]:");
            println!("{}", analysis);
        }
        Err(e) => {
            println!("[!] Vision verification failed: {}", e);
            println!("[*] (This doesn't mean the test failed - just that vision model is unavailable)");
        }
    }

    // Cleanup
    vision.disable();
    input.disable();

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           TEST COMPLETE                                       ║");
    println!("║           Check your screen - Firefox should show ebay.com!   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}

async fn verify_ebay(base64_image: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let request_body = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Is this the eBay website? Answer YES or NO, then briefly describe what you see."
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
        "max_tokens": 200,
        "temperature": 0.1
    });

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
        Err(format!("Model error: {}", error).into())
    } else {
        Err("Unexpected response format".into())
    }
}
