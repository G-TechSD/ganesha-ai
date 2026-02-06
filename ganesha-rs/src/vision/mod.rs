//! Vision Module - Screenshot Capture and Screen Analysis
//!
//! SAFETY: This module is disabled by default and requires explicit opt-in
//! via the `vision` feature flag: `--features vision`
//!
//! This enables Ganesha to "see" the screen for GUI automation tasks.

#[cfg(feature = "vision")]
use base64_lib::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(feature = "vision")]
use xcap::Monitor;
#[cfg(feature = "vision")]
use std::io::Cursor;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Global kill switch for vision capabilities
static VISION_ENABLED: AtomicBool = AtomicBool::new(false);
static VISION_KILL_SWITCH: AtomicBool = AtomicBool::new(false);

/// Rate limiting: max screenshots per minute (increased for fast polling)
const MAX_SCREENSHOTS_PER_MINUTE: u64 = 300;  // 5 per second average
static SCREENSHOT_COUNT: AtomicU64 = AtomicU64::new(0);
static RATE_LIMIT_RESET: AtomicU64 = AtomicU64::new(0);

/// Screenshot result with metadata
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Base64-encoded PNG image data
    pub data: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Monitor/display index
    pub monitor: usize,
    /// Timestamp when captured
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Vision capability status
#[derive(Debug, Clone)]
pub struct VisionStatus {
    pub feature_compiled: bool,
    pub enabled: bool,
    pub kill_switch_active: bool,
    pub screenshots_this_minute: u64,
    pub rate_limit: u64,
}

/// Vision controller with safety mechanisms
pub struct VisionController {
    /// Whether vision was explicitly enabled by user
    enabled: Arc<AtomicBool>,
    /// Emergency kill switch
    kill_switch: Arc<AtomicBool>,
    /// Last activity timestamp for timeout
    last_activity: std::sync::Mutex<Instant>,
    /// Auto-disable timeout (default 5 minutes of inactivity)
    inactivity_timeout: Duration,
}

impl Default for VisionController {
    fn default() -> Self {
        Self::new()
    }
}

impl VisionController {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            kill_switch: Arc::new(AtomicBool::new(false)),
            last_activity: std::sync::Mutex::new(Instant::now()),
            inactivity_timeout: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Enable vision capabilities (requires user consent)
    ///
    /// # Safety
    /// This should only be called after explicit user confirmation
    pub fn enable(&self) -> Result<(), VisionError> {
        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(VisionError::KillSwitchActive);
        }

        #[cfg(not(feature = "vision"))]
        return Err(VisionError::FeatureNotCompiled);

        #[cfg(feature = "vision")]
        {
            VISION_ENABLED.store(true, Ordering::SeqCst);
            self.enabled.store(true, Ordering::SeqCst);
            *self.last_activity.lock().expect("Last activity lock poisoned - unable to update vision timestamp") = Instant::now();
            Ok(())
        }
    }

    /// Disable vision capabilities
    pub fn disable(&self) {
        VISION_ENABLED.store(false, Ordering::SeqCst);
        self.enabled.store(false, Ordering::SeqCst);
    }

    /// Activate emergency kill switch (cannot be reversed without restart)
    pub fn activate_kill_switch(&self) {
        VISION_KILL_SWITCH.store(true, Ordering::SeqCst);
        self.kill_switch.store(true, Ordering::SeqCst);
        self.disable();
    }

    /// Check if vision is currently available
    pub fn is_available(&self) -> bool {
        #[cfg(not(feature = "vision"))]
        return false;

        #[cfg(feature = "vision")]
        {
            self.enabled.load(Ordering::SeqCst)
                && !self.kill_switch.load(Ordering::SeqCst)
                && !self.is_inactive_timeout()
        }
    }

    /// Check if inactive timeout has expired
    fn is_inactive_timeout(&self) -> bool {
        let last = self.last_activity.lock().expect("Last activity lock poisoned - unable to check vision timeout");
        last.elapsed() > self.inactivity_timeout
    }

    /// Update activity timestamp
    fn touch(&self) {
        *self.last_activity.lock().expect("Last activity lock poisoned - unable to update vision activity timestamp") = Instant::now();
    }

    /// Check rate limit
    fn check_rate_limit(&self) -> Result<(), VisionError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let reset_time = RATE_LIMIT_RESET.load(Ordering::SeqCst);

        // Reset counter every minute
        if now > reset_time + 60 {
            SCREENSHOT_COUNT.store(0, Ordering::SeqCst);
            RATE_LIMIT_RESET.store(now, Ordering::SeqCst);
        }

        let count = SCREENSHOT_COUNT.fetch_add(1, Ordering::SeqCst);
        if count >= MAX_SCREENSHOTS_PER_MINUTE {
            return Err(VisionError::RateLimitExceeded);
        }

        Ok(())
    }

    /// Get current status
    pub fn status(&self) -> VisionStatus {
        VisionStatus {
            feature_compiled: cfg!(feature = "vision"),
            enabled: self.enabled.load(Ordering::SeqCst),
            kill_switch_active: self.kill_switch.load(Ordering::SeqCst),
            screenshots_this_minute: SCREENSHOT_COUNT.load(Ordering::SeqCst),
            rate_limit: MAX_SCREENSHOTS_PER_MINUTE,
        }
    }

    /// Capture screenshot of primary monitor (full resolution)
    #[cfg(feature = "vision")]
    pub fn capture_screen(&self) -> Result<Screenshot, VisionError> {
        self.capture_monitor(0)
    }

    /// Capture fast/low-res screenshot for quick polling (640x360)
    /// Resolution floor ensures UI elements remain visible
    /// Much faster for VLM analysis while maintaining situational awareness
    #[cfg(feature = "vision")]
    pub fn capture_screen_fast(&self) -> Result<Screenshot, VisionError> {
        // 640x360 is the floor - UI elements still visible, mouse cursor identifiable
        self.capture_screen_scaled(640, 360)
    }

    /// Minimum safe resolution where UI is still usable
    pub const MIN_SAFE_WIDTH: u32 = 640;
    pub const MIN_SAFE_HEIGHT: u32 = 360;

    /// Get primary screen dimensions
    #[cfg(feature = "vision")]
    pub fn get_screen_size(&self) -> Result<(u32, u32), VisionError> {
        let monitors = Monitor::all().map_err(|e| VisionError::CaptureError(e.to_string()))?;
        let monitor = monitors
            .first()
            .ok_or(VisionError::InvalidMonitor(0))?;
        Ok((monitor.width(), monitor.height()))
    }

    /// Get primary screen dimensions (stub when vision feature disabled)
    #[cfg(not(feature = "vision"))]
    pub fn get_screen_size(&self) -> Result<(u32, u32), VisionError> {
        Ok((1024, 768)) // Default fallback
    }

    /// Capture screenshot scaled to specific dimensions
    #[cfg(feature = "vision")]
    pub fn capture_screen_scaled(&self, target_width: u32, target_height: u32) -> Result<Screenshot, VisionError> {
        // Safety checks
        if !self.is_available() {
            return Err(VisionError::NotEnabled);
        }

        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(VisionError::KillSwitchActive);
        }

        self.check_rate_limit()?;
        self.touch();

        // Get primary monitor
        let monitors = Monitor::all().map_err(|e| VisionError::CaptureError(e.to_string()))?;
        let monitor = monitors
            .first()
            .ok_or(VisionError::InvalidMonitor(0))?;

        // Capture full screen
        let image = monitor
            .capture_image()
            .map_err(|e| VisionError::CaptureError(e.to_string()))?;

        // Resize using fast nearest-neighbor for speed
        let resized = xcap::image::imageops::resize(
            &image,
            target_width,
            target_height,
            xcap::image::imageops::FilterType::Nearest
        );

        // Convert RGBA to RGB (JPEG doesn't support alpha channel)
        let rgb_image: xcap::image::RgbImage = xcap::image::DynamicImage::ImageRgba8(resized).to_rgb8();

        // Convert to JPEG for smaller payloads (better for local vision models)
        let mut buffer = Cursor::new(Vec::new());
        rgb_image
            .write_to(&mut buffer, xcap::image::ImageFormat::Jpeg)
            .map_err(|e| VisionError::EncodingError(e.to_string()))?;

        let data = BASE64.encode(buffer.into_inner());

        Ok(Screenshot {
            data,
            width: target_width,
            height: target_height,
            monitor: 0,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Capture screenshot of specific monitor
    #[cfg(feature = "vision")]
    pub fn capture_monitor(&self, monitor_index: usize) -> Result<Screenshot, VisionError> {
        // Safety checks
        if !self.is_available() {
            return Err(VisionError::NotEnabled);
        }

        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(VisionError::KillSwitchActive);
        }

        self.check_rate_limit()?;
        self.touch();

        // Get monitors
        let monitors = Monitor::all().map_err(|e| VisionError::CaptureError(e.to_string()))?;

        let monitor = monitors
            .get(monitor_index)
            .ok_or(VisionError::InvalidMonitor(monitor_index))?;

        // Capture
        let image = monitor
            .capture_image()
            .map_err(|e| VisionError::CaptureError(e.to_string()))?;

        let width = image.width();
        let height = image.height();

        // Convert to PNG and base64 using xcap's image types
        let mut buffer = Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, xcap::image::ImageFormat::Png)
            .map_err(|e| VisionError::EncodingError(e.to_string()))?;

        let data = BASE64.encode(buffer.into_inner());

        Ok(Screenshot {
            data,
            width,
            height,
            monitor: monitor_index,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Capture specific region of screen
    #[cfg(feature = "vision")]
    pub fn capture_region(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<Screenshot, VisionError> {
        // Safety checks
        if !self.is_available() {
            return Err(VisionError::NotEnabled);
        }

        self.check_rate_limit()?;
        self.touch();

        // Get primary monitor
        let monitors = Monitor::all().map_err(|e| VisionError::CaptureError(e.to_string()))?;

        let monitor = monitors
            .first()
            .ok_or(VisionError::InvalidMonitor(0))?;

        // Capture full screen first
        let full_image = monitor
            .capture_image()
            .map_err(|e| VisionError::CaptureError(e.to_string()))?;

        // Crop to region using xcap's image types
        let cropped = xcap::image::imageops::crop_imm(&full_image, x, y, width, height).to_image();

        // Convert to PNG and base64
        let mut buffer = Cursor::new(Vec::new());
        cropped
            .write_to(&mut buffer, xcap::image::ImageFormat::Png)
            .map_err(|e| VisionError::EncodingError(e.to_string()))?;

        let data = BASE64.encode(buffer.into_inner());

        Ok(Screenshot {
            data,
            width,
            height,
            monitor: 0,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get list of available monitors
    #[cfg(feature = "vision")]
    pub fn list_monitors(&self) -> Result<Vec<MonitorInfo>, VisionError> {
        let monitors = Monitor::all().map_err(|e| VisionError::CaptureError(e.to_string()))?;

        Ok(monitors
            .iter()
            .enumerate()
            .map(|(i, m)| MonitorInfo {
                index: i,
                name: m.name().to_string(),
                width: m.width(),
                height: m.height(),
                is_primary: i == 0,
            })
            .collect())
    }

    /// Stub implementations when feature not compiled
    #[cfg(not(feature = "vision"))]
    pub fn capture_screen(&self) -> Result<Screenshot, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "vision"))]
    pub fn capture_screen_fast(&self) -> Result<Screenshot, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "vision"))]
    pub fn capture_screen_scaled(&self, _w: u32, _h: u32) -> Result<Screenshot, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "vision"))]
    pub fn capture_monitor(&self, _monitor_index: usize) -> Result<Screenshot, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "vision"))]
    pub fn capture_region(
        &self,
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
    ) -> Result<Screenshot, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "vision"))]
    pub fn list_monitors(&self) -> Result<Vec<MonitorInfo>, VisionError> {
        Err(VisionError::FeatureNotCompiled)
    }
}

/// Monitor information
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Vision errors
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("Vision feature not compiled. Rebuild with --features vision")]
    FeatureNotCompiled,

    #[error("Vision not enabled. Call enable() with user consent first")]
    NotEnabled,

    #[error("Emergency kill switch is active. Restart required")]
    KillSwitchActive,

    #[error("Rate limit exceeded ({MAX_SCREENSHOTS_PER_MINUTE}/minute)")]
    RateLimitExceeded,

    #[error("Invalid monitor index: {0}")]
    InvalidMonitor(usize),

    #[error("Screen capture failed: {0}")]
    CaptureError(String),

    #[error("Image encoding failed: {0}")]
    EncodingError(String),

    #[error("Inactivity timeout - vision auto-disabled")]
    InactivityTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_disabled_by_default() {
        let controller = VisionController::new();
        assert!(!controller.is_available());
    }

    #[test]
    fn test_kill_switch() {
        let controller = VisionController::new();
        controller.activate_kill_switch();
        assert!(controller.kill_switch.load(Ordering::SeqCst));
        assert!(controller.enable().is_err());
    }
}
