//! AI Cursor - Visual Feedback for Ganesha's Mouse Control
//!
//! When Ganesha commands the mouse, the cursor transforms to show
//! that the AI is in control - a magical Ganesha symbol that
//! replaces the standard mouse cursor.
//!
//! The custom cursor persists for a configurable duration after the
//! last AI action so it doesn't disappear too quickly.
//!
//! On X11: Uses xsetroot/xcursor for actual cursor replacement
//! Fallback: Overlay window that follows the cursor

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::io::Write;
use std::env;

/// Cursor style when AI is in control
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorStyle {
    /// Default Ganesha symbol (ॐ)
    Ganesha,
    /// Elephant trunk icon
    Trunk,
    /// Eye symbol (vision active)
    Eye,
    /// Custom symbol
    Custom,
}

/// AI Cursor controller
pub struct AiCursor {
    /// Current style
    style: CursorStyle,
    /// Overlay process (yad or zenity)
    overlay_process: Arc<Mutex<Option<Child>>>,
    /// Last action timestamp
    last_action: Arc<Mutex<Instant>>,
    /// How long to keep cursor visible after last action
    linger_duration: Duration,
    /// Is the cursor currently visible
    is_visible: Arc<Mutex<bool>>,
    /// Custom symbol if using CursorStyle::Custom
    custom_symbol: String,
    /// Cursor size
    size: u32,
}

impl Default for AiCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl AiCursor {
    pub fn new() -> Self {
        Self {
            style: CursorStyle::Ganesha,
            overlay_process: Arc::new(Mutex::new(None)),
            last_action: Arc::new(Mutex::new(Instant::now())),
            linger_duration: Duration::from_secs(1), // Stay visible 1 second after last action
            is_visible: Arc::new(Mutex::new(false)),
            custom_symbol: String::new(),
            size: 48, // Larger than normal cursor
        }
    }

    /// Set cursor style
    pub fn with_style(mut self, style: CursorStyle) -> Self {
        self.style = style;
        self
    }

    /// Set linger duration (how long to stay visible after last action)
    pub fn with_linger(mut self, duration: Duration) -> Self {
        self.linger_duration = duration;
        self
    }

    /// Set custom symbol
    pub fn with_custom_symbol(mut self, symbol: &str) -> Self {
        self.custom_symbol = symbol.to_string();
        self.style = CursorStyle::Custom;
        self
    }

    /// Set cursor size
    pub fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    /// Get the symbol for current style
    fn get_symbol(&self) -> &str {
        match self.style {
            CursorStyle::Ganesha => "🕉️",  // Om symbol (Ganesha mantra)
            CursorStyle::Trunk => "🐘",    // Elephant
            CursorStyle::Eye => "👁️",      // Eye (vision)
            CursorStyle::Custom => &self.custom_symbol,
        }
    }

    /// Show the AI cursor at a specific position
    pub fn show_at(&self, x: i32, y: i32) -> Result<(), String> {
        // Update last action time
        *self.last_action.lock().unwrap() = Instant::now();

        // Kill existing overlay if any
        self.hide();

        // Create new overlay at cursor position
        // Using yad for a floating label
        let symbol = self.get_symbol();

        // Create overlay window slightly offset from cursor
        let offset_x = x + 20;
        let offset_y = y + 20;

        let child = Command::new("yad")
            .args([
                "--text-info",
                "--no-buttons",
                "--undecorated",
                "--skip-taskbar",
                "--on-top",
                "--sticky",
                "--geometry",
                &format!("{}x{}+{}+{}", self.size + 10, self.size + 10, offset_x, offset_y),
                "--fore", "#FFD700",  // Gold color
                "--back", "#00000080", // Semi-transparent black
                "--fontname", &format!("Sans {}", self.size),
                "--timeout", &(self.linger_duration.as_secs() + 5).to_string(), // Auto-close
            ])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn cursor overlay: {}", e))?;

        // Write symbol to stdin
        if let Some(mut stdin) = child.stdin.as_ref().and_then(|_| None::<std::process::ChildStdin>) {
            use std::io::Write;
            let _ = stdin.write_all(symbol.as_bytes());
        }

        // Alternative: use a simpler approach with notify-send or a custom script
        // For now, let's use a GTK-based approach that's more reliable

        *self.overlay_process.lock().unwrap() = Some(child);
        *self.is_visible.lock().unwrap() = true;

        Ok(())
    }

    /// Show cursor overlay using a simpler GTK approach
    pub fn show_cursor_overlay(&self, x: i32, y: i32) -> Result<(), String> {
        // Update last action time
        *self.last_action.lock().unwrap() = Instant::now();

        let symbol = self.get_symbol();

        // Use a floating GTK window via yad's notification mode
        // This is more reliable for cursor following
        let child = Command::new("yad")
            .args([
                "--notification",
                "--image", "dialog-information",
                "--text", &format!("{} AI Active", symbol),
                "--command", "echo",
                "--no-middle",
            ])
            .spawn()
            .map_err(|e| format!("Failed to show notification: {}", e))?;

        *self.overlay_process.lock().unwrap() = Some(child);
        *self.is_visible.lock().unwrap() = true;

        Ok(())
    }

    /// Show the AI cursor at current mouse position
    pub fn show(&self) -> Result<(), String> {
        // Get current mouse position using xdotool
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .map_err(|e| format!("Failed to get mouse position: {}", e))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut x = 0i32;
        let mut y = 0i32;

        for line in output_str.lines() {
            if line.starts_with("X=") {
                x = line[2..].parse().unwrap_or(0);
            } else if line.starts_with("Y=") {
                y = line[2..].parse().unwrap_or(0);
            }
        }

        self.show_at(x, y)
    }

    /// Hide the AI cursor
    pub fn hide(&self) {
        let mut proc = self.overlay_process.lock().unwrap();
        if let Some(ref mut child) = *proc {
            let _ = child.kill();
        }
        *proc = None;
        *self.is_visible.lock().unwrap() = false;
    }

    /// Check if cursor should still be visible (within linger duration)
    pub fn should_linger(&self) -> bool {
        let last = *self.last_action.lock().unwrap();
        last.elapsed() < self.linger_duration
    }

    /// Called when AI performs a mouse action - updates cursor position
    pub fn on_mouse_action(&self, x: i32, y: i32) -> Result<(), String> {
        self.show_at(x, y)
    }

    /// Called when AI is done with mouse control
    /// Cursor will linger for the configured duration
    pub fn on_mouse_release(&self) {
        // Don't hide immediately - let it linger
        let overlay = self.overlay_process.clone();
        let linger = self.linger_duration;
        let is_visible = self.is_visible.clone();

        std::thread::spawn(move || {
            std::thread::sleep(linger);
            let mut proc = overlay.lock().unwrap();
            if let Some(ref mut child) = *proc {
                let _ = child.kill();
            }
            *proc = None;
            *is_visible.lock().unwrap() = false;
        });
    }

    /// Update cursor position without creating new window
    pub fn update_position(&self, x: i32, y: i32) -> Result<(), String> {
        // For now, recreate the overlay at new position
        // A more sophisticated implementation would use GTK window move
        self.show_at(x, y)
    }

    /// Is the cursor currently visible?
    pub fn is_visible(&self) -> bool {
        *self.is_visible.lock().unwrap()
    }

    /// Create a cursor overlay using a custom X11 cursor
    /// This requires xcursor-themes and xdotool
    pub fn set_system_cursor(&self) -> Result<(), String> {
        // Create and apply custom cursor
        let cursor_manager = X11CursorManager::new()?;
        cursor_manager.set_ganesha_cursor()
    }

    /// Restore the default system cursor
    pub fn restore_system_cursor(&self) -> Result<(), String> {
        let cursor_manager = X11CursorManager::new()?;
        cursor_manager.restore_default()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// X11 CURSOR MANAGER - Actual mouse cursor replacement
// ═══════════════════════════════════════════════════════════════════════════════

/// Manages X11 cursor theming for AI mouse control
pub struct X11CursorManager {
    cursor_dir: PathBuf,
    original_theme: Option<String>,
}

impl X11CursorManager {
    pub fn new() -> Result<Self, String> {
        // Get cache directory without external dependency
        let cache_base = env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".cache"))
                    .unwrap_or_else(|_| PathBuf::from("/tmp"))
            });
        let cursor_dir = cache_base.join("ganesha").join("cursors");

        std::fs::create_dir_all(&cursor_dir)
            .map_err(|e| format!("Failed to create cursor directory: {}", e))?;

        // Get current cursor theme
        let original_theme = Self::get_current_theme();

        Ok(Self {
            cursor_dir,
            original_theme,
        })
    }

    /// Get current cursor theme name
    fn get_current_theme() -> Option<String> {
        // Try gsettings (GNOME)
        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "cursor-theme"])
            .output()
        {
            let theme = String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            if !theme.is_empty() {
                return Some(theme);
            }
        }

        // Try xfconf (XFCE)
        if let Ok(output) = Command::new("xfconf-query")
            .args(["-c", "xsettings", "-p", "/Gtk/CursorThemeName"])
            .output()
        {
            let theme = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !theme.is_empty() {
                return Some(theme);
            }
        }

        Some("default".into())
    }

    /// Create the Ganesha cursor XPM file
    fn create_ganesha_cursor_xpm(&self) -> Result<PathBuf, String> {
        // Ganesha Om symbol (ॐ) as a 32x32 cursor
        // This is a simplified pixel art representation
        let xpm_content = r##"/* XPM */
static char *ganesha_cursor[] = {
/* width height ncolors chars_per_pixel */
"32 32 4 1",
/* colors */
"  c None",
"X c #FFD700",
"O c #FF8C00",
"o c #000000",
/* pixels */
"                                ",
"          XXXX                  ",
"        XXXXXXXX                ",
"       XXXXXXXXXX               ",
"      XXXXooooXXXX              ",
"     XXXXooXXooXXXX             ",
"     XXXooXXXXooXXX             ",
"     XXXoXXXXXXoXXX             ",
"     XXXoXXXXXXoXXX    XXX      ",
"     XXXooXXXXooXXX   XXXXX     ",
"      XXXooooooXXX   XXXXXXX    ",
"       XXXooooXXX   XXXOOOXXX   ",
"        XXXXXXXX   XXXOOOOOXX   ",
"         XXXXXX   XXXOOOOOOOX   ",
"          XXXX   XXXOOOOOOOOOX  ",
"           XX   XXXOOOOOOOOOOX  ",
"               XXXOOOOOXXXOOOOX ",
"              XXXOOOOXXXXXXXOOO ",
"             XXXOOOOXXXXXXXXXXXO",
"            XXXOOOOXXXXXXXXXXXXX",
"           XXXOOOOXXXXXXXXXXXXXX",
"          XXXOOOOXXXXXXXXXXXXXXX",
"         XXXOOOOXXXXXXXXXXXXXXXX",
"        XXXOOOOXXXXXXXXXXXXXXXXX",
"       XXXOOOOXXXXXXXXXXXXXXXXXX",
"      XXXOOOOXXXXXXXXXXXXXXXXXXX",
"     XXXOOOOXXXXXXXXXXXXXXXXXXXX",
"    XXXOOOOXXXXXXXXXXXXXXXXXXXXX",
"   XXXOOOOXXXXXXXXXXXXXXXXXXXXXX",
"  XXXOOOXXXXXXXXXXXXXXXXXXXXXXXX",
" XXXOOXXXXXXXXXXXXXXXXXXXXXXXXXX",
"XXXOXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
};
"##;

        let xpm_path = self.cursor_dir.join("ganesha.xpm");
        std::fs::write(&xpm_path, xpm_content)
            .map_err(|e| format!("Failed to write cursor XPM: {}", e))?;

        Ok(xpm_path)
    }

    /// Create the Ganesha cursor as PNG (better quality)
    fn create_ganesha_cursor_png(&self) -> Result<PathBuf, String> {
        let png_path = self.cursor_dir.join("ganesha.png");

        // Use ImageMagick to create a high-quality cursor from text
        let result = Command::new("convert")
            .args([
                "-size", "48x48",
                "-background", "transparent",
                "-fill", "#FFD700",      // Gold
                "-stroke", "#FF8C00",    // Dark gold outline
                "-strokewidth", "1",
                "-font", "Noto-Sans-Symbols2",
                "-pointsize", "36",
                "-gravity", "center",
                "label:ॐ",               // Om symbol
                png_path.to_str().unwrap(),
            ])
            .output();

        if result.is_err() || !png_path.exists() {
            // Fallback: create a simple colored circle cursor
            Command::new("convert")
                .args([
                    "-size", "32x32",
                    "xc:transparent",
                    "-fill", "#FFD700",
                    "-stroke", "#FF8C00",
                    "-strokewidth", "2",
                    "-draw", "circle 16,16 16,4",
                    "-fill", "#FF8C00",
                    "-draw", "circle 16,16 16,10",
                    png_path.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| format!("Failed to create cursor image: {}", e))?;
        }

        Ok(png_path)
    }

    /// Create a proper xcursor file
    fn create_xcursor(&self) -> Result<PathBuf, String> {
        let png_path = self.create_ganesha_cursor_png()?;
        let cursor_path = self.cursor_dir.join("ganesha_cursor");

        // Create xcursor config file
        let config_path = self.cursor_dir.join("cursor.cfg");
        let config_content = format!("32 0 0 {}\n", png_path.display());
        std::fs::write(&config_path, config_content)
            .map_err(|e| format!("Failed to write cursor config: {}", e))?;

        // Use xcursorgen to create the cursor
        let result = Command::new("xcursorgen")
            .args([
                config_path.to_str().unwrap(),
                cursor_path.to_str().unwrap(),
            ])
            .output();

        if let Err(e) = result {
            // xcursorgen not available, use fallback
            return Err(format!("xcursorgen not available: {}", e));
        }

        Ok(cursor_path)
    }

    /// Set the Ganesha cursor as the active cursor
    pub fn set_ganesha_cursor(&self) -> Result<(), String> {
        // Method 1: Try using xsetroot with cursor file
        if let Ok(xpm_path) = self.create_ganesha_cursor_xpm() {
            let mask_path = xpm_path.clone(); // Use same as mask for now

            let result = Command::new("xsetroot")
                .args([
                    "-cursor",
                    xpm_path.to_str().unwrap(),
                    mask_path.to_str().unwrap(),
                ])
                .output();

            if result.is_ok() {
                return Ok(());
            }
        }

        // Method 2: Try creating and applying xcursor theme
        if let Ok(_cursor_path) = self.create_xcursor() {
            // Create a mini cursor theme
            let theme_dir = self.cursor_dir.join("GaneshaAI").join("cursors");
            std::fs::create_dir_all(&theme_dir).ok();

            // Copy cursor file for all cursor types
            let cursor_types = ["left_ptr", "arrow", "default", "pointer"];
            for ctype in cursor_types {
                let src = self.cursor_dir.join("ganesha_cursor");
                let dst = theme_dir.join(ctype);
                std::fs::copy(&src, &dst).ok();
            }

            // Create theme index
            let index_content = r#"[Icon Theme]
Name=GaneshaAI
Comment=AI Control Cursor
Inherits=default
"#;
            std::fs::write(theme_dir.parent().unwrap().join("index.theme"), index_content).ok();

            // Apply the theme
            self.apply_theme("GaneshaAI")?;
            return Ok(());
        }

        // Method 3: Fallback to overlay approach
        Err("Could not set system cursor, use overlay mode instead".into())
    }

    /// Apply a cursor theme
    fn apply_theme(&self, theme_name: &str) -> Result<(), String> {
        // Set for GNOME
        Command::new("gsettings")
            .args([
                "set", "org.gnome.desktop.interface",
                "cursor-theme", theme_name,
            ])
            .output()
            .ok();

        // Set via xrdb
        let xresources = format!("Xcursor.theme: {}\n", theme_name);
        let mut child = Command::new("xrdb")
            .args(["-merge"])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to run xrdb: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(xresources.as_bytes()).ok();
        }
        child.wait().ok();

        Ok(())
    }

    /// Restore the default cursor
    pub fn restore_default(&self) -> Result<(), String> {
        if let Some(ref theme) = self.original_theme {
            self.apply_theme(theme)?;
        } else {
            self.apply_theme("default")?;
        }

        // Also try xsetroot to reset root cursor
        Command::new("xsetroot")
            .args(["-cursor_name", "left_ptr"])
            .output()
            .ok();

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SMOOTH MOUSE MOVEMENT - Tracer-like animation
// ═══════════════════════════════════════════════════════════════════════════════

/// Animated mouse movement with easing - like tracer rounds
pub struct TracerMouse {
    /// Duration for the movement animation
    duration_ms: u64,
    /// Steps for smooth animation (higher = smoother but slower)
    steps: u32,
    /// Easing function type
    easing: EasingType,
}

/// Easing function type for mouse movement
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingType {
    /// Linear movement (constant speed)
    Linear,
    /// Ease out (fast start, slow end) - most natural
    EaseOut,
    /// Ease in (slow start, fast end)
    EaseIn,
    /// Ease in-out (slow start and end, fast middle)
    EaseInOut,
    /// Exponential ease out (very fast start, gradual slow)
    ExpoOut,
}

impl Default for TracerMouse {
    fn default() -> Self {
        Self::new()
    }
}

impl TracerMouse {
    pub fn new() -> Self {
        Self {
            duration_ms: 150, // Swift but visible
            steps: 20,        // Smooth but fast
            easing: EasingType::EaseOut, // Fast start, slow approach
        }
    }

    /// Set animation duration in milliseconds
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Set number of animation steps
    pub fn with_steps(mut self, steps: u32) -> Self {
        self.steps = steps;
        self
    }

    /// Set easing type
    pub fn with_easing(mut self, easing: EasingType) -> Self {
        self.easing = easing;
        self
    }

    /// Apply easing function to progress (0.0 to 1.0)
    fn ease(&self, t: f64) -> f64 {
        match self.easing {
            EasingType::Linear => t,
            EasingType::EaseOut => 1.0 - (1.0 - t).powi(3),  // Cubic ease out
            EasingType::EaseIn => t.powi(3),                  // Cubic ease in
            EasingType::EaseInOut => {
                if t < 0.5 {
                    4.0 * t.powi(3)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            EasingType::ExpoOut => {
                if t >= 1.0 { 1.0 } else { 1.0 - 2.0_f64.powf(-10.0 * t) }
            }
        }
    }

    /// Get current mouse position
    pub fn get_position() -> Result<(i32, i32), String> {
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .map_err(|e| format!("Failed to get mouse position: {}", e))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut x = 0i32;
        let mut y = 0i32;

        for line in output_str.lines() {
            if line.starts_with("X=") {
                x = line[2..].parse().unwrap_or(0);
            } else if line.starts_with("Y=") {
                y = line[2..].parse().unwrap_or(0);
            }
        }

        Ok((x, y))
    }

    /// Move mouse to position instantly
    fn move_instant(x: i32, y: i32) -> Result<(), String> {
        Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .output()
            .map_err(|e| format!("Failed to move mouse: {}", e))?;
        Ok(())
    }

    /// Move mouse from current position to target with smooth animation
    pub fn move_to(&self, target_x: i32, target_y: i32) -> Result<(), String> {
        let (start_x, start_y) = Self::get_position()?;
        self.move_from_to(start_x, start_y, target_x, target_y)
    }

    /// Move mouse from point A to point B with smooth animation
    pub fn move_from_to(&self, start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<(), String> {
        let step_delay = Duration::from_micros((self.duration_ms * 1000) / self.steps as u64);

        for i in 1..=self.steps {
            let t = i as f64 / self.steps as f64;
            let eased_t = self.ease(t);

            let current_x = start_x + ((end_x - start_x) as f64 * eased_t) as i32;
            let current_y = start_y + ((end_y - start_y) as f64 * eased_t) as i32;

            Self::move_instant(current_x, current_y)?;
            std::thread::sleep(step_delay);
        }

        // Ensure we end exactly at target
        Self::move_instant(end_x, end_y)?;
        Ok(())
    }

    /// Move mouse with visible "tracer" effect (shows trail)
    pub fn move_with_tracer(&self, target_x: i32, target_y: i32, cursor: &AiCursor) -> Result<(), String> {
        let (start_x, start_y) = Self::get_position()?;
        let step_delay = Duration::from_micros((self.duration_ms * 1000) / self.steps as u64);

        for i in 1..=self.steps {
            let t = i as f64 / self.steps as f64;
            let eased_t = self.ease(t);

            let current_x = start_x + ((target_x - start_x) as f64 * eased_t) as i32;
            let current_y = start_y + ((target_y - start_y) as f64 * eased_t) as i32;

            Self::move_instant(current_x, current_y)?;

            // Update cursor overlay position
            cursor.on_mouse_action(current_x, current_y).ok();

            std::thread::sleep(step_delay);
        }

        // Ensure we end exactly at target
        Self::move_instant(target_x, target_y)?;
        cursor.on_mouse_action(target_x, target_y).ok();

        Ok(())
    }

    /// Click at position with smooth movement
    pub fn click_at(&self, x: i32, y: i32) -> Result<(), String> {
        self.move_to(x, y)?;

        // Small pause before click (more natural)
        std::thread::sleep(Duration::from_millis(50));

        Command::new("xdotool")
            .args(["click", "1"]) // Left click
            .output()
            .map_err(|e| format!("Failed to click: {}", e))?;

        Ok(())
    }

    /// Double-click at position with smooth movement
    pub fn double_click_at(&self, x: i32, y: i32) -> Result<(), String> {
        self.move_to(x, y)?;

        std::thread::sleep(Duration::from_millis(50));

        Command::new("xdotool")
            .args(["click", "--repeat", "2", "--delay", "100", "1"])
            .output()
            .map_err(|e| format!("Failed to double-click: {}", e))?;

        Ok(())
    }

    /// Right-click at position with smooth movement
    pub fn right_click_at(&self, x: i32, y: i32) -> Result<(), String> {
        self.move_to(x, y)?;

        std::thread::sleep(Duration::from_millis(50));

        Command::new("xdotool")
            .args(["click", "3"]) // Right click
            .output()
            .map_err(|e| format!("Failed to right-click: {}", e))?;

        Ok(())
    }

    /// Drag from point A to point B with smooth movement
    pub fn drag(&self, start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<(), String> {
        // Move to start
        self.move_to(start_x, start_y)?;

        // Mouse down
        Command::new("xdotool")
            .args(["mousedown", "1"])
            .output()
            .map_err(|e| format!("Failed mousedown: {}", e))?;

        // Smooth movement to end
        std::thread::sleep(Duration::from_millis(50));
        self.move_from_to(start_x, start_y, end_x, end_y)?;

        // Mouse up
        std::thread::sleep(Duration::from_millis(50));
        Command::new("xdotool")
            .args(["mouseup", "1"])
            .output()
            .map_err(|e| format!("Failed mouseup: {}", e))?;

        Ok(())
    }

    /// Scroll at current position
    pub fn scroll(&self, direction: ScrollDirection, amount: u32) -> Result<(), String> {
        let button = match direction {
            ScrollDirection::Up => "4",
            ScrollDirection::Down => "5",
            ScrollDirection::Left => "6",
            ScrollDirection::Right => "7",
        };

        for _ in 0..amount {
            Command::new("xdotool")
                .args(["click", button])
                .output()
                .map_err(|e| format!("Failed to scroll: {}", e))?;

            std::thread::sleep(Duration::from_millis(30));
        }

        Ok(())
    }
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Convenience function for quick smooth move
pub fn smooth_move(x: i32, y: i32) -> Result<(), String> {
    TracerMouse::new().move_to(x, y)
}

/// Convenience function for quick smooth click
pub fn smooth_click(x: i32, y: i32) -> Result<(), String> {
    TracerMouse::new().click_at(x, y)
}

// ═══════════════════════════════════════════════════════════════════════════════
// SPEED CONTROL - From step-by-step auditing to maximum speed
// ═══════════════════════════════════════════════════════════════════════════════

/// Speed mode for AI actions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedMode {
    /// Step-by-step - wait for user confirmation between each action
    /// "I want to see EVERYTHING the AI does"
    StepByStep,
    /// Audit - very slow, maximum visibility
    /// "Training mode - I'm learning what the AI can do"
    Audit,
    /// Slow - AI intentionally goes slower than capable so you can follow along
    /// "Movie mode - watching the AI work is entertaining"
    Slow,
    /// Normal - balanced speed, human-like
    /// "Default - looks natural, like a competent human"
    Normal,
    /// Fast - quick but visible
    /// "I trust the AI, just get it done efficiently"
    Fast,
    /// PowerUser - AI goes as fast as the IT guy at work after coffee
    /// "Experienced user - I know what I'm doing, speed it up"
    PowerUser,
    /// Beast - AI outperforms the whole IT department
    /// "Maximum performance - unleash the full power"
    Beast,
}

impl SpeedMode {
    /// Get the animation duration in milliseconds
    pub fn animation_ms(&self) -> u64 {
        match self {
            SpeedMode::StepByStep => 1000,
            SpeedMode::Audit => 500,
            SpeedMode::Slow => 300,
            SpeedMode::Normal => 150,
            SpeedMode::Fast => 50,
            SpeedMode::PowerUser => 20,
            SpeedMode::Beast => 0,
        }
    }

    /// Get the delay between actions in milliseconds
    pub fn action_delay_ms(&self) -> u64 {
        match self {
            SpeedMode::StepByStep => 0, // Special: wait for confirmation
            SpeedMode::Audit => 2000,
            SpeedMode::Slow => 500,
            SpeedMode::Normal => 100,
            SpeedMode::Fast => 30,
            SpeedMode::PowerUser => 10,
            SpeedMode::Beast => 0,
        }
    }

    /// Get animation steps (higher = smoother)
    pub fn steps(&self) -> u32 {
        match self {
            SpeedMode::StepByStep => 30,
            SpeedMode::Audit => 30,
            SpeedMode::Slow => 25,
            SpeedMode::Normal => 20,
            SpeedMode::Fast => 10,
            SpeedMode::PowerUser => 5,
            SpeedMode::Beast => 1,
        }
    }

    /// Does this mode require user confirmation?
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, SpeedMode::StepByStep)
    }

    /// Create TracerMouse with this speed mode
    pub fn create_tracer(&self) -> TracerMouse {
        TracerMouse::new()
            .with_duration(self.animation_ms())
            .with_steps(self.steps())
    }
}

/// Speed controller for AI actions
pub struct SpeedController {
    mode: SpeedMode,
    /// Callback for step-by-step confirmation
    confirmation_callback: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    /// Current action description
    current_action: String,
}

impl Default for SpeedController {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedController {
    pub fn new() -> Self {
        Self {
            mode: SpeedMode::Normal,
            confirmation_callback: None,
            current_action: String::new(),
        }
    }

    /// Set speed mode
    pub fn set_mode(&mut self, mode: SpeedMode) {
        self.mode = mode;
    }

    /// Get current mode
    pub fn mode(&self) -> SpeedMode {
        self.mode
    }

    /// Set confirmation callback for step-by-step mode
    pub fn set_confirmation_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.confirmation_callback = Some(Box::new(callback));
    }

    /// Get tracer mouse for current speed
    pub fn tracer(&self) -> TracerMouse {
        self.mode.create_tracer()
    }

    /// Wait for action delay (between actions)
    pub fn wait_action_delay(&self) {
        let delay = self.mode.action_delay_ms();
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
    }

    /// Request confirmation before action (for step-by-step mode)
    pub fn confirm_action(&self, description: &str) -> bool {
        if !self.mode.requires_confirmation() {
            return true; // Auto-confirm for other modes
        }

        // If we have a callback, use it
        if let Some(ref callback) = self.confirmation_callback {
            return callback(description);
        }

        // Default: print to terminal and wait for input
        eprintln!("\n\x1b[33m🕉️ Next action: {}\x1b[0m", description);
        eprintln!("Press Enter to continue, 'n' to skip, 'q' to quit...");

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim().to_lowercase();
            if input == "q" || input == "quit" {
                return false;
            }
            if input == "n" || input == "skip" {
                return false;
            }
        }
        true
    }

    /// Execute action with speed control
    pub fn execute<F, R>(&self, description: &str, action: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        // Confirm if in step-by-step mode
        if !self.confirm_action(description) {
            return None;
        }

        // Execute the action
        let result = action();

        // Wait for action delay
        self.wait_action_delay();

        Some(result)
    }

    /// Move mouse with speed control
    pub fn move_mouse(&self, x: i32, y: i32) -> Result<(), String> {
        if !self.confirm_action(&format!("Move mouse to ({}, {})", x, y)) {
            return Ok(()); // Skipped
        }

        let tracer = self.tracer();
        tracer.move_to(x, y)?;

        self.wait_action_delay();
        Ok(())
    }

    /// Click with speed control
    pub fn click(&self, x: i32, y: i32) -> Result<(), String> {
        if !self.confirm_action(&format!("Click at ({}, {})", x, y)) {
            return Ok(());
        }

        let tracer = self.tracer();
        tracer.click_at(x, y)?;

        self.wait_action_delay();
        Ok(())
    }

    /// Type text with speed control
    pub fn type_text(&self, text: &str) -> Result<(), String> {
        if !self.confirm_action(&format!("Type: '{}'", text)) {
            return Ok(());
        }

        let delay = match self.mode {
            SpeedMode::StepByStep | SpeedMode::Audit => 100,
            SpeedMode::Slow => 50,
            SpeedMode::Normal => 20,
            SpeedMode::Fast => 10,
            SpeedMode::PowerUser => 5,
            SpeedMode::Beast => 0,
        };

        Command::new("xdotool")
            .args(["type", "--delay", &delay.to_string(), text])
            .output()
            .map_err(|e| format!("Failed to type: {}", e))?;

        self.wait_action_delay();
        Ok(())
    }

    /// Press key with speed control
    pub fn press_key(&self, key: &str) -> Result<(), String> {
        if !self.confirm_action(&format!("Press key: {}", key)) {
            return Ok(());
        }

        Command::new("xdotool")
            .args(["key", key])
            .output()
            .map_err(|e| format!("Failed to press key: {}", e))?;

        self.wait_action_delay();
        Ok(())
    }
}

/// Preset speed configurations
impl SpeedMode {
    /// Speed for interactive demonstration - watching AI work is entertaining
    pub fn demo() -> Self {
        SpeedMode::Slow
    }

    /// Speed for automated testing
    pub fn testing() -> Self {
        SpeedMode::Fast
    }

    /// Speed for production batch processing - IT guy after coffee
    pub fn batch() -> Self {
        SpeedMode::PowerUser
    }

    /// Speed for debugging - see everything
    pub fn debug() -> Self {
        SpeedMode::Audit
    }

    /// Unleash the beast - outperform the whole IT department
    pub fn unleash() -> Self {
        SpeedMode::Beast
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            SpeedMode::StepByStep => "Step-by-step: I want to see EVERYTHING the AI does",
            SpeedMode::Audit => "Training mode: I'm learning what the AI can do",
            SpeedMode::Slow => "Movie mode: Watching the AI work is entertaining",
            SpeedMode::Normal => "Default: Looks natural, like a competent human",
            SpeedMode::Fast => "Efficient: I trust the AI, just get it done",
            SpeedMode::PowerUser => "IT guy after coffee: Speed it up!",
            SpeedMode::Beast => "Beast mode: Outperforms the whole IT department",
        }
    }
}

impl Drop for AiCursor {
    fn drop(&mut self) {
        self.hide();
    }
}

/// Simple cursor indicator using terminal colors
/// Falls back to this if yad is not available
pub struct TerminalCursor {
    last_x: i32,
    last_y: i32,
}

impl TerminalCursor {
    pub fn new() -> Self {
        Self { last_x: 0, last_y: 0 }
    }

    /// Print cursor position indicator
    pub fn indicate(&mut self, x: i32, y: i32) {
        if x != self.last_x || y != self.last_y {
            eprintln!("\x1b[33m🕉️ AI Cursor: ({}, {})\x1b[0m", x, y);
            self.last_x = x;
            self.last_y = y;
        }
    }
}

impl Default for TerminalCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Floating cursor overlay that follows the mouse
/// Uses a GTK window for smooth tracking
pub struct FloatingCursor {
    process: Option<Child>,
    symbol: String,
    color: String,
}

impl FloatingCursor {
    pub fn new() -> Self {
        Self {
            process: None,
            symbol: "🕉️".into(),
            color: "#FFD700".into(), // Gold
        }
    }

    pub fn with_symbol(mut self, symbol: &str) -> Self {
        self.symbol = symbol.to_string();
        self
    }

    pub fn with_color(mut self, color: &str) -> Self {
        self.color = color.to_string();
        self
    }

    /// Start the floating cursor (tracks mouse automatically)
    pub fn start(&mut self) -> Result<(), String> {
        // Use a Python/GTK script for smooth cursor tracking
        let script = format!(
            r#"
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk, GLib
import subprocess

class CursorWindow(Gtk.Window):
    def __init__(self):
        super().__init__()
        self.set_decorated(False)
        self.set_skip_taskbar_hint(True)
        self.set_skip_pager_hint(True)
        self.set_keep_above(True)
        self.set_accept_focus(False)
        self.set_can_focus(False)
        self.set_opacity(0.9)
        self.set_default_size(60, 60)

        # Make window click-through
        self.set_app_paintable(True)
        screen = self.get_screen()
        visual = screen.get_rgba_visual()
        if visual:
            self.set_visual(visual)

        label = Gtk.Label()
        label.set_markup('<span font="32" foreground="{}">{}</span>')
        self.add(label)

        GLib.timeout_add(50, self.update_position)
        self.show_all()

    def update_position(self):
        try:
            result = subprocess.run(['xdotool', 'getmouselocation', '--shell'],
                                    capture_output=True, text=True)
            for line in result.stdout.split('\n'):
                if line.startswith('X='):
                    x = int(line[2:])
                elif line.startswith('Y='):
                    y = int(line[2:])
            self.move(x + 25, y + 25)
        except:
            pass
        return True

win = CursorWindow()
win.connect('destroy', Gtk.main_quit)
Gtk.main()
"#,
            self.color, self.symbol
        );

        // Save script to temp file and run it
        let temp_path = "/tmp/ganesha_cursor.py";
        std::fs::write(temp_path, script)
            .map_err(|e| format!("Failed to write cursor script: {}", e))?;

        let child = Command::new("python3")
            .arg(temp_path)
            .spawn()
            .map_err(|e| format!("Failed to start floating cursor: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    /// Stop the floating cursor
    pub fn stop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
        }
        self.process = None;
    }
}

impl Default for FloatingCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FloatingCursor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_creation() {
        let cursor = AiCursor::new()
            .with_style(CursorStyle::Ganesha)
            .with_linger(Duration::from_secs(2));

        assert_eq!(cursor.get_symbol(), "🕉️");
    }

    #[test]
    fn test_cursor_styles() {
        let cursor = AiCursor::new();

        let ganesha = AiCursor::new().with_style(CursorStyle::Ganesha);
        assert_eq!(ganesha.get_symbol(), "🕉️");

        let trunk = AiCursor::new().with_style(CursorStyle::Trunk);
        assert_eq!(trunk.get_symbol(), "🐘");

        let eye = AiCursor::new().with_style(CursorStyle::Eye);
        assert_eq!(eye.get_symbol(), "👁️");
    }

    #[test]
    fn test_custom_symbol() {
        let cursor = AiCursor::new()
            .with_custom_symbol("🔮");

        assert_eq!(cursor.get_symbol(), "🔮");
    }
}
