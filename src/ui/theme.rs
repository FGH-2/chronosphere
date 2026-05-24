use ratatui::style::{Color, Modifier, Style};

/// Tokyo Night Storm-inspired palette, blue-forward.
pub struct Theme;

impl Theme {
    pub const BG: Color = Color::Rgb(0x24, 0x28, 0x3b);
    pub const PANEL_BG: Color = Color::Rgb(0x1f, 0x23, 0x35);
    pub const FG: Color = Color::Rgb(0xc0, 0xca, 0xf5);
    pub const MUTED: Color = Color::Rgb(0x56, 0x5f, 0x89);
    pub const ACCENT: Color = Color::Rgb(0x7a, 0xa2, 0xf7);
    pub const ACCENT_BRIGHT: Color = Color::Rgb(0x2a, 0xc3, 0xde);
    pub const SELECT_BG: Color = Color::Rgb(0x36, 0x4a, 0x82);
    pub const MULTI_BG: Color = Color::Rgb(0x21, 0x3d, 0x82);
    pub const BORDER: Color = Color::Rgb(0x3d, 0x59, 0xa1);
    pub const BORDER_ACTIVE: Color = Color::Rgb(0x7a, 0xa2, 0xf7);
    pub const SUCCESS: Color = Color::Rgb(0x9e, 0xce, 0x6a);
    pub const WARN: Color = Color::Rgb(0xe0, 0xaf, 0x68);
    pub const ERROR: Color = Color::Rgb(0xf7, 0x76, 0x8e);
    pub const MAGENTA: Color = Color::Rgb(0xbb, 0x9a, 0xf7);

    pub fn base() -> Style {
        Style::default().fg(Self::FG).bg(Self::BG)
    }
    pub fn panel() -> Style {
        Style::default().fg(Self::FG).bg(Self::PANEL_BG)
    }
    pub fn muted() -> Style {
        Style::default().fg(Self::MUTED).bg(Self::PANEL_BG)
    }
    pub fn accent() -> Style {
        Style::default().fg(Self::ACCENT).bg(Self::PANEL_BG)
    }
    pub fn accent_bold() -> Style {
        Self::accent().add_modifier(Modifier::BOLD)
    }
    pub fn border() -> Style {
        Style::default().fg(Self::BORDER).bg(Self::PANEL_BG)
    }
    pub fn border_active() -> Style {
        Style::default()
            .fg(Self::BORDER_ACTIVE)
            .bg(Self::PANEL_BG)
            .add_modifier(Modifier::BOLD)
    }
    pub fn selected() -> Style {
        Style::default()
            .fg(Self::FG)
            .bg(Self::SELECT_BG)
            .add_modifier(Modifier::BOLD)
    }
    pub fn multi_selected() -> Style {
        Style::default().fg(Self::FG).bg(Self::MULTI_BG)
    }
    pub fn success() -> Style {
        Style::default().fg(Self::SUCCESS).bg(Self::PANEL_BG)
    }
    pub fn warn() -> Style {
        Style::default().fg(Self::WARN).bg(Self::PANEL_BG)
    }
    pub fn error() -> Style {
        Style::default().fg(Self::ERROR).bg(Self::PANEL_BG)
    }
    pub fn magenta() -> Style {
        Style::default().fg(Self::MAGENTA).bg(Self::PANEL_BG)
    }
}
