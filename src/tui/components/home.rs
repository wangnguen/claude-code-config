use ratatui::{prelude::*, widgets::*};
use super::super::theme::*;
use crate::config::VERSION;

pub struct HomeDashboard;

impl HomeDashboard {
    pub fn new() -> Self {
        HomeDashboard
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
                Constraint::Length(5),  // Title
                Constraint::Length(2), // Subtitle
                Constraint::Length(1), // Spacer
                Constraint::Min(10),   // Commands list
                Constraint::Length(3), // Actions hint
            ])
            .split(inner);

        self.render_title(frame, layout[0]);
        self.render_subtitle(frame, layout[1]);
        self.render_commands(frame, layout[3]);
        self.render_actions(frame, layout[4]);
    }

    fn render_title(&self, frame: &mut Frame, area: Rect) {
        let title = vec![
            Line::from(vec![
                Span::styled(" ╔", Style::default().fg(ACCENT)),
                Span::styled("═".repeat(54), Style::default().fg(ACCENT)),
                Span::styled("╗", Style::default().fg(ACCENT)),
            ]),
            Line::from(vec![
                Span::styled(" ║", Style::default().fg(ACCENT)),
                Span::styled("  🔑 ", Style::default().fg(Color::Yellow)),
                Span::styled("Claude Code Config CLI", Style::default().fg(Color::White).bold()),
                Span::raw("                              ║"),
            ]),
            Line::from(vec![
                Span::styled(" ╚", Style::default().fg(ACCENT)),
                Span::styled("═".repeat(54), Style::default().fg(ACCENT)),
                Span::styled("╝", Style::default().fg(ACCENT)),
            ]),
        ];
        let para = Paragraph::new(title)
            .alignment(Alignment::Center)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(para, area);
    }

    fn render_subtitle(&self, frame: &mut Frame, area: Rect) {
        let subtitle = Line::from(vec![
            Span::styled("v", Style::default().fg(DIM)),
            Span::styled(VERSION, Style::default().fg(TEXT_DIM)),
            Span::styled("  •  ", Style::default().fg(DIM)),
            Span::styled("Manage Claude Code configuration with ease", Style::default().fg(TEXT_DIM)),
        ]);
        let para = Paragraph::new(subtitle)
            .alignment(Alignment::Center)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(para, area);
    }

    fn render_commands(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("  Available Commands  ")
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        let inner = block.inner(area);
        frame.render_widget(&block, area);

        let commands = vec![
            ("ccc key",      "Manage API keys",              Color::Green),
            ("ccc show",     "Show config (global/local)",   Color::Cyan),
            ("ccc init",     "Copy config to current dir",   Color::Magenta),
            ("ccc doctor",   "Check environment status",     Color::Yellow),
            ("ccc check",    "Test API connection",           Color::Rgb(100, 149, 237)),
            ("ccc update",   "Check for updates",            Color::Rgb(255, 165, 0)),
            ("ccc version",  "Show version",                 Color::Gray),
        ];

        let header = Row::new(vec!["Command", "Description"])
            .style(Style::default().fg(ACCENT).bold())
            .bottom_margin(1);

        let rows: Vec<Row> = commands.iter().enumerate().map(|(i, (cmd, desc, color))| {
            let bg = if i % 2 == 0 { PANEL_BG } else { Color::Rgb(25, 25, 40) };
            Row::new(vec![
                Cell::from(Span::raw(format!("  {}", cmd)).style(Style::default().fg(*color).bold())),
                Cell::from(Span::raw(*desc)).style(Style::default().fg(TEXT)),
            ]).style(Style::default().bg(bg))
        }).collect();

        let table = Table::new(rows, [
            Constraint::Percentage(35),
            Constraint::Percentage(65),
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
        frame.render_widget(&block, area);

        let inner = block.inner(area);
        let line = Line::from(vec![
            Span::styled(" [key]", Style::default().fg(Color::Green).bold()),
            Span::styled("Manage Keys  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[show]", Style::default().fg(Color::Cyan).bold()),
            Span::styled("Show Config  ", Style::default().fg(TEXT_DIM)),
            Span::styled("[q]", Style::default().fg(Color::Rgb(80, 80, 100)).bold()),
            Span::styled("Quit", Style::default().fg(TEXT_DIM)),
        ]);
        frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), inner);
    }
}