use ratatui::{prelude::*, widgets::*};
use super::super::theme::*;

pub struct Toast {
    pub text: String,
    pub success: bool,
}

impl Toast {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let w = (self.text.len() as u16 + 6).min(area.width.saturating_sub(4));
        let r = centered_rect(w, 5, area);
        frame.render_widget(Clear, r);

        let color = if self.success { SUCCESS } else { ERROR };
        let icon = if self.success { ICON_CHECK.to_string() } else { ICON_CROSS.to_string() };
        let block = Block::default()
            .title(format!(" {} ", if self.success { "Success" } else { "Error" }))
            .title_style(Style::default().fg(color).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color))
            .style(Style::default().bg(MODAL_BG));
        let inner = block.inner(r);
        frame.render_widget(block, r);

        let line = Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(color).bold()),
            Span::styled(&self.text, Style::default().fg(Color::White)),
        ]);
        frame.render_widget(
            Paragraph::new(vec![Line::from(""), line]).alignment(Alignment::Center),
            inner,
        );
    }
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}
