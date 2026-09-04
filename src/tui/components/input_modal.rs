use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::component::Action;
use super::super::input::InputField;
use super::super::theme::*;

pub struct InputModal {
    pub title: String,
    pub label: String,
    pub input: InputField,
    pub step: u16,
    pub total: u16,
    /// Called with the input value on Enter. Returns an Action.
    on_submit: Box<dyn Fn(String) -> Action>,
}

impl InputModal {
    pub fn new(
        title: impl Into<String>,
        label: impl Into<String>,
        step: u16,
        total: u16,
        on_submit: impl Fn(String) -> Action + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            label: label.into(),
            input: InputField::new(),
            step,
            total,
            on_submit: Box::new(on_submit),
        }
    }

    pub fn handle_event(&mut self, key: KeyCode) -> Action {
        match key {
            KeyCode::Esc => Action::Switch(super::super::component::ModeSwitch::Normal),
            KeyCode::Enter => {
                let value = self.input.value.clone();
                (self.on_submit)(value)
            }
            other => {
                self.input.handle_key(other);
                Action::None
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let r = centered_rect(60, 8, area);
        frame.render_widget(Clear, r);

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(MODAL_BG));
        let inner = block.inner(r);
        frame.render_widget(block, r);

        // Step indicator
        let step_text = format!("Step {}/{}", self.step, self.total);
        let step_line = Line::from(vec![
            Span::styled(&self.label, Style::default().fg(TEXT_DIM)),
            Span::raw("  "),
            Span::styled(step_text, Style::default().fg(DIM).italic()),
        ]);
        frame.render_widget(
            Paragraph::new(step_line),
            Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        );

        // Input box
        let input_area = Rect::new(inner.x + 1, inner.y + 2, inner.width.saturating_sub(2), 3);
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(70, 70, 100)))
            .style(Style::default().bg(Color::Rgb(20, 20, 35)));

        let input_inner = input_block.inner(input_area);
        frame.render_widget(input_block, input_area);

        // Text with cursor
        let before: String = self.input.value.chars().take(self.input.cursor).collect();
        let cursor_char = self.input.value.chars().nth(self.input.cursor).unwrap_or(' ');
        let after: String = self.input.value.chars().skip(self.input.cursor + 1).collect();

        let input_line = Line::from(vec![
            Span::styled(&before, Style::default().fg(Color::White)),
            Span::styled(cursor_char.to_string(), Style::default().fg(Color::Black).bg(ACCENT)),
            Span::styled(&after, Style::default().fg(Color::White)),
        ]);
        frame.render_widget(Paragraph::new(input_line), input_inner);
    }
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}
