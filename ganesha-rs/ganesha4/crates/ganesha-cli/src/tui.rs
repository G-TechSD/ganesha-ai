//! # TUI Mode
//!
//! Terminal User Interface using ratatui.

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

/// TUI application state
struct App {
    /// Current input text
    input: String,
    /// Cursor position in input
    cursor_position: usize,
    /// Conversation history
    messages: Vec<(String, String)>, // (role, content)
    /// Scroll position
    scroll: u16,
    /// Is running
    running: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            messages: vec![
                ("system".to_string(), "Welcome to Ganesha TUI! Type /help for commands.".to_string()),
            ],
            scroll: 0,
            running: true,
        }
    }
}

impl App {
    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.cursor_position.saturating_sub(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.cursor_position.saturating_add(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        self.input.insert(self.cursor_position, new_char);
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.cursor_position != 0;
        if is_not_cursor_leftmost {
            let current_index = self.cursor_position;
            let from_left_to_current_index = current_index - 1;
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.len())
    }

    fn submit_message(&mut self) {
        if self.input.is_empty() {
            return;
        }

        let message = self.input.clone();
        self.messages.push(("user".to_string(), message.clone()));

        // Check for commands
        if message.starts_with('/') {
            match message.trim() {
                "/help" | "/h" => {
                    self.messages.push((
                        "system".to_string(),
                        "Commands: /help, /clear, /quit, /mode <code|ask|arch>".to_string(),
                    ));
                }
                "/clear" | "/c" => {
                    self.messages.clear();
                    self.messages.push((
                        "system".to_string(),
                        "Conversation cleared.".to_string(),
                    ));
                }
                "/quit" | "/q" => {
                    self.running = false;
                }
                _ => {
                    self.messages.push((
                        "system".to_string(),
                        format!("Unknown command: {}", message),
                    ));
                }
            }
        } else {
            // Simulate assistant response
            self.messages.push((
                "assistant".to_string(),
                format!("I received: \"{}\" (TUI mode - responses not yet connected)", message),
            ));
        }

        self.input.clear();
        self.cursor_position = 0;
    }
}

/// Run the TUI
pub async fn run() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::default();

    // Main loop
    while app.running {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter => app.submit_message(),
                        KeyCode::Char(c) => app.enter_char(c),
                        KeyCode::Backspace => app.delete_char(),
                        KeyCode::Left => app.move_cursor_left(),
                        KeyCode::Right => app.move_cursor_right(),
                        KeyCode::Esc => app.running = false,
                        KeyCode::Up => {
                            app.scroll = app.scroll.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            app.scroll = app.scroll.saturating_add(1);
                        }
                        _ => {}
                    }
                }
            }
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

    Ok(())
}

/// Draw the UI
fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(3),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status
        ])
        .split(f.area());

    // Messages area
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .map(|(role, content)| {
            let style = match role.as_str() {
                "user" => Style::default().fg(Color::Cyan),
                "assistant" => Style::default().fg(Color::Green),
                "system" => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            };

            let prefix = match role.as_str() {
                "user" => "You: ",
                "assistant" => "🐘 ",
                "system" => "ℹ️  ",
                _ => "",
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                Span::styled(content, style),
            ]))
        })
        .collect();

    let messages_block = List::new(messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Ganesha 🐘 "),
        )
        .style(Style::default());

    f.render_widget(messages_block, chunks[0]);

    // Input area
    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message (Enter to send, Esc to quit) "),
        );

    f.render_widget(input, chunks[1]);

    // Set cursor position
    f.set_cursor_position((
        chunks[1].x + app.cursor_position as u16 + 1,
        chunks[1].y + 1,
    ));

    // Status bar
    let status = Paragraph::new(" Mode: code | Model: default | /help for commands")
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(status, chunks[2]);
}
