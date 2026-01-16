use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::core::config::{GaneshaConfig, ConfigManager, ModelTier};

pub struct SettingsView {
    pub config: GaneshaConfig,
    pub list_state: ListState,
    pub selected_provider_idx: Option<usize>,
}

impl SettingsView {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new();
        let config = config_manager.load();
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            config,
            list_state,
            selected_provider_idx: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(70),
            ])
            .split(area);

        self.render_provider_list(frame, chunks[0]);
        self.render_details(frame, chunks[1]);
    }

    fn render_provider_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.config.providers
            .iter()
            .map(|p| {
                let color = match p.tier {
                    ModelTier::Fast => Color::Green,
                    ModelTier::Capable => Color::Yellow,
                    ModelTier::Vision => Color::Magenta,
                    ModelTier::Cloud => Color::Blue,
                    ModelTier::Premium => Color::Cyan,
                    ModelTier::Standard => Color::White,
                };
                ListItem::new(Line::from(vec![
                    Span::styled(&p.name, Style::default().fg(color)),
                    Span::styled(format!(" ({})", p.model), Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Providers "))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let selected_idx = self.list_state.selected().unwrap_or(0);
        if let Some(provider) = self.config.providers.get(selected_idx) {
            let details = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&provider.name),
                ]),
                Line::from(vec![
                    Span::styled("Endpoint: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&provider.endpoint),
                ]),
                Line::from(vec![
                    Span::styled("Model: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&provider.model),
                ]),
                Line::from(vec![
                    Span::styled("Tier: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{:?}", provider.tier)),
                ]),
                Line::from(vec![
                    Span::styled("Max Concurrent: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(provider.max_concurrent.to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Cost/1k Tokens: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("${}", provider.cost_per_1k_tokens)),
                ]),
            ];

            let paragraph = Paragraph::new(details)
                .block(Block::default().borders(Borders::ALL).title(" Provider Details "));
            frame.render_widget(paragraph, area);
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Up => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.config.providers.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            KeyCode::Down => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i >= self.config.providers.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            _ => {}
        }
    }
}
