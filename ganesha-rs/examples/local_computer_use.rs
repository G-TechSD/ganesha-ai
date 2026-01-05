//! Local Computer Use Test - No external models required
//!
//! Tests pure mouse/keyboard/vision capabilities without LLM calls
//!
//! Run with: cargo run --example local_computer_use --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA LOCAL COMPUTER USE TEST                     ║");
    println!("║           No external models - pure input/vision test         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = VisionController::new();
    let input = InputController::new();

    println!("[*] Enabling modules...");
    vision.enable()?;
    input.enable()?;
    println!("[✓] Ready");
    println!();

    // Capture before
    println!("[1] Capturing initial screenshot...");
    let before = vision.capture_screen()?;
    println!("    {}x{} ({} bytes)", before.width, before.height, before.data.len());

    // Open Activities
    println!("[2] Clicking Activities (50, 14)...");
    input.mouse_move(50, 14)?;
    input.mouse_click(MouseButton::Left)?;
    sleep(Duration::from_millis(800)).await;

    // Search Firefox
    println!("[3] Typing 'firefox'...");
    input.type_text("firefox")?;
    sleep(Duration::from_millis(800)).await;

    // Launch
    println!("[4] Pressing Enter...");
    input.key_press("Return")?;
    sleep(Duration::from_secs(3)).await;

    // Navigate
    println!("[5] Ctrl+L (focus address bar)...");
    input.key_combination("ctrl+l")?;
    sleep(Duration::from_millis(300)).await;

    println!("[6] Typing 'ebay.com'...");
    input.type_text("ebay.com")?;
    sleep(Duration::from_millis(200)).await;

    println!("[7] Pressing Enter...");
    input.key_press("Return")?;
    sleep(Duration::from_secs(3)).await;

    // Capture after
    println!("[8] Capturing final screenshot...");
    let after = vision.capture_screen()?;
    println!("    {}x{} ({} bytes)", after.width, after.height, after.data.len());

    // Save screenshots for manual inspection
    println!();
    println!("[*] Saving screenshots to /tmp/...");

    use std::fs;
    use base64::Engine;

    let before_bytes = base64::engine::general_purpose::STANDARD.decode(&before.data)?;
    let after_bytes = base64::engine::general_purpose::STANDARD.decode(&after.data)?;

    fs::write("/tmp/ganesha_before.png", &before_bytes)?;
    fs::write("/tmp/ganesha_after.png", &after_bytes)?;

    println!("[✓] Saved: /tmp/ganesha_before.png");
    println!("[✓] Saved: /tmp/ganesha_after.png");

    vision.disable();
    input.disable();

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  DONE - Check /tmp/ganesha_*.png to verify                    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}
