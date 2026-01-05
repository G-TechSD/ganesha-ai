//! Ralph Autonomous - Dual Model Architecture
//!
//! Clean separation of concerns:
//! - VISION (ministral-3b): Only describes what's on screen
//! - PLANNER (gpt-oss-20b): Decides actions based on vision reports
//!
//! This prevents context pollution and keeps each model focused.
//!
//! Run with: cargo run --example ralph_autonomous --features computer-use

use ganesha::agent::AgentControl;
use ganesha::input::InputController;
use ganesha::vision::VisionController;
use std::sync::Arc;
use std::time::Duration;
use std::process::Command;
use tokio::time::sleep;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// VISION MODEL - Only describes what it sees (fast, minimal)
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3b-2410";

// PLANNER MODEL - Decides actions (smart, strategic)
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

// Screen dimensions
const SCREEN_WIDTH: u32 = 1920;
const SCREEN_HEIGHT: u32 = 1080;

/// What the vision model reports
#[derive(Debug, Clone)]
struct ScreenReport {
    app: String,              // Firefox, Writer, Terminal, etc.
    url: Option<String>,      // Current URL if browser
    state: String,            // Brief description of what's visible
}

/// What the planner decides
#[derive(Debug, Clone)]
struct PlannedAction {
    action: String,           // CLICK, TYPE, KEY, SCROLL, VISIT, DONE
    target: String,           // What to click/type/press
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           RALPH AUTONOMOUS - Dual Model Architecture          ║");
    println!("║                                                               ║");
    println!("║   VISION: ministral-3b (Bedroom) - sees the screen            ║");
    println!("║   PLANNER: gpt-oss-20b (Beast) - decides actions              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());
    let mut control = AgentControl::new();

    vision.enable().map_err(box_err)?;
    input.enable().map_err(box_err)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    // Get goal from user
    println!("What would you like me to do?");
    println!("Examples:");
    println!("  - Open Firefox and search for 'rust programming'");
    println!("  - Create a document about cats in LibreOffice Writer");
    println!("  - Take a screenshot and save it");
    println!();
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();

    if goal.is_empty() {
        println!("No goal provided. Exiting.");
        return Ok(());
    }

    println!("\n[*] Goal: {}", goal);
    println!("[*] Engaging autonomous mode...\n");

    // Take control with overlay
    control.take_control(false, false)?;

    // Run the autonomous task
    let result = run_autonomous(&client, &vision, &input, &control, goal).await;

    control.release_control();

    match result {
        Ok(summary) => {
            println!("\n╔═══════════════════════════════════════════════════════════════╗");
            println!("║  ✓ GOAL ACHIEVED                                              ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            println!("\n{}", summary);
        }
        Err(e) => {
            println!("\n[✗] Failed: {}", e);
        }
    }

    vision.disable();
    input.disable();

    Ok(())
}

/// Simple hash for motion detection (like NVR)
fn image_hash(data: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Sample every 100th char for fast comparison
    data.chars().step_by(100).collect::<String>().hash(&mut hasher);
    hasher.finish()
}

async fn run_autonomous(
    client: &reqwest::Client,
    vision: &VisionController,
    _input: &InputController,
    control: &AgentControl,
    goal: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let mut history: Vec<String> = Vec::new();
    let mut step_count = 0;
    let max_steps = 30;
    let mut last_hash: u64 = 0;
    let mut wait_count = 0;

    println!("[*] Goal: {}\n", goal);

    while step_count < max_steps {
        if control.is_interrupted() {
            return Err("Interrupted by user".into());
        }

        // Capture screenshot
        let screenshot = vision.capture_screen_scaled(1280, 720)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        // Motion detection - skip if screen hasn't changed
        let current_hash = image_hash(&screenshot.data);
        if current_hash == last_hash {
            wait_count += 1;
            if wait_count < 10 {
                print!(".");
                use std::io::Write;
                std::io::stdout().flush().ok();
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            // After 10 waits with no change, force a check
            println!(" (forcing check)");
        } else {
            wait_count = 0;
        }
        last_hash = current_hash;
        step_count += 1;

        // STEP 1: Vision model describes the screen (minimal context)
        let screen = ask_vision(client, &screenshot).await?;
        println!("[Step {}] 👁 Screen: {} {}", step_count, screen.app,
            screen.url.as_deref().unwrap_or(""));

        // GOAL RECOGNITION - Check if we've achieved the goal
        let goal_lower = goal.to_lowercase();
        let already_searched = history.iter().any(|h| h.contains("SEARCHED"));

        // If goal was "search X" and we already did a search, we're done!
        if goal_lower.contains("search") && already_searched {
            println!("  ✅ GOAL ACHIEVED - search completed!");
            println!("\n╔═══════════════════════════════════════════════════════════════╗");
            println!("║  ✓ TASK COMPLETE                                              ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            return Ok(format!("Successfully searched. Completed in {} steps.", step_count));
        }

        // STEP 2: Planner decides next action using TOOL CALLS
        let action = ask_planner_tools(client, goal, &screen, &history).await?;
        println!("  🧠 Plan: {} {}", action.action, action.target);

        // STEP 3: Execute the action
        match action.action.to_uppercase().as_str() {
            "SEARCH_EBAY" => {
                // Check if we already searched for this
                let search_key = format!("SEARCHED_EBAY {}", action.target);
                if history.contains(&search_key) {
                    println!("  ⏭ Already searched, scrolling to see more...");
                    xdotool(&["click", "--repeat", "3", "--delay", "100", "5"])?; // scroll down
                    history.push("SCROLLED down".to_string());
                } else {
                    println!("  ⚡ SEARCH EBAY: \"{}\"", action.target);
                    let encoded = action.target.replace(' ', "+");
                    let url = format!("https://www.ebay.com/sch/i.html?_nkw={}", encoded);
                    xdotool(&["key", "ctrl+l"])?;
                    sleep(Duration::from_millis(100)).await;
                    xdotool(&["type", "--clearmodifiers", "--delay", "5", &url])?;
                    sleep(Duration::from_millis(50)).await;
                    xdotool(&["key", "Return"])?;
                    sleep(Duration::from_millis(2000)).await; // Wait for page load
                    history.push(search_key);
                }
            }
            "SEARCH_GOOGLE" => {
                println!("  ⚡ SEARCH GOOGLE: \"{}\"", action.target);
                let encoded = action.target.replace(' ', "+");
                let url = format!("https://www.google.com/search?q={}", encoded);
                xdotool(&["key", "ctrl+l"])?;
                sleep(Duration::from_millis(100)).await;
                xdotool(&["type", "--clearmodifiers", "--delay", "5", &url])?;
                sleep(Duration::from_millis(50)).await;
                xdotool(&["key", "Return"])?;
                history.push(format!("SEARCHED_GOOGLE {}", action.target));
            }
            "VISIT" | "GOTO" => {
                // Check if we already visited
                let visit_key = format!("VISITED {}", action.target);
                if history.iter().any(|h| h.contains(&action.target)) {
                    println!("  ⏭ Already visited {}", action.target);
                } else {
                    println!("  ⚡ VISIT {}", action.target);
                    xdotool(&["key", "ctrl+l"])?;
                    sleep(Duration::from_millis(100)).await;
                    xdotool(&["type", "--clearmodifiers", "--delay", "5", &action.target])?;
                    sleep(Duration::from_millis(50)).await;
                    xdotool(&["key", "Return"])?;
                    sleep(Duration::from_millis(2000)).await; // Wait for page load
                    history.push(visit_key);
                }
            }
            "TYPE" => {
                println!("  ⚡ TYPE \"{}\"", action.target);
                xdotool(&["type", "--clearmodifiers", "--delay", "5", &action.target])?;
                history.push(format!("TYPED {}", action.target));
            }
            "TYPE_ENTER" | "SEARCH" => {
                println!("  ⚡ SEARCH \"{}\"", action.target);
                // Use URL-based search - much more reliable than clicking search bars
                // Encode search query for URL
                let encoded = action.target.replace(' ', "+");
                let search_url = format!("https://www.ebay.com/sch/i.html?_nkw={}", encoded);
                xdotool(&["key", "ctrl+l"])?;
                sleep(Duration::from_millis(100)).await;
                xdotool(&["type", "--clearmodifiers", "--delay", "5", &search_url])?;
                sleep(Duration::from_millis(50)).await;
                xdotool(&["key", "Return"])?;
                history.push(format!("SEARCHED {}", action.target));
            }
            "KEY" => {
                let key = match action.target.to_lowercase().as_str() {
                    "enter" | "return" => "Return",
                    "escape" | "esc" => "Escape",
                    "tab" => "Tab",
                    _ => &action.target,
                };
                println!("  ⚡ KEY {}", key);
                xdotool(&["key", key])?;
                history.push(format!("KEY {}", key));
            }
            "CLICK" => {
                // Parse coordinates from target like "500,300" or launch app by name
                if action.target.contains(',') {
                    let parts: Vec<&str> = action.target.split(',').collect();
                    if parts.len() == 2 {
                        let x = parts[0].trim();
                        let y = parts[1].trim();
                        println!("  ⚡ CLICK ({}, {})", x, y);
                        xdotool(&["mousemove", "--sync", x, y])?;
                        xdotool(&["click", "1"])?;
                        history.push(format!("CLICKED ({}, {})", x, y));
                    }
                } else {
                    // Click by name - launch app
                    println!("  ⚡ LAUNCH {}", action.target);
                    launch_app(&action.target)?;
                    history.push(format!("LAUNCHED {}", action.target));
                }
            }
            "SCROLL" => {
                let dir = action.target.to_lowercase();
                println!("  ⚡ SCROLL {}", dir);
                let button = if dir.contains("up") { "4" } else { "5" };
                xdotool(&["click", "--repeat", "5", "--delay", "10", button])?;
                history.push(format!("SCROLLED {}", dir));
            }
            "WAIT" => {
                println!("  ⏳ WAIT");
                sleep(Duration::from_millis(500)).await;
                history.push("WAITED".to_string());
            }
            "DONE" => {
                println!("  ✓ Task complete!");
                return Ok(format!("Completed in {} steps.", step_count));
            }
            _ => {
                println!("  ⚠ Unknown: {} {}", action.action, action.target);
            }
        }

        // Brief pause for UI
        sleep(Duration::from_millis(100)).await;
    }

    Err(format!("Did not complete goal in {} steps", max_steps).into())
}

/// Launch an app by name
fn launch_app(name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cmd = match name.to_lowercase().as_str() {
        "firefox" => "firefox",
        "writer" | "libreoffice" => "libreoffice --writer",
        "terminal" => "gnome-terminal",
        "files" | "nautilus" => "nautilus",
        _ => name,
    };
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":1".to_string());
    Command::new("sh")
        .args(["-c", &format!("{} &", cmd)])
        .env("DISPLAY", &display)
        .spawn()?;
    Ok(())
}

/// VISION MODEL: Only describes what it sees - minimal context, no decision making
async fn ask_vision(
    client: &reqwest::Client,
    screenshot: &ganesha::vision::Screenshot,
) -> Result<ScreenReport, Box<dyn std::error::Error + Send + Sync>> {

    let system_msg = "You are a screen reader. Describe what you see. Be concise.";

    let user_msg = r#"What app is visible? If browser, what URL?
Format: APP: [name] | URL: [url] | STATE: [brief description]
Example: APP: Firefox | URL: google.com | STATE: search page with results"#;

    let request = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [
            {"role": "system", "content": system_msg},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": user_msg},
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
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unknown");

    // Parse the response
    let mut app = "Unknown".to_string();
    let mut url = None;
    let mut state = content.to_string();

    for part in content.split('|') {
        let part = part.trim();
        if part.to_uppercase().starts_with("APP:") {
            app = part[4..].trim().to_string();
        } else if part.to_uppercase().starts_with("URL:") {
            let u = part[4..].trim();
            if !u.is_empty() && u.to_lowercase() != "none" && u.to_lowercase() != "n/a" {
                url = Some(u.to_string());
            }
        } else if part.to_uppercase().starts_with("STATE:") {
            state = part[6..].trim().to_string();
        }
    }

    Ok(ScreenReport { app, url, state })
}

/// PLANNER MODEL: Uses TOOL CALLS for reliable action selection
async fn ask_planner_tools(
    client: &reqwest::Client,
    goal: &str,
    screen: &ScreenReport,
    history: &[String],
) -> Result<PlannedAction, Box<dyn std::error::Error + Send + Sync>> {

    let history_text = if history.is_empty() {
        "None yet".to_string()
    } else {
        history.iter().rev().take(5).rev().cloned().collect::<Vec<_>>().join(" → ")
    };

    let screen_desc = format!("{} | {} | {}",
        screen.app,
        screen.url.as_deref().unwrap_or("no URL"),
        screen.state);

    let tools = serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "search_ebay",
                "description": "Search eBay for products",
                "parameters": {
                    "type": "object",
                    "properties": {"query": {"type": "string", "description": "Search query"}},
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_google",
                "description": "Search Google",
                "parameters": {
                    "type": "object",
                    "properties": {"query": {"type": "string", "description": "Search query"}},
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "visit_url",
                "description": "Navigate browser to a URL",
                "parameters": {
                    "type": "object",
                    "properties": {"url": {"type": "string", "description": "URL to visit"}},
                    "required": ["url"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "scroll",
                "description": "Scroll the page to see more content",
                "parameters": {
                    "type": "object",
                    "properties": {"direction": {"type": "string", "enum": ["up", "down"]}},
                    "required": ["direction"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "task_done",
                "description": "Mark task as complete when goal is achieved",
                "parameters": {"type": "object", "properties": {}}
            }
        }
    ]);

    // Check if we already did a search
    let already_searched = history_text.contains("SEARCHED");
    let on_results = screen.url.as_ref().map(|u| u.contains("sch/i.html") || u.contains("/search")).unwrap_or(false);

    let context = if on_results && already_searched {
        format!("GOAL: {}\nSCREEN: {} (VIEWING SEARCH RESULTS - scroll to see more or mark DONE)\nHISTORY: {}", goal, screen_desc, history_text)
    } else {
        format!("GOAL: {}\nSCREEN: {}\nHISTORY: {}", goal, screen_desc, history_text)
    };

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {"role": "system", "content": "You control a computer. Call ONE tool. If viewing search results, use scroll or done."},
            {"role": "user", "content": context}
        ],
        "tools": tools,
        "tool_choice": "required",
        "max_tokens": 100,
        "temperature": 0.0
    });

    let response = client.post(PLANNER_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    // Parse tool call
    if let Some(tool_calls) = result["choices"][0]["message"]["tool_calls"].as_array() {
        if let Some(call) = tool_calls.first() {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: serde_json::Value = serde_json::from_str(
                call["function"]["arguments"].as_str().unwrap_or("{}")
            ).unwrap_or(serde_json::json!({}));

            return match name {
                "search_ebay" => Ok(PlannedAction {
                    action: "SEARCH_EBAY".to_string(),
                    target: args["query"].as_str().unwrap_or("").to_string(),
                }),
                "search_google" => Ok(PlannedAction {
                    action: "SEARCH_GOOGLE".to_string(),
                    target: args["query"].as_str().unwrap_or("").to_string(),
                }),
                "type_and_enter" => Ok(PlannedAction {
                    action: "TYPE_ENTER".to_string(),
                    target: args["text"].as_str().unwrap_or("").to_string(),
                }),
                "visit_url" => Ok(PlannedAction {
                    action: "VISIT".to_string(),
                    target: args["url"].as_str().unwrap_or("").to_string(),
                }),
                "scroll" => Ok(PlannedAction {
                    action: "SCROLL".to_string(),
                    target: args["direction"].as_str().unwrap_or("down").to_string(),
                }),
                "task_done" => Ok(PlannedAction {
                    action: "DONE".to_string(),
                    target: String::new(),
                }),
                _ => Ok(PlannedAction {
                    action: "WAIT".to_string(),
                    target: String::new(),
                }),
            };
        }
    }

    // Fallback if no tool call
    Ok(PlannedAction {
        action: "WAIT".to_string(),
        target: String::new(),
    })
}

// Helper functions
fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e)
}

fn xdotool(args: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":1".to_string());
    Command::new("xdotool")
        .args(args)
        .env("DISPLAY", &display)
        .status()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    Ok(())
}
