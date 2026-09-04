use ratatui::{prelude::*, widgets::*};
use super::super::theme::*;

pub struct Header {
    pub total: usize,
}

impl Header {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(vec![
            Span::styled(format!("  {} ", ICON_KEY), Style::default().fg(Color::Yellow)),
            Span::styled("Key Manager", Style::default().fg(ACCENT).bold()),
            Span::styled(format!("  ({} keys)", self.total), Style::default().fg(DIM)),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(Paragraph::new(title).block(block).alignment(Alignment::Center), area);
    }
}
