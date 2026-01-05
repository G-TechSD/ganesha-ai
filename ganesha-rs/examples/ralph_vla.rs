//! Ralph VLA - Hybrid Planner + VLA Executor
//!
//! Best of both worlds:
//! - Beast (gpt-oss-20b): Smart planner with world knowledge
//! - ShowUI (2B): Precise VLA that grounds actions to coordinates
//!
//! Flow:
//!   Goal → Beast plans steps → ShowUI executes each step precisely
//!
//! Unlike the dual-model approach where vision describes and planner guesses,
//! ShowUI actually SEES the screen and outputs exact click coordinates.
//!
//! Run with: cargo run --example ralph_vla --features computer-use

use ganesha::agent::AgentControl;
use ganesha::input::InputController;
use ganesha::vision::VisionController;
use std::sync::Arc;
use std::time::Duration;
use std::process::Command;
use tokio::time::sleep;

// ShowUI VLA endpoint (Bedroom Windows 11 + RTX 2080 Ti)
const VLA_ENDPOINT: &str = "http://192.168.27.182:1235/v1/gui/action";

// Beast planner endpoint
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

/// A planned step from Beast (what to do)
#[derive(Debug, Clone)]
struct PlannedStep {
    instruction: String,  // e.g., "Click on the Firefox icon in the dock"
    action_hint: String,  // e.g., "click", "type", "key"
}

/// An executable action from ShowUI (how to do it)
#[derive(Debug, Clone)]
struct VLAAction {
    action_type: String,
    coordinates: Option<(i32, i32)>,
    text: Option<String>,
    key: Option<String>,
    confidence: f32,
    raw_response: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           RALPH VLA                                           ║");
    println!("║           Single-model Vision-Language-Action                 ║");
    println!("║           Screen → ShowUI-2B → Direct Action                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = Arc::new(VisionController::new());
    let input = Arc::new(InputController::new());
    let mut control = AgentControl::new();

    vision.enable().map_err(box_err)?;
    input.enable().map_err(box_err)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Check if VLA server is running
    println!("[*] Checking VLA server...");
    match client.get("http://127.0.0.1:1235/health").send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("[✓] ShowUI VLA server is running");
        }
        _ => {
            println!("[✗] ShowUI VLA server not found at {}", VLA_ENDPOINT);
            println!();
            println!("To start the server on Bedroom:");
            println!("  cd /home/bill/projects/showui/ShowUI");
            println!("  source venv/bin/activate");
            println!("  python showui_server.py");
            return Ok(());
        }
    }

    // Get goal from user
    println!();
    println!("What would you like me to do?");
    println!("Examples:");
    println!("  - Click on the Firefox icon");
    println!("  - Open the file manager");
    println!("  - Type 'Hello World' in the text editor");
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
    println!("[*] Engaging VLA mode...\n");

    // Take control with overlay
    control.take_control(false, false)?;

    // Run the VLA loop
    let result = run_vla(&client, &vision, &input, &control, goal).await;

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

async fn run_vla(
    client: &reqwest::Client,
    vision: &VisionController,
    input: &InputController,
    control: &AgentControl,
    goal: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let mut step_count = 0;
    let max_steps = 15;
    let mut history: Vec<String> = Vec::new();

    println!("[*] Phase 1: Beast plans the steps...");

    // Get initial plan from Beast
    let mut planned_steps = get_plan_from_beast(client, goal).await?;

    println!("    Plan: {} steps", planned_steps.len());
    for (i, step) in planned_steps.iter().enumerate() {
        println!("    {}. {} ({})", i + 1, step.instruction, step.action_hint);
    }
    println!();

    println!("[*] Phase 2: ShowUI executes each step...\n");

    while let Some(step) = planned_steps.first().cloned() {
        if control.is_interrupted() {
            return Err("Interrupted by user".into());
        }

        step_count += 1;
        if step_count > max_steps {
            break;
        }

        println!("[Step {}] {}", step_count, step.instruction);

        // Capture current screen
        let screenshot = vision.capture_screen_fast()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        // ShowUI grounds the instruction to exact action
        let action = get_vla_action(client, &screenshot.data, &step.instruction).await?;

        if action.action_type == "unknown" || action.confidence < 0.5 {
            println!("  [!] ShowUI uncertain ({}%), asking Beast for alternative...",
                    (action.confidence * 100.0) as i32);

            // Ask Beast for clarification
            let alt_instruction = get_alternative_from_beast(
                client, goal, &step.instruction, &action.raw_response
            ).await?;

            if !alt_instruction.is_empty() {
                println!("  [*] Trying: {}", alt_instruction);
                let alt_action = get_vla_action(client, &screenshot.data, &alt_instruction).await?;
                if alt_action.confidence > action.confidence {
                    // Use the alternative
                    let result = execute_vla_action(input, &alt_action).await;
                    match result {
                        Ok(msg) => {
                            println!("  Result: ✓ {}", msg);
                            history.push(format!("{}", step.instruction));
                            planned_steps.remove(0);
                        }
                        Err(e) => println!("  Result: ✗ {}", e),
                    }
                    sleep(Duration::from_millis(600)).await;
                    continue;
                }
            }
        }

        // Execute the VLA action
        match action.action_type.as_str() {
            "click" => {
                if let Some((x, y)) = action.coordinates {
                    println!("  → click({}, {})", x, y);
                }
            }
            "type" => {
                if let Some(ref text) = action.text {
                    println!("  → type(\"{}\")", &text[..text.len().min(30)]);
                }
            }
            "key" => {
                if let Some(ref key) = action.key {
                    println!("  → key({})", key);
                }
            }
            _ => {}
        }

        let result = execute_vla_action(input, &action).await;

        match result {
            Ok(msg) => {
                println!("  Result: ✓ {}", msg);
                history.push(step.instruction.clone());
                planned_steps.remove(0);  // Step completed, remove from plan
            }
            Err(e) => {
                println!("  Result: ✗ {}", e);
                // Don't remove - will retry or skip after max attempts
            }
        }

        // Brief pause for UI to update
        sleep(Duration::from_millis(600)).await;
    }

    if planned_steps.is_empty() {
        Ok(format!("Completed in {} steps.\n\nActions: {}",
                  step_count, history.join(" → ")))
    } else {
        Err(format!("Incomplete: {} steps remaining", planned_steps.len()).into())
    }
}

/// Ask Beast to create a step-by-step plan
async fn get_plan_from_beast(
    client: &reqwest::Client,
    goal: &str,
) -> Result<Vec<PlannedStep>, Box<dyn std::error::Error + Send + Sync>> {

    let prompt = format!(r#"You are a GUI automation planner. Break this goal into specific UI actions.

Goal: {}

Output each step on a new line in format:
ACTION_TYPE: instruction

Where ACTION_TYPE is one of: CLICK, TYPE, KEY, SCROLL, WAIT, DONE

Example for "Open Firefox and search for cats":
CLICK: Click on the Firefox icon in the dock or applications menu
WAIT: Wait for Firefox to open
CLICK: Click on the URL/search bar at the top
TYPE: cats
KEY: Press Enter to search
DONE: Task complete

Now plan for the goal above:"#, goal);

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "max_tokens": 300,
        "temperature": 0.1
    });

    let response = client.post(PLANNER_ENDPOINT)
        .json(&request)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    // Also check reasoning field for this model
    let reasoning = result["choices"][0]["message"]["reasoning"]
        .as_str()
        .unwrap_or("");

    let text = if content.trim().is_empty() { reasoning } else { content };

    // Parse steps
    let mut steps = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        // Parse "ACTION: instruction" format
        if let Some(colon_idx) = line.find(':') {
            let action_hint = line[..colon_idx].trim().to_uppercase();
            let instruction = line[colon_idx + 1..].trim().to_string();

            if action_hint == "DONE" {
                break;  // Plan complete
            }

            if !instruction.is_empty() {
                steps.push(PlannedStep {
                    instruction,
                    action_hint: action_hint.to_lowercase(),
                });
            }
        }
    }

    // Fallback if no steps parsed
    if steps.is_empty() {
        steps.push(PlannedStep {
            instruction: goal.to_string(),
            action_hint: "click".to_string(),
        });
    }

    Ok(steps)
}

/// Ask Beast for an alternative instruction when ShowUI is uncertain
async fn get_alternative_from_beast(
    client: &reqwest::Client,
    goal: &str,
    failed_instruction: &str,
    vla_response: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let prompt = format!(
        r#"The GUI agent couldn't execute: "{}"
VLA response: "{}"

Suggest a simpler, more specific instruction to achieve the same thing.
Output just the new instruction, nothing else."#,
        failed_instruction, vla_response
    );

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "max_tokens": 100,
        "temperature": 0.2
    });

    let response = client.post(PLANNER_ENDPOINT)
        .json(&request)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    Ok(content.trim().to_string())
}

async fn get_vla_action(
    client: &reqwest::Client,
    screenshot_b64: &str,
    instruction: &str,
) -> Result<VLAAction, Box<dyn std::error::Error + Send + Sync>> {

    let request = serde_json::json!({
        "image": screenshot_b64,
        "instruction": instruction
    });

    let response = client.post(VLA_ENDPOINT)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("VLA server error: {}", response.status()).into());
    }

    let result: serde_json::Value = response.json().await?;

    Ok(VLAAction {
        action_type: result["action_type"].as_str().unwrap_or("unknown").to_string(),
        coordinates: result["coordinates"].as_array().map(|arr| {
            (arr[0].as_i64().unwrap_or(0) as i32,
             arr[1].as_i64().unwrap_or(0) as i32)
        }),
        text: result["text"].as_str().map(String::from),
        key: result["key"].as_str().map(String::from),
        confidence: result["confidence"].as_f64().unwrap_or(0.5) as f32,
        raw_response: result["raw_response"].as_str().unwrap_or("").to_string(),
    })
}

async fn execute_vla_action(
    input: &InputController,
    action: &VLAAction,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    match action.action_type.as_str() {
        "click" => {
            if let Some((x, y)) = action.coordinates {
                xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
                sleep(Duration::from_millis(50)).await;
                xdotool(&["click", "1"])?;
                Ok(format!("Clicked at ({}, {})", x, y))
            } else {
                Err("Click action missing coordinates".into())
            }
        }

        "type" => {
            if let Some(text) = &action.text {
                input.type_text(text).map_err(box_err)?;
                Ok(format!("Typed: {}", &text[..text.len().min(30)]))
            } else {
                Err("Type action missing text".into())
            }
        }

        "key" => {
            if let Some(key) = &action.key {
                xdotool(&["key", key])?;
                Ok(format!("Pressed: {}", key))
            } else {
                Err("Key action missing key name".into())
            }
        }

        "scroll" => {
            let direction = action.text.as_deref().unwrap_or("down");
            let button = if direction == "up" { "4" } else { "5" };
            xdotool(&["click", button])?;
            Ok(format!("Scrolled {}", direction))
        }

        "double_click" => {
            if let Some((x, y)) = action.coordinates {
                xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
                sleep(Duration::from_millis(50)).await;
                xdotool(&["click", "--repeat", "2", "--delay", "100", "1"])?;
                Ok(format!("Double-clicked at ({}, {})", x, y))
            } else {
                Err("Double-click action missing coordinates".into())
            }
        }

        "right_click" => {
            if let Some((x, y)) = action.coordinates {
                xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
                sleep(Duration::from_millis(50)).await;
                xdotool(&["click", "3"])?;
                Ok(format!("Right-clicked at ({}, {})", x, y))
            } else {
                Err("Right-click action missing coordinates".into())
            }
        }

        "drag" => {
            // Would need start and end coordinates
            Ok("Drag not yet implemented".to_string())
        }

        "wait" => {
            sleep(Duration::from_secs(1)).await;
            Ok("Waited 1 second".to_string())
        }

        "done" | "complete" => {
            Ok("Goal marked complete".to_string())
        }

        _ => {
            Ok(format!("Unknown action: {} (raw: {})",
                      action.action_type, &action.raw_response[..action.raw_response.len().min(50)]))
        }
    }
}

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e)
}

fn xdotool(args: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Command::new("xdotool")
        .args(args)
        .env("DISPLAY", ":1")
        .status()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    Ok(())
}
