use ratatui::{prelude::*, widgets::*};

use super::super::theme::*;
use super::super::widgets;

/// A yes/no question. Only y and n (or Esc) answer it, so a stray key cannot
/// remove a key or throw away unsaved picks.
pub struct ConfirmModal {
    title: String,
    lines: Vec<String>,
    /// Red for questions whose "yes" loses something.
    destructive: bool,
}

impl ConfirmModal {
    pub fn new(title: impl Into<String>, lines: Vec<String>, destructive: bool) -> Self {
        Self { title: title.into(), lines, destructive }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let colour = if self.destructive { ERROR } else { ACCENT };
        let width = 64u16;
        let text_width = width.saturating_sub(4) as usize;
        let wrapped: Vec<String> = self
            .lines
            .iter()
            .flat_map(|line| textwrap(line, text_width))
            .collect();
        // Border and padding take four rows; the blank line and the y/n hint two more.
        let rect = widgets::centered(width, wrapped.len() as u16 + 6, area);
        frame.render_widget(Clear, rect);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colour))
            .style(Style::default().bg(MODAL_BG))
            .title(Span::styled(format!(" {} ", self.title), Style::default().fg(colour).bold()));
        let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 1 });
        frame.render_widget(block, rect);

        let mut lines: Vec<Line> = wrapped
            .into_iter()
            .enumerate()
            .map(|(i, l)| {
                let style = if i == 0 { Style::default().fg(Color::White).bold() } else { Style::default().fg(TEXT) };
                Line::from(Span::styled(l, style))
            })
            .collect();
        lines.push(Line::from(""));
        lines.push(widgets::hint_line(&[("y", "yes"), ("n", "no")], text_width).centered());
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Greedy word wrap by display width.
pub fn textwrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let needed = console::measure_text_width(&current)
            + usize::from(!current.is_empty())
            + console::measure_text_width(word);
        if !current.is_empty() && needed > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines.into_iter().map(|l| widgets::fit(&l, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::textwrap;

    #[test]
    fn wraps_on_words_within_width() {
        let lines = textwrap("one two three four five", 9);
        assert_eq!(lines, vec!["one two", "three", "four five"]);
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut() {
        let lines = textwrap("abcdefghijkl", 5);
        assert_eq!(console::measure_text_width(&lines[0]), 5);
    }
}
