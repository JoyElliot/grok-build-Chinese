//! Scrollback entries for the subagent lifecycle.
//!
//! Similar to BgTaskBlock: always collapsed, an animated bullet while running, a colored bullet when done.
//! Enter / Ctrl-F opens the subagent view.
//!
//! Two modes:
//! - **Blocking** (sync): one `Started` block; it blinks while running and turns green/red when done. Text: `Subagent "description"`
//! - **Background** (async): the `Started` block stays forever (turns gray) and a separate `Completed`/`Failed` block is added when done.
//!   Started text: `Subagent started: "description"`
//!   Completed text: `Subagent completed in 43s: "description"`

use std::time::Duration;

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::app::subagent::format_subagent_meta;
use crate::appearance::AppearanceConfig;
use crate::render::color::blend_color;
use crate::render::line_utils::truncate_str;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{AccentStyle, BlockContext, BlockOutput, DisplayMode};
use crate::theme::Theme;
use crate::util::format_duration;

/// What kind of subagent lifecycle event this block represents.
#[derive(Debug, Clone)]
pub enum SubagentBlockKind {
    /// Subagent is running (or was running; `finish_running` stops animation).
    Started,
    /// Subagent completed successfully.
    Completed { elapsed: Duration },
    /// Subagent failed.
    Failed {
        elapsed: Duration,
        error: Option<String>,
    },
    /// Subagent was cancelled.
    Cancelled { elapsed: Duration },
}

/// Always collapsed and not foldable; groupable and selectable.
/// Enter / Ctrl-F opens the subagent view.
#[derive(Debug, Clone)]
pub struct SubagentBlock {
    /// Human-readable description of the task.
    pub description: String,
    /// Child session ID (for opening the subagent view).
    pub child_session_id: String,
    /// Subagent type (e.g. "general-purpose", "explore").
    pub subagent_type: String,
    /// Named persona applied to this subagent, if any.
    pub persona: Option<String>,
    /// Role that supplied defaults for this subagent, if any.
    pub role: Option<String>,
    /// Effective model ID used by the subagent, if available.
    pub model: Option<String>,
    /// Whether the subagent was launched in background mode.
    pub is_background: bool,
    /// Lifecycle kind.
    pub kind: SubagentBlockKind,
    /// Live activity label from the child session's turn tracker.
    ///
    /// Updated on each `SubagentProgress` tick while the subagent is running.
    /// Shown inline in the collapsed scrollback line (e.g. "Thinking", "Running: cargo build").
    /// The user sees interactive progress without opening the subagent view.
    pub activity_label: Option<String>,
}

impl SubagentBlock {
    /// Create a "Subagent started" block (for both sync and async).
    pub fn started(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        subagent_type: impl Into<String>,
        persona: Option<String>,
        role: Option<String>,
        model: Option<String>,
        is_background: bool,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: subagent_type.into(),
            persona,
            role,
            model,
            is_background,
            kind: SubagentBlockKind::Started,
            activity_label: None,
        }
    }

    /// Create a "Subagent completed" block (background mode only).
    pub fn completed(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Completed { elapsed },
            activity_label: None,
        }
    }

    /// Create a "Subagent failed" block (background mode only).
    pub fn failed(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
        error: Option<String>,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Failed { elapsed, error },
            activity_label: None,
        }
    }

    /// Create a "Subagent cancelled" block (background mode only).
    pub fn cancelled(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Cancelled { elapsed },
            activity_label: None,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.kind, SubagentBlockKind::Started)
    }
}

/// Truncate description and wrap in quotes for display.
fn quoted_desc(desc: &str, max_width: usize) -> String {
    // Reserve 2 chars for quotes
    if max_width <= 2 {
        return "\u{201C}\u{2026}\u{201D}".to_string(); // "…"
    }
    let inner = truncate_str(desc, max_width - 2);
    format!("\u{201C}{inner}\u{201D}")
}

fn localized_subagent_error(locale: &crate::locale::LocaleContext, error: &str) -> String {
    const TRUNCATED_SUFFIX: &str = "; the subagent's turn was truncated by the output token limit — the structured answer is likely incomplete";
    let (base, was_truncated) = error
        .strip_suffix(TRUNCATED_SUFFIX)
        .map_or((error, false), |base| (base, true));
    let text = |id: &str, english: &str| locale.named_text(id, english).into_owned();

    let localized = if let Some((id, english)) = match base {
        "structured output requested but none produced" => Some((
            "scrollback.subagent.error.structured_output_missing",
            "structured output requested but none produced",
        )),
        "Parent follow-up unexpectedly produced structured output" => Some((
            "scrollback.subagent.error.parent_followup_structured",
            "Parent follow-up unexpectedly produced structured output",
        )),
        "Subagent turn was rewound" => Some((
            "scrollback.subagent.error.rewound",
            "Subagent turn was rewound",
        )),
        "Subagent turn was removed before it ran" => Some((
            "scrollback.subagent.error.removed_before_run",
            "Subagent turn was removed before it ran",
        )),
        "Subagent turn was cancelled: user rejected a permission prompt" => Some((
            "scrollback.subagent.error.permission_rejected",
            "Subagent turn was cancelled: user rejected a permission prompt",
        )),
        "Subagent turn was cancelled: user cancelled a permission prompt" => Some((
            "scrollback.subagent.error.permission_cancelled",
            "Subagent turn was cancelled: user cancelled a permission prompt",
        )),
        "Subagent turn was cancelled: blocked by a hook" => Some((
            "scrollback.subagent.error.hook_blocked",
            "Subagent turn was cancelled: blocked by a hook",
        )),
        "Subagent turn was cancelled: aborted mid-turn" => Some((
            "scrollback.subagent.error.mid_turn_abort",
            "Subagent turn was cancelled: aborted mid-turn",
        )),
        "Subagent turn was cancelled" => Some((
            "scrollback.subagent.error.turn_cancelled",
            "Subagent turn was cancelled",
        )),
        "Subagent was cancelled" => Some((
            "scrollback.subagent.error.cancelled",
            "Subagent was cancelled",
        )),
        "Subagent initial prompt was not admitted before the deadline" => Some((
            "scrollback.subagent.error.initial_prompt_timeout",
            "Subagent initial prompt was not admitted before the deadline",
        )),
        "Child session dropped unexpectedly" => Some((
            "scrollback.subagent.error.child_session_dropped",
            "Child session dropped unexpectedly",
        )),
        "interrupted by process restart" => Some((
            "scrollback.subagent.error.process_restart",
            "interrupted by process restart",
        )),
        "orphaned while parent session stayed live" => Some((
            "scrollback.subagent.error.orphaned",
            "orphaned while parent session stayed live",
        )),
        "Unknown subagent error" => Some((
            "scrollback.subagent.error.unknown",
            "Unknown subagent error",
        )),
        _ => None,
    } {
        text(id, english)
    } else if let Some(detail) = base.strip_prefix("structured output validation failed: ") {
        text(
            "scrollback.subagent.error.structured_validation_failed",
            "structured output validation failed: {error}",
        )
        .replace("{error}", detail)
    } else if let Some(detail) = base.strip_prefix("Session error: ") {
        text(
            "scrollback.subagent.error.session_error",
            "Session error: {error}",
        )
        .replace("{error}", detail)
    } else if let Some(limit) = base
        .strip_prefix("max turns reached (limit: ")
        .and_then(|rest| rest.strip_suffix(')'))
        .filter(|limit| !limit.is_empty() && limit.chars().all(|c| c.is_ascii_digit()))
    {
        text(
            "scrollback.subagent.error.max_turns",
            "max turns reached (limit: {limit})",
        )
        .replace("{limit}", limit)
    } else if let Some(detail) =
        base.strip_prefix("Subagent turn was cancelled: user rejected permission — ")
    {
        text(
            "scrollback.subagent.error.permission_rejected_detail",
            "Subagent turn was cancelled: user rejected permission — {detail}",
        )
        .replace("{detail}", detail)
    } else if let Some(detail) = base.strip_prefix("Subagent turn was cancelled: hook denied — ")
    {
        text(
            "scrollback.subagent.error.hook_denied_detail",
            "Subagent turn was cancelled: hook denied — {detail}",
        )
        .replace("{detail}", detail)
    } else if let Some((requested, source)) = base
        .strip_prefix("Cannot resume with subagent_type '")
        .and_then(|rest| rest.split_once("': source subagent was '"))
        .and_then(|(requested, rest)| {
            rest.strip_suffix("'. Resumed sessions must use the same subagent type as the source.")
                .map(|source| (requested, source))
        })
        .filter(|(requested, source)| !requested.is_empty() && !source.is_empty())
    {
        replace_placeholders_once(
            &text(
                "scrollback.subagent.error.resume_type_mismatch",
                "Cannot resume with subagent_type '{requested}': source subagent was '{source}'. Resumed sessions must use the same subagent type as the source.",
            ),
            &[("{requested}", requested), ("{source}", source)],
        )
    } else if let Some((requested, source)) = base
        .strip_prefix("Cannot resume with persona '")
        .and_then(|rest| rest.split_once("': source subagent used "))
        .and_then(|(requested, rest)| {
            rest.strip_suffix(". Resumed sessions must use the same persona as the source.")
                .map(|source| (requested, source))
        })
        .filter(|(requested, source)| !requested.is_empty() && !source.is_empty())
    {
        replace_placeholders_once(
            &text(
                "scrollback.subagent.error.resume_persona_mismatch",
                "Cannot resume with persona '{requested}': source subagent used {source}. Resumed sessions must use the same persona as the source.",
            ),
            &[("{requested}", requested), ("{source}", source)],
        )
    } else if let Some((subagent, reason)) = base
        .strip_prefix("Cannot resume from subagent '")
        .and_then(|rest| rest.split_once("': "))
        .filter(|(subagent, _)| !subagent.is_empty())
    {
        localized_resume_error(locale, subagent, reason).unwrap_or_else(|| base.to_owned())
    } else {
        base.to_owned()
    };

    if was_truncated {
        text(
            "scrollback.subagent.error.truncated_suffix",
            "{error}; the subagent's turn was truncated by the output token limit — the structured answer is likely incomplete",
        )
        .replace("{error}", &localized)
    } else {
        localized
    }
}

fn localized_resume_error(
    locale: &crate::locale::LocaleContext,
    subagent: &str,
    reason: &str,
) -> Option<String> {
    let text = |id: &str, english: &str| locale.named_text(id, english).into_owned();

    match reason {
        "it is still running. Wait for it to complete before resuming." => {
            Some(replace_placeholders_once(
                &text(
                    "scrollback.subagent.error.resume_still_running",
                    "Cannot resume from subagent '{subagent}': it is still running. Wait for it to complete before resuming.",
                ),
                &[("{subagent}", subagent)],
            ))
        }
        "not found. The subagent may have been evicted or the ID is invalid." => {
            Some(replace_placeholders_once(
                &text(
                    "scrollback.subagent.error.resume_not_found",
                    "Cannot resume from subagent '{subagent}': not found. The subagent may have been evicted or the ID is invalid.",
                ),
                &[("{subagent}", subagent)],
            ))
        }
        "copied transcript is empty" => Some(replace_placeholders_once(
            &text(
                "scrollback.subagent.error.resume_empty",
                "Cannot resume from subagent '{subagent}': copied transcript is empty",
            ),
            &[("{subagent}", subagent)],
        )),
        _ => {
            if let Some(model) = reason
                .strip_prefix("source model '")
                .and_then(|rest| {
                    rest.strip_suffix("' is no longer available in the model catalogue.")
                })
                .filter(|model| !model.is_empty())
            {
                return Some(replace_placeholders_once(
                    &text(
                        "scrollback.subagent.error.resume_source_model_unavailable",
                        "Cannot resume from subagent '{subagent}': source model '{model}' is no longer available in the model catalogue.",
                    ),
                    &[("{subagent}", subagent), ("{model}", model)],
                ));
            }

            for (prefix, id, english) in [
                (
                    "failed to load copied transcript: ",
                    "scrollback.subagent.error.resume_load_failed",
                    "Cannot resume from subagent '{subagent}': failed to load copied transcript: {error}",
                ),
                (
                    "failed to copy source session data: ",
                    "scrollback.subagent.error.resume_copy_failed",
                    "Cannot resume from subagent '{subagent}': failed to copy source session data: {error}",
                ),
            ] {
                if let Some(detail) = reason
                    .strip_prefix(prefix)
                    .filter(|detail| !detail.is_empty())
                {
                    return Some(replace_placeholders_once(
                        &text(id, english),
                        &[("{subagent}", subagent), ("{error}", detail)],
                    ));
                }
            }

            let rest = reason.strip_prefix("source transcript (~")?;
            if let Some((estimated, rest)) = rest.split_once(" tokens) exceeds the resume limit (")
                && let Some((limit, rest)) = rest.split_once(" of ")
                && let Some(context) = rest.strip_suffix(
                    " tokens). Compact the source first, or resume on a model with a larger context window.",
                )
                && [estimated, limit, context]
                    .iter()
                    .all(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            {
                return Some(replace_placeholders_once(
                    &text(
                        "scrollback.subagent.error.resume_limit",
                        "Cannot resume from subagent '{subagent}': source transcript (~{estimated} tokens) exceeds the resume limit ({limit} of {context} tokens). Compact the source first, or resume on a model with a larger context window.",
                    ),
                    &[
                        ("{subagent}", subagent),
                        ("{estimated}", estimated),
                        ("{limit}", limit),
                        ("{context}", context),
                    ],
                ));
            }
            if let Some((estimated, rest)) = rest.split_once(
                " tokens) is over the auto-compact threshold (",
            ) && let Some((threshold, rest)) = rest.split_once("% of ")
                && let Some(context) = rest.strip_suffix(
                    "), but this child has an output token budget and cannot compact a fat transcript. Compact the source first, or resume without a budget.",
                )
                && [estimated, threshold, context]
                    .iter()
                    .all(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            {
                return Some(replace_placeholders_once(
                    &text(
                        "scrollback.subagent.error.resume_budgeted_compaction",
                        "Cannot resume from subagent '{subagent}': source transcript (~{estimated} tokens) is over the auto-compact threshold ({threshold}% of {context}), but this child has an output token budget and cannot compact a fat transcript. Compact the source first, or resume without a budget.",
                    ),
                    &[
                        ("{subagent}", subagent),
                        ("{estimated}", estimated),
                        ("{threshold}", threshold),
                        ("{context}", context),
                    ],
                ));
            }
            None
        }
    }
}

fn replace_placeholders_once(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    loop {
        let Some((index, placeholder, value)) = replacements
            .iter()
            .filter_map(|(placeholder, value)| {
                remaining
                    .find(placeholder)
                    .map(|index| (index, *placeholder, *value))
            })
            .min_by_key(|(index, _, _)| *index)
        else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..index]);
        output.push_str(value);
        remaining = &remaining[index + placeholder.len()..];
    }
    output
}

impl BlockContent for SubagentBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        let theme = Theme::current();
        // When selected, lift only the bold "Subagent" label to `text_primary` so it reads as undimmed
        // This mirrors `read.rs` and `search.rs`, which bump only the label and leave the rest at `muted`
        // The detail text (verb, description, meta) stays muted in every state
        let bold = if ctx.is_selected {
            theme.primary().add_modifier(Modifier::BOLD)
        } else {
            theme.muted().add_modifier(Modifier::BOLD)
        };
        let muted = theme.muted();
        let w = ctx.width as usize;
        let subagent_label = ctx
            .locale
            .named_static_text("scrollback.subagent.label", "Subagent ");

        let line = match (&self.kind, self.is_background) {
            (SubagentBlockKind::Started, bg) => {
                let verb = if bg {
                    ctx.locale
                        .named_static_text("scrollback.subagent.started", "started: ")
                } else {
                    ctx.locale
                        .named_static_text("scrollback.subagent.running", "running: ")
                };
                let activity_suffix: String = self
                    .activity_label
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|a| format!(" \u{00b7} {a}"))
                    .unwrap_or_default();
                let meta = format_subagent_meta(
                    self.persona.as_deref(),
                    self.role.as_deref(),
                    self.model.as_deref(),
                );
                let overhead =
                    subagent_label.width() + verb.width() + meta.width() + activity_suffix.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(overhead));
                let mut spans = vec![
                    Span::styled(subagent_label, bold),
                    Span::styled(verb, muted),
                    Span::styled(desc, muted),
                ];
                if !activity_suffix.is_empty() {
                    spans.push(Span::styled(activity_suffix, muted));
                }
                spans.push(Span::styled(meta, muted));
                Line::from(spans)
            }
            // Completed: Subagent completed in Xs: "description"
            (SubagentBlockKind::Completed { elapsed }, _) => {
                let time_str = format_duration(*elapsed);
                let detail = ctx
                    .locale
                    .named_text("scrollback.subagent.completed", "completed in {duration}: ")
                    .replace("{duration}", &time_str);
                let prefix_len = subagent_label.width() + detail.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                Line::from(vec![
                    Span::styled(subagent_label, bold),
                    Span::styled(detail, muted),
                    Span::styled(desc, muted),
                ])
            }
            // Failed: Subagent failed in Xs: "description"
            (SubagentBlockKind::Failed { elapsed, error }, _) => {
                let time_str = format_duration(*elapsed);
                let error_detail = error
                    .as_deref()
                    .map(|e| format!(" ({})", localized_subagent_error(&ctx.locale, e)))
                    .unwrap_or_default();
                let detail = ctx
                    .locale
                    .named_text(
                        "scrollback.subagent.failed",
                        "failed in {duration}{detail}: ",
                    )
                    .replace("{duration}", &time_str)
                    .replace("{detail}", &error_detail);
                let prefix_len = subagent_label.width() + detail.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                Line::from(vec![
                    Span::styled(subagent_label, bold),
                    Span::styled(detail, muted),
                    Span::styled(desc, muted),
                ])
            }
            // Cancelled: Subagent cancelled in Xs: "description"
            (SubagentBlockKind::Cancelled { elapsed }, _) => {
                let time_str = format_duration(*elapsed);
                let detail = ctx
                    .locale
                    .named_text("scrollback.subagent.cancelled", "cancelled in {duration}: ")
                    .replace("{duration}", &time_str);
                let prefix_len = subagent_label.width() + detail.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                Line::from(vec![
                    Span::styled(subagent_label, bold),
                    Span::styled(detail, muted),
                    Span::styled(desc, muted),
                ])
            }
        };

        BlockOutput {
            lines: vec![line.into()],
        }
    }

    fn accent(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        match &self.kind {
            SubagentBlockKind::Started if ctx.is_running => {
                Some(AccentStyle::static_color(theme.accent_running))
            }
            _ => None,
        }
    }

    fn bullet(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        match &self.kind {
            SubagentBlockKind::Started => {
                if ctx.is_running {
                    let dim = ctx.appearance.scrollback.display.dim_accent;
                    let dimmed = blend_color(theme.bg_base, theme.accent_running, dim)
                        .unwrap_or(theme.accent_running);
                    Some(AccentStyle::animated(dimmed))
                } else {
                    // Finished: gray bullet (same as bg task "started" after completion)
                    None
                }
            }
            SubagentBlockKind::Completed { .. } => {
                Some(AccentStyle::static_color(theme.accent_success))
            }
            SubagentBlockKind::Failed { .. } | SubagentBlockKind::Cancelled { .. } => {
                Some(AccentStyle::static_color(theme.accent_error))
            }
        }
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false
    }

    fn has_raw_mode(&self) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        false
    }

    fn default_display_mode(&self) -> DisplayMode {
        DisplayMode::Collapsed
    }

    fn is_selectable(&self) -> bool {
        true
    }

    fn has_bullet(&self, _ctx: &BlockContext) -> bool {
        true
    }

    fn is_groupable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::localized_subagent_error;
    use crate::locale::{LocaleContext, LocaleSource, ResolvedLocale, UiLocale};

    fn zh_locale() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    #[test]
    fn zh_subagent_errors_translate_only_known_display_copy() {
        let zh = zh_locale();
        assert_eq!(
            localized_subagent_error(&zh, "Subagent turn was removed before it ran"),
            "子代理回合在运行前已从队列移除"
        );
        assert_eq!(
            localized_subagent_error(
                &zh,
                "Subagent turn was cancelled: hook denied — blocked for tool `bash` (hook: audit)"
            ),
            "子代理回合已取消：钩子拒绝 — blocked for tool `bash` (hook: audit)"
        );
        assert_eq!(
            localized_subagent_error(&zh, "Subagent turn was removed before it ran extra"),
            "Subagent turn was removed before it ran extra"
        );
    }

    #[test]
    fn zh_subagent_resume_errors_preserve_ids_and_limits() {
        let zh = zh_locale();
        let error = "Cannot resume from subagent 'child-7': source transcript (~12345 tokens) exceeds the resume limit (12000 of 16000 tokens). Compact the source first, or resume on a model with a larger context window.";
        assert_eq!(
            localized_subagent_error(&zh, error),
            "无法从子代理“child-7”续跑：源会话记录约 12345 Token，超过续跑限制（12000/16000 Token）。请先压缩源会话，或改用上下文窗口更大的模型。"
        );

        let placeholder_like_values = "Cannot resume from subagent 'child-{error}': failed to load copied transcript: disk {subagent} full";
        assert_eq!(
            localized_subagent_error(&zh, placeholder_like_values),
            "无法从子代理“child-{error}”续跑：加载复制的会话记录失败：disk {subagent} full"
        );

        assert_eq!(
            localized_subagent_error(
                &zh,
                "Cannot resume from subagent 'child-8': it is still running. Wait for it to complete before resuming."
            ),
            "无法从子代理“child-8”续跑：该子代理仍在运行。请等待其完成后再续跑。"
        );
        assert_eq!(
            localized_subagent_error(
                &zh,
                "Cannot resume from subagent 'child-9': source model 'preview-x' is no longer available in the model catalogue."
            ),
            "无法从子代理“child-9”续跑：源模型“preview-x”已不在模型目录中。"
        );
    }

    #[test]
    fn zh_subagent_resume_identity_errors_preserve_values() {
        let zh = zh_locale();
        assert_eq!(
            localized_subagent_error(
                &zh,
                "Cannot resume with subagent_type 'explore': source subagent was 'general-purpose'. Resumed sessions must use the same subagent type as the source."
            ),
            "无法使用子代理类型“explore”续跑：源子代理类型为“general-purpose”。续跑会话必须使用与源相同的子代理类型。"
        );
        assert_eq!(
            localized_subagent_error(
                &zh,
                "Cannot resume with persona 'reviewer': source subagent used Some(\"implementer\"). Resumed sessions must use the same persona as the source."
            ),
            "无法使用角色“reviewer”续跑：源子代理使用的角色为 Some(\"implementer\")。续跑会话必须使用与源相同的角色。"
        );
    }

    #[test]
    fn zh_subagent_truncation_suffix_localizes_without_rewriting_detail() {
        let zh = zh_locale();
        let error = "structured output validation failed: schema.path missing; the subagent's turn was truncated by the output token limit — the structured answer is likely incomplete";
        assert_eq!(
            localized_subagent_error(&zh, error),
            "结构化输出验证失败：schema.path missing；子代理回合因输出 Token 限制被截断，结构化答案可能不完整"
        );
        assert_eq!(
            localized_subagent_error(&LocaleContext::default(), error),
            error
        );
    }
}
