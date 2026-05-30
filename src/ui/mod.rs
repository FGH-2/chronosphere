pub mod modals;
pub mod panels;
pub mod splash;
pub mod theme;

use crate::app::{App, Focus, Modal};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Splash screen takes over the whole frame until the user dismisses it.
    if let Some(splash) = app.splash.as_ref() {
        splash::draw(f, area, splash);
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body = root[0];
    let status = root[1];

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Percentage(44),
            Constraint::Min(20),
        ])
        .split(body);

    panels::categories::render(f, columns[0], app);
    panels::commands::render(f, columns[1], app);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[2]);

    panels::preview::render(f, right[0], app);
    panels::jobs::render(f, right[1], app);

    panels::status_bar::render(f, status, app);

    match &app.modal {
        Modal::None => {}
        Modal::Help => modals::help::render(f, area, app),
        Modal::Engagement(_) => modals::engagement::render(f, area, app),
        Modal::Target(_) => modals::target::render(f, area, app),
        Modal::Ap(_) => modals::ap::render(f, area, app),
        Modal::Creds(_) => modals::creds::render(f, area, app),
        Modal::Variables(_) => modals::variables::render(f, area, app),
        Modal::Tools(_) => modals::tools::render(f, area, app),
        Modal::Search { .. } => modals::search::render(f, area, app),
        Modal::Edit(_) => modals::edit::render(f, area, app),
        Modal::JobLog(_) => modals::job_log::render(f, area, app),
    }

    let _ = Focus::Categories; // keep import alive
    let _ = Rect::default;
}

pub fn centered_rect(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(v[1]);
    h[1]
}
