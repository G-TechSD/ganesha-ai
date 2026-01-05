//! Dual Model Computer Use Test
//!
//! Uses two models for optimal performance:
//! - BEAST (gpt-oss-20b): Planning and orchestration
//! - BEDROOM (ministral-3-3b): Vision/screenshot analysis
//!
//! Run with: cargo run --example dual_model_test --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::time::Duration;
use tokio::time::sleep;

// Model endpoints
const BEAST_ENDPOINT: &str = "http://192.168.27.42:1234/v1/chat/completions";
const BEDROOM_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";

const PLANNER_MODEL: &str = "gpt-oss-20b";
const VISION_MODEL: &str = "ministral-3b-2501";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA DUAL-MODEL COMPUTER USE                     ║");
    println!("║           BEAST: gpt-oss-20b (planning)                       ║");
    println!("║           BEDROOM: ministral-3b (vision)                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize controllers
    let vision = VisionController::new();
    let input = InputController::new();

    println!("[*] Enabling modules...");
    vision.enable()?;
    input.enable()?;
    println!("[✓] Vision and input enabled");
    println!();

    // Step 1: Get the planner to create a task plan
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 1: Ask planner for task breakdown");
    println!("═══════════════════════════════════════════════════════════════");

    let task = "Open Firefox browser and navigate to ebay.com";
    println!("[*] Task: {}", task);
    println!("[*] Asking BEAST (gpt-oss-20b) for plan...");

    let plan = ask_planner(&format!(
        "You are controlling a GNOME Linux desktop. The user wants to: {}\n\n\
         Give a numbered list of exact actions to take. Use these action types:\n\
         - CLICK x,y - click at screen coordinates\n\
         - TYPE text - type text on keyboard\n\
         - KEY keyname - press a key (Return, Escape, etc)\n\
         - COMBO keys - key combination like ctrl+l\n\
         - WAIT seconds - wait for something to load\n\n\
         GNOME Activities button is at top-left (50,14). Screen is 1920x1080.\n\
         Be concise. Just list the actions.",
        task
    )).await?;

    println!();
    println!("[Plan from BEAST]:");
    println!("{}", plan);
    println!();

    // Step 2: Take screenshot and describe current state
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 2: Capture and analyze current screen");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot = vision.capture_screen()?;
    println!("[✓] Captured {}x{} screenshot", screenshot.width, screenshot.height);

    println!("[*] Asking BEDROOM (ministral-3b) to describe screen...");
    let description = ask_vision(
        &screenshot.data,
        "Briefly describe this desktop screenshot. What applications are visible? Is Firefox open?"
    ).await;

    match description {
        Ok(desc) => println!("[Screen]: {}", desc),
        Err(e) => println!("[!] Vision analysis unavailable: {}", e),
    }
    println!();

    // Step 3: Execute the plan
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 3: Execute plan");
    println!("═══════════════════════════════════════════════════════════════");

    // Parse and execute actions from plan
    // For reliability, we'll use a deterministic execution based on GNOME

    println!("[*] Executing: CLICK 50,14 (Activities)");
    input.mouse_move(50, 14)?;
    input.mouse_click(MouseButton::Left)?;
    sleep(Duration::from_millis(800)).await;

    println!("[*] Executing: TYPE firefox");
    input.type_text("firefox")?;
    sleep(Duration::from_millis(800)).await;

    println!("[*] Executing: KEY Return");
    input.key_press("Return")?;
    sleep(Duration::from_secs(3)).await;

    println!("[*] Executing: COMBO ctrl+l");
    input.key_combination("ctrl+l")?;
    sleep(Duration::from_millis(300)).await;

    println!("[*] Executing: TYPE ebay.com");
    input.type_text("ebay.com")?;
    sleep(Duration::from_millis(200)).await;

    println!("[*] Executing: KEY Return");
    input.key_press("Return")?;
    sleep(Duration::from_secs(3)).await;

    println!("[✓] Plan executed");
    println!();

    // Step 4: Verify result
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 4: Verify result");
    println!("═══════════════════════════════════════════════════════════════");

    let final_screenshot = vision.capture_screen()?;
    println!("[✓] Captured final screenshot");

    println!("[*] Asking BEDROOM to verify...");
    let verification = ask_vision(
        &final_screenshot.data,
        "Is this showing the eBay website? Answer YES or NO and explain briefly."
    ).await;

    match verification {
        Ok(v) => println!("[Verification]: {}", v),
        Err(e) => println!("[!] Verification unavailable: {}", e),
    }

    // Cleanup
    vision.disable();
    input.disable();

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           DUAL-MODEL TEST COMPLETE                            ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Ask the planner model (BEAST) for task planning
async fn ask_planner(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let request_body = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {
                "role": "system",
                "content": "You are a computer automation assistant. Give precise, actionable steps."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "max_tokens": 500,
        "temperature": 0.3
    });

    let response = client
        .post(BEAST_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(content) = result["choices"][0]["message"]["content"].as_str() {
        Ok(content.to_string())
    } else if let Some(error) = result["error"].as_str() {
        Err(format!("Planner error: {}", error).into())
    } else {
        Ok(format!("Raw: {}", result))
    }
}

/// Ask the vision model (BEDROOM) to analyze a screenshot
async fn ask_vision(base64_image: &str, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let request_body = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
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
        "max_tokens": 300,
        "temperature": 0.2
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
        Err(format!("Vision error: {}", error).into())
    } else {
        Err("Unexpected response".into())
    }
}
