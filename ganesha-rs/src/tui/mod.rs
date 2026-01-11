//! Terminal User Interface for Ganesha
//!
//! Provides a rich TUI experience with:
//! - Scrolling chat history
//! - Status bar with animations
//! - Session logging

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use chrono::Local;

/// A message in the chat history
#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<Local>,
}

#[derive(Clone, PartialEq)]
pub enum MessageRole {
    User,
    Ganesha,
    System,
    Error,
}

/// Status bar state
pub struct StatusBar {
    pub message: String,
    pub spinner_idx: usize,
    pub is_busy: bool,
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl StatusBar {
    pub fn new() -> Self {
        Self {
            message: "Ready".into(),
            spinner_idx: 0,
            is_busy: false,
        }
    }

    pub fn tick(&mut self) {
        if self.is_busy {
            self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
        }
    }

    pub fn set_busy(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.is_busy = true;
    }

    pub fn set_ready(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.is_busy = false;
    }

    pub fn render(&self) -> String {
        if self.is_busy {
            format!("{} {}", SPINNER_FRAMES[self.spinner_idx], self.message)
        } else {
            format!("● {}", self.message)
        }
    }
}

/// TUI Application state
pub struct TuiApp {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub input_cursor: usize,
    pub scroll_offset: u16,
    pub status: StatusBar,
    pub session_log: Vec<String>,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            messages: vec![],
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            status: StatusBar::new(),
            session_log: vec![format!("Session started: {}", Local::now().format("%Y-%m-%d %H:%M:%S"))],
            should_quit: false,
        }
    }

    /// Add a message to the chat
    pub fn add_message(&mut self, role: MessageRole, content: &str) {
        let msg = ChatMessage {
            role: role.clone(),
            content: content.to_string(),
            timestamp: Local::now(),
        };

        // Log to session transcript
        let role_str = match role {
            MessageRole::User => "USER",
            MessageRole::Ganesha => "GANESHA",
            MessageRole::System => "SYSTEM",
            MessageRole::Error => "ERROR",
        };
        self.session_log.push(format!("[{}] {}: {}",
            msg.timestamp.format("%H:%M:%S"),
            role_str,
            content
        ));

        self.messages.push(msg);
        // Auto-scroll to bottom
        self.scroll_to_bottom();
    }

    fn scroll_to_bottom(&mut self) {
        // Will be calculated during render
        self.scroll_offset = u16::MAX;
    }

    /// Save session log to file
    pub fn save_log(&self, path: Option<&str>) -> io::Result<String> {
        let filename = path.map(|p| p.to_string()).unwrap_or_else(|| {
            format!("ganesha-session-{}.log", Local::now().format("%Y%m%d-%H%M%S"))
        });

        let mut file = std::fs::File::create(&filename)?;
        for line in &self.session_log {
            writeln!(file, "{}", line)?;
        }
        Ok(filename)
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<String> {
        match code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    self.input.clear();
                    self.input_cursor = 0;
                    return Some(input);
                }
            }
            KeyCode::Char(c) => {
                if modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                    self.should_quit = true;
                } else {
                    self.input.insert(self.input_cursor, c);
                    self.input_cursor += 1;
                }
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.input_cursor = 0;
            }
            KeyCode::End => {
                self.input_cursor = self.input.len();
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
        None
    }
}

/// Render the TUI
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),      // Chat area (flexible)
            Constraint::Length(3),   // Input area
            Constraint::Length(1),   // Status bar
        ])
        .split(frame.area());

    render_chat(frame, app, chunks[0]);
    render_input(frame, app, chunks[1]);
    render_status(frame, app, chunks[2]);
}

fn render_chat(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    let mut lines: Vec<Line> = vec![];

    for msg in &app.messages {
        let (prefix, style) = match msg.role {
            MessageRole::User => ("▶ You: ", Style::default().fg(Color::Cyan)),
            MessageRole::Ganesha => ("◆ Ganesha: ", Style::default().fg(Color::Yellow)),
            MessageRole::System => ("● System: ", Style::default().fg(Color::Gray)),
            MessageRole::Error => ("✗ Error: ", Style::default().fg(Color::Red)),
        };

        // Add timestamp and role prefix
        lines.push(Line::from(vec![
            Span::styled(
                msg.timestamp.format("%H:%M ").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
        ]));

        // Add message content (wrapped)
        for line in msg.content.lines() {
            lines.push(Line::from(Span::styled(
                format!("    {}", line),
                style,
            )));
        }
        lines.push(Line::from("")); // spacing
    }

    // Calculate scroll
    let content_height = lines.len() as u16;
    let visible_height = area.height.saturating_sub(2); // Account for border

    if app.scroll_offset == u16::MAX {
        // Auto-scroll to bottom
        app.scroll_offset = content_height.saturating_sub(visible_height);
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" Chat ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(" ganesha> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));

    frame.render_widget(input, area);

    // Set cursor position
    frame.set_cursor_position((
        area.x + app.input_cursor as u16 + 1,
        area.y + 1,
    ));
}

fn render_status(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let status_text = app.status.render();
    let color = if app.status.is_busy { Color::Yellow } else { Color::Green };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(status_text, Style::default().fg(color)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Ctrl+C: quit", Style::default().fg(Color::DarkGray)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("PgUp/PgDn: scroll", Style::default().fg(Color::DarkGray)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("/log: save session", Style::default().fg(Color::DarkGray)),
    ]));

    frame.render_widget(status, area);
}

/// Callback type for processing input
pub type ProcessInputFn = Box<dyn FnMut(String, &mut TuiApp) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>;

/// Run the TUI application
pub async fn run_tui(mut process_input: ProcessInputFn) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new();
    app.add_message(MessageRole::System, "Welcome to Ganesha - The Remover of Obstacles");
    app.add_message(MessageRole::System, "Type your request or /help for commands");

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| render(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let Some(input) = app.handle_key(key.code, key.modifiers) {
                    // Handle special commands
                    let input_lower = input.to_lowercase();

                    if input_lower == "exit" || input_lower == "quit" || input_lower == "q" {
                        app.should_quit = true;
                    } else if input_lower == "/help" || input_lower == "help" {
                        app.add_message(MessageRole::System,
                            "Commands:\n  /1: <task> - Fast tier\n  /2: <task> - Balanced tier\n  /3: <task> - Premium tier\n  /log [file] - Save session\n  /config - Provider settings\n  exit - Quit Ganesha");
                    } else if input_lower.starts_with("/log") {
                        let path = input.strip_prefix("/log").map(|s| s.trim()).filter(|s| !s.is_empty());
                        match app.save_log(path) {
                            Ok(filename) => app.add_message(MessageRole::System, &format!("Session saved to: {}", filename)),
                            Err(e) => app.add_message(MessageRole::Error, &format!("Failed to save log: {}", e)),
                        }
                    } else {
                        // Add user message and process
                        app.add_message(MessageRole::User, &input);
                        app.status.set_busy("Processing...");

                        // Process the input
                        process_input(input, &mut app).await;

                        app.status.set_ready("Ready");
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.status.tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("Namaste");
    Ok(())
}
