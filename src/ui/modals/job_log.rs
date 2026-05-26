use crate::app::{App, JobLogModal, Modal};
use crate::engagement::{JobRecord, JobStatus};
use crate::ui::centered_rect;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::fs;

pub fn load_log_lines(job: &JobRecord) -> Vec<String> {
    match job.log_path.as_ref() {
        Some(p) if p.exists() => match fs::read_to_string(p) {
            Ok(body) if body.is_empty() => vec!["(log file is empty)".to_string()],
            Ok(body) => body.lines().map(String::from).collect(),
            Err(err) => vec![format!("(failed to read log: {})", err)],
        },
        Some(p) => vec![format!("(log not found: {})", p.display())],
        None => vec!["(no log path for this job)".to_string()],
    }
}

pub fn sync_modal_from_job(modal: &mut JobLogModal, job: Option<&JobRecord>, visible_lines: usize) {
    if let Some(job) = job {
        modal.title = job.command_title.clone();
        modal.status = job.status;
        modal.exit_code = job.exit_code;
        modal.lines = load_log_lines(job);
    }
    let max_scroll = modal.lines.len().saturating_sub(visible_lines.max(1));
    if modal.follow {
        modal.scroll = max_scroll;
    } else {
        modal.scroll = modal.scroll.min(max_scroll);
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let Modal::JobLog(modal) = &mut app.modal else {
        return;
    };

    let popup = centered_rect(area, 88, 85);
    f.render_widget(Clear, popup);

    let status_label = format!("{:?}", modal.status).to_lowercase();
    let follow = if modal.follow { "follow" } else { "scroll" };
    let title = format!(
        " job: {}  [{}]  {}{} ",
        truncate(&modal.title, 40),
        status_label,
        follow,
        modal
            .exit_code
            .map(|c| format!("  exit={c}"))
            .unwrap_or_default()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, Theme::accent_bold()))
        .border_style(Theme::border_active())
        .style(Theme::panel());
    f.render_widget(block.clone(), popup);
    let inner = block.inner(popup);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let visible_lines = layout[0].height.saturating_sub(2) as usize;
    let job = app.jobs.iter().find(|j| j.id == modal.job_id);
    sync_modal_from_job(modal, job, visible_lines);

    let width = layout[0].width.saturating_sub(2) as usize;
    let body: Vec<Line> = modal
        .lines
        .iter()
        .skip(modal.scroll)
        .take(visible_lines.max(1))
        .map(|line| Line::from(truncate(line, width)))
        .collect();

    let empty = body.is_empty();
    let paragraph = Paragraph::new(if empty {
        vec![Line::from(Span::styled(
            "(no output yet)",
            Theme::muted(),
        ))]
    } else {
        body
    })
    .style(Style::default())
    .wrap(Wrap { trim: false });
    f.render_widget(paragraph, layout[0]);

    let position = if modal.lines.is_empty() {
        "0/0".to_string()
    } else {
        let end = (modal.scroll + visible_lines.max(1)).min(modal.lines.len());
        format!("{}/{} lines", end, modal.lines.len())
    };

    let hints = Line::from(vec![
        Span::styled(" j/k ", Theme::magenta()),
        Span::raw("scroll  "),
        Span::styled("Ctrl-d/u", Theme::magenta()),
        Span::raw(" page  "),
        Span::styled("g/G", Theme::magenta()),
        Span::raw(" top/bottom  "),
        Span::styled("f", Theme::magenta()),
        Span::raw(" follow  "),
        Span::styled("o", Theme::magenta()),
        Span::raw(" tmux  "),
        Span::styled("Esc", Theme::magenta()),
        Span::raw(" close  "),
        Span::styled(position, Theme::muted().add_modifier(Modifier::ITALIC)),
    ]);
    f.render_widget(Paragraph::new(hints).style(Theme::muted()), layout[1]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::JobLogModal;

    #[test]
    fn follow_mode_scrolls_to_bottom() {
        let mut modal = JobLogModal {
            job_id: "j".into(),
            title: "scan".into(),
            status: JobStatus::Running,
            exit_code: None,
            lines: (1..=20).map(|i| format!("line{i}")).collect(),
            scroll: 0,
            follow: true,
        };
        sync_modal_from_job(&mut modal, None, 10);
        assert_eq!(modal.scroll, 10);
    }

    #[test]
    fn manual_scroll_clamped() {
        let mut modal = JobLogModal {
            job_id: "j".into(),
            title: "scan".into(),
            status: JobStatus::Completed,
            exit_code: Some(0),
            lines: vec!["a".into(), "b".into(), "c".into()],
            scroll: 99,
            follow: false,
        };
        sync_modal_from_job(&mut modal, None, 2);
        assert_eq!(modal.scroll, 1);
    }
}
