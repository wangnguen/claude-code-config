use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::component::{Action, KeyOp, ModeSwitch};
use super::super::theme::*;

pub struct ConfirmModal {
    pub name: String,
}

impl ConfirmModal {
    pub fn handle_event(&mut self, key: KeyCode) -> Action {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                Action::Op(KeyOp::Remove(self.name.clone()))
            }
            _ => Action::Switch(ModeSwitch::Normal),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let r = centered_rect(50, 6, area);
        frame.render_widget(Clear, r);

        let block = Block::default()
            .title(" Confirm ".to_string())
            .title_style(Style::default().fg(ERROR).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ERROR))
            .style(Style::default().bg(MODAL_BG));
        let inner = block.inner(r);
        frame.render_widget(block, r);

        let message = format!("Remove '{}'?", self.name);
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(message, Style::default().fg(Color::White).bold())),
            Line::from(vec![
                Span::styled("[y]", Style::default().fg(ERROR).bold()),
                Span::styled(" yes   ", Style::default().fg(TEXT_DIM)),
                Span::styled("any key", Style::default().fg(DIM)),
                Span::styled(" cancel", Style::default().fg(TEXT_DIM)),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
    }
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}
