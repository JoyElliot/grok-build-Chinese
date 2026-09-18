//! Helpers shared by the dashboard render and chrome test modules.

use ratatui::buffer::Buffer;
use unicode_width::UnicodeWidthStr;

use super::row::DashboardRow;
use super::state::{DashboardRowId, RowState};

/// Read visible text row-by-row, skipping wide-glyph continuation cells.
pub(super) fn buf_to_text(buf: &Buffer) -> String {
    let mut content = String::new();
    for y in buf.area.top()..buf.area.bottom() {
        let mut skip = 0usize;
        for x in buf.area.left()..buf.area.right() {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let symbol = cell.symbol();
            if skip == 0 {
                content.push_str(symbol);
            }
            skip = skip.max(symbol.width()).saturating_sub(1);
        }
        content.push('\n');
    }
    content
}

/// A top-level dashboard row with defaults, for chrome and render tests.
pub(super) fn header_test_row(id: u32, state: RowState, label: &str) -> DashboardRow {
    use crate::app::agent::AgentId;
    DashboardRow {
        id: DashboardRowId::TopLevel(AgentId(id as usize)),
        session_id: None,
        label: label.to_string(),
        subtitle: None,
        state,
        activity: None,
        secondary_line: None,
        cwd_display: String::new(),
        cwd: std::path::PathBuf::from("/tmp"),
        last_change_at: std::time::SystemTime::now(),
        pinned: false,
        badges: Vec::new(),
        context_pct: None,
        indent: 0,
        parent_label: None,
        is_more_placeholder: false,
        more_count: 0,
    }
}
