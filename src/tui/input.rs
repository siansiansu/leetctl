//! A single-line text field for the search and tag prompts
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A readline-ish one-line editor. The cursor is a byte offset, always on a char boundary.
#[derive(Debug, Default, Clone)]
pub(crate) struct Input {
    value: String,
    cursor: usize,
}

impl Input {
    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    /// Applies a key, reporting whether it was consumed so the caller can treat the rest as
    /// commands.
    pub(crate) fn handle(&mut self, k: &KeyEvent) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Char('u') if ctrl => {
                self.value.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => self.delete_word_back(),
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.value.len(),
            KeyCode::Char(c) if !ctrl && !k.modifiers.contains(KeyModifiers::ALT) => {
                self.value.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            KeyCode::Backspace => {
                if let Some((i, _)) = self.value[..self.cursor].char_indices().next_back() {
                    self.value.remove(i);
                    self.cursor = i;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if let Some((i, _)) = self.value[..self.cursor].char_indices().next_back() {
                    self.cursor = i;
                }
            }
            KeyCode::Right => {
                if let Some(c) = self.value[self.cursor..].chars().next() {
                    self.cursor += c.len_utf8();
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.value.len(),
            _ => return false,
        }

        true
    }

    /// Splits the value for rendering into text before the cursor, the character under it, and the
    /// text after. The caller styles the middle one reversed.
    pub(crate) fn render_parts(&self) -> (&str, String, &str) {
        let before = &self.value[..self.cursor];
        match self.value[self.cursor..].chars().next() {
            Some(c) => {
                let end = self.cursor + c.len_utf8();
                (before, c.to_string(), &self.value[end..])
            }
            None => (before, " ".to_string(), ""),
        }
    }

    fn delete_word_back(&mut self) {
        let before = &self.value[..self.cursor];
        let trimmed = before.trim_end();
        let cut = trimmed
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        self.value.replace_range(cut..self.cursor, "");
        self.cursor = cut;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn typed(text: &str) -> Input {
        let mut input = Input::default();
        for c in text.chars() {
            input.handle(&key(KeyCode::Char(c)));
        }
        input
    }

    #[test]
    fn typing_and_backspace_edit_at_the_cursor() {
        let mut input = typed("two sum");
        assert_eq!(input.value(), "two sum");

        input.handle(&key(KeyCode::Backspace));
        assert_eq!(input.value(), "two su");

        input.handle(&key(KeyCode::Home));
        input.handle(&key(KeyCode::Char('!')));
        assert_eq!(input.value(), "!two su");
    }

    #[test]
    fn ctrl_u_clears_to_the_start_and_ctrl_w_deletes_a_word() {
        let mut input = typed("longest common prefix");
        input.handle(&ctrl('w'));
        assert_eq!(input.value(), "longest common ");

        input.handle(&ctrl('u'));
        assert_eq!(input.value(), "");
    }

    #[test]
    fn arrows_move_over_multibyte_characters() {
        let mut input = typed("兩數之和");

        input.handle(&key(KeyCode::Left));
        input.handle(&key(KeyCode::Char('x')));
        assert_eq!(input.value(), "兩數之x和");

        input.handle(&key(KeyCode::Right));
        input.handle(&key(KeyCode::Backspace));
        assert_eq!(input.value(), "兩數之x");
    }

    #[test]
    fn unhandled_keys_are_left_to_the_caller() {
        let mut input = typed("abc");

        assert!(!input.handle(&key(KeyCode::Enter)));
        assert!(!input.handle(&key(KeyCode::Esc)));
        assert!(!input.handle(&key(KeyCode::Tab)));
        assert_eq!(input.value(), "abc");
    }

    #[test]
    fn render_parts_marks_the_character_under_the_cursor() {
        let mut input = typed("ab");
        assert_eq!(input.render_parts(), ("ab", " ".to_string(), ""));

        input.handle(&key(KeyCode::Left));
        assert_eq!(input.render_parts(), ("a", "b".to_string(), ""));
    }
}
