//! The agent dashboard lists every top-level agent and its subagents, grouped by state, with peek, attach, and dispatch actions.
//!
//! Owned by `AppView::dashboard` (`Option<DashboardState>`); active only when `app.active_view == ActiveView::AgentDashboard`.
//! State survives the user closing and reopening the dashboard within a single pager process (the `Option` is reset only on shutdown).
//!
//! ## Module layout
//!
//! - [`state`]: public `DashboardState`, `DashboardRowId`, `RowState`, `Grouping`, `Filter`, `FilterValue`, `PersistedDashboard`.
//! - [`row`]: `DashboardRow`, `build_rows()`, classifiers, sort.
//! - [`row_activity`]: parent activity, secondary-line text, and live-work badge counts.
//! - [`row_title`]: title, subtitle, and chip painting for wide and narrow rows.
//! - [`layout`]: pure rect computation.
//! - [`render`]: `Widget`-style rendering routine.
//! - [`chrome`]: the header row and the primary actions row above the list.
//! - [`actions_focus`]: the keyboard cursor on the actions row and its `←`/`→` walk.
//! - [`peek`]: peek panel state and rendering.
//! - [`usage_modal`]: input routing for the dashboard-hosted `/usage` modal.
//!
//! ## Lifetime
//!
//! Rows are rebuilt every render frame off `app.agents`; nothing is cached.
//! The per-row sort key (state and last_change_at) is recomputed each frame; with single-digit agent counts in one pager process this is free.

mod actions_focus;
pub(crate) mod animation;
mod chrome;
pub mod layout;
pub mod peek;
pub mod peek_tail;
pub mod render;
pub mod row;
mod row_activity;
mod row_title;
mod search;
pub mod state;
#[cfg(test)]
mod test_support;
mod usage_modal;

pub use chrome::HeaderUpgradeCta;
pub(crate) use render::render_dashboard;
pub(crate) use render::render_dashboard_with_locale;
pub use render::{popup_rect, render_popup_overlay, render_popup_overlay_with_locale};
pub use row::{
    DashboardRow, RowBadge, build_rows, build_rows_with_roster, classify_subagent,
    classify_top_level, roster_activity_to_state, sort_rows,
};
pub(crate) use row::{WorkspaceRowInputs, build_rows_with_workspace};
pub(crate) use state::DashboardStopAction;
pub use state::{
    DashboardDispatchMode, DashboardRowId, DashboardState, Filter, FilterValue, Focusable,
    Grouping, LocationCandidate, LocationPickerState, PendingDispatchModel, PersistedDashboard,
    PersistedRowId, RowState, SectionKey, SessionIdResolver, ShortcutsModalState, load_persisted,
    parse_filter, parse_row_state_token,
};

/// Localize age columns without changing their coarse time buckets.
pub(crate) fn format_time_ago_with_locale(
    elapsed: std::time::Duration,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let text = |id: &str, english: &'static str| {
        locale
            .map(|locale| locale.named_static_text(id, english))
            .unwrap_or(english)
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        return text("dashboard.time.just_now", "just now").to_string();
    }
    let (value, id, english) = if secs < 3_600 {
        (secs / 60, "dashboard.time.minutes", "{value}m")
    } else if secs < 86_400 {
        (secs / 3_600, "dashboard.time.hours", "{value}h")
    } else {
        let days = secs / 86_400;
        if days < 30 {
            (days, "dashboard.time.days", "{value}d")
        } else if days < 365 {
            (days / 30, "dashboard.time.months", "{value}mo")
        } else {
            (days / 365, "dashboard.time.years", "{value}y")
        }
    };
    text(id, english).replace("{value}", &value.to_string())
}

/// Top-level agents visible in the dashboard's row list, in the exact order [`render_dashboard`]
/// paints them. "Previous" / "next" then follow what the user actually sees instead of the agent
/// map's insertion order.
pub fn overlay_cycle_order(
    state: &DashboardState,
    agents: &indexmap::IndexMap<crate::app::agent::AgentId, crate::app::agent_view::AgentView>,
) -> Vec<crate::app::agent::AgentId> {
    let home = render::cached_home();
    let rows = build_rows(
        agents,
        &state.pinned,
        &state.reorder,
        state.grouping,
        &state.filter,
        home,
    );
    rows.iter()
        .filter_map(|r| match &r.id {
            DashboardRowId::TopLevel(id) if !r.is_more_placeholder => Some(*id),
            _ => None,
        })
        .collect()
}

/// The env override wins (`GROK_AGENT_DASHBOARD=0` turns the dashboard off), else the persisted `[dashboard].enabled` flag (default `true`).
/// The slash command and CLI subcommand check this before opening; on `false` they print a toast and stay where they are.
/// `var_os` avoids the per-call allocation of `var`.
pub fn dashboard_enabled() -> bool {
    if std::env::var_os("GROK_AGENT_DASHBOARD")
        .as_deref()
        .is_some_and(|v| v == std::ffi::OsStr::new("0"))
    {
        return false;
    }
    state::load_persisted_enabled().unwrap_or(true)
}

/// `None` when it is off: the tip would name a refused command, so callers fall back to a plain
/// session-id banner.
pub(crate) fn session_switch_hint_command(minimal: bool) -> Option<&'static str> {
    if minimal {
        Some("/resume")
    } else if dashboard_enabled() {
        Some("/dashboard")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_localization_dashboard_relative_time_preserves_bucket_boundaries() {
        let locale = crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::ZhCn,
            source: crate::locale::LocaleSource::Cli,
        });
        for (seconds, english, chinese) in [
            (0, "just now", "刚刚"),
            (59, "just now", "刚刚"),
            (60, "1m", "1分钟前"),
            (3_599, "59m", "59分钟前"),
            (3_600, "1h", "1小时前"),
            (86_399, "23h", "23小时前"),
            (86_400, "1d", "1天前"),
            (29 * 86_400, "29d", "29天前"),
            (30 * 86_400, "1mo", "1个月前"),
            (364 * 86_400, "12mo", "12个月前"),
            (365 * 86_400, "1y", "1年前"),
            (730 * 86_400, "2y", "2年前"),
        ] {
            let elapsed = std::time::Duration::from_secs(seconds);
            assert_eq!(format_time_ago_with_locale(elapsed, None), english);
            assert_eq!(format_time_ago_with_locale(elapsed, Some(&locale)), chinese);
            assert_eq!(crate::util::format_time_ago(elapsed), english);
        }
    }

    /// Minimal mode always points at `/resume`: the dashboard is refused there no matter what the feature flag says, so the hint must not depend on it.
    /// Runs under the same serial key as the other `GROK_AGENT_DASHBOARD` env-mutating tests.
    #[serial_test::serial(GROK_AGENT_DASHBOARD)]
    #[test]
    fn switch_hint_minimal_is_resume_even_with_dashboard_disabled() {
        // SAFETY: the test temporarily mutates a process-wide env var.
        // `serial_test`'s lock ensures no other test marked with the same
        // `GROK_AGENT_DASHBOARD` key reads it concurrently.
        unsafe { std::env::set_var("GROK_AGENT_DASHBOARD", "0") };
        assert_eq!(session_switch_hint_command(true), Some("/resume"));
        unsafe { std::env::remove_var("GROK_AGENT_DASHBOARD") };
    }

    /// Outside minimal the hint mirrors the dashboard flag.
    /// `None` when the env override disables it (the tip would name a refused command), otherwise whatever `dashboard_enabled()` says.
    /// The second assert checks consistency, not a fixed value, so the test doesn't depend on the machine's persisted `[dashboard].enabled`.
    #[serial_test::serial(GROK_AGENT_DASHBOARD)]
    #[test]
    fn switch_hint_non_minimal_follows_dashboard_flag() {
        // SAFETY: see above; serialized on the GROK_AGENT_DASHBOARD key
        unsafe { std::env::set_var("GROK_AGENT_DASHBOARD", "0") };
        assert_eq!(session_switch_hint_command(false), None);
        unsafe { std::env::remove_var("GROK_AGENT_DASHBOARD") };
        assert_eq!(
            session_switch_hint_command(false),
            dashboard_enabled().then_some("/dashboard")
        );
    }
}
