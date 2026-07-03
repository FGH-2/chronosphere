use crate::app::Modal;
use ratatui::layout::{Position, Rect};

/// Interactive bounds for a list panel, updated each frame during render.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListRegion {
    pub panel: Rect,
    pub list_inner: Rect,
    pub list_offset: usize,
}

impl ListRegion {
    /// Inner area of a `Block` with `Borders::ALL`.
    pub fn block_inner(area: Rect) -> Rect {
        Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    }

    pub fn contains_panel(&self, column: u16, row: u16) -> bool {
        self.panel.contains(Position { x: column, y: row })
    }

    pub fn contains_list(&self, column: u16, row: u16) -> bool {
        self.list_inner.contains(Position { x: column, y: row })
    }

    /// Map a click to a list index, accounting for scroll offset.
    pub fn list_index_at(&self, column: u16, row: u16, item_count: usize) -> Option<usize> {
        if item_count == 0 {
            return None;
        }
        let inner = self.list_inner;
        if column < inner.x || column >= inner.x.saturating_add(inner.width) {
            return None;
        }
        if row < inner.y || row >= inner.y.saturating_add(inner.height) {
            return None;
        }
        let rel = (row - inner.y) as usize;
        let index = self.list_offset.saturating_add(rel);
        (index < item_count).then_some(index)
    }
}

/// Scrollable text viewport (preview, help body, job log, CVE detail).
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollRegion {
    pub area: Rect,
    pub visible_lines: usize,
}

impl ScrollRegion {
    pub fn from_block(area: Rect) -> Self {
        let inner = ListRegion::block_inner(area);
        Self {
            area: inner,
            visible_lines: inner.height.max(1) as usize,
        }
    }

    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.area.contains(Position { x: column, y: row })
    }
}

pub fn scroll_by(current: usize, delta: i32, max: usize) -> usize {
    if delta >= 0 {
        current.saturating_add(delta as usize).min(max)
    } else {
        current.saturating_sub((-delta) as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAction {
    Engagement,
    Target,
    Ap,
    Pivot,
    Creds,
    Jobs,
}

#[derive(Debug, Clone, Copy)]
pub struct StatusChipHit {
    pub area: Rect,
    pub action: StatusBarAction,
}

#[derive(Debug, Clone, Default)]
pub struct StatusBarHits {
    pub bar: Rect,
    pub chips: Vec<StatusChipHit>,
}

impl StatusBarHits {
    pub fn chip_at(&self, column: u16, row: u16) -> Option<StatusBarAction> {
        if !self.bar.contains(Position { x: column, y: row }) {
            return None;
        }
        self.chips
            .iter()
            .find(|c| c.area.contains(Position { x: column, y: row }))
            .map(|c| c.action)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EditHitRegions {
    pub textarea_panel: Rect,
    pub textarea: ScrollRegion,
    pub suggestions: Option<ListRegion>,
}

/// Last-frame layout snapshot for mouse hit-testing.
#[derive(Debug, Clone, Default)]
pub struct HitRegions {
    pub frame: Rect,
    pub categories: ListRegion,
    pub commands: ListRegion,
    pub preview: ScrollRegion,
    pub jobs: ListRegion,
    pub status_bar: StatusBarHits,
    pub edit: Option<EditHitRegions>,
    pub modal_popup: Option<Rect>,
    pub help_scroll: Option<ScrollRegion>,
    pub job_log_scroll: Option<ScrollRegion>,
    pub search_list: Option<ListRegion>,
    pub cve_list: Option<ListRegion>,
    pub cve_detail_scroll: Option<ScrollRegion>,
    pub engagement_list: Option<ListRegion>,
    pub target_list: Option<ListRegion>,
    pub ap_list: Option<ListRegion>,
    pub pivot_list: Option<ListRegion>,
    pub creds_list: Option<ListRegion>,
    pub variables_list: Option<ListRegion>,
}

impl HitRegions {
    pub fn modal_popup_rect(frame: Rect, modal: &Modal) -> Option<Rect> {
        let (pct_x, pct_y) = match modal {
            Modal::None => return None,
            Modal::Help(_) => (70, 80),
            Modal::Engagement(_) => (60, 60),
            Modal::Target(_) => (70, 70),
            Modal::Ap(_) => (75, 75),
            Modal::Pivot(_) => (80, 78),
            Modal::Creds(_) => (70, 70),
            Modal::Variables(_) => (75, 75),
            Modal::Tools(_) => (60, 70),
            Modal::Search { .. } => (80, 70),
            Modal::Edit(_) => (80, 55),
            Modal::JobLog(_) => (88, 85),
            Modal::Cve(_) => (90, 85),
        };
        Some(super::centered_rect(frame, pct_x, pct_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_index_respects_offset_and_bounds() {
        let region = ListRegion {
            panel: Rect::new(0, 0, 10, 10),
            list_inner: Rect::new(1, 1, 8, 5),
            list_offset: 3,
        };
        assert_eq!(region.list_index_at(2, 1, 10), Some(3));
        assert_eq!(region.list_index_at(2, 3, 10), Some(5));
        assert_eq!(region.list_index_at(2, 6, 10), None);
        assert_eq!(region.list_index_at(0, 2, 10), None);
        assert_eq!(region.list_index_at(2, 3, 5), None);
    }

    #[test]
    fn scroll_by_clamps() {
        assert_eq!(scroll_by(2, 5, 4), 4);
        assert_eq!(scroll_by(2, -3, 4), 0);
        assert_eq!(scroll_by(2, 1, 4), 3);
    }
}
