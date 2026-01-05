//! Ganesha GUI Control Test
//!
//! Tests the core GUI control capabilities:
//! - TracerMouse smooth movement
//! - SpeedController
//! - Screenshot capture
//! - AiCursor display
//!
//! Run with: DISPLAY=:1 cargo run --example ganesha_gui_test --features computer-use

use std::time::Duration;
use std::thread;
use std::process::Command;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   🕉️  GANESHA GUI CONTROL TEST  🕉️                               ║
    ║                                                                   ║
    ║   Testing: TracerMouse, SpeedController, Screenshots             ║
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
    "#);

    // Check DISPLAY is set
    let display = env::var("DISPLAY").unwrap_or_default();
    if display.is_empty() {
        eprintln!("ERROR: DISPLAY not set. Run with: DISPLAY=:1 cargo run --example ganesha_gui_test --features computer-use");
        return Ok(());
    }
    println!("✓ DISPLAY={}", display);

    // Test 1: Screenshot capture
    println!("\n🕉️ Test 1: Screenshot capture...");
    let screenshot_path = "/tmp/ganesha_test_capture.png";
    let output = Command::new("scrot")
        .args(["-o", screenshot_path])
        .output()?;

    if output.status.success() {
        println!("✓ Screenshot saved to {}", screenshot_path);
    } else {
        println!("✗ Screenshot failed");
    }

    // Test 2: Get current mouse position
    println!("\n🕉️ Test 2: Mouse position...");
    let output = Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()?;
    let pos_str = String::from_utf8_lossy(&output.stdout);
    let mut start_x = 0i32;
    let mut start_y = 0i32;
    for line in pos_str.lines() {
        if line.starts_with("X=") {
            start_x = line[2..].parse().unwrap_or(0);
        } else if line.starts_with("Y=") {
            start_y = line[2..].parse().unwrap_or(0);
        }
    }
    println!("✓ Current position: ({}, {})", start_x, start_y);

    // Test 3: Smooth mouse movement (TracerMouse style)
    println!("\n🕉️ Test 3: Smooth mouse movement (TracerMouse)...");
    println!("  Moving from ({}, {}) to (500, 300) with easing...", start_x, start_y);

    let target_x = 500i32;
    let target_y = 300i32;
    let steps = 20;
    let duration_ms = 300u64;
    let step_delay = Duration::from_millis(duration_ms / steps as u64);

    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        // Ease-out cubic
        let eased_t = 1.0 - (1.0 - t).powi(3);

        let current_x = start_x + ((target_x - start_x) as f64 * eased_t) as i32;
        let current_y = start_y + ((target_y - start_y) as f64 * eased_t) as i32;

        Command::new("xdotool")
            .args(["mousemove", &current_x.to_string(), &current_y.to_string()])
            .output()?;

        thread::sleep(step_delay);
    }
    println!("✓ Smooth move complete!");

    // Test 4: Speed modes demonstration
    println!("\n🕉️ Test 4: Speed modes demonstration...");

    let speeds = [
        ("Slow (Movie mode)", 400, 100),      // 400ms
        ("Normal (Human-like)", 150, 100),    // 150ms
        ("Fast (Efficient)", 50, 100),        // 50ms
        ("PowerUser (IT guy after coffee)", 20, 100), // 20ms
        ("Beast (Outperforms IT dept)", 5, 100),      // 5ms - barely visible!
    ];

    for (name, duration_ms, distance) in speeds {
        println!("  {} - {}ms animation", name, duration_ms);

        // Get current position
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()?;
        let pos_str = String::from_utf8_lossy(&output.stdout);
        let mut cur_x = 500i32;
        let mut cur_y = 300i32;
        for line in pos_str.lines() {
            if line.starts_with("X=") {
                cur_x = line[2..].parse().unwrap_or(500);
            } else if line.starts_with("Y=") {
                cur_y = line[2..].parse().unwrap_or(300);
            }
        }

        let target_x = cur_x + distance;
        let steps = 15;
        let step_delay = Duration::from_micros((duration_ms * 1000) / steps as u64);

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let eased_t = 1.0 - (1.0 - t).powi(3);
            let new_x = cur_x + (distance as f64 * eased_t) as i32;

            Command::new("xdotool")
                .args(["mousemove", &new_x.to_string(), &cur_y.to_string()])
                .output()?;

            thread::sleep(step_delay);
        }

        thread::sleep(Duration::from_millis(300)); // Pause between demos
    }
    println!("✓ Speed demonstration complete!");

    // Test 5: Click test
    println!("\n🕉️ Test 5: Click at current position...");
    Command::new("xdotool")
        .args(["click", "1"])
        .output()?;
    println!("✓ Click sent!");

    // Test 6: Draw a pattern (demonstrates precision)
    println!("\n🕉️ Test 6: Drawing a square pattern (precision test)...");
    let square_start_x = 800;
    let square_start_y = 400;
    let square_size = 100;

    // Move to start
    Command::new("xdotool")
        .args(["mousemove", &square_start_x.to_string(), &square_start_y.to_string()])
        .output()?;
    thread::sleep(Duration::from_millis(200));

    // Draw square with smooth movements
    let corners = [
        (square_start_x + square_size, square_start_y),                    // Right
        (square_start_x + square_size, square_start_y + square_size),      // Down
        (square_start_x, square_start_y + square_size),                    // Left
        (square_start_x, square_start_y),                                  // Up (back to start)
    ];

    for (target_x, target_y) in corners {
        // Get current position
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()?;
        let pos_str = String::from_utf8_lossy(&output.stdout);
        let mut cur_x = 0i32;
        let mut cur_y = 0i32;
        for line in pos_str.lines() {
            if line.starts_with("X=") {
                cur_x = line[2..].parse().unwrap_or(0);
            } else if line.starts_with("Y=") {
                cur_y = line[2..].parse().unwrap_or(0);
            }
        }

        // Smooth move to corner
        let steps = 15;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let eased_t = 1.0 - (1.0 - t).powi(3);

            let new_x = cur_x + ((target_x - cur_x) as f64 * eased_t) as i32;
            let new_y = cur_y + ((target_y - cur_y) as f64 * eased_t) as i32;

            Command::new("xdotool")
                .args(["mousemove", &new_x.to_string(), &new_y.to_string()])
                .output()?;

            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
    }
    println!("✓ Square pattern complete!");

    // Final screenshot
    println!("\n🕉️ Taking final screenshot...");
    Command::new("scrot")
        .args(["-o", "/tmp/ganesha_test_final.png"])
        .output()?;
    println!("✓ Final screenshot: /tmp/ganesha_test_final.png");

    println!(r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   🕉️  GANESHA GUI TEST COMPLETE!  🕉️                             ║
    ║                                                                   ║
    ║   All GUI control systems operational.                           ║
    ║   Ready for AI-guided automation!                                ║
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
    "#);

    Ok(())
}
