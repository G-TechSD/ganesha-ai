//! Ganesha Fast Agent - Two-model architecture
//! Vision: ministral-3-3b (describe screen)
//! Planning: gpt-oss-20b (decide actions)

use std::env;
use std::process::Command;
use std::time::Duration;
use std::thread;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

const VISION_API: &str = "http://172.18.16.116:8234/v1/chat/completions";  // Bedroom - ministral
const PLANNING_API: &str = "http://172.18.16.116:8234/v1/chat/completions"; // Also Bedroom - ministral (gpt-oss uses reasoning mode)
const VISION_MODEL: &str = "mistralai/ministral-3-3b";
const PLANNING_MODEL: &str = "mistralai/ministral-3-3b";  // Same model for both - gpt-oss-20b reasoning mode doesn't work for simple actions

use std::process::Child;

/// Wrapper that ensures red border is killed when dropped
struct RedBorderGuard {
    child: Child,
}

impl Drop for RedBorderGuard {
    fn drop(&mut self) {
        eprintln!("🔴 Cleaning up red border...");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn get_display_env() -> (String, String) {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":1".to_string());
    let xauth = std::env::var("XAUTHORITY").unwrap_or_else(|_| {
        let uid = std::env::var("UID").unwrap_or_else(|_| "1000".to_string());
        format!("/run/user/{}/gdm/Xauthority", uid)
    });
    (display, xauth)
}

fn start_red_border() -> Option<Child> {
    // Solid 4-window border approach - works with Ubuntu compositor
    let script = r#"
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk, GLib
import signal
import sys

def quit_handler(*args):
    Gtk.main_quit()
    sys.exit(0)

signal.signal(signal.SIGTERM, quit_handler)
signal.signal(signal.SIGINT, quit_handler)

display = Gdk.Display.get_default()
if not display:
    print("ERROR: No display!", flush=True)
    sys.exit(1)

monitor = display.get_primary_monitor()
geom = monitor.get_geometry()
w, h = geom.width, geom.height

BORDER_WIDTH = 8
windows = []

def create_border_window(x, y, width, height):
    win = Gtk.Window(type=Gtk.WindowType.POPUP)
    win.set_default_size(width, height)
    win.move(x, y)
    win.set_decorated(False)
    win.set_keep_above(True)
    win.set_skip_taskbar_hint(True)
    win.set_skip_pager_hint(True)
    win.set_accept_focus(False)
    win.set_type_hint(Gdk.WindowTypeHint.DOCK)  # DOCK stays on top better than NOTIFICATION
    win.stick()  # Appear on all workspaces
    css = Gtk.CssProvider()
    css.load_from_data(b"window { background-color: #FF0000; }")
    win.get_style_context().add_provider(css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    win.show_all()
    return win

# Top border
windows.append(create_border_window(0, 0, w, BORDER_WIDTH))
# Bottom border
windows.append(create_border_window(0, h - BORDER_WIDTH, w, BORDER_WIDTH))
# Left border
windows.append(create_border_window(0, BORDER_WIDTH, BORDER_WIDTH, h - 2*BORDER_WIDTH))
# Right border
windows.append(create_border_window(w - BORDER_WIDTH, BORDER_WIDTH, BORDER_WIDTH, h - 2*BORDER_WIDTH))

# Periodically ensure windows stay on top
def keep_on_top():
    for win in windows:
        if win.get_window():
            win.get_window().raise_()
            win.set_keep_above(True)
    return True  # Keep timer running

GLib.timeout_add(500, keep_on_top)  # Every 500ms

print("RED_BORDER_ACTIVE", flush=True)
Gtk.main()
"#;

    let (display, xauth) = get_display_env();

    let mut cmd = Command::new("python3");
    cmd.args(["-c", script])
        .env("DISPLAY", &display)
        .env("XAUTHORITY", &xauth)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    match cmd.spawn() {
        Ok(mut child) => {
            // Wait briefly to confirm border started
            thread::sleep(Duration::from_millis(200));
            Some(child)
        }
        Err(e) => {
            eprintln!("Failed to start red border: {}", e);
            None
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let task = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "click on firefox".to_string()
    };

    println!("🕉️ GANESHA FAST - Two-Model Architecture");
    println!("   Task: {}", task);
    println!("   Vision: {}", VISION_MODEL);
    println!("   Planning: {}", PLANNING_MODEL);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // RED BORDER IS MANDATORY - DO NOT RUN WITHOUT IT
    let overlay = start_red_border();
    if overlay.is_none() {
        eprintln!("❌ FATAL: Red border overlay failed to start. REFUSING TO RUN.");
        eprintln!("   Install: sudo apt install python3-gi gir1.2-gtk-3.0");
        std::process::exit(1);
    }
    let mut overlay = RedBorderGuard { child: overlay.unwrap() };

    // Verify border is visible by waiting and checking
    thread::sleep(Duration::from_millis(500));

    // Check if border is still running
    match overlay.child.try_wait() {
        Ok(Some(status)) => {
            eprintln!("❌ FATAL: Red border exited immediately with status: {:?}", status);
            eprintln!("   The safety border MUST be visible during operation.");
            std::process::exit(1);
        }
        Ok(None) => {
            // Still running - good
            println!("🔴 Red border ACTIVE - safety indicator displayed");
        }
        Err(e) => {
            eprintln!("❌ FATAL: Could not check red border status: {}", e);
            std::process::exit(1);
        }
    }

    let mut history: Vec<String> = Vec::new();

    for iteration in 1..=20 {
        // SAFETY CHECK: Ensure red border is still running
        match overlay.child.try_wait() {
            Ok(Some(_)) => {
                eprintln!("\n❌ FATAL: Red border died! STOPPING IMMEDIATELY.");
                eprintln!("   Ganesha will NOT operate without the safety indicator.");
                std::process::exit(1);
            }
            Ok(None) => {} // Still running - good
            Err(_) => {
                eprintln!("\n❌ FATAL: Lost red border process! STOPPING.");
                std::process::exit(1);
            }
        }

        println!("\n🔄 Iteration {}/20", iteration);

        // 1. CAPTURE SCREEN
        print!("👁️  Capturing... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let screenshot = capture_screen()?;
        println!("✓");

        // 2. VISION: Describe what's on screen
        print!("🔍 Vision analyzing... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let description = get_vision_description(&screenshot)?;
        println!("✓");
        println!("   {}", &description[..description.len().min(100)]);

        // 3. PLANNING: Decide action based on description
        print!("🧠 Planning action... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let action = get_planned_action(&task, &description, &history)?;
        println!("✓");
        // Show full action for debugging
        let action_display = if action.len() > 80 {
            format!("{}...", &action[..80])
        } else {
            action.clone()
        };
        println!("   → {}", action_display);

        // 4. EXECUTE ACTION
        print!("🖱️  Executing... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let (done, action_desc) = execute_action(&action)?;
        println!("✓");

        history.push(format!("{} -> {}", &description[..description.len().min(50)], action_desc));
        if history.len() > 5 {
            history.remove(0);
        }

        if done {
            println!("\n✅ Task complete!");
            break;
        }

        thread::sleep(Duration::from_millis(300));
    }

    // Border cleanup handled by RedBorderGuard Drop implementation
    Ok(())
}

fn capture_screen() -> Result<String, Box<dyn std::error::Error>> {
    let (display, xauth) = get_display_env();

    Command::new("scrot")
        .args(["-o", "/tmp/ganesha_screen_full.png"])
        .env("DISPLAY", &display)
        .env("XAUTHORITY", &xauth)
        .output()?;

    Command::new("convert")
        .args([
            "/tmp/ganesha_screen_full.png",
            "-resize", "1280x720>",
            "-quality", "70",
            "/tmp/ganesha_screen.jpg"
        ])
        .output()?;

    let image_data = std::fs::read("/tmp/ganesha_screen.jpg")?;
    Ok(BASE64.encode(&image_data))
}

fn get_vision_description(screenshot_b64: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = r#"Describe this Ubuntu desktop screenshot in ONE line. Report:
- STATE: "ACTIVITIES" (if search bar visible at top), "DESKTOP" (normal), or "APP_OPEN:[name]" (if app window visible)
- SEARCH_TEXT: text already typed in search bar, or "empty"
- SEARCH_RESULTS: "firefox visible", "no results", or "searching"
- DOCK_FIREFOX: "yes" or "no" (firefox icon in left dock)

Example: STATE:ACTIVITIES, SEARCH_TEXT:fire, SEARCH_RESULTS:firefox visible, DOCK_FIREFOX:yes"#;

    let request_body = serde_json::json!({
        "model": VISION_MODEL,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": format!("data:image/jpeg;base64,{}", screenshot_b64)}}
            ]
        }],
        "max_tokens": 150,
        "temperature": 0.1
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let response = client
        .post(VISION_API)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()?;

    let json: serde_json::Value = response.json()?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Unable to describe screen")
        .to_string();

    Ok(content)
}

fn get_planned_action(task: &str, screen_description: &str, history: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let history_str = if history.is_empty() {
        "None".to_string()
    } else {
        history.join("\n")
    };

    // Simple decision tree based on screen state
    let prompt = format!(r#"Task: {}
Screen: {}
Last actions: {}

DECISION TREE (follow in order):
1. APP_OPEN:Firefox → {{"action":"DONE"}}
2. SEARCH_RESULTS:firefox visible → {{"action":"KEY","key":"Return"}}
3. SEARCH_TEXT has text (not empty) → {{"action":"KEY","key":"Return"}}
4. STATE:ACTIVITIES + SEARCH_TEXT:empty → {{"action":"TYPE","text":"firefox"}}
5. STATE:DESKTOP + DOCK_FIREFOX:yes → {{"action":"CLICK","x":25,"y":50}}
6. STATE:DESKTOP → {{"action":"KEY","key":"super"}}

Output ONLY the JSON for the matching condition:"#, task, screen_description, history_str);

    let request_body = serde_json::json!({
        "model": PLANNING_MODEL,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 100,
        "temperature": 0.1
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()?;

    let response = client
        .post(PLANNING_API)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()?;

    let json: serde_json::Value = response.json()?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or(r#"{"action": "DONE"}"#)
        .trim()
        .to_string();

    Ok(content)
}

fn execute_action(action_json: &str) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let (display, xauth) = get_display_env();

    // Clean up JSON - remove markdown, find JSON object
    let mut clean = action_json.trim().to_string();

    if clean.starts_with("```") {
        if let Some(pos) = clean.find('\n') {
            clean = clean[pos + 1..].to_string();
        }
    }
    if clean.ends_with("```") {
        clean = clean[..clean.len() - 3].to_string();
    }

    // Strip // comments from JSON (ministral-3-3b quirk)
    let lines: Vec<&str> = clean.lines()
        .map(|line| {
            if let Some(pos) = line.find("//") {
                line[..pos].trim_end()
            } else {
                line
            }
        })
        .collect();
    clean = lines.join("\n");

    if let Some(start) = clean.find('{') {
        if let Some(end) = clean.rfind('}') {
            clean = clean[start..=end].to_string();
        }
    }

    let action: serde_json::Value = serde_json::from_str(&clean)
        .unwrap_or_else(|_| serde_json::json!({"action": "KEY", "key": "Return"}));

    let action_type = action["action"].as_str().unwrap_or("DONE").to_uppercase();

    let desc = match action_type.as_str() {
        "CLICK" => {
            let x = action["x"].as_i64().unwrap_or(500) as i32;
            let y = action["y"].as_i64().unwrap_or(300) as i32;
            Command::new("xdotool")
                .args(["mousemove", &x.to_string(), &y.to_string()])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            thread::sleep(Duration::from_millis(50));
            Command::new("xdotool")
                .args(["click", "1"])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            format!("CLICK ({}, {})", x, y)
        }
        "DOUBLE_CLICK" => {
            let x = action["x"].as_i64().unwrap_or(500) as i32;
            let y = action["y"].as_i64().unwrap_or(300) as i32;
            Command::new("xdotool")
                .args(["mousemove", &x.to_string(), &y.to_string()])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            thread::sleep(Duration::from_millis(50));
            Command::new("xdotool")
                .args(["click", "--repeat", "2", "--delay", "100", "1"])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            format!("DOUBLE_CLICK ({}, {})", x, y)
        }
        "KEY" => {
            let key = action["key"].as_str().unwrap_or("Return");
            Command::new("xdotool")
                .args(["key", key])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            // Extra delay for Super key to let Activities view open
            if key.to_lowercase() == "super" {
                thread::sleep(Duration::from_millis(800));
            }
            format!("KEY {}", key)
        }
        "TYPE" => {
            let text = action["text"].as_str().unwrap_or("");
            Command::new("xdotool")
                .args(["type", "--delay", "50", "--", text])
                .env("DISPLAY", &display)
                .env("XAUTHORITY", &xauth)
                .output()?;
            format!("TYPE \"{}\"", text)
        }
        "DONE" => {
            return Ok((true, "DONE".to_string()));
        }
        _ => {
            format!("UNKNOWN: {}", action_type)
        }
    };

    Ok((false, desc))
}
