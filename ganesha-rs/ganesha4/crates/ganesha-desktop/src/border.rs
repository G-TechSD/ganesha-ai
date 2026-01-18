//! Screen border overlay indicator

use crate::{config::DesktopConfig, Result};

/// Manages the screen border overlay that shows when Ganesha has control
pub struct BorderOverlay {
    enabled: bool,
    visible: bool,
    color: String,
    width: u32,
    animate: bool,
}

impl BorderOverlay {
    /// Create a new border overlay
    pub fn new(config: &DesktopConfig) -> Result<Self> {
        Ok(Self {
            enabled: config.border.enabled,
            visible: false,
            color: config.border.color.clone(),
            width: config.border.width,
            animate: config.border.animate,
        })
    }

    /// Show the border overlay
    pub fn show(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.visible = true;

        // Implementation approach:
        // Create 4 thin windows (top, bottom, left, right) around screen edges
        // Make them always-on-top, click-through, and borderless
        // Fill with the configured color

        tracing::info!("Border overlay shown (color: {}, width: {}px)", self.color, self.width);
        Ok(())
    }

    /// Hide the border overlay
    pub fn hide(&mut self) -> Result<()> {
        self.visible = false;
        tracing::info!("Border overlay hidden");
        Ok(())
    }

    /// Check if overlay is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Update border color
    pub fn set_color(&mut self, color: &str) -> Result<()> {
        self.color = color.to_string();
        if self.visible {
            // Refresh the overlay with new color
            self.refresh()?;
        }
        Ok(())
    }

    /// Update border width
    pub fn set_width(&mut self, width: u32) -> Result<()> {
        self.width = width;
        if self.visible {
            self.refresh()?;
        }
        Ok(())
    }

    /// Refresh the overlay (redraw with current settings)
    fn refresh(&self) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        // Redraw the border windows
        Ok(())
    }

    /// Flash the border (for attention)
    pub fn flash(&self, times: u32, interval_ms: u64) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Temporarily show/hide the border for attention effect
        tracing::debug!("Border flash: {} times at {}ms interval", times, interval_ms);
        Ok(())
    }

    /// Pulse animation (gradual opacity change)
    pub fn pulse(&self) -> Result<()> {
        if !self.animate {
            return Ok(());
        }

        // Create a smooth pulsing animation
        tracing::debug!("Border pulse animation");
        Ok(())
    }

    /// Set border to indicate different states
    pub fn set_state(&mut self, state: BorderState) -> Result<()> {
        match state {
            BorderState::Active => {
                self.color = "#00FF00".to_string(); // Green
                self.show()?;
            }
            BorderState::Processing => {
                self.color = "#FFAA00".to_string(); // Orange
                self.show()?;
                if self.animate {
                    self.pulse()?;
                }
            }
            BorderState::Warning => {
                self.color = "#FF0000".to_string(); // Red
                self.show()?;
                self.flash(3, 200)?;
            }
            BorderState::Inactive => {
                self.hide()?;
            }
        }
        Ok(())
    }

    /// Get current color
    pub fn color(&self) -> &str {
        &self.color
    }

    /// Get current width
    pub fn width(&self) -> u32 {
        self.width
    }
}

/// Border overlay states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderState {
    /// Ganesha has active control (green)
    Active,
    /// Processing a request (orange, pulsing)
    Processing,
    /// Warning/attention needed (red, flashing)
    Warning,
    /// No control, hidden
    Inactive,
}

/// Border window position
#[derive(Debug, Clone, Copy)]
pub enum BorderPosition {
    Top,
    Bottom,
    Left,
    Right,
}

/// Calculate border window geometry
pub fn calculate_border_geometry(
    screen_width: u32,
    screen_height: u32,
    border_width: u32,
    position: BorderPosition,
) -> (i32, i32, u32, u32) {
    match position {
        BorderPosition::Top => (0, 0, screen_width, border_width),
        BorderPosition::Bottom => (
            0,
            (screen_height - border_width) as i32,
            screen_width,
            border_width,
        ),
        BorderPosition::Left => (0, 0, border_width, screen_height),
        BorderPosition::Right => (
            (screen_width - border_width) as i32,
            0,
            border_width,
            screen_height,
        ),
    }
}
