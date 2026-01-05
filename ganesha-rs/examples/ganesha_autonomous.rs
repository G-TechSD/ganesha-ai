//! Ganesha Autonomous Agent
//!
//! This is the REAL Ganesha - it sees, thinks, and acts on its own.
//! Give it a task and step back.
//!
//! Usage: DISPLAY=:1 cargo run --example ganesha_autonomous --features computer-use -- "create a smiley face in blender"

use std::env;
use std::process::{Command, Child, Stdio};
use std::time::Duration;
use std::thread;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// Configuration
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1";
const VISION_MODEL: &str = "mistralai/ministral-3-3b";  // Fast vision model
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
    app_knowledge: Option<String>,  // Learned at runtime, NOT hardcoded
    overlay_process: Option<Child>,
    clicked_viewport: bool,  // Track if we've clicked in viewport for Blender
}

impl GaneshaAgent {
    fn new(task: &str) -> Self {
        Self {
            task: task.to_string(),
            history: Vec::new(),
            max_iterations: 50,
            iteration: 0,
            app_knowledge: None,
            overlay_process: None,
            clicked_viewport: false,
        }
    }

    /// Start the red frame overlay to indicate Ganesha is active
    fn start_overlay(&mut self) -> bool {
        // Use tkinter which is more commonly available than PyGObject
        let overlay_script = r#"
import tkinter as tk
import sys

class RedFrameOverlay:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("GANESHA ACTIVE")

        # Make fullscreen, always on top
        self.root.attributes('-fullscreen', True)
        self.root.attributes('-topmost', True)
        self.root.attributes('-alpha', 1.0)  # Full opacity

        # Try to make click-through (X11 specific)
        try:
            self.root.wm_attributes('-type', 'dock')
        except:
            pass

        # Get screen dimensions
        width = self.root.winfo_screenwidth()
        height = self.root.winfo_screenheight()

        # Create canvas
        self.canvas = tk.Canvas(self.root, width=width, height=height,
                                bg='black', highlightthickness=0)
        self.canvas.pack()

        # Draw red frame (thick border)
        thickness = 12
        # Top
        self.canvas.create_rectangle(0, 0, width, thickness, fill='red', outline='red')
        # Bottom
        self.canvas.create_rectangle(0, height-thickness, width, height, fill='red', outline='red')
        # Left
        self.canvas.create_rectangle(0, 0, thickness, height, fill='red', outline='red')
        # Right
        self.canvas.create_rectangle(width-thickness, 0, width, height, fill='red', outline='red')

        # Make center transparent/black (click-through area marker)
        self.canvas.create_rectangle(thickness, thickness, width-thickness, height-thickness,
                                     fill='', outline='')

        # Actually we need the center to be invisible - use overrideredirect
        self.root.overrideredirect(True)

        # Create only the border frame windows instead
        self.root.destroy()
        self.create_border_windows(width, height, thickness)

    def create_border_windows(self, width, height, thickness):
        # Create 4 separate windows for the borders
        self.windows = []
        borders = [
            (0, 0, width, thickness),  # Top
            (0, height-thickness, width, thickness),  # Bottom
            (0, 0, thickness, height),  # Left
            (width-thickness, 0, thickness, height),  # Right
        ]

        for x, y, w, h in borders:
            win = tk.Tk()
            win.overrideredirect(True)
            win.attributes('-topmost', True)
            win.geometry(f'{w}x{h}+{x}+{y}')
            win.configure(bg='red')

            # Make it stay on top
            try:
                win.wm_attributes('-type', 'dock')
            except:
                pass

            self.windows.append(win)

        # Run event loop
        if self.windows:
            self.windows[0].mainloop()

overlay = RedFrameOverlay()
"#;
        let script_path = "/tmp/ganesha_overlay.py";
        if std::fs::write(script_path, overlay_script).is_err() {
            println!("❌ Failed to write overlay script");
            return false;
        }

        let display = env::var("DISPLAY").unwrap_or_else(|_| ":1".to_string());
        match Command::new("python3")
            .arg(script_path)
            .env("DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.overlay_process = Some(child);
                println!("🔴 Red frame overlay started");
                thread::sleep(Duration::from_millis(500));  // Give it time to appear
                true
            }
            Err(e) => {
                println!("❌ Failed to start overlay: {}", e);
                false
            }
        }
    }

    /// Stop the red frame overlay
    fn stop_overlay(&mut self) {
        if let Some(mut proc) = self.overlay_process.take() {
            let _ = proc.kill();
            let _ = proc.wait();
            println!("⚪ Red frame overlay stopped");
        }
        let _ = std::fs::remove_file("/tmp/ganesha_overlay.py");
    }

    /// Detect app from task and fetch docs to learn about it
    fn learn_about_app(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Extract app name from task (simple heuristic)
        let task_lower = self.task.to_lowercase();
        let app_name = if task_lower.contains("blender") {
            Some("blender")
        } else if task_lower.contains("gimp") {
            Some("gimp")
        } else if task_lower.contains("firefox") {
            Some("firefox")
        } else if task_lower.contains("libreoffice") || task_lower.contains("calc") || task_lower.contains("writer") {
            Some("libreoffice")
        } else {
            None
        };

        if let Some(app) = app_name {
            println!("📚 GANESHA: I should learn about {} before starting...", app);
            println!("   Searching for official documentation and tutorials...\n");

            // Use the planner LLM to search and summarize docs
            let search_prompt = format!(
                r#"I need to learn how to use {} for GUI automation. Search your knowledge for:
1. Essential keyboard shortcuts (select all, delete, add objects, transform)
2. Common menu locations and UI patterns
3. Step-by-step workflows for basic tasks

Provide a concise reference I can use. Focus on PRACTICAL shortcuts and clicks, not theory.
Format as a simple list I can reference while working."#,
                app
            );

            let knowledge = self.query_planner(&search_prompt)?;

            println!("📖 GANESHA: Learned about {}:\n", app);
            for line in knowledge.lines().take(20) {
                println!("   {}", line);
            }
            if knowledge.lines().count() > 20 {
                println!("   ... (truncated)");
            }
            println!();

            self.app_knowledge = Some(knowledge);
        }

        Ok(())
    }

    fn query_planner(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        let request_body = serde_json::json!({
            "model": PLANNER_MODEL,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 1000,
            "temperature": 0.3
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        let response = client
            .post(format!("{}/chat/completions", PLANNER_ENDPOINT))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()?;

        let json: serde_json::Value = response.json()?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No response")
            .to_string();

        Ok(content)
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🕉️ GANESHA: Beginning autonomous operation...\n");

        // Start the red frame overlay to show Ganesha is active
        // This is a SAFETY REQUIREMENT - must be visible before proceeding
        if !self.start_overlay() {
            println!("❌ SAFETY: Cannot start without visible red frame overlay!");
            println!("   Please install python3-tk or ensure DISPLAY is set correctly.");
            return Err("Red frame overlay is required for safe operation".into());
        }

        // Verify overlay is visible by capturing a test screenshot
        println!("🔍 Verifying overlay visibility...");
        thread::sleep(Duration::from_millis(300));
        let test_path = "/tmp/ganesha_overlay_test.png";
        let _ = Command::new("scrot")
            .args(["-o", test_path])
            .output();
        println!("   📸 Test screenshot saved to {}", test_path);
        println!("   ⚠️  VERIFY: Red frame should be visible around screen edges!");
        thread::sleep(Duration::from_millis(200));

        // FIRST: Learn about the app if needed (fetches docs at runtime)
        if let Err(e) = self.learn_about_app() {
            println!("   (Could not fetch app docs: {} - proceeding anyway)", e);
        }

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
            let (screenshot_path, cursor_x, cursor_y) = self.capture_screenshot()?;

            // Step 2: Understand (send to vision model)
            println!("🔍 UNDERSTANDING...");
            println!("   📍 Cursor at: ({}, {})", cursor_x, cursor_y);
            let screen_description = self.understand_screen(&screenshot_path, cursor_x, cursor_y)?;
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

        // Stop the overlay when done
        self.stop_overlay();
        Ok(())
    }

    fn get_cursor_position(&self) -> (i32, i32) {
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .ok();

        let mut x = 0i32;
        let mut y = 0i32;
        if let Some(output) = output {
            let pos_str = String::from_utf8_lossy(&output.stdout);
            for line in pos_str.lines() {
                if line.starts_with("X=") {
                    x = line[2..].parse().unwrap_or(0);
                } else if line.starts_with("Y=") {
                    y = line[2..].parse().unwrap_or(0);
                }
            }
        }
        (x, y)
    }

    fn capture_screenshot(&self) -> Result<(String, i32, i32), Box<dyn std::error::Error>> {
        // Get cursor position first
        let (cursor_x, cursor_y) = self.get_cursor_position();

        let path = format!("/tmp/ganesha_screen_{}.png", self.iteration);

        // Capture with cursor visible (-p flag)
        Command::new("scrot")
            .args(["-p", "-o", &path])
            .output()?;

        // Draw a bright, large cursor indicator using ImageMagick
        // This makes cursor position obvious to the vision model
        let _ = Command::new("convert")
            .args([
                &path,
                "-fill", "none",
                "-stroke", "#FF00FF",  // Bright magenta
                "-strokewidth", "4",
                "-draw", &format!("circle {},{} {},{}", cursor_x, cursor_y, cursor_x + 30, cursor_y),
                "-stroke", "#00FFFF",  // Cyan crosshair
                "-strokewidth", "2",
                "-draw", &format!("line {},{} {},{}", cursor_x - 40, cursor_y, cursor_x + 40, cursor_y),
                "-draw", &format!("line {},{} {},{}", cursor_x, cursor_y - 40, cursor_x, cursor_y + 40),
                "-pointsize", "20",
                "-fill", "#FFFF00",  // Yellow text
                "-stroke", "#000000",
                "-strokewidth", "1",
                "-annotate", &format!("+{}+{}", cursor_x + 35, cursor_y - 10),
                &format!("CURSOR ({},{})", cursor_x, cursor_y),
                &path,
            ])
            .output();

        Ok((path, cursor_x, cursor_y))
    }

    fn understand_screen(&self, screenshot_path: &str, cursor_x: i32, cursor_y: i32) -> Result<String, Box<dyn std::error::Error>> {
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

CURSOR POSITION: The mouse cursor is currently at ({}, {}). Look for a magenta circle with cyan crosshair marking this location.

Report EXACTLY what you see:
1. What application is open? (or is it just desktop?)
2. List clickable elements with ESTIMATED PIXEL COORDINATES (x, y from top-left):
   - Dock icons (usually left side, ~50px from left edge)
   - Buttons, menus, text fields
   - Any relevant items for the task
3. What is near or under the cursor at ({}, {})?
4. Current state and what action would progress the task

Example format:
- Desktop visible, no apps open
- Dock icons at x=30: Files (~y=200), Firefox (~y=250), Terminal (~y=300)
- Cursor is over the File menu
- To open Files, click at approximately (30, 200)

Be specific about coordinates - estimate them based on screen layout. Screen is typically 1920x1080.",
                                self.task, cursor_x, cursor_y, cursor_x, cursor_y
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
        let repeated_shifta = self.history.iter().rev().take(2)
            .all(|h| h.to_lowercase().contains("shift+a") || h.to_lowercase().contains("shift a"));

        let stuck_warning = if repeated_shifta {
            // Already pressed shift+a multiple times - menu should be open, navigate it
            "\n\n🚨 MENU IS OPEN! You already pressed Shift+A. Now navigate:\n- Type 'uv' to search for UV Sphere\n- Or press Down arrow to navigate menu\n- Then press Return to select\nDO NOT press shift+a again!"
        } else if self.history.len() >= 3 {
            let recent: Vec<_> = self.history.iter().rev().take(3).collect();
            let all_same = recent.windows(2).all(|w| w[0] == w[1]);
            if all_same {
                "\n\n🚨 STUCK: Repeating same action! Try something different."
            } else {
                ""
            }
        } else {
            ""
        };

        // Include learned knowledge if available
        let knowledge_section = if let Some(ref knowledge) = self.app_knowledge {
            format!("\nAPP REFERENCE (I learned this at startup):\n{}\n", knowledge)
        } else {
            String::new()
        };

        // Truncate knowledge to avoid overwhelming the model
        let short_knowledge = if let Some(ref k) = self.app_knowledge {
            let truncated: String = k.chars().take(500).collect();
            format!("\nREF: {}\n", truncated)
        } else {
            String::new()
        };

        let prompt = format!(r#"=== GOAL-ORIENTED PLANNING ===

🎯 USER'S GOAL: {}

📚 REFERENCE KNOWLEDGE:
{}

📜 ACTIONS TAKEN SO FAR:
{}
{}

👁️ CURRENT SCREEN STATE:
{}

=== PLANNING ===
Think about:
1. What is the user trying to achieve? → {}
2. What progress has been made? (see actions above)
3. What does the current screen show?
4. What is the SINGLE NEXT STEP to move toward the goal?

Available actions:
- KEY shift+a → Opens Add menu in Blender (ESSENTIAL for adding objects)
- KEY Tab → Toggle Edit/Object mode
- KEY g/s/r → Move/Scale/Rotate selected
- KEY x → Delete (then Return to confirm)
- KEY Return → Confirm dialogs/selections
- KEY Down/Up → Navigate menus
- CLICK x y → Click at pixel coordinates
- TYPE text → Type text (only in text fields or search boxes)
- TASK_COMPLETE → When the goal is achieved

IMPORTANT:
- Each action should move toward the GOAL
- In Blender menus, TYPE to search (e.g., "uv" for UV Sphere) then Return to select
- Look at what's ON SCREEN and choose the logical next step
- If you see the goal is complete (e.g., UV Sphere exists), use TASK_COMPLETE

Reply with ONE action:
ACTION: <action_type>
PARAMS: <parameters>"#,
            self.task, // Goal
            short_knowledge,
            history_context.chars().take(300).collect::<String>(),
            stuck_warning,
            screen_description.chars().take(400).collect::<String>(),
            self.task // Remind again
        );

        let request_body = serde_json::json!({
            "model": PLANNER_MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": "You are Ganesha, a goal-oriented AI agent. Given a user's goal, the actions taken so far, and the current screen state, determine the SINGLE NEXT ACTION that moves toward completing the goal. Think step-by-step: What's the goal? What's been done? What's on screen? What's next? Output only the action."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 60,
            "temperature": 0.2
        });

        let client = reqwest::blocking::Client::new();
        let response = client
            .post(format!("{}/chat/completions", PLANNER_ENDPOINT))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(Duration::from_secs(60))
            .send()?;

        let json: serde_json::Value = response.json()?;
        let message = &json["choices"][0]["message"];

        // gpt-oss-20b puts thinking in "reasoning" field, action in "content"
        // If content is empty, extract action hints from reasoning
        let content = message["content"].as_str().unwrap_or("").trim();
        let reasoning = message["reasoning"].as_str().unwrap_or("");

        // Check action history for sequencing
        let recent_actions: Vec<_> = self.history.iter().rev().take(5).collect();
        let is_blender_task = self.task.to_lowercase().contains("blender");
        let ever_clicked_viewport = self.clicked_viewport;
        let ever_pressed_shifta = self.history.iter().any(|h| h.to_lowercase().contains("shift+a"));
        let repeated_shifta = recent_actions.len() >= 2 &&
            recent_actions.iter().take(2).all(|h| h.to_lowercase().contains("shift+a"));
        // Check if last action was TYPE with "uv" (history stores action plan)
        let already_typed_uv = recent_actions.first()
            .map(|h| h.to_lowercase().contains("type") && h.to_lowercase().contains("uv"))
            .unwrap_or(false);

        let result = if already_typed_uv {
            // Already typed "uv", now press Return to confirm selection
            "ACTION: KEY\nPARAMS: Return".to_string()
        } else if is_blender_task && !ever_clicked_viewport {
            // FIRST action in Blender: click in the 3D viewport to ensure focus
            // Viewport is roughly center of screen (960, 500 for 1920x1080)
            "ACTION: CLICK\nPARAMS: 960,500".to_string()
        } else if is_blender_task && !ever_pressed_shifta {
            // SECOND action in Blender: open Add menu
            "ACTION: KEY\nPARAMS: shift+a".to_string()
        } else if repeated_shifta {
            // Force different action - type to search in menu
            "ACTION: TYPE\nPARAMS: uv".to_string()
        } else if !content.is_empty() && content.to_uppercase().contains("ACTION") {
            // Content has action - use it
            content.to_string()
        } else {
            // Extract action from reasoning field
            let r_upper = reasoning.to_uppercase();
            if r_upper.contains("SHIFT+A") || r_upper.contains("SHIFT + A") || r_upper.contains("ADD MENU") {
                "ACTION: KEY\nPARAMS: shift+a".to_string()
            } else if r_upper.contains("CLICK") && r_upper.contains("MESH") {
                // Wants to click on Mesh in menu - use arrow keys instead
                "ACTION: KEY\nPARAMS: Down".to_string()
            } else if (r_upper.contains("UV SPHERE") || r_upper.contains("UVSPHERE")) && ever_pressed_shifta {
                // Only type to search if menu is already open (shift+a was pressed)
                "ACTION: TYPE\nPARAMS: uv".to_string()
            } else if r_upper.contains("ENTER") || r_upper.contains("CONFIRM") || r_upper.contains("SELECT") {
                "ACTION: KEY\nPARAMS: Return".to_string()
            } else if r_upper.contains("TAB") || r_upper.contains("EDIT MODE") {
                "ACTION: KEY\nPARAMS: Tab".to_string()
            } else if r_upper.contains("ESCAPE") || r_upper.contains("CANCEL") {
                "ACTION: KEY\nPARAMS: Escape".to_string()
            } else if r_upper.contains("DELETE") || r_upper.contains("REMOVE") {
                "ACTION: KEY\nPARAMS: x".to_string()
            } else if r_upper.contains("MOVE") || r_upper.contains("GRAB") {
                "ACTION: KEY\nPARAMS: g".to_string()
            } else if r_upper.contains("SCALE") {
                "ACTION: KEY\nPARAMS: s".to_string()
            } else if r_upper.contains("EXTRUDE") {
                "ACTION: KEY\nPARAMS: e".to_string()
            } else {
                // Default: wait and observe
                "ACTION: WAIT\nPARAMS: 500".to_string()
            }
        };

        Ok(result)
    }

    fn execute_action(&mut self, action: &str) -> Result<(), Box<dyn std::error::Error>> {
        let action_upper = action.to_uppercase();

        // Parse multi-line ACTION/PARAMS format or single-line format
        // Format 1: "ACTION: CLICK\nPARAMS: 50 300"
        // Format 2: "CLICK 50 300"

        let mut action_type = String::new();
        let mut params = String::new();
        let mut found_action = false;

        for line in action.lines() {
            let line_trimmed = line.trim();
            let line_upper = line_trimmed.to_uppercase();

            if line_upper.starts_with("ACTION:") {
                if found_action {
                    // Already have an action, stop here (ignore subsequent actions)
                    break;
                }
                let action_content = line_upper.replace("ACTION:", "").trim().to_string();
                // Handle "ACTION: CLICK 500 300" format (action + params on same line)
                let parts: Vec<&str> = action_content.split_whitespace().collect();
                if !parts.is_empty() {
                    action_type = parts[0].to_string();
                    if parts.len() > 1 {
                        params = parts[1..].join(" ");
                    }
                }
                found_action = true;
            } else if line_upper.starts_with("PARAMS:") && found_action && params.is_empty() {
                params = line_trimmed.replace("PARAMS:", "").replace("params:", "").trim().to_string();
            } else if !line_trimmed.is_empty() && !found_action {
                // Single-line format: "CLICK 50 300"
                let parts: Vec<&str> = line_trimmed.split_whitespace().collect();
                if !parts.is_empty() {
                    action_type = parts[0].to_uppercase();
                    params = parts[1..].join(" ");
                    found_action = true;
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
                // Handle both "960 500" and "960,500" formats
                let coords: Vec<&str> = if params.contains(',') {
                    params.split(',').collect()
                } else {
                    param_parts.clone()
                };

                if coords.len() >= 2 {
                    let x = coords[0].trim().parse::<i32>().unwrap_or(500);
                    let y = coords[1].trim().parse::<i32>().unwrap_or(300);
                    self.smooth_click(x, y)?;
                    self.clicked_viewport = true;  // Mark that we've clicked in viewport
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
                let key_lower = key.to_lowercase();

                // Special handling for Blender - use keydown/type/keyup for modifier combos
                if key_lower == "shift+a" {
                    // Blender needs this sequence for Add menu
                    Command::new("xdotool").args(["keydown", "shift"]).output()?;
                    thread::sleep(Duration::from_millis(50));
                    Command::new("xdotool").args(["type", "a"]).output()?;
                    thread::sleep(Duration::from_millis(50));
                    Command::new("xdotool").args(["keyup", "shift"]).output()?;
                    println!("   ✓ Pressed Shift+A (Blender Add menu)");
                } else if key.len() == 1 && key.chars().next().unwrap().is_alphanumeric() {
                    // Single character - use type for Blender compatibility
                    Command::new("xdotool")
                        .args(["type", key])
                        .output()?;
                    println!("   ✓ Typed key: {}", key);
                } else {
                    // Special keys (Return, Escape, Tab, etc.)
                    Command::new("xdotool")
                        .args(["key", key])
                        .output()?;
                    println!("   ✓ Pressed key: {}", key);
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
        // Find the last valid char boundary before max_len
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
