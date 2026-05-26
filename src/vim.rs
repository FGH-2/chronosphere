use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    SearchGlobal,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Search => "SEARCH",
            Mode::SearchGlobal => "SEARCH-ALL",
        }
    }
}

/// A pre-parsed user intent emitted from `KeyParser`. The app layer maps these to mutations.
#[derive(Debug, Clone)]
pub enum Action {
    None,
    Quit,
    // navigation
    FocusLeft,
    FocusRight,
    Down(u32),
    Up(u32),
    PageDown,
    PageUp,
    Top,
    Bottom,
    // commands
    Run { count: u32 },
    RunAllVisible,
    YankResolved,
    YankRaw,
    EditInline,
    ToggleSelect,
    ClearSelection,
    OpenActiveJob,
    OpenJobLog,
    NextJob,
    PrevJob,
    KillJob,
    RepeatLast,
    // modal entry
    EnterCommandMode,
    EnterSearchMode,
    EnterSearchGlobalMode,
    OpenHelp,
    SetMark(char),
    JumpMark(char),
    // misc
    Refresh,
    Tick,
}

/// Tiny multi-key parser that buffers count prefix + multi-char sequences (gg, dj, m{x}, '{x}, ]j, [j).
pub struct KeyParser {
    pub pending: String,
    pub count: Option<u32>,
}

impl KeyParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            count: None,
        }
    }

    pub fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
    }

    pub fn feed_normal(&mut self, ev: KeyEvent) -> Action {
        if ev.modifiers.contains(KeyModifiers::CONTROL) {
            self.reset();
            return match ev.code {
                KeyCode::Char('d') => Action::PageDown,
                KeyCode::Char('u') => Action::PageUp,
                KeyCode::Char('c') => Action::ClearSelection,
                KeyCode::Char('r') => Action::Refresh,
                _ => Action::None,
            };
        }

        match ev.code {
            KeyCode::Esc => {
                self.reset();
                Action::ClearSelection
            }
            KeyCode::Char(ch) => self.feed_char(ch),
            KeyCode::Enter => {
                let count = self.count.take().unwrap_or(1);
                self.pending.clear();
                Action::Run { count }
            }
            KeyCode::Tab => {
                self.reset();
                Action::FocusRight
            }
            KeyCode::BackTab => {
                self.reset();
                Action::FocusLeft
            }
            KeyCode::Left => {
                self.reset();
                Action::FocusLeft
            }
            KeyCode::Right => {
                self.reset();
                Action::FocusRight
            }
            KeyCode::Down => {
                let count = self.count.take().unwrap_or(1);
                Action::Down(count)
            }
            KeyCode::Up => {
                let count = self.count.take().unwrap_or(1);
                Action::Up(count)
            }
            _ => Action::None,
        }
    }

    fn feed_char(&mut self, ch: char) -> Action {
        // count prefix: 1..=9 starts, then 0..=9 continues. A bare `0` with no count goes to "first col" but we
        // don't have columns; treat `0` without active count as no-op.
        if ch.is_ascii_digit() {
            if self.count.is_none() && ch == '0' {
                return Action::None;
            }
            let digit = ch.to_digit(10).unwrap();
            self.count = Some(self.count.unwrap_or(0) * 10 + digit);
            return Action::None;
        }

        // multi-char sequences: gg, dj, m{x}, '{x}, ]j, [j
        if !self.pending.is_empty() {
            let seq = format!("{}{}", self.pending, ch);
            self.pending.clear();
            return match seq.as_str() {
                "gg" => {
                    self.count = None;
                    Action::Top
                }
                "dj" => {
                    self.count = None;
                    Action::KillJob
                }
                "]j" => Action::NextJob,
                "[j" => Action::PrevJob,
                s if s.starts_with('m') && s.len() == 2 => {
                    let mk = s.chars().nth(1).unwrap();
                    self.count = None;
                    Action::SetMark(mk)
                }
                s if s.starts_with('\'') && s.len() == 2 => {
                    let mk = s.chars().nth(1).unwrap();
                    self.count = None;
                    Action::JumpMark(mk)
                }
                s if s.starts_with('g') && s.len() == 2 && s.chars().nth(1) == Some('/') => {
                    self.count = None;
                    Action::EnterSearchGlobalMode
                }
                _ => Action::None,
            };
        }

        match ch {
            'h' => Action::FocusLeft,
            'l' => Action::FocusRight,
            'j' => {
                let count = self.count.take().unwrap_or(1);
                Action::Down(count)
            }
            'k' => {
                let count = self.count.take().unwrap_or(1);
                Action::Up(count)
            }
            'G' => {
                self.count = None;
                Action::Bottom
            }
            'g' | 'd' | 'm' | '\'' | ']' | '[' => {
                self.pending.push(ch);
                Action::None
            }
            'r' => {
                let count = self.count.take().unwrap_or(1);
                Action::Run { count }
            }
            'R' => {
                self.count = None;
                Action::RunAllVisible
            }
            'y' => Action::YankResolved,
            'Y' => Action::YankRaw,
            'e' => Action::EditInline,
            ' ' => Action::ToggleSelect,
            'o' => Action::OpenActiveJob,
            'L' => Action::OpenJobLog,
            '.' => Action::RepeatLast,
            ':' => Action::EnterCommandMode,
            '/' => Action::EnterSearchMode,
            '?' => Action::OpenHelp,
            'q' => Action::Quit,
            _ => Action::None,
        }
    }
}

impl Default for KeyParser {
    fn default() -> Self {
        Self::new()
    }
}
