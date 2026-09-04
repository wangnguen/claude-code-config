use ratatui::{prelude::*, widgets::*};
use super::super::theme::*;

pub enum FooterMode {
    Normal,
    Input,
    Confirm,
    Dismiss,
}

pub struct Footer {
    pub mode: FooterMode,
}

impl Footer {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let line = match self.mode {
            FooterMode::Input => Line::from(vec![
                Span::styled(" Enter", Style::default().fg(SUCCESS).bold()),
                Span::styled(" confirm  ", Style::default().fg(TEXT_DIM)),
                Span::styled("Esc", Style::default().fg(ERROR).bold()),
                Span::styled(" cancel ", Style::default().fg(TEXT_DIM)),
            ]),
            FooterMode::Confirm => Line::from(vec![
                Span::styled(" [y]", Style::default().fg(ERROR).bold()),
                Span::styled("es  ", Style::default().fg(TEXT_DIM)),
                Span::styled("any key", Style::default().fg(DIM).bold()),
                Span::styled(" cancel ", Style::default().fg(TEXT_DIM)),
            ]),
            FooterMode::Dismiss => Line::from(vec![
                Span::styled(" Press any key to continue ", Style::default().fg(TEXT_DIM)),
            ]),
            FooterMode::Normal => Line::from(vec![
                Span::styled(" [a]", Style::default().fg(Color::Green).bold()),
                Span::styled("dd  ", Style::default().fg(TEXT_DIM)),
                Span::styled("[d]", Style::default().fg(Color::Yellow).bold()),
                Span::styled("efault  ", Style::default().fg(TEXT_DIM)),
                Span::styled("[u]", Style::default().fg(Color::Rgb(100, 149, 237)).bold()),
                Span::styled("se  ", Style::default().fg(TEXT_DIM)),
                Span::styled("[r]", Style::default().fg(ERROR).bold()),
                Span::styled("emove  ", Style::default().fg(TEXT_DIM)),
                Span::styled("re[n]", Style::default().fg(Color::Magenta).bold()),
                Span::styled("ame  ", Style::default().fg(TEXT_DIM)),
                Span::styled("[s]", Style::default().fg(Color::Cyan).bold()),
                Span::styled("tatus  ", Style::default().fg(TEXT_DIM)),
                Span::styled("[q]", Style::default().fg(Color::Rgb(80, 80, 100)).bold()),
                Span::styled("uit ", Style::default().fg(TEXT_DIM)),
            ]),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(Paragraph::new(line).alignment(Alignment::Center).block(block), area);
    }
}
