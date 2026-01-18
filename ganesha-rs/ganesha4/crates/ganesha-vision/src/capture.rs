//! Screen capture functionality for the Vision/VLA system.
//!
//! This module provides:
//! - Platform-abstracted screen capture via the `ScreenCapture` trait
//! - Full screen capture with multi-monitor support
//! - Window-specific capture
//! - Region-based capture by coordinates
//! - Image format conversion and encoding

use crate::config::{CaptureSettings, ImageFormat};
use async_trait::async_trait;
use image::{DynamicImage, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use thiserror::Error;

/// Errors that can occur during screen capture.
#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Screen capture not available on this platform")]
    NotAvailable,

    #[error("Failed to capture screen: {0}")]
    CaptureFailed(String),

    #[error("Window not found: {0}")]
    WindowNotFound(String),

    #[error("Monitor not found: {0}")]
    MonitorNotFound(u32),

    #[error("Invalid region: {0}")]
    InvalidRegion(String),

    #[error("Image encoding failed: {0}")]
    EncodingFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

/// Result type for capture operations.
pub type CaptureResult<T> = Result<T, CaptureError>;

/// A rectangular region on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// X coordinate of the top-left corner
    pub x: i32,
    /// Y coordinate of the top-left corner
    pub y: i32,
    /// Width of the region
    pub width: u32,
    /// Height of the region
    pub height: u32,
}

impl Region {
    /// Create a new region.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if this region is valid (has positive dimensions).
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Check if a point is within this region.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// Get the center point of this region.
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width / 2) as i32,
            self.y + (self.height / 2) as i32,
        )
    }
}

/// Information about a display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Monitor index (0-based)
    pub index: u32,
    /// Monitor name/identifier
    pub name: String,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Monitor region (position and size)
    pub region: Region,
    /// Scale factor (for HiDPI displays)
    pub scale_factor: f64,
}

/// Information about a window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Window ID (platform-specific)
    pub id: u64,
    /// Window title
    pub title: String,
    /// Process name
    pub process_name: String,
    /// Process ID
    pub pid: u32,
    /// Window region (position and size)
    pub region: Region,
    /// Whether the window is minimized
    pub is_minimized: bool,
    /// Whether the window is visible
    pub is_visible: bool,
}

/// A captured screenshot.
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// The captured image
    pub image: DynamicImage,
    /// Region that was captured
    pub region: Region,
    /// Timestamp of capture (Unix milliseconds)
    pub timestamp: i64,
    /// Source of the capture (monitor name, window title, etc.)
    pub source: String,
}

impl Screenshot {
    /// Create a new screenshot from an image.
    pub fn new(image: DynamicImage, region: Region, source: impl Into<String>) -> Self {
        Self {
            image,
            region,
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: source.into(),
        }
    }

    /// Get the width of the screenshot.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Get the height of the screenshot.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Encode the screenshot to bytes in the specified format.
    pub fn encode(&self, settings: &CaptureSettings) -> CaptureResult<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());

        // Scale down if needed
        let image = if self.image.width() > settings.max_dimension
            || self.image.height() > settings.max_dimension
        {
            let scale = settings.max_dimension as f64
                / self.image.width().max(self.image.height()) as f64;
            let new_width = (self.image.width() as f64 * scale) as u32;
            let new_height = (self.image.height() as f64 * scale) as u32;
            self.image
                .resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
        } else {
            self.image.clone()
        };

        match settings.format {
            ImageFormat::Png => {
                image
                    .write_to(&mut buffer, image::ImageFormat::Png)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
            ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut buffer,
                    settings.jpeg_quality,
                );
                image
                    .write_with_encoder(encoder)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
            ImageFormat::WebP => {
                // WebP support via image crate
                image
                    .write_to(&mut buffer, image::ImageFormat::WebP)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
        }

        Ok(buffer.into_inner())
    }

    /// Encode to base64 for API transmission.
    pub fn to_base64(&self, settings: &CaptureSettings) -> CaptureResult<String> {
        let bytes = self.encode(settings)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &bytes,
        ))
    }

    /// Crop the screenshot to a specific region.
    pub fn crop(&self, region: Region) -> CaptureResult<Screenshot> {
        // Validate region is within bounds
        if region.x < 0 || region.y < 0 {
            return Err(CaptureError::InvalidRegion(
                "Region coordinates must be non-negative".to_string(),
            ));
        }

        let x = region.x as u32;
        let y = region.y as u32;

        if x + region.width > self.image.width() || y + region.height > self.image.height() {
            return Err(CaptureError::InvalidRegion(
                "Region extends beyond image bounds".to_string(),
            ));
        }

        let cropped = self.image.crop_imm(x, y, region.width, region.height);

        Ok(Screenshot::new(
            cropped,
            region,
            format!("{} (cropped)", self.source),
        ))
    }
}

/// Trait for platform-specific screen capture implementations.
#[async_trait]
pub trait ScreenCapture: Send + Sync {
    /// Check if screen capture is available on this platform.
    fn is_available(&self) -> bool;

    /// Get a list of all monitors.
    async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>>;

    /// Get the primary monitor.
    async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo>;

    /// Capture the entire screen (all monitors).
    async fn capture_all(&self) -> CaptureResult<Screenshot>;

    /// Capture a specific monitor.
    async fn capture_monitor(&self, monitor_index: u32) -> CaptureResult<Screenshot>;

    /// Capture a specific region.
    async fn capture_region(&self, region: Region) -> CaptureResult<Screenshot>;

    /// Get a list of all visible windows.
    async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>>;

    /// Find a window by title (partial match).
    async fn find_window_by_title(&self, title: &str) -> CaptureResult<Option<WindowInfo>>;

    /// Find windows by process name.
    async fn find_windows_by_process(&self, process_name: &str) -> CaptureResult<Vec<WindowInfo>>;

    /// Capture a specific window.
    async fn capture_window(&self, window_id: u64) -> CaptureResult<Screenshot>;

    /// Capture a window by title.
    async fn capture_window_by_title(&self, title: &str) -> CaptureResult<Screenshot> {
        let window = self
            .find_window_by_title(title)
            .await?
            .ok_or_else(|| CaptureError::WindowNotFound(title.to_string()))?;
        self.capture_window(window.id).await
    }
}

/// Platform-specific screen capture implementation using xcap.
#[cfg(feature = "gui-automation")]
pub mod platform {
    use super::*;

    /// Cross-platform screen capture implementation using xcap.
    pub struct XcapCapture {
        settings: CaptureSettings,
    }

    impl XcapCapture {
        /// Create a new xcap-based capture implementation.
        pub fn new(settings: CaptureSettings) -> Self {
            Self { settings }
        }

        /// Convert xcap image to DynamicImage.
        fn convert_image(
            data: Vec<u8>,
            width: u32,
            height: u32,
        ) -> CaptureResult<DynamicImage> {
            // xcap returns BGRA format
            let mut rgba_data = data;
            for chunk in rgba_data.chunks_exact_mut(4) {
                chunk.swap(0, 2); // Swap B and R
            }

            let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_raw(width, height, rgba_data).ok_or_else(|| {
                    CaptureError::CaptureFailed("Failed to create image buffer".to_string())
                })?;

            Ok(DynamicImage::ImageRgba8(buffer))
        }
    }

    #[async_trait]
    impl ScreenCapture for XcapCapture {
        fn is_available(&self) -> bool {
            // xcap should be available on supported platforms
            true
        }

        async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>> {
            let monitors = xcap::Monitor::all()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            Ok(monitors
                .into_iter()
                .enumerate()
                .map(|(i, m)| MonitorInfo {
                    index: i as u32,
                    name: m.name().to_string(),
                    is_primary: m.is_primary(),
                    region: Region::new(m.x(), m.y(), m.width(), m.height()),
                    scale_factor: m.scale_factor() as f64,
                })
                .collect())
        }

        async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo> {
            let monitors = self.get_monitors().await?;
            monitors
                .into_iter()
                .find(|m| m.is_primary)
                .or_else(|| None)
                .ok_or_else(|| CaptureError::MonitorNotFound(0))
        }

        async fn capture_all(&self) -> CaptureResult<Screenshot> {
            // Capture primary monitor for now
            // TODO: Implement stitching multiple monitors
            let monitor = xcap::Monitor::all()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?
                .into_iter()
                .find(|m| m.is_primary())
                .ok_or_else(|| CaptureError::MonitorNotFound(0))?;

            let capture = monitor
                .capture_image()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            let width = capture.width();
            let height = capture.height();
            let data = capture.into_raw();

            let image = Self::convert_image(data, width, height)?;
            let region = Region::new(monitor.x(), monitor.y(), width, height);

            Ok(Screenshot::new(image, region, monitor.name()))
        }

        async fn capture_monitor(&self, monitor_index: u32) -> CaptureResult<Screenshot> {
            let monitors = xcap::Monitor::all()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            let monitor = monitors
                .into_iter()
                .nth(monitor_index as usize)
                .ok_or(CaptureError::MonitorNotFound(monitor_index))?;

            let capture = monitor
                .capture_image()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            let width = capture.width();
            let height = capture.height();
            let data = capture.into_raw();

            let image = Self::convert_image(data, width, height)?;
            let region = Region::new(monitor.x(), monitor.y(), width, height);

            Ok(Screenshot::new(image, region, monitor.name()))
        }

        async fn capture_region(&self, region: Region) -> CaptureResult<Screenshot> {
            if !region.is_valid() {
                return Err(CaptureError::InvalidRegion(
                    "Region must have positive dimensions".to_string(),
                ));
            }

            // Capture full screen and crop
            let full = self.capture_all().await?;
            full.crop(region)
        }

        async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>> {
            let windows = xcap::Window::all()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            Ok(windows
                .into_iter()
                .filter(|w| !w.is_minimized())
                .map(|w| WindowInfo {
                    id: w.id() as u64,
                    title: w.title().to_string(),
                    process_name: w.app_name().to_string(),
                    pid: w.id(), // xcap Window doesn't have pid(), use id() instead
                    region: Region::new(w.x(), w.y(), w.width(), w.height()),
                    is_minimized: w.is_minimized(),
                    is_visible: !w.is_minimized(),
                })
                .collect())
        }

        async fn find_window_by_title(&self, title: &str) -> CaptureResult<Option<WindowInfo>> {
            let windows = self.get_windows().await?;
            let title_lower = title.to_lowercase();
            Ok(windows
                .into_iter()
                .find(|w| w.title.to_lowercase().contains(&title_lower)))
        }

        async fn find_windows_by_process(
            &self,
            process_name: &str,
        ) -> CaptureResult<Vec<WindowInfo>> {
            let windows = self.get_windows().await?;
            let name_lower = process_name.to_lowercase();
            Ok(windows
                .into_iter()
                .filter(|w| w.process_name.to_lowercase().contains(&name_lower))
                .collect())
        }

        async fn capture_window(&self, window_id: u64) -> CaptureResult<Screenshot> {
            let windows = xcap::Window::all()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            let window = windows
                .into_iter()
                .find(|w| w.id() as u64 == window_id)
                .ok_or_else(|| CaptureError::WindowNotFound(format!("ID: {}", window_id)))?;

            let capture = window
                .capture_image()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            let width = capture.width();
            let height = capture.height();
            let data = capture.into_raw();

            let image = Self::convert_image(data, width, height)?;
            let region = Region::new(window.x(), window.y(), width, height);

            Ok(Screenshot::new(image, region, window.title()))
        }
    }
}

/// Create the default screen capture implementation for the current platform.
#[cfg(feature = "gui-automation")]
pub fn create_screen_capture(settings: CaptureSettings) -> impl ScreenCapture {
    platform::XcapCapture::new(settings)
}

/// Mock screen capture for testing or when gui-automation feature is disabled.
#[cfg(not(feature = "gui-automation"))]
pub mod mock {
    use super::*;

    /// Mock screen capture that always fails.
    pub struct MockCapture;

    #[async_trait]
    impl ScreenCapture for MockCapture {
        fn is_available(&self) -> bool {
            false
        }

        async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>> {
            Err(CaptureError::NotAvailable)
        }

        async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo> {
            Err(CaptureError::NotAvailable)
        }

        async fn capture_all(&self) -> CaptureResult<Screenshot> {
            Err(CaptureError::NotAvailable)
        }

        async fn capture_monitor(&self, _monitor_index: u32) -> CaptureResult<Screenshot> {
            Err(CaptureError::NotAvailable)
        }

        async fn capture_region(&self, _region: Region) -> CaptureResult<Screenshot> {
            Err(CaptureError::NotAvailable)
        }

        async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>> {
            Err(CaptureError::NotAvailable)
        }

        async fn find_window_by_title(&self, _title: &str) -> CaptureResult<Option<WindowInfo>> {
            Err(CaptureError::NotAvailable)
        }

        async fn find_windows_by_process(
            &self,
            _process_name: &str,
        ) -> CaptureResult<Vec<WindowInfo>> {
            Err(CaptureError::NotAvailable)
        }

        async fn capture_window(&self, _window_id: u64) -> CaptureResult<Screenshot> {
            Err(CaptureError::NotAvailable)
        }
    }
}

#[cfg(not(feature = "gui-automation"))]
pub fn create_screen_capture(_settings: CaptureSettings) -> impl ScreenCapture {
    mock::MockCapture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_contains() {
        let region = Region::new(100, 100, 200, 200);
        assert!(region.contains(150, 150));
        assert!(region.contains(100, 100));
        assert!(!region.contains(50, 50));
        assert!(!region.contains(350, 150));
    }

    #[test]
    fn test_region_center() {
        let region = Region::new(100, 100, 200, 200);
        assert_eq!(region.center(), (200, 200));
    }

    #[test]
    fn test_region_valid() {
        assert!(Region::new(0, 0, 100, 100).is_valid());
        assert!(!Region::new(0, 0, 0, 100).is_valid());
        assert!(!Region::new(0, 0, 100, 0).is_valid());
    }
}
