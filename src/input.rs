//! Normalized text-editing key handling across terminals (Linux/macOS/SSH/tmux).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEditAction {
    Backspace,
    DeleteWord,
    ClearLine,
    Insert(char),
    None,
}

/// Map a key event to a text-editing action.
pub fn text_edit_action(ke: &KeyEvent) -> TextEditAction {
    let ctrl = ke.modifiers.contains(KeyModifiers::CONTROL);
    let shift = ke.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl && !shift {
        match ke.code {
            KeyCode::Char('h' | 'H') | KeyCode::Char('\x08') | KeyCode::Backspace => {
                return TextEditAction::Backspace;
            }
            KeyCode::Char('w' | 'W') => return TextEditAction::DeleteWord,
            KeyCode::Char('u' | 'U') => return TextEditAction::ClearLine,
            _ => {}
        }
    }

    match ke.code {
        KeyCode::Backspace | KeyCode::Delete => return TextEditAction::Backspace,
        KeyCode::Char('\x7f') | KeyCode::Char('\x08') => return TextEditAction::Backspace,
        KeyCode::Char('h' | 'H') if ctrl => return TextEditAction::Backspace,
        _ => {}
    }

    if let Some(c) = text_char(ke) {
        return TextEditAction::Insert(c);
    }

    TextEditAction::None
}

fn text_char(ke: &KeyEvent) -> Option<char> {
    match ke.code {
        KeyCode::Char(c)
            if !ke.modifiers.contains(KeyModifiers::CONTROL)
                && !ke.modifiers.contains(KeyModifiers::ALT)
                && c != '\x7f'
                && c != '\x08' =>
        {
            Some(c)
        }
        _ => None,
    }
}

pub fn apply_to_string(s: &mut String, action: TextEditAction) {
    match action {
        TextEditAction::Backspace => {
            s.pop();
        }
        TextEditAction::DeleteWord => {
            pop_word(s);
        }
        TextEditAction::ClearLine => {
            s.clear();
        }
        TextEditAction::Insert(c) => {
            s.push(c);
        }
        TextEditAction::None => {}
    }
}

pub fn apply_to_textarea(ta: &mut TextArea<'static>, action: TextEditAction) {
    match action {
        TextEditAction::Backspace => {
            ta.delete_char();
        }
        TextEditAction::DeleteWord => {
            ta.delete_word();
        }
        TextEditAction::ClearLine => {
            ta.delete_line_by_head();
        }
        TextEditAction::Insert(c) => {
            ta.insert_char(c);
        }
        TextEditAction::None => {}
    }
}

fn pop_word(s: &mut String) {
    while s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() {
        return;
    }
    if let Some(i) = s
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
    {
        s.truncate(i);
    } else {
        s.clear();
    }
    while s.ends_with(' ') {
        s.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn del_char_is_backspace() {
        assert_eq!(
            text_edit_action(&key(KeyCode::Char('\x7f'), KeyModifiers::empty())),
            TextEditAction::Backspace
        );
    }

    #[test]
    fn ctrl_h_is_backspace() {
        assert_eq!(
            text_edit_action(&key(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            TextEditAction::Backspace
        );
    }

    #[test]
    fn delete_key_is_backspace() {
        assert_eq!(
            text_edit_action(&key(KeyCode::Delete, KeyModifiers::empty())),
            TextEditAction::Backspace
        );
    }

    #[test]
    fn ctrl_w_deletes_word() {
        let mut s = "hello world".to_string();
        apply_to_string(&mut s, TextEditAction::DeleteWord);
        assert_eq!(s, "hello");
    }
}
