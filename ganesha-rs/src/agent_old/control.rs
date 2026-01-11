//! Agent Control System
//!
//! Provides visual/audio indicators and input control for safe agent operation:
//! - Red frame overlay when agent is in control
//! - Optional audio indicator
//! - Input grab (blocks user mouse/keyboard)
//! - Foolproof interrupt mechanism (both Shift keys + Escape)
//!
//! The interrupt is monitored at the evdev (raw hardware) level, so it works
//! even when X11 input is grabbed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::process::{Command, Child, Stdio};
use std::time::{Duration, Instant};
use std::io::{BufRead, BufReader};
use std::thread;

/// The sacred interrupt sequence: Both Shift keys + Escape
/// This is monitored at hardware level and cannot be blocked
pub const INTERRUPT_SEQUENCE: &str = "LEFT_SHIFT + RIGHT_SHIFT + ESCAPE";

/// Agent control state
pub struct AgentControl {
    /// Is the agent currently in control?
    active: Arc<AtomicBool>,
    /// Has user requested interrupt?
    interrupted: Arc<AtomicBool>,
    /// Overlay window process
    overlay_proc: Option<Child>,
    /// Sound loop process
    sound_proc: Option<Child>,
    /// Input grab active
    input_grabbed: bool,
    /// Original xinput device states (for restore)
    disabled_devices: Vec<String>,
    /// Interrupt monitor thread handle
    interrupt_monitor: Option<thread::JoinHandle<()>>,
}

impl AgentControl {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            interrupted: Arc::new(AtomicBool::new(false)),
            overlay_proc: None,
            sound_proc: None,
            input_grabbed: false,
            disabled_devices: Vec::new(),
            interrupt_monitor: None,
        }
    }

    /// Take control - shows overlay, optionally grabs input
    pub fn take_control(&mut self, grab_input: bool, play_sound: bool) -> Result<(), ControlError> {
        if self.active.load(Ordering::SeqCst) {
            return Ok(()); // Already in control
        }

        self.interrupted.store(false, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);

        // Start overlay
        self.start_overlay()?;

        // Start interrupt monitor (before grabbing input!)
        self.start_interrupt_monitor();

        // Optionally grab input
        if grab_input {
            self.grab_input()?;
        }

        // Optionally play sound
        if play_sound {
            self.start_sound_loop();
        }

        println!("[AgentControl] ⚠️  AGENT IN CONTROL");
        println!("[AgentControl] Interrupt: {}", INTERRUPT_SEQUENCE);

        Ok(())
    }

    /// Release control - hides overlay, releases input
    pub fn release_control(&mut self) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }

        self.active.store(false, Ordering::SeqCst);

        // Release input first
        self.release_input();

        // Stop sound
        self.stop_sound_loop();

        // Stop overlay
        self.stop_overlay();

        println!("[AgentControl] ✓ Control released");
    }

    /// Check if user has requested interrupt
    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    /// Check if agent is in control
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Start the red frame overlay
    fn start_overlay(&mut self) -> Result<(), ControlError> {
        // Create overlay using Python + GTK (most reliable cross-platform)
        let overlay_script = r#"
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk
import cairo
import signal
import sys

signal.signal(signal.SIGTERM, lambda *_: Gtk.main_quit())
signal.signal(signal.SIGINT, lambda *_: Gtk.main_quit())

class RedFrameOverlay(Gtk.Window):
    def __init__(self):
        super().__init__(type=Gtk.WindowType.POPUP)

        screen = Gdk.Screen.get_default()
        self.set_size_request(screen.get_width(), screen.get_height())
        self.set_position(Gtk.WindowPosition.CENTER)
        self.set_decorated(False)
        self.set_app_paintable(True)
        self.set_keep_above(True)
        self.set_skip_taskbar_hint(True)
        self.set_skip_pager_hint(True)
        self.set_accept_focus(False)
        self.set_can_focus(False)

        # Make window transparent and click-through
        self.set_visual(screen.get_rgba_visual())
        self.connect('draw', self.on_draw)
        self.connect('realize', self.on_realize)

    def on_realize(self, widget):
        # Make click-through
        region = cairo.Region(cairo.RectangleInt(0, 0, 0, 0))
        self.get_window().input_shape_combine_region(region, 0, 0)

    def on_draw(self, widget, cr):
        # Clear background
        cr.set_operator(cairo.OPERATOR_SOURCE)
        cr.set_source_rgba(0, 0, 0, 0)
        cr.paint()

        # Draw red frame (8 pixels thick)
        width = self.get_allocated_width()
        height = self.get_allocated_height()
        thickness = 8

        cr.set_source_rgba(1, 0, 0, 0.9)  # Bright red, slightly transparent
        cr.set_line_width(thickness)

        # Draw rectangle just inside the edges
        cr.rectangle(thickness/2, thickness/2,
                    width - thickness, height - thickness)
        cr.stroke()

        # Pulsing effect - redraw periodically
        return False

win = RedFrameOverlay()
win.show_all()

# Pulse animation
import threading
def pulse():
    import time
    while True:
        time.sleep(0.5)
        try:
            win.queue_draw()
        except:
            break

t = threading.Thread(target=pulse, daemon=True)
t.start()

Gtk.main()
"#;

        // Write script to temp file
        let script_path = "/tmp/ganesha_overlay.py";
        std::fs::write(script_path, overlay_script)
            .map_err(|e| ControlError::OverlayFailed(e.to_string()))?;

        // Launch overlay - use DISPLAY from environment or default to :1
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":1".to_string());
        println!("[AgentControl] Starting red border overlay on DISPLAY={}", display);

        let proc = Command::new("python3")
            .arg(script_path)
            .env("DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())  // Capture stderr for debugging
            .spawn()
            .map_err(|e| ControlError::OverlayFailed(e.to_string()))?;

        self.overlay_proc = Some(proc);

        // Give it time to appear
        std::thread::sleep(Duration::from_millis(200));

        Ok(())
    }

    /// Stop the overlay
    fn stop_overlay(&mut self) {
        if let Some(mut proc) = self.overlay_proc.take() {
            let _ = proc.kill();
            let _ = proc.wait();
        }
        let _ = std::fs::remove_file("/tmp/ganesha_overlay.py");
    }

    /// Start looping sound effect
    fn start_sound_loop(&mut self) {
        // Generate a subtle tone using paplay or create a simple beep
        // Using a low-volume ambient sound

        // First try to use a built-in sound
        let sounds = [
            "/usr/share/sounds/freedesktop/stereo/message.oga",
            "/usr/share/sounds/gnome/default/alerts/glass.ogg",
            "/usr/share/sounds/ubuntu/stereo/message.ogg",
        ];

        let sound_file = sounds.iter().find(|p| std::path::Path::new(p).exists());

        if let Some(sound) = sound_file {
            // Loop the sound at low volume using a shell loop
            let proc = Command::new("bash")
                .args(["-c", &format!(
                    "while true; do paplay --volume=16384 {} 2>/dev/null || break; sleep 2; done",
                    sound
                )])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(p) = proc {
                self.sound_proc = Some(p);
            }
        }
    }

    /// Stop the sound loop
    fn stop_sound_loop(&mut self) {
        if let Some(mut proc) = self.sound_proc.take() {
            let _ = proc.kill();
            let _ = proc.wait();
        }
        // Kill any lingering paplay
        let _ = Command::new("pkill").args(["-f", "paplay.*ganesha"]).status();
    }

    /// Grab input - disable user's mouse/keyboard at X11 level
    /// Agent still works via xdotool (which injects at X server level)
    fn grab_input(&mut self) -> Result<(), ControlError> {
        // Get list of input devices
        let output = Command::new("xinput")
            .arg("list")
            .env("DISPLAY", ":1")
            .output()
            .map_err(|e| ControlError::InputGrabFailed(e.to_string()))?;

        let list = String::from_utf8_lossy(&output.stdout);

        // Find keyboard and mouse devices (skip virtual/xdotool devices)
        let mut devices_to_disable = Vec::new();

        for line in list.lines() {
            let line_lower = line.to_lowercase();

            // Skip virtual devices, xtest (used by xdotool), and core devices
            if line_lower.contains("virtual") ||
               line_lower.contains("xtest") ||
               line_lower.contains("power button") ||
               line_lower.contains("video bus") {
                continue;
            }

            // Find actual keyboards and mice
            if (line_lower.contains("keyboard") || line_lower.contains("mouse") ||
                line_lower.contains("pointer") || line_lower.contains("touchpad")) &&
               line.contains("id=") {
                // Extract device ID
                if let Some(id_start) = line.find("id=") {
                    let id_part = &line[id_start + 3..];
                    if let Some(id_end) = id_part.find(|c: char| !c.is_numeric()) {
                        let id = &id_part[..id_end];
                        devices_to_disable.push(id.to_string());
                    }
                }
            }
        }

        // Disable each device
        for device_id in &devices_to_disable {
            let result = Command::new("xinput")
                .args(["disable", device_id])
                .env("DISPLAY", ":1")
                .status();

            if result.is_ok() {
                self.disabled_devices.push(device_id.clone());
            }
        }

        self.input_grabbed = true;
        println!("[AgentControl] Input grabbed ({} devices disabled)", self.disabled_devices.len());

        Ok(())
    }

    /// Release input - re-enable user's devices
    fn release_input(&mut self) {
        for device_id in &self.disabled_devices {
            let _ = Command::new("xinput")
                .args(["enable", device_id])
                .env("DISPLAY", ":1")
                .status();
        }

        self.disabled_devices.clear();
        self.input_grabbed = false;
    }

    /// Start monitoring for interrupt sequence at evdev level
    fn start_interrupt_monitor(&mut self) {
        let interrupted = Arc::clone(&self.interrupted);
        let active = Arc::clone(&self.active);

        let handle = thread::spawn(move || {
            // Monitor /dev/input/event* for the interrupt sequence
            // This works even when X11 input is grabbed

            let mut left_shift = false;
            let mut right_shift = false;
            let mut last_check = Instant::now();

            // Try to find keyboard device
            let kbd_device = find_keyboard_device();

            if let Some(device_path) = kbd_device {
                // Use evtest or direct read
                if let Ok(mut child) = Command::new("evtest")
                    .arg(&device_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);

                        for line in reader.lines() {
                            if !active.load(Ordering::SeqCst) {
                                break;
                            }

                            if let Ok(line) = line {
                                // Parse evtest output for key events
                                if line.contains("KEY_LEFTSHIFT") {
                                    left_shift = line.contains("value 1");
                                } else if line.contains("KEY_RIGHTSHIFT") {
                                    right_shift = line.contains("value 1");
                                } else if line.contains("KEY_ESC") && line.contains("value 1") {
                                    // Check if both shifts are held
                                    if left_shift && right_shift {
                                        println!("\n[AgentControl] 🛑 INTERRUPT DETECTED!");
                                        interrupted.store(true, Ordering::SeqCst);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    let _ = child.kill();
                }
            } else {
                // Fallback: poll for interrupt using alternative method
                while active.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));

                    // Check if a specific file exists (user can touch it)
                    if std::path::Path::new("/tmp/ganesha_interrupt").exists() {
                        println!("\n[AgentControl] 🛑 INTERRUPT DETECTED (file signal)!");
                        interrupted.store(true, Ordering::SeqCst);
                        let _ = std::fs::remove_file("/tmp/ganesha_interrupt");
                        break;
                    }
                }
            }
        });

        self.interrupt_monitor = Some(handle);
    }
}

impl Drop for AgentControl {
    fn drop(&mut self) {
        self.release_control();
    }
}

/// Find keyboard event device
fn find_keyboard_device() -> Option<String> {
    // Read /proc/bus/input/devices to find keyboard
    if let Ok(content) = std::fs::read_to_string("/proc/bus/input/devices") {
        let mut current_handlers = String::new();
        let mut is_keyboard = false;

        for line in content.lines() {
            if line.starts_with("N: Name=") {
                let name = line.to_lowercase();
                is_keyboard = name.contains("keyboard") && !name.contains("virtual");
            } else if line.starts_with("H: Handlers=") {
                current_handlers = line.to_string();
            } else if line.is_empty() && is_keyboard {
                // Found a keyboard, extract event device
                for part in current_handlers.split_whitespace() {
                    if part.starts_with("event") {
                        return Some(format!("/dev/input/{}", part));
                    }
                }
            }
        }
    }

    // Fallback: try common paths
    for i in 0..10 {
        let path = format!("/dev/input/event{}", i);
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    None
}

#[derive(Debug)]
pub enum ControlError {
    OverlayFailed(String),
    InputGrabFailed(String),
    SoundFailed(String),
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::OverlayFailed(e) => write!(f, "Overlay failed: {}", e),
            ControlError::InputGrabFailed(e) => write!(f, "Input grab failed: {}", e),
            ControlError::SoundFailed(e) => write!(f, "Sound failed: {}", e),
        }
    }
}

impl std::error::Error for ControlError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_creation() {
        let control = AgentControl::new();
        assert!(!control.is_active());
        assert!(!control.is_interrupted());
    }
}
