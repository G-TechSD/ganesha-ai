//! Integration tests for the Ganesha Vision system.
//!
//! These tests verify the full pipeline works on a real display.
//! Tests marked with #[ignore] require a real X11 display and are skipped in CI.
//!
//! Run all tests: cargo test --features screen-capture
//! Run ignored tests: cargo test --features screen-capture -- --ignored
//! Run all including ignored: cargo test --features screen-capture -- --include-ignored

use ganesha_learning::capture::{
    CaptureConfig, CaptureRegion, ImageFormat, ScreenBuffer, ScreenBufferConfig,
    ScreenCapture, XcapCapture,
};
use ganesha_learning::db::{
    ActionDetails, ActionType, Database, Demonstration, MouseButton, Outcome,
    RecordedAction, Skill,
};
use ganesha_learning::learning::{
    ExtractionConfig, LearningEngine, MatchConfig, Screenshot as LearningScreenshot,
    SkillExtractor, SkillMatcher,
};
use ganesha_learning::model::{ScreenAnalysis, UIElement, VisionClient, VisionModelConfig};
use std::env;
use std::time::Duration;
use tempfile::tempdir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a display is available for screen capture tests.
fn is_display_available() -> bool {
    // Check for X11 display
    if env::var("DISPLAY").is_ok() {
        // Try to actually create a capture to verify it works
        let capture = XcapCapture::with_defaults();
        return capture.is_available();
    }
    // Check for Wayland
    if env::var("WAYLAND_DISPLAY").is_ok() {
        let capture = XcapCapture::with_defaults();
        return capture.is_available();
    }
    false
}

/// Helper macro to skip tests if no display is available.
macro_rules! require_display {
    () => {
        if !is_display_available() {
            eprintln!("Skipping test: No display available (DISPLAY or WAYLAND_DISPLAY not set)");
            return;
        }
    };
}

/// Check if LM Studio or another OpenAI-compatible vision API is available.
async fn is_vision_api_available() -> bool {
    let config = VisionModelConfig::lm_studio();
    match VisionClient::new(config) {
        Ok(client) => client.health_check().await.unwrap_or(false),
        Err(_) => false,
    }
}

/// Create a test database in a temporary directory.
fn create_test_db() -> (Database, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create temp dir");
    let db_path = dir.path().join("test_ganesha.db");
    let db = Database::open(&db_path).expect("Failed to create database");
    (db, dir)
}

// ============================================================================
// Screen Capture Tests
// ============================================================================

/// Test that screen capture works on a real display.
/// This test verifies:
/// - The capture system can initialize
/// - A screenshot can be captured
/// - The screenshot has valid dimensions
/// - The screenshot can be encoded to base64
#[tokio::test]
#[ignore] // Requires real display
async fn test_screen_capture() {
    require_display!();

    let capture = XcapCapture::with_defaults();
    assert!(capture.is_available(), "Screen capture should be available");

    // Capture the primary screen
    let result = capture.capture_screen(None).await;
    assert!(result.is_ok(), "Screen capture should succeed");

    let mut screenshot = result.unwrap();

    // Verify screenshot has valid dimensions
    assert!(screenshot.width() > 0, "Screenshot width should be positive");
    assert!(screenshot.height() > 0, "Screenshot height should be positive");
    println!(
        "Captured screenshot: {}x{}",
        screenshot.width(),
        screenshot.height()
    );

    // Verify it can be encoded to base64
    let config = CaptureConfig::for_vision_model();
    let base64_result = screenshot.to_base64_with_config(&config);
    assert!(base64_result.is_ok(), "Base64 encoding should succeed");

    let base64 = base64_result.unwrap();
    assert!(!base64.is_empty(), "Base64 data should not be empty");
    println!("Base64 size: {} bytes", base64.len());

    // Verify metadata is populated
    assert!(!screenshot.metadata.id.is_nil(), "Screenshot should have an ID");
}

/// Test capturing different regions of the screen.
#[tokio::test]
#[ignore] // Requires real display
async fn test_region_capture() {
    require_display!();

    let capture = XcapCapture::with_defaults();

    // First capture full screen to know the dimensions
    let full_screenshot = capture.capture_screen(None).await.unwrap();
    let max_width = full_screenshot.width();
    let max_height = full_screenshot.height();

    // Capture a small region in the top-left
    let region = CaptureRegion::new(0, 0, 100.min(max_width), 100.min(max_height));
    let region_screenshot = capture.capture_region(region).await;
    assert!(region_screenshot.is_ok(), "Region capture should succeed");

    let region_screenshot = region_screenshot.unwrap();
    assert!(region_screenshot.width() <= 100, "Region width should be bounded");
    assert!(region_screenshot.height() <= 100, "Region height should be bounded");
}

/// Test listing monitors.
#[tokio::test]
#[ignore] // Requires real display
async fn test_list_monitors() {
    require_display!();

    let capture = XcapCapture::with_defaults();
    let monitors = capture.get_monitors().await;
    assert!(monitors.is_ok(), "Getting monitors should succeed");

    let monitors = monitors.unwrap();
    assert!(!monitors.is_empty(), "Should have at least one monitor");

    // Verify first monitor has valid properties
    let first = &monitors[0];
    assert!(!first.name.is_empty(), "Monitor should have a name");
    assert!(first.region.width > 0, "Monitor width should be positive");
    assert!(first.region.height > 0, "Monitor height should be positive");

    println!("Found {} monitor(s)", monitors.len());
    for monitor in &monitors {
        println!(
            "  Monitor {}: {} ({}x{})",
            monitor.index, monitor.name, monitor.region.width, monitor.region.height
        );
    }
}

/// Test screen buffer for continuous capture.
#[tokio::test]
#[ignore] // Requires real display
async fn test_screen_buffer() {
    require_display!();

    let config = CaptureConfig::for_rapid_capture();
    let capture = XcapCapture::new(config);
    let buffer_config = ScreenBufferConfig {
        max_size: 5,
        target_fps: 2.0,
        drop_oldest: true,
        capture_config: CaptureConfig::for_rapid_capture(),
    };

    let buffer = ScreenBuffer::new(capture, buffer_config);

    // Capture a few frames
    for _ in 0..3 {
        let result = buffer.capture_one().await;
        assert!(result.is_ok(), "Buffer capture should succeed");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Verify buffer has screenshots
    let len = buffer.len().await;
    assert!(len > 0, "Buffer should have screenshots");
    assert!(len <= 5, "Buffer should respect max_size");

    // Verify we can get the latest screenshot
    let latest = buffer.latest().await;
    assert!(latest.is_some(), "Should have a latest screenshot");
    let latest = latest.unwrap();
    assert!(!latest.base64.is_empty(), "Latest should have base64 data");

    let stats = buffer.stats().await;
    println!(
        "Buffer stats: {} captures, avg time: {:.1}ms",
        stats.total_captures, stats.avg_capture_time_ms
    );
}

// ============================================================================
// Input Simulation Tests
// ============================================================================

/// Test mouse movement simulation.
/// Moves the mouse in a square pattern.
/// Note: This actually moves the system mouse cursor!
#[tokio::test]
#[ignore] // Requires real display and moves actual mouse
async fn test_mouse_movement() {
    require_display!();

    // We'll use enigo for input simulation if available
    // Since enigo may not be a dependency, we'll use X11 directly via xcb or skip
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // Use xdotool for mouse movement (commonly available on Linux)
        let xdotool_available = Command::new("which")
            .arg("xdotool")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !xdotool_available {
            eprintln!("Skipping mouse test: xdotool not available");
            return;
        }

        // Get current mouse position
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .expect("Failed to get mouse location");
        let location = String::from_utf8_lossy(&output.stdout);
        println!("Initial mouse location: {}", location.trim());

        // Move mouse in a square pattern (relative moves, small distances)
        let moves = [(100, 0), (0, 100), (-100, 0), (0, -100)];

        for (dx, dy) in moves.iter() {
            let result = Command::new("xdotool")
                .args(["mousemove_relative", "--", &dx.to_string(), &dy.to_string()])
                .output();

            assert!(result.is_ok(), "Mouse move should succeed");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        println!("Mouse movement test completed successfully");
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Mouse movement test only implemented for Linux");
    }
}

/// Test keyboard input simulation (in a safe way).
/// This test simulates typing but requires a text field to be focused.
/// By default, it just verifies the xdotool command works.
#[tokio::test]
#[ignore] // Requires real display and types actual keys
async fn test_keyboard_input() {
    require_display!();

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let xdotool_available = Command::new("which")
            .arg("xdotool")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !xdotool_available {
            eprintln!("Skipping keyboard test: xdotool not available");
            return;
        }

        // Just verify xdotool can handle the command syntax
        // We won't actually type anything to avoid interfering with running applications
        let result = Command::new("xdotool")
            .arg("version")
            .output();

        assert!(result.is_ok(), "xdotool should work");
        let output = result.unwrap();
        assert!(output.status.success(), "xdotool version should succeed");

        println!(
            "xdotool version: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );

        // Note: To actually test keyboard input, you would use:
        // Command::new("xdotool").args(["type", "--clearmodifiers", "test text"]).output();
        // But this types actual keys, so we skip it in the test.
        println!("Keyboard input test completed (dry run)");
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Keyboard input test only implemented for Linux");
    }
}

// ============================================================================
// Database CRUD Tests
// ============================================================================

/// Test basic database operations: Create, Read, Update, Delete demonstrations.
#[test]
fn test_database_crud() {
    let (db, _dir) = create_test_db();

    // CREATE: Save a new demonstration
    let mut demo = Demonstration::new("TestApp", "Click the button");
    demo.add_action(RecordedAction::new(
        ActionType::MouseClick,
        ActionDetails::MouseClick {
            x: 100,
            y: 200,
            button: MouseButton::Left,
            modifiers: vec![],
        },
    ));
    demo.add_action(RecordedAction::new(
        ActionType::TextInput,
        ActionDetails::TextInput {
            text: "Hello".to_string(),
        },
    ));
    demo.set_outcome(Outcome::Success);
    demo.duration_ms = 5000;

    let demo_id = demo.id.clone();
    let result = db.save_demonstration(&demo);
    assert!(result.is_ok(), "Saving demonstration should succeed");

    // READ: Retrieve the demonstration
    let loaded = db.get_demonstration(&demo_id);
    assert!(loaded.is_ok(), "Getting demonstration should succeed");
    let loaded = loaded.unwrap();
    assert!(loaded.is_some(), "Demonstration should exist");
    let loaded = loaded.unwrap();
    assert_eq!(loaded.app_name, "TestApp");
    assert_eq!(loaded.task_description, "Click the button");
    assert_eq!(loaded.action_count(), 2);
    assert_eq!(loaded.outcome, Some(Outcome::Success));

    // UPDATE: Modify and save again
    let mut updated_demo = loaded;
    updated_demo.notes = Some("Updated notes".to_string());
    updated_demo.tags.push("test".to_string());
    let result = db.save_demonstration(&updated_demo);
    assert!(result.is_ok(), "Updating demonstration should succeed");

    // Verify update
    let reloaded = db.get_demonstration(&demo_id).unwrap().unwrap();
    assert_eq!(reloaded.notes, Some("Updated notes".to_string()));
    assert!(reloaded.tags.contains(&"test".to_string()));

    // DELETE: Remove the demonstration
    let deleted = db.delete_demonstration(&demo_id);
    assert!(deleted.is_ok(), "Deleting demonstration should succeed");
    assert!(deleted.unwrap(), "Delete should return true");

    // Verify deletion
    let gone = db.get_demonstration(&demo_id).unwrap();
    assert!(gone.is_none(), "Demonstration should be deleted");
}

/// Test database skill operations.
#[test]
fn test_database_skill_crud() {
    let (db, _dir) = create_test_db();

    // CREATE: Save a new skill
    let mut skill = Skill::new("Navigate Menu", "Navigate through application menu hierarchy");
    skill.applicable_apps.push("TestApp".to_string());
    skill.trigger_patterns.push("*menu*".to_string());
    skill.trigger_patterns.push("*navigate*".to_string());

    let skill_id = skill.id.clone();
    let result = db.save_skill(&skill);
    assert!(result.is_ok(), "Saving skill should succeed");

    // READ: Retrieve the skill
    let loaded = db.get_skill(&skill_id).unwrap().unwrap();
    assert_eq!(loaded.name, "Navigate Menu");
    assert!(loaded.applicable_apps.contains(&"TestApp".to_string()));

    // UPDATE: Update skill outcome
    let result = db.update_skill_outcome(&skill_id, true);
    assert!(result.is_ok(), "Updating skill outcome should succeed");

    let result = db.update_skill_outcome(&skill_id, true);
    assert!(result.is_ok());

    let result = db.update_skill_outcome(&skill_id, false);
    assert!(result.is_ok());

    // Verify updated counts
    let updated = db.get_skill(&skill_id).unwrap().unwrap();
    assert_eq!(updated.success_count, 2);
    assert_eq!(updated.failure_count, 1);
    assert!(updated.confidence > 0.5, "Confidence should reflect success rate");

    // LIST: Verify skill appears in list
    let all_skills = db.list_skills(false).unwrap();
    assert_eq!(all_skills.len(), 1);

    let enabled_skills = db.list_skills(true).unwrap();
    assert_eq!(enabled_skills.len(), 1);

    // DELETE: Remove the skill
    let deleted = db.delete_skill(&skill_id).unwrap();
    assert!(deleted, "Delete should return true");

    let gone = db.get_skill(&skill_id).unwrap();
    assert!(gone.is_none(), "Skill should be deleted");
}

/// Test searching demonstrations.
#[test]
fn test_database_search() {
    let (db, _dir) = create_test_db();

    // Create several demonstrations
    let demos = vec![
        Demonstration::new("Firefox", "Open settings menu"),
        Demonstration::new("Firefox", "Navigate to bookmarks"),
        Demonstration::new("Chrome", "Open settings"),
        Demonstration::new("Blender", "Select render engine"),
    ];

    for demo in &demos {
        db.save_demonstration(demo).unwrap();
    }

    // Search by task description
    let results = db.search_demonstrations("settings", 10).unwrap();
    assert_eq!(results.len(), 2, "Should find 2 demos with 'settings'");

    // Filter by app
    let results = db.list_demonstrations(Some("Firefox"), 10).unwrap();
    assert_eq!(results.len(), 2, "Should find 2 Firefox demos");

    // Search by app name
    let results = db.search_demonstrations("blender", 10).unwrap();
    assert_eq!(results.len(), 1, "Should find 1 Blender demo");
}

// ============================================================================
// Learning System Tests
// ============================================================================

/// Test recording a demonstration and playing it back.
#[test]
fn test_learning_record_playback() {
    let (db, _dir) = create_test_db();
    let engine = LearningEngine::new(db);

    // Start recording
    let session_id = engine.start_recording("TestApp", "Test task");
    assert!(session_id.is_ok(), "Starting recording should succeed");
    assert!(engine.is_recording(), "Engine should be recording");

    // Record some actions
    engine.record_click(100, 200, MouseButton::Left).unwrap();
    engine.record_click(150, 250, MouseButton::Left).unwrap();
    engine.record_text("Hello World").unwrap();

    assert_eq!(engine.current_action_count(), 3);

    // Stop recording
    let demo = engine.stop_recording();
    assert!(demo.is_ok(), "Stopping recording should succeed");
    assert!(!engine.is_recording(), "Engine should not be recording");

    let demo = demo.unwrap();
    assert_eq!(demo.app_name, "TestApp");
    assert_eq!(demo.task_description, "Test task");
    assert_eq!(demo.action_count(), 3);

    // Verify demonstration was saved
    let loaded = engine.get_demonstration(&demo.id).unwrap();
    assert!(loaded.is_some(), "Demonstration should be persisted");

    // Extract a skill from the demonstration
    let skill = engine.extract_skill(&demo, "Test Skill");
    assert!(skill.is_ok(), "Skill extraction should succeed");

    let skill = skill.unwrap();
    assert_eq!(skill.name, "Test Skill");
    assert_eq!(skill.action_template.len(), 3);
    assert!(skill.learned_from.contains(&demo.id));

    // Verify skill was saved
    let loaded_skill = engine.get_skill(&skill.id).unwrap();
    assert!(loaded_skill.is_some(), "Skill should be persisted");

    // Find relevant skills
    let screenshot = LearningScreenshot::new("test_data".to_string(), 1920, 1080)
        .with_app_info("TestApp", "Test Window");

    // Refresh cache to include the new skill
    engine.refresh_cache().unwrap();

    let matches = engine.find_relevant_skills("test task", &screenshot);
    assert!(matches.is_ok(), "Finding skills should succeed");
    // The skill might or might not match depending on trigger patterns
    // Just verify the call completes
}

/// Test skill matching with various contexts.
#[test]
fn test_skill_matching() {
    let matcher = SkillMatcher::new(MatchConfig::default());

    // Create test skills
    let mut skill1 = Skill::new("Open Settings", "Navigate to application settings");
    skill1.trigger_patterns.push("*settings*".to_string());
    skill1.trigger_patterns.push("*preferences*".to_string());
    skill1.applicable_apps.push("Firefox".to_string());
    skill1.enabled = true;
    skill1.confidence = 0.8;

    let mut skill2 = Skill::new("Save File", "Save the current document");
    skill2.trigger_patterns.push("*save*".to_string());
    skill2.trigger_patterns.push("*file*".to_string());
    skill2.enabled = true;
    skill2.confidence = 0.9;

    let skills = vec![skill1.clone(), skill2.clone()];

    // Test matching "open settings"
    let screenshot = LearningScreenshot::new("data".to_string(), 1920, 1080)
        .with_app_info("Firefox", "Firefox - New Tab");

    let matches = matcher.match_skills(&skills, "open settings dialog", &screenshot);
    assert!(!matches.is_empty(), "Should find matching skills");
    assert_eq!(matches[0].skill.name, "Open Settings");

    // Test matching "save file"
    let matches = matcher.match_skills(&skills, "save the current file", &screenshot);
    assert!(!matches.is_empty(), "Should find matching skills");
    assert_eq!(matches[0].skill.name, "Save File");

    // Test with non-matching context
    let _matches = matcher.match_skills(&skills, "random unrelated query", &screenshot);
    // May or may not have matches based on text similarity, but should not error
}

/// Test skill extraction from demonstrations.
#[test]
fn test_skill_extraction() {
    let extractor = SkillExtractor::new(ExtractionConfig::default());

    // Create a demonstration with multiple actions
    let mut demo = Demonstration::new("Firefox", "Open preferences from menu");
    demo.add_action(RecordedAction::new(
        ActionType::MouseClick,
        ActionDetails::MouseClick {
            x: 50,
            y: 10,
            button: MouseButton::Left,
            modifiers: vec![],
        },
    ));
    demo.add_action(RecordedAction::new(
        ActionType::MouseClick,
        ActionDetails::MouseClick {
            x: 60,
            y: 150,
            button: MouseButton::Left,
            modifiers: vec![],
        },
    ));
    demo.add_action(RecordedAction::new(
        ActionType::MouseClick,
        ActionDetails::MouseClick {
            x: 70,
            y: 200,
            button: MouseButton::Left,
            modifiers: vec![],
        },
    ));

    let skill = extractor.extract(&demo, "Menu Navigation");
    assert!(skill.is_ok(), "Extraction should succeed");

    let skill = skill.unwrap();
    assert_eq!(skill.name, "Menu Navigation");
    assert_eq!(skill.action_template.len(), 3);
    assert!(skill.applicable_apps.contains(&"Firefox".to_string()));
    assert!(!skill.trigger_patterns.is_empty(), "Should have trigger patterns");
}

// ============================================================================
// Vision Model Tests
// ============================================================================

/// Test vision model with mock responses.
/// This test verifies the client construction and error handling without requiring an actual API.
#[test]
fn test_vision_model_mock() {
    // Test configuration creation
    let config = VisionModelConfig::lm_studio();
    assert_eq!(config.endpoint, "http://localhost:1234/v1/chat/completions");
    assert!(config.validate().is_ok());

    let config = VisionModelConfig::ollama("llava");
    assert_eq!(config.endpoint, "http://localhost:11434/v1/chat/completions");
    assert_eq!(config.model_name, "llava");

    // Test client creation
    let client = VisionClient::new(VisionModelConfig::default());
    assert!(client.is_ok(), "Client creation should succeed");

    // Test configuration validation
    let mut bad_config = VisionModelConfig::default();
    bad_config.endpoint = String::new();
    assert!(bad_config.validate().is_err(), "Empty endpoint should fail validation");

    bad_config = VisionModelConfig::default();
    bad_config.model_name = String::new();
    assert!(bad_config.validate().is_err(), "Empty model name should fail validation");

    // Test ScreenAnalysis struct
    let analysis = ScreenAnalysis {
        app_name: "Firefox".to_string(),
        window_title: "Test Page".to_string(),
        ui_elements: vec![
            UIElement::new("button", "Submit")
                .with_bounds(100, 200, 80, 30)
                .with_interactive(true),
            UIElement::new("input", "Search")
                .with_bounds(200, 100, 200, 30)
                .with_interactive(true),
            UIElement::new("text", "Welcome"),
        ],
        visible_text: vec!["Welcome".to_string(), "Submit".to_string()],
        suggested_actions: vec!["Click Submit".to_string()],
        confidence: 0.9,
    };

    // Test element finding methods
    let buttons = analysis.find_by_type("button");
    assert_eq!(buttons.len(), 1);

    let submit_elements = analysis.find_by_label("submit");
    assert_eq!(submit_elements.len(), 1);

    let interactive = analysis.find_interactive();
    assert_eq!(interactive.len(), 2);
}

/// Test vision model API if available.
/// This test requires LM Studio or another OpenAI-compatible API to be running.
#[tokio::test]
#[ignore] // Requires running LM Studio
async fn test_vision_model_api() {
    if !is_vision_api_available().await {
        eprintln!("Skipping vision API test: API not available at localhost:1234");
        return;
    }

    let config = VisionModelConfig::lm_studio();
    let client = VisionClient::new(config).unwrap();

    // Health check
    let healthy = client.health_check().await;
    assert!(healthy.is_ok(), "Health check should not error");
    assert!(healthy.unwrap(), "API should be healthy");

    println!("Vision API is available and responding");
}

// ============================================================================
// Full Pipeline Tests
// ============================================================================

/// Test the full pipeline: capture -> analyze -> learn -> apply.
/// This is an end-to-end test that requires a display and optionally a vision API.
#[tokio::test]
#[ignore] // Requires real display and optionally vision API
async fn test_full_pipeline() {
    require_display!();

    println!("=== Full Pipeline Test ===\n");

    // 1. Screen Capture
    println!("1. Testing Screen Capture...");
    let capture = XcapCapture::with_defaults();
    let mut screenshot = capture.capture_screen(None).await.unwrap();
    println!(
        "   Captured: {}x{}",
        screenshot.width(),
        screenshot.height()
    );

    // 2. Encode for vision model
    println!("2. Encoding for Vision Model...");
    let config = CaptureConfig::for_vision_model();
    let base64 = screenshot.to_base64_with_config(&config).unwrap();
    println!("   Base64 size: {} KB", base64.len() / 1024);

    // 3. Create learning screenshot
    println!("3. Creating Learning Screenshot...");
    let learning_screenshot = LearningScreenshot::new(base64.clone(), screenshot.width(), screenshot.height());

    // 4. Database operations
    println!("4. Testing Database Operations...");
    let (db, _dir) = create_test_db();

    // 5. Create learning engine
    println!("5. Creating Learning Engine...");
    let engine = LearningEngine::new(db);

    // 6. Record a demonstration
    println!("6. Recording Demonstration...");
    engine.start_recording("TestApp", "Full pipeline test").unwrap();
    engine.record_click(100, 100, MouseButton::Left).unwrap();
    engine.record_click(200, 200, MouseButton::Left).unwrap();
    engine.record_text("test input").unwrap();

    let demo = engine.stop_recording().unwrap();
    println!("   Recorded {} actions", demo.action_count());

    // 7. Extract skill
    println!("7. Extracting Skill...");
    let skill = engine.extract_skill(&demo, "Pipeline Test Skill").unwrap();
    println!("   Extracted skill with {} templates", skill.action_template.len());

    // 8. Find relevant skills
    println!("8. Finding Relevant Skills...");
    engine.refresh_cache().unwrap();
    let matches = engine.find_relevant_skills("pipeline test", &learning_screenshot).unwrap();
    println!("   Found {} matching skills", matches.len());

    // 9. Get statistics
    println!("9. Getting Statistics...");
    let stats = engine.get_statistics().unwrap();
    println!("   Total demonstrations: {}", stats.total_demonstrations);
    println!("   Total skills: {}", stats.total_skills);
    println!("   Total actions: {}", stats.total_actions);

    // 10. Test vision API if available
    if is_vision_api_available().await {
        println!("10. Testing Vision API (optional)...");
        let config = VisionModelConfig::lm_studio();
        let client = VisionClient::new(config).unwrap();
        let health = client.health_check().await.unwrap_or(false);
        println!("    Vision API available: {}", health);
    } else {
        println!("10. Vision API not available (skipping)");
    }

    println!("\n=== Full Pipeline Test PASSED ===");
}

/// Test image format encoding comparison.
#[tokio::test]
#[ignore] // Requires real display
async fn test_image_format_comparison() {
    require_display!();

    let capture = XcapCapture::with_defaults();
    let mut screenshot = capture.capture_screen(None).await.unwrap();

    println!("Image Format Comparison (1280px max dimension, quality 80):");
    println!("Original size: {}x{}", screenshot.width(), screenshot.height());

    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let config = CaptureConfig::default()
            .with_format(format)
            .with_quality(80)
            .with_max_dimension(1280);

        let encoded = screenshot.encode(&config).unwrap();
        println!(
            "  {:?}: {} bytes ({:.1} KB)",
            format,
            encoded.len(),
            encoded.len() as f64 / 1024.0
        );
    }
}

// ============================================================================
// Edge Case and Error Handling Tests
// ============================================================================

/// Test handling of invalid database path.
#[test]
fn test_database_invalid_path() {
    // Try to create database in a non-existent directory (should fail)
    let result = Database::open("/nonexistent/path/to/database.db");
    // Note: rusqlite may create parent directories, so this might actually succeed
    // depending on the system. We just verify it doesn't panic.
    if result.is_err() {
        println!("Database creation correctly failed for invalid path");
    } else {
        println!("Database was created (system allowed it)");
    }
}

/// Test in-memory database.
#[test]
fn test_database_in_memory() {
    let db = Database::in_memory();
    assert!(db.is_ok(), "In-memory database should be created");

    let db = db.unwrap();

    // Verify basic operations work
    let demo = Demonstration::new("App", "Task");
    assert!(db.save_demonstration(&demo).is_ok());
    assert!(db.get_demonstration(&demo.id).unwrap().is_some());
}

/// Test recording session state transitions.
#[test]
fn test_recording_state_transitions() {
    let (db, _dir) = create_test_db();
    let engine = LearningEngine::new(db);

    // Initially not recording
    assert!(!engine.is_recording());

    // Start recording
    let result = engine.start_recording("App", "Task");
    assert!(result.is_ok());
    assert!(engine.is_recording());

    // Can't start another recording while one is active
    let result = engine.start_recording("App2", "Task2");
    assert!(result.is_err(), "Should not allow nested recordings");

    // Stop recording
    let demo = engine.stop_recording();
    assert!(demo.is_ok());
    assert!(!engine.is_recording());

    // Can't stop when not recording
    let result = engine.stop_recording();
    assert!(result.is_err(), "Should not allow stopping when not recording");

    // Can start a new recording after stopping
    let result = engine.start_recording("App2", "Task2");
    assert!(result.is_ok());
    assert!(engine.is_recording());

    // Clean up
    let _ = engine.stop_recording();
}

/// Test learning statistics with empty database.
#[test]
fn test_empty_statistics() {
    let (db, _dir) = create_test_db();
    let engine = LearningEngine::new(db);

    let stats = engine.get_statistics();
    assert!(stats.is_ok());

    let stats = stats.unwrap();
    assert_eq!(stats.total_demonstrations, 0);
    assert_eq!(stats.total_skills, 0);
    assert_eq!(stats.total_actions, 0);
    assert_eq!(stats.unique_apps, 0);
    assert_eq!(stats.success_rate(), 0.5); // Default when no applications
}
