//! Tabbed usage and session-info modal, opened by `/usage`, `/session-info`, `/context`, and the context-bar click.
//! Minimal mode keeps the scrollback blocks instead; this modal never opens there.
//!
//! The Session-info tab supports click-to-copy on value rows (with hover) and in-app drag-select after a movement threshold.
//! `c` and `y` stay as programmatic copy.
//! The modal opens with loading placeholders; the task-result handlers fill the slots in as the fetches land.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::views::drag_select::{
    TextDrag, TextEndpoint, endpoint_at, endpoint_at_clamped, paint_text_drag, text_for_drag,
};

use crate::scrollback::blocks::ContextInfoBlock;
use crate::theme::Theme;
use crate::views::credit_bar::{CreditBalance, localized_reset_display};
use crate::views::modal_window::{
    self as mw, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};

/// Footer shortcut ID for "copy session ID".
pub const COPY_SESSION_ID_SHORTCUT: usize = 1;

/// Footer shortcut ID for "copy all session info".
pub const COPY_ALL_SESSION_INFO_SHORTCUT: usize = 2;

/// The three tabs, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageInfoTab {
    ContextUsage,
    UsageLimit,
    SessionInfo,
}

impl UsageInfoTab {
    pub const ALL: [UsageInfoTab; 3] = [
        UsageInfoTab::ContextUsage,
        UsageInfoTab::UsageLimit,
        UsageInfoTab::SessionInfo,
    ];

    pub fn label(self, locale: &crate::locale::LocaleContext) -> &'static str {
        match self {
            UsageInfoTab::ContextUsage => {
                locale.named_static_text("palette.context_usage", "Context usage")
            }
            UsageInfoTab::UsageLimit => {
                locale.named_static_text("usage.modal.tab.limit", "Usage limit")
            }
            UsageInfoTab::SessionInfo => {
                locale.named_static_text("palette.session_info", "Session info")
            }
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        match Self::ALL.get(i) {
            Some(&tab) => tab,
            None => UsageInfoTab::ContextUsage,
        }
    }
}

/// Account/session facts captured when the modal opens.
pub struct UsageInfoContext {
    /// Session ID for the copy shortcut (`None` before the session starts).
    pub session_id: Option<String>,
    /// False for team/enterprise accounts, which have no consumer billing.
    pub usage_visible: bool,
    /// True for gateway chat sessions, which have no Build coding credits.
    pub chat_kind: bool,
    /// Remote-settings kill switch: link out instead of showing billing.
    pub billing_redirect_url: Option<String>,
    /// Plan name for the allowance header (e.g. "SuperGrok").
    pub subscription_tier: Option<String>,
}

/// Modal state. Billing figures are NOT stored here.
/// The render reads the agent's cached `credit_balance` mirror, so a silent billing refresh updates the open modal for free.
pub struct UsageInfoModalState {
    pub window: ModalWindowState,
    pub active_tab: UsageInfoTab,
    pub scroll: u16,
    pub ctx: UsageInfoContext,
    pub context: Option<ContextInfoBlock>,
    pub context_error: Option<String>,
    /// Structured `/session-info` rows, built upstream from typed session data (never by re-parsing a formatted string).
    pub session_fields: Option<Vec<SessionInfoField>>,
    pub session_error: Option<String>,
    /// Pre-formatted session token/cost summary (`session_usage_block_text`).
    pub session_usage_text: Option<String>,
    pub billing_loading: bool,
    pub billing_error: Option<String>,
    /// Fetch generation stamped at open; results from an earlier open (same session, modal reopened) are dropped instead of overwriting.
    pub fetch_nonce: u64,
    /// Hit rects for copyable value rows, refreshed every render.
    pub copy_hits: Vec<SessionCopyHit>,
    /// Content-line index of the hovered value row.
    pub hovered_copy_line: Option<usize>,
    content_rect: Rect,
    plain_lines: Vec<String>,
    /// Press held until movement promotes a drag (same threshold as scrollback).
    pending_press: Option<PendingPress>,
    text_drag: Option<TextDrag>,
}

/// One labeled Session-info row, built upstream from typed session data.
/// The modal renders and copies straight from these; it never parses a formatted string.
/// `compact` selects the dense `Label: value` layout for the model/runtime group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfoField {
    pub label: &'static str,
    pub value: String,
    pub compact: bool,
}

/// On-screen hit for a click-to-copy value row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCopyHit {
    pub rect: Rect,
    pub value: String,
    pub line_idx: usize,
}

/// Left-button press before the movement threshold promotes a drag.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPress {
    start_col: u16,
    start_row: u16,
    endpoint: TextEndpoint,
    /// Set when the press landed on a value row; copied on release without drag.
    click_value: Option<String>,
}

impl UsageInfoModalState {
    pub fn new(tab: UsageInfoTab, ctx: UsageInfoContext) -> Self {
        Self {
            window: ModalWindowState::with_tabs(UsageInfoTab::ALL.len()),
            active_tab: tab,
            scroll: 0,
            ctx,
            context: None,
            context_error: None,
            session_error: None,
            session_usage_text: None,
            billing_loading: false,
            billing_error: None,
            fetch_nonce: Default::default(),
            session_fields: None,
            copy_hits: Vec::new(),
            hovered_copy_line: None,
            content_rect: Rect::default(),
            plain_lines: Vec::new(),
            pending_press: None,
            text_drag: None,
        }
    }

    pub fn set_tab(&mut self, tab: UsageInfoTab) {
        if self.active_tab != tab {
            self.active_tab = tab;
            self.scroll = 0;
            self.clear_text_drag();
        }
    }

    /// Drop any in-progress press/drag and the hover highlight (chrome calls this).
    pub fn clear_text_drag(&mut self) {
        self.pending_press = None;
        self.text_drag = None;
        self.hovered_copy_line = None;
    }

    pub(crate) fn has_active_drag(&self) -> bool {
        self.text_drag.is_some()
    }

    fn endpoint_at(&self, column: u16, row: u16) -> Option<TextEndpoint> {
        endpoint_at(
            &self.plain_lines,
            self.content_rect,
            self.scroll as usize,
            column,
            row,
        )
    }

    fn endpoint_at_clamped(&self, column: u16, row: u16) -> Option<TextEndpoint> {
        endpoint_at_clamped(
            &self.plain_lines,
            self.content_rect,
            self.scroll as usize,
            column,
            row,
        )
    }

    fn scroll_to(&mut self, offset: u16) {
        self.scroll = offset;
        // Content moves under a still pointer; drop an unfinished gesture.
        self.clear_text_drag();
    }

    /// Finish an active drag whose `Up(Left)` never arrived (bare `Moved`). Unlike scrollback recovery
    /// (which discards), a non-empty drag still copies, so the selection band does not vanish without
    /// copying. The pending press is left alone: it paints nothing and the next Down overwrites it.
    pub(crate) fn finish_lost_drag(&mut self) -> UsageModalOutcome {
        let outcome = if let Some(drag) = self.text_drag.take()
            && drag.is_non_empty()
            && let Some(text) = text_for_drag(drag, &self.plain_lines, self.content_rect.width)
        {
            UsageModalOutcome::CopyText(text)
        } else {
            UsageModalOutcome::Changed
        };
        self.hovered_copy_line = None;
        outcome
    }

    /// The full Session-info block as one readable, clipboard-friendly string (`Label: value` lines).
    /// Returns `None` unless the Session info tab is active and its rows have loaded.
    pub fn session_info_copy_all(&self) -> Option<String> {
        if self.active_tab != UsageInfoTab::SessionInfo {
            return None;
        }
        let fields = self.session_fields.as_ref().filter(|f| !f.is_empty())?;
        Some(
            fields
                .iter()
                .map(|f| format!("{}: {}", f.label, f.value))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn step_tab(&mut self, forward: bool) {
        let n = UsageInfoTab::ALL.len();
        let i = self.active_tab.index();
        let next = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.set_tab(UsageInfoTab::from_index(next));
    }
}

/// Outcome of routing one key/mouse event through the modal.
/// The host (agent view or dashboard) owns the modal slot, the clipboard, and the toast surface, so every variant is a request to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageModalOutcome {
    /// Drop the modal (Esc, `[✗]`, click outside). Only the chrome routers emit this.
    Close,
    /// Copy the session ID to the clipboard.
    /// Emitted by the `c` shortcut and the footer button.
    CopySessionId,
    /// Copy Session-info text.
    /// Emitted by `y`, a click on a value row, the footer "copy all" button, and a finished drag-select.
    CopyText(String),
    Changed,
    Unchanged,
}

/// The modal's chrome config: no title, tabs, footer, or fold tree of its own (tabs and shortcuts are painted by `render_usage_modal`).
fn chrome_cfg() -> ModalWindowConfig<'static> {
    ModalWindowConfig {
        title: "",
        tabs: None,
        shortcuts: &[],
        sizing: ModalSizing::default(),
        fold_info: None,
    }
}

/// Route a key: `modal_window` chrome first (Esc closes), then the content keys.
pub fn route_usage_modal_key(state: &mut UsageInfoModalState, key: &KeyEvent) -> UsageModalOutcome {
    match mw::handle_modal_key(&mut state.window, key, &chrome_cfg()) {
        mw::ModalWindowOutcome::CloseRequested => UsageModalOutcome::Close,
        mw::ModalWindowOutcome::Unhandled => handle_usage_modal_key(state, key),
        // The chrome config declares no tabs, footer, or fold tree, so these never fire for keys
        mw::ModalWindowOutcome::Handled
        | mw::ModalWindowOutcome::TabChanged(_)
        | mw::ModalWindowOutcome::ShortcutActivated(_)
        | mw::ModalWindowOutcome::CollapseGroup
        | mw::ModalWindowOutcome::ExpandGroup
        | mw::ModalWindowOutcome::CollapseDetails
        | mw::ModalWindowOutcome::ExpandDetails
        | mw::ModalWindowOutcome::JumpToParent(_) => UsageModalOutcome::Changed,
    }
}

/// Route a mouse event: chrome first (close button, tab headers, footer buttons stay clickable), then drag / wheel / click-to-copy.
pub fn route_usage_modal_mouse(
    state: &mut UsageInfoModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> UsageModalOutcome {
    match mw::handle_modal_mouse(&mut state.window, kind, column, row) {
        mw::ModalWindowOutcome::CloseRequested => UsageModalOutcome::Close,
        mw::ModalWindowOutcome::TabChanged(idx) => {
            state.set_tab(UsageInfoTab::from_index(idx));
            UsageModalOutcome::Changed
        }
        mw::ModalWindowOutcome::ShortcutActivated(id) => {
            // Footer click: drop gesture and hover
            state.clear_text_drag();
            if id == COPY_SESSION_ID_SHORTCUT {
                UsageModalOutcome::CopySessionId
            } else if id == COPY_ALL_SESSION_INFO_SHORTCUT
                && let Some(text) = state.session_info_copy_all()
            {
                UsageModalOutcome::CopyText(text)
            } else {
                UsageModalOutcome::Changed
            }
        }
        mw::ModalWindowOutcome::Handled => {
            // Same rule as content: a bare Moved with an active drag is a lost Up, and a non-empty drag still copies
            // The pending press is left alone for click-to-copy
            if matches!(kind, MouseEventKind::Moved) {
                if state.has_active_drag() {
                    return state.finish_lost_drag();
                }
                state.hovered_copy_line = None;
            } else {
                // Same-tab click and other chrome Downs: drop gesture and hover
                state.clear_text_drag();
            }
            UsageModalOutcome::Changed
        }
        mw::ModalWindowOutcome::Unhandled => handle_usage_modal_mouse(state, kind, column, row),
        // Fold-tree navigation belongs to the settings modal; this window has no fold info
        mw::ModalWindowOutcome::CollapseGroup
        | mw::ModalWindowOutcome::ExpandGroup
        | mw::ModalWindowOutcome::CollapseDetails
        | mw::ModalWindowOutcome::ExpandDetails
        | mw::ModalWindowOutcome::JumpToParent(_) => UsageModalOutcome::Changed,
    }
}

/// Content keys (tabs, scroll, copy); chrome has already had its turn in [`route_usage_modal_key`].
fn handle_usage_modal_key(state: &mut UsageInfoModalState, key: &KeyEvent) -> UsageModalOutcome {
    use crossterm::event::KeyModifiers;
    // BackTab and `G` legitimately carry SHIFT; reject only real chords
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return UsageModalOutcome::Unchanged;
    }
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            state.step_tab(true);
            UsageModalOutcome::Changed
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            state.step_tab(false);
            UsageModalOutcome::Changed
        }
        KeyCode::Char(c @ '1'..='3') => {
            state.set_tab(UsageInfoTab::from_index(c as usize - '1' as usize));
            UsageModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll_to(state.scroll.saturating_sub(1));
            UsageModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll_to(state.scroll.saturating_add(1));
            UsageModalOutcome::Changed
        }
        KeyCode::PageUp => {
            state.scroll_to(state.scroll.saturating_sub(10));
            UsageModalOutcome::Changed
        }
        KeyCode::PageDown => {
            state.scroll_to(state.scroll.saturating_add(10));
            UsageModalOutcome::Changed
        }
        KeyCode::Home => {
            state.scroll_to(0);
            UsageModalOutcome::Changed
        }
        // Clamped to content height at render time.
        KeyCode::End | KeyCode::Char('G') => {
            state.scroll_to(u16::MAX);
            UsageModalOutcome::Changed
        }
        KeyCode::Char('c') if state.ctx.session_id.is_some() => UsageModalOutcome::CopySessionId,
        KeyCode::Char('y') => match state.session_info_copy_all() {
            Some(text) => UsageModalOutcome::CopyText(text),
            None => UsageModalOutcome::Unchanged,
        },
        _ => UsageModalOutcome::Unchanged,
    }
}

/// Content gestures (wheel, click-to-copy, drag-select); chrome has already had its turn in [`route_usage_modal_mouse`].
fn handle_usage_modal_mouse(
    state: &mut UsageInfoModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> UsageModalOutcome {
    let hit_at = |state: &UsageInfoModalState| -> Option<SessionCopyHit> {
        state
            .copy_hits
            .iter()
            .find(|h| {
                column >= h.rect.x
                    && column < h.rect.x.saturating_add(h.rect.width)
                    && row >= h.rect.y
                    && row < h.rect.y.saturating_add(h.rect.height)
            })
            .cloned()
    };
    match kind {
        MouseEventKind::ScrollUp => {
            state.scroll_to(state.scroll.saturating_sub(3));
            UsageModalOutcome::Changed
        }
        MouseEventKind::ScrollDown => {
            state.scroll_to(state.scroll.saturating_add(3));
            UsageModalOutcome::Changed
        }
        MouseEventKind::Down(MouseButton::Left)
            if state.active_tab == UsageInfoTab::SessionInfo && in_content(state, column, row) =>
        {
            let Some(ep) = state.endpoint_at(column, row) else {
                return UsageModalOutcome::Unchanged;
            };
            // Hold the press; promote to drag only after a 1-cell move (same threshold as scrollback)
            state.pending_press = Some(PendingPress {
                start_col: column,
                start_row: row,
                endpoint: ep,
                click_value: hit_at(state).map(|h| h.value),
            });
            state.text_drag = None;
            UsageModalOutcome::Changed
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(pending) = state.pending_press.as_ref() {
                // Same 1-cell threshold as scrollback `drag_threshold_exceeded`.
                let moved =
                    pending.start_col.abs_diff(column) >= 1 || pending.start_row.abs_diff(row) >= 1;
                if !moved {
                    return UsageModalOutcome::Unchanged;
                }
                let ep = pending.endpoint;
                state.pending_press = None;
                state.text_drag = Some(TextDrag {
                    anchor: ep,
                    head: ep,
                });
            }
            if state.text_drag.is_none() {
                return UsageModalOutcome::Unchanged;
            }
            let rect = state.content_rect;
            if rect.width == 0 || rect.height == 0 {
                state.clear_text_drag();
                return UsageModalOutcome::Changed;
            }
            if row < rect.y {
                state.scroll = state.scroll.saturating_sub(1);
            } else if row >= rect.y.saturating_add(rect.height) {
                state.scroll = state.scroll.saturating_add(1);
            }
            if let Some(ep) = state.endpoint_at_clamped(column, row)
                && let Some(d) = state.text_drag.as_mut()
            {
                d.head = ep;
            }
            UsageModalOutcome::Changed
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(pending) = state.pending_press.take() {
                return match pending.click_value {
                    Some(text) => UsageModalOutcome::CopyText(text),
                    None => UsageModalOutcome::Changed,
                };
            }
            let Some(mut final_drag) = state.text_drag.take() else {
                return UsageModalOutcome::Unchanged;
            };
            if let Some(ep) = state.endpoint_at_clamped(column, row) {
                final_drag.head = ep;
            }
            if final_drag.is_non_empty()
                && let Some(text) =
                    text_for_drag(final_drag, &state.plain_lines, state.content_rect.width)
            {
                return UsageModalOutcome::CopyText(text);
            }
            UsageModalOutcome::Changed
        }
        MouseEventKind::Moved => {
            // Bare Moved with an active drag: treat as lost Up and finish (copy).
            // Pending press is not cleared here (click still works if Up arrives).
            let lost = if state.has_active_drag() {
                Some(state.finish_lost_drag())
            } else {
                None
            };
            let new_hover = hit_at(state).map(|h| h.line_idx);
            let hover_changed = new_hover != state.hovered_copy_line;
            if hover_changed {
                state.hovered_copy_line = new_hover;
            }
            match lost {
                Some(UsageModalOutcome::CopyText(text)) => UsageModalOutcome::CopyText(text),
                Some(UsageModalOutcome::Changed) | None if hover_changed => {
                    UsageModalOutcome::Changed
                }
                Some(other) => other,
                None => UsageModalOutcome::Unchanged,
            }
        }
        MouseEventKind::Down(_) => {
            // A new non-left press (or Left outside content) ends a stuck drag.
            if state.text_drag.take().is_some() {
                state.hovered_copy_line = None;
                UsageModalOutcome::Changed
            } else {
                UsageModalOutcome::Unchanged
            }
        }
        _ => UsageModalOutcome::Unchanged,
    }
}

pub fn render_usage_modal(
    buf: &mut Buffer,
    area: Rect,
    state: &mut UsageInfoModalState,
    balance: Option<&CreditBalance>,
    compact: bool,
    theme: &Theme,
    locale: &crate::locale::LocaleContext,
) {
    let labels: Vec<&str> = UsageInfoTab::ALL.iter().map(|t| t.label(locale)).collect();
    state.window.active_tab = state.active_tab.index();

    let mut shortcuts: Vec<Shortcut> = vec![
        Shortcut {
            label: locale.named_static_text("usage.modal.footer.tabs", "Tab switch"),
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: locale
                .named_static_text("usage.modal.footer.scroll", "\u{2191}/\u{2193} scroll"),
            clickable: false,
            id: 0,
        },
    ];
    if state.ctx.session_id.is_some() {
        shortcuts.push(Shortcut {
            label: locale
                .named_static_text("usage.modal.footer.copy_session_id", "c copy session ID"),
            clickable: true,
            id: COPY_SESSION_ID_SHORTCUT,
        });
    }
    if state.active_tab == UsageInfoTab::SessionInfo
        && state.session_fields.as_ref().is_some_and(|f| !f.is_empty())
    {
        shortcuts.push(Shortcut {
            label: locale.named_static_text("usage.modal.footer.copy_all", "y copy all"),
            clickable: true,
            id: COPY_ALL_SESSION_INFO_SHORTCUT,
        });
    }
    shortcuts.push(Shortcut {
        label: locale.named_static_text("usage.modal.footer.close", "Esc close"),
        clickable: false,
        id: 0,
    });

    // v_pad and footer_lines pad the body top and bottom (shortcuts render bottom-aligned, so the spare footer row reads as bottom padding)
    let sizing = ModalSizing {
        width_pct: 0.65,
        max_width: 100,
        min_width: 44,
        v_margin: 2,
        h_pad: 2,
        v_pad: 2,
        footer_lines: 3,
    }
    .with_compact(compact);
    // No border title; the tab bar is the header, as in the extensions modal
    let config = ModalWindowConfig {
        title: "",
        tabs: Some(&labels),
        shortcuts: &shortcuts,
        sizing,
        fold_info: None,
    };

    // The chrome always fills `area` minus `v_margin`, so cap the height ourselves: tall terminals would otherwise get a mostly-empty box
    // 30 rows is about the widest tab's content (the context grid plus legend) plus chrome
    const MAX_MODAL_HEIGHT: u16 = 30;
    let outer = MAX_MODAL_HEIGHT + sizing.v_margin * 2;
    let area = if area.height > outer {
        Rect {
            x: area.x,
            y: area.y + (area.height - outer) / 2,
            width: area.width,
            height: outer,
        }
    } else {
        area
    };

    let Some(mca) = mw::render_modal_window(buf, area, &mut state.window, &config, theme) else {
        // Too small to paint: zero geometry must not leave a live drag.
        state.content_rect = Rect::default();
        state.plain_lines.clear();
        state.copy_hits.clear();
        state.clear_text_drag();
        return;
    };
    let content = mca.content;
    let tab = tab_content(state, balance, theme, content.width, locale);
    state.content_rect = content;
    let plain_lines: Vec<String> = tab.lines.iter().map(ToString::to_string).collect();
    if state.plain_lines != plain_lines {
        state.clear_text_drag();
    }
    state.plain_lines = plain_lines;
    // No wrapping: one row per logical line keeps the scroll clamp exact.
    let max_scroll = state
        .plain_lines
        .len()
        .saturating_sub(content.height as usize);
    state.scroll = (state.scroll as usize).min(max_scroll) as u16;
    state.copy_hits = tab
        .copy_targets
        .iter()
        .filter_map(|t| {
            let visible_row = t.line_idx.checked_sub(state.scroll as usize)?;
            (visible_row < content.height as usize).then(|| SessionCopyHit {
                rect: Rect {
                    x: content.x,
                    y: content.y + visible_row as u16,
                    width: content.width,
                    height: 1,
                },
                value: t.value.clone(),
                line_idx: t.line_idx,
            })
        })
        .collect();
    let visible: Vec<Line> = tab
        .lines
        .into_iter()
        .skip(state.scroll as usize)
        .take(content.height as usize)
        .collect();
    Paragraph::new(visible).render(content, buf);
    if let Some(drag) = state.text_drag {
        paint_text_drag(
            drag,
            &state.plain_lines,
            state.content_rect,
            state.scroll as usize,
            buf,
            theme,
        );
    }
}

fn in_content(state: &UsageInfoModalState, column: u16, row: u16) -> bool {
    let r = state.content_rect;
    r.width > 0
        && r.height > 0
        && column >= r.x
        && column < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}

/// A copyable value row: `line_idx` indexes the tab's lines; `value` is copied on click.
struct CopyTarget {
    line_idx: usize,
    value: String,
}

struct TabContent {
    lines: Vec<Line<'static>>,
    copy_targets: Vec<CopyTarget>,
}

impl TabContent {
    fn from_lines(lines: Vec<Line<'static>>) -> Self {
        Self {
            lines,
            copy_targets: Vec::new(),
        }
    }
}

fn tab_content(
    state: &UsageInfoModalState,
    balance: Option<&CreditBalance>,
    theme: &Theme,
    width: u16,
    locale: &crate::locale::LocaleContext,
) -> TabContent {
    match state.active_tab {
        UsageInfoTab::ContextUsage => {
            TabContent::from_lines(context_tab_lines(state, theme, width, locale))
        }
        UsageInfoTab::UsageLimit => {
            TabContent::from_lines(usage_limit_lines(state, balance, theme, locale))
        }
        UsageInfoTab::SessionInfo => session_info_content(state, theme, locale),
    }
}

fn header_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .add_modifier(Modifier::BOLD)
}

fn plain(theme: &Theme, s: impl Into<String>) -> Line<'static> {
    Line::styled(s.into(), Style::default().fg(theme.text_primary))
}

fn muted_line(theme: &Theme, s: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(s.into(), theme.muted()))
}

fn localized_template(
    locale: &crate::locale::LocaleContext,
    id: &str,
    english: &str,
    arguments: &[(&str, &str)],
) -> String {
    let mut output = locale.named_text(id, english).into_owned();
    for (name, value) in arguments {
        output = output.replace(&format!("{{{name}}}"), value);
    }
    output
}

fn context_tab_lines(
    state: &UsageInfoModalState,
    theme: &Theme,
    width: u16,
    locale: &crate::locale::LocaleContext,
) -> Vec<Line<'static>> {
    if let Some(error) = &state.context_error {
        return vec![muted_line(
            theme,
            localized_template(
                locale,
                "usage.modal.context.load_failed",
                "Couldn't load context usage: {error}",
                &[("error", error)],
            ),
        )];
    }
    if let Some(block) = &state.context {
        return block.lines_for_width(theme, width);
    }
    if state.ctx.session_id.is_none() {
        return vec![muted_line(
            theme,
            locale
                .named_text("usage.modal.no_active_session", "No active session.")
                .into_owned(),
        )];
    }
    vec![muted_line(
        theme,
        locale
            .named_text(
                "usage.modal.context.loading",
                "Loading context usage\u{2026}",
            )
            .into_owned(),
    )]
}

/// Account allowance followed by this session's token/cost totals.
fn usage_limit_lines(
    state: &UsageInfoModalState,
    balance: Option<&CreditBalance>,
    theme: &Theme,
    locale: &crate::locale::LocaleContext,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if state.ctx.chat_kind {
        // Gateway chat sessions have no Build coding credits to show.
    } else if !state.ctx.usage_visible {
        lines.push(muted_line(
            theme,
            locale
                .named_text(
                    "usage.modal.limit.managed_by_team",
                    "Usage limits are managed by your team.",
                )
                .into_owned(),
        ));
    } else if let Some(url) = &state.ctx.billing_redirect_url {
        lines.push(plain(
            theme,
            localized_template(
                locale,
                "status.usage.redirect",
                "Please check your usage on {url}",
                &[("url", url)],
            ),
        ));
    } else if let Some(bal) = balance {
        lines.extend(allowance_lines(state, bal, theme, locale));
    } else if let Some(error) = &state.billing_error {
        lines.push(muted_line(
            theme,
            localized_template(
                locale,
                "usage.modal.limit.load_failed",
                "Couldn't load usage: {error}",
                &[("error", error)],
            ),
        ));
    } else if state.billing_loading {
        lines.push(muted_line(
            theme,
            locale
                .named_text("usage.modal.limit.loading", "Loading usage\u{2026}")
                .into_owned(),
        ));
    } else {
        lines.push(muted_line(
            theme,
            locale
                .named_text("status.billing.no_data", "No billing data available.")
                .into_owned(),
        ));
    }

    if let Some(usage_text) = &state.session_usage_text {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        for (i, row) in usage_text.lines().enumerate() {
            if i == 0 {
                lines.push(Line::styled(row.to_string(), header_style(theme)));
            } else {
                lines.push(plain(theme, row));
            }
        }
    } else if state.ctx.session_id.is_some() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(muted_line(
            theme,
            locale
                .named_text(
                    "usage.modal.session_usage.loading",
                    "Loading session usage\u{2026}",
                )
                .into_owned(),
        ));
    }
    lines
}

fn allowance_lines(
    state: &UsageInfoModalState,
    bal: &CreditBalance,
    theme: &Theme,
    locale: &crate::locale::LocaleContext,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // "Weekly limit", "Monthly limit", or "Usage", plus the plan name
    let header = match &state.ctx.subscription_tier {
        Some(tier) => format!("{} ({tier})", bal.usage_label_with_locale(Some(locale))),
        None => bal.usage_label_with_locale(Some(locale)).into_owned(),
    };
    lines.push(Line::styled(header, header_style(theme)));
    lines.push(Line::default());

    const BAR_WIDTH: usize = 30;
    let pct = bal.usage_pct.clamp(0.0, 100.0);
    let filled = (((pct / 100.0) * BAR_WIDTH as f64).round() as usize).min(BAR_WIDTH);
    lines.push(Line::from(vec![
        Span::styled(
            "\u{2588}".repeat(filled),
            Style::default().fg(theme.gray_bright),
        ),
        Span::styled(
            "\u{2591}".repeat(BAR_WIDTH - filled),
            Style::default().fg(theme.gray_dim),
        ),
        // Floored to match the backend's truncation.
        Span::styled(
            format!("  {}%", bal.usage_pct.floor() as i64),
            Style::default().fg(theme.text_primary),
        ),
    ]));

    if let Some(reset) = &bal.period_end_display {
        let reset = localized_reset_display(reset, Some(locale));
        lines.push(muted_line(
            theme,
            localized_template(
                locale,
                "status.billing.next_reset",
                "Resets: {time}",
                &[("time", &reset)],
            ),
        ));
    }

    // Prepaid credits (stored as negative cents, an accounting convention)
    if let Some(prepaid) = bal.prepaid_balance_cents.map(i64::abs).filter(|c| *c > 0) {
        lines.push(Line::default());
        let amount = format!("${:.2}", prepaid as f64 / 100.0);
        lines.push(plain(
            theme,
            localized_template(
                locale,
                "status.billing.credits",
                "Credits: {amount}",
                &[("amount", &amount)],
            ),
        ));
    }

    // Legacy on-demand (pay-as-you-go) billing.
    if bal.pay_as_you_go {
        let used = bal.on_demand_used_cents.unwrap_or(0).abs() as f64 / 100.0;
        let cap = bal.on_demand_cap_cents.unwrap_or(0).abs() as f64 / 100.0;
        lines.push(Line::default());
        lines.push(Line::styled(
            locale
                .named_text("usage.modal.limit.payg_enabled", "Pay as you go: Enabled")
                .into_owned(),
            header_style(theme),
        ));
        let used = format!("${used:.2}");
        let cap = format!("${cap:.2}");
        lines.push(muted_line(
            theme,
            localized_template(
                locale,
                "usage.modal.limit.payg_usage",
                "Usage: {used} / {cap} per month",
                &[("used", &used), ("cap", &cap)],
            ),
        ));
    }
    lines
}

/// Model/runtime details rendered as one compact `Label: value` block; every
/// other field gets a spaced label-over-value group.
fn is_compact_session_field(label: &str) -> bool {
    matches!(
        label,
        "Model" | "Model Hash" | "API Backend" | "Sandbox" | "Turn" | "Context"
    )
}

fn localized_session_field(locale: &crate::locale::LocaleContext, label: &str) -> String {
    let (id, english) = match label {
        "Title" => ("usage.modal.session.field.title", "Title"),
        "Shell version" => ("usage.modal.session.field.shell_version", "Shell version"),
        "Session ID" => ("usage.modal.session.field.session_id", "Session ID"),
        "Conversation ID" => (
            "usage.modal.session.field.conversation_id",
            "Conversation ID",
        ),
        "Working directory" => ("usage.modal.session.field.cwd", "Working directory"),
        "Model" => ("usage.modal.session.field.model", "Model"),
        "Model Hash" => ("usage.modal.session.field.model_hash", "Model Hash"),
        "API Backend" => ("usage.modal.session.field.api_backend", "API Backend"),
        "Sandbox" => ("usage.modal.session.field.sandbox", "Sandbox"),
        "Turn" => ("usage.modal.session.field.turn", "Turn"),
        "Context" => ("usage.modal.session.field.context", "Context"),
        "Auth method" => ("usage.modal.session.field.auth_method", "Auth method"),
        _ => return label.to_string(),
    };
    locale.named_text(id, english).into_owned()
}

/// Localize the canonical `format_session_info` text for minimal-mode
/// scrollback. The full modal keeps the canonical labels internally so its
/// grouping and click-to-copy logic remain stable, then localizes at render.
pub(crate) fn localize_session_info_text(
    text: &str,
    locale: &crate::locale::LocaleContext,
) -> String {
    text.lines()
        .map(|row| {
            let trimmed = row.trim_start();
            let indent = &row[..row.len() - trimmed.len()];
            if trimmed.starts_with("Run `grok-zh login`") {
                return format!(
                    "{indent}{}",
                    locale.named_static_text(
                        "usage.modal.session.login_upsell",
                        "Run `grok-zh login` to use your SuperGrok subscription instead.",
                    )
                );
            }
            let Some((label, value)) = trimmed.split_once(": ") else {
                return row.to_string();
            };
            format!(
                "{indent}{}: {value}",
                localized_session_field(locale, label)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn session_info_content(
    state: &UsageInfoModalState,
    theme: &Theme,
    locale: &crate::locale::LocaleContext,
) -> TabContent {
    if let Some(error) = &state.session_error {
        return TabContent::from_lines(vec![muted_line(
            theme,
            localized_template(
                locale,
                "session.info.load_failed",
                "Couldn't load session info: {error}",
                &[("error", error)],
            ),
        )]);
    }
    let Some(fields) = state.session_fields.as_ref().filter(|f| !f.is_empty()) else {
        if state.ctx.session_id.is_none() {
            return TabContent::from_lines(vec![muted_line(
                theme,
                locale
                    .named_text("usage.modal.no_active_session", "No active session.")
                    .into_owned(),
            )]);
        }
        return TabContent::from_lines(vec![muted_line(
            theme,
            locale
                .named_text(
                    "usage.modal.session.loading",
                    "Loading session info\u{2026}",
                )
                .into_owned(),
        )]);
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(
            locale
                .named_text("palette.session_info", "Session info")
                .into_owned(),
            header_style(theme),
        ),
        Span::styled(
            format!(
                "   {}",
                locale.named_static_text("usage.modal.session.copy_hint", "click or drag to copy",)
            ),
            Style::default().fg(theme.gray_dim),
        ),
    ])];
    let mut copy_targets: Vec<CopyTarget> = Vec::new();
    let mut prev_compact = false;
    for field in fields {
        let compact = field.compact;
        let display_label = localized_session_field(locale, field.label);
        if !(compact && prev_compact) {
            lines.push(Line::default());
        }
        if compact {
            let value_idx = lines.len();
            let hovered = state.hovered_copy_line == Some(value_idx);
            let label_style = if hovered {
                theme.muted().patch(theme.hover_overlay())
            } else {
                theme.muted()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{display_label}: "), label_style),
                Span::styled(field.value.clone(), copy_value_style(theme, hovered)),
            ]));
            copy_targets.push(CopyTarget {
                line_idx: value_idx,
                value: format!("{}: {}", field.label, field.value),
            });
        } else {
            lines.push(Line::from(Span::styled(
                format!("{display_label}:"),
                theme.muted(),
            )));
            let value_idx = lines.len();
            let hovered = state.hovered_copy_line == Some(value_idx);
            lines.push(Line::from(Span::styled(
                field.value.clone(),
                copy_value_style(theme, hovered),
            )));
            copy_targets.push(CopyTarget {
                line_idx: value_idx,
                value: field.value.clone(),
            });
        }
        prev_compact = compact;
    }
    TabContent {
        lines,
        copy_targets,
    }
}

fn copy_value_style(theme: &Theme, hovered: bool) -> Style {
    let mut style = Style::default().fg(theme.text_primary);
    if hovered {
        style = style.patch(theme.hover_overlay());
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LocaleContext, LocaleSource, ResolvedLocale, UiLocale};
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn zh_locale() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn field(label: &'static str, value: &str, compact: bool) -> SessionInfoField {
        SessionInfoField {
            label,
            value: value.to_string(),
            compact,
        }
    }

    fn state_with_session() -> UsageInfoModalState {
        UsageInfoModalState::new(
            UsageInfoTab::UsageLimit,
            UsageInfoContext {
                session_id: Some("sid-123".to_string()),
                usage_visible: true,
                chat_kind: false,
                billing_redirect_url: None,
                subscription_tier: Some("SuperGrok".to_string()),
            },
        )
    }

    #[test]
    fn tab_cycling_wraps_and_resets_scroll() {
        let mut state = state_with_session();
        state.scroll = 7;
        assert_eq!(
            handle_usage_modal_key(&mut state, &key(KeyCode::Tab)),
            UsageModalOutcome::Changed
        );
        assert_eq!(state.active_tab, UsageInfoTab::SessionInfo);
        assert_eq!(state.scroll, 0, "tab switch resets scroll");
        handle_usage_modal_key(&mut state, &key(KeyCode::Tab));
        assert_eq!(state.active_tab, UsageInfoTab::ContextUsage, "wraps");
        handle_usage_modal_key(&mut state, &key(KeyCode::BackTab));
        assert_eq!(state.active_tab, UsageInfoTab::SessionInfo, "wraps back");
    }

    #[test]
    fn copy_shortcut_requires_a_session_id() {
        let mut state = state_with_session();
        assert_eq!(
            handle_usage_modal_key(&mut state, &key(KeyCode::Char('c'))),
            UsageModalOutcome::CopySessionId
        );
        state.ctx.session_id = None;
        assert_eq!(
            handle_usage_modal_key(&mut state, &key(KeyCode::Char('c'))),
            UsageModalOutcome::Unchanged
        );
    }

    #[test]
    fn usage_limit_tab_shows_allowance_and_payg() {
        let state = state_with_session();
        let bal = CreditBalance {
            usage_pct: 50.67,
            effective_usage_pct: 50.67,
            period_end_display: Some("May 29, 00:00".to_string()),
            pay_as_you_go: true,
            on_demand_cap_cents: Some(10_000),
            on_demand_used_cents: Some(0),
            prepaid_balance_cents: None,
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".to_string()),
            is_unified_billing_user: None,
        };
        let theme = Theme::current();
        let lines = usage_limit_lines(&state, Some(&bal), &theme, &LocaleContext::default());
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(
            text.first().map(String::as_str),
            Some("Weekly limit (SuperGrok)")
        );
        assert!(
            text.get(2).is_some_and(|l| l.ends_with("50%")),
            "bar row: {:?}",
            text.get(2)
        );
        assert!(text.iter().any(|l| l.contains("Resets: May 29, 00:00")));
        assert!(text.iter().any(|l| l == "Pay as you go: Enabled"));
        assert!(
            text.iter().any(|l| l == "Usage: $0.00 / $100.00 per month"),
            "{text:?}"
        );
        assert!(
            !text.iter().any(|l| l.to_lowercase().contains("top")),
            "no auto top-up surface: {text:?}"
        );
    }

    #[test]
    fn usage_limit_tab_states() {
        let theme = Theme::current();
        let mut state = state_with_session();
        state.billing_loading = true;
        let lines = usage_limit_lines(&state, None, &theme, &LocaleContext::default());
        assert!(
            lines
                .first()
                .is_some_and(|l| l.to_string().contains("Loading usage"))
        );

        state.ctx.billing_redirect_url = Some("https://x.example/usage".to_string());
        let lines = usage_limit_lines(&state, None, &theme, &LocaleContext::default());
        assert!(
            lines
                .first()
                .is_some_and(|l| l.to_string().contains("https://x.example/usage"))
        );

        state.ctx.usage_visible = false;
        let lines = usage_limit_lines(&state, None, &theme, &LocaleContext::default());
        assert!(
            lines
                .first()
                .is_some_and(|l| l.to_string().contains("managed by your team"))
        );

        // Gateway chat sessions show no billing at all
        state.ctx.chat_kind = true;
        let lines = usage_limit_lines(&state, None, &theme, &LocaleContext::default());
        assert!(
            lines
                .first()
                .is_some_and(|l| l.to_string().contains("Loading session usage"))
        );
    }

    #[test]
    fn render_smoke_shows_tabs_and_copy_shortcut() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.session_usage_text = Some("Session usage: no model calls yet.".to_string());
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let text: String = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
                    .collect::<String>()
                    + "\n"
            })
            .collect();
        for needle in [
            "Context usage",
            "Usage limit",
            "Session info",
            "copy session ID",
            "Session usage: no model calls yet.",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        assert_eq!(state.window.tab_rects.len(), 3);
        assert!(state.window.close_button_rect.is_some());
    }

    #[test]
    fn copy_all_returns_readable_block_and_footer_button() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        assert_eq!(state.session_info_copy_all(), None);
        state.set_tab(UsageInfoTab::SessionInfo);
        assert_eq!(state.session_info_copy_all(), None);
        state.session_fields = Some(vec![
            field("Session ID", "sid-123", false),
            field("Model Hash", "fp-abc", true),
            field("Turn", "3", true),
        ]);
        assert_eq!(
            state.session_info_copy_all().as_deref(),
            Some("Session ID: sid-123\nModel Hash: fp-abc\nTurn: 3")
        );
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let text: String = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
                    .collect::<String>()
                    + "\n"
            })
            .collect();
        assert!(
            text.contains("copy all"),
            "missing copy-all button:\n{text}"
        );
        assert!(
            text.contains("click or drag to copy"),
            "missing copy hint:\n{text}"
        );
        assert_eq!(
            handle_usage_modal_key(&mut state, &key(KeyCode::Char('y'))),
            UsageModalOutcome::CopyText(
                "Session ID: sid-123\nModel Hash: fp-abc\nTurn: 3".to_string()
            )
        );
        state.set_tab(UsageInfoTab::UsageLimit);
        assert_eq!(state.session_info_copy_all(), None);
        assert_eq!(
            handle_usage_modal_key(&mut state, &key(KeyCode::Char('y'))),
            UsageModalOutcome::Unchanged
        );
    }

    #[test]
    fn click_value_row_copies_without_drag() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.set_tab(UsageInfoTab::SessionInfo);
        state.session_fields = Some(vec![
            field("Title", "t", false),
            field("Session ID", "sid-123", false),
            field("Model Hash", "fp-abc", true),
        ]);
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let hit = state
            .copy_hits
            .iter()
            .find(|hit| hit.value == "Model Hash: fp-abc")
            .cloned()
            .expect("model hash row visible");
        assert_eq!(
            handle_usage_modal_mouse(
                &mut state,
                MouseEventKind::Down(MouseButton::Left),
                hit.rect.x,
                hit.rect.y,
            ),
            UsageModalOutcome::Changed
        );
        assert_eq!(
            handle_usage_modal_mouse(
                &mut state,
                MouseEventKind::Up(MouseButton::Left),
                hit.rect.x,
                hit.rect.y,
            ),
            UsageModalOutcome::CopyText("Model Hash: fp-abc".to_string())
        );
    }

    #[test]
    fn drag_select_copies_slice_and_paints_selection_band() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.set_tab(UsageInfoTab::SessionInfo);
        state.session_fields = Some(vec![field("Model Hash", "fp-abc", true)]);
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );

        let line_idx = state
            .plain_lines
            .iter()
            .position(|l| l.contains("fp-abc"))
            .expect("hash row");
        let Some(line) = state.plain_lines.get(line_idx) else {
            panic!("plain line {line_idx} missing");
        };
        let hash_col = line.find("fp-abc").expect("hash") as u16;
        let rect = state.content_rect;
        let y = rect.y + (line_idx as u16).saturating_sub(state.scroll);
        let x0 = rect.x + hash_col;
        let x1 = x0 + 5;

        assert_eq!(
            handle_usage_modal_mouse(&mut state, MouseEventKind::Down(MouseButton::Left), x0, y,),
            UsageModalOutcome::Changed
        );
        // Same-cell drag stays pending (not promoted)
        assert_eq!(
            handle_usage_modal_mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), x0, y,),
            UsageModalOutcome::Unchanged
        );
        assert_eq!(
            handle_usage_modal_mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), x1, y,),
            UsageModalOutcome::Changed
        );

        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let Some(cell) = buf.cell((x0, y)) else {
            panic!("cell ({x0},{y}) missing");
        };
        if theme.text_primary != ratatui::style::Color::Reset
            && theme.bg_base != ratatui::style::Color::Reset
        {
            assert_eq!(cell.bg, theme.text_primary, "selection band");
            assert_eq!(cell.fg, theme.bg_base);
        } else {
            assert!(cell.modifier.contains(ratatui::style::Modifier::REVERSED));
        }

        assert_eq!(
            handle_usage_modal_mouse(&mut state, MouseEventKind::Up(MouseButton::Left), x1, y),
            UsageModalOutcome::CopyText("fp-abc".to_string())
        );
    }

    #[test]
    fn drag_from_blank_line_keeps_newline() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.set_tab(UsageInfoTab::SessionInfo);
        state.session_fields = Some(vec![field("Title", "t", false)]);
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &Theme::current(),
            &LocaleContext::default(),
        );

        let blank = state
            .plain_lines
            .iter()
            .position(|l| l.is_empty())
            .expect("separator");
        let title = state
            .plain_lines
            .iter()
            .position(|l| l == "Title:")
            .expect("title");
        let rect = state.content_rect;
        let y0 = rect.y + blank as u16;
        let y1 = rect.y + title as u16;
        handle_usage_modal_mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            rect.x,
            y0,
        );
        handle_usage_modal_mouse(
            &mut state,
            MouseEventKind::Drag(MouseButton::Left),
            rect.x + 6,
            y1,
        );
        assert_eq!(
            handle_usage_modal_mouse(
                &mut state,
                MouseEventKind::Up(MouseButton::Left),
                rect.x + 6,
                y1,
            ),
            UsageModalOutcome::CopyText("\nTitle:".to_string())
        );
    }

    #[test]
    fn content_reload_cancels_in_progress_drag() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.set_tab(UsageInfoTab::SessionInfo);
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let rect = state.content_rect;
        assert_eq!(
            handle_usage_modal_mouse(
                &mut state,
                MouseEventKind::Down(MouseButton::Left),
                rect.x,
                rect.y,
            ),
            UsageModalOutcome::Changed
        );
        state.session_fields = Some(vec![field("Title", "t", false)]);
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        assert_eq!(
            handle_usage_modal_mouse(
                &mut state,
                MouseEventKind::Up(MouseButton::Left),
                rect.x + 4,
                rect.y,
            ),
            UsageModalOutcome::Unchanged
        );
        // Other tabs never expose the rect.
        state.set_tab(UsageInfoTab::UsageLimit);
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        assert!(state.copy_hits.is_empty());
    }

    #[test]
    fn popup_height_is_capped_on_tall_terminals() {
        let area = Rect::new(0, 0, 100, 60);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &LocaleContext::default(),
        );
        let popup = state.window.popup_area.expect("popup rendered");
        assert_eq!(popup.height, 30);
        // Still vertically centered.
        assert_eq!(popup.y, (60 - 30) / 2);
    }

    #[test]
    fn session_info_tab_spaces_groups_and_compacts_model_block() {
        let mut state = state_with_session();
        state.session_fields = Some(vec![
            field("Title", "t", false),
            field("Session ID", "sid-123", false),
            field("Working directory", "/tmp", false),
            field("Model", "Grok", true),
            field("Context", "1 / 2", true),
        ]);
        let theme = Theme::current();
        let tab = session_info_content(&state, &theme, &LocaleContext::default());
        let text: Vec<String> = tab.lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(
            text,
            [
                "Session info   click or drag to copy",
                "",
                "Title:",
                "t",
                "",
                "Session ID:",
                "sid-123",
                "",
                "Working directory:",
                "/tmp",
                "",
                "Model: Grok",
                "Context: 1 / 2",
            ]
        );
    }

    #[test]
    fn zh_locale_localizes_usage_modal_chrome_and_session_fields() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = state_with_session();
        state.set_tab(UsageInfoTab::SessionInfo);
        state.session_fields = Some(vec![
            field("Title", "t", false),
            field("Session ID", "sid-123", false),
            field("Working directory", "/tmp", false),
            field("Model", "Grok", true),
        ]);
        let theme = Theme::current();
        render_usage_modal(
            &mut buf,
            area,
            &mut state,
            None,
            false,
            &theme,
            &zh_locale(),
        );
        let text: String = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    + "\n"
            })
            .collect();
        // Wide CJK glyphs occupy two terminal cells; ratatui leaves the
        // continuation cells blank, so compact those placeholders before
        // asserting human-readable labels.
        let compact_text = text.replace(' ', "");
        for needle in [
            "上下文用量",
            "用量限制",
            "会话信息",
            "会话ID",
            "工作目录",
            "模型",
            "全部复制",
        ] {
            assert!(
                compact_text.contains(needle),
                "missing {needle:?} in:\n{text}"
            );
        }
        assert!(
            text.contains("sid-123"),
            "dynamic session ID changed: {text}"
        );
        assert!(text.contains("Grok"), "dynamic model name changed: {text}");
    }

    #[test]
    fn zh_locale_localizes_minimal_session_info_without_changing_values() {
        let text = localize_session_info_text(
            "  Title: demo\n  Auth method: OAuth\n  Run `grok-zh login` to use your SuperGrok subscription instead.\n  Session ID: sid-123\n  Working directory: C:\\repo\n  Model: grok-build",
            &zh_locale(),
        );
        for needle in [
            "标题: demo",
            "身份验证方式: OAuth",
            "运行 `grok-zh login`",
            "会话 ID: sid-123",
            "工作目录: C:\\repo",
            "模型: grok-build",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
    }
}
