//! Screen capture demonstration for Ganesha Vision.
//!
//! This example demonstrates various screen capture capabilities.
//!
//! Run with: cargo run --example capture_demo --features screen-capture

use ganesha_vision::capture::{
    CaptureConfig, CaptureRegion, ImageFormat, ScreenBuffer, ScreenBufferConfig, ScreenCapture,
    XcapCapture,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Ganesha Vision Screen Capture Demo ===\n");

    // Create capture with default config
    let capture = XcapCapture::with_defaults();

    // Check if capture is available
    if !capture.is_available() {
        eprintln!("Screen capture is not available on this system.");
        return Ok(());
    }

    // 1. List available monitors
    println!("1. Available Monitors:");
    println!("-----------------------");
    let monitors = capture.get_monitors().await?;
    for monitor in &monitors {
        println!(
            "  Monitor {}: {} ({}x{}) at ({},{}) - scale: {}x{}",
            monitor.index,
            monitor.name,
            monitor.region.width,
            monitor.region.height,
            monitor.region.x,
            monitor.region.y,
            monitor.scale_factor,
            if monitor.is_primary { " [PRIMARY]" } else { "" }
        );
    }
    println!();

    // 2. Capture primary screen
    println!("2. Capturing Primary Screen...");
    println!("-------------------------------");
    let mut screenshot = capture.capture_screen(None).await?;
    println!(
        "  Captured: {}x{} from '{}' at {}",
        screenshot.width(),
        screenshot.height(),
        screenshot.metadata.source,
        screenshot.metadata.timestamp
    );

    // Convert to base64 for vision model
    let config = CaptureConfig::for_vision_model();
    let base64 = screenshot.to_base64_with_config(&config)?;
    println!(
        "  Base64 size: {} bytes ({:.1} KB)",
        base64.len(),
        base64.len() as f64 / 1024.0
    );
    println!();

    // 3. Capture a region
    println!("3. Capturing Region (100,100 - 400x300)...");
    println!("------------------------------------------");
    let region = CaptureRegion::new(100, 100, 400, 300);
    let region_screenshot = capture.capture_region(region).await?;
    println!(
        "  Captured region: {}x{}",
        region_screenshot.width(),
        region_screenshot.height()
    );
    println!();

    // 4. List windows
    println!("4. Visible Windows:");
    println!("-------------------");
    let windows = capture.get_windows().await?;
    for (i, window) in windows.iter().take(10).enumerate() {
        println!(
            "  {}: [{}] '{}' ({}x{}) - {}",
            i + 1,
            window.app_name,
            if window.title.len() > 50 {
                format!("{}...", &window.title[..47])
            } else {
                window.title.clone()
            },
            window.region.width,
            window.region.height,
            if window.is_visible {
                "visible"
            } else {
                "hidden"
            }
        );
    }
    if windows.len() > 10 {
        println!("  ... and {} more windows", windows.len() - 10);
    }
    println!();

    // 5. Demonstrate screenshot buffer for rapid capture
    println!("5. Screenshot Buffer Demo (1fps for 3 seconds)...");
    println!("-------------------------------------------------");

    let rapid_config = CaptureConfig::for_rapid_capture();
    let rapid_capture = XcapCapture::new(rapid_config);
    let buffer_config = ScreenBufferConfig {
        max_size: 10,
        target_fps: 1.0,
        drop_oldest: true,
        capture_config: CaptureConfig::for_rapid_capture(),
    };

    let buffer = ScreenBuffer::new(rapid_capture, buffer_config);

    // Capture a few screenshots manually
    for i in 0..3 {
        buffer.capture_one().await?;
        let stats = buffer.stats().await;
        println!(
            "  Captured frame {}: buffer size = {}, avg capture time = {:.1}ms",
            i + 1,
            stats.current_size,
            stats.avg_capture_time_ms
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Get the latest screenshot from buffer
    if let Some(latest) = buffer.latest().await {
        println!(
            "\n  Latest buffered screenshot: {} bytes (from {})",
            latest.encoded_bytes.len(),
            latest.metadata.timestamp
        );
    }

    // Drain all buffered screenshots
    let all_screenshots = buffer.drain().await;
    println!("  Drained {} screenshots from buffer", all_screenshots.len());
    println!();

    // 6. Different image formats comparison
    println!("6. Image Format Comparison:");
    println!("---------------------------");
    let mut test_screenshot = capture.capture_screen(None).await?;

    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let config = CaptureConfig::default()
            .with_format(format)
            .with_quality(80)
            .with_max_dimension(1280);

        let encoded = test_screenshot.encode(&config)?;
        println!(
            "  {:?}: {} bytes ({:.1} KB)",
            format,
            encoded.len(),
            encoded.len() as f64 / 1024.0
        );
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
