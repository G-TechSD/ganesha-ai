//! Ralph Hybrid - Playwright + Vision Architecture
//!
//! - PLAYWRIGHT: Fast DOM access, precise control, structured data
//! - VISION: Situational awareness, polls every 3-4 seconds
//! - PLANNER: Strategic decisions based on both inputs
//!
//! Run: cargo run --example ralph_hybrid --features computer-use

use ganesha::vision::VisionController;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3b-2410";
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

const VISION_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct BrowserState {
    url: String,
    title: String,
    // Structured data from Playwright
    items: Vec<String>,
}

#[derive(Debug, Clone)]
struct VisionState {
    description: String,
    timestamp: Instant,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          RALPH HYBRID - Playwright + Vision                   ║");
    println!("║                                                               ║");
    println!("║   PLAYWRIGHT: DOM control, structured data                    ║");
    println!("║   VISION: Situational awareness (polls every 3s)              ║");
    println!("║   PLANNER: gpt-oss-20b strategic decisions                    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Ensure Chromium is running with CDP
    ensure_browser()?;

    let vision = Arc::new(VisionController::new());
    vision.enable().map_err(|e| format!("Vision error: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    // Get goal
    println!("What would you like me to do?");
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();

    if goal.is_empty() {
        println!("No goal. Exiting.");
        return Ok(());
    }

    println!("\n[*] Goal: {}\n", goal);

    // Run hybrid loop
    let result = run_hybrid(&client, &vision, goal).await;

    vision.disable();

    match result {
        Ok(summary) => println!("\n✓ {}", summary),
        Err(e) => println!("\n✗ Error: {}", e),
    }

    Ok(())
}

fn ensure_browser() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if CDP is available
    let check = Command::new("curl")
        .args(["-s", "http://127.0.0.1:9222/json/version"])
        .output();

    if check.is_err() || !check.unwrap().status.success() {
        println!("[*] Launching Chromium with CDP...");
        Command::new("chromium")
            .args(["--remote-debugging-port=9222", "--no-first-run"])
            .env("DISPLAY", std::env::var("DISPLAY").unwrap_or(":1".into()))
            .spawn()?;
        std::thread::sleep(Duration::from_secs(3));
    }
    Ok(())
}

/// Execute Playwright command via bridge script
fn playwright(cmd: &str, args: &[&str]) -> Result<serde_json::Value, String> {
    let script_path = std::env::current_dir()
        .unwrap()
        .join("scripts/playwright_bridge.js");

    let mut command = Command::new("node");
    command.arg(&script_path).arg(cmd);
    for arg in args {
        command.arg(arg);
    }

    let output = command.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str(&stdout).map_err(|e| format!("Parse error: {} - {}", e, stdout))
}

async fn run_hybrid(
    client: &reqwest::Client,
    vision: &VisionController,
    goal: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut history: Vec<String> = Vec::new();
    let mut last_vision_check = Instant::now();
    let mut vision_state: Option<VisionState> = None;
    let max_steps = 20;

    for step in 1..=max_steps {
        // STEP 1: Get browser state via Playwright (fast)
        let browser_state = get_browser_state()?;
        println!("[Step {}] 🌐 {}", step, browser_state.title);

        // STEP 2: Vision check if needed (every 3-4 seconds or on demand)
        let need_vision = last_vision_check.elapsed() > VISION_POLL_INTERVAL
            || vision_state.is_none()
            || browser_state.title.is_empty();

        if need_vision {
            print!("  👁 Vision check... ");
            let screenshot = vision.capture_screen_scaled(1280, 720)
                .map_err(|e| format!("Screenshot error: {}", e))?;

            let desc = ask_vision(client, &screenshot).await?;
            println!("{}", desc);

            vision_state = Some(VisionState {
                description: desc,
                timestamp: Instant::now(),
            });
            last_vision_check = Instant::now();
        }

        // STEP 3: Goal recognition
        let goal_lower = goal.to_lowercase();
        let on_search_results = browser_state.url.contains("sch/i.html")
            || browser_state.url.contains("/search")
            || browser_state.title.to_lowercase().contains("for sale");

        // If goal is "search X" and we're on search results page
        if goal_lower.contains("search") && on_search_results {
            println!("\n✅ GOAL ACHIEVED - viewing search results!");
            return Ok(format!("Completed in {} steps", step));
        }

        // STEP 4: Ask planner what to do
        let vision_desc = vision_state.as_ref()
            .map(|v| v.description.clone())
            .unwrap_or_default();

        let action = ask_planner(client, goal, &browser_state, &vision_desc, &history).await?;
        println!("  🧠 {}: {}", action.0, action.1);

        // STEP 5: Execute via Playwright
        match action.0.as_str() {
            "SEARCH_EBAY" => {
                if history.iter().any(|h| h.contains("SEARCHED_EBAY")) {
                    // Already searched - scroll instead
                    let _ = playwright("scroll", &["down"]);
                    history.push("SCROLLED".into());
                } else {
                    let result = playwright("search_ebay", &[&action.1])?;
                    if result["success"].as_bool().unwrap_or(false) {
                        println!("  ⚡ Searched eBay: {}", action.1);
                        history.push(format!("SEARCHED_EBAY {}", action.1));
                        sleep(Duration::from_secs(2)).await; // Wait for results
                    }
                }
            }
            "SCROLL" => {
                let _ = playwright("scroll", &[&action.1]);
                history.push(format!("SCROLLED {}", action.1));
            }
            "VISIT" => {
                let result = playwright("goto", &[&action.1])?;
                if result["success"].as_bool().unwrap_or(false) {
                    history.push(format!("VISITED {}", action.1));
                }
            }
            "DONE" => {
                return Ok(format!("Task complete in {} steps", step));
            }
            _ => {
                println!("  ⚠ Unknown action: {}", action.0);
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    Err("Max steps reached".into())
}

fn get_browser_state() -> Result<BrowserState, String> {
    let result = playwright("get_state", &[])?;

    Ok(BrowserState {
        url: result["url"].as_str().unwrap_or("").to_string(),
        title: result["title"].as_str().unwrap_or("").to_string(),
        items: vec![],
    })
}

async fn ask_vision(
    client: &reqwest::Client,
    screenshot: &ganesha::vision::Screenshot,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {"role": "system", "content": "Describe what you see briefly. Focus on: app, page content, any errors or popups."},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "What's on screen?"},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/png;base64,{}", screenshot.data)
                    }}
                ]
            }
        ],
        "max_tokens": 100,
        "temperature": 0.0
    });

    let response = client.post(VISION_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string())
}

async fn ask_planner(
    client: &reqwest::Client,
    goal: &str,
    browser: &BrowserState,
    vision_desc: &str,
    history: &[String],
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let history_text = if history.is_empty() {
        "None".into()
    } else {
        history.join(" → ")
    };

    let tools = serde_json::json!([
        {"type": "function", "function": {
            "name": "search_ebay", "description": "Search eBay",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        }},
        {"type": "function", "function": {
            "name": "scroll", "description": "Scroll page",
            "parameters": {"type": "object", "properties": {"direction": {"type": "string", "enum": ["up", "down"]}}, "required": ["direction"]}
        }},
        {"type": "function", "function": {
            "name": "visit", "description": "Go to URL",
            "parameters": {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}
        }},
        {"type": "function", "function": {
            "name": "done", "description": "Task complete",
            "parameters": {"type": "object", "properties": {}}
        }}
    ]);

    let context = format!(
        "GOAL: {}\nBROWSER: {} ({})\nVISION: {}\nHISTORY: {}",
        goal, browser.title, browser.url, vision_desc, history_text
    );

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {"role": "system", "content": "You control a browser. Pick ONE action. If goal achieved, use done."},
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
                "visit" => ("VISIT".into(), args["url"].as_str().unwrap_or("").into()),
                "done" => ("DONE".into(), String::new()),
                _ => ("WAIT".into(), String::new()),
            });
        }
    }

    Ok(("WAIT".into(), String::new()))
}
