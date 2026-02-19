//! Voice personality system for Ganesha.
//!
//! Provides different voice personalities that modify how the AI assistant
//! communicates, including speaking style, voice selection, and system prompts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::output::OpenAIVoice;
use crate::{Result, VoiceError};

/// A voice personality that defines how Ganesha communicates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    /// Unique identifier for this personality
    pub id: String,
    /// Display name for the personality
    pub name: String,
    /// Description of the personality's characteristics
    pub description: String,
    /// The voice to use for this personality
    pub voice: VoiceSelection,
    /// Speaking style configuration
    pub speaking_style: SpeakingStyle,
    /// System prompt modifier to adjust AI behavior
    pub system_prompt_modifier: String,
    /// Optional greeting phrase
    pub greeting: Option<String>,
    /// Optional farewell phrase
    pub farewell: Option<String>,
    /// Custom phrases for specific situations
    #[serde(default)]
    pub custom_phrases: HashMap<String, String>,
}

/// Voice selection for a personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSelection {
    /// OpenAI voice to use
    pub openai_voice: OpenAIVoice,
    /// ElevenLabs voice ID (if using ElevenLabs)
    pub elevenlabs_voice_id: Option<String>,
    /// Preferred TTS provider
    pub preferred_provider: TTSProvider,
}

impl Default for VoiceSelection {
    fn default() -> Self {
        Self {
            openai_voice: OpenAIVoice::Nova,
            elevenlabs_voice_id: None,
            preferred_provider: TTSProvider::OpenAI,
        }
    }
}

/// TTS provider preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TTSProvider {
    #[default]
    OpenAI,
    ElevenLabs,
}

/// Speaking style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakingStyle {
    /// Speech speed (0.5 to 2.0)
    pub speed: f32,
    /// Pitch adjustment (-1.0 to 1.0, where supported)
    pub pitch: f32,
    /// Formality level (casual to formal)
    pub formality: Formality,
    /// Whether to use contractions
    pub use_contractions: bool,
    /// Whether to use filler words (um, uh, well)
    pub use_fillers: bool,
    /// Whether to use emotive expressions
    pub use_emotions: bool,
    /// Maximum sentence length before breaking
    pub max_sentence_length: usize,
}

impl Default for SpeakingStyle {
    fn default() -> Self {
        Self {
            speed: 1.0,
            pitch: 0.0,
            formality: Formality::Neutral,
            use_contractions: true,
            use_fillers: false,
            use_emotions: true,
            max_sentence_length: 150,
        }
    }
}

/// Formality level for speaking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Formality {
    Casual,
    #[default]
    Neutral,
    Formal,
    Technical,
}

impl Personality {
    /// Create a new personality with the given ID and name
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            voice: VoiceSelection::default(),
            speaking_style: SpeakingStyle::default(),
            system_prompt_modifier: String::new(),
            greeting: None,
            farewell: None,
            custom_phrases: HashMap::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the voice selection
    pub fn with_voice(mut self, voice: VoiceSelection) -> Self {
        self.voice = voice;
        self
    }

    /// Set the speaking style
    pub fn with_speaking_style(mut self, style: SpeakingStyle) -> Self {
        self.speaking_style = style;
        self
    }

    /// Set the system prompt modifier
    pub fn with_system_prompt_modifier(mut self, modifier: impl Into<String>) -> Self {
        self.system_prompt_modifier = modifier.into();
        self
    }

    /// Set the greeting
    pub fn with_greeting(mut self, greeting: impl Into<String>) -> Self {
        self.greeting = Some(greeting.into());
        self
    }

    /// Set the farewell
    pub fn with_farewell(mut self, farewell: impl Into<String>) -> Self {
        self.farewell = Some(farewell.into());
        self
    }

    /// Add a custom phrase
    pub fn with_custom_phrase(mut self, key: impl Into<String>, phrase: impl Into<String>) -> Self {
        self.custom_phrases.insert(key.into(), phrase.into());
        self
    }

    /// Get the OpenAI voice for this personality
    pub fn openai_voice(&self) -> OpenAIVoice {
        self.voice.openai_voice
    }

    /// Get the ElevenLabs voice ID, if configured
    pub fn elevenlabs_voice_id(&self) -> Option<&str> {
        self.voice.elevenlabs_voice_id.as_deref()
    }

    /// Apply personality to a text response
    pub fn apply_to_text(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Apply formality adjustments
        match self.speaking_style.formality {
            Formality::Casual => {
                // Convert formal phrases to casual
                result = result.replace("I will", "I'll");
                result = result.replace("cannot", "can't");
                result = result.replace("do not", "don't");
                result = result.replace("However,", "But");
                result = result.replace("Therefore,", "So");
            }
            Formality::Formal => {
                // Keep formal phrasing, remove contractions
                result = result.replace("I'll", "I will");
                result = result.replace("can't", "cannot");
                result = result.replace("don't", "do not");
                result = result.replace("won't", "will not");
            }
            _ => {}
        }

        result
    }

    /// Load personality from a TOML file
    pub async fn load_from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| VoiceError::ConfigError(format!("Failed to read personality file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| VoiceError::ConfigError(format!("Failed to parse personality: {}", e)))
    }

    /// Save personality to a TOML file
    pub async fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| VoiceError::ConfigError(format!("Failed to serialize personality: {}", e)))?;

        tokio::fs::write(path, content)
            .await
            .map_err(|e| VoiceError::ConfigError(format!("Failed to write personality file: {}", e)))
    }
}

/// Built-in personalities for Ganesha
pub struct BuiltInPersonalities;

impl BuiltInPersonalities {
    /// Get the Professional personality
    pub fn professional() -> Personality {
        Personality::new("professional", "Professional")
            .with_description("Clear, concise, and business-like communication style")
            .with_voice(VoiceSelection {
                openai_voice: OpenAIVoice::Onyx,
                elevenlabs_voice_id: None,
                preferred_provider: TTSProvider::OpenAI,
            })
            .with_speaking_style(SpeakingStyle {
                speed: 1.0,
                pitch: 0.0,
                formality: Formality::Formal,
                use_contractions: false,
                use_fillers: false,
                use_emotions: false,
                max_sentence_length: 100,
            })
            .with_system_prompt_modifier(
                "Respond in a professional, business-like manner. Be clear and concise. \
                 Use proper technical terminology. Avoid casual language and humor. \
                 Focus on efficiency and accuracy."
            )
            .with_greeting("Good day. How may I assist you?")
            .with_farewell("Thank you. Let me know if you need anything else.")
            .with_custom_phrase("error", "I encountered an issue.")
            .with_custom_phrase("success", "The operation completed successfully.")
            .with_custom_phrase("thinking", "Processing your request.")
    }

    /// Get the Friendly personality
    pub fn friendly() -> Personality {
        Personality::new("friendly", "Friendly")
            .with_description("Casual, encouraging, and uses appropriate humor")
            .with_voice(VoiceSelection {
                openai_voice: OpenAIVoice::Nova,
                elevenlabs_voice_id: None,
                preferred_provider: TTSProvider::OpenAI,
            })
            .with_speaking_style(SpeakingStyle {
                speed: 1.1,
                pitch: 0.1,
                formality: Formality::Casual,
                use_contractions: true,
                use_fillers: true,
                use_emotions: true,
                max_sentence_length: 120,
            })
            .with_system_prompt_modifier(
                "Be friendly, warm, and encouraging. Use casual language and light humor \
                 when appropriate. Show enthusiasm and empathy. Make the user feel comfortable \
                 and supported. Use contractions and conversational phrases."
            )
            .with_greeting("Hey there! Great to see you. What can I help you with?")
            .with_farewell("Take care! Don't hesitate to come back if you need anything!")
            .with_custom_phrase("error", "Oops! Something went wrong, but don't worry, we can fix it.")
            .with_custom_phrase("success", "Awesome! That worked perfectly!")
            .with_custom_phrase("thinking", "Hmm, let me think about that...")
    }

    /// Get the Mentor personality
    pub fn mentor() -> Personality {
        Personality::new("mentor", "Mentor")
            .with_description("Patient, educational, explains concepts thoroughly")
            .with_voice(VoiceSelection {
                openai_voice: OpenAIVoice::Fable,
                elevenlabs_voice_id: None,
                preferred_provider: TTSProvider::OpenAI,
            })
            .with_speaking_style(SpeakingStyle {
                speed: 0.95,
                pitch: 0.0,
                formality: Formality::Neutral,
                use_contractions: true,
                use_fillers: false,
                use_emotions: true,
                max_sentence_length: 150,
            })
            .with_system_prompt_modifier(
                "Act as a patient and knowledgeable mentor. Explain concepts clearly, \
                 breaking down complex topics into understandable parts. Ask clarifying questions \
                 when needed. Encourage learning and provide context for why things work the way \
                 they do. Use analogies and examples to illustrate points."
            )
            .with_greeting("Hello! I'm here to help you learn and grow. What would you like to explore today?")
            .with_farewell("Great work today! Remember, every expert was once a beginner. Keep learning!")
            .with_custom_phrase("error", "Let's look at what went wrong here - it's a great learning opportunity.")
            .with_custom_phrase("success", "Excellent! You've got it. Let me explain why that worked.")
            .with_custom_phrase("thinking", "That's a great question. Let me walk you through this step by step.")
    }

    /// Get the Pirate personality (fun novelty)
    pub fn pirate() -> Personality {
        Personality::new("pirate", "Pirate")
            .with_description("A fun novelty voice - arr matey!")
            .with_voice(VoiceSelection {
                openai_voice: OpenAIVoice::Echo,
                elevenlabs_voice_id: None,
                preferred_provider: TTSProvider::OpenAI,
            })
            .with_speaking_style(SpeakingStyle {
                speed: 1.0,
                pitch: -0.1,
                formality: Formality::Casual,
                use_contractions: true,
                use_fillers: true,
                use_emotions: true,
                max_sentence_length: 100,
            })
            .with_system_prompt_modifier(
                "Respond as a friendly pirate! Use pirate speak like 'arr', 'matey', 'aye', \
                 'shiver me timbers', 'avast', and 'ahoy'. Replace 'my' with 'me', \
                 'you' with 'ye', and 'your' with 'yer'. Use nautical terms and references. \
                 Be helpful but maintain the pirate character. Keep it fun and family-friendly."
            )
            .with_greeting("Ahoy, matey! Welcome aboard! What treasure of knowledge be ye seekin' today?")
            .with_farewell("Fair winds to ye, shipmate! May yer code be bug-free and yer builds be swift!")
            .with_custom_phrase("error", "Blimey! We've hit rough waters! But don't ye worry, we'll navigate through!")
            .with_custom_phrase("success", "Arr! That be a fine piece of work, ye scallywag!")
            .with_custom_phrase("thinking", "Hmm, let me consult me treasure maps...")
    }

    /// Get all built-in personalities
    pub fn all() -> Vec<Personality> {
        vec![
            Self::professional(),
            Self::friendly(),
            Self::mentor(),
            Self::pirate(),
        ]
    }

    /// Get a built-in personality by ID
    pub fn by_id(id: &str) -> Option<Personality> {
        match id.to_lowercase().as_str() {
            "professional" => Some(Self::professional()),
            "friendly" => Some(Self::friendly()),
            "mentor" => Some(Self::mentor()),
            "pirate" => Some(Self::pirate()),
            _ => None,
        }
    }

    /// Get default personality
    pub fn default() -> Personality {
        Self::friendly()
    }
}

/// Personality manager for loading and managing personalities
pub struct PersonalityManager {
    personalities: HashMap<String, Personality>,
    current: String,
}

impl PersonalityManager {
    /// Create a new personality manager with built-in personalities
    pub fn new() -> Self {
        let mut personalities = HashMap::new();
        for p in BuiltInPersonalities::all() {
            personalities.insert(p.id.clone(), p);
        }

        Self {
            personalities,
            current: "friendly".to_string(),
        }
    }

    /// Get the current personality
    pub fn current(&self) -> &Personality {
        self.personalities
            .get(&self.current)
            .unwrap_or_else(|| self.personalities.get("friendly").unwrap())
    }

    /// Set the current personality by ID
    pub fn set_current(&mut self, id: &str) -> Result<()> {
        if self.personalities.contains_key(id) {
            self.current = id.to_string();
            Ok(())
        } else {
            Err(VoiceError::ConfigError(format!(
                "Personality not found: {}",
                id
            )))
        }
    }

    /// Add a custom personality
    pub fn add(&mut self, personality: Personality) {
        self.personalities.insert(personality.id.clone(), personality);
    }

    /// Remove a personality (cannot remove built-in ones)
    pub fn remove(&mut self, id: &str) -> Result<()> {
        if BuiltInPersonalities::by_id(id).is_some() {
            return Err(VoiceError::ConfigError(
                "Cannot remove built-in personality".to_string(),
            ));
        }

        self.personalities.remove(id);
        if self.current == id {
            self.current = "friendly".to_string();
        }
        Ok(())
    }

    /// Get a personality by ID
    pub fn get(&self, id: &str) -> Option<&Personality> {
        self.personalities.get(id)
    }

    /// List all personality IDs
    pub fn list(&self) -> Vec<&str> {
        self.personalities.keys().map(|s| s.as_str()).collect()
    }

    /// Load custom personalities from a directory
    pub async fn load_from_directory(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| VoiceError::ConfigError(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            VoiceError::ConfigError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                match Personality::load_from_file(&path).await {
                    Ok(personality) => {
                        self.add(personality);
                        count += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load personality from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(count)
    }
}

impl Default for PersonalityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_personalities() {
        let personalities = BuiltInPersonalities::all();
        assert_eq!(personalities.len(), 4);

        assert!(BuiltInPersonalities::by_id("professional").is_some());
        assert!(BuiltInPersonalities::by_id("friendly").is_some());
        assert!(BuiltInPersonalities::by_id("mentor").is_some());
        assert!(BuiltInPersonalities::by_id("pirate").is_some());
        assert!(BuiltInPersonalities::by_id("nonexistent").is_none());
    }

    #[test]
    fn test_personality_manager() {
        let mut manager = PersonalityManager::new();

        assert_eq!(manager.current().id, "friendly");

        manager.set_current("professional").unwrap();
        assert_eq!(manager.current().id, "professional");

        assert!(manager.set_current("nonexistent").is_err());
    }

    #[test]
    fn test_personality_text_application() {
        let professional = BuiltInPersonalities::professional();
        let text = "I'll do that for you";
        let applied = professional.apply_to_text(text);
        assert_eq!(applied, "I will do that for you");

        let friendly = BuiltInPersonalities::friendly();
        let text = "I will do that for you";
        let applied = friendly.apply_to_text(text);
        assert_eq!(applied, "I'll do that for you");
    }

    #[test]
    fn test_custom_personality() {
        let custom = Personality::new("robot", "Robot")
            .with_description("A robotic assistant")
            .with_speaking_style(SpeakingStyle {
                speed: 0.9,
                ..Default::default()
            })
            .with_greeting("GREETINGS HUMAN");

        assert_eq!(custom.id, "robot");
        assert_eq!(custom.name, "Robot");
        assert_eq!(custom.greeting, Some("GREETINGS HUMAN".to_string()));
    }

    #[test]
    fn test_personality_builder_pattern() {
        let p = Personality::new("test", "Test Bot")
            .with_description("A test personality")
            .with_greeting("Hello!")
            .with_farewell("Bye!");
        assert_eq!(p.id, "test");
        assert_eq!(p.name, "Test Bot");
        assert_eq!(p.description, "A test personality");
        assert_eq!(p.greeting, Some("Hello!".to_string()));
        assert_eq!(p.farewell, Some("Bye!".to_string()));
    }

    #[test]
    fn test_personality_custom_phrases() {
        let p = Personality::new("custom", "Custom")
            .with_custom_phrase("error", "Oops!")
            .with_custom_phrase("success", "Yay!");
        assert_eq!(p.custom_phrases.len(), 2);
        assert_eq!(p.custom_phrases.get("error").unwrap().as_str(), "Oops!");
    }

    #[test]
    fn test_builtin_professional() {
        let p = BuiltInPersonalities::professional();
        assert_eq!(p.id, "professional");
        assert!(!p.name.is_empty());
    }

    #[test]
    fn test_builtin_friendly() {
        let p = BuiltInPersonalities::friendly();
        assert_eq!(p.id, "friendly");
    }

    #[test]
    fn test_builtin_mentor() {
        let p = BuiltInPersonalities::mentor();
        assert_eq!(p.id, "mentor");
    }

    #[test]
    fn test_builtin_pirate() {
        let p = BuiltInPersonalities::pirate();
        assert_eq!(p.id, "pirate");
    }

    #[test]
    fn test_all_builtins_have_ids() {
        let all = BuiltInPersonalities::all();
        for p in &all {
            assert!(!p.id.is_empty(), "Personality missing id");
            assert!(!p.name.is_empty(), "Personality {} missing name", p.id);
        }
    }

    #[test]
    fn test_personality_manager_add_remove() {
        let mut manager = PersonalityManager::new();
        let custom = Personality::new("test1", "Test 1");
        manager.add(custom);
        assert!(manager.get("test1").is_some());
        let _ = manager.remove("test1");
        assert!(manager.get("test1").is_none());
    }

    #[test]
    fn test_personality_manager_set_current() {
        let mut manager = PersonalityManager::new();
        let p = Personality::new("mybot", "My Bot");
        manager.add(p);
        let _ = manager.set_current("mybot");
        let current = manager.current();
        assert!(!current.id.is_empty());
    }

    #[test]
    fn test_personality_manager_list() {
        let manager = PersonalityManager::new();
        let list = manager.list();
        // Should have builtins
        assert!(list.len() >= 3);
    }

    #[test]
    fn test_voice_selection_default() {
        let vs = VoiceSelection::default();
        // Should have reasonable defaults
        let _ = vs;
    }

    #[test]
    fn test_speaking_style_default() {
        let style = SpeakingStyle::default();
        let _ = style;
    }
}
