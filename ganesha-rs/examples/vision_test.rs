//! Vision Test - Screenshots with Qwen VL Analysis
//!
//! Run with: cargo run --example vision_test --features vision

use ganesha::vision::VisionController;
use std::io::Write;

const BEDROOM_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA VISION TEST                                 ║");
    println!("║           Using Qwen3-VL on BEDROOM                           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize vision controller
    let vision = VisionController::new();

    // Enable vision (in production, this requires user consent)
    println!("[*] Enabling vision module...");
    vision.enable()?;
    println!("[✓] Vision enabled");

    // Take screenshot
    println!("[*] Capturing screenshot...");
    let screenshot = vision.capture_screen()?;
    println!(
        "[✓] Captured {}x{} screenshot ({} bytes base64)",
        screenshot.width,
        screenshot.height,
        screenshot.data.len()
    );

    // Send to vision model
    println!("[*] Sending to Qwen3-VL for analysis...");
    let analysis = analyze_screenshot(&screenshot.data).await?;

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           VISION ANALYSIS RESULT                              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("{}", analysis);

    // Disable vision
    vision.disable();
    println!();
    println!("[✓] Vision disabled");

    Ok(())
}

async fn analyze_screenshot(base64_image: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant that describes what you see on computer screens. Be concise but thorough. Identify: the operating system, open applications, visible UI elements, and any text or content shown."
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What do you see on this screen? Describe the desktop, applications, and any notable content."
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
        "max_tokens": 1000,
        "temperature": 0.3
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
    } else {
        Ok(format!("Raw response: {}", result))
    }
}
