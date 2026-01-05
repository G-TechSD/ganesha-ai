//! Ralph Eagle-Ant Architecture
//!
//! 🦅 EAGLE (Vision) - Aerial recon, verification layer, truth source
//! 🐜 ANT (Playwright) - Ground-level precision, DOM interaction
//! 🧠 HUMAN (Planner) - Command & control, strategic decisions
//!
//! Vision VERIFIES what Playwright reports - it's the sanity check.
//! If Playwright says "clicked button" but Eagle sees "error popup", trust Eagle.
//!
//! Run: cargo run --example ralph_eagle_ant --features computer-use

use ganesha::vision::VisionController;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3-3b";
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

/// 🦅 Eagle's aerial view - what's really on screen
#[derive(Debug, Clone)]
struct EagleRecon {
    situation: String,      // High-level: "eBay search results page"
    anomalies: Vec<String>, // Popups, errors, unexpected states
    confidence: f32,        // How sure is Eagle about what it sees
    timestamp: Instant,
}

/// 🐜 Ant's ground truth - precise DOM data
#[derive(Debug, Clone)]
struct AntReport {
    url: String,
    title: String,
    element_count: usize,
    visible_text: Vec<String>,
}

/// Action result with verification status
#[derive(Debug)]
struct VerifiedAction {
    action: String,
    success: bool,
    ant_says: String,      // What Playwright reports
    eagle_confirms: bool,  // Does vision verify it?
    discrepancy: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           RALPH - Eagle/Ant Architecture                      ║");
    println!("║                                                               ║");
    println!("║   🦅 EAGLE (Vision): Aerial recon, verification               ║");
    println!("║   🐜 ANT (Playwright): Ground precision, DOM control          ║");
    println!("║   🧠 HUMAN (Planner): Command & control                       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    ensure_browser()?;

    let vision = Arc::new(VisionController::new());
    vision.enable().map_err(|e| format!("Vision: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120)) // Longer timeout for vision with images
        .build()?;

    println!("What's the mission?");
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();

    if goal.is_empty() {
        return Ok(());
    }

    println!("\n[MISSION] {}\n", goal);

    let result = execute_mission(&client, &vision, goal).await;
    vision.disable();

    match result {
        Ok(summary) => println!("\n✓ MISSION COMPLETE: {}", summary),
        Err(e) => println!("\n✗ MISSION FAILED: {}", e),
    }

    Ok(())
}

async fn execute_mission(
    client: &reqwest::Client,
    vision: &VisionController,
    goal: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut history: Vec<String> = Vec::new();
    let mut last_eagle_recon: Option<EagleRecon> = None;

    for step in 1..=15 {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[STEP {}]", step);

        // 🐜 ANT: Get ground-level DOM data
        let ant = ant_scout()?;
        println!("  🐜 ANT: {} | {}", ant.title, ant.url);

        // 🚧 OBSTACLE CHECK: Remove obstacles before proceeding (Ganesha removes obstacles!)
        let obstacles_removed = remove_obstacles().await;
        if obstacles_removed > 0 {
            println!("  🚧 OBSTACLES REMOVED: {} (cookies/popups dismissed)", obstacles_removed);
            // Re-scout after obstacle removal
            let ant = ant_scout()?;
        }

        // 🦅 EAGLE: Aerial verification
        let eagle = eagle_recon(client, vision).await?;
        println!("  🦅 EAGLE: {}", eagle.situation);

        if !eagle.anomalies.is_empty() {
            println!("  ⚠️  ANOMALIES: {:?}", eagle.anomalies);
        }

        // VERIFY: Does Ant's report match Eagle's view?
        let verified = verify_state(&ant, &eagle);
        if !verified {
            println!("  🔴 DISCREPANCY: Ant and Eagle disagree! Trusting Eagle.");
        }

        // GOAL CHECK: Are we done?
        if check_goal_achieved(goal, &ant, &eagle, &history) {
            println!("\n  ✅ GOAL ACHIEVED!");
            return Ok(format!("Mission complete in {} steps", step));
        }

        // 🧠 HUMAN: Decide next action based on BOTH perspectives
        let action = human_decide(client, goal, &ant, &eagle, &history).await?;
        println!("  🧠 DECIDE: {} {}", action.0, action.1);

        // 🐜 ANT: Execute the action
        let result = ant_execute(&action.0, &action.1)?;
        println!("  🐜 EXECUTE: {}", result.ant_says);

        // 🦅 EAGLE: Verify the action worked
        sleep(Duration::from_millis(1500)).await; // Let page settle
        let post_eagle = eagle_recon(client, vision).await?;

        let eagle_confirms = verify_action_success(&action.0, &eagle, &post_eagle);
        if eagle_confirms {
            println!("  🦅 VERIFIED: Action successful");
            history.push(format!("{} {} ✓", action.0, action.1));
        } else {
            println!("  🦅 UNVERIFIED: Eagle doesn't see expected change");
            history.push(format!("{} {} ?", action.0, action.1));
        }

        last_eagle_recon = Some(post_eagle);
    }

    Err("Max steps reached".into())
}

/// 🚧 Remove obstacles (cookies, popups) - Ganesha clears the path!
async fn remove_obstacles() -> usize {
    let mut removed = 0;

    // Check for obstacles
    if let Ok(result) = playwright("detect_obstacles", &[]) {
        if let Some(obstacles) = result["obstacles"].as_array() {
            for obstacle in obstacles {
                let obstacle_type = obstacle["type"].as_str().unwrap_or("");
                match obstacle_type {
                    "cookie_consent" => {
                        // Reject all non-essential cookies
                        if let Ok(r) = playwright("dismiss_cookies", &[]) {
                            if r["success"].as_bool().unwrap_or(false) {
                                removed += 1;
                            }
                        }
                    }
                    "modal" => {
                        // Try to close modal
                        let close_selectors = [
                            "button:has-text('Close')",
                            "button:has-text('×')",
                            "[aria-label='Close']",
                            ".modal-close",
                        ];
                        for sel in &close_selectors {
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

/// 🐜 Ant scouts the DOM for precise data
fn ant_scout() -> Result<AntReport, String> {
    let result = playwright("get_state", &[])?;

    Ok(AntReport {
        url: result["url"].as_str().unwrap_or("").to_string(),
        title: result["title"].as_str().unwrap_or("").to_string(),
        element_count: 0,
        visible_text: vec![],
    })
}

/// 🐜 Ant executes a precise action
fn ant_execute(action: &str, target: &str) -> Result<VerifiedAction, String> {
    let result = match action {
        "SEARCH_EBAY" => playwright("search_ebay", &[target])?,
        "SEARCH_GOOGLE" => playwright("search_google", &[target])?,
        "SCROLL" => playwright("scroll", &[target])?,
        "GOTO" => playwright("goto", &[target])?,
        "CLICK" => playwright("click", &[target])?,
        _ => return Err(format!("Unknown action: {}", action)),
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

    Ok(VerifiedAction {
        action: action.to_string(),
        success,
        ant_says: msg,
        eagle_confirms: false, // Will be set after verification
        discrepancy: None,
    })
}

/// 🦅 Eagle does aerial reconnaissance
async fn eagle_recon(
    client: &reqwest::Client,
    vision: &VisionController,
) -> Result<EagleRecon, Box<dyn std::error::Error + Send + Sync>> {
    let screenshot = vision.capture_screen_scaled(1280, 720)
        .map_err(|e| format!("Screenshot: {}", e))?;

    let prompt = r#"You are an aerial reconnaissance drone. Report:
1. SITUATION: What page/app is visible? (one line)
2. ANOMALIES: Any popups, errors, loading spinners, unexpected states? (list or "none")
3. CONFIDENCE: How clear is the view? (high/medium/low)

Format: SITUATION: ... | ANOMALIES: ... | CONFIDENCE: ..."#;

    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {"role": "system", "content": "You are a reconnaissance drone providing tactical intel."},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }
        ],
        "max_tokens": 150,
        "temperature": 0.0
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown");

    // Parse Eagle's report
    let mut situation = content.to_string();
    let mut anomalies = vec![];
    let mut confidence = 0.8;

    for part in content.split('|') {
        let part = part.trim();
        if part.to_uppercase().starts_with("SITUATION:") {
            situation = part[10..].trim().to_string();
        } else if part.to_uppercase().starts_with("ANOMALIES:") {
            let a = part[10..].trim();
            if !a.to_lowercase().contains("none") {
                anomalies = a.split(',').map(|s| s.trim().to_string()).collect();
            }
        } else if part.to_uppercase().starts_with("CONFIDENCE:") {
            let c = part[11..].trim().to_lowercase();
            confidence = match c.as_str() {
                "high" => 0.9,
                "medium" => 0.7,
                "low" => 0.5,
                _ => 0.8,
            };
        }
    }

    Ok(EagleRecon {
        situation,
        anomalies,
        confidence,
        timestamp: Instant::now(),
    })
}

/// Verify that Ant and Eagle agree on state
fn verify_state(ant: &AntReport, eagle: &EagleRecon) -> bool {
    // Basic check: if Ant says eBay, Eagle should see eBay
    if ant.url.contains("ebay") && !eagle.situation.to_lowercase().contains("ebay") {
        return false;
    }
    if ant.url.contains("google") && !eagle.situation.to_lowercase().contains("google") {
        return false;
    }
    // No major anomalies
    eagle.anomalies.is_empty() || eagle.confidence > 0.7
}

/// Verify an action actually worked by comparing before/after Eagle views
fn verify_action_success(action: &str, before: &EagleRecon, after: &EagleRecon) -> bool {
    match action {
        "SEARCH_EBAY" | "SEARCH_GOOGLE" => {
            // Should see search results
            after.situation.to_lowercase().contains("search")
                || after.situation.to_lowercase().contains("results")
                || after.situation.to_lowercase().contains("for sale")
        }
        "SCROLL" => {
            // Page should look different (hard to verify, assume success)
            true
        }
        "GOTO" => {
            // URL context should change
            before.situation != after.situation
        }
        _ => true,
    }
}

/// Check if goal is achieved based on both perspectives
fn check_goal_achieved(goal: &str, ant: &AntReport, eagle: &EagleRecon, history: &[String]) -> bool {
    let goal_lower = goal.to_lowercase();

    if goal_lower.contains("search") {
        // Both Ant and Eagle should agree we're on search results
        let ant_sees_results = ant.url.contains("sch/i.html")
            || ant.url.contains("/search")
            || ant.title.to_lowercase().contains("for sale");

        let eagle_sees_results = eagle.situation.to_lowercase().contains("search")
            || eagle.situation.to_lowercase().contains("results")
            || eagle.situation.to_lowercase().contains("for sale")
            || eagle.situation.to_lowercase().contains("listings");

        return ant_sees_results && eagle_sees_results;
    }

    false
}

/// 🧠 Human decides next action based on both Eagle and Ant intel
async fn human_decide(
    client: &reqwest::Client,
    goal: &str,
    ant: &AntReport,
    eagle: &EagleRecon,
    history: &[String],
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
            "name": "scroll",
            "description": "Scroll to see more",
            "parameters": {"type": "object", "properties": {"direction": {"type": "string", "enum": ["up", "down"]}}, "required": ["direction"]}
        }},
        {"type": "function", "function": {
            "name": "goto",
            "description": "Navigate to URL",
            "parameters": {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}
        }},
        {"type": "function", "function": {
            "name": "done",
            "description": "Mission complete",
            "parameters": {"type": "object", "properties": {}}
        }}
    ]);

    // Give planner BOTH perspectives
    let context = format!(
        "MISSION: {}\n\n🐜 ANT REPORT:\n  URL: {}\n  Title: {}\n\n🦅 EAGLE RECON:\n  {}\n  Anomalies: {:?}\n\nHISTORY: {}",
        goal, ant.url, ant.title, eagle.situation, eagle.anomalies, history_text
    );

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {"role": "system", "content": "You command a recon mission. Use Ant for precision, trust Eagle for truth. Pick ONE action."},
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
                "scroll" => ("SCROLL".into(), args["direction"].as_str().unwrap_or("down").into()),
                "goto" => ("GOTO".into(), args["url"].as_str().unwrap_or("").into()),
                "done" => ("DONE".into(), String::new()),
                _ => ("WAIT".into(), String::new()),
            });
        }
    }

    Ok(("WAIT".into(), String::new()))
}

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
