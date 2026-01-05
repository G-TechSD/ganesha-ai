//! Ralph Swarm Architecture - Parallel Processing with Weighted Interest
//!
//! 🦅 EAGLE FLOCK: Multiple vision queries in parallel, filtered by relevance
//! 🐜 ANT SWARM: Parallel DOM queries, aggregated into unified intel
//! 🐦 HUMMINGBIRD: Gimbal-stabilized extraction from moving/flaky targets
//! 📺 NVR ZONES: Ignore static UI, ads, whitespace - only process motion/focus
//! 📋 DOSSIER: Complete system situational awareness
//! 💾 MEMORY: SpacetimeDB-backed temporal activity log
//! ⏱️ OVERLAY: Visible timer for human + AI awareness
//! 📚 DOCS: Dynamic documentation loading (Context7 + local)
//!
//! Like a flock of eagles spotting thousands of starlings but honing in on ONE,
//! we run many queries but filter to what's relevant to the goal.
//!
//! Run: cargo run --example ralph_swarm --features computer-use

use ganesha::vision::VisionController;
use ganesha::zones::{ZoneManager, detect_motion};
use ganesha::dossier::SystemDossier;
use ganesha::memory::TemporalMemory;
use ganesha::overlay::{ActivityOverlay, OverlayPosition};
use ganesha::docs::DocsLoader;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3-3b";
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// 🦅 Single Eagle's observation
#[derive(Debug, Clone)]
struct EagleObservation {
    query_type: String,     // "layout", "text", "obstacles", "progress"
    finding: String,
    relevance: f32,         // 0.0-1.0 relevance to current goal
    confidence: f32,
}

/// 🦅 Aggregated flock intel
#[derive(Debug, Clone)]
struct FlockIntel {
    situation: String,
    key_findings: Vec<String>,  // Filtered to relevant only
    anomalies: Vec<String>,
    timestamp: Instant,
}

/// 🐜 Single Ant's report
#[derive(Debug, Clone)]
struct AntFinding {
    query_type: String,     // "state", "links", "items", "forms", "buttons"
    data: serde_json::Value,
    relevance: f32,
}

/// 🐜 Aggregated swarm intel
#[derive(Debug, Clone)]
struct SwarmIntel {
    url: String,
    title: String,
    /// Clean markdown content (Markdowser-style)
    markdown_content: String,
    /// Structured actionable elements
    buttons: Vec<String>,
    links: Vec<String>,
    inputs: Vec<String>,
    /// Filtered to relevant
    relevant_elements: Vec<String>,
    actionable_targets: Vec<String>,
}

/// 🐦 Hummingbird extraction result
#[derive(Debug, Clone)]
struct NectarExtract {
    target: String,
    value: Option<String>,
    attempts: usize,
    stabilized: bool,
}

/// 📺 NVR Zone status - what changed, what to ignore
#[derive(Debug, Clone)]
struct NvrStatus {
    changed_zones: Vec<String>,
    ignored_zones: Vec<String>,
    focus_zone: Option<String>,
    motion_detected: bool,
}

/// Combined intel from all sources
#[derive(Debug, Clone)]
struct UnifiedIntel {
    flock: FlockIntel,
    swarm: SwarmIntel,
    nvr: NvrStatus,
    extracts: Vec<NectarExtract>,
    goal_progress: f32,     // 0.0-1.0 estimated progress toward goal
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║              GANESHA SWARM - Full Integration                     ║");
    println!("║                                                                   ║");
    println!("║   🦅 EAGLE FLOCK: Parallel vision, relevance filtered             ║");
    println!("║   🐜 ANT SWARM: Parallel DOM queries, aggregated intel            ║");
    println!("║   🐦 HUMMINGBIRD: Gimbal-stabilized precision extraction          ║");
    println!("║   📋 DOSSIER: System situational awareness                        ║");
    println!("║   💾 MEMORY: Temporal activity log                                ║");
    println!("║   ⏱️  OVERLAY: Visible AI timer                                    ║");
    println!("║   📚 DOCS: Dynamic documentation (Context7 + local)               ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    // ══════════════════════════════════════════════════════════════════════════
    // INITIALIZE ALL SUBSYSTEMS
    // ══════════════════════════════════════════════════════════════════════════

    // 📋 System Dossier - get initial system state
    println!("[INIT] 📋 Collecting system dossier...");
    let dossier = SystemDossier::collect().unwrap_or_else(|e| {
        println!("  ⚠️  Dossier partial: {}", e);
        // Return a minimal dossier
        SystemDossier::collect().unwrap() // Try again or panic
    });
    println!("  OS: {} {} ({})", dossier.os.name, dossier.os.version, dossier.os.desktop_env);
    println!("  Display: {}x{}", dossier.display.width, dossier.display.height);
    println!("  Windows: {}", dossier.windows.len());

    // Ensure browser is running
    ensure_browser()?;

    // 👁️ Vision Controller
    println!("[INIT] 👁️ Enabling vision...");
    let vision = Arc::new(VisionController::new());
    vision.enable().map_err(|e| format!("Vision: {}", e))?;

    // 🌐 HTTP Client
    let client = Arc::new(reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?);

    // 💾 Temporal Memory
    println!("[INIT] 💾 Initializing temporal memory...");
    let memory = Arc::new(TemporalMemory::new(500)); // Keep last 500 entries

    // ⏱️ Activity Overlay
    println!("[INIT] ⏱️ Starting activity overlay...");
    let mut overlay = ActivityOverlay::new(OverlayPosition::TopRight);
    if let Err(e) = overlay.start() {
        println!("  ⚠️  Overlay unavailable: {} (continuing without)", e);
    }

    // 📚 Documentation Loader
    println!("[INIT] 📚 Loading documentation providers...");
    let docs_loader = Arc::new(DocsLoader::default());

    // 📺 NVR Zone Manager
    println!("[INIT] 📺 Initializing zone manager...");
    let zone_manager = Arc::new(RwLock::new(ZoneManager::new(
        dossier.display.width,
        dossier.display.height,
    )));

    // ══════════════════════════════════════════════════════════════════════════
    // GET MISSION
    // ══════════════════════════════════════════════════════════════════════════

    println!("\nWhat's the mission?");
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim().to_string();

    if goal.is_empty() {
        overlay.stop();
        return Ok(());
    }

    println!("\n[MISSION] {}\n", goal);
    overlay.set_goal(&goal);
    overlay.update("Starting mission", "working", 0);

    // Extract goal keywords for relevance scoring
    let goal_keywords = extract_keywords(&goal);
    println!("[KEYWORDS] {:?}\n", goal_keywords);

    // 📚 Load relevant documentation
    println!("[DOCS] Loading context-aware documentation...");
    let focused_app = dossier.focused_window()
        .map(|w| w.app_name.clone())
        .unwrap_or_else(|| "browser".into());
    let docs = docs_loader.get_context_docs(
        &focused_app,
        &dossier.os.name,
        &dossier.os.desktop_env,
        &goal,
    ).await;
    if !docs.is_empty() {
        println!("  Loaded {} doc snippets for {}", docs.len(), focused_app);
    }
    let docs_context = DocsLoader::format_for_context(&docs, 1500);

    // Record mission start in memory
    memory.record_goal_progress(&goal, goal_keywords.clone(), 0.0, 0, 0, "started");

    let result = execute_swarm_mission(
        client,
        vision.clone(),
        zone_manager,
        memory.clone(),
        Arc::new(overlay),
        &goal,
        &goal_keywords,
        &docs_context,
    ).await;

    // Record final status
    match &result {
        Ok(summary) => {
            memory.record_goal_progress(&goal, goal_keywords, 1.0, 99, 0, "achieved");
            println!("\n✓ MISSION COMPLETE: {}", summary);
        }
        Err(e) => {
            memory.record_goal_progress(&goal, goal_keywords, 0.0, 99, 0, "failed");
            println!("\n✗ MISSION FAILED: {}", e);
        }
    }

    // Print memory stats
    println!("\n📊 {}", memory.stats());

    // Show context that was built (for debugging)
    let final_context = memory.generate_context(&goal, 1000);
    println!("\n💾 TEMPORAL MEMORY CONTEXT:\n{}", final_context);

    // Cleanup
    vision.disable();

    Ok(())
}

/// Extract keywords from goal for relevance scoring
fn extract_keywords(goal: &str) -> Vec<String> {
    let stopwords = ["the", "a", "an", "for", "on", "in", "to", "and", "or", "of", "me", "i", "find", "search", "look", "get"];
    goal.to_lowercase()
        .split_whitespace()
        .filter(|w| !stopwords.contains(w) && w.len() > 2)
        .map(|s| s.to_string())
        .collect()
}

/// Score relevance of text to goal keywords
fn score_relevance(text: &str, keywords: &[String]) -> f32 {
    let text_lower = text.to_lowercase();
    let matches: usize = keywords.iter()
        .filter(|kw| text_lower.contains(kw.as_str()))
        .count();

    if keywords.is_empty() {
        return 0.5;
    }
    (matches as f32 / keywords.len() as f32).min(1.0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// SWARM MISSION EXECUTION
// ═══════════════════════════════════════════════════════════════════════════════

async fn execute_swarm_mission(
    client: Arc<reqwest::Client>,
    vision: Arc<VisionController>,
    zone_manager: Arc<RwLock<ZoneManager>>,
    memory: Arc<TemporalMemory>,
    overlay: Arc<ActivityOverlay>,
    goal: &str,
    keywords: &[String],
    docs_context: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut history: Vec<String> = Vec::new();
    let mut _last_screenshot_hash: u64 = 0;

    for step in 1..=15 {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[STEP {}]", step);

        // 🚧 GANESHA: Remove obstacles first
        let obstacles = remove_obstacles().await;
        if obstacles > 0 {
            println!("  🚧 GANESHA removed {} obstacles", obstacles);
        }

        // ══════════════════════════════════════════════════════════════════════
        // 📺 NVR ZONE DETECTION - What changed? What to ignore?
        // ══════════════════════════════════════════════════════════════════════

        // Get current screenshot for motion detection
        let screenshot = vision.capture_screen_scaled(1920, 1080)
            .map_err(|e| format!("Screenshot: {}", e))?;

        // Auto-detect preset based on current context
        let swarm_quick = ant_swarm_scout(keywords)?;
        {
            let mut zm = zone_manager.write().await;
            zm.auto_detect_preset(&swarm_quick.url, &swarm_quick.title);
        }

        // Detect motion in zones
        let (changed_zones, nvr_status) = {
            let mut zm = zone_manager.write().await;

            // Decode screenshot for motion detection (simplified - assume raw pixels available)
            // In production, we'd decode the PNG properly
            let changed = if let Ok(pixels) = decode_screenshot_pixels(&screenshot.data) {
                detect_motion(&mut zm, &pixels, 1920)
            } else {
                vec!["page_content".to_string()] // Fallback: assume main content changed
            };

            // Learn static zones over time
            zm.learn_static_zones(Duration::from_secs(30));

            let ignored: Vec<String> = zm.get_ignore_zones().iter().map(|z| z.id.clone()).collect();
            let focus = zm.zones.get("active_focus").map(|z| z.id.clone());

            let nvr = NvrStatus {
                changed_zones: changed.clone(),
                ignored_zones: ignored,
                focus_zone: focus,
                motion_detected: !changed.is_empty(),
            };

            (changed, nvr)
        };

        // Report NVR status
        if !changed_zones.is_empty() {
            println!("  📺 NVR: Motion in {:?}", changed_zones);
        } else {
            println!("  📺 NVR: No motion - screen stable");
        }

        // ══════════════════════════════════════════════════════════════════════
        // PARALLEL INTELLIGENCE GATHERING (only if motion or first step)
        // ══════════════════════════════════════════════════════════════════════

        let should_analyze = nvr_status.motion_detected || step == 1;

        let (flock, swarm) = if should_analyze {
            // Launch Eagle Flock and Ant Swarm in PARALLEL
            let client_flock = client.clone();
            let vision_flock = vision.clone();
            let keywords_flock = keywords.to_vec();
            let zones_flock = zone_manager.clone();

            let flock_handle = tokio::spawn(async move {
                eagle_flock_recon(&client_flock, &vision_flock, &keywords_flock, &zones_flock).await
            });

            let keywords_swarm = keywords.to_vec();
            let swarm_handle = tokio::spawn(async move {
                ant_swarm_scout(&keywords_swarm)
            });

            // Await both in parallel
            let (flock_result, swarm_result) = tokio::join!(flock_handle, swarm_handle);

            let flock = flock_result.map_err(|e| format!("Flock join: {}", e))?
                .map_err(|e| format!("Flock: {}", e))?;
            let swarm = swarm_result.map_err(|e| format!("Swarm join: {}", e))?
                .map_err(|e| format!("Swarm: {}", e))?;

            (flock, swarm)
        } else {
            // No motion - use cached/minimal data
            println!("  ⏸️  Skipping vision (no motion)");
            let flock = FlockIntel {
                situation: "Screen stable - no change".to_string(),
                key_findings: vec![],
                anomalies: vec![],
                timestamp: Instant::now(),
            };
            (flock, swarm_quick)
        };

        // Report intel
        if should_analyze {
            println!("  🦅 FLOCK: {}", flock.situation);
            if !flock.key_findings.is_empty() {
                for finding in &flock.key_findings[..flock.key_findings.len().min(3)] {
                    println!("     └─ {}", finding);
                }
            }
            if !flock.anomalies.is_empty() {
                println!("  ⚠️  ANOMALIES: {:?}", flock.anomalies);
            }
        }

        println!("  🐜 SWARM: {} | {}", swarm.title, swarm.url);
        if !swarm.actionable_targets.is_empty() {
            println!("     └─ Targets: {:?}", &swarm.actionable_targets[..swarm.actionable_targets.len().min(5)]);
        }

        // 💾 MEMORY: Record this snapshot
        let snapshot_id = memory.record_snapshot(
            &swarm.url,
            &swarm.title,
            0, // TODO: actual screen hash
            nvr_status.changed_zones.clone(),
            &flock.situation,
            &swarm.markdown_content.chars().take(500).collect::<String>(),
            flock.anomalies.clone(),
        );

        // ══════════════════════════════════════════════════════════════════════
        // GOAL CHECK
        // ══════════════════════════════════════════════════════════════════════

        let unified = UnifiedIntel {
            flock: flock.clone(),
            swarm: swarm.clone(),
            nvr: nvr_status.clone(),
            extracts: vec![],
            goal_progress: estimate_progress(goal, &flock, &swarm),
        };

        println!("  📊 PROGRESS: {:.0}%", unified.goal_progress * 100.0);

        // ⏱️ Update overlay
        overlay.update(
            &format!("Step {} - {:.0}%", step, unified.goal_progress * 100.0),
            "working",
            (unified.goal_progress * 100.0) as u8,
        );

        // 💾 Record progress
        memory.record_goal_progress(
            goal,
            keywords.to_vec(),
            unified.goal_progress,
            step,
            snapshot_id,
            if unified.goal_progress >= 0.9 { "achieved" } else { "in_progress" },
        );

        if unified.goal_progress >= 0.9 {
            println!("\n  ✅ GOAL ACHIEVED!");
            overlay.action_completed("GOAL ACHIEVED");
            return Ok(format!("Mission complete in {} steps", step));
        }

        // Check if stuck (using temporal memory)
        if memory.is_stuck(goal, 3) {
            println!("  ⚠️  STUCK DETECTED: No progress in last 3 steps");
            overlay.update("Stuck - trying alternative", "stuck", (unified.goal_progress * 100.0) as u8);
        }

        // ══════════════════════════════════════════════════════════════════════
        // DECISION (with docs context and memory context)
        // ══════════════════════════════════════════════════════════════════════

        let memory_context = memory.generate_context(goal, 500);
        let action = planner_decide(&client, goal, &unified, &history, docs_context, &memory_context).await?;
        println!("  🧠 DECIDE: {} {}", action.0, action.1);

        if action.0 == "DONE" {
            overlay.action_completed("DONE");
            return Ok(format!("Mission complete in {} steps", step));
        }

        // Check if we've tried this action recently (loop detection)
        if memory.has_tried_action(&action.0, &action.1, 30) {
            println!("  ⚠️  LOOP DETECTED: Already tried {} {} recently", action.0, action.1);
        }

        // ══════════════════════════════════════════════════════════════════════
        // EXECUTION (with Hummingbird precision if needed)
        // ══════════════════════════════════════════════════════════════════════

        let action_start = Instant::now();
        overlay.update(&format!("{} {}", action.0, action.1), "working", (unified.goal_progress * 100.0) as u8);

        let (exec_success, exec_result, exec_error) = if action.0 == "EXTRACT" {
            // 🐦 Hummingbird for precise extraction
            let nectar = hummingbird_extract(&action.1, 3).await;
            println!("  🐦 HUMMINGBIRD: {} (attempts: {}, stable: {})",
                nectar.value.as_deref().unwrap_or("failed"),
                nectar.attempts,
                nectar.stabilized);
            (nectar.value.is_some(), nectar.value.unwrap_or_default(), None)
        } else {
            // 🐜 Standard ant execution
            match ant_execute(&action.0, &action.1) {
                Ok(v) => {
                    println!("  🐜 EXECUTE: {}", v.ant_says);
                    (v.success, v.ant_says, None)
                }
                Err(e) => {
                    println!("  ❌ FAILED: {}", e);
                    (false, String::new(), Some(e))
                }
            }
        };
        let action_duration = action_start.elapsed();

        // 💾 Record action in memory
        memory.record_action(
            snapshot_id,
            &action.0,
            &action.1,
            exec_success,
            &exec_result,
            true, // TODO: eagle verification
            exec_error.as_deref(),
            action_duration.as_millis() as u64,
        );

        // ⏱️ Mark action completed (resets timer)
        overlay.action_completed(&format!("{} {}", action.0, action.1));

        let result = exec_success;

        history.push(format!("{} {} {}", action.0, action.1, if result { "✓" } else { "✗" }));

        // Brief pause for page to settle
        sleep(Duration::from_millis(800)).await;
    }

    Err("Max steps reached".into())
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Decode base64 PNG to raw RGBA pixels (simplified)
fn decode_screenshot_pixels(base64_data: &str) -> Result<Vec<u8>, String> {
    use std::io::Cursor;

    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        base64_data
    ).map_err(|e| format!("Base64 decode: {}", e))?;

    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| format!("PNG read: {}", e))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut buf).map_err(|e| format!("PNG frame: {}", e))?;

    Ok(buf)
}

// ═══════════════════════════════════════════════════════════════════════════════
// 🦅 EAGLE FLOCK - Parallel Vision Queries
// ═══════════════════════════════════════════════════════════════════════════════

async fn eagle_flock_recon(
    client: &reqwest::Client,
    vision: &VisionController,
    keywords: &[String],
    zone_manager: &Arc<RwLock<ZoneManager>>,
) -> Result<FlockIntel, Box<dyn std::error::Error + Send + Sync>> {
    // Get active zones to tell vision what to focus on
    let focus_hint = {
        let zm = zone_manager.read().await;
        let active: Vec<_> = zm.get_active_zones().iter().map(|z| z.id.clone()).collect();
        if active.is_empty() {
            "full screen".to_string()
        } else {
            format!("Focus on: {:?}", active)
        }
    };

    // Capture screenshot once, send to multiple analysis queries
    let screenshot = vision.capture_screen_scaled(1280, 720)
        .map_err(|e| format!("Screenshot: {}", e))?;

    let screenshot_data = screenshot.data.clone();

    // Define multiple eagle queries (different perspectives on same image)
    let queries = vec![
        ("situation", "What app/website is shown? What page? One sentence."),
        ("content", "What content is visible? List key items, products, or information."),
        ("obstacles", "Any popups, modals, cookie banners, errors, or loading states? List or say 'none'."),
        ("actions", "What interactive elements are visible? Buttons, links, search boxes?"),
    ];

    // For efficiency, we'll run ONE vision call with a comprehensive prompt
    // (Multiple calls would be too slow - the model can answer multiple questions at once)
    let combined_prompt = format!(
        r#"Analyze this screen. Answer each briefly:
1. SITUATION: What app/page is this?
2. CONTENT: Key visible items/products (keywords: {:?})
3. OBSTACLES: Any popups, errors, loading? (or "none")
4. ACTIONS: Visible buttons, links, inputs?

Format as: 1: ... | 2: ... | 3: ... | 4: ..."#,
        keywords
    );

    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {"role": "system", "content": "Concise screen analyst. Answer all questions briefly."},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": combined_prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot_data)
                    }}
                ]
            }
        ],
        "max_tokens": 300,
        "temperature": 0.0
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown");

    // Parse the numbered responses
    let mut situation = String::new();
    let mut key_findings = Vec::new();
    let mut anomalies = Vec::new();

    for part in content.split('|') {
        let part = part.trim();
        if part.starts_with("1:") || part.to_uppercase().starts_with("SITUATION") {
            situation = part.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
        } else if part.starts_with("2:") || part.to_uppercase().starts_with("CONTENT") {
            let content_text = part.splitn(2, ':').nth(1).unwrap_or("").trim();
            // Filter findings by relevance
            for item in content_text.split(',') {
                let item = item.trim();
                let relevance = score_relevance(item, keywords);
                if relevance > 0.2 || item.len() > 5 {
                    key_findings.push(item.to_string());
                }
            }
        } else if part.starts_with("3:") || part.to_uppercase().starts_with("OBSTACLE") {
            let obs = part.splitn(2, ':').nth(1).unwrap_or("").trim().to_lowercase();
            if !obs.contains("none") && !obs.is_empty() {
                anomalies.push(obs);
            }
        }
        // Part 4 (actions) informs swarm, not stored separately
    }

    Ok(FlockIntel {
        situation,
        key_findings,
        anomalies,
        timestamp: Instant::now(),
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// 🐜 ANT SWARM - Parallel DOM Queries
// ═══════════════════════════════════════════════════════════════════════════════

fn ant_swarm_scout(keywords: &[String]) -> Result<SwarmIntel, String> {
    // 🐜 Swarm: Multiple ants gather different intel in parallel
    // Each ant specializes in a different type of data

    // Ant 1: Get basic state
    let state = playwright("get_state", &[])?;
    let url = state["url"].as_str().unwrap_or("").to_string();
    let title = state["title"].as_str().unwrap_or("").to_string();

    // Ant 2: Get clean markdown content (Markdowser-style)
    let markdown_content = if let Ok(md_result) = playwright("get_markdown", &[]) {
        md_result["markdown"].as_str().unwrap_or("").to_string()
    } else {
        String::new()
    };

    // Ant 3: Get structured actionable elements
    let (buttons, links, inputs) = if let Ok(struct_result) = playwright("get_structured", &[]) {
        let btns: Vec<String> = struct_result["buttons"]
            .as_array()
            .map(|arr| arr.iter()
                .filter_map(|b| b["text"].as_str().map(|s| s.to_string()))
                .collect())
            .unwrap_or_default();

        let lnks: Vec<String> = struct_result["links"]
            .as_array()
            .map(|arr| arr.iter()
                .filter_map(|l| {
                    let text = l["text"].as_str().unwrap_or("");
                    let href = l["href"].as_str().unwrap_or("");
                    if !text.is_empty() {
                        Some(format!("[{}]({})", text, href))
                    } else {
                        None
                    }
                })
                .collect())
            .unwrap_or_default();

        let inps: Vec<String> = struct_result["inputs"]
            .as_array()
            .map(|arr| arr.iter()
                .filter_map(|i| {
                    let placeholder = i["placeholder"].as_str().unwrap_or("");
                    let name = i["name"].as_str().unwrap_or("");
                    let id = i["id"].as_str().unwrap_or("");
                    Some(format!("input#{} ({})", id, placeholder.chars().take(30).collect::<String>()))
                })
                .collect())
            .unwrap_or_default();

        (btns, lnks, inps)
    } else {
        (vec![], vec![], vec![])
    };

    // Filter for relevance using keywords
    let mut relevant_elements = Vec::new();
    let mut actionable_targets = Vec::new();

    // Score markdown content sections
    for line in markdown_content.lines() {
        let relevance = score_relevance(line, keywords);
        if relevance > 0.3 && line.len() > 10 {
            relevant_elements.push(line.to_string());
        }
    }

    // Score buttons
    for btn in &buttons {
        let relevance = score_relevance(btn, keywords);
        if relevance > 0.2 || btn.to_lowercase().contains("search") || btn.to_lowercase().contains("submit") {
            actionable_targets.push(format!("btn: {}", btn));
        }
    }

    // Score links
    for link in &links {
        let relevance = score_relevance(link, keywords);
        if relevance > 0.3 {
            actionable_targets.push(link.clone());
        }
    }

    // Ant 4: Site-specific structured data (eBay, Amazon, etc.)
    if url.contains("ebay") {
        if let Ok(items_result) = playwright("get_items", &[]) {
            if let Some(items) = items_result["items"].as_array() {
                for item in items.iter().take(10) {
                    let item_title = item["title"].as_str().unwrap_or("");
                    let price = item["price"].as_str().unwrap_or("");
                    let relevance = score_relevance(item_title, keywords);

                    if relevance > 0.2 {
                        relevant_elements.push(format!("📦 {} - {}", item_title, price));
                    }
                }
            }
        }
    }

    // Limit results to prevent noise
    relevant_elements.truncate(15);
    actionable_targets.truncate(10);

    Ok(SwarmIntel {
        url,
        title,
        markdown_content: if markdown_content.len() > 2000 {
            format!("{}...", &markdown_content[..2000])
        } else {
            markdown_content
        },
        buttons,
        links,
        inputs,
        relevant_elements,
        actionable_targets,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// 🐦 HUMMINGBIRD - Stabilized Precision Extraction
// ═══════════════════════════════════════════════════════════════════════════════

async fn hummingbird_extract(selector: &str, max_attempts: usize) -> NectarExtract {
    let mut attempts = 0;
    let mut last_value: Option<String> = None;
    let mut stable_count = 0;

    // Gimbal stabilization: retry until value is stable across 2 reads
    while attempts < max_attempts {
        attempts += 1;

        if let Ok(result) = playwright("get_text", &[selector]) {
            let value = result["text"].as_str().map(|s| s.trim().to_string());

            if value == last_value && value.is_some() {
                stable_count += 1;
                if stable_count >= 1 {
                    // Stable reading confirmed
                    return NectarExtract {
                        target: selector.to_string(),
                        value,
                        attempts,
                        stabilized: true,
                    };
                }
            } else {
                stable_count = 0;
            }
            last_value = value;
        }

        // Brief pause before retry (let animations settle)
        sleep(Duration::from_millis(200)).await;
    }

    NectarExtract {
        target: selector.to_string(),
        value: last_value,
        attempts,
        stabilized: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PROGRESS ESTIMATION & DECISION
// ═══════════════════════════════════════════════════════════════════════════════

fn estimate_progress(goal: &str, flock: &FlockIntel, swarm: &SwarmIntel) -> f32 {
    let goal_lower = goal.to_lowercase();

    // Search goal: are we on results page?
    if goal_lower.contains("search") {
        let on_results = swarm.url.contains("sch/i.html")
            || swarm.url.contains("/search")
            || swarm.title.to_lowercase().contains("for sale")
            || flock.situation.to_lowercase().contains("search results")
            || flock.situation.to_lowercase().contains("listings");

        if on_results && !swarm.relevant_elements.is_empty() {
            return 1.0;  // Goal achieved
        } else if on_results {
            return 0.8;  // On results but no relevant items found
        } else if swarm.url.contains("ebay") || swarm.url.contains("google") {
            return 0.3;  // At least on the right site
        }
    }

    // Default: no clear progress
    0.1
}

async fn planner_decide(
    client: &reqwest::Client,
    goal: &str,
    intel: &UnifiedIntel,
    history: &[String],
    docs_context: &str,
    memory_context: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let history_text = if history.is_empty() {
        "None".into()
    } else {
        history.join(" → ")
    };

    let tools = serde_json::json!([
        {"type": "function", "function": {
            "name": "search_ebay",
            "description": "Search eBay for products",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        }},
        {"type": "function", "function": {
            "name": "search_google",
            "description": "Search Google",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        }},
        {"type": "function", "function": {
            "name": "scroll",
            "description": "Scroll to see more content",
            "parameters": {"type": "object", "properties": {"direction": {"type": "string", "enum": ["up", "down"]}}, "required": ["direction"]}
        }},
        {"type": "function", "function": {
            "name": "click",
            "description": "Click an element",
            "parameters": {"type": "object", "properties": {"target": {"type": "string"}}, "required": ["target"]}
        }},
        {"type": "function", "function": {
            "name": "extract",
            "description": "Extract specific data with precision (hummingbird mode)",
            "parameters": {"type": "object", "properties": {"selector": {"type": "string"}}, "required": ["selector"]}
        }},
        {"type": "function", "function": {
            "name": "done",
            "description": "Mission complete - goal achieved",
            "parameters": {"type": "object", "properties": {}}
        }}
    ]);

    // Prepare concise markdown summary (first 500 chars of relevant content)
    let content_summary: String = intel.swarm.relevant_elements
        .iter()
        .take(5)
        .map(|s| format!("- {}", s))
        .collect::<Vec<_>>()
        .join("\n");

    // Build comprehensive context with docs and memory
    let context = format!(
        r#"MISSION: {}

INTEL REPORT:
🦅 EAGLE SEES: {}
   Anomalies: {:?}

🐜 ANT REPORTS:
   Page: {} ({})
   Content:
{}

   Buttons: {:?}
   Inputs: {:?}
   Links: {:?}

📺 NVR: Motion in {:?}, Ignoring {:?}
📊 Progress: {:.0}%

HISTORY: {}

{}

{}"#,
        goal,
        intel.flock.situation,
        intel.flock.anomalies,
        intel.swarm.title,
        intel.swarm.url,
        content_summary,
        intel.swarm.buttons.iter().take(5).collect::<Vec<_>>(),
        intel.swarm.inputs.iter().take(3).collect::<Vec<_>>(),
        intel.swarm.actionable_targets.iter().take(5).collect::<Vec<_>>(),
        intel.nvr.changed_zones,
        intel.nvr.ignored_zones,
        intel.goal_progress * 100.0,
        history_text,
        if !memory_context.is_empty() { format!("💾 MEMORY:\n{}", memory_context) } else { String::new() },
        if !docs_context.is_empty() { format!("📚 DOCS:\n{}", docs_context) } else { String::new() }
    );

    let system_prompt = r#"You are Ganesha - the Remover of Obstacles. You command reconnaissance forces to achieve the user's mission.

Your powers:
- 🦅 Eagle vision sees what humans cannot
- 🐜 Ant precision does what humans cannot
- 🐘 Elephant strength breaks down walls when needed
- 🚧 You remove obstacles (popups, cookies, errors) automatically

Pick ONE action to progress. If stuck, try alternative approaches. If progress >= 90%, use done."#;

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": context}
        ],
        "tools": tools,
        "tool_choice": "required",
        "max_tokens": 100,
        "temperature": 0.0
    });

    let response = client.post(PLANNER_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    if let Some(calls) = result["choices"][0]["message"]["tool_calls"].as_array() {
        if let Some(call) = calls.first() {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: serde_json::Value = serde_json::from_str(
                call["function"]["arguments"].as_str().unwrap_or("{}")
            ).unwrap_or_default();

            return Ok(match name {
                "search_ebay" => ("SEARCH_EBAY".into(), args["query"].as_str().unwrap_or("").into()),
                "search_google" => ("SEARCH_GOOGLE".into(), args["query"].as_str().unwrap_or("").into()),
                "scroll" => ("SCROLL".into(), args["direction"].as_str().unwrap_or("down").into()),
                "click" => ("CLICK".into(), args["target"].as_str().unwrap_or("").into()),
                "extract" => ("EXTRACT".into(), args["selector"].as_str().unwrap_or("").into()),
                "done" => ("DONE".into(), String::new()),
                _ => ("WAIT".into(), String::new()),
            });
        }
    }

    Ok(("WAIT".into(), String::new()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// 🚧 GANESHA - Obstacle Removal
// ═══════════════════════════════════════════════════════════════════════════════

async fn remove_obstacles() -> usize {
    let mut removed = 0;

    if let Ok(result) = playwright("detect_obstacles", &[]) {
        if let Some(obstacles) = result["obstacles"].as_array() {
            for obstacle in obstacles {
                match obstacle["type"].as_str().unwrap_or("") {
                    "cookie_consent" => {
                        if let Ok(r) = playwright("dismiss_cookies", &[]) {
                            if r["success"].as_bool().unwrap_or(false) {
                                removed += 1;
                            }
                        }
                    }
                    "modal" => {
                        for sel in &["button:has-text('Close')", "[aria-label='Close']", ".modal-close"] {
                            if playwright("click", &[sel]).is_ok() {
                                removed += 1;
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    removed
}

// ═══════════════════════════════════════════════════════════════════════════════
// 🐜 ANT EXECUTION
// ═══════════════════════════════════════════════════════════════════════════════

struct AntExecResult {
    success: bool,
    ant_says: String,
}

fn ant_execute(action: &str, target: &str) -> Result<AntExecResult, String> {
    let result = match action {
        "SEARCH_EBAY" => playwright("search_ebay", &[target])?,
        "SEARCH_GOOGLE" => playwright("search_google", &[target])?,
        "SCROLL" => playwright("scroll", &[target])?,
        "GOTO" => playwright("goto", &[target])?,
        "CLICK" => playwright("click", &[target])?,
        _ => return Err(format!("Unknown: {}", action)),
    };

    let success = result["success"].as_bool().unwrap_or(false);
    let msg = if success {
        result["title"].as_str()
            .or(result["url"].as_str())
            .unwrap_or("OK")
            .to_string()
    } else {
        result["error"].as_str().unwrap_or("Failed").to_string()
    };

    Ok(AntExecResult { success, ant_says: msg })
}

// ═══════════════════════════════════════════════════════════════════════════════
// INFRASTRUCTURE
// ═══════════════════════════════════════════════════════════════════════════════

fn ensure_browser() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let check = Command::new("curl")
        .args(["-s", "http://127.0.0.1:9222/json/version"])
        .output();

    if check.is_err() || !check.unwrap().status.success() {
        println!("[*] Launching Chromium...");
        Command::new("chromium")
            .args(["--remote-debugging-port=9222", "--no-first-run"])
            .env("DISPLAY", std::env::var("DISPLAY").unwrap_or(":1".into()))
            .spawn()?;
        std::thread::sleep(Duration::from_secs(3));
    }
    Ok(())
}

fn playwright(cmd: &str, args: &[&str]) -> Result<serde_json::Value, String> {
    let script = std::env::current_dir()
        .unwrap()
        .join("scripts/playwright_bridge.js");

    let mut command = Command::new("node");
    command.arg(&script).arg(cmd);
    for arg in args {
        command.arg(arg);
    }

    let output = command.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str(&stdout).map_err(|e| format!("Parse: {} - {}", e, stdout))
}
