//! # Custom Widgets
//!
//! Reusable TUI widgets for Ganesha.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

use super::app::{SpinnerState, ProgressState, ThemeColors, InputMode};

// ============================================================================
// Spinner Widget
// ============================================================================

/// A spinner widget for showing async operations
pub struct Spinner<'a> {
    state: &'a SpinnerState,
    style: Style,
}

impl<'a> Spinner<'a> {
    pub fn new(state: &'a SpinnerState) -> Self {
        Self {
            state,
            style: Style::default().fg(Color::Cyan),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a> Widget for Spinner<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        let elapsed = self.state.elapsed();
        let elapsed_str = format!("{:.1}s", elapsed.as_secs_f64());

        let text = format!(
            "{} {} ({})",
            self.state.current_frame(),
            self.state.message,
            elapsed_str
        );

        let span = Span::styled(text, self.style);
        buf.set_span(area.x, area.y, &span, area.width);
    }
}

// ============================================================================
// Progress Bar Widget
// ============================================================================

/// A progress bar widget
pub struct ProgressBar<'a> {
    state: &'a ProgressState,
    style: Style,
    filled_style: Style,
    unfilled_style: Style,
}

impl<'a> ProgressBar<'a> {
    pub fn new(state: &'a ProgressState) -> Self {
        Self {
            state,
            style: Style::default(),
            filled_style: Style::default().fg(Color::Green),
            unfilled_style: Style::default().fg(Color::DarkGray),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn filled_style(mut self, style: Style) -> Self {
        self.filled_style = style;
        self
    }

    pub fn unfilled_style(mut self, style: Style) -> Self {
        self.unfilled_style = style;
        self
    }
}

impl<'a> Widget for ProgressBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }

        // Calculate bar dimensions
        let percent = self.state.percent();
        let percent_text = format!(" {:>3.0}% ", percent);
        let percent_width = percent_text.width() as u16;

        let bar_width = area.width.saturating_sub(percent_width + 2); // 2 for brackets
        let filled_width = ((bar_width as f64 * percent / 100.0).round() as u16).min(bar_width);
        let unfilled_width = bar_width - filled_width;

        // Render the bar
        let mut x = area.x;

        // Opening bracket
        buf.set_string(x, area.y, "[", self.style);
        x += 1;

        // Filled portion
        let filled_char = "";
        for _ in 0..filled_width {
            buf.set_string(x, area.y, filled_char, self.filled_style);
            x += 1;
        }

        // Unfilled portion
        let unfilled_char = "";
        for _ in 0..unfilled_width {
            buf.set_string(x, area.y, unfilled_char, self.unfilled_style);
            x += 1;
        }

        // Closing bracket
        buf.set_string(x, area.y, "]", self.style);
        x += 1;

        // Percentage
        buf.set_string(x, area.y, &percent_text, self.style);
    }
}

// ============================================================================
// Mode Indicator Widget
// ============================================================================

/// Mode indicator widget showing current input mode
pub struct ModeIndicator {
    mode: InputMode,
}

impl ModeIndicator {
    pub fn new(mode: InputMode) -> Self {
        Self { mode }
    }
}

impl Widget for ModeIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 8 || area.height < 1 {
            return;
        }

        let text = format!(" {} ", self.mode.as_str());
        let style = Style::default()
            .fg(Color::Black)
            .bg(self.mode.color())
            .add_modifier(Modifier::BOLD);

        buf.set_string(area.x, area.y, &text, style);
    }
}

// ============================================================================
// Risk Level Indicator
// ============================================================================

/// Risk level indicator with color coding
pub struct RiskIndicator {
    level: ganesha_core::RiskLevel,
}

impl RiskIndicator {
    pub fn new(level: ganesha_core::RiskLevel) -> Self {
        Self { level }
    }
}

impl Widget for RiskIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 8 || area.height < 1 {
            return;
        }

        let (icon, color) = match self.level {
            ganesha_core::RiskLevel::Safe => ("", Color::Green),
            ganesha_core::RiskLevel::Normal => ("", Color::Yellow),
            ganesha_core::RiskLevel::Trusted => ("", Color::Rgb(255, 165, 0)),
            ganesha_core::RiskLevel::Yolo => ("", Color::Red),
        };

        let text = format!(" {} {} ", icon, self.level);
        let style = Style::default().fg(color);

        buf.set_string(area.x, area.y, &text, style);
    }
}

// ============================================================================
// Token Counter Widget
// ============================================================================

/// Token counter display
pub struct TokenCounter {
    session_tokens: u64,
    total_tokens: u64,
}

impl TokenCounter {
    pub fn new(session_tokens: u64, total_tokens: u64) -> Self {
        Self {
            session_tokens,
            total_tokens,
        }
    }

    fn format_tokens(n: u64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }
}

impl Widget for TokenCounter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }

        let session = Self::format_tokens(self.session_tokens);
        let total = Self::format_tokens(self.total_tokens);
        let text = format!(" {} / {} tokens ", session, total);

        let style = Style::default().fg(Color::Cyan);
        buf.set_string(area.x, area.y, &text, style);
    }
}

// ============================================================================
// Scrollbar Widget
// ============================================================================

/// A vertical scrollbar widget
pub struct Scrollbar {
    position: usize,
    content_length: usize,
    viewport_height: usize,
    style: Style,
    thumb_style: Style,
}

impl Scrollbar {
    pub fn new(position: usize, content_length: usize, viewport_height: usize) -> Self {
        Self {
            position,
            content_length,
            viewport_height,
            style: Style::default().fg(Color::DarkGray),
            thumb_style: Style::default().fg(Color::Gray),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn thumb_style(mut self, style: Style) -> Self {
        self.thumb_style = style;
        self
    }
}

impl Widget for Scrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || self.content_length <= self.viewport_height {
            return;
        }

        let track_height = area.height as usize;

        // Calculate thumb size and position
        let thumb_height = ((self.viewport_height as f64 / self.content_length as f64)
            * track_height as f64)
            .max(1.0)
            .min(track_height as f64) as usize;

        let max_scroll = self.content_length.saturating_sub(self.viewport_height);
        let thumb_pos = if max_scroll > 0 {
            ((self.position as f64 / max_scroll as f64)
                * (track_height - thumb_height) as f64) as usize
        } else {
            0
        };

        // Render track and thumb
        for y in 0..track_height {
            let char = if y >= thumb_pos && y < thumb_pos + thumb_height {
                ""
            } else {
                ""
            };

            let style = if y >= thumb_pos && y < thumb_pos + thumb_height {
                self.thumb_style
            } else {
                self.style
            };

            buf.set_string(area.x, area.y + y as u16, char, style);
        }
    }
}

// ============================================================================
// Markdown Rendering Helpers
// ============================================================================

/// Parse markdown content into styled spans for TUI rendering
pub fn parse_markdown(content: &str, colors: &ThemeColors) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End code block
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}", "".repeat(40)),
                        Style::default().fg(colors.muted),
                    ),
                ]));
                in_code_block = false;
                code_lang.clear();
            } else {
                // Start code block
                code_lang = line[3..].trim().to_string();
                let lang_display = if code_lang.is_empty() {
                    "".to_string()
                } else {
                    format!(" {} ", code_lang)
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}{}", "".repeat(40), lang_display),
                        Style::default().fg(colors.muted),
                    ),
                ]));
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            // Code line
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Rgb(166, 227, 161)),
                ),
            ]));
        } else if line.starts_with("# ") {
            // H1
            lines.push(Line::from(vec![
                Span::styled(
                    line[2..].to_string(),
                    Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("## ") {
            // H2
            lines.push(Line::from(vec![
                Span::styled(
                    line[3..].to_string(),
                    Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("### ") {
            // H3
            lines.push(Line::from(vec![
                Span::styled(
                    line[4..].to_string(),
                    Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("- ") || line.starts_with("* ") {
            // Bullet
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("", Style::default().fg(colors.success)),
                Span::styled(" ", Style::default()),
                Span::styled(line[2..].to_string(), Style::default().fg(colors.fg)),
            ]));
        } else if line.starts_with("> ") {
            // Blockquote
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("", Style::default().fg(colors.muted)),
                Span::styled(" ", Style::default()),
                Span::styled(
                    line[2..].to_string(),
                    Style::default()
                        .fg(colors.muted)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        } else {
            // Regular line with inline formatting
            lines.push(parse_inline_markdown(line, colors));
        }
    }

    lines
}

/// Parse inline markdown (bold, italic, code)
fn parse_inline_markdown(line: &str, colors: &ThemeColors) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_code = false;

    while let Some(c) = chars.next() {
        if c == '`' && !in_code {
            // Start inline code
            if !current.is_empty() {
                let style = get_inline_style(in_bold, in_italic, false, colors);
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            in_code = true;
        } else if c == '`' && in_code {
            // End inline code
            let style = Style::default()
                .fg(Color::Rgb(166, 227, 161))
                .bg(Color::Rgb(49, 50, 68));
            spans.push(Span::styled(std::mem::take(&mut current), style));
            in_code = false;
        } else if c == '*' && chars.peek() == Some(&'*') && !in_code {
            chars.next(); // consume second *
            if !current.is_empty() {
                let style = get_inline_style(in_bold, in_italic, false, colors);
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            in_bold = !in_bold;
        } else if c == '*' && !in_code {
            if !current.is_empty() {
                let style = get_inline_style(in_bold, in_italic, false, colors);
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            in_italic = !in_italic;
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        let style = get_inline_style(in_bold, in_italic, in_code, colors);
        spans.push(Span::styled(current, style));
    }

    Line::from(spans)
}

fn get_inline_style(bold: bool, italic: bool, code: bool, colors: &ThemeColors) -> Style {
    let mut style = Style::default().fg(colors.fg);

    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if code {
        style = style.fg(Color::Rgb(166, 227, 161)).bg(Color::Rgb(49, 50, 68));
    }

    style
}

// ============================================================================
// Help Content
// ============================================================================

/// Generate help content for the help panel
pub fn help_content() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "Input Modes",
            vec![
                ("i", "Enter INSERT mode"),
                ("v", "Enter VISUAL mode"),
                ("/ or :", "Enter COMMAND mode"),
                ("Esc", "Return to NORMAL mode"),
            ],
        ),
        (
            "Navigation",
            vec![
                ("j/k or Up/Down", "Scroll up/down"),
                ("g/G", "Go to top/bottom"),
                ("Ctrl+D/U", "Page down/up"),
                ("Tab/Shift+Tab", "Next/prev panel"),
                ("1-4", "Focus panel by number"),
            ],
        ),
        (
            "Editing",
            vec![
                ("Backspace", "Delete char before cursor"),
                ("Ctrl+W", "Delete word before cursor"),
                ("Ctrl+U", "Clear input"),
                ("Ctrl+A/E", "Go to start/end of line"),
                ("Up/Down", "History prev/next"),
            ],
        ),
        (
            "Global",
            vec![
                ("Ctrl+P", "Open command palette"),
                ("Ctrl+B", "Toggle side panel"),
                ("Ctrl+T", "Toggle theme"),
                ("Ctrl+L", "Clear conversation"),
                ("Ctrl+C/Q", "Quit"),
                ("?", "Toggle help"),
            ],
        ),
        (
            "Commands",
            vec![
                ("/help", "Show this help"),
                ("/clear", "Clear conversation"),
                ("/model <name>", "Switch model"),
                ("/risk <level>", "Set risk level"),
                ("/theme", "Toggle theme"),
                ("/quit", "Exit Ganesha"),
            ],
        ),
    ]
}

// ============================================================================
// Header Widget
// ============================================================================

/// Header widget showing model, risk level, etc.
pub struct Header<'a> {
    model_name: &'a str,
    risk_level: ganesha_core::RiskLevel,
    mode: InputMode,
    session_tokens: u64,
    total_tokens: u64,
    git_branch: Option<&'a str>,
    colors: &'a ThemeColors,
}

impl<'a> Header<'a> {
    pub fn new(
        model_name: &'a str,
        risk_level: ganesha_core::RiskLevel,
        mode: InputMode,
        session_tokens: u64,
        total_tokens: u64,
        git_branch: Option<&'a str>,
        colors: &'a ThemeColors,
    ) -> Self {
        Self {
            model_name,
            risk_level,
            mode,
            session_tokens,
            total_tokens,
            git_branch,
            colors,
        }
    }
}

impl<'a> Widget for Header<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        // Split header into sections
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(12), // Mode indicator
                Constraint::Length(2),  // Spacer
                Constraint::Min(20),    // Model + risk
                Constraint::Length(20), // Tokens
                Constraint::Length(15), // Git branch
            ])
            .split(area);

        // Mode indicator
        ModeIndicator::new(self.mode).render(chunks[0], buf);

        // Model and risk level
        let risk_icon = self.risk_level.icon();
        let model_text = format!("  {}  {} {}", self.model_name, risk_icon, self.risk_level);
        let model_style = Style::default().fg(self.colors.fg);
        buf.set_string(chunks[2].x, chunks[2].y, &model_text, model_style);

        // Token counter
        TokenCounter::new(self.session_tokens, self.total_tokens).render(chunks[3], buf);

        // Git branch
        if let Some(branch) = self.git_branch {
            let git_text = format!("  {}", branch);
            let git_style = Style::default().fg(self.colors.muted);
            buf.set_string(chunks[4].x, chunks[4].y, &git_text, git_style);
        }
    }
}

// ============================================================================
// Status Bar Widget
// ============================================================================

/// Status bar at the bottom of the screen
pub struct StatusBar<'a> {
    message: Option<&'a str>,
    hint: &'a str,
    colors: &'a ThemeColors,
}

impl<'a> StatusBar<'a> {
    pub fn new(message: Option<&'a str>, hint: &'a str, colors: &'a ThemeColors) -> Self {
        Self {
            message,
            hint,
            colors,
        }
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let bg = self.colors.bg;

        // Fill background
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", Style::default().bg(bg));
        }

        // Render message or hint
        let text = if let Some(msg) = self.message {
            msg
        } else {
            self.hint
        };

        let style = if self.message.is_some() {
            Style::default().fg(self.colors.warning)
        } else {
            Style::default().fg(self.colors.muted)
        };

        buf.set_string(area.x + 1, area.y, text, style);
    }
}

// ============================================================================
// Input Box Widget
// ============================================================================

/// Text input box with cursor
pub struct InputBox<'a> {
    content: &'a str,
    cursor_position: usize,
    mode: InputMode,
    colors: &'a ThemeColors,
    focused: bool,
}

impl<'a> InputBox<'a> {
    pub fn new(
        content: &'a str,
        cursor_position: usize,
        mode: InputMode,
        colors: &'a ThemeColors,
    ) -> Self {
        Self {
            content,
            cursor_position,
            mode,
            colors,
            focused: true,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl<'a> Widget for InputBox<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create block with border
        let border_style = if self.focused {
            Style::default().fg(self.colors.border_focused)
        } else {
            Style::default().fg(self.colors.border)
        };

        let title = match self.mode {
            InputMode::Normal => " Message ",
            InputMode::Insert => " Message [INSERT] ",
            InputMode::Command => " Command ",
            InputMode::Visual => " Message [VISUAL] ",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
            .title_style(Style::default().fg(self.mode.color()));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render content
        let content_style = Style::default().fg(self.colors.fg);
        buf.set_string(inner.x, inner.y, self.content, content_style);

        // Note: Cursor positioning is handled separately via Frame::set_cursor_position
    }
}

// ============================================================================
// Command Palette Widget
// ============================================================================

/// Command palette overlay
pub struct CommandPalette<'a> {
    input: &'a str,
    entries: &'a [(usize, &'a str, &'a str)], // (index, name, description)
    selected: usize,
    colors: &'a ThemeColors,
}

impl<'a> CommandPalette<'a> {
    pub fn new(
        input: &'a str,
        entries: &'a [(usize, &'a str, &'a str)],
        selected: usize,
        colors: &'a ThemeColors,
    ) -> Self {
        Self {
            input,
            entries,
            selected,
            colors,
        }
    }
}

impl<'a> Widget for CommandPalette<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear area with background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf.set_string(x, y, " ", Style::default().bg(self.colors.bg));
            }
        }

        // Draw border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.colors.border_focused))
            .title(" Command Palette ")
            .title_style(Style::default().fg(self.colors.accent));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return;
        }

        // Search input
        let search_text = format!("> {}", self.input);
        buf.set_string(
            inner.x,
            inner.y,
            &search_text,
            Style::default().fg(self.colors.fg),
        );

        // Separator
        if inner.height > 1 {
            let sep = "".repeat(inner.width as usize);
            buf.set_string(
                inner.x,
                inner.y + 1,
                &sep,
                Style::default().fg(self.colors.border),
            );
        }

        // Entries
        for (i, (_, name, desc)) in self.entries.iter().enumerate() {
            let y = inner.y + 2 + i as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = i == self.selected;
            let style = if is_selected {
                Style::default()
                    .fg(self.colors.fg)
                    .bg(self.colors.border)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.colors.fg)
            };

            let prefix = if is_selected { "> " } else { "  " };
            let text = format!("{}{:<15} {}", prefix, name, desc);
            let truncated = if text.len() > inner.width as usize {
                format!("{}...", &text[..inner.width as usize - 3])
            } else {
                text
            };

            buf.set_string(inner.x, y, &truncated, style);
        }
    }
}
