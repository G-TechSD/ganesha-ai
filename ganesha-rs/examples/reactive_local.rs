//! Reactive Agent - Local Only (no remote models)
//!
//! Demonstrates the vision-in-the-loop architecture with local screenshot
//! polling. When models are available, just change the endpoints.
//!
//! Run with: cargo run --example reactive_local --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Lightweight screen state (no LLM analysis, just metrics)
#[derive(Debug, Clone)]
struct ScreenState {
    timestamp: Instant,
    width: u32,
    height: u32,
    size_bytes: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA REACTIVE AGENT (Local)                      ║");
    println!("║           Continuous screenshot polling demo                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());

    vision.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    input.enable().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    println!("[✓] Vision and input enabled");

    // Shared state
    let current_state: Arc<RwLock<Option<ScreenState>>> = Arc::new(RwLock::new(None));
    let screenshot_count = Arc::new(AtomicU64::new(0));
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    // Start screenshot polling task (300ms = ~3 FPS)
    let poll_vision = Arc::clone(&vision);
    let poll_state = Arc::clone(&current_state);
    let poll_count = Arc::clone(&screenshot_count);
    let poll_running = Arc::clone(&running);

    let poll_handle = tokio::spawn(async move {
        println!("[Polling] Started at 300ms interval (~3 FPS)");
        while poll_running.load(Ordering::SeqCst) {
            let start = Instant::now();

            if let Ok(screenshot) = poll_vision.capture_screen() {
                poll_count.fetch_add(1, Ordering::SeqCst);

                let state = ScreenState {
                    timestamp: Instant::now(),
                    width: screenshot.width,
                    height: screenshot.height,
                    size_bytes: screenshot.data.len(),
                };

                *poll_state.write().await = Some(state);
            }

            let elapsed = start.elapsed();
            if elapsed < Duration::from_millis(300) {
                sleep(Duration::from_millis(300) - elapsed).await;
            }
        }
        println!("[Polling] Stopped");
    });

    // Give polling a moment to start
    sleep(Duration::from_millis(500)).await;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("TASK: Close Firefox, Open Writer, Write about cats");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    // Helper to show state
    let show_state = |label: &str, state: &Option<ScreenState>, count: u64| {
        if let Some(s) = state {
            println!("[{}] {}x{}, {}KB (screenshot #{})",
                label, s.width, s.height, s.size_bytes / 1024, count);
        }
    };

    // Initial state
    show_state("Initial", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    // Step 1: Close Firefox
    println!("\n[Step 1] Closing Firefox (click X at 1900,14)...");
    input.mouse_move(1900, 14).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    input.mouse_click(MouseButton::Left).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    // Wait and observe screen changes
    sleep(Duration::from_millis(500)).await;
    let before_size = current_state.read().await.as_ref().map(|s| s.size_bytes).unwrap_or(0);
    sleep(Duration::from_millis(500)).await;
    let after_size = current_state.read().await.as_ref().map(|s| s.size_bytes).unwrap_or(0);

    if before_size != after_size {
        println!("[✓] Screen changed: {}KB -> {}KB", before_size/1024, after_size/1024);
    }
    show_state("After close", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    // Step 2: Open Activities
    println!("\n[Step 2] Opening Activities (click 50,14)...");
    input.mouse_move(50, 14).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    input.mouse_click(MouseButton::Left).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_millis(800)).await;
    show_state("Activities", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    // Step 3: Search for Writer
    println!("\n[Step 3] Typing 'writer'...");
    input.type_text("writer").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_millis(800)).await;
    show_state("Search", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    // Step 4: Launch Writer
    println!("\n[Step 4] Pressing Enter...");
    input.key_press("Return").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    // Wait for Writer to load - poll for screen stability
    println!("[*] Waiting for Writer to open (watching for screen stability)...");
    let mut last_size = 0usize;
    let mut stable_count = 0;
    for _ in 0..30 {  // Max 9 seconds
        sleep(Duration::from_millis(300)).await;
        let size = current_state.read().await.as_ref().map(|s| s.size_bytes).unwrap_or(0);
        if size == last_size {
            stable_count += 1;
            if stable_count >= 3 {  // Stable for ~1 second
                println!("[✓] Screen stable (Writer likely loaded)");
                break;
            }
        } else {
            stable_count = 0;
        }
        last_size = size;
    }
    show_state("Writer", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    // Step 5: Type document
    println!("\n[Step 5] Writing about cats...");
    let paragraphs = [
        "The Wonderful World of Cats\n\n",
        "Cats have been companions to humans for thousands of years.\n\n",
        "The domestic cat, Felis catus, comes in over 70 breeds.\n\n",
        "Ancient Egyptians worshipped cats as sacred animals.\n\n",
        "A cat's purr frequency may promote healing.\n\n",
    ];

    for (i, para) in paragraphs.iter().enumerate() {
        print!("[*] Paragraph {}... ", i + 1);
        input.type_text(para).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        println!("(screenshot #{})", screenshot_count.load(Ordering::SeqCst));
        sleep(Duration::from_millis(100)).await;
    }

    // Step 6: Save
    println!("\n[Step 6] Saving (Ctrl+S)...");
    input.key_combination("ctrl+s").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_secs(1)).await;
    show_state("Save dialog", &*current_state.read().await, screenshot_count.load(Ordering::SeqCst));

    input.type_text("cats_reactive_local").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_millis(200)).await;
    input.key_press("Return").map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    sleep(Duration::from_secs(1)).await;

    // Stop polling
    running.store(false, Ordering::SeqCst);
    let _ = poll_handle.await;

    // Final stats
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("COMPLETE");
    println!("═══════════════════════════════════════════════════════════════");
    println!("[Total Screenshots] {}", screenshot_count.load(Ordering::SeqCst));
    println!();
    println!("With a vision model, each screenshot would be analyzed for:");
    println!("  - What app is open?");
    println!("  - Any dialogs/popups?");
    println!("  - Did the action succeed?");
    println!("  - What should we do next?");
    println!();
    println!("The orchestrator (GPT-5/Claude) would only see TEXT descriptions,");
    println!("not images - making it 10-100x cheaper than sending screenshots.");

    vision.disable();
    input.disable();

    println!("\n[✓] Done");
    Ok(())
}
