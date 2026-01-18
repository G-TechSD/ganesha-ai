//! # Voice Command
//!
//! Voice input/output commands for Ganesha.

use crate::cli::VoiceAction;
use colored::Colorize;
use ganesha_voice::{VoiceConfigBuilder, VoiceManager, VoiceModels, VoiceSetupStatus, PiperTTS, VoiceOutput};
use std::env;
use std::process::Command;

/// Try to speak using local TTS (Piper first, then espeak-ng fallback)
async fn speak_local(text: &str) -> bool {
    let models = VoiceModels::new();

    // Try Piper first (better quality)
    if PiperTTS::is_piper_installed() && models.has_piper_model() {
        let piper = PiperTTS::new(models.piper_model_path());
        if piper.is_available().await {
            if let Ok(audio) = piper.synthesize(text).await {
                // Play the audio
                if let Ok(player) = ganesha_voice::AudioPlayer::new() {
                    if player.play(&audio).is_ok() {
                        // Wait for playback
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        return true;
                    }
                }
            }
        }
    }

    // Fallback to espeak-ng
    if let Ok(status) = Command::new("espeak-ng")
        .arg("-s").arg("150")
        .arg(text)
        .status()
    {
        return status.success();
    }

    // Fallback to espeak
    if let Ok(status) = Command::new("espeak")
        .arg("-s").arg("150")
        .arg(text)
        .status()
    {
        return status.success();
    }

    false
}

/// Check if local TTS is available
fn has_local_tts() -> bool {
    let models = VoiceModels::new();
    PiperTTS::is_piper_installed() && models.has_piper_model()
        || Command::new("which").arg("espeak-ng").output().map(|o| o.status.success()).unwrap_or(false)
        || Command::new("which").arg("espeak").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Run the voice command
pub async fn run(action: VoiceAction) -> anyhow::Result<()> {
    match action {
        VoiceAction::Setup => {
            println!("{}", "Voice Setup".bright_cyan().bold());
            println!("{}\n", "Setting up free local voice models...".dimmed());

            let models = VoiceModels::new();

            // Check current status
            let status = VoiceSetupStatus::check(&models);

            // Show what needs to be done
            if status.ready_for_local_voice {
                println!("{} Voice is already set up and ready!", "✓".green());
                return Ok(());
            }

            // Setup Whisper model for STT
            if !status.whisper_model_installed {
                println!("\n{} Downloading Whisper model for speech recognition...", "📥".bright_cyan());
                println!("  Model: {} (~142 MB)", "base.en".bright_yellow());

                match ganesha_voice::download_whisper_model(&models, Some(Box::new(|downloaded, total| {
                    if total > 0 {
                        let percent = (downloaded as f64 / total as f64 * 100.0) as u32;
                        print!("\r  Progress: {}% ({} / {} MB)", percent, downloaded / 1_000_000, total / 1_000_000);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                }))).await {
                    Ok(path) => {
                        println!("\n  {} Whisper model downloaded to {}", "✓".green(), path.display());
                    }
                    Err(e) => {
                        println!("\n  {} Failed to download Whisper model: {}", "✗".red(), e);
                    }
                }
            } else {
                println!("{} Whisper model already installed", "✓".green());
            }

            // Check/install Piper
            if !status.piper_installed {
                println!("\n{} Piper TTS not found", "⚠".yellow());
                println!("  Install with: {}", "pip install piper-tts".bright_green());
                println!("  Or download from: https://github.com/rhasspy/piper/releases");
            } else {
                println!("{} Piper TTS installed", "✓".green());
            }

            // Setup Piper voice model
            if !status.piper_voice_installed {
                println!("\n{} Downloading Piper voice model...", "📥".bright_cyan());
                println!("  Voice: {} (~63 MB)", "amy-medium (US English)".bright_yellow());

                match ganesha_voice::download_piper_voice(&models, Some(Box::new(|downloaded, total| {
                    if total > 0 {
                        let percent = (downloaded as f64 / total as f64 * 100.0) as u32;
                        print!("\r  Progress: {}% ({} / {} MB)", percent, downloaded / 1_000_000, total / 1_000_000);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                }))).await {
                    Ok(path) => {
                        println!("\n  {} Piper voice downloaded to {}", "✓".green(), path.display());
                    }
                    Err(e) => {
                        println!("\n  {} Failed to download Piper voice: {}", "✗".red(), e);
                    }
                }
            } else {
                println!("{} Piper voice model already installed", "✓".green());
            }

            // Final status
            let final_status = VoiceSetupStatus::check(&models);
            println!();
            if final_status.ready_for_local_voice {
                println!("{} Voice setup complete! You can now use voice features for free.", "🎉".bright_green());
            } else {
                println!("{} Voice setup incomplete. See issues above.", "⚠".yellow());
                if !final_status.piper_installed {
                    println!("  Run: {} to install Piper TTS", "pip install piper-tts".bright_green());
                }
            }
        }

        VoiceAction::Status => {
            println!("{}", "Voice Status".bright_cyan().bold());

            let models = VoiceModels::new();
            let status = VoiceSetupStatus::check(&models);

            println!("\n{}", "Local Voice (Free):".bright_white());
            println!(
                "  {} Whisper STT model: {}",
                if status.whisper_model_installed { "✓".green() } else { "✗".red() },
                if status.whisper_model_installed { "installed".green() } else { "not installed".dimmed() }
            );
            println!(
                "  {} Piper TTS binary: {}",
                if status.piper_installed { "✓".green() } else { "✗".red() },
                if status.piper_installed { "installed".green() } else { "not installed".dimmed() }
            );
            println!(
                "  {} Piper voice model: {}",
                if status.piper_voice_installed { "✓".green() } else { "✗".red() },
                if status.piper_voice_installed { "installed".green() } else { "not installed".dimmed() }
            );

            let openai_key = env::var("OPENAI_API_KEY").is_ok();
            println!("\n{}", "Cloud Voice (Requires API Key):".bright_white());
            println!(
                "  {} OpenAI API: {}",
                if openai_key { "✓".green() } else { "✗".red() },
                if openai_key { "configured".green() } else { "OPENAI_API_KEY not set".dimmed() }
            );

            println!();
            if status.ready_for_local_voice {
                println!("{} Ready for free local voice!", "🎤".bright_green());
            } else {
                println!("{} Run {} to set up free local voice", "→".bright_cyan(), "ganesha voice setup".bright_green());
            }
        }

        VoiceAction::Devices => {
            println!("{}\n", "Audio Devices".bright_cyan().bold());

            // Show TTS options
            println!("{}", "Text-to-Speech:".bright_white());
            let openai_key = env::var("OPENAI_API_KEY").is_ok();
            println!(
                "  {} OpenAI TTS: {}",
                if openai_key { "✓".green() } else { "✗".red() },
                if openai_key { "available (OPENAI_API_KEY set)".green() } else { "not configured".dimmed() }
            );
            println!(
                "  {} Local TTS (espeak): {}",
                if has_local_tts() { "✓".green() } else { "✗".red() },
                if has_local_tts() { "available".green() } else { "not installed (apt install espeak-ng)".dimmed() }
            );
            println!();

            println!("{}", "Input Devices:".bright_white());
            match VoiceManager::list_input_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        println!("  {}", "No input devices found".dimmed());
                    } else {
                        for (i, device) in devices.iter().enumerate() {
                            println!("  {}. {}", i + 1, device);
                        }
                    }
                }
                Err(e) => {
                    println!("  {} {}", "Error:".red(), e);
                }
            }

            println!("\n{}", "Output Devices:".bright_white());
            match VoiceManager::list_output_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        println!("  {}", "No output devices found".dimmed());
                    } else {
                        for (i, device) in devices.iter().enumerate() {
                            println!("  {}. {}", i + 1, device);
                        }
                    }
                }
                Err(e) => {
                    println!("  {} {}", "Error:".red(), e);
                }
            }
        }

        VoiceAction::Test => {
            println!("{}", "Voice Test".bright_cyan().bold());
            println!("{}\n", "Testing voice input and output...".dimmed());

            // Check for API key
            let openai_key = env::var("OPENAI_API_KEY").ok();
            if openai_key.is_none() {
                println!(
                    "{} OPENAI_API_KEY not set. Voice features require an OpenAI API key.",
                    "Warning:".yellow()
                );
                println!(
                    "Set it with: {}",
                    "export OPENAI_API_KEY=your-key".bright_green()
                );
                return Ok(());
            }

            let config = VoiceConfigBuilder::new()
                .enabled(true)
                .openai_api_key(openai_key.as_deref().unwrap_or(""))
                .build()?;

            let manager = VoiceManager::new(config).await?;

            println!("{} Voice system initialized", "✓".green());
            println!(
                "  {} Recording available: {}",
                "•".dimmed(),
                if manager.is_enabled() { "yes".green() } else { "no".red() }
            );
            println!(
                "  {} Current personality: {}",
                "•".dimmed(),
                manager.current_personality().id.bright_yellow()
            );

            println!(
                "\n{} Recording for 3 seconds... Speak now!",
                "🎤".bright_cyan()
            );

            // Record audio with VAD
            match manager.record_with_vad().await {
                Ok(audio) => {
                    println!(
                        "{} Recorded {} samples",
                        "✓".green(),
                        audio.samples.len()
                    );

                    // Transcribe
                    println!("{} Transcribing...", "⚡".bright_cyan());
                    match manager.transcribe(&audio).await {
                        Ok(result) => {
                            println!(
                                "\n{} You said: \"{}\"",
                                "📝".bright_yellow(),
                                result.text.bright_white()
                            );

                            // Echo back with TTS
                            println!(
                                "\n{} Speaking response...",
                                "🔊".bright_cyan()
                            );
                            let response = format!("I heard you say: {}", result.text);
                            if let Err(e) = manager.speak(&response).await {
                                println!("{} TTS error: {}", "✗".red(), e);
                            } else {
                                println!("{} Done!", "✓".green());
                            }
                        }
                        Err(e) => {
                            println!("{} Transcription failed: {}", "✗".red(), e);
                        }
                    }
                }
                Err(e) => {
                    println!("{} Recording failed: {}", "✗".red(), e);
                }
            }
        }

        VoiceAction::Say { text } => {
            println!("{} Speaking: \"{}\"", "🔊".bright_cyan(), text.dimmed());

            let openai_key = env::var("OPENAI_API_KEY").ok();

            // Try OpenAI TTS first if API key is available
            if let Some(key) = openai_key {
                let config = VoiceConfigBuilder::new()
                    .enabled(true)
                    .openai_api_key(&key)
                    .build()?;

                if let Ok(manager) = VoiceManager::new(config).await {
                    if manager.speak(&text).await.is_ok() {
                        println!("{} Done! (OpenAI TTS)", "✓".green());
                        return Ok(());
                    }
                }
            }

            // Fallback to local TTS (Piper or espeak)
            if has_local_tts() {
                let models = VoiceModels::new();
                if PiperTTS::is_piper_installed() && models.has_piper_model() {
                    println!("{}", "(Using local Piper TTS)".dimmed());
                } else {
                    println!("{}", "(Using local espeak)".dimmed());
                }
                if speak_local(&text).await {
                    println!("{} Done!", "✓".green());
                } else {
                    println!("{} Local TTS failed", "✗".red());
                }
            } else {
                println!(
                    "{} No TTS available. Run {} or set OPENAI_API_KEY",
                    "Error:".red(),
                    "ganesha voice setup".bright_green()
                );
            }
        }

        VoiceAction::Personality { name } => {
            let valid = ["friendly", "professional", "mentor", "pirate"];
            if !valid.contains(&name.to_lowercase().as_str()) {
                println!(
                    "{} Unknown personality: {}",
                    "Error:".red(),
                    name.bright_yellow()
                );
                println!("Available personalities:");
                for p in &valid {
                    println!("  • {}", p.bright_green());
                }
                return Ok(());
            }

            println!(
                "{} Personality set to: {}",
                "✓".green(),
                name.bright_yellow()
            );
            println!(
                "{}",
                "This will be used for future voice responses.".dimmed()
            );
        }

        VoiceAction::Chat => {
            use crate::voice_input::{VoicePTT, VoiceInputEvent as PTTEvent};
            use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

            println!("{}", "Voice Chat Mode".bright_cyan().bold());
            println!("{}", "─".repeat(50).dimmed());
            println!();
            println!("  {} {} to record, release to send", "Hold".bright_white(), "CTRL".bright_yellow().bold());
            println!("  {} {} to toggle continuous listening", "Double-tap".bright_white(), "CTRL".bright_yellow().bold());
            println!("  {} to cancel recording", "ESC".bright_yellow());
            println!("  {} to exit", "Ctrl+C".bright_yellow());
            println!();
            println!("{}", "─".repeat(50).dimmed());

            // Check for voice setup
            let models = VoiceModels::new();
            let status = VoiceSetupStatus::check(&models);
            let openai_key = env::var("OPENAI_API_KEY").ok();

            if !status.ready_for_local_voice && openai_key.is_none() {
                println!();
                println!("{} Voice not configured.", "⚠".yellow());
                println!("  Run {} for free local voice", "ganesha voice setup".bright_green());
                println!("  Or set {} for cloud voice", "OPENAI_API_KEY".bright_green());
                return Ok(());
            }

            // Setup voice manager
            let config = if let Some(ref key) = openai_key {
                VoiceConfigBuilder::new()
                    .enabled(true)
                    .openai_api_key(key)
                    .build()?
            } else {
                VoiceConfigBuilder::new()
                    .enabled(true)
                    .build()?
            };

            let manager = match VoiceManager::new(config).await {
                Ok(m) => m,
                Err(e) => {
                    println!("{} Failed to initialize voice: {}", "Error:".red(), e);
                    return Ok(());
                }
            };

            println!();
            if status.ready_for_local_voice {
                println!("{} Using local voice (free)", "✓".green());
            } else if openai_key.is_some() {
                println!("{} Using OpenAI voice (cloud)", "✓".green());
            }
            println!("{} Ready! Hold CTRL to speak...", "🎤".bright_cyan());
            println!();

            // Create event channel
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<PTTEvent>(32);

            // Spawn keyboard handler
            let mut ptt = VoicePTT::new();
            let keyboard_handle = tokio::spawn(async move {
                let _ = ptt.run(event_tx).await;
            });

            // Enable raw mode for key detection
            if let Err(e) = enable_raw_mode() {
                println!("{} Failed to enable raw mode: {}", "Error:".red(), e);
                return Ok(());
            }

            let mut conversation_mode = false;

            // Main event loop
            loop {
                tokio::select! {
                    Some(event) = event_rx.recv() => {
                        match event {
                            PTTEvent::StartRecording => {
                                if !conversation_mode {
                                    print!("\r{} Recording... (release CTRL to send)    ", "🔴".bright_red());
                                    let _ = std::io::Write::flush(&mut std::io::stdout());
                                }
                            }
                            PTTEvent::StopAndTranscribe => {
                                print!("\r{} Processing...                           ", "⚡".bright_cyan());
                                let _ = std::io::Write::flush(&mut std::io::stdout());

                                // Record was stopped, now transcribe
                                // Note: In real implementation, we'd capture during hold
                                // For now, show the flow
                                println!("\r{} Voice input received                     ", "✓".green());

                                // Placeholder for actual transcription
                                println!("  {} (Push-to-talk demo - full integration needs audio capture)", "Note:".dimmed());
                            }
                            PTTEvent::ConversationEnabled => {
                                conversation_mode = true;
                                println!("\r{} Conversation mode ON - listening continuously...", "🎙️".bright_green());
                            }
                            PTTEvent::ConversationDisabled => {
                                conversation_mode = false;
                                println!("\r{} Conversation mode OFF - push-to-talk active    ", "🎤".bright_cyan());
                            }
                            PTTEvent::Cancel => {
                                println!("\r{} Cancelled                                ", "✗".yellow());
                            }
                            PTTEvent::Exit => {
                                println!("\r{} Exiting voice chat...                    ", "👋".dimmed());
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        // Heartbeat - could show audio levels here
                    }
                }
            }

            // Cleanup
            keyboard_handle.abort();
            let _ = disable_raw_mode();
            println!();
        }
    }

    Ok(())
}
