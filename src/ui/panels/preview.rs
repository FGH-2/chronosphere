use crate::app::{App, Focus};
use crate::ui::layout::ListRegion;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn preview_line_count(app: &App) -> usize {
    build_preview_lines(app).len()
}

pub fn max_preview_scroll(app: &App, visible_lines: usize) -> usize {
    preview_line_count(app).saturating_sub(visible_lines.max(1))
}

pub fn clamp_preview_scroll(app: &mut App, visible_lines: usize) {
    app.preview_scroll = app
        .preview_scroll
        .min(max_preview_scroll(app, visible_lines));
}

fn build_preview_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    match app.current_command() {
        Some(cmd) => {
            let (resolved, unresolved) = app.render_command_preview(&cmd);
            lines.push(Line::from(vec![Span::styled(
                cmd.title.clone(),
                Theme::accent_bold(),
            )]));
            if let Some(desc) = &cmd.description {
                lines.push(Line::from(vec![Span::styled(desc.clone(), Theme::muted())]));
            }
            lines.push(Line::from(""));
            for chunk in resolved.split('\n') {
                lines.push(Line::from(vec![Span::styled(
                    chunk.to_string(),
                    Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD),
                )]));
            }
            lines.push(Line::from(""));
            if !cmd.tags.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("tags: ", Theme::muted()),
                    Span::styled(cmd.tags.join(", "), Theme::magenta()),
                ]));
            }
            if !cmd.requires.is_empty() {
                let mut parts = vec![Span::styled("requires: ", Theme::muted())];
                for (i, t) in cmd.requires.iter().enumerate() {
                    if i > 0 {
                        parts.push(Span::raw(", "));
                    }
                    let style = if which::which(t).is_ok() {
                        Theme::success()
                    } else {
                        Theme::error()
                    };
                    parts.push(Span::styled(t.clone(), style));
                }
                lines.push(Line::from(parts));
            }
            if !unresolved.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("unresolved: ", Theme::warn()),
                    Span::styled(unresolved.join(", "), Theme::warn()),
                ]));
            }
            if cmd.interactive {
                lines.push(Line::from(vec![Span::styled(
                    "interactive — will spawn an external terminal",
                    Theme::warn(),
                )]));
            }
        }
        None => {
            lines.push(Line::from(Span::styled(
                "no command selected".to_string(),
                Theme::muted(),
            )));
        }
    }

    lines
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Preview;
    let border_style = if is_focused {
        Theme::border_active()
    } else {
        Theme::border()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" preview ", Theme::accent_bold()))
        .border_style(border_style)
        .style(Theme::panel());

    let visible_lines = ListRegion::block_inner(area).height.max(1) as usize;
    app.preview_visible_lines = visible_lines;
    clamp_preview_scroll(app, visible_lines);

    let lines = build_preview_lines(app);
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll as u16, 0));
    f.render_widget(p, area);
}
