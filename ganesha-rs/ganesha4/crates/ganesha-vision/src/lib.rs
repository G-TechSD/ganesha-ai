//! # Ganesha Vision
//!
//! Vision and VLA (Vision-Language-Action) capabilities for Ganesha.
//!
//! ## Features
//!
//! - Screen capture and analysis
//! - GUI automation via VLA models
//! - Application interaction (Blender, Bambu Studio, CapCut, OBS)
//! - App whitelist/blacklist for vision access
//!
//! ## Coming Soon
//!
//! This crate is a placeholder for the vision subsystem.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VisionError {
    #[error("Screen capture failed: {0}")]
    CaptureError(String),

    #[error("Vision model error: {0}")]
    ModelError(String),

    #[error("App not whitelisted: {0}")]
    AppNotAllowed(String),
}

pub type Result<T> = std::result::Result<T, VisionError>;

/// Vision system placeholder
pub struct VisionSystem {
    enabled: bool,
}

impl VisionSystem {
    pub fn new() -> Self {
        Self { enabled: false }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for VisionSystem {
    fn default() -> Self {
        Self::new()
    }
}
