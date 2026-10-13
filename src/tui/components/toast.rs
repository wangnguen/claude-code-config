use ratatui::{prelude::*, widgets::*};

use super::confirm_modal::textwrap;
use super::super::theme::*;
use super::super::widgets;

/// A short result message over the current screen; any key dismisses it.
pub struct Toast {
    text: String,
    success: bool,
}

impl Toast {
    pub fn new(text: &str, success: bool) -> Self {
        Toast { text: text.to_string(), success }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let max_width = area.width.saturating_sub(8).clamp(20, 76);
        let text_width = console::measure_text_width(&self.text) as u16;
        let width = (text_width + 8).clamp(30, max_width);
        let lines = textwrap(&self.text, width.saturating_sub(6) as usize);
        let rect = widgets::centered(width, lines.len() as u16 + 4, area);
        frame.render_widget(Clear, rect);

        let (colour, title, mark) = if self.success {
            (SUCCESS, "Done", SYM_OK)
        } else {
            (ERROR, "Can't do that", SYM_FAIL)
        };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colour))
            .style(Style::default().bg(MODAL_BG))
            .title(Span::styled(format!(" {mark} {title} "), Style::default().fg(colour).bold()))
            .title_bottom(Line::from(Span::styled(" any key ", Style::default().fg(TEXT_DIM))).centered());
        let inner = block.inner(rect).inner(Margin { horizontal: 2, vertical: 1 });
        frame.render_widget(block, rect);

        let lines: Vec<Line> = lines
            .into_iter()
            .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::White))))
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
