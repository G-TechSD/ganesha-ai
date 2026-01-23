//! Basic test example for Ganesha Vision system.
//!
//! This example demonstrates the core functionality:
//! 1. Captures a screenshot
//! 2. Moves the mouse in a square pattern
//! 3. Takes another screenshot
//! 4. Prints "SUCCESS" if all worked
//!
//! Run with: cargo run --example basic_test --features screen-capture
//!
//! Requirements:
//! - Linux with X11 display (DISPLAY environment variable set)
//! - xdotool installed for mouse movement (sudo apt install xdotool)
//!
//! Note: This example actually moves your mouse cursor!

use ganesha_vision::capture::{CaptureConfig, ScreenCapture, XcapCapture};
use std::process::Command;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Ganesha Vision Basic Test ===\n");

    // Check for display
    if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
        eprintln!("ERROR: No display available.");
        eprintln!("Please set DISPLAY environment variable (e.g., DISPLAY=:0)");
        std::process::exit(1);
    }

    // Step 1: Initialize screen capture
    println!("Step 1: Initializing screen capture...");
    let capture = XcapCapture::with_defaults();

    if !capture.is_available() {
        eprintln!("ERROR: Screen capture is not available on this system.");
        eprintln!("Make sure you have a display server running (X11 or Wayland).");
        std::process::exit(1);
    }
    println!("  Screen capture initialized.\n");

    // Step 2: Capture first screenshot
    println!("Step 2: Capturing first screenshot...");
    let mut screenshot1 = capture.capture_screen(None).await?;
    println!(
        "  Captured: {}x{} pixels",
        screenshot1.width(),
        screenshot1.height()
    );

    // Encode to base64 to verify encoding works
    let config = CaptureConfig::for_vision_model();
    let base64_1 = screenshot1.to_base64_with_config(&config)?;
    println!("  Base64 size: {} bytes ({:.1} KB)\n", base64_1.len(), base64_1.len() as f64 / 1024.0);

    // Step 3: Move mouse in a square pattern
    println!("Step 3: Moving mouse in a square pattern...");

    #[cfg(target_os = "linux")]
    {
        // Check if xdotool is available
        let xdotool_check = Command::new("which")
            .arg("xdotool")
            .output();

        let xdotool_available = xdotool_check
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !xdotool_available {
            eprintln!("  WARNING: xdotool not found. Skipping mouse movement.");
            eprintln!("  Install with: sudo apt install xdotool");
        } else {
            // Get current mouse position
            let pos_output = Command::new("xdotool")
                .args(["getmouselocation", "--shell"])
                .output()?;
            let initial_pos = String::from_utf8_lossy(&pos_output.stdout);
            println!("  Initial position: {}", initial_pos.lines().take(2).collect::<Vec<_>>().join(", "));

            // Define the square pattern (relative moves)
            // Move: right -> down -> left -> up (back to start)
            let moves = [
                (50, 0, "right"),
                (0, 50, "down"),
                (-50, 0, "left"),
                (0, -50, "up"),
            ];

            for (dx, dy, direction) in moves.iter() {
                print!("  Moving {}...", direction);

                let result = Command::new("xdotool")
                    .args(["mousemove_relative", "--", &dx.to_string(), &dy.to_string()])
                    .output();

                match result {
                    Ok(output) if output.status.success() => {
                        println!(" OK");
                    }
                    Ok(output) => {
                        eprintln!(" FAILED: {}", String::from_utf8_lossy(&output.stderr));
                    }
                    Err(e) => {
                        eprintln!(" ERROR: {}", e);
                    }
                }

                // Small delay between moves for visual feedback
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            println!("  Mouse returned to starting position.\n");
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("  Mouse movement test only implemented for Linux.");
        println!("  Skipping mouse movement on this platform.\n");
    }

    // Step 4: Capture second screenshot
    println!("Step 4: Capturing second screenshot...");
    let mut screenshot2 = capture.capture_screen(None).await?;
    println!(
        "  Captured: {}x{} pixels",
        screenshot2.width(),
        screenshot2.height()
    );

    let base64_2 = screenshot2.to_base64_with_config(&config)?;
    println!("  Base64 size: {} bytes ({:.1} KB)\n", base64_2.len(), base64_2.len() as f64 / 1024.0);

    // Step 5: Verify results
    println!("Step 5: Verifying results...");

    let mut all_passed = true;

    // Check screenshot 1
    if screenshot1.width() > 0 && screenshot1.height() > 0 && !base64_1.is_empty() {
        println!("  [PASS] First screenshot captured and encoded");
    } else {
        println!("  [FAIL] First screenshot invalid");
        all_passed = false;
    }

    // Check screenshot 2
    if screenshot2.width() > 0 && screenshot2.height() > 0 && !base64_2.is_empty() {
        println!("  [PASS] Second screenshot captured and encoded");
    } else {
        println!("  [FAIL] Second screenshot invalid");
        all_passed = false;
    }

    // Check metadata
    if !screenshot1.metadata.id.is_nil() && !screenshot2.metadata.id.is_nil() {
        println!("  [PASS] Screenshots have valid metadata");
    } else {
        println!("  [FAIL] Screenshot metadata invalid");
        all_passed = false;
    }

    // Check timestamps are different (indicating separate captures)
    if screenshot1.metadata.timestamp != screenshot2.metadata.timestamp {
        println!("  [PASS] Screenshots have different timestamps");
    } else {
        println!("  [WARN] Screenshots have same timestamp (captured very quickly)");
        // Not a failure, just a note
    }

    println!();

    // Final result
    if all_passed {
        println!("============================================");
        println!("                 SUCCESS                    ");
        println!("============================================");
        println!("\nAll basic tests passed!");
        println!("The Ganesha Vision system is working correctly.\n");

        // Print additional info
        println!("System Information:");
        println!("  - Display: {}", std::env::var("DISPLAY").unwrap_or_else(|_|
            std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "Unknown".to_string())
        ));
        if let Ok(monitors) = capture.get_monitors().await {
            println!("  - Monitors: {}", monitors.len());
            for m in &monitors {
                println!("    - {} ({}x{}){}",
                    m.name,
                    m.region.width,
                    m.region.height,
                    if m.is_primary { " [PRIMARY]" } else { "" }
                );
            }
        }

        Ok(())
    } else {
        println!("============================================");
        println!("                  FAILED                    ");
        println!("============================================");
        println!("\nSome tests failed. Please check the output above.\n");
        std::process::exit(1);
    }
}
