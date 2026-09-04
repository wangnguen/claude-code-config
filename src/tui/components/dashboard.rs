use ratatui::{prelude::*, widgets::*};
use super::super::theme::*;
use crate::api::get_api_config;
use crate::config::KeysStore;
use crate::utils::mask_key;

pub struct Dashboard {
    pub total_keys: usize,
    pub active_key: Option<String>,
    pub active_key_masked: Option<String>,
    pub api_url: String,
    pub model: String,
    pub ok_count: usize,
    pub fail_count: usize,
}

impl Dashboard {
    pub fn new() -> Self {
        let store = KeysStore::load();
        let (api_url, model) = get_api_config();

        let (active_key, active_key_masked) = store.get_active_key()
            .map(|k| (Some(k.clone()), Some(mask_key(k))))
            .unwrap_or((None, None));

        Dashboard {
            total_keys: store.keys.len(),
            active_key,
            active_key_masked,
            api_url,
            model,
            ok_count: 0,
            fail_count: 0,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(BG));
        let inner = outer_block.inner(area);
        frame.render_widget(&outer_block, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4),  // Title banner
                Constraint::Length(1),  // Spacer
                Constraint::Length(8),  // Quick stats
                Constraint::Length(1),  // Spacer
                Constraint::Min(5),     // Key list preview
                Constraint::Length(3),  // Actions hint
            ])
            .split(inner);

        self.render_banner(frame, layout[0]);
        self.render_stats(frame, layout[2]);
        self.render_key_preview(frame, layout[4]);
        self.render_actions(frame, layout[5]);
    }

    fn render_banner(&self, frame: &mut Frame, area: Rect) {
        // ASCII art style banner
        let banner = vec![
            Line::from(vec![
                Span::styled(" ╔", Style::default().fg(ACCENT)),
                Span::styled("═".repeat(50), Style::default().fg(ACCENT)),
                Span::styled("╗", Style::default().fg(ACCENT)),
            ]),
            Line::from(vec![
                Span::styled(" ║", Style::default().fg(ACCENT)),
                Span::styled("  🔑 ", Style::default().fg(Color::Yellow)),
                Span::styled("K E Y   M A N A G E R", Style::default().fg(Color::White).bold()),
                Span::styled(format!(" {:>20}", format!("{} keys", self.total_keys)), Style::default().fg(DIM)),
                Span::styled("  ║", Style::default().fg(ACCENT)),
            ]),
            Line::from(vec![
                Span::styled(" ╚", Style::default().fg(ACCENT)),
                Span::styled("═".repeat(50), Style::default().fg(ACCENT)),
                Span::styled("╝", Style::default().fg(ACCENT)),
            ]),
        ];
        let para = Paragraph::new(banner)
            .alignment(Alignment::Center)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(para, area);
    }

    fn render_stats(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("  Quick Stats  ")
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        let inner = block.inner(area);
        frame.render_widget(&block, area);

        let stats_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(inner);

        // Left column: Active key info
        let left_content = vec![
            Line::from(vec![
                Span::styled(" ● ", Style::default().fg(Color::Green)),
                Span::styled("Active Key", Style::default().fg(DIM)),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    self.active_key.as_deref().unwrap_or("(none)"),
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    self.active_key_masked.as_deref().unwrap_or(""),
                    Style::default().fg(TEXT_DIM).italic(),
                ),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(left_content).style(Style::default().bg(PANEL_BG)),
            stats_layout[0],
        );

        // Right column: API config
        let right_content = vec![
            Line::from(vec![
                Span::styled(" ○ ", Style::default().fg(Color::Cyan)),
                Span::styled("API", Style::default().fg(DIM)),
                Span::raw("  "),
                Span::styled(&self.api_url, Style::default().fg(TEXT)),
            ]),
            Line::from(vec![
                Span::styled(" ○ ", Style::default().fg(Color::Cyan)),
                Span::styled("Model", Style::default().fg(DIM)),
                Span::raw(" "),
                Span::styled(&self.model, Style::default().fg(TEXT)),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(right_content).style(Style::default().bg(PANEL_BG)),
            stats_layout[1],
        );
    }

    fn render_key_preview(&self, frame: &mut Frame, area: Rect) {
        let store = KeysStore::load();
        let block = Block::default()
            .title("  Keys  ")
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));

        if store.keys.is_empty() {
            let empty = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("No keys saved. ", Style::default().fg(DIM)),
                    Span::styled("Press ", Style::default().fg(TEXT_DIM)),
                    Span::styled("[a]", Style::default().fg(Color::Green).bold()),
                    Span::styled(" to add your first key", Style::default().fg(TEXT_DIM)),
                ]),
                Line::from(""),
            ])
            .alignment(Alignment::Center)
            .style(Style::default().bg(PANEL_BG))
            .block(block);
            frame.render_widget(empty, area);
            return;
        }

        let inner = block.inner(area);
        frame.render_widget(&block, area);

        let header = Row::new(vec!["", "  Name", "Key"])
            .style(Style::default().fg(ACCENT).bold())
            .bottom_margin(1);

        let rows: Vec<Row> = store.keys.iter()
            .enumerate()
            .map(|(i, (name, value))| {
                let is_active = store.active.as_deref() == Some(name.as_str());
                let bg = if i % 2 == 0 { PANEL_BG } else { Color::Rgb(25, 25, 40) };
                Row::new(vec![
                    Cell::from(if is_active { " ▶ " } else { "   " }).style(Style::default().fg(ACCENT)),
                    Cell::from(format!("  {}{}", name, if is_active { " ★" } else { "" })).style(Style::default().fg(if is_active { Color::White } else { TEXT })),
                    Cell::from(mask_key(value)).style(Style::default().fg(TEXT_DIM)),
                ]).style(Style::default().bg(bg))
            })
            .collect();

        let table = Table::new(rows, [
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .header(header);
        frame.render_widget(table, inner);
    }

    fn render_actions(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        let inner = block.inner(area);
        frame.render_widget(&block, area);
        let line = Line::from(vec![
            Span::styled(" [a]", Style::default().fg(Color::Green).bold()),
            Span::styled("Add  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[d]", Style::default().fg(Color::Yellow).bold()),
            Span::styled("Default  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[u]", Style::default().fg(Color::Rgb(100, 149, 237)).bold()),
            Span::styled("Use  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[s]", Style::default().fg(Color::Cyan).bold()),
            Span::styled("Status  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[←/→]", Style::default().fg(Color::Magenta).bold()),
            Span::styled("Navigate  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[q]", Style::default().fg(Color::Rgb(80, 80, 100)).bold()),
            Span::styled("Quit", Style::default().fg(TEXT_DIM)),
        ]);
        frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), inner);
    }
}