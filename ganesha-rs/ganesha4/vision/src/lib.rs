//! # Ganesha Vision System
//!
//! A learning-from-demonstration system with persistent memory for
//! recording, storing, and replaying UI interactions. This is the KEY
//! differentiator for Ganesha - it learns from user demonstrations.
//!
//! # Architecture
//!
//! The vision system is organized into the following modules:
//!
//! - `capture` - Screen capture with xcap (multi-monitor, window, region support)
//! - `controller` - Main orchestrator for vision-driven computer control
//! - `db` - SQLite database layer for persistent storage
//! - `error` - Error types and handling
//! - `learning` - Learning engine for recording, extracting, and applying skills
//! - `model` - Vision model integration for local LLMs
//! - `overlay` - Desktop overlay for visual control indicators (red border, status window)
//!
//! # Learning from Demonstration
//!
//! The key insight: Show Ganesha how to navigate Blender menus once, and it
//! should be able to navigate similar menus in other apps by generalizing
//! the pattern.
//!
//! ```no_run
//! use ganesha_vision::{Database, LearningEngine};
//! use ganesha_vision::learning::Screenshot as LearningScreenshot;
//! use ganesha_vision::db::MouseButton;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ganesha_vision::Error> {
//!     // Initialize the learning engine
//!     let db = Database::open("ganesha_vision.db")?;
//!     let engine = LearningEngine::new(db);
//!
//!     // Start recording a demonstration
//!     let _session_id = engine.start_recording("Blender", "Navigate to render settings")?;
//!
//!     // Record user actions (in practice, these come from input monitoring)
//!     engine.record_click(100, 50, MouseButton::Left)?;
//!     engine.record_click(150, 100, MouseButton::Left)?;
//!     engine.record_text("settings")?;
//!
//!     // Stop recording and extract a skill
//!     let demo = engine.stop_recording()?;
//!     let skill = engine.extract_skill(&demo, "Navigate menu hierarchy")?;
//!
//!     // Later, when facing a similar task in a different app...
//!     let screenshot = LearningScreenshot::new("base64_data".to_string(), 1920, 1080)
//!         .with_app_info("GIMP", "GIMP - Image Editor");
//!
//!     // Find relevant skills
//!     let matches = engine.find_relevant_skills("open preferences dialog", &screenshot)?;
//!     if !matches.is_empty() {
//!         // Apply the best matching skill
//!         let actions = engine.apply_skill(&matches[0].skill, &screenshot)?;
//!         println!("Generated {} actions to apply", actions.len());
//!
//!         // After execution, report the outcome
//!         engine.report_outcome(&matches[0].skill.id, true)?;
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Vision Model Integration
//!
//! ```no_run
//! use ganesha_vision::model::{VisionClient, VisionModelConfig, Screenshot};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client with LM Studio defaults
//!     let config = VisionModelConfig::lm_studio();
//!     let client = VisionClient::new(config)?;
//!
//!     // Analyze a screenshot (assuming you have one)
//!     // let analysis = client.analyze_screen(&screenshot).await?;
//!     Ok(())
//! }
//! ```

pub mod capture;
pub mod controller;
pub mod db;
pub mod error;
pub mod learning;
pub mod model;
pub mod overlay;

// Re-export error types
pub use error::{Error, Result};

// ============================================================================
// Database/Persistence exports
// ============================================================================

pub use db::{
    // Core types
    Action,
    ActionDetails,
    ActionTemplate,
    ActionType,
    Database,
    Demonstration,
    ElementType,
    MouseButton,
    Modifier,
    Outcome,
    RecordedAction,
    Session,
    Skill,
    UiElement,
    UiPattern,
};

// ============================================================================
// Learning engine exports
// ============================================================================

pub use learning::{
    // Main engine
    LearningEngine,
    LearningStatistics,

    // Recording
    RecordingSession,
    RecordingState,

    // Skill matching
    MatchConfig,
    SkillMatch,
    SkillMatcher,

    // Skill extraction
    ExtractionConfig,
    SkillExtractor,

    // Skill application
    ApplicationConfig,
    ApplicationResult,
    SkillApplicator,

    // Screenshot type for learning (distinct from model::Screenshot)
    Screenshot as LearningScreenshot,
};

// ============================================================================
// Model exports (for vision analysis)
// ============================================================================

pub use model::{
    DualModelConfig,
    ImageFormat,
    ScreenAnalysis,
    Screenshot,
    UIElement,
    VisionClient,
    VisionModelConfig,
    VisionModelError,
};

// ============================================================================
// Capture exports (for screen recording)
// ============================================================================

// Note: capture types are available via capture:: module
// Use fully qualified names to avoid confusion with learning types
pub use capture::{
    BufferStats,
    BufferedScreenshot,
    CaptureConfig,
    CaptureError,
    CaptureRegion,
    CaptureResult,
    CaptureSource,
    DefaultCapture,
    MonitorInfo,
    ScreenBuffer,
    ScreenBufferConfig,
    ScreenCapture,
    ScreenshotMetadata,
    WindowInfo,
    create_capture,
};

// ============================================================================
// Controller exports (main orchestrator)
// ============================================================================

pub use controller::{
    // Main controller
    VisionController,
    VisionControllerConfig,

    // Configuration
    ModelConfig,
    SpeedMode,

    // State and status
    ControllerState,
    ControllerStatus,
    TaskResult,

    // Safety
    SafetyChecker,
    SafetyResult,

    // Planning
    ActionPlanner,
    PlannedAction,

    // Confirmation
    ConfirmationHandler,
    ConfirmationRequest,

    // Input execution
    InputExecutor,
};

// ============================================================================
// Overlay exports (visual control indicators)
// ============================================================================

pub use overlay::{
    // Main overlay
    ControlOverlay,
    ScreenBorder,
    OverlayConfig,

    // Configuration types
    Color,
    WindowPosition,

    // State and events
    OverlayState,
    OverlayCommand,
    OverlayEvent,

    // Error handling
    OverlayError,
    OverlayResult,

    // Backend trait
    OverlayBackend,
    StubOverlayBackend,

    // Window identification (for vision system to ignore overlay)
    GANESHA_WINDOW_PREFIX,
    GANESHA_WINDOW_CLASS,
    GANESHA_APP_IDENTIFIER,
    GANESHA_WINDOW_TITLES,
    CONTROL_PANEL_TITLE,
    SCREEN_BORDER_TITLE,
    is_ganesha_window,
    is_ganesha_window_class,
    is_ganesha_app,
    filter_ganesha_windows,
};

// Platform-specific overlay backends
#[cfg(target_os = "linux")]
pub use overlay::LinuxOverlayBackend;

#[cfg(target_os = "windows")]
pub use overlay::WindowsOverlayBackend;

#[cfg(target_os = "macos")]
pub use overlay::MacOSOverlayBackend;

#[cfg(feature = "screen-capture")]
pub use capture::XcapCapture;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{
        // Error handling
        Error, Result,

        // Database
        Database,

        // Core types
        Action, ActionDetails, ActionType, Demonstration, Skill,
        RecordedAction, MouseButton, Modifier, Outcome,
        ElementType, UiElement, UiPattern,

        // Learning
        LearningEngine, LearningStatistics,
        RecordingSession, RecordingState,
        SkillMatch, SkillMatcher, MatchConfig,
        SkillExtractor, ExtractionConfig,
        SkillApplicator, ApplicationConfig, ApplicationResult,
        LearningScreenshot,

        // Vision model
        VisionClient, VisionModelConfig, Screenshot, ScreenAnalysis,

        // Capture
        ScreenCapture, CaptureConfig, DefaultCapture,

        // Controller (main orchestrator)
        VisionController, VisionControllerConfig,
        ControllerState, ControllerStatus, TaskResult,
        SpeedMode, ModelConfig,
        SafetyChecker, SafetyResult,

        // Overlay (visual control indicators)
        ControlOverlay, OverlayConfig, ScreenBorder,
        is_ganesha_window, filter_ganesha_windows,
    };
}
