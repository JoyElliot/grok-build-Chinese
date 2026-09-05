pub mod agent;
pub mod agent_status;
pub mod agents_modal;
pub mod announcements;
pub mod block_viewer;
pub mod btw_overlay;
pub mod completion_dropdown;
pub mod context_bar;
pub mod credit_bar;
pub mod dashboard;
pub mod debug_style;
pub mod dock;
pub mod elicitation_view;
pub mod extensions_modal;
pub mod file_search;
pub mod fps_hud;
pub mod goal_detail;
pub mod history_search;
pub mod import_claude_modal;
pub mod jump;
pub mod list_pane;
pub(crate) mod managed_mcp_localization;
pub mod mcps_modal;
pub mod memory_modal;
pub mod modal;
pub mod modal_window;
pub mod new_worktree_dialog;
pub mod overlay;
pub mod overlay_list;
pub mod permission_view;
pub mod persona_detail;
pub mod picker;
pub mod plan_approval_view;
pub mod privacy_banner;
pub mod progress_bar;
pub mod prompt_suggestion;
pub mod prompt_widget;
pub mod question_view;
pub mod queue_pane;
pub mod rewind;
pub mod scroll_debug_hud;
pub mod session_picker;
pub mod session_picker_surface;
pub mod session_title;
pub mod settings_modal;
pub mod shortcuts_bar;
pub mod shortcuts_help;
pub mod slash_dropdown;
pub mod status_bar;
pub mod status_line;
pub mod subagent_catalog_pane;
pub mod suggestion_controller;
pub mod tasks_pane;
pub mod timeline;
pub mod todo_pane;
pub mod turn_status;
pub mod tutorial;
pub mod usage_modal;
pub mod welcome;
pub mod workflows;

/// Format a model name with its display-only reasoning-effort label. The model
/// identifier and the canonical effort value stored in session state are never
/// changed; only the label painted in the UI is localized.
pub fn localized_model_name(
    model_name: impl Into<String>,
    reasoning_effort: Option<xai_grok_shell::sampling::types::ReasoningEffort>,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let model_name = model_name.into();
    let Some(effort) = reasoning_effort else {
        return model_name;
    };
    let effort = effort.as_str();
    let label = locale
        .map(|locale| {
            locale
                .named_text(&format!("reasoning_effort.{effort}.label"), effort)
                .into_owned()
        })
        .unwrap_or_else(|| effort.to_owned());
    format!("{model_name} ({label})")
}

#[cfg(test)]
mod locale_tests {
    use super::*;
    use crate::locale::{LocaleContext, LocaleSource, ResolvedLocale, UiLocale};
    use xai_grok_shell::sampling::types::ReasoningEffort;

    fn zh_locale() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    #[test]
    fn localization_regression_model_display_changes_only_effort_label() {
        let locale = zh_locale();
        assert_eq!(
            localized_model_name("Grok 4.5", Some(ReasoningEffort::High), Some(&locale)),
            "Grok 4.5 (高)"
        );
        assert_eq!(
            localized_model_name("Grok 4.5", Some(ReasoningEffort::High), None),
            "Grok 4.5 (high)"
        );
        assert_eq!(
            localized_model_name("grok-4.5", None, Some(&locale)),
            "grok-4.5"
        );
    }
}
