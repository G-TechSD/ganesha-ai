//! Screen capture module for Ganesha Vision system.
//!
//! This module provides cross-platform screen capture functionality using the xcap crate,
//! with support for:
//! - Full screen capture with multi-monitor support
//! - Region-based capture
//! - Window-specific capture
//! - Image compression with configurable quality (JPEG/PNG/WebP)
//! - Base64 encoding for sending to vision models
//! - Screenshot buffer/queue for rapid capture (target: 1fps sustained)
//!
//! # Example
//!
//! ```rust,no_run
//! use ganesha_learning::capture::{XcapCapture, ScreenCapture, CaptureConfig, ImageFormat};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = CaptureConfig::default();
//!     let capture = XcapCapture::new(config);
//!
//!     // Capture full screen
//!     let mut screenshot = capture.capture_screen(None).await.unwrap();
//!
//!     // Convert to base64 for vision model
//!     let base64 = screenshot.to_base64().unwrap();
//!     println!("Screenshot size: {} bytes", base64.len());
//! }
//! ```

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chrono::{DateTime, Utc};
use image::{DynamicImage, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during screen capture operations.
#[derive(Error, Debug)]
pub enum CaptureError {
    /// Screen capture is not available on this platform or feature is disabled.
    #[error("Screen capture not available: {0}")]
    NotAvailable(String),

    /// Failed to capture the screen.
    #[error("Failed to capture screen: {0}")]
    CaptureFailed(String),

    /// The specified window was not found.
    #[error("Window not found: {0}")]
    WindowNotFound(String),

    /// The specified monitor was not found.
    #[error("Monitor not found: index {0}")]
    MonitorNotFound(u32),

    /// The specified region is invalid.
    #[error("Invalid region: {0}")]
    InvalidRegion(String),

    /// Image encoding failed.
    #[error("Image encoding failed: {0}")]
    EncodingFailed(String),

    /// Permission denied for screen capture.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Buffer operation failed.
    #[error("Buffer error: {0}")]
    BufferError(String),

    /// Timeout waiting for operation.
    #[error("Operation timed out: {0}")]
    Timeout(String),
}

/// Result type for capture operations.
pub type CaptureResult<T> = Result<T, CaptureError>;

// ============================================================================
// Configuration Types
// ============================================================================

/// Image format for screen captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// PNG format (lossless, larger files).
    #[default]
    Png,
    /// JPEG format (lossy, smaller files).
    Jpeg,
    /// WebP format (modern, efficient compression).
    WebP,
}

impl ImageFormat {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }

    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }

    /// Get the data URI prefix for this format.
    pub fn data_uri_prefix(&self) -> &'static str {
        match self {
            Self::Png => "data:image/png;base64,",
            Self::Jpeg => "data:image/jpeg;base64,",
            Self::WebP => "data:image/webp;base64,",
        }
    }
}

/// Configuration for screen capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Image format for encoding.
    pub format: ImageFormat,
    /// JPEG quality (1-100), only used for JPEG format.
    pub quality: u8,
    /// WebP quality (1-100), only used for WebP format.
    pub webp_quality: u8,
    /// Maximum image dimension (width or height). Images larger than this will be scaled down.
    pub max_dimension: u32,
    /// Optional region to capture (if None, captures full screen).
    pub region: Option<CaptureRegion>,
    /// Whether to include the cursor in captures.
    pub include_cursor: bool,
    /// Capture timeout in milliseconds.
    pub timeout_ms: u64,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            format: ImageFormat::Png,
            quality: 85,
            webp_quality: 80,
            max_dimension: 1920,
            region: None,
            include_cursor: false,
            timeout_ms: 5000,
        }
    }
}

impl CaptureConfig {
    /// Create a new capture config with JPEG format optimized for vision models.
    pub fn for_vision_model() -> Self {
        Self {
            format: ImageFormat::Jpeg,
            quality: 75,
            webp_quality: 75,
            max_dimension: 1280, // Lower resolution for faster processing
            region: None,
            include_cursor: true,
            timeout_ms: 5000,
        }
    }

    /// Create a config optimized for rapid capture (1fps sustained).
    pub fn for_rapid_capture() -> Self {
        Self {
            format: ImageFormat::Jpeg,
            quality: 60,
            webp_quality: 60,
            max_dimension: 1280,
            region: None,
            include_cursor: false,
            timeout_ms: 500,
        }
    }

    /// Set the image format.
    pub fn with_format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the quality (for JPEG/WebP).
    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.min(100).max(1);
        self.webp_quality = quality.min(100).max(1);
        self
    }

    /// Set the maximum dimension.
    pub fn with_max_dimension(mut self, max_dimension: u32) -> Self {
        self.max_dimension = max_dimension.max(100);
        self
    }

    /// Set the capture region.
    pub fn with_region(mut self, region: CaptureRegion) -> Self {
        self.region = Some(region);
        self
    }
}

/// A rectangular region on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureRegion {
    /// X coordinate of the top-left corner.
    pub x: i32,
    /// Y coordinate of the top-left corner.
    pub y: i32,
    /// Width of the region.
    pub width: u32,
    /// Height of the region.
    pub height: u32,
}

impl CaptureRegion {
    /// Create a new capture region.
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

    /// Calculate the area of this region.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Calculate intersection with another region.
    pub fn intersect(&self, other: &CaptureRegion) -> Option<CaptureRegion> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width as i32).min(other.x + other.width as i32);
        let y2 = (self.y + self.height as i32).min(other.y + other.height as i32);

        if x2 > x1 && y2 > y1 {
            Some(CaptureRegion::new(x1, y1, (x2 - x1) as u32, (y2 - y1) as u32))
        } else {
            None
        }
    }
}

// ============================================================================
// Metadata Types
// ============================================================================

/// Information about a display monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Monitor index (0-based).
    pub index: u32,
    /// Monitor name/identifier.
    pub name: String,
    /// Whether this is the primary monitor.
    pub is_primary: bool,
    /// Monitor region (position and size).
    pub region: CaptureRegion,
    /// Scale factor (for HiDPI displays).
    pub scale_factor: f64,
    /// Refresh rate in Hz (if available).
    pub refresh_rate: Option<u32>,
}

impl MonitorInfo {
    /// Get the effective resolution accounting for scale factor.
    pub fn effective_resolution(&self) -> (u32, u32) {
        let width = (self.region.width as f64 * self.scale_factor) as u32;
        let height = (self.region.height as f64 * self.scale_factor) as u32;
        (width, height)
    }
}

/// Information about a window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window ID (platform-specific).
    pub id: u64,
    /// Window title.
    pub title: String,
    /// Application/process name.
    pub app_name: String,
    /// Process ID.
    pub pid: u32,
    /// Window region (position and size).
    pub region: CaptureRegion,
    /// Whether the window is minimized.
    pub is_minimized: bool,
    /// Whether the window is visible.
    pub is_visible: bool,
    /// Monitor index the window is primarily on.
    pub monitor_index: Option<u32>,
}

/// Metadata for a captured screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotMetadata {
    /// Unique identifier for this screenshot.
    pub id: Uuid,
    /// Timestamp of capture.
    pub timestamp: DateTime<Utc>,
    /// Original dimensions before any scaling.
    pub original_width: u32,
    /// Original height before any scaling.
    pub original_height: u32,
    /// Final dimensions after scaling.
    pub final_width: u32,
    /// Final height after scaling.
    pub final_height: u32,
    /// Image format used.
    pub format: ImageFormat,
    /// Quality setting used.
    pub quality: u8,
    /// Source of the capture.
    pub source: CaptureSource,
    /// Monitor info (if captured from a specific monitor).
    pub monitor: Option<MonitorInfo>,
    /// Window info (if captured from a specific window).
    pub window: Option<WindowInfo>,
    /// Time taken to capture (in milliseconds).
    pub capture_time_ms: u64,
    /// Size of encoded image in bytes.
    pub encoded_size: usize,
}

/// Source of the screenshot capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
    /// Full screen capture (all monitors).
    FullScreen,
    /// Specific monitor by index.
    Monitor(u32),
    /// Specific region.
    Region(CaptureRegion),
    /// Specific window by ID.
    Window(u64),
}

impl std::fmt::Display for CaptureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullScreen => write!(f, "Full Screen"),
            Self::Monitor(idx) => write!(f, "Monitor {}", idx),
            Self::Region(r) => write!(f, "Region ({}x{} at {},{})", r.width, r.height, r.x, r.y),
            Self::Window(id) => write!(f, "Window {}", id),
        }
    }
}

// ============================================================================
// Screenshot
// ============================================================================

/// A captured screenshot with image data and metadata.
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// The captured image.
    image: DynamicImage,
    /// Screenshot metadata.
    pub metadata: ScreenshotMetadata,
    /// Cached encoded bytes (for avoiding re-encoding).
    encoded_cache: Option<Vec<u8>>,
}

impl Screenshot {
    /// Create a new screenshot from an image.
    pub fn new(
        image: DynamicImage,
        source: CaptureSource,
        capture_start: Instant,
    ) -> Self {
        let original_width = image.width();
        let original_height = image.height();
        let capture_time_ms = capture_start.elapsed().as_millis() as u64;

        let metadata = ScreenshotMetadata {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            original_width,
            original_height,
            final_width: original_width,
            final_height: original_height,
            format: ImageFormat::Png,
            quality: 100,
            source,
            monitor: None,
            window: None,
            capture_time_ms,
            encoded_size: 0,
        };

        Self {
            image,
            metadata,
            encoded_cache: None,
        }
    }

    /// Set monitor info on the metadata.
    pub fn with_monitor(mut self, monitor: MonitorInfo) -> Self {
        self.metadata.monitor = Some(monitor);
        self
    }

    /// Set window info on the metadata.
    pub fn with_window(mut self, window: WindowInfo) -> Self {
        self.metadata.window = Some(window);
        self
    }

    /// Get the width of the screenshot.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Get the height of the screenshot.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Get a reference to the underlying image.
    pub fn image(&self) -> &DynamicImage {
        &self.image
    }

    /// Get the capture region.
    pub fn region(&self) -> CaptureRegion {
        CaptureRegion::new(0, 0, self.width(), self.height())
    }

    /// Scale the image if it exceeds the maximum dimension.
    pub fn scale_to_max_dimension(&mut self, max_dimension: u32) {
        let width = self.image.width();
        let height = self.image.height();

        if width > max_dimension || height > max_dimension {
            let scale = max_dimension as f64 / width.max(height) as f64;
            let new_width = (width as f64 * scale) as u32;
            let new_height = (height as f64 * scale) as u32;

            debug!(
                "Scaling image from {}x{} to {}x{}",
                width, height, new_width, new_height
            );

            self.image = self.image.resize(
                new_width,
                new_height,
                image::imageops::FilterType::Lanczos3,
            );
            self.metadata.final_width = new_width;
            self.metadata.final_height = new_height;
            self.encoded_cache = None; // Invalidate cache
        }
    }

    /// Crop the screenshot to a specific region.
    pub fn crop(&self, region: CaptureRegion) -> CaptureResult<Screenshot> {
        if region.x < 0 || region.y < 0 {
            return Err(CaptureError::InvalidRegion(
                "Region coordinates must be non-negative".to_string(),
            ));
        }

        let x = region.x as u32;
        let y = region.y as u32;

        if x + region.width > self.image.width() || y + region.height > self.image.height() {
            return Err(CaptureError::InvalidRegion(format!(
                "Region ({},{} {}x{}) extends beyond image bounds ({}x{})",
                region.x,
                region.y,
                region.width,
                region.height,
                self.image.width(),
                self.image.height()
            )));
        }

        let cropped = self.image.crop_imm(x, y, region.width, region.height);
        let mut screenshot = Screenshot::new(
            cropped,
            CaptureSource::Region(region),
            Instant::now(),
        );
        screenshot.metadata.timestamp = self.metadata.timestamp;
        screenshot.metadata.monitor = self.metadata.monitor.clone();

        Ok(screenshot)
    }

    /// Encode the screenshot to bytes in the specified format.
    pub fn encode(&mut self, config: &CaptureConfig) -> CaptureResult<Vec<u8>> {
        // Check if we have a valid cache
        if let Some(ref cached) = self.encoded_cache {
            if self.metadata.format == config.format && self.metadata.quality == config.quality {
                return Ok(cached.clone());
            }
        }

        // Scale if needed
        self.scale_to_max_dimension(config.max_dimension);

        let mut buffer = Cursor::new(Vec::new());

        match config.format {
            ImageFormat::Png => {
                self.image
                    .write_to(&mut buffer, image::ImageFormat::Png)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
            ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut buffer,
                    config.quality,
                );
                self.image
                    .write_with_encoder(encoder)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
            ImageFormat::WebP => {
                self.image
                    .write_to(&mut buffer, image::ImageFormat::WebP)
                    .map_err(|e| CaptureError::EncodingFailed(e.to_string()))?;
            }
        }

        let bytes = buffer.into_inner();
        self.metadata.format = config.format;
        self.metadata.quality = config.quality;
        self.metadata.encoded_size = bytes.len();
        self.encoded_cache = Some(bytes.clone());

        Ok(bytes)
    }

    /// Encode to base64 string for API transmission.
    pub fn to_base64(&mut self) -> CaptureResult<String> {
        let config = CaptureConfig::default();
        self.to_base64_with_config(&config)
    }

    /// Encode to base64 string with custom configuration.
    pub fn to_base64_with_config(&mut self, config: &CaptureConfig) -> CaptureResult<String> {
        let bytes = self.encode(config)?;
        Ok(BASE64_STANDARD.encode(&bytes))
    }

    /// Encode to data URI format for embedding in HTML/markdown.
    pub fn to_data_uri(&mut self, config: &CaptureConfig) -> CaptureResult<String> {
        let base64 = self.to_base64_with_config(config)?;
        Ok(format!("{}{}", config.format.data_uri_prefix(), base64))
    }

    /// Save the screenshot to a file.
    pub fn save(&self, path: &std::path::Path) -> CaptureResult<()> {
        self.image
            .save(path)
            .map_err(|e| CaptureError::EncodingFailed(format!("Failed to save image: {}", e)))
    }
}

// ============================================================================
// ScreenCapture Trait
// ============================================================================

/// Trait for platform-specific screen capture implementations.
#[async_trait]
pub trait ScreenCapture: Send + Sync {
    /// Check if screen capture is available on this platform.
    fn is_available(&self) -> bool;

    /// Get the configuration.
    fn config(&self) -> &CaptureConfig;

    /// Get a list of all monitors.
    async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>>;

    /// Get the primary monitor.
    async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo>;

    /// Capture the full screen (primary monitor or specific monitor).
    /// If monitor_index is None, captures the primary monitor.
    async fn capture_screen(&self, monitor_index: Option<u32>) -> CaptureResult<Screenshot>;

    /// Capture all monitors stitched together.
    async fn capture_all_monitors(&self) -> CaptureResult<Screenshot>;

    /// Capture a specific region.
    async fn capture_region(&self, region: CaptureRegion) -> CaptureResult<Screenshot>;

    /// Get a list of all visible windows.
    async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>>;

    /// Find a window by title (partial match, case-insensitive).
    async fn find_window_by_title(&self, title: &str) -> CaptureResult<Option<WindowInfo>>;

    /// Find windows by application/process name.
    async fn find_windows_by_app(&self, app_name: &str) -> CaptureResult<Vec<WindowInfo>>;

    /// Capture a specific window by ID.
    async fn capture_window(&self, window_id: u64) -> CaptureResult<Screenshot>;

    /// Capture a window by title (partial match).
    async fn capture_window_by_title(&self, title: &str) -> CaptureResult<Screenshot> {
        let window = self
            .find_window_by_title(title)
            .await?
            .ok_or_else(|| CaptureError::WindowNotFound(title.to_string()))?;
        self.capture_window(window.id).await
    }
}

// ============================================================================
// XcapCapture Implementation
// ============================================================================

/// Cross-platform screen capture implementation using the xcap crate.
#[cfg(feature = "screen-capture")]
pub struct XcapCapture {
    config: CaptureConfig,
}

#[cfg(feature = "screen-capture")]
impl XcapCapture {
    /// Create a new XcapCapture with the given configuration.
    pub fn new(config: CaptureConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CaptureConfig::default())
    }

    /// Convert xcap image buffer to DynamicImage.
    fn convert_image(
        data: Vec<u8>,
        width: u32,
        height: u32,
    ) -> CaptureResult<DynamicImage> {
        // xcap returns BGRA format, we need to convert to RGBA
        let mut rgba_data = data;
        for chunk in rgba_data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // Swap B and R channels
        }

        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width, height, rgba_data).ok_or_else(|| {
                CaptureError::CaptureFailed("Failed to create image buffer".to_string())
            })?;

        Ok(DynamicImage::ImageRgba8(buffer))
    }

    /// Convert xcap Monitor to MonitorInfo.
    fn monitor_to_info(monitor: &xcap::Monitor, index: u32) -> MonitorInfo {
        let name = monitor.name().unwrap_or_else(|_| format!("Monitor {}", index));
        let is_primary = monitor.is_primary().unwrap_or(index == 0);
        let x = monitor.x().unwrap_or(0);
        let y = monitor.y().unwrap_or(0);
        let width = monitor.width().unwrap_or(1920);
        let height = monitor.height().unwrap_or(1080);
        let scale_factor = monitor.scale_factor().unwrap_or(1.0) as f64;

        MonitorInfo {
            index,
            name,
            is_primary,
            region: CaptureRegion::new(x, y, width, height),
            scale_factor,
            refresh_rate: None, // xcap doesn't provide this
        }
    }

    /// Convert xcap Window to WindowInfo.
    fn window_to_info(window: &xcap::Window) -> WindowInfo {
        let id = window.id().unwrap_or(0) as u64;
        let title = window.title().unwrap_or_else(|_| String::new());
        let app_name = window.app_name().unwrap_or_else(|_| String::new());
        let x = window.x().unwrap_or(0);
        let y = window.y().unwrap_or(0);
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        let is_minimized = window.is_minimized().unwrap_or(false);

        WindowInfo {
            id,
            title,
            app_name,
            pid: id as u32, // xcap uses id as pid approximation
            region: CaptureRegion::new(x, y, width, height),
            is_minimized,
            is_visible: !is_minimized,
            monitor_index: None,
        }
    }
}

#[cfg(feature = "screen-capture")]
#[async_trait]
impl ScreenCapture for XcapCapture {
    fn is_available(&self) -> bool {
        // Check if we can get monitors
        xcap::Monitor::all().is_ok()
    }

    fn config(&self) -> &CaptureConfig {
        &self.config
    }

    async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>> {
        let monitors = xcap::Monitor::all()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        Ok(monitors
            .iter()
            .enumerate()
            .map(|(i, m)| Self::monitor_to_info(m, i as u32))
            .collect())
    }

    async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo> {
        let monitors = self.get_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.is_primary)
            .or_else(|| None)
            .ok_or(CaptureError::MonitorNotFound(0))
    }

    async fn capture_screen(&self, monitor_index: Option<u32>) -> CaptureResult<Screenshot> {
        let start = Instant::now();

        let monitors = xcap::Monitor::all()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        let (monitor, index) = if let Some(idx) = monitor_index {
            let m = monitors
                .into_iter()
                .nth(idx as usize)
                .ok_or(CaptureError::MonitorNotFound(idx))?;
            (m, idx)
        } else {
            // Find primary monitor
            let primary_idx = monitors
                .iter()
                .position(|m| m.is_primary().unwrap_or(false))
                .unwrap_or(0);
            let m = monitors
                .into_iter()
                .nth(primary_idx)
                .ok_or(CaptureError::MonitorNotFound(0))?;
            (m, primary_idx as u32)
        };

        let monitor_info = Self::monitor_to_info(&monitor, index);

        let capture = monitor
            .capture_image()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        let width = capture.width();
        let height = capture.height();
        let data = capture.into_raw();

        let image = Self::convert_image(data, width, height)?;

        let mut screenshot = Screenshot::new(image, CaptureSource::Monitor(index), start);
        screenshot = screenshot.with_monitor(monitor_info);

        info!(
            "Captured monitor {} ({}x{}) in {}ms",
            index,
            width,
            height,
            start.elapsed().as_millis()
        );

        Ok(screenshot)
    }

    async fn capture_all_monitors(&self) -> CaptureResult<Screenshot> {
        let start = Instant::now();

        let monitors = xcap::Monitor::all()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        if monitors.is_empty() {
            return Err(CaptureError::NotAvailable("No monitors found".to_string()));
        }

        // For now, just capture the primary monitor
        // TODO: Implement proper multi-monitor stitching
        let primary_idx = monitors
            .iter()
            .position(|m| m.is_primary().unwrap_or(false))
            .unwrap_or(0);

        let monitor = &monitors[primary_idx];
        let capture = monitor
            .capture_image()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        let width = capture.width();
        let height = capture.height();
        let data = capture.into_raw();

        let image = Self::convert_image(data, width, height)?;
        let screenshot = Screenshot::new(image, CaptureSource::FullScreen, start);

        info!(
            "Captured all monitors ({}x{}) in {}ms",
            width,
            height,
            start.elapsed().as_millis()
        );

        Ok(screenshot)
    }

    async fn capture_region(&self, region: CaptureRegion) -> CaptureResult<Screenshot> {
        if !region.is_valid() {
            return Err(CaptureError::InvalidRegion(
                "Region must have positive dimensions".to_string(),
            ));
        }

        // Capture full screen and crop
        let full = self.capture_screen(None).await?;
        full.crop(region)
    }

    async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>> {
        let windows = xcap::Window::all()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        Ok(windows
            .iter()
            .filter(|w| !w.is_minimized().unwrap_or(false))
            .map(Self::window_to_info)
            .collect())
    }

    async fn find_window_by_title(&self, title: &str) -> CaptureResult<Option<WindowInfo>> {
        let windows = self.get_windows().await?;
        let title_lower = title.to_lowercase();
        Ok(windows
            .into_iter()
            .find(|w| w.title.to_lowercase().contains(&title_lower)))
    }

    async fn find_windows_by_app(&self, app_name: &str) -> CaptureResult<Vec<WindowInfo>> {
        let windows = self.get_windows().await?;
        let name_lower = app_name.to_lowercase();
        Ok(windows
            .into_iter()
            .filter(|w| w.app_name.to_lowercase().contains(&name_lower))
            .collect())
    }

    async fn capture_window(&self, window_id: u64) -> CaptureResult<Screenshot> {
        let start = Instant::now();

        let windows = xcap::Window::all()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        let window = windows
            .into_iter()
            .find(|w| w.id().unwrap_or(0) as u64 == window_id)
            .ok_or_else(|| CaptureError::WindowNotFound(format!("ID: {}", window_id)))?;

        let window_info = Self::window_to_info(&window);

        let capture = window
            .capture_image()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        let width = capture.width();
        let height = capture.height();
        let data = capture.into_raw();

        let image = Self::convert_image(data, width, height)?;

        let mut screenshot = Screenshot::new(image, CaptureSource::Window(window_id), start);
        screenshot = screenshot.with_window(window_info);

        info!(
            "Captured window {} ({}x{}) in {}ms",
            window_id,
            width,
            height,
            start.elapsed().as_millis()
        );

        Ok(screenshot)
    }
}

// ============================================================================
// ScreenBuffer - Screenshot Queue for Rapid Capture
// ============================================================================

/// Configuration for the screen buffer.
#[derive(Debug, Clone)]
pub struct ScreenBufferConfig {
    /// Maximum number of screenshots to buffer.
    pub max_size: usize,
    /// Target capture rate (frames per second).
    pub target_fps: f32,
    /// Whether to drop old frames when buffer is full.
    pub drop_oldest: bool,
    /// Capture configuration.
    pub capture_config: CaptureConfig,
}

impl Default for ScreenBufferConfig {
    fn default() -> Self {
        Self {
            max_size: 60, // 1 minute at 1fps
            target_fps: 1.0,
            drop_oldest: true,
            capture_config: CaptureConfig::for_rapid_capture(),
        }
    }
}

/// A buffered screenshot with pre-encoded data.
#[derive(Debug, Clone)]
pub struct BufferedScreenshot {
    /// Screenshot metadata.
    pub metadata: ScreenshotMetadata,
    /// Pre-encoded image bytes.
    pub encoded_bytes: Vec<u8>,
    /// Pre-computed base64 string.
    pub base64: String,
}

impl BufferedScreenshot {
    /// Create from a screenshot.
    pub fn from_screenshot(mut screenshot: Screenshot, config: &CaptureConfig) -> CaptureResult<Self> {
        let encoded_bytes = screenshot.encode(config)?;
        let base64 = BASE64_STANDARD.encode(&encoded_bytes);

        Ok(Self {
            metadata: screenshot.metadata,
            encoded_bytes,
            base64,
        })
    }
}

/// A thread-safe screenshot buffer for rapid capture.
///
/// This buffer maintains a queue of recent screenshots and can
/// capture at a sustained rate of 1fps or higher.
pub struct ScreenBuffer<C: ScreenCapture> {
    /// Capture implementation.
    capture: Arc<C>,
    /// Buffer configuration.
    config: ScreenBufferConfig,
    /// Screenshot queue.
    buffer: Arc<Mutex<VecDeque<BufferedScreenshot>>>,
    /// Whether continuous capture is running.
    running: Arc<RwLock<bool>>,
    /// Statistics.
    stats: Arc<RwLock<BufferStats>>,
}

/// Statistics for the screen buffer.
#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    /// Total screenshots captured.
    pub total_captures: u64,
    /// Screenshots dropped due to buffer full.
    pub dropped_frames: u64,
    /// Capture errors encountered.
    pub capture_errors: u64,
    /// Average capture time in milliseconds.
    pub avg_capture_time_ms: f64,
    /// Last capture timestamp.
    pub last_capture: Option<DateTime<Utc>>,
    /// Current buffer size.
    pub current_size: usize,
    /// Actual FPS achieved.
    pub actual_fps: f64,
}

impl<C: ScreenCapture + 'static> ScreenBuffer<C> {
    /// Create a new screen buffer.
    pub fn new(capture: C, config: ScreenBufferConfig) -> Self {
        Self {
            capture: Arc::new(capture),
            config,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            running: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(BufferStats::default())),
        }
    }

    /// Get the buffer configuration.
    pub fn config(&self) -> &ScreenBufferConfig {
        &self.config
    }

    /// Get current buffer statistics.
    pub async fn stats(&self) -> BufferStats {
        self.stats.read().await.clone()
    }

    /// Get the current number of buffered screenshots.
    pub async fn len(&self) -> usize {
        self.buffer.lock().await.len()
    }

    /// Check if the buffer is empty.
    pub async fn is_empty(&self) -> bool {
        self.buffer.lock().await.is_empty()
    }

    /// Capture a single screenshot and add to buffer.
    pub async fn capture_one(&self) -> CaptureResult<()> {
        let start = Instant::now();

        let screenshot = self.capture.capture_screen(None).await?;
        let buffered = BufferedScreenshot::from_screenshot(
            screenshot,
            &self.config.capture_config,
        )?;

        let mut buffer = self.buffer.lock().await;
        let mut stats = self.stats.write().await;

        // Drop oldest if buffer is full
        if buffer.len() >= self.config.max_size {
            if self.config.drop_oldest {
                buffer.pop_front();
                stats.dropped_frames += 1;
            } else {
                return Err(CaptureError::BufferError("Buffer full".to_string()));
            }
        }

        buffer.push_back(buffered);
        stats.total_captures += 1;
        stats.last_capture = Some(Utc::now());
        stats.current_size = buffer.len();

        // Update average capture time
        let capture_time = start.elapsed().as_millis() as f64;
        stats.avg_capture_time_ms = if stats.total_captures == 1 {
            capture_time
        } else {
            (stats.avg_capture_time_ms * (stats.total_captures - 1) as f64 + capture_time)
                / stats.total_captures as f64
        };

        Ok(())
    }

    /// Start continuous capture at the configured frame rate.
    pub async fn start_continuous(&self) {
        let mut running = self.running.write().await;
        if *running {
            warn!("Continuous capture already running");
            return;
        }
        *running = true;
        drop(running);

        let capture = self.capture.clone();
        let buffer = self.buffer.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let interval = Duration::from_secs_f32(1.0 / config.target_fps);
            let mut last_capture = Instant::now();
            let mut fps_counter = 0u64;
            let mut fps_timer = Instant::now();

            info!(
                "Starting continuous capture at {} fps (interval: {:?})",
                config.target_fps, interval
            );

            loop {
                // Check if we should stop
                if !*running.read().await {
                    info!("Stopping continuous capture");
                    break;
                }

                // Calculate time to next capture
                let elapsed = last_capture.elapsed();
                if elapsed < interval {
                    tokio::time::sleep(interval - elapsed).await;
                }

                last_capture = Instant::now();

                // Capture
                match capture.capture_screen(None).await {
                    Ok(screenshot) => {
                        match BufferedScreenshot::from_screenshot(
                            screenshot,
                            &config.capture_config,
                        ) {
                            Ok(buffered) => {
                                let mut buf = buffer.lock().await;
                                let mut st = stats.write().await;

                                if buf.len() >= config.max_size {
                                    if config.drop_oldest {
                                        buf.pop_front();
                                        st.dropped_frames += 1;
                                    }
                                }

                                buf.push_back(buffered);
                                st.total_captures += 1;
                                st.last_capture = Some(Utc::now());
                                st.current_size = buf.len();
                                fps_counter += 1;
                            }
                            Err(e) => {
                                error!("Failed to encode screenshot: {}", e);
                                let mut st = stats.write().await;
                                st.capture_errors += 1;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to capture: {}", e);
                        let mut st = stats.write().await;
                        st.capture_errors += 1;
                    }
                }

                // Update FPS stats every second
                if fps_timer.elapsed() >= Duration::from_secs(1) {
                    let mut st = stats.write().await;
                    st.actual_fps = fps_counter as f64 / fps_timer.elapsed().as_secs_f64();
                    fps_counter = 0;
                    fps_timer = Instant::now();
                }
            }
        });
    }

    /// Stop continuous capture.
    pub async fn stop_continuous(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Check if continuous capture is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get the most recent screenshot.
    pub async fn latest(&self) -> Option<BufferedScreenshot> {
        self.buffer.lock().await.back().cloned()
    }

    /// Get the oldest screenshot.
    pub async fn oldest(&self) -> Option<BufferedScreenshot> {
        self.buffer.lock().await.front().cloned()
    }

    /// Pop the oldest screenshot from the buffer.
    pub async fn pop(&self) -> Option<BufferedScreenshot> {
        let mut buffer = self.buffer.lock().await;
        let result = buffer.pop_front();

        let mut stats = self.stats.write().await;
        stats.current_size = buffer.len();

        result
    }

    /// Get all buffered screenshots.
    pub async fn drain(&self) -> Vec<BufferedScreenshot> {
        let mut buffer = self.buffer.lock().await;
        let result: Vec<_> = buffer.drain(..).collect();

        let mut stats = self.stats.write().await;
        stats.current_size = 0;

        result
    }

    /// Clear the buffer.
    pub async fn clear(&self) {
        let mut buffer = self.buffer.lock().await;
        buffer.clear();

        let mut stats = self.stats.write().await;
        stats.current_size = 0;
    }

    /// Get screenshots within a time range.
    pub async fn get_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<BufferedScreenshot> {
        let buffer = self.buffer.lock().await;
        buffer
            .iter()
            .filter(|s| s.metadata.timestamp >= start && s.metadata.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Get the last N screenshots.
    pub async fn get_last(&self, n: usize) -> Vec<BufferedScreenshot> {
        let buffer = self.buffer.lock().await;
        buffer.iter().rev().take(n).cloned().collect()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Capture the primary screen with default settings.
#[cfg(feature = "screen-capture")]
pub async fn capture_screen() -> CaptureResult<Screenshot> {
    let capture = XcapCapture::with_defaults();
    capture.capture_screen(None).await
}

/// Capture a specific region with default settings.
#[cfg(feature = "screen-capture")]
pub async fn capture_region(region: CaptureRegion) -> CaptureResult<Screenshot> {
    let capture = XcapCapture::with_defaults();
    capture.capture_region(region).await
}

/// Capture a window by title with default settings.
#[cfg(feature = "screen-capture")]
pub async fn capture_window(title: &str) -> CaptureResult<Screenshot> {
    let capture = XcapCapture::with_defaults();
    capture.capture_window_by_title(title).await
}

/// Convert a screenshot to base64 with vision model optimized settings.
pub fn to_base64(screenshot: &mut Screenshot) -> CaptureResult<String> {
    let config = CaptureConfig::for_vision_model();
    screenshot.to_base64_with_config(&config)
}

// ============================================================================
// Mock Implementation (when screen-capture feature is disabled)
// ============================================================================

/// Mock screen capture for testing or when screen-capture feature is disabled.
#[cfg(not(feature = "screen-capture"))]
pub struct MockCapture {
    config: CaptureConfig,
}

#[cfg(not(feature = "screen-capture"))]
impl MockCapture {
    pub fn new(config: CaptureConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(CaptureConfig::default())
    }
}

#[cfg(not(feature = "screen-capture"))]
#[async_trait]
impl ScreenCapture for MockCapture {
    fn is_available(&self) -> bool {
        false
    }

    fn config(&self) -> &CaptureConfig {
        &self.config
    }

    async fn get_monitors(&self) -> CaptureResult<Vec<MonitorInfo>> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn get_primary_monitor(&self) -> CaptureResult<MonitorInfo> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn capture_screen(&self, _monitor_index: Option<u32>) -> CaptureResult<Screenshot> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn capture_all_monitors(&self) -> CaptureResult<Screenshot> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn capture_region(&self, _region: CaptureRegion) -> CaptureResult<Screenshot> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn get_windows(&self) -> CaptureResult<Vec<WindowInfo>> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn find_window_by_title(&self, _title: &str) -> CaptureResult<Option<WindowInfo>> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn find_windows_by_app(&self, _app_name: &str) -> CaptureResult<Vec<WindowInfo>> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }

    async fn capture_window(&self, _window_id: u64) -> CaptureResult<Screenshot> {
        Err(CaptureError::NotAvailable(
            "Screen capture feature not enabled".to_string(),
        ))
    }
}

/// Type alias for the default capture implementation.
#[cfg(feature = "screen-capture")]
pub type DefaultCapture = XcapCapture;

#[cfg(not(feature = "screen-capture"))]
pub type DefaultCapture = MockCapture;

/// Create the default screen capture implementation.
pub fn create_capture(config: CaptureConfig) -> DefaultCapture {
    DefaultCapture::new(config)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_region_validity() {
        let valid = CaptureRegion::new(0, 0, 100, 100);
        assert!(valid.is_valid());

        let invalid_width = CaptureRegion::new(0, 0, 0, 100);
        assert!(!invalid_width.is_valid());

        let invalid_height = CaptureRegion::new(0, 0, 100, 0);
        assert!(!invalid_height.is_valid());
    }

    #[test]
    fn test_capture_region_contains() {
        let region = CaptureRegion::new(100, 100, 200, 200);
        assert!(region.contains(150, 150));
        assert!(region.contains(100, 100));
        assert!(!region.contains(50, 50));
        assert!(!region.contains(350, 150));
    }

    #[test]
    fn test_capture_region_center() {
        let region = CaptureRegion::new(100, 100, 200, 200);
        assert_eq!(region.center(), (200, 200));
    }

    #[test]
    fn test_capture_region_intersect() {
        let r1 = CaptureRegion::new(0, 0, 100, 100);
        let r2 = CaptureRegion::new(50, 50, 100, 100);

        let intersection = r1.intersect(&r2).unwrap();
        assert_eq!(intersection.x, 50);
        assert_eq!(intersection.y, 50);
        assert_eq!(intersection.width, 50);
        assert_eq!(intersection.height, 50);

        let r3 = CaptureRegion::new(200, 200, 100, 100);
        assert!(r1.intersect(&r3).is_none());
    }

    #[test]
    fn test_image_format() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.data_uri_prefix(), "data:image/webp;base64,");
    }

    #[test]
    fn test_capture_config_defaults() {
        let config = CaptureConfig::default();
        assert_eq!(config.format, ImageFormat::Png);
        assert_eq!(config.quality, 85);
        assert_eq!(config.max_dimension, 1920);
    }

    #[test]
    fn test_capture_config_for_vision_model() {
        let config = CaptureConfig::for_vision_model();
        assert_eq!(config.format, ImageFormat::Jpeg);
        assert_eq!(config.quality, 75);
        assert_eq!(config.max_dimension, 1280);
    }

    #[test]
    fn test_capture_config_builder() {
        let config = CaptureConfig::default()
            .with_format(ImageFormat::WebP)
            .with_quality(90)
            .with_max_dimension(2048);

        assert_eq!(config.format, ImageFormat::WebP);
        assert_eq!(config.quality, 90);
        assert_eq!(config.webp_quality, 90);
        assert_eq!(config.max_dimension, 2048);
    }

    #[test]
    fn test_screen_buffer_config_defaults() {
        let config = ScreenBufferConfig::default();
        assert_eq!(config.max_size, 60);
        assert_eq!(config.target_fps, 1.0);
        assert!(config.drop_oldest);
    }

    #[test]
    fn test_monitor_info_effective_resolution() {
        let monitor = MonitorInfo {
            index: 0,
            name: "Test".to_string(),
            is_primary: true,
            region: CaptureRegion::new(0, 0, 1920, 1080),
            scale_factor: 2.0,
            refresh_rate: Some(60),
        };

        let (width, height) = monitor.effective_resolution();
        assert_eq!(width, 3840);
        assert_eq!(height, 2160);
    }
}
