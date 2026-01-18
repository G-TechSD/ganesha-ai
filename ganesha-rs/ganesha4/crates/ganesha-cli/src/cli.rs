//! # CLI Arguments
//!
//! Command-line argument definitions using clap.

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// Ganesha - AI coding assistant
#[derive(Parser, Debug)]
#[command(name = "ganesha")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Model to use (overrides config)
    #[arg(short, long, env = "GANESHA_MODEL")]
    pub model: Option<String>,

    /// Provider to use (overrides config)
    #[arg(short, long, env = "GANESHA_PROVIDER")]
    pub provider: Option<String>,

    /// Risk level: safe, normal, trusted, yolo
    #[arg(short, long, default_value = "normal")]
    pub risk: RiskLevel,

    /// Start in TUI mode
    #[arg(long)]
    pub tui: bool,

    /// Enable voice mode (speak responses, accept voice input)
    #[arg(long)]
    pub voice: bool,

    /// Start in a specific chat mode
    #[arg(long, default_value = "code")]
    pub mode: ChatMode,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Suppress warnings
    #[arg(short, long)]
    pub quiet: bool,

    /// Allow all apps for vision (override blacklist)
    #[arg(short = 'A', long)]
    pub allow_all_vision: bool,

    /// Working directory
    #[arg(short = 'C', long)]
    pub directory: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Send a single message and exit
    Chat {
        /// Message to send
        message: String,
        /// Model to use for this request
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Initialize Ganesha in a directory
    Init {
        /// Force re-initialization
        #[arg(short, long)]
        force: bool,
    },

    /// View or set configuration
    Config {
        /// Configuration key
        key: Option<String>,
        /// Value to set
        value: Option<String>,
    },

    /// Manage MCP servers
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// List available models
    Models {
        /// Filter by provider
        #[arg(short, long)]
        provider: Option<String>,
    },

    /// Manage sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Start TUI mode
    Tui,

    /// Voice commands
    Voice {
        #[command(subcommand)]
        action: VoiceAction,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// List configured MCP servers
    List,
    /// Add an MCP server
    Add {
        /// Server name/ID
        name: String,
        /// Server command or URL
        source: String,
    },
    /// Remove an MCP server
    Remove {
        /// Server name/ID
        name: String,
    },
    /// Connect to an MCP server
    Connect {
        /// Server name/ID
        name: String,
    },
    /// Disconnect from an MCP server
    Disconnect {
        /// Server name/ID
        name: String,
    },
    /// Show available tools
    Tools,
    /// Install a known MCP server
    Install {
        /// Server ID from registry
        server_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum VoiceAction {
    /// Setup local voice (download free models)
    Setup,
    /// Check voice status
    Status,
    /// Test voice input (record and transcribe)
    Test,
    /// List audio devices
    Devices,
    /// Speak some text
    Say {
        /// Text to speak
        text: String,
    },
    /// Set personality
    Personality {
        /// Personality name (friendly, professional, mentor, pirate)
        name: String,
    },
    /// Start voice chat mode
    Chat,
}

#[derive(Subcommand, Debug)]
pub enum SessionAction {
    /// List saved sessions
    List,
    /// Resume a session
    Resume {
        /// Session ID or name
        session: String,
    },
    /// Save current session
    Save {
        /// Session name
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Delete a session
    Delete {
        /// Session ID
        session: String,
    },
    /// Export session to file
    Export {
        /// Session ID
        session: String,
        /// Output file
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Risk level for operations
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum RiskLevel {
    /// Maximum safety - ask before everything
    Safe,
    /// Normal mode - ask for dangerous operations
    #[default]
    Normal,
    /// Trusted mode - auto-approve most operations
    Trusted,
    /// YOLO mode - approve everything automatically
    Yolo,
}

impl From<RiskLevel> for ganesha_core::RiskLevel {
    fn from(level: RiskLevel) -> Self {
        match level {
            RiskLevel::Safe => ganesha_core::RiskLevel::Safe,
            RiskLevel::Normal => ganesha_core::RiskLevel::Normal,
            RiskLevel::Trusted => ganesha_core::RiskLevel::Trusted,
            RiskLevel::Yolo => ganesha_core::RiskLevel::Yolo,
        }
    }
}

/// Chat mode
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ChatMode {
    /// Edit code directly
    #[default]
    Code,
    /// Ask questions without editing
    Ask,
    /// Plan then edit (architect + editor)
    Architect,
    /// Get help about Ganesha
    Help,
}

/// Generate shell completions
pub fn generate_completions(shell: Shell) {
    use clap::CommandFactory;
    use clap_complete::generate;
    use std::io;

    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "ganesha", &mut io::stdout());
}
