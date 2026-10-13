use crossterm::event::KeyCode;

/// A one-line text field. Characters are kept as `char`s, not bytes, so the
/// cursor never lands inside a multi-byte character — typing "cá nhân" used to
/// panic the moment the cursor passed the "á".
#[derive(Default)]
pub struct InputField {
    chars: Vec<char>,
    cursor: usize,
}

impl InputField {
    pub fn with_value(value: &str) -> Self {
        let chars: Vec<char> = value.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The characters, for rendering.
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right if self.cursor < self.chars.len() => self.cursor += 1,
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(field: &mut InputField, text: &str) {
        for c in text.chars() {
            field.handle_key(KeyCode::Char(c));
        }
    }

    #[test]
    fn typing_accented_text_does_not_panic() {
        let mut field = InputField::default();
        type_str(&mut field, "cá nhân");
        assert_eq!(field.value(), "cá nhân");
        assert_eq!(field.cursor(), 7);
    }

    #[test]
    fn editing_in_the_middle_of_multibyte_text() {
        let mut field = InputField::default();
        type_str(&mut field, "ệệ");
        field.handle_key(KeyCode::Left);
        field.handle_key(KeyCode::Char('x'));
        assert_eq!(field.value(), "ệxệ");
        field.handle_key(KeyCode::Backspace);
        field.handle_key(KeyCode::Delete);
        assert_eq!(field.value(), "ệ");
    }

    #[test]
    fn cursor_stays_in_bounds() {
        let mut field = InputField::with_value("ab");
        field.handle_key(KeyCode::Right);
        assert_eq!(field.cursor(), 2);
        field.handle_key(KeyCode::Home);
        field.handle_key(KeyCode::Backspace);
        field.handle_key(KeyCode::Left);
        assert_eq!(field.cursor(), 0);
        assert_eq!(field.value(), "ab");
    }
}
