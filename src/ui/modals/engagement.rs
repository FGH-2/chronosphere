use crate::app::{App, EngagementModal, Modal};
use crate::ui::centered_rect;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let r = centered_rect(area, 60, 60);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" engagements ", Theme::accent_bold()))
        .border_style(Theme::border_active())
        .style(Theme::panel());

    let modal = match &app.modal {
        Modal::Engagement(m) => m,
        _ => return,
    };

    if let Some(prompt) = &modal.new_name_prompt {
        let p = Paragraph::new(vec![
            Line::from(Span::styled(
                "new engagement name:".to_string(),
                Theme::accent_bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(format!("  {}_", prompt), Theme::accent_bold())),
            Line::from(""),
            Line::from(Span::styled(
                "Enter to confirm, Esc to cancel".to_string(),
                Theme::muted(),
            )),
        ])
        .block(block);
        f.render_widget(p, r);
        return;
    }

    let items: Vec<ListItem> = modal
        .available
        .iter()
        .map(|name| {
            let is_active = app
                .engagement
                .as_ref()
                .map(|e| &e.meta.name == name)
                .unwrap_or(false);
            let label = if is_active {
                format!("● {} (active)", name)
            } else {
                format!("  {}", name)
            };
            let style = if is_active {
                Theme::accent_bold()
            } else {
                Theme::accent()
            };
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(modal.cursor.min(items.len() - 1)));
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::selected())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, r, &mut state);

    let _ = EngagementModal::default;
}
