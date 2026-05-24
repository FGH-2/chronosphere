use crate::app::{App, Modal};
use crate::ui::centered_rect;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let r = centered_rect(area, 80, 50);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" edit before running ", Theme::accent_bold()))
        .border_style(Theme::border_active())
        .style(Theme::panel());
    f.render_widget(block.clone(), r);
    let inner = block.inner(r);

    let modal = match &app.modal {
        Modal::Edit(m) => m,
        _ => return,
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2), Constraint::Length(1)])
        .split(inner);

    f.render_widget(&modal.textarea, layout[0]);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl-S", Theme::magenta()),
        Span::raw(" run  "),
        Span::styled("Ctrl-W", Theme::magenta()),
        Span::raw(" save as new command id  "),
        Span::styled("Esc", Theme::magenta()),
        Span::raw(" cancel"),
    ]))
    .style(Theme::muted());
    f.render_widget(hints, layout[1]);

    if let Some(prompt) = &modal.save_as_prompt {
        let line = Paragraph::new(Line::from(vec![
            Span::styled(" save as id: ", Theme::accent_bold()),
            Span::styled(prompt.clone(), Theme::accent_bold()),
            Span::styled("_", Theme::accent_bold()),
        ]))
        .style(Theme::panel());
        f.render_widget(line, layout[2]);
    }
}
