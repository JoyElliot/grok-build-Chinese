use std::time::Duration;

use ratatui::text::Span;

use super::{
    HookRunCounts, HookRunEntry, HookRunStatus, ToolCallHookData, localized_fixed_hook_error,
    render_group_hook_counts_inline_suffix, render_hooks_inline_suffix, render_stop_hooks_summary,
};

fn run(status: HookRunStatus) -> HookRunEntry {
    HookRunEntry {
        name: "hook".to_owned(),
        status,
        output: None,
    }
}

fn text(spans: Vec<Span<'static>>) -> String {
    let mut text = String::new();
    for span in spans {
        text.push_str(span.content.as_ref());
    }
    text
}

#[test]
fn compact_suffix_keeps_blocked_and_failure_formatting() {
    let elapsed = Duration::from_millis(1);
    let data = ToolCallHookData {
        post_hooks: vec![
            run(HookRunStatus::Blocked {
                detail: "denied".to_owned(),
                elapsed,
            }),
            run(HookRunStatus::Failed {
                error: "exit 1".to_owned(),
                elapsed,
            }),
        ],
        ..ToolCallHookData::default()
    };
    assert_eq!(
        text(render_hooks_inline_suffix(&data).expect("hook suffix")),
        "  [hooks: 1/1]"
    );
    let stop_groups = [("stop".to_owned(), data.post_hooks)];
    assert_eq!(
        text(render_stop_hooks_summary(&stop_groups).expect("stop suffix")),
        "stop  [hooks: 1/1]"
    );
}

#[test]
fn zh_localization_group_hook_suffix_translates_fixed_status_labels() {
    let locale = crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
        locale: crate::locale::UiLocale::ZhCn,
        source: crate::locale::LocaleSource::Cli,
    });
    let spans = render_group_hook_counts_inline_suffix(
        HookRunCounts {
            success: 2,
            blocked: 1,
            failed: 1,
        },
        &crate::theme::Theme::current(),
        Some(&locale),
    )
    .expect("hook suffix");

    assert_eq!(text(spans), "  [钩子：2 成功, 1 已阻止, 1 失败]");
}

#[test]
fn zh_localization_hook_errors_preserves_protocol_fields_and_dynamic_detail() {
    let locale = crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
        locale: crate::locale::UiLocale::ZhCn,
        source: crate::locale::LocaleSource::Cli,
    });
    assert_eq!(
        localized_fixed_hook_error("client hook timed out", Some(&locale)),
        "客户端钩子超时"
    );
    assert_eq!(
        localized_fixed_hook_error(
            "updatedMCPToolOutput does not match the tool's kind",
            Some(&locale)
        ),
        "updatedMCPToolOutput 与工具类型不匹配"
    );
    assert_eq!(
        localized_fixed_hook_error(
            "updatedToolOutput failed to parse: unexpected token at line 4",
            Some(&locale)
        ),
        "updatedToolOutput 解析失败：unexpected token at line 4"
    );
    assert_eq!(
        localized_fixed_hook_error("provider-specific failure", Some(&locale)),
        "provider-specific failure"
    );
}
