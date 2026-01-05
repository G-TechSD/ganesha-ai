//! Ganesha Autonomous Agent
//!
//! This is the REAL Ganesha - it sees, thinks, and acts on its own.
//! Give it a task and step back.
//!
//! Usage: DISPLAY=:1 cargo run --example ganesha_autonomous --features computer-use -- "create a smiley face in blender"

use std::env;
use std::process::Command;
use std::time::Duration;
use std::thread;
use std::io::Write;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// Configuration
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1";
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";  // Actual vision model!
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1";
const PLANNER_MODEL: &str = "openai/gpt-oss-20b";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get task from command line
    let args: Vec<String> = env::args().collect();
    let task = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "create a smiley face in blender".to_string()
    };

    println!(r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   🕉️  GANESHA AUTONOMOUS AGENT  🕉️                               ║
    ║                                                                   ║
    ║   "Vakratunda Mahakaya, Surya Koti Samaprabha"                   ║
    ║   The Obstacle Remover operates independently.                   ║
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
    "#);

    println!("📋 TASK: {}", task);
    println!("👁️  Vision: {} @ {}", VISION_MODEL, VISION_ENDPOINT);
    println!("🧠 Planner: {} @ {}", PLANNER_MODEL, PLANNER_ENDPOINT);
    println!("");

    // Check DISPLAY
    let display = env::var("DISPLAY").unwrap_or_default();
    if display.is_empty() {
        eprintln!("ERROR: DISPLAY not set");
        return Ok(());
    }

    let mut agent = GaneshaAgent::new(&task);
    agent.run()?;

    Ok(())
}

struct GaneshaAgent {
    task: String,
    history: Vec<String>,
    max_iterations: usize,
    iteration: usize,
}

impl GaneshaAgent {
    fn new(task: &str) -> Self {
        Self {
            task: task.to_string(),
            history: Vec::new(),
            max_iterations: 50,
            iteration: 0,
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🕉️ GANESHA: Beginning autonomous operation...\n");

        loop {
            self.iteration += 1;
            if self.iteration > self.max_iterations {
                println!("⚠️ Max iterations reached. Stopping.");
                break;
            }

            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("🔄 Iteration {}/{}", self.iteration, self.max_iterations);

            // Step 1: See (capture screenshot)
            println!("👁️  SEEING...");
            let screenshot_path = self.capture_screenshot()?;

            // Step 2: Understand (send to vision model)
            println!("🔍 UNDERSTANDING...");
            let screen_description = self.understand_screen(&screenshot_path)?;
            println!("   Vision says: {}", truncate(&screen_description, 200));

            // Step 3: Think (send to planner)
            println!("🧠 THINKING...");
            let action = self.plan_action(&screen_description)?;
            println!("   Plan: {}", truncate(&action, 200));

            // Check if task is complete
            if action.to_lowercase().contains("task complete") ||
               action.to_lowercase().contains("task_complete") ||
               action.to_lowercase().contains("done") && action.to_lowercase().contains("smiley") {
                println!("\n🎉 GANESHA: Task completed successfully!");
                break;
            }

            // Step 4: Act (execute the action)
            println!("🖱️  ACTING...");
            self.execute_action(&action)?;

            // Record history
            self.history.push(format!("Iteration {}: {} -> {}",
                self.iteration,
                truncate(&screen_description, 50),
                truncate(&action, 50)
            ));

            // Small delay between iterations
            thread::sleep(Duration::from_millis(500));
        }

        Ok(())
    }

    fn capture_screenshot(&self) -> Result<String, Box<dyn std::error::Error>> {
        let path = format!("/tmp/ganesha_screen_{}.png", self.iteration);
        Command::new("scrot")
            .args(["-o", &path])
            .output()?;
        Ok(path)
    }

    fn understand_screen(&self, screenshot_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Read and encode image
        let image_data = std::fs::read(screenshot_path)?;
        let base64_image = BASE64.encode(&image_data);

        // Build vision request
        let request_body = serde_json::json!({
            "model": VISION_MODEL,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": format!(
                                "You are Ganesha's eyes for GUI automation. Analyze this screenshot carefully.

TASK: {}

Report EXACTLY what you see:
1. What application is open? (or is it just desktop?)
2. List clickable elements with ESTIMATED PIXEL COORDINATES (x, y from top-left):
   - Dock icons (usually left side, ~50px from left edge)
   - Buttons, menus, text fields
   - Any relevant items for the task
3. Current state and what action would progress the task

Example format:
- Desktop visible, no apps open
- Dock icons at x=30: Files (~y=200), Firefox (~y=250), Terminal (~y=300)
- To open Files, click at approximately (30, 200)

Be specific about coordinates - estimate them based on screen layout. Screen is typically 1920x1080.",
                                self.task
                            )
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{}", base64_image)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 500
        });

        // Send to vision endpoint (vision models need more time for image processing)
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        let response = client
            .post(format!("{}/chat/completions", VISION_ENDPOINT))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .map_err(|e| format!("Vision request failed: {} - Is {} loaded in LM Studio?", e, VISION_MODEL))?;

        let json: serde_json::Value = response.json()?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("Could not understand screen")
            .to_string();

        Ok(content)
    }

    fn plan_action(&self, screen_description: &str) -> Result<String, Box<dyn std::error::Error>> {
        let history_context = if self.history.is_empty() {
            "No previous actions.".to_string()
        } else {
            self.history.iter().rev().take(5).cloned().collect::<Vec<_>>().join("\n")
        };

        // Stuck detection - check if recent actions are repetitive
        let stuck_warning = if self.history.len() >= 3 {
            let recent: Vec<_> = self.history.iter().rev().take(3).collect();
            let all_clicks = recent.iter().all(|h| h.contains("CLICK") || h.contains("click"));
            let all_same_spot = recent.windows(2).all(|w| {
                // Check if clicking similar coordinates
                w[0].split_whitespace().last() == w[1].split_whitespace().last()
            });
            if all_clicks {
                if all_same_spot {
                    "\n\n🚨 CRITICAL: You're STUCK clicking the same spot! You MUST try something completely different NOW:\n- Press KEY Escape to close any blocking windows\n- Press KEY Super to open Activities and search\n- If in Blender already, use shift+a for Add menu\nDO NOT click the same coordinates again!"
                } else {
                    "\n\n⚠️ WARNING: Multiple clicks without progress. Consider:\n- Use DOUBLE_CLICK to open items\n- Use keyboard shortcuts (Super, Escape, Tab)\n- Close blocking windows first"
                }
            } else {
                ""
            }
        } else {
            ""
        };

        let prompt = format!(r#"You are Ganesha - the SMOOTH OPERATOR. You remove obstacles elegantly, not crudely.

TASK: {}

CURRENT SCREEN STATE:
{}

RECENT HISTORY:
{}
{}

BE A SMOOTH OPERATOR:
- ADAPT when something doesn't work - don't repeat the same action!
- Use KEYBOARD SHORTCUTS - they're more reliable than clicking
- If an unwanted window opens, press Escape or close it first
- Think strategically about the best path forward

FORMAT:
ACTION: <type>
PARAMS: <parameters>

ACTIONS:
- CLICK x y - Single click (buttons, menus)
- DOUBLE_CLICK x y - Open folders/files/apps
- TYPE text - Type text (in focused field/terminal)
- KEY key - Press key: Escape, Return, Tab, shift+a, ctrl+s, Super (opens Activities)
- COMBO keys - Press multiple keys in sequence with pauses (e.g., COMBO X Return) - VERY USEFUL!
- SCROLL up/down amount - Scroll
- WAIT ms - Wait
- TASK_COMPLETE - Done!

KEYBOARD SHORTCUTS ARE MORE RELIABLE THAN CLICKING!
- Escape - Close menus/dialogs
- Return - Confirm dialogs (ALWAYS use this to confirm, not clicking!)
- Tab - Navigate between fields
- Up/Down/Left/Right - Navigate menus

BLENDER KEYBOARD WORKFLOW (use COMBO for sequences!):
1. To delete object: COMBO X Return (deletes selected object instantly!)
2. To add sphere: shift+a to open menu, then CLICK on Mesh, then UV Sphere
3. To scale: KEY s, move mouse, CLICK to confirm
4. To move: KEY g, move mouse, CLICK to confirm
5. To rotate: KEY r, move mouse, CLICK to confirm

USE COMBO for reliable key sequences - it presses keys with pauses between them!

⚠️ CRITICAL: Output EXACTLY ONE action. Not two, not zero - exactly ONE.
Example response:
ACTION: CLICK
PARAMS: 500 300

Your single action:"#,
            self.task, screen_description, history_context, stuck_warning
        );

        let request_body = serde_json::json!({
            "model": PLANNER_MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an AI that controls a computer. Output only the next action to take."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 100,
            "temperature": 0.3
        });

        let client = reqwest::blocking::Client::new();
        let response = client
            .post(format!("{}/chat/completions", PLANNER_ENDPOINT))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(Duration::from_secs(60))
            .send()?;

        let json: serde_json::Value = response.json()?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("WAIT 1000")
            .to_string();

        Ok(content)
    }

    fn execute_action(&self, action: &str) -> Result<(), Box<dyn std::error::Error>> {
        let action_upper = action.to_uppercase();

        // Parse multi-line ACTION/PARAMS format or single-line format
        // Format 1: "ACTION: CLICK\nPARAMS: 50 300"
        // Format 2: "CLICK 50 300"

        let mut action_type = String::new();
        let mut params = String::new();

        for line in action.lines() {
            let line_trimmed = line.trim();
            let line_upper = line_trimmed.to_uppercase();

            if line_upper.starts_with("ACTION:") {
                action_type = line_upper.replace("ACTION:", "").trim().to_string();
            } else if line_upper.starts_with("PARAMS:") {
                params = line_trimmed.replace("PARAMS:", "").replace("params:", "").trim().to_string();
            } else if !line_trimmed.is_empty() && action_type.is_empty() {
                // Single-line format: "CLICK 50 300"
                let parts: Vec<&str> = line_trimmed.split_whitespace().collect();
                if !parts.is_empty() {
                    action_type = parts[0].to_uppercase();
                    params = parts[1..].join(" ");
                }
            }
        }

        // Also check the whole string for task completion
        if action_upper.contains("TASK_COMPLETE") || action_upper.contains("TASK COMPLETE") {
            println!("   ✓ Task marked complete");
            return Ok(());
        }

        // Execute the parsed action
        let param_parts: Vec<&str> = params.split_whitespace().collect();

        match action_type.as_str() {
            "CLICK" => {
                if param_parts.len() >= 2 {
                    let x = param_parts[0].parse::<i32>().unwrap_or(500);
                    let y = param_parts[1].parse::<i32>().unwrap_or(300);
                    self.smooth_click(x, y)?;
                    println!("   ✓ Clicked at ({}, {})", x, y);
                } else {
                    println!("   ⚠ CLICK missing coordinates, params: '{}'", params);
                }
            }
            "TYPE" => {
                if !params.is_empty() {
                    Command::new("xdotool")
                        .args(["type", "--delay", "20", &params])
                        .output()?;
                    println!("   ✓ Typed: {}", truncate(&params, 30));
                }
            }
            "KEY" => {
                let key = if !params.is_empty() { &params } else { "Return" };
                // For single printable characters, use 'type' (works better with apps like Blender)
                // For special keys (Return, Escape, F3, ctrl+x), use 'key'
                let is_special = key.len() > 1 || key.contains('+');
                if is_special {
                    Command::new("xdotool")
                        .args(["key", key])
                        .output()?;
                    println!("   ✓ Pressed key: {}", key);
                } else {
                    // Single character - use type for better compatibility
                    Command::new("xdotool")
                        .args(["type", key])
                        .output()?;
                    println!("   ✓ Typed key: {}", key);
                }
            }
            "MOVE" => {
                if param_parts.len() >= 2 {
                    let x = param_parts[0].parse::<i32>().unwrap_or(500);
                    let y = param_parts[1].parse::<i32>().unwrap_or(300);
                    self.smooth_move(x, y)?;
                    println!("   ✓ Moved to ({}, {})", x, y);
                }
            }
            "SCROLL" => {
                let direction = if params.to_uppercase().contains("UP") { "4" } else { "5" };
                let amount = param_parts.last()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(3);
                for _ in 0..amount {
                    Command::new("xdotool").args(["click", direction]).output()?;
                    thread::sleep(Duration::from_millis(50));
                }
                println!("   ✓ Scrolled {} times", amount);
            }
            "WAIT" => {
                let ms = param_parts.first()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(500);
                thread::sleep(Duration::from_millis(ms));
                println!("   ✓ Waited {}ms", ms);
            }
            "DOUBLE_CLICK" | "DOUBLECLICK" => {
                if param_parts.len() >= 2 {
                    let x = param_parts[0].parse::<i32>().unwrap_or(500);
                    let y = param_parts[1].parse::<i32>().unwrap_or(300);
                    self.smooth_move(x, y)?;
                    thread::sleep(Duration::from_millis(50));
                    Command::new("xdotool").args(["click", "--repeat", "2", "--delay", "50", "1"]).output()?;
                    println!("   ✓ Double-clicked at ({}, {})", x, y);
                }
            }
            "COMBO" => {
                // Execute multiple keys in sequence with pauses
                let keys: Vec<&str> = params.split_whitespace().collect();
                for (i, key) in keys.iter().enumerate() {
                    Command::new("xdotool")
                        .args(["key", key])
                        .output()?;
                    if i < keys.len() - 1 {
                        thread::sleep(Duration::from_millis(200)); // Pause between keys
                    }
                }
                println!("   ✓ Combo executed: {}", keys.join(" → "));
            }
            _ => {
                if action_type.is_empty() {
                    println!("   ⚠ Empty plan received - waiting 500ms");
                    thread::sleep(Duration::from_millis(500));
                } else {
                    println!("   ⚠ Unknown action: '{}' with params: '{}'", action_type, params);
                }
            }
        }

        Ok(())
    }

    fn smooth_move(&self, target_x: i32, target_y: i32) -> Result<(), Box<dyn std::error::Error>> {
        // Get current position
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

        // Smooth movement with easing
        let steps = 15;
        let duration_ms = 150;
        let step_delay = Duration::from_micros((duration_ms * 1000) / steps);

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let eased_t = 1.0 - (1.0 - t).powi(3); // Ease-out

            let x = start_x + ((target_x - start_x) as f64 * eased_t) as i32;
            let y = start_y + ((target_y - start_y) as f64 * eased_t) as i32;

            Command::new("xdotool")
                .args(["mousemove", &x.to_string(), &y.to_string()])
                .output()?;

            thread::sleep(step_delay);
        }

        Ok(())
    }

    fn smooth_click(&self, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        self.smooth_move(x, y)?;
        thread::sleep(Duration::from_millis(50));
        Command::new("xdotool").args(["click", "1"]).output()?;
        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
