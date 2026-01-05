//! Ganesha Library
//!
//! Core functionality for the Ganesha AI system control tool.
//!
//! "Vakratunda Mahakaya, Surya Koti Samaprabha"
//! (O Lord with curved trunk and mighty body,
//!  with the radiance of a million suns)
//!
//! Like the mythical Ganesha who rides upon Mushika the mouse,
//! so too does this Ganesha command the mouse, keyboard, and screen.

pub mod cli;
pub mod core;
pub mod logging;
pub mod providers;

// Computer Use modules (optional, dangerous by default)
#[cfg(any(feature = "vision", feature = "input", feature = "computer-use"))]
pub mod vision;
#[cfg(any(feature = "vision", feature = "input", feature = "computer-use"))]
pub mod input;

// Real-time voice (optional)
#[cfg(feature = "voice")]
pub mod voice;

// Reactive Agent (requires computer-use)
#[cfg(feature = "computer-use")]
pub mod agent;

// NVR-style zone filtering (requires computer-use)
#[cfg(feature = "computer-use")]
pub mod zones;

// System dossier - complete situational awareness
#[cfg(feature = "computer-use")]
pub mod dossier;

// Temporal memory - SpacetimeDB-backed activity log
#[cfg(feature = "computer-use")]
pub mod memory;

// Activity overlay - visible timer for human + AI
#[cfg(feature = "computer-use")]
pub mod overlay;

// Dynamic documentation loader (Context7, local, etc)
#[cfg(feature = "computer-use")]
pub mod docs;

// Smell Test - Ganesha's trunk detects the rotten (always available for validation)
pub mod smell;

// AI Cursor - Visual feedback when AI controls the mouse
#[cfg(feature = "computer-use")]
pub mod cursor;

// Sentinel - Independent security guardian (always compiled, can be disabled at runtime)
pub mod sentinel;

pub use core::access_control::{AccessController, AccessLevel, AccessPolicy};
pub use core::{Action, ExecutionPlan, ExecutionResult, GaneshaEngine, Session};
pub use logging::{EventId, GaneshaEvent, LogLevel, SystemLogger};
pub use providers::{Anthropic, LlmProvider, Ollama, OpenAiCompatible, ProviderChain};

// Re-export computer use when enabled
#[cfg(feature = "vision")]
pub use vision::VisionController;
#[cfg(feature = "input")]
pub use input::{InputController, GuiAutomation};
#[cfg(feature = "voice")]
pub use voice::{VoiceController, VoiceStream};

// Sentinel is always available
pub use sentinel::{Sentinel, SentinelAnalysis, Verdict, ThreatCategory, Severity};

// Reactive Agent
#[cfg(feature = "computer-use")]
pub use agent::{ReactiveAgent, AgentConfig, AgentAction, WaitCondition, ScreenState};

// Zone filtering
#[cfg(feature = "computer-use")]
pub use zones::{Zone, ZoneManager, ZoneType, detect_motion, hash_region};

// System dossier
#[cfg(feature = "computer-use")]
pub use dossier::{SystemDossier, WindowInfo, ProcessInfo, InstalledApp};

// Temporal memory
#[cfg(feature = "computer-use")]
pub use memory::{TemporalMemory, ScreenSnapshot, ActionRecord, GoalProgress};

// Activity overlay
#[cfg(feature = "computer-use")]
pub use overlay::{ActivityOverlay, OverlayPosition, OverlayState};

// Documentation loader
#[cfg(feature = "computer-use")]
pub use docs::{DocsLoader, DocsProvider, DocSnippet, Context7Provider, LocalDocsProvider};

// Smell Test - always available
pub use smell::{Trunk, SmellTest, SmellWarning, SmellCategory, quick_smell};

// AI Cursor and mouse control
#[cfg(feature = "computer-use")]
pub use cursor::{
    AiCursor, CursorStyle, X11CursorManager,
    TracerMouse, EasingType, ScrollDirection,
    SpeedMode, SpeedController,
    smooth_move, smooth_click,
};
