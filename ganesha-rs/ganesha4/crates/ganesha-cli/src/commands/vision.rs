//! # Vision Command
//!
//! Learning-from-demonstration and vision control commands.

use crate::cli::VisionAction;
use colored::Colorize;
use ganesha_learning::{Database, LearningEngine};
use std::path::PathBuf;

/// Get the default database path
fn get_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ganesha")
        .join("learning.db")
}

/// Run the vision command
pub async fn run(action: VisionAction) -> anyhow::Result<()> {
    match action {
        VisionAction::Record { app, description } => {
            let db_path = get_db_path();

            // Ensure parent directory exists
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let db = Database::open(&db_path)?;
            let engine = LearningEngine::new(db);

            let desc = description.unwrap_or_else(|| format!("Demonstration in {}", app));

            println!(
                "{} Starting demonstration recording for {}",
                "📹".bright_green(),
                app.bright_yellow()
            );
            println!("   Description: {}", desc.dimmed());
            println!();

            match engine.start_recording(&app, &desc) {
                Ok(session_id) => {
                    println!("{} Recording started!", "✓".green());
                    println!("   Session ID: {}", session_id.bright_cyan());
                    println!();
                    println!("Perform your actions, then run {} to stop.", "ganesha vision stop".bright_green());

                    // Store session ID in a temp file for stop command
                    let session_file = dirs::cache_dir()
                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                        .join("ganesha_recording_session");
                    std::fs::write(&session_file, session_id.to_string())?;
                }
                Err(e) => {
                    println!("{} Failed to start recording: {}", "✗".red(), e);
                }
            }
        }

        VisionAction::Stop => {
            let db_path = get_db_path();

            if !db_path.exists() {
                println!("{} No active recording session found", "!".yellow());
                return Ok(());
            }

            let db = Database::open(&db_path)?;
            let engine = LearningEngine::new(db);

            // Check if there's an active recording
            let session_file = dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("ganesha_recording_session");

            if !session_file.exists() {
                println!("{} No active recording session found", "!".yellow());
                return Ok(());
            }

            println!("{} Stopping demonstration recording...", "⏹".bright_yellow());

            match engine.stop_recording() {
                Ok(demo) => {
                    println!("{} Recording stopped!", "✓".green());
                    println!();
                    println!("  Demonstration: {}", demo.task_description.bright_cyan());
                    println!("  Application: {}", demo.app_name.bright_yellow());
                    println!("  Actions recorded: {}", demo.actions.len().to_string().bright_yellow());
                    println!("  Duration: {:.1}s", demo.duration_ms as f64 / 1000.0);
                    println!();

                    // Extract skill
                    println!("Extracting skill from demonstration...");
                    match engine.extract_skill(&demo, &demo.task_description) {
                        Ok(skill) => {
                            println!("{} Skill extracted!", "✓".green());
                            println!("  Skill ID: {}", skill.id.bright_cyan());
                            println!("  Name: {}", skill.name);
                            println!();
                            println!("You can now use this skill with:");
                            println!("  {}", format!("ganesha vision skill {}", skill.id).bright_green());
                        }
                        Err(e) => {
                            println!("{} Failed to extract skill: {}", "!".yellow(), e);
                            println!("  The demonstration was saved but skill extraction failed.");
                        }
                    }

                    // Clean up session file
                    let _ = std::fs::remove_file(&session_file);
                }
                Err(e) => {
                    println!("{} Failed to stop recording: {}", "✗".red(), e);
                }
            }
        }

        VisionAction::Skills { app } => {
            let db_path = get_db_path();

            if !db_path.exists() {
                println!("{}", "No skills found".dimmed());
                println!();
                println!("Start by recording a demonstration:");
                println!("  {}", "ganesha vision record <app-name>".bright_green());
                return Ok(());
            }

            let db = Database::open(&db_path)?;

            println!("{}", "Saved Skills".bright_cyan().bold());
            println!();

            let skills = db.list_skills(false)?;

            // Filter by app if specified
            let skills: Vec<_> = if let Some(ref app_name) = app {
                skills.into_iter()
                    .filter(|s| s.applicable_apps.iter().any(|a| a.to_lowercase().contains(&app_name.to_lowercase())))
                    .collect()
            } else {
                skills
            };

            if skills.is_empty() {
                println!("{}", "No skills found".dimmed());
            } else {
                for skill in skills {
                    let id_short = if skill.id.len() > 8 { &skill.id[..8] } else { &skill.id };
                    println!(
                        "  {} {} ({})",
                        "•".bright_cyan(),
                        skill.name,
                        id_short.dimmed()
                    );
                    let apps = if skill.applicable_apps.is_empty() {
                        "any".to_string()
                    } else {
                        skill.applicable_apps.join(", ")
                    };
                    println!(
                        "    Apps: {} | Templates: {} | Success rate: {:.0}%",
                        apps.bright_yellow(),
                        skill.action_template.len(),
                        skill.success_rate() * 100.0
                    );
                }
            }
        }

        VisionAction::Skill { id } => {
            let db_path = get_db_path();

            if !db_path.exists() {
                println!("{} Skill not found", "✗".red());
                return Ok(());
            }

            let db = Database::open(&db_path)?;

            match db.get_skill(&id) {
                Ok(Some(skill)) => {
                    println!("{}", "Skill Details".bright_cyan().bold());
                    println!();
                    println!("  ID: {}", skill.id.bright_cyan());
                    println!("  Name: {}", skill.name.bright_yellow());
                    println!("  Description: {}", if skill.description.is_empty() { "N/A".to_string() } else { skill.description.clone() }.dimmed());
                    let apps = if skill.applicable_apps.is_empty() {
                        "any".to_string()
                    } else {
                        skill.applicable_apps.join(", ")
                    };
                    println!("  Applicable apps: {}", apps);
                    println!("  Action templates: {}", skill.action_template.len());
                    println!("  Uses: {} ({} success, {} failure)",
                        skill.success_count + skill.failure_count,
                        skill.success_count,
                        skill.failure_count
                    );
                    println!("  Success rate: {:.0}%", skill.success_rate() * 100.0);
                    println!("  Confidence: {:.0}%", skill.confidence * 100.0);
                    println!();

                    if !skill.trigger_patterns.is_empty() {
                        println!("  Trigger patterns:");
                        for pattern in &skill.trigger_patterns {
                            println!("    • {}", pattern);
                        }
                        println!();
                    }

                    if !skill.action_template.is_empty() {
                        println!("  Action templates:");
                        for (i, template) in skill.action_template.iter().enumerate() {
                            let target = template.target_pattern.as_deref().unwrap_or("any");
                            println!(
                                "    {}. {:?} -> {}",
                                i + 1,
                                template.action_type,
                                target.bright_green()
                            );
                        }
                    }
                }
                Ok(None) => {
                    println!("{} Skill not found: {}", "✗".red(), id);
                }
                Err(e) => {
                    println!("{} Error loading skill: {}", "✗".red(), e);
                }
            }
        }

        VisionAction::Delete { id } => {
            let db_path = get_db_path();

            if !db_path.exists() {
                println!("{} Skill not found", "✗".red());
                return Ok(());
            }

            let db = Database::open(&db_path)?;

            match db.delete_skill(&id) {
                Ok(true) => {
                    println!("{} Skill deleted: {}", "✓".green(), id);
                }
                Ok(false) => {
                    println!("{} Skill not found: {}", "✗".red(), id);
                }
                Err(e) => {
                    println!("{} Error deleting skill: {}", "✗".red(), e);
                }
            }
        }

        VisionAction::Test => {
            println!("{}", "Vision System Test".bright_cyan().bold());
            println!();

            #[cfg(feature = "gui-automation")]
            {
                use ganesha_learning::capture::{ScreenCapture, XcapCapture};

                println!("Initializing screen capture...");

                let capture = XcapCapture::with_defaults();

                if !capture.is_available() {
                    println!("{} Screen capture not available", "✗".red());
                    println!();
                    println!("Note: Vision requires a display server (X11/Wayland).");
                } else {
                    println!("{} Screen capture initialized", "✓".green());

                    println!("Capturing screenshot...");
                    match capture.capture_screen(None).await {
                        Ok(screenshot) => {
                            println!("{} Screenshot captured", "✓".green());
                            println!();
                            println!("  Resolution: {}x{}", screenshot.width(), screenshot.height());
                            println!("  Format: {:?}", screenshot.metadata.format);
                            println!("  Capture time: {}ms", screenshot.metadata.capture_time_ms);

                            // List monitors
                            println!();
                            println!("Available monitors:");
                            match capture.get_monitors().await {
                                Ok(monitors) => {
                                    for m in monitors {
                                        let primary = if m.is_primary { " [PRIMARY]" } else { "" };
                                        println!("  • {} ({}x{}){}", m.name, m.region.width, m.region.height, primary.bright_cyan());
                                    }
                                }
                                Err(e) => println!("  Error listing monitors: {}", e),
                            }
                        }
                        Err(e) => {
                            println!("{} Capture failed: {}", "✗".red(), e);
                        }
                    }
                }
            }

            #[cfg(not(feature = "gui-automation"))]
            {
                println!("{} Vision features not compiled in", "!".yellow());
                println!("Rebuild with: cargo build --features gui-automation");
            }
        }

        VisionAction::Status => {
            println!("{}", "Vision System Status".bright_cyan().bold());
            println!();

            let db_path = get_db_path();

            // Database status
            if db_path.exists() {
                println!("  {} Database: {} {}", "•".bright_cyan(), db_path.display(), "(exists)".green());

                if let Ok(db) = Database::open(&db_path) {
                    if let Ok(skills) = db.list_skills(false) {
                        println!("    Skills stored: {}", skills.len().to_string().bright_yellow());
                    }
                }
            } else {
                println!("  {} Database: {} {}", "•".bright_cyan(), db_path.display(), "(not created)".dimmed());
            }

            // Recording status
            let session_file = dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("ganesha_recording_session");

            if session_file.exists() {
                if let Ok(session_id) = std::fs::read_to_string(&session_file) {
                    println!("  {} Recording: {} {}", "•".bright_cyan(), "ACTIVE".bright_red().bold(), format!("({})", session_id.trim()).dimmed());
                }
            } else {
                println!("  {} Recording: {}", "•".bright_cyan(), "inactive".dimmed());
            }

            // Display status
            if let Ok(display) = std::env::var("DISPLAY") {
                println!("  {} Display: {}", "•".bright_cyan(), display.bright_green());
            } else if let Ok(wayland) = std::env::var("WAYLAND_DISPLAY") {
                println!("  {} Wayland: {}", "•".bright_cyan(), wayland.bright_green());
            } else {
                println!("  {} Display: {}", "•".bright_cyan(), "not detected".yellow());
            }
        }

        VisionAction::Control { task, speed } => {
            println!("{} Vision control mode", "🖥".bright_cyan().bold());
            println!();
            println!("  Task: {}", task.bright_yellow());
            println!("  Speed: {}", speed.bright_green());
            println!();

            println!("{} Vision control mode is not yet implemented", "!".yellow());
            println!();
            println!("When implemented, Ganesha will:");
            println!("  1. Capture screenshots continuously");
            println!("  2. Analyze the screen with a vision model");
            println!("  3. Plan actions to accomplish the task");
            println!("  4. Execute mouse/keyboard actions");
            println!("  5. Verify progress and repeat until done");
            println!();
            println!("A red border will appear around your screen when Ganesha");
            println!("is in control, and you can press {} to stop.", "ESC".bright_red().bold());
        }

        VisionAction::Capture => {
            println!("{} Capturing and analyzing screen...", "📸".bright_cyan());
            println!();

            #[cfg(feature = "gui-automation")]
            {
                use ganesha_learning::capture::{ScreenCapture, XcapCapture};

                let capture = XcapCapture::with_defaults();

                if !capture.is_available() {
                    println!("{} Screen capture not available", "✗".red());
                } else {
                    match capture.capture_screen(None).await {
                        Ok(mut screenshot) => {
                            println!("  {} Captured: {}x{} pixels", "✓".green(), screenshot.width(), screenshot.height());

                            // Encode and save to temp file
                            let config = ganesha_learning::capture::CaptureConfig::default();
                            match screenshot.encode(&config) {
                                Ok(encoded) => {
                                    println!("  {} Size: {:.1} KB", "✓".green(), encoded.len() as f64 / 1024.0);

                                    let temp_path = std::env::temp_dir().join("ganesha_capture.png");
                                    if let Ok(_) = std::fs::write(&temp_path, &encoded) {
                                        println!();
                                        println!("  Saved to: {}", temp_path.display().to_string().bright_green());
                                    }
                                }
                                Err(e) => {
                                    println!("  {} Failed to encode: {}", "!".yellow(), e);
                                }
                            }

                            println!();
                            println!("{} Screen analysis requires a vision model connection", "!".yellow());
                        }
                        Err(e) => {
                            println!("{} Capture failed: {}", "✗".red(), e);
                        }
                    }
                }
            }

            #[cfg(not(feature = "gui-automation"))]
            {
                println!("{} Vision features not compiled in", "!".yellow());
            }
        }

        VisionAction::Stats => {
            println!("{}", "Learning Statistics".bright_cyan().bold());
            println!();

            let db_path = get_db_path();

            if !db_path.exists() {
                println!("{}", "No learning data yet".dimmed());
                return Ok(());
            }

            let db = Database::open(&db_path)?;
            let engine = LearningEngine::new(db);

            let stats = engine.get_statistics()?;

            println!("  Total demonstrations: {}", stats.total_demonstrations.to_string().bright_yellow());
            println!("  Total skills: {} ({} enabled)",
                stats.total_skills.to_string().bright_yellow(),
                stats.enabled_skills
            );
            println!("  Total actions recorded: {}", stats.total_actions.to_string().bright_yellow());
            println!("  Skill applications: {} ({} success, {} failure)",
                stats.total_applications,
                stats.total_successes.to_string().bright_green(),
                stats.total_failures.to_string().bright_red()
            );
            println!();

            if !stats.apps.is_empty() {
                println!("  Applications with demonstrations ({}):", stats.unique_apps);
                for app in &stats.apps {
                    println!("    • {}", app.bright_green());
                }
            }

            println!();
            let success_rate = if stats.total_applications > 0 {
                stats.total_successes as f32 / stats.total_applications as f32 * 100.0
            } else {
                0.0
            };
            println!("  Average skill success rate: {:.0}%", success_rate);
            println!("  Average skill confidence: {:.0}%", stats.average_skill_confidence * 100.0);
        }
    }

    Ok(())
}
