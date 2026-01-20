//! # Model Quality Tiers
//!
//! Classification of models by their capability for agentic tasks.
//!
//! ## Tiers
//!
//! - 🟢 **Exceptional**: Best models for complex, multi-step tasks
//! - 🟡 **Capable**: Good for most tasks with occasional issues
//! - 🟠 **Limited**: Works for simple tasks, may struggle with complex
//! - 🔴 **Unsafe**: May produce dangerous/incorrect commands

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Model quality tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModelTier {
    /// Best for agentic tasks
    Exceptional,
    /// Good for most tasks
    Capable,
    /// Simple tasks only
    Limited,
    /// May be dangerous
    Unsafe,
    /// Unknown model
    Unknown,
}

impl ModelTier {
    /// Icon for this tier
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Exceptional => "🟢",
            Self::Capable => "🟡",
            Self::Limited => "🟠",
            Self::Unsafe => "🔴",
            Self::Unknown => "⚪",
        }
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Exceptional => "Excellent for complex, multi-step agentic tasks",
            Self::Capable => "Good for most tasks, occasional issues with complex work",
            Self::Limited => "Works for simple tasks, struggles with complexity",
            Self::Unsafe => "May produce dangerous or incorrect commands",
            Self::Unknown => "Unknown model - use with caution",
        }
    }

    /// Should we warn the user about this tier?
    pub fn should_warn(&self) -> bool {
        matches!(self, Self::Limited | Self::Unsafe | Self::Unknown)
    }
}

/// Information about a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub tier: ModelTier,
    pub context_length: Option<u32>,
    pub supports_vision: bool,
    pub supports_tools: bool,
}

/// Known model tiers
static MODEL_TIERS: LazyLock<HashMap<&'static str, ModelTier>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Exceptional tier - best for agentic tasks
    m.insert("claude-3-5-sonnet", ModelTier::Exceptional);
    m.insert("claude-sonnet-4", ModelTier::Exceptional);
    m.insert("claude-opus-4", ModelTier::Exceptional);
    m.insert("claude-3-opus", ModelTier::Exceptional);
    m.insert("gpt-4o", ModelTier::Exceptional);
    m.insert("gpt-4-turbo", ModelTier::Exceptional);
    m.insert("o1", ModelTier::Exceptional);
    m.insert("o1-preview", ModelTier::Exceptional);
    m.insert("gemini-2.0-pro", ModelTier::Exceptional);
    m.insert("gemini-1.5-pro", ModelTier::Exceptional);
    m.insert("deepseek-v3", ModelTier::Exceptional);
    m.insert("qwen-2.5-72b", ModelTier::Exceptional);
    m.insert("llama-3.1-405b", ModelTier::Exceptional);

    // Capable tier
    m.insert("gpt-4o-mini", ModelTier::Capable);
    m.insert("o3-mini", ModelTier::Capable);
    m.insert("claude-3-5-haiku", ModelTier::Capable);
    m.insert("claude-3-haiku", ModelTier::Capable);
    m.insert("llama-3.1-70b", ModelTier::Capable);
    m.insert("mistral-large", ModelTier::Capable);
    m.insert("qwen-2.5-32b", ModelTier::Capable);
    m.insert("gemini-2.0-flash", ModelTier::Capable);
    m.insert("gemini-1.5-flash", ModelTier::Capable);
    m.insert("deepseek-coder", ModelTier::Capable);

    // Limited tier
    m.insert("llama-3.1-8b", ModelTier::Limited);
    m.insert("mistral-7b", ModelTier::Limited);
    m.insert("phi-3", ModelTier::Limited);
    m.insert("qwen-2.5-7b", ModelTier::Limited);
    m.insert("gemma-2-9b", ModelTier::Limited);

    // Unsafe tier - very small or untuned models
    m.insert("phi-2", ModelTier::Unsafe);
    m.insert("tinyllama", ModelTier::Unsafe);

    m
});

/// Get the tier for a model by ID
pub fn get_model_tier(model_id: &str) -> ModelTier {
    let lower = model_id.to_lowercase();

    // Try exact match first
    if let Some(&tier) = MODEL_TIERS.get(lower.as_str()) {
        return tier;
    }

    // Try prefix match
    for (pattern, &tier) in MODEL_TIERS.iter() {
        if lower.starts_with(pattern) || lower.contains(pattern) {
            return tier;
        }
    }

    // Check for known patterns
    if lower.contains("405b") || lower.contains("claude-3-opus") {
        return ModelTier::Exceptional;
    }
    if lower.contains("70b") || lower.contains("72b") {
        return ModelTier::Capable;
    }
    if lower.contains("7b") || lower.contains("8b") {
        return ModelTier::Limited;
    }
    if lower.contains("1b") || lower.contains("2b") || lower.contains("3b") {
        return ModelTier::Unsafe;
    }

    ModelTier::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_models() {
        assert_eq!(get_model_tier("claude-3-5-sonnet-20241022"), ModelTier::Exceptional);
        assert_eq!(get_model_tier("gpt-4o"), ModelTier::Exceptional);
        assert_eq!(get_model_tier("gpt-4o-mini"), ModelTier::Capable);
        assert_eq!(get_model_tier("llama-3.1-8b-instruct"), ModelTier::Limited);
    }

    #[test]
    fn test_size_inference() {
        assert_eq!(get_model_tier("some-model-70b"), ModelTier::Capable);
        assert_eq!(get_model_tier("some-model-7b"), ModelTier::Limited);
        assert_eq!(get_model_tier("tiny-2b-model"), ModelTier::Unsafe);
    }

    #[test]
    fn test_unknown() {
        assert_eq!(get_model_tier("completely-unknown-model"), ModelTier::Unknown);
    }
}
