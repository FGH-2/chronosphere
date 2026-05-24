use crate::app::App;
use crate::ui::centered_rect;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

const HELP: &[(&str, &str)] = &[
    ("h / l", "focus left / right panel"),
    ("j / k", "move down / up"),
    ("gg / G", "top / bottom"),
    ("Ctrl-d / Ctrl-u", "half-page down / up"),
    ("Enter / r", "run highlighted command in background tmux window"),
    ("Nr (e.g. 5r)", "run command N times in parallel"),
    ("space", "toggle multi-select on highlighted command"),
    ("R", "run all multi-selected (or all visible in category)"),
    ("y / Y", "yank resolved / raw template to clipboard"),
    ("e", "inline edit the resolved command before running"),
    ("o", "focus the active job's tmux window"),
    ("]j / [j", "next / previous job"),
    ("dj", "kill the active job"),
    (".", "repeat last action"),
    ("/", "fuzzy search within current category"),
    ("g/", "fuzzy search across all commands"),
    (":", "command palette"),
    ("? / :help", "show this help"),
    ("Esc / Ctrl-c", "clear selection / dismiss modal"),
    ("q / :q", "quit"),
    ("", ""),
    (":engagement", "list/switch/new engagement"),
    (":target", "list/edit/switch active target"),
    (":creds", "list/edit/switch active credential profile"),
    (":tools", "show which referenced tools are on $PATH"),
    (":reload", "force-reload command library from disk"),
    (":write", "save inline-edited command as new id in engagement overrides"),
];

pub fn render(f: &mut Frame, area: Rect, _app: &App) {
    let r = centered_rect(area, 70, 80);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" help ", Theme::accent_bold()))
        .border_style(Theme::border_active())
        .style(Theme::panel());

    let mut lines: Vec<Line> = Vec::with_capacity(HELP.len() + 4);
    lines.push(Line::from(Span::styled(
        "chronosphere — vim keymap".to_string(),
        Theme::accent_bold(),
    )));
    lines.push(Line::from(""));
    for (k, d) in HELP {
        if k.is_empty() && d.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", k), Theme::magenta()),
            Span::raw(" "),
            Span::raw(d.to_string()),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "press Esc or ? to close".to_string(),
        Theme::muted(),
    )));

    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, r);
}
