use crate::app::App;
use crate::ui::centered_rect;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let r = centered_rect(area, 60, 70);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" tools on $PATH ", Theme::accent_bold()))
        .border_style(Theme::border_active())
        .style(Theme::panel());

    let mut tools: Vec<String> = app.library.all_tools_referenced().into_iter().collect();
    tools.sort();
    let items: Vec<ListItem> = tools
        .iter()
        .map(|t| {
            let (icon, style) = if which::which(t).is_ok() {
                ("✓", Theme::success())
            } else {
                ("✗", Theme::error())
            };
            ListItem::new(Line::from(vec![
                Span::styled(icon.to_string(), style),
                Span::raw("  "),
                Span::raw(t.clone()),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::selected())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, r, &mut state);
}
