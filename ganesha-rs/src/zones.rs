//! NVR-Style Zone Filtering for Screen Analysis
//!
//! Like a surveillance NVR that ignores static areas and focuses on motion,
//! this module tracks screen zones and filters noise before vision analysis.
//!
//! - IGNORE ZONES: UI chrome, ads, whitespace (never process)
//! - MOTION ZONES: Only process when changed
//! - FOCUS ZONES: Always process (current task area)
//! - SPATIAL MEMORY: Remember last state of each zone

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Zone types for filtering
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoneType {
    /// Always ignore (taskbar, browser chrome, known ads)
    Ignore,
    /// Only process when motion detected
    Motion,
    /// Always process (active task area)
    Focus,
    /// Dynamically learned to be irrelevant
    Learned,
}

/// A rectangular zone on screen
#[derive(Debug, Clone)]
pub struct Zone {
    pub id: String,
    pub zone_type: ZoneType,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Last content hash for motion detection
    pub last_hash: u64,
    /// Last time this zone changed
    pub last_change: Instant,
    /// Last known content description
    pub last_content: String,
    /// How many times we've seen this unchanged
    pub stable_count: u32,
}

impl Zone {
    pub fn new(id: &str, zone_type: ZoneType, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            id: id.to_string(),
            zone_type,
            x,
            y,
            width,
            height,
            last_hash: 0,
            last_change: Instant::now(),
            last_content: String::new(),
            stable_count: 0,
        }
    }

    /// Check if zone has been stable (unchanged) for a while
    pub fn is_stable(&self, threshold: Duration) -> bool {
        self.last_change.elapsed() > threshold
    }

    /// Check if zone should be processed based on type and state
    pub fn should_process(&self) -> bool {
        match self.zone_type {
            ZoneType::Ignore => false,
            ZoneType::Focus => true,
            ZoneType::Motion => self.stable_count < 3, // Process if recently changed
            ZoneType::Learned => self.stable_count < 5,
        }
    }
}

/// Screen zone manager with spatial memory
pub struct ZoneManager {
    pub zones: HashMap<String, Zone>,
    pub screen_width: u32,
    pub screen_height: u32,
    /// Default zones for common UI patterns
    pub presets: HashMap<String, Vec<Zone>>,
}

/// Zone Manager for NVR-style screen region filtering
///
/// Implements a BIOS-configurable zone system that filters vision processing
/// to specific screen regions, reducing false positives and improving performance.
/// Similar to Network Video Recorder (NVR) motion detection zones.
impl ZoneManager {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let mut manager = Self {
            zones: HashMap::new(),
            screen_width,
            screen_height,
            presets: HashMap::new(),
        };

        // Define common presets
        manager.define_presets();
        manager
    }

    /// Define preset zone configurations for common screen layouts
    ///
    /// Presets include:
    /// - `fullscreen`: Process entire display (1920x1048)
    /// - `main_content`: Center content area excluding UI chrome
    /// - `ignore_taskbar`: Everything except bottom taskbar
    fn define_presets(&mut self) {
        // Linux desktop preset (GNOME-like)
        let linux_desktop = vec![
            // Top panel - usually static
            Zone::new("top_panel", ZoneType::Ignore, 0, 0, 1920, 32),
            // Dock/taskbar at bottom
            Zone::new("bottom_dock", ZoneType::Ignore, 0, 1048, 1920, 32),
            // Main content area - motion detection
            Zone::new("main_content", ZoneType::Motion, 0, 32, 1920, 1016),
        ];
        self.presets.insert("linux_desktop".into(), linux_desktop);

        // Browser preset
        let browser = vec![
            // Browser toolbar/tabs
            Zone::new("browser_chrome", ZoneType::Ignore, 0, 0, 1920, 120),
            // Left sidebar (bookmarks, etc) - often ignorable
            Zone::new("left_sidebar", ZoneType::Learned, 0, 120, 200, 900),
            // Right sidebar (ads often here)
            Zone::new("right_sidebar", ZoneType::Learned, 1720, 120, 200, 900),
            // Main content
            Zone::new("page_content", ZoneType::Focus, 200, 120, 1520, 900),
        ];
        self.presets.insert("browser".into(), browser);

        // eBay specific
        let ebay = vec![
            Zone::new("header", ZoneType::Ignore, 0, 0, 1920, 180),
            Zone::new("left_filters", ZoneType::Learned, 0, 180, 250, 800),
            Zone::new("listings", ZoneType::Focus, 250, 180, 1400, 800),
            Zone::new("right_ads", ZoneType::Ignore, 1650, 180, 270, 800),
            Zone::new("footer", ZoneType::Ignore, 0, 980, 1920, 100),
        ];
        self.presets.insert("ebay".into(), ebay);
    }

    /// Load a preset zone configuration
    pub fn load_preset(&mut self, name: &str) {
        if let Some(zones) = self.presets.get(name).cloned() {
            self.zones.clear();
            for zone in zones {
                self.zones.insert(zone.id.clone(), zone);
            }
        }
    }

    /// Auto-detect which preset to use based on URL/title
    pub fn auto_detect_preset(&mut self, url: &str, title: &str) {
        let url_lower = url.to_lowercase();

        if url_lower.contains("ebay.com") {
            self.load_preset("ebay");
        } else if url_lower.starts_with("http") {
            self.load_preset("browser");
        } else {
            self.load_preset("linux_desktop");
        }
    }

    /// Add a custom ignore zone (e.g., detected ad)
    pub fn add_ignore_zone(&mut self, id: &str, x: u32, y: u32, width: u32, height: u32) {
        self.zones.insert(
            id.to_string(),
            Zone::new(id, ZoneType::Ignore, x, y, width, height),
        );
    }

    /// Set focus zone (where the action is happening)
    pub fn set_focus(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.zones.insert(
            "active_focus".to_string(),
            Zone::new("active_focus", ZoneType::Focus, x, y, width, height),
        );
    }

    /// Update zone with new content hash, returns true if changed
    pub fn update_zone(&mut self, zone_id: &str, new_hash: u64, content_desc: &str) -> bool {
        if let Some(zone) = self.zones.get_mut(zone_id) {
            if zone.last_hash != new_hash {
                zone.last_hash = new_hash;
                zone.last_change = Instant::now();
                zone.last_content = content_desc.to_string();
                zone.stable_count = 0;
                return true;
            } else {
                zone.stable_count += 1;
            }
        }
        false
    }

    /// Get zones that should be processed (changed or focus)
    pub fn get_active_zones(&self) -> Vec<&Zone> {
        self.zones
            .values()
            .filter(|z| z.should_process())
            .collect()
    }

    /// Get zones to ignore (for masking)
    pub fn get_ignore_zones(&self) -> Vec<&Zone> {
        self.zones
            .values()
            .filter(|z| !z.should_process())
            .collect()
    }

    /// Create a mask image (black = ignore, white = process)
    /// Returns raw bytes for a grayscale mask
    pub fn create_mask(&self) -> Vec<u8> {
        let mut mask = vec![255u8; (self.screen_width * self.screen_height) as usize];

        // Black out ignore zones
        for zone in self.get_ignore_zones() {
            for y in zone.y..(zone.y + zone.height).min(self.screen_height) {
                for x in zone.x..(zone.x + zone.width).min(self.screen_width) {
                    let idx = (y * self.screen_width + x) as usize;
                    if idx < mask.len() {
                        mask[idx] = 0; // Black = ignore
                    }
                }
            }
        }

        mask
    }

    /// Get summary of what zones are active
    pub fn status(&self) -> String {
        let active: Vec<_> = self.get_active_zones().iter().map(|z| &z.id).collect();
        let ignored: Vec<_> = self.get_ignore_zones().iter().map(|z| &z.id).collect();

        format!(
            "Active: {:?}, Ignored: {:?}",
            active, ignored
        )
    }

    /// Learn to ignore a zone that hasn't changed in a while
    pub fn learn_static_zones(&mut self, stable_threshold: Duration) {
        for zone in self.zones.values_mut() {
            if zone.zone_type == ZoneType::Motion && zone.is_stable(stable_threshold) {
                zone.zone_type = ZoneType::Learned;
            }
        }
    }
}

/// Simple hash for image region (for motion detection)
pub fn hash_region(pixels: &[u8], width: u32, x: u32, y: u32, w: u32, h: u32) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    // Sample every 10th pixel for speed
    for row in (y..y + h).step_by(10) {
        for col in (x..x + w).step_by(10) {
            let idx = ((row * width + col) * 4) as usize; // RGBA
            if idx + 3 < pixels.len() {
                // Hash RGB (ignore alpha)
                pixels[idx..idx + 3].hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

/// Detect which zones have motion (changed since last check)
pub fn detect_motion(
    manager: &mut ZoneManager,
    current_pixels: &[u8],
    img_width: u32,
) -> Vec<String> {
    let mut changed = Vec::new();

    for (id, zone) in manager.zones.iter_mut() {
        if zone.zone_type == ZoneType::Ignore {
            continue;
        }

        let new_hash = hash_region(
            current_pixels,
            img_width,
            zone.x,
            zone.y,
            zone.width,
            zone.height,
        );

        if zone.last_hash != new_hash {
            zone.last_hash = new_hash;
            zone.last_change = Instant::now();
            zone.stable_count = 0;
            changed.push(id.clone());
        } else {
            zone.stable_count += 1;
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_manager() {
        let mut manager = ZoneManager::new(1920, 1080);
        manager.load_preset("browser");

        assert!(manager.zones.contains_key("browser_chrome"));
        assert!(manager.zones.contains_key("page_content"));

        let active = manager.get_active_zones();
        assert!(active.iter().any(|z| z.id == "page_content"));
    }

    #[test]
    fn test_auto_detect() {
        let mut manager = ZoneManager::new(1920, 1080);
        manager.auto_detect_preset("https://www.ebay.com/sch/i.html", "eBay");

        assert!(manager.zones.contains_key("listings"));
        assert!(manager.zones.contains_key("right_ads"));
    }
}
