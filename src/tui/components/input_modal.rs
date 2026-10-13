use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::input::InputField;
use super::super::theme::*;
use super::super::widgets;

/// A one-line prompt over the current screen. Secret prompts show a dot per
/// character, so a key pasted in is never on screen.
pub struct InputModal {
    title: String,
    label: String,
    input: InputField,
    secret: bool,
}

impl InputModal {
    pub fn new(title: impl Into<String>, label: impl Into<String>, value: &str, secret: bool) -> Self {
        Self {
            title: title.into(),
            label: label.into(),
            input: InputField::with_value(value),
            secret,
        }
    }

    pub fn value(&self) -> String {
        self.input.value()
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        self.input.handle_key(code);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let rect = widgets::centered(64, 8, area);
        frame.render_widget(Clear, rect);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(MODAL_BG))
            .title(Span::styled(format!(" {} ", self.title), Style::default().fg(ACCENT).bold()));
        let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
        frame.render_widget(block, rect);

        let [label, field, hints] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .flex(layout::Flex::SpaceBetween)
        .areas(inner);

        frame.render_widget(
            Paragraph::new(Span::styled(widgets::fit(&self.label, label.width as usize), Style::default().fg(TEXT))),
            label,
        );

        let field_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(BG));
        let text_area = field_block.inner(field);
        frame.render_widget(field_block, field);
        frame.render_widget(Paragraph::new(self.field_line(text_area.width as usize)), text_area);

        let hint = [("Enter", "save"), ("Esc", "cancel")];
        frame.render_widget(
            Paragraph::new(widgets::hint_line(&hint, hints.width as usize)).alignment(Alignment::Right),
            hints,
        );
    }

    /// The visible part of the text, scrolled so the cursor stays in view.
    fn field_line(&self, width: usize) -> Line<'static> {
        let chars: Vec<char> = if self.secret {
            vec!['•'; self.input.len()]
        } else {
            self.input.chars().to_vec()
        };
        let cursor = self.input.cursor();
        // One column for the cursor block itself.
        let room = width.saturating_sub(1).max(1);
        let start = cursor.saturating_sub(room.saturating_sub(1));
        let end = (start + room).min(chars.len());

        let before: String = chars[start..cursor.min(end)].iter().collect();
        let at = chars.get(cursor).copied().unwrap_or(' ');
        let after: String = chars.get(cursor + 1..end).map(|c| c.iter().collect()).unwrap_or_default();
        Line::from(vec![
            Span::styled(before, Style::default().fg(Color::White)),
            Span::styled(at.to_string(), Style::default().fg(Color::Black).bg(ACCENT)),
            Span::styled(after, Style::default().fg(Color::White)),
        ])
    }
}
