//! # UI Rendering
//!
//! Main rendering functions for the TUI.
//! Uses ratatui for terminal rendering with a clean, professional layout.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget, Wrap},
    Frame,
};

use super::app::{AppState, ChatMessage, DiffEntry, DiffStatus, FileEntry, Panel, ThemeColors};
use super::widgets::{
    self, CommandPalette, Header, InputBox, ModeIndicator, ProgressBar, RiskIndicator,
    Scrollbar as CustomScrollbar, Spinner, StatusBar, TokenCounter,
};
use ganesha_providers::message::MessageRole;

/// Main view function - renders the entire UI
pub fn view(f: &mut Frame, state: &AppState) {
    let colors = state.theme.colors();
    let area = f.area();

    // Main layout: header, body, input, status
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(5),    // Body (conversation + side panels)
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Render header
    render_header(f, state, &colors, main_chunks[0]);

    // Render body (conversation + optional side panel)
    render_body(f, state, &colors, main_chunks[1]);

    // Render input area
    render_input(f, state, &colors, main_chunks[2]);

    // Render status bar
    render_status_bar(f, state, &colors, main_chunks[3]);

    // Render overlays (command palette, help, etc.)
    if state.command_palette_open {
        render_command_palette(f, state, &colors);
    }

    if state.active_panel == Panel::Help {
        render_help_overlay(f, state, &colors);
    }
}

/// Render the header bar
fn render_header(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    Header::new(
        &state.model_name,
        state.risk_level,
        state.input_mode,
        state.session_tokens,
        state.total_tokens,
        state.git_branch.as_deref(),
        colors,
    )
    .render(area, f.buffer_mut());
}

/// Render the main body area
fn render_body(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    if state.show_side_panel && area.width >= 80 {
        // Split into conversation and side panel
        let side_width = state.side_panel_width.min(area.width / 3);
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(40),                   // Conversation
                Constraint::Length(side_width as u16), // Side panel
            ])
            .split(area);

        render_conversation(f, state, colors, body_chunks[0]);
        render_side_panel(f, state, colors, body_chunks[1]);
    } else {
        // Full width conversation
        render_conversation(f, state, colors, area);
    }
}

/// Render the conversation panel
fn render_conversation(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    let is_focused = state.active_panel == Panel::Conversation;
    let border_style = if is_focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border)
    };

    let title = if state.is_ai_thinking {
        if let Some(ref spinner) = state.spinner {
            format!(" {} {} ", spinner.current_frame(), spinner.message)
        } else {
            " Ganesha ".to_string()
        }
    } else {
        " Ganesha ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .title_style(Style::default().fg(colors.accent).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render messages
    let messages: Vec<ListItem> = state
        .messages
        .iter()
        .flat_map(|msg| message_to_list_items(msg, colors, inner.width as usize))
        .collect();

    // Calculate scroll
    let total_items = messages.len();
    let visible_items = inner.height as usize;
    let scroll_offset = if total_items > visible_items {
        total_items.saturating_sub(visible_items)
    } else {
        0
    };

    let messages_list = List::new(messages)
        .style(Style::default().fg(colors.fg));

    // Render with scroll offset
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(scroll_offset));
    f.render_stateful_widget(messages_list, inner, &mut list_state);

    // Render scrollbar if needed
    if total_items > visible_items {
        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1,
            width: 1,
            height: area.height - 2,
        };

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(total_items)
            .position(state.conversation_scroll);

        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .symbols(ratatui::symbols::scrollbar::VERTICAL)
                .style(Style::default().fg(colors.muted)),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

/// Convert a message to list items (may be multiple lines)
fn message_to_list_items<'a>(
    msg: &ChatMessage,
    colors: &ThemeColors,
    _max_width: usize,
) -> Vec<ListItem<'a>> {
    let (prefix, style) = match msg.role {
        MessageRole::User => ("You: ", Style::default().fg(colors.user_msg)),
        MessageRole::Assistant => ("", Style::default().fg(colors.assistant_msg)),
        MessageRole::System => ("", Style::default().fg(colors.system_msg)),
        MessageRole::Tool => ("", Style::default().fg(colors.muted)),
    };

    let mut items = Vec::new();

    // Parse markdown and create styled lines
    let parsed = widgets::parse_markdown(&msg.content, colors);

    // First line with prefix
    let first_line = if !parsed.is_empty() {
        let mut spans = vec![
            Span::styled(prefix.to_string(), style.add_modifier(Modifier::BOLD)),
        ];
        spans.extend(parsed[0].spans.clone());
        Line::from(spans)
    } else {
        Line::from(vec![Span::styled(prefix.to_string(), style)])
    };

    items.push(ListItem::new(first_line));

    // Subsequent lines
    for line in parsed.into_iter().skip(1) {
        items.push(ListItem::new(line));
    }

    // Add separator
    items.push(ListItem::new(Line::from("")));

    items
}

/// Render the side panel (file tree, diff, tool output)
fn render_side_panel(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    // Split side panel vertically into sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Files
            Constraint::Percentage(30), // Diff
            Constraint::Percentage(30), // Tool Output
        ])
        .split(area);

    render_file_tree(f, state, colors, chunks[0]);
    render_diff_panel(f, state, colors, chunks[1]);
    render_tool_output(f, state, colors, chunks[2]);
}

/// Render the file tree
fn render_file_tree(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    let is_focused = state.active_panel == Panel::FileTree;
    let border_style = if is_focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Files ")
        .title_style(Style::default().fg(colors.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.file_entries.is_empty() {
        let hint = Paragraph::new("No files loaded.\nUse /files to refresh.")
            .style(Style::default().fg(colors.muted))
            .alignment(Alignment::Center);
        f.render_widget(hint, inner);
        return;
    }

    let items: Vec<ListItem> = state
        .file_entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = state.selected_file == Some(i);

            let indent = "  ".repeat(entry.depth);
            let icon = if entry.is_dir {
                if entry.is_expanded {
                    ""
                } else {
                    ""
                }
            } else {
                file_icon(&entry.name)
            };

            let mut style = Style::default().fg(colors.fg);

            if entry.is_modified {
                style = style.fg(colors.warning);
            }
            if entry.is_staged {
                style = style.fg(colors.success);
            }
            if is_selected {
                style = style.bg(colors.border).add_modifier(Modifier::BOLD);
            }

            ListItem::new(Line::from(vec![
                Span::raw(indent),
                Span::styled(format!("{} ", icon), style),
                Span::styled(entry.name.clone(), style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Get icon for a file based on extension
fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "rs" => "",
        "py" => "",
        "js" | "jsx" => "",
        "ts" | "tsx" => "",
        "json" => "",
        "toml" | "yaml" | "yml" => "",
        "md" => "",
        "html" => "",
        "css" => "",
        "sh" | "bash" => "",
        "git" => "",
        "lock" => "",
        _ => "",
    }
}

/// Render the diff panel
fn render_diff_panel(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    let is_focused = state.active_panel == Panel::Diff;
    let border_style = if is_focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Changes ")
        .title_style(Style::default().fg(colors.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.diff_entries.is_empty() {
        let hint = Paragraph::new("No pending changes.")
            .style(Style::default().fg(colors.muted))
            .alignment(Alignment::Center);
        f.render_widget(hint, inner);
        return;
    }

    let items: Vec<ListItem> = state
        .diff_entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = state.selected_diff == Some(i);
            let status_style = Style::default().fg(entry.status.color());
            let name_style = if is_selected {
                Style::default().bg(colors.border).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.fg)
            };

            let file_name = entry
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", entry.status.symbol()), status_style),
                Span::styled(file_name, name_style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Render the tool output panel
fn render_tool_output(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    let is_focused = state.active_panel == Panel::ToolOutput;
    let border_style = if is_focused {
        Style::default().fg(colors.border_focused)
    } else {
        Style::default().fg(colors.border)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Tool Output ")
        .title_style(Style::default().fg(colors.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.tool_outputs.is_empty() {
        let hint = Paragraph::new("No tool output.")
            .style(Style::default().fg(colors.muted))
            .alignment(Alignment::Center);
        f.render_widget(hint, inner);
        return;
    }

    // Show most recent tool outputs
    let items: Vec<ListItem> = state
        .tool_outputs
        .iter()
        .rev()
        .take(inner.height as usize)
        .map(|output| {
            let icon = if output.success { "" } else { "" };
            let icon_style = if output.success {
                Style::default().fg(colors.success)
            } else {
                Style::default().fg(colors.error)
            };

            let duration = format!("{:.1}s", output.duration.as_secs_f64());

            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), icon_style),
                Span::styled(&output.tool_name, Style::default().fg(colors.fg)),
                Span::styled(format!(" ({})", duration), Style::default().fg(colors.muted)),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Render the input area
fn render_input(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    InputBox::new(
        &state.input_buffer,
        state.input_cursor,
        state.input_mode,
        colors,
    )
    .render(area, f.buffer_mut());

    // Position cursor
    let inner_x = area.x + 1;
    let inner_y = area.y + 1;

    // Calculate cursor position accounting for Unicode
    let cursor_x = inner_x
        + state.input_buffer[..state.input_cursor]
            .chars()
            .count() as u16;

    // Only show cursor in insert/command mode
    match state.input_mode {
        super::app::InputMode::Insert | super::app::InputMode::Command => {
            f.set_cursor_position((cursor_x, inner_y));
        }
        _ => {}
    }
}

/// Render the status bar
fn render_status_bar(f: &mut Frame, state: &AppState, colors: &ThemeColors, area: Rect) {
    let message = state.status_message.as_ref().map(|(m, _)| m.as_str());

    let hint = match state.input_mode {
        super::app::InputMode::Normal => "i: insert | /: command | ?: help | Tab: panels | q: quit",
        super::app::InputMode::Insert => "Esc: normal | Enter: send | Ctrl+C: quit",
        super::app::InputMode::Command => "Esc: cancel | Enter: execute | Tab: complete",
        super::app::InputMode::Visual => "Esc: cancel | y: yank | d: delete",
    };

    StatusBar::new(message, hint, colors).render(area, f.buffer_mut());
}

/// Render the command palette overlay
fn render_command_palette(f: &mut Frame, state: &AppState, colors: &ThemeColors) {
    let area = f.area();

    // Center the palette
    let palette_width = 50.min(area.width - 4);
    let palette_height = 15.min(area.height - 4);
    let palette_x = (area.width - palette_width) / 2;
    let palette_y = (area.height - palette_height) / 3;

    let palette_area = Rect {
        x: palette_x,
        y: palette_y,
        width: palette_width,
        height: palette_height,
    };

    // Clear the area
    f.render_widget(Clear, palette_area);

    // Prepare entries
    let entries: Vec<(usize, &str, &str)> = state
        .command_palette_filtered
        .iter()
        .filter_map(|&i| {
            state
                .command_palette_entries
                .get(i)
                .map(|e| (i, e.name.as_str(), e.description.as_str()))
        })
        .collect();

    CommandPalette::new(
        &state.command_palette_input,
        &entries,
        state.command_palette_selected,
        colors,
    )
    .render(palette_area, f.buffer_mut());
}

/// Render the help overlay
fn render_help_overlay(f: &mut Frame, state: &AppState, colors: &ThemeColors) {
    let area = f.area();

    // Calculate overlay size
    let help_width = 60.min(area.width - 4);
    let help_height = (area.height - 4).min(30);
    let help_x = (area.width - help_width) / 2;
    let help_y = (area.height - help_height) / 2;

    let help_area = Rect {
        x: help_x,
        y: help_y,
        width: help_width,
        height: help_height,
    };

    // Clear the area
    f.render_widget(Clear, help_area);

    // Draw border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.border_focused))
        .title(" Help - Press ? or Esc to close ")
        .title_style(Style::default().fg(colors.accent).add_modifier(Modifier::BOLD));

    let inner = block.inner(help_area);
    f.render_widget(block, help_area);

    // Render help content
    let help_content = widgets::help_content();
    let mut lines: Vec<Line> = Vec::new();

    for (section, bindings) in help_content {
        // Section header
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", section),
                Style::default()
                    .fg(colors.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from("")); // Spacer

        // Bindings
        for (key, desc) in bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<20}", key), Style::default().fg(colors.warning)),
                Span::styled(desc.to_string(), Style::default().fg(colors.fg)),
            ]));
        }

        lines.push(Line::from("")); // Spacer between sections
    }

    let help_text = Paragraph::new(lines)
        .style(Style::default().fg(colors.fg))
        .wrap(Wrap { trim: false });

    f.render_widget(help_text, inner);
}
