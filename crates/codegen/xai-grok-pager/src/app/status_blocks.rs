//! Read-only system-block text for `/queue`, `/tasks`, and `/usage`.
//!
//! Plain text committed into scrollback; minimal mode has no interactive panes, so these blocks are its main way to inspect that state.
//! The formatting lives outside `dispatch` so it is easy to unit test.
use crate::app::agent::{BgTaskState, BgTaskStatus};
use crate::app::agent_view::AgentView;
use crate::app::subagent::format_subagent_label_with_locale;
use crate::util::{format_duration, group_thousands};

fn status_text<'a>(
    locale: Option<&crate::locale::LocaleContext>,
    id: &str,
    english: &'a str,
) -> std::borrow::Cow<'a, str> {
    locale
        .map(|locale| locale.named_text(id, english))
        .unwrap_or_else(|| std::borrow::Cow::Borrowed(english))
}

fn localized_status(locale: Option<&crate::locale::LocaleContext>, status: &str) -> String {
    let english = status.replace('_', " ");
    let id = match status {
        "active" | "running" => Some("status.state.running"),
        "complete" | "done" => Some("status.state.done"),
        "stopping" => Some("status.state.stopping"),
        "failed" => Some("status.state.failed"),
        "scheduled" => Some("status.state.scheduled"),
        "cancelled" => Some("status.state.cancelled"),
        "interrupted" => Some("status.state.interrupted"),
        "blocked" => Some("status.state.blocked"),
        "user_paused" | "back_off_paused" | "no_progress_paused" | "infra_paused" => {
            Some("status.state.paused")
        }
        _ => None,
    };
    id.map(|id| status_text(locale, id, &english).into_owned())
        .unwrap_or(english)
}

fn status_column(locale: Option<&crate::locale::LocaleContext>, status: &str) -> String {
    let status = localized_status(locale, status);
    if locale.is_some_and(|locale| locale.locale() == crate::locale::UiLocale::ZhCn) {
        format!("{status} ")
    } else {
        format!("{status:<9}")
    }
}

impl BgTaskState {
    pub(crate) fn display_kind(&self) -> &'static str {
        if self.is_monitor { "Monitor" } else { "Task" }
    }
}
/// `/queue` body: a read-only list of the queued prompts.
/// Rows from the server's shared queue (minus the prompt already running) come first in broadcast order, then the local queue (`pending_prompts`).
/// This matches [`crate::views::queue_pane::QueuePane::sync_from_merged`]'s ordering.
pub(crate) fn queue_block_text(agent: &AgentView) -> String {
    queue_block_text_with_locale(agent, None)
}

/// `/queue` body — a read-only list of the queued prompts.
///
/// Server-authoritative shared-queue rows (the in-flight prompt excluded) come
/// first in broadcast order, then the local drip-feed queue — matching
/// [`crate::views::queue_pane::QueuePane::sync_from_merged`]'s ordering.
pub(crate) fn queue_block_text_with_locale(
    agent: &AgentView,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let running_id = agent.session.current_prompt_id.as_deref();
    let mut rows: Vec<String> = Vec::new();
    let mut pos = 1usize;
    for wire in &agent.shared_queue {
        if running_id == Some(wire.id.as_str()) {
            continue;
        }
        rows.push(format_queue_row_with_locale(pos, &wire.text, locale));
        pos += 1;
    }
    for prompt in &agent.session.pending_prompts {
        rows.push(format_queue_row_with_locale(pos, &prompt.text, locale));
        pos += 1;
    }
    if rows.is_empty() {
        status_text(locale, "status.queue.empty", "Queue is empty.").into_owned()
    } else {
        let english = format!(
            "Queued prompt{} ({}):",
            if rows.len() == 1 { "" } else { "s" },
            rows.len()
        );
        let header = status_text(locale, "status.queue.header", &english)
            .replace("{count}", &rows.len().to_string());
        join_header_rows(header, rows)
    }
}
/// `/tasks` body: [`crate::views::tasks_pane::TasksPane`] without its styled rows.
pub(crate) fn tasks_block_text(agent: &AgentView) -> String {
    tasks_block_text_with_locale(agent, None)
}

pub(crate) fn tasks_block_text_with_locale(
    agent: &AgentView,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let mut rows: Vec<String> = Vec::new();
    let mut workflows: Vec<_> = agent.workflow_runs.iter().collect();
    workflows.sort_by(|a, b| {
        b.is_active()
            .cmp(&a.is_active())
            .then(b.received_at.cmp(&a.received_at))
            .then(a.run_id.cmp(&b.run_id))
    });
    for run in workflows {
        let active = run.active_agent_count();
        let agents = match active {
            0 => String::new(),
            n => format!(
                " · {}",
                status_text(locale, "status.tasks.agent_count", "{count} agents")
                    .replace("{count}", &n.to_string())
            ),
        };
        let phase = run
            .current_phase
            .as_deref()
            .map(str::trim)
            .filter(|phase| !phase.is_empty())
            .map(|phase| format!(" · {phase}"))
            .unwrap_or_default();
        let status = if run.is_active() {
            "running"
        } else {
            run.status.as_str()
        };
        rows.push(format!(
            "  {}{} · {}{phase}{agents}  ({})",
            status_column(locale, status),
            status_text(locale, "status.tasks.workflow", "Workflow"),
            run.name,
            format_duration(std::time::Duration::from_millis(run.live_elapsed_ms()))
        ));
    }
    let mut subs: Vec<_> = agent
        .subagent_sessions
        .values()
        .filter(|s| s.attempt.workflow_run_id.is_none())
        .collect();
    subs.sort_by(|a, b| {
        b.is_running()
            .cmp(&a.is_running())
            .then(b.attempt.started_at.cmp(&a.attempt.started_at))
            .then(a.child_session_id.cmp(&b.child_session_id))
    });
    for info in subs {
        let (type_label, desc) = format_subagent_label_with_locale(info, locale);
        let status = if info.attempt.pending_kill {
            "stopping"
        } else if info.is_running() {
            "running"
        } else {
            info.attempt.status.as_deref().unwrap_or("done")
        };
        let label = if desc.is_empty() {
            type_label
        } else {
            format!("{type_label} · {desc}")
        };
        rows.push(format!(
            "  {}{label}  ({})",
            status_column(locale, status),
            format_duration(info.display_elapsed())
        ));
    }
    let mut tasks: Vec<_> = agent.session.bg_tasks.values().collect();
    tasks.sort_by(|a, b| {
        let (ar, br) = (
            a.status == BgTaskStatus::Running,
            b.status == BgTaskStatus::Running,
        );
        br.cmp(&ar)
            .then(b.start_time.cmp(&a.start_time))
            .then(a.task_id.cmp(&b.task_id))
    });
    for task in tasks {
        let kind = if task.is_monitor {
            status_text(locale, "status.tasks.monitor", "Monitor")
        } else {
            status_text(locale, "status.tasks.task", "Task")
        };
        let one_line = task
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| first_nonempty_line(&task.command));
        let status = if task.pending_kill {
            "stopping"
        } else {
            match task.status {
                BgTaskStatus::Running => "running",
                BgTaskStatus::Done => "done",
                BgTaskStatus::Failed => "failed",
            }
        };
        rows.push(format!(
            "  {}{kind} · {one_line}  ({})",
            status_column(locale, status),
            format_duration(task.elapsed())
        ));
    }
    let mut sched: Vec<_> = agent.session.scheduled_tasks.values().collect();
    sched.sort_by(|a, b| {
        a.tag
            .cmp(&b.tag)
            .then(a.human_schedule.cmp(&b.human_schedule))
            .then(a.task_id.cmp(&b.task_id))
    });
    for info in sched {
        rows.push(format!(
            "  {}{} · {} · {}",
            status_column(locale, "scheduled"),
            info.tag,
            info.human_schedule,
            first_nonempty_line(&info.prompt)
        ));
    }
    if rows.is_empty() {
        status_text(
            locale,
            "status.tasks.empty",
            "No background tasks, workflows, or subagents.",
        )
        .into_owned()
    } else {
        let english = format!(
            "Task{} ({}):",
            if rows.len() == 1 { "" } else { "s" },
            rows.len()
        );
        let header = status_text(locale, "status.tasks.header", &english)
            .replace("{count}", &rows.len().to_string());
        join_header_rows(header, rows)
    }
}
/// `/usage` body: per-session token and cost totals, covering the ledger's lifetime (since session start, or since the last `/resume`).
pub(crate) fn session_usage_block_text(
    usage: &xai_grok_shell::extensions::notification::PromptUsage,
) -> String {
    session_usage_block_text_with_locale(usage, None)
}

pub(crate) fn session_usage_block_text_with_locale(
    usage: &xai_grok_shell::extensions::notification::PromptUsage,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let t = &usage.totals;
    if t.model_calls == 0 && usage.model_usage.is_empty() {
        return if usage.usage_is_incomplete {
            status_text(
                locale,
                "status.usage.empty_incomplete",
                "Session usage: none recorded, but tracking is incomplete and may under-count.",
            )
            .into_owned()
        } else {
            status_text(
                locale,
                "status.usage.empty",
                "Session usage: no model calls yet in this session.",
            )
            .into_owned()
        };
    }
    let mut rows = Vec::new();
    rows.push(
        status_text(
            locale,
            "status.usage.input_tokens",
            "  Input tokens:   {input} ({cached} cached)",
        )
        .replace("{input}", &group_thousands(t.input_tokens))
        .replace("{cached}", &group_thousands(t.cached_read_tokens)),
    );
    rows.push(
        status_text(
            locale,
            "status.usage.output_tokens",
            "  Output tokens:  {output} ({reasoning} reasoning)",
        )
        .replace("{output}", &group_thousands(t.output_tokens))
        .replace("{reasoning}", &group_thousands(t.reasoning_tokens)),
    );
    rows.push(
        status_text(
            locale,
            "status.usage.total_tokens",
            "  Total tokens:   {total}",
        )
        .replace("{total}", &group_thousands(t.total_tokens)),
    );
    rows.push(
        status_text(
            locale,
            "status.usage.model_calls",
            "  Model calls:    {calls} · API time: {time}",
        )
        .replace("{calls}", &group_thousands(t.model_calls))
        .replace(
            "{time}",
            &format_duration(std::time::Duration::from_millis(t.api_duration_ms)),
        ),
    );
    rows.push(
        status_text(locale, "status.usage.cost", "  Cost:           {cost}")
            .replace("{cost}", &format_cost(t, locale)),
    );

    if usage.model_usage.len() > 1 {
        rows.push(String::new());
        rows.push(status_text(locale, "status.usage.by_model", "  By model:").into_owned());
        for (model, m) in &usage.model_usage {
            rows.push(format_by_model_row(model, m));
        }
    }
    if usage.usage_is_incomplete {
        rows.push(
            status_text(
                locale,
                "status.usage.incomplete_note",
                "  Note: usage is incomplete and may under-count.",
            )
            .into_owned(),
        );
    }
    join_header_rows(
        status_text(
            locale,
            "status.usage.header",
            "Session usage (since start or last resume):",
        )
        .into_owned(),
        rows,
    )
}
/// Formats one `By model:` row. The cost cell is omitted when `cost_usd_ticks` is `None`.
fn format_by_model_row(
    model: &str,
    m: &xai_grok_shell::extensions::notification::PromptUsageModel,
) -> String {
    let tokens = group_thousands(m.total_tokens);
    if m.cost_usd_ticks.is_some() {
        format!("    {model}: {tokens} Tokens · {}", format_cost(m, None))
    } else {
        format!("    {model}: {tokens} Tokens")
    }
}

/// Cost cell. Ticks are 1e10 per USD; partial sums are scrubbed to absent.
fn format_cost(
    m: &xai_grok_shell::extensions::notification::PromptUsageModel,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    use xai_grok_shell::extensions::notification::ticks_to_usd;
    match m.cost_usd_ticks {
        Some(ticks) => format!("${:.4}", ticks_to_usd(ticks)),
        None if m.cost_is_partial => status_text(
            locale,
            "status.usage.cost_partial",
            "not available (not reported for some calls)",
        )
        .into_owned(),
        None => status_text(
            locale,
            "status.usage.cost_unavailable",
            "not available (not reported)",
        )
        .into_owned(),
    }
}
/// First non-empty, trimmed line of `text` (empty string if none). Collapses a multi-line prompt/command to a single display line.
pub(crate) fn first_nonempty_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

/// Format one `/queue` row as `  #N  <first non-empty line>` with a
/// `(+K more lines)` suffix for multi-line prompts.
#[cfg(test)]
fn format_queue_row(pos: usize, text: &str) -> String {
    format_queue_row_with_locale(pos, text, None)
}

fn format_queue_row_with_locale(
    pos: usize,
    text: &str,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let first_line = first_nonempty_line(text);
    let extra = text.lines().count().saturating_sub(1);
    if extra > 0 {
        let english = format!("(+{extra} more line{})", if extra == 1 { "" } else { "s" });
        let suffix = status_text(locale, "status.queue.more_lines", &english)
            .replace("{count}", &extra.to_string());
        format!("  #{pos}  {first_line}  {suffix}")
    } else {
        format!("  #{pos}  {first_line}")
    }
}
fn join_header_rows(header: String, rows: Vec<String>) -> String {
    std::iter::once(header)
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::subagent::SubagentInfo;
    use xai_grok_shell::extensions::notification::{PromptUsage, PromptUsageModel};

    fn zh_locale() -> crate::locale::LocaleContext {
        crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::ZhCn,
            source: crate::locale::LocaleSource::Cli,
        })
    }

    fn explore_subagent() -> SubagentInfo {
        let mut info = crate::app::subagent::test_support::make_info();
        info.child_session_id = "child-1".into();
        info.description = "Workspace smoke-test probe".into();
        info.attempt.role = Some("explore".into());
        info
    }
    fn model_row(input: u64, output: u64, ticks: Option<i64>) -> PromptUsageModel {
        PromptUsageModel {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            cached_read_tokens: 0,
            cache_creation_tokens: 0,
            reasoning_tokens: 0,
            model_calls: 1,
            api_duration_ms: 1_000,
            cost_usd_ticks: ticks,
            cost_is_partial: false,
            cost_missing_calls: 0,
        }
    }
    #[test]
    fn session_usage_block_empty_ledger() {
        let usage = PromptUsage::default();
        assert_eq!(
            session_usage_block_text(&usage),
            "Session usage: no model calls yet in this session."
        );
        let incomplete = PromptUsage {
            usage_is_incomplete: true,
            ..Default::default()
        };
        assert!(session_usage_block_text(&incomplete).contains("incomplete"));
    }
    #[test]
    fn zh_localization_tasks_block_localizes_builtin_type_but_not_description() {
        let mut agent = crate::app::agent_view::test_fixtures::make_agent();
        agent
            .subagent_sessions
            .insert("child-1".to_string(), explore_subagent());
        let locale = zh_locale();
        let text = tasks_block_text_with_locale(&agent, Some(&locale));
        assert!(text.contains("探索 · Workspace smoke-test probe"), "{text}");
        assert!(!text.contains("Explore"), "{text}");
    }

    #[test]
    fn session_usage_block_formats_tokens_and_cost() {
        let mut totals = model_row(1_234_567, 45_678, Some(12_345_000_000));
        totals.cached_read_tokens = 1_000_000;
        totals.reasoning_tokens = 12_000;
        totals.model_calls = 42;
        totals.api_duration_ms = 192_000;
        let usage = PromptUsage {
            totals,
            ..Default::default()
        };
        let text = session_usage_block_text(&usage);
        insta::assert_snapshot!("session_usage_block_full", text);
    }
    #[test]
    fn localization_regression_session_usage_preserves_models_and_numbers() {
        let mut totals = model_row(1_234_567, 45_678, None);
        totals.cached_read_tokens = 1_000_000;
        totals.reasoning_tokens = 12_000;
        totals.model_calls = 42;
        let mut usage = PromptUsage {
            totals,
            usage_is_incomplete: true,
            ..Default::default()
        };
        usage
            .model_usage
            .insert("grok-4.5".into(), model_row(1_200_000, 40_000, None));
        usage
            .model_usage
            .insert("grok-4".into(), model_row(34_567, 5_678, None));
        let locale = zh_locale();
        let text = session_usage_block_text_with_locale(&usage, Some(&locale));
        for expected in [
            "会话用量（自启动或上次恢复以来）：",
            "输入 Token：1,234,567",
            "输出 Token：45,678",
            "模型调用：42",
            "按模型：",
            "grok-4.5",
            "grok-4",
            "统计结果可能偏低",
        ] {
            assert!(text.contains(expected), "missing {expected:?}: {text}");
        }
        assert!(!text.contains("Input tokens"), "{text}");
        assert!(!text.contains("By model"), "{text}");
    }

    #[test]
    fn session_usage_block_lists_models_when_multiple() {
        let mut usage = PromptUsage {
            totals: model_row(150, 15, None),
            ..Default::default()
        };
        usage
            .model_usage
            .insert("grok-build".into(), model_row(100, 10, None));
        usage
            .model_usage
            .insert("grok-4".into(), model_row(50, 5, None));
        let text = session_usage_block_text(&usage);
        insta::assert_snapshot!("session_usage_block_by_model", text);
    }
    #[test]
    fn session_usage_block_by_model_shows_cost_only_when_known() {
        let mut usage = PromptUsage {
            totals: model_row(123_556, 1_010, Some(20_000_000)),
            ..Default::default()
        };
        usage.model_usage.insert(
            "grok-4.7-build".into(),
            model_row(123_456, 1_000, Some(20_000_000)),
        );
        usage
            .model_usage
            .insert("grok-4".into(), model_row(100, 10, None));
        let text = session_usage_block_text(&usage);
        let rows: Vec<&str> = text
            .lines()
            .skip_while(|line| *line != "  By model:")
            .skip(1)
            .collect();
        assert_eq!(
            rows,
            [
                "    grok-4.7-build: 124,456 Tokens · $0.0020",
                "    grok-4: 110 Tokens",
            ],
            "{text}"
        );
    }
    #[test]
    fn session_usage_block_absent_cost_is_unknown_not_free() {
        let usage = PromptUsage {
            totals: model_row(100, 10, None),
            ..Default::default()
        };
        let text = session_usage_block_text(&usage);
        insta::assert_snapshot!("session_usage_block_absent_cost", text);
        assert!(!text.contains("$0"), "{text}");
    }
    #[test]
    fn session_usage_block_flags_partial_and_incomplete() {
        let mut totals = model_row(100, 10, None);
        totals.cost_is_partial = true;
        let usage = PromptUsage {
            totals,
            usage_is_incomplete: true,
            ..Default::default()
        };
        let text = session_usage_block_text(&usage);
        assert!(text.contains("not reported for some calls"), "{text}");
        assert!(text.contains("usage is incomplete"), "{text}");
    }
    #[test]
    fn group_thousands_groups_digits() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_000), "1,000");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }
    #[test]
    fn first_nonempty_line_skips_blank_leading_lines() {
        assert_eq!(first_nonempty_line("\n  \n  hello \nworld"), "hello");
        assert_eq!(first_nonempty_line("   "), "");
        assert_eq!(first_nonempty_line(""), "");
        assert_eq!(first_nonempty_line("only"), "only");
    }
    #[test]
    fn format_queue_row_single_line() {
        assert_eq!(format_queue_row(1, "fix the bug"), "  #1  fix the bug");
    }
    #[test]
    fn format_queue_row_multiline_reports_extra_lines() {
        assert_eq!(
            format_queue_row(2, "first\nsecond"),
            "  #2  first  (+1 more line)"
        );
        assert_eq!(
            format_queue_row(3, "first\nsecond\nthird"),
            "  #3  first  (+2 more lines)"
        );
        let locale = zh_locale();
        assert_eq!(
            format_queue_row_with_locale(3, "first\nsecond\nthird", Some(&locale)),
            "  #3  first  （另有 2 行）"
        );
    }
}
