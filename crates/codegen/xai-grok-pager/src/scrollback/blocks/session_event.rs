//! Unlike [`super::SystemMessageBlock`] (which renders arbitrary text), `SessionEventBlock` uses a [`SessionEvent`] enum.
//! Each event variant carries structured data (e.g., elapsed time, error messages, token counts).
//! This enables variant-specific rendering and future styling differentiation.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use xai_grok_shell::extensions::notification::HookAnnotationKind;

use crate::app::actions::PermissionLabel;
use crate::appearance::AppearanceConfig;
use crate::render::wrapping::word_wrap_lines;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{
    AccentStyle, BlockContext, BlockLine, BlockOutput, DisplayMode, Selectable,
};
use crate::theme::Theme;
use crate::util::format_duration;
use crate::views::plan_approval_view::PlanReviewOutcome;
use xai_grok_shell::extensions::notification::{
    MODEL_FAMILY_SWITCH_COMPACT_BANNER, MemoryCaptureDebugEntry,
};

/// Shared text-selection range id for recap body lines (header is excluded).
const RECAP_BODY_RANGE: u16 = 0;

/// Which pager-local memory command a [`SessionEvent::MemoryCommandStarted`] marker belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCommandKind {
    Flush,
    Dream,
}

impl MemoryCommandKind {
    fn started_text(self) -> &'static str {
        match self {
            Self::Flush => "Flushing memory…",
            Self::Dream => "Consolidating memory…",
        }
    }
}

/// A session-level event with structured data.
/// Each variant carries the information needed to render a concise, informational message in the scrollback.
/// These are non-interactive: unselectable, unfoldable, no accent.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Agent turn completed normally.
    TurnCompleted {
        /// Wall-clock elapsed time for the turn.
        /// `None` when unknown: a wake turn whose deltas carried no `turnStartMs` (old shells) renders without a duration instead of a fake "0.0s".
        elapsed: Option<Duration>,
    },
    /// Agent turn was cancelled.
    TurnCancelled {
        /// Wall-clock elapsed time before cancellation.
        elapsed: Duration,
        /// Named from `_meta.cancelTrigger` / `_meta.cancellationCategory`.
        cause: crate::scrollback::blocks::CancelledBy,
    },
    /// Agent turn ended because a hook denied it, today only a `UserPromptSubmit` block (a `PreToolUse` deny feeds back and the turn continues).
    /// Distinct from [`SessionEvent::TurnCancelled`] so the marker never claims the USER cancelled a policy block.
    /// The warning annotation above the marker attributes the hook and reason.
    TurnBlockedByHook {
        /// Wall-clock elapsed time before the block.
        elapsed: Duration,
    },
    /// Agent turn was halted by the system (e.g. doom loop detection).
    TurnHalted {
        /// Wall-clock elapsed time before the turn was halted.
        elapsed: Duration,
    },
    /// Agent turn failed with an error.
    TurnFailed {
        /// Error description.
        error: String,
        /// Elapsed time, if available.
        elapsed: Option<Duration>,
    },
    /// Auto-compaction started (context window threshold reached).
    CompactionStarted {
        /// Percentage of context window used (e.g., 85).
        percentage: u8,
        reason: String,
    },
    /// Auto-compaction completed successfully.
    CompactionCompleted {
        /// Tokens used before compaction (`None` from older shells).
        tokens_before: Option<u64>,
        /// Tokens used after compaction.
        tokens_after: u64,
        /// How long compaction took (milliseconds).
        elapsed_ms: Option<i64>,
    },
    /// Auto-compaction failed.
    CompactionFailed {
        /// Error description.
        error: String,
    },
    /// Auto-compaction was cancelled (turn was cancelled mid-compact).
    CompactionCancelled,
    /// Retry failed: all retries exhausted or a non-retryable error.
    ///
    /// Covers both `RetryState::Exhausted` (tried N times, all failed) and `RetryState::Failed` (non-retryable error like auth or 413).
    RetryFailed {
        /// Human-readable error description.
        error: String,
        /// Structured error category from `RetryState::Failed::error_type`.
        /// Used to match known error patterns without fragile string matching.
        error_type: Option<String>,
    },
    /// A non-success API / HTTP response (or similar terminal request error).
    /// Rendered like [`SessionEvent::ReAuthRequired`]: warning color and accent, no JSON dump.
    RequestFailed {
        /// HTTP status when known. `None` for transport / idle-timeout / etc.
        status: Option<u16>,
        /// Short headline, e.g. `"Server error (500)"`.
        headline: String,
        /// Sanitized one-line detail (server message or fallback guidance).
        detail: String,
    },
    /// The server rejected the credentials (401 / auth error) and automatic recovery was exhausted.
    /// Rendered as a prominent call-to-action that points the user at `/login` to re-authenticate.
    /// It replaces the raw "Retry failed: Unauthorized (401) …" dump.
    ReAuthRequired,
    /// Terminal context overflow, ideally unreachable since auto-compaction should shrink the conversation first.
    /// A safeguard for when it didn't (estimate drift vs the server's max_prompt_length, or compaction suppressed/failed).
    /// One actionable prompt, replacing the stacked CompactionFailed, RetryFailed, and TurnFailed banners.
    ContextTooLarge,
    /// Session disk is full.
    DiskFull,
    /// Manual `/compact` command started. The invocation marker that pairs each `/compact` with its own outcome line.
    /// Without it, back-to-back failures render as adjacent identical lines that read as one duplicated flow.
    /// Local scrollback block only: like the manual outcome lines it is not persisted, so a resumed session replays neither.
    CompactStarted,
    /// Manual `/compact` command completed.
    CompactCompleted {
        /// Wall-clock elapsed time for the command.
        elapsed: Duration,
    },
    /// `/flush` or `/dream` started; the invocation marker that pairs each run with its outcome line.
    /// Local scrollback block only, like [`SessionEvent::CompactStarted`].
    MemoryCommandStarted { command: MemoryCommandKind },
    /// `/flush` or `/dream` finished. `summary` comes from the shell's typed response.
    MemoryCommandCompleted {
        summary: String,
        /// False when the run did not achieve what the user asked (failed, timed out, disabled).
        succeeded: bool,
        elapsed: Duration,
    },
    /// Hook annotation, displayed inline after a tool call.
    /// The message comes from the agent via `XaiSessionUpdate::HookAnnotation`.
    HookAnnotation {
        message: String,
        /// Shell-owned template provenance. `None` means an older replay or
        /// arbitrary hook text and is always rendered verbatim.
        kind: Option<HookAnnotationKind>,
    },
    /// A hook's verdict on the tool call above it (deny, failure, timeout); this block draws the tool-row bullet.
    HookOutcome { message: String },
    /// The session's persisted model is no longer available after re-auth.
    /// Both IDs are empty when re-shown on blocked prompt attempts.
    ModelUnavailable {
        previous_model_id: String,
        new_model_id: String,
        reason: String,
    },
    /// Memory was saved (flush, dream, or session-end).
    MemorySaved {
        /// File path that was written.
        path: String,
        /// What triggered the save: "session-end", "flush", or "dream".
        trigger: String,
    },
    /// A `/goal` finished (status reached Complete).
    /// Carries the goal's total elapsed time across all its turns, distinct from the per-turn "Worked for" marker.
    GoalCompleted {
        /// Goal end-to-end elapsed time (`GoalUpdated.elapsed_ms`).
        elapsed: Duration,
    },
    /// A session recap: a short "where was I" summary of the session so far.
    /// Shown on demand via `/recap` (`auto = false`) or automatically when the user returns to the terminal after being away (`auto = true`).
    Recap {
        /// The one-line recap text.
        summary: String,
        /// `true` for the automatic return-from-away recap, `false` for `/recap`.
        auto: bool,
    },
    /// Not persisted: a resumed session shows only the `Plan: Enter` tool row.
    PlanModeEnteredByAgent { permission: PermissionLabel },
    PlanReviewClosed {
        outcome: PlanReviewOutcome,
        permission: PermissionLabel,
    },
}

/// Format trusted memory-capture status chrome without promoting arbitrary
/// activity strings, model diagnostics, or local paths into a user-facing notice.
pub(crate) fn memory_capture_status_text(
    activity: &str,
    from_turn: u32,
    through_turn: u32,
    attempt: u32,
    locale: &crate::locale::LocaleContext,
) -> String {
    let (id, english) = match activity {
        "queued" => ("memory.capture.status.queued", "queued"),
        "running" => ("memory.capture.status.running", "running"),
        "completed" => ("memory.capture.status.completed", "completed"),
        "no_op" => ("memory.capture.status.no_op", "completed with no changes"),
        "retry" => ("memory.capture.status.retry", "scheduled for retry"),
        "failed" => ("memory.capture.status.failed", "failed"),
        _ => ("memory.capture.status.updated", "updated"),
    };
    let activity = locale.named_text(id, english);
    let attempt_suffix = if attempt > 1 {
        locale
            .named_text("memory.capture.attempt", " (attempt {attempt})")
            .replace("{attempt}", &attempt.to_string())
    } else {
        String::new()
    };
    locale
        .named_text(
            "memory.capture.notice",
            "Memory capture {activity} for turns {from}-{through}{attempt_suffix}",
        )
        .replace("{activity}", &activity)
        .replace("{from}", &from_turn.to_string())
        .replace("{through}", &through_turn.to_string())
        .replace("{attempt_suffix}", &attempt_suffix)
}

/// Debug-only, foldable view of observations created by a memory-v2 capture.
#[derive(Debug, Clone)]
pub struct MemoryCaptureBlock {
    from_turn: u32,
    through_turn: u32,
    entries: Vec<MemoryCaptureDebugEntry>,
}

impl MemoryCaptureBlock {
    pub fn new(from_turn: u32, through_turn: u32, entries: Vec<MemoryCaptureDebugEntry>) -> Self {
        Self {
            from_turn,
            through_turn,
            entries: entries
                .into_iter()
                .map(|entry| MemoryCaptureDebugEntry {
                    statement: sanitize_model_debug_text(&entry.statement),
                    body: entry.body.map(|body| sanitize_model_debug_text(&body)),
                    // The path is produced only after create-only persistence
                    // succeeds and remains the block's sole trusted link target.
                    path: entry.path,
                })
                .collect(),
        }
    }

    fn title(&self) -> String {
        let noun = if self.entries.len() == 1 {
            "memory"
        } else {
            "memories"
        };
        format!(
            "Model-generated memory debug output: {} {noun} for turns {}-{}",
            self.entries.len(),
            self.from_turn,
            self.through_turn
        )
    }

    fn title_with_locale(&self, locale: &crate::locale::LocaleContext) -> String {
        let (id, english) = if self.entries.len() == 1 {
            (
                "memory.capture.title.one",
                "Model-generated memory debug output: {count} memory for turns {from}-{through}",
            )
        } else {
            (
                "memory.capture.title.many",
                "Model-generated memory debug output: {count} memories for turns {from}-{through}",
            )
        };
        locale
            .named_text(id, english)
            .replace("{count}", &self.entries.len().to_string())
            .replace("{from}", &self.from_turn.to_string())
            .replace("{through}", &self.through_turn.to_string())
    }

    pub(crate) fn searchable_text(&self) -> String {
        let mut parts = vec![self.title()];
        for entry in &self.entries {
            parts.push(entry.statement.clone());
            if let Some(body) = &entry.body {
                parts.push(body.clone());
            }
            parts.push(entry.path.clone());
        }
        parts.join("\n")
    }
}

impl BlockContent for MemoryCaptureBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        let theme = Theme::current();
        let title_style = if ctx.mode == DisplayMode::Collapsed {
            theme.muted().add_modifier(Modifier::BOLD)
        } else {
            theme.primary().add_modifier(Modifier::BOLD)
        };
        let mut lines = vec![BlockLine::styled(Line::from(Span::styled(
            self.title_with_locale(&ctx.locale),
            title_style,
        )))];

        if ctx.mode != DisplayMode::Collapsed {
            let width = ctx.content_width().max(1);
            for (index, entry) in self.entries.iter().enumerate() {
                lines.push(BlockLine::separator(Line::default()));
                lines.push(BlockLine::styled(Line::from(Span::styled(
                    ctx.locale
                        .named_text(
                            "memory.capture.observation",
                            "Untrusted model-generated observation {index}",
                        )
                        .replace("{index}", &(index + 1).to_string()),
                    theme.muted().add_modifier(Modifier::BOLD),
                ))));
                lines.extend(
                    word_wrap_lines(
                        entry
                            .statement
                            .lines()
                            .map(|line| Line::from(Span::styled(line.to_owned(), theme.primary())))
                            .collect::<Vec<_>>(),
                        width,
                    )
                    .into_iter()
                    .map(BlockLine::styled),
                );
                if let Some(body) = entry.body.as_deref() {
                    lines.extend(
                        word_wrap_lines(
                            body.lines()
                                .map(|line| {
                                    Line::from(Span::styled(line.to_owned(), theme.muted()))
                                })
                                .collect::<Vec<_>>(),
                            width,
                        )
                        .into_iter()
                        .map(BlockLine::styled),
                    );
                }

                let label = Path::new(&entry.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(entry.path.as_str());
                let mut path_line = BlockLine::styled(Line::from(vec![
                    Span::styled(
                        ctx.locale
                            .named_static_text("memory.capture.open_file", "Open file \u{2192} "),
                        theme.muted(),
                    ),
                    Span::styled(
                        label.to_owned(),
                        theme.primary().add_modifier(Modifier::UNDERLINED),
                    ),
                ]));
                path_line.link_target = Some(crate::render::osc8::LinkTarget::File(Arc::from(
                    Path::new(&entry.path),
                )));
                lines.push(path_line);
            }
        }

        if let Some(max_lines) = ctx.max_lines {
            lines.truncate(max_lines as usize);
        }
        BlockOutput { lines }
    }

    fn accent(&self, _ctx: &BlockContext) -> Option<AccentStyle> {
        None
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false
    }

    fn default_display_mode(&self) -> DisplayMode {
        DisplayMode::Collapsed
    }

    fn has_bullet(&self, _ctx: &BlockContext) -> bool {
        true
    }

    fn is_groupable(&self) -> bool {
        true
    }
}

fn sanitize_model_debug_text(text: &str) -> String {
    let stripped = strip_ansi_escapes::strip_str(text);
    let mut sanitized = String::with_capacity(stripped.len());
    let mut characters = stripped.chars().peekable();
    let mut previous = None;
    while let Some(character) = characters.next() {
        if character.is_control() && !matches!(character, '\n' | '\t') {
            sanitized.push('\u{fffd}');
            continue;
        }
        if matches!(
            character,
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        ) {
            sanitized.push('\u{fffd}');
            continue;
        }
        sanitized.push(character);
        // Keep debug prose from becoming terminal-native links. Word joiners break
        // URL/email recognition without changing visible text; the committed local
        // path below is linked explicitly through `LinkTarget::File`.
        let domain_dot = character == '.'
            && previous.is_some_and(char::is_alphanumeric)
            && characters.peek().is_some_and(|next| next.is_alphanumeric());
        if matches!(character, ':' | '/' | '@') || domain_dot {
            sanitized.push('\u{2060}');
        }
        previous = Some(character);
    }
    sanitized
}

impl SessionEvent {
    /// Format the event as a human-readable string.
    pub fn message(&self) -> String {
        match self {
            // Deliberately period-less: don't re-punctuate
            SessionEvent::TurnCompleted {
                elapsed: Some(elapsed),
            } => {
                format!("Worked for {}", format_duration(*elapsed))
            }
            SessionEvent::TurnCompleted { elapsed: None } => "Turn completed.".to_string(),
            SessionEvent::TurnCancelled { elapsed, cause } => {
                format!("{} in {}.", cause.phrase(), format_duration(*elapsed))
            }
            SessionEvent::TurnBlockedByHook { elapsed } => {
                format!("Turn blocked by a hook in {}.", format_duration(*elapsed))
            }
            SessionEvent::TurnHalted { elapsed } => {
                format!(
                    "Agent was unable to make progress. Turn ended in {}.",
                    format_duration(*elapsed)
                )
            }
            SessionEvent::TurnFailed {
                error,
                elapsed: Some(elapsed),
            } => {
                format!("Turn failed in {}: {error}", format_duration(*elapsed))
            }
            SessionEvent::TurnFailed {
                error,
                elapsed: None,
            } => {
                format!("Turn failed: {error}")
            }
            SessionEvent::CompactionStarted { percentage, reason } => {
                if reason == MODEL_FAMILY_SWITCH_COMPACT_BANNER {
                    MODEL_FAMILY_SWITCH_COMPACT_BANNER.to_string()
                } else {
                    format!("Context {percentage}% full. Compacting…")
                }
            }
            SessionEvent::CompactionCompleted {
                tokens_before,
                tokens_after,
                elapsed_ms,
            } => {
                let after = format_tokens(*tokens_after);
                // Older shells don't send tokens_before; keep the legacy format
                let body = match tokens_before {
                    Some(before) if *before > 0 => {
                        format!(
                            "Context compacted: {} → {after} tokens",
                            format_tokens(*before)
                        )
                    }
                    _ => format!("Context compacted → {after} tokens"),
                };
                if let Some(ms) = elapsed_ms {
                    let secs = *ms as f64 / 1000.0;
                    format!("{body} ({secs:.1}s)")
                } else {
                    body
                }
            }
            SessionEvent::CompactionFailed { error } => {
                if error.trim().is_empty() {
                    "Compaction failed.".to_string()
                } else {
                    // Multi-line errors (guidance and detail) split in `output`; old one-line replays render unchanged
                    format!("Compaction failed - {error}")
                }
            }
            SessionEvent::CompactionCancelled => "Compaction cancelled.".to_string(),
            SessionEvent::RetryFailed { error, error_type } => {
                use crate::app::error_display::WireErrorType;
                if WireErrorType::parse(error_type.as_deref())
                    == WireErrorType::EncryptedContentMismatch
                {
                    "This session's conversation history is incompatible with the \
                     current model. Please start a new session."
                        .to_string()
                } else {
                    format!("Retry failed: {error}")
                }
            }
            SessionEvent::RequestFailed {
                headline, detail, ..
            } => crate::app::error_display::banner_message(headline, detail),
            SessionEvent::ReAuthRequired => {
                "Authentication required: your session has expired or your \
                 credentials were rejected. Run /login to re-authenticate, then resend \
                 your message."
                    .to_string()
            }
            SessionEvent::ContextTooLarge => {
                "This conversation is too large for the model's context window. \
                 Use /new to start a new session."
                    .to_string()
            }
            SessionEvent::DiskFull => {
                xai_grok_shell::extensions::notification::DISK_FULL_USER_MESSAGE.to_string()
            }
            // No "Context N% full." prefix; that phrasing is the auto marker's
            SessionEvent::CompactStarted => "Compacting conversation…".to_string(),
            SessionEvent::CompactCompleted { elapsed } => {
                format!("Compaction completed in {}.", format_duration(*elapsed))
            }
            SessionEvent::MemoryCommandStarted { command } => command.started_text().to_string(),
            SessionEvent::MemoryCommandCompleted {
                summary,
                succeeded,
                elapsed,
            } => {
                if *succeeded {
                    format!(
                        "{summary} ({})  \u{00b7}  /memory to view",
                        format_duration(*elapsed)
                    )
                } else {
                    summary.clone()
                }
            }
            SessionEvent::HookAnnotation { message, .. }
            | SessionEvent::HookOutcome { message } => message.clone(),
            SessionEvent::ModelUnavailable {
                new_model_id,
                reason,
                ..
            } => {
                if new_model_id.is_empty() {
                    reason.clone()
                } else {
                    format!("{reason} Switched to \"{new_model_id}\".")
                }
            }
            SessionEvent::MemorySaved { path, trigger } => {
                let short_path = crate::util::abbreviate_path(path);
                format!("Memory saved ({trigger}) \u{2192} {short_path}  \u{00b7}  /memory to view")
            }
            SessionEvent::GoalCompleted { elapsed } => {
                format!("Goal complete in {} end-to-end.", format_duration(*elapsed))
            }
            SessionEvent::Recap { summary, auto: _ } => {
                // Always "Recap:" (manual `/recap` and auto return-from-away).
                format!("Recap: {summary}")
            }
            SessionEvent::PlanModeEnteredByAgent { permission } => {
                format!(
                    "Agent entered plan mode · active permission mode: {permission} · file edits outside session plan.md blocked until plan mode exits"
                )
            }
            SessionEvent::PlanReviewClosed {
                outcome,
                permission,
            } => {
                let verdict = match outcome {
                    PlanReviewOutcome::Approved => "approved",
                    PlanReviewOutcome::Abandoned => "abandoned",
                };
                format!("Plan {verdict} · plan mode off · active permission mode: {permission}")
            }
        }
    }

    /// Format display chrome for the selected UI locale while preserving the
    /// structured event payload, error text, model ids, paths, and command
    /// names exactly as received.
    pub fn message_with_locale(&self, locale: &crate::locale::LocaleContext) -> String {
        let text = |id: &str, english: &str| locale.named_text(id, english).into_owned();
        match self {
            SessionEvent::TurnCompleted {
                elapsed: Some(elapsed),
            } => text(
                "scrollback.session_event.turn_completed_duration",
                "Worked for {duration}",
            )
            .replace("{duration}", &format_duration(*elapsed)),
            SessionEvent::TurnCompleted { elapsed: None } => {
                text("scrollback.session_event.turn_completed", "Turn completed.")
            }
            SessionEvent::TurnCancelled { elapsed, cause } => {
                let id = match cause {
                    super::CancelledBy::User => "scrollback.cancel.user",
                    super::CancelledBy::SessionClosed => "scrollback.cancel.session_closed",
                    super::CancelledBy::Shutdown => "scrollback.cancel.shutdown",
                    super::CancelledBy::MaxTurns => "scrollback.cancel.max_turns",
                    super::CancelledBy::PermissionDenied => "scrollback.cancel.permission_denied",
                    super::CancelledBy::PermissionDismissed => "scrollback.cancel.permission_dismissed",
                    super::CancelledBy::HostInterrupt => "scrollback.cancel.host_interrupt",
                    super::CancelledBy::Unspecified => "scrollback.cancel.unspecified",
                };
                text("scrollback.cancel.duration", "{cause} in {duration}.")
                    .replace("{cause}", &text(id, cause.phrase()))
                    .replace("{duration}", &format_duration(*elapsed))
            },
            SessionEvent::TurnBlockedByHook { elapsed } => text(
                "scrollback.session_event.turn_blocked_by_hook",
                "Turn blocked by a hook in {duration}.",
            )
            .replace("{duration}", &format_duration(*elapsed)),
            SessionEvent::TurnHalted { elapsed } => text(
                "scrollback.session_event.turn_halted",
                "Agent was unable to make progress — turn ended in {duration}.",
            )
            .replace("{duration}", &format_duration(*elapsed)),
            SessionEvent::TurnFailed {
                error,
                elapsed: Some(elapsed),
            } => text(
                "scrollback.session_event.turn_failed_duration",
                "Turn failed in {duration}: {error}",
            )
            .replace("{duration}", &format_duration(*elapsed))
            .replace("{error}", error),
            SessionEvent::TurnFailed {
                error,
                elapsed: None,
            } => text(
                "scrollback.session_event.turn_failed",
                "Turn failed: {error}",
            )
            .replace("{error}", error),
            SessionEvent::CompactionStarted { percentage, reason } => {
                if reason == MODEL_FAMILY_SWITCH_COMPACT_BANNER {
                    text("turn.switching_model", MODEL_FAMILY_SWITCH_COMPACT_BANNER)
                } else {
                    text("scrollback.session_event.compaction_started", "Context {percentage}% full. Compacting…")
                        .replace("{percentage}", &percentage.to_string())
                }
            },
            SessionEvent::CompactionCompleted {
                tokens_before,
                tokens_after,
                elapsed_ms,
            } => {
                let after = format_tokens(*tokens_after);
                let body = match tokens_before {
                    Some(before) if *before > 0 => text(
                        "scrollback.session_event.compaction_completed_before_after",
                        "Context compacted: {before} → {after} tokens",
                    )
                    .replace("{before}", &format_tokens(*before))
                    .replace("{after}", &after),
                    _ => text(
                        "scrollback.session_event.compaction_completed_after",
                        "Context compacted → {after} tokens",
                    )
                    .replace("{after}", &after),
                };
                if let Some(ms) = elapsed_ms {
                    text(
                        "scrollback.session_event.duration_suffix",
                        "{body} ({duration})",
                    )
                    .replace("{body}", &body)
                    .replace("{duration}", &format!("{:.1}s", *ms as f64 / 1000.0))
                } else {
                    body
                }
            }
            SessionEvent::CompactionFailed { error } => {
                if error.trim().is_empty() {
                    text(
                        "scrollback.session_event.compaction_failed_empty",
                        "Compaction failed.",
                    )
                } else {
                    text(
                        "scrollback.session_event.compaction_failed",
                        "Compaction failed - {error}",
                    )
                    .replace("{error}", error)
                }
            }
            SessionEvent::CompactionCancelled => text(
                "scrollback.session_event.compaction_cancelled",
                "Compaction cancelled.",
            ),
            SessionEvent::RetryFailed { error, error_type } => {
                if error_type.as_deref() == Some("encrypted_content_mismatch") {
                    text(
                        "scrollback.session_event.history_incompatible",
                        "This session's conversation history is incompatible with the current model. Please start a new session.",
                    )
                } else {
                    text(
                        "scrollback.session_event.retry_failed",
                        "Retry failed: {error}",
                    )
                    .replace("{error}", error)
                }
            }
            SessionEvent::RequestFailed {
                headline, detail, ..
            } => crate::app::error_display::banner_message(headline, detail),
            SessionEvent::ReAuthRequired => text(
                "scrollback.session_event.reauth_required",
                "Authentication required — your session has expired or your credentials were rejected. Run /login to re-authenticate, then resend your message.",
            ),
            SessionEvent::ContextTooLarge => text(
                "scrollback.session_event.context_too_large",
                "This conversation is too large for the model's context window. Use /new to start a new session.",
            ),
            SessionEvent::DiskFull => text(
                "scrollback.session_event.disk_full",
                xai_grok_shell::extensions::notification::DISK_FULL_USER_MESSAGE,
            ),
            SessionEvent::CompactStarted => text(
                "scrollback.session_event.compact_started",
                "Compacting conversation…",
            ),
            SessionEvent::CompactCompleted { elapsed } => text(
                "scrollback.session_event.compact_completed",
                "Compaction completed in {duration}.",
            )
            .replace("{duration}", &format_duration(*elapsed)),
            SessionEvent::MemoryCommandStarted { command } => match command {
                MemoryCommandKind::Flush => text("turn.flushing_memory", command.started_text()),
                MemoryCommandKind::Dream => text("turn.consolidating_memory", command.started_text()),
            },
            SessionEvent::MemoryCommandCompleted { summary, succeeded, elapsed } => {
                if *succeeded {
                    text("scrollback.memory_command.completed", "{summary} ({duration})  ·  /memory to view")
                        .replace("{summary}", summary)
                        .replace("{duration}", &format_duration(*elapsed))
                } else {
                    summary.clone()
                }
            },
            SessionEvent::HookAnnotation { message, kind } => {
                localized_hook_annotation(locale, *kind, message)
            }
            // Hook-owned verdict text is opaque; only the block chrome is ours.
            SessionEvent::HookOutcome { message } => message.clone(),
            SessionEvent::ModelUnavailable {
                new_model_id,
                reason,
                ..
            } => {
                let reason = localized_model_unavailable_reason(locale, reason);
                if new_model_id.is_empty() {
                    reason
                } else {
                    replace_placeholders_once(
                        &text(
                            "scrollback.session_event.model_switched",
                            "{reason} Switched to \"{model}\".",
                        ),
                        &[("{reason}", &reason), ("{model}", new_model_id)],
                    )
                }
            }
            SessionEvent::MemorySaved { path, trigger } => text(
                "scrollback.session_event.memory_saved",
                "Memory saved ({trigger}) → {path}  ·  /memory to view",
            )
            .replace("{trigger}", trigger)
            .replace("{path}", &crate::util::abbreviate_path(path)),
            SessionEvent::GoalCompleted { elapsed } => text(
                "scrollback.session_event.goal_completed",
                "Goal complete — {duration} end-to-end.",
            )
            .replace("{duration}", &format_duration(*elapsed)),
            SessionEvent::Recap { summary, auto: _ } => {
                text("scrollback.session_event.recap", "Recap — {summary}")
                    .replace("{summary}", summary)
            }
            SessionEvent::PlanModeEnteredByAgent { permission } => text(
                "scrollback.session_event.plan_mode_entered",
                "Agent entered plan mode · active permission mode: {permission} · file edits outside session plan.md blocked until plan mode exits",
            )
            .replace("{permission}", permission.as_canonical()),
            SessionEvent::PlanReviewClosed {
                outcome,
                permission,
            } => {
                let (id, english) = match outcome {
                    PlanReviewOutcome::Approved => (
                        "scrollback.session_event.plan_review_approved",
                        "Plan approved · plan mode off · active permission mode: {permission}",
                    ),
                    PlanReviewOutcome::Abandoned => (
                        "scrollback.session_event.plan_review_abandoned",
                        "Plan abandoned · plan mode off · active permission mode: {permission}",
                    ),
                };
                text(id, english).replace("{permission}", permission.as_canonical())
            }
        }
    }

    /// The recap summary text when this is a [`SessionEvent::Recap`].
    /// Recap events render in the tool-call visual style (bullet, bold "Recap" header, muted body); other variants stay plain informational lines.
    /// This accessor is the single branch point the `SessionEventBlock` trait methods use to opt the recap into that style.
    fn recap_summary(&self) -> Option<&str> {
        match self {
            SessionEvent::Recap { summary, .. } => Some(summary.as_str()),
            _ => None,
        }
    }

    /// Failures and actionable prompts stand out (warning color and accent bar).
    fn is_warning_banner(&self) -> bool {
        matches!(
            self,
            SessionEvent::ReAuthRequired
                | SessionEvent::ContextTooLarge
                | SessionEvent::DiskFull
                | SessionEvent::CompactionFailed { .. }
                | SessionEvent::RequestFailed { .. }
                | SessionEvent::RetryFailed { .. }
                | SessionEvent::TurnFailed { .. }
                | SessionEvent::MemoryCommandCompleted {
                    succeeded: false,
                    ..
                }
        )
    }

    /// Whether this event ends an agent turn. [`SessionEvent::RequestFailed`] and [`SessionEvent::ReAuthRequired`] are
    /// excluded: RetryState may push them before PromptResponse, and dedicated banners skip the TurnFailed marker.
    pub fn is_turn_terminal(&self) -> bool {
        matches!(
            self,
            SessionEvent::TurnCompleted { .. }
                | SessionEvent::TurnCancelled { .. }
                | SessionEvent::TurnBlockedByHook { .. }
                | SessionEvent::TurnHalted { .. }
                | SessionEvent::TurnFailed { .. }
        )
    }
}

pub(crate) fn localized_model_unavailable_reason(
    locale: &crate::locale::LocaleContext,
    reason: &str,
) -> String {
    let text = |id: &str, english: &str| locale.named_text(id, english).into_owned();
    const ORG_DENIAL: &str =
        "This model isn't allowed by your organization's policy. Contact your administrator.";
    const USER_DENIAL: &str = "This model isn't allowed by your allowed_models setting.";
    const PREVIOUS_UNAVAILABLE: &str =
        "Your previous model is no longer available. Please start a new session.";
    const PREVIOUS_NO_COMPATIBLE: &str = "Your previous model is no longer available and could not be switched to a compatible model. Please start a new session.";

    if let Some((id, english)) = match reason {
        ORG_DENIAL => Some((
            "scrollback.session_event.model_unavailable.denied_org",
            ORG_DENIAL,
        )),
        USER_DENIAL => Some((
            "scrollback.session_event.model_unavailable.denied_user",
            USER_DENIAL,
        )),
        PREVIOUS_UNAVAILABLE => Some((
            "scrollback.session_event.model_unavailable.previous_new_session",
            PREVIOUS_UNAVAILABLE,
        )),
        PREVIOUS_NO_COMPATIBLE => Some((
            "scrollback.session_event.model_unavailable.previous_no_compatible",
            PREVIOUS_NO_COMPATIBLE,
        )),
        "The organization model policy is invalid. Contact your administrator." => Some((
            "scrollback.session_event.model_unavailable.policy_invalid",
            "The organization model policy is invalid. Contact your administrator.",
        )),
        "None of your models are allowed by your organization's policy. Contact your administrator." => {
            Some((
                "scrollback.session_event.model_unavailable.none_allowed_org",
                "None of your models are allowed by your organization's policy. Contact your administrator.",
            ))
        }
        "None of your models are allowed by allowed_models. Broaden it or remove it from your config, then restart." => {
            Some((
                "scrollback.session_event.model_unavailable.none_allowed_user",
                "None of your models are allowed by allowed_models. Broaden it or remove it from your config, then restart.",
            ))
        }
        _ => None,
    } {
        return text(id, english);
    }

    if let Some((requested, rest)) = reason
        .strip_prefix('"')
        .and_then(|rest| rest.split_once("\": "))
        && !requested.is_empty()
    {
        for (denial, id, english) in [
            (
                ORG_DENIAL,
                "scrollback.session_event.model_unavailable.allowlist_org",
                "\"{requested}\": This model isn't allowed by your organization's policy. Contact your administrator. This session is using \"{current}\".",
            ),
            (
                USER_DENIAL,
                "scrollback.session_event.model_unavailable.allowlist_user",
                "\"{requested}\": This model isn't allowed by your allowed_models setting. This session is using \"{current}\".",
            ),
        ] {
            if let Some(current) = rest
                .strip_prefix(denial)
                .and_then(|rest| rest.strip_prefix(" This session is using \""))
                .and_then(|rest| rest.strip_suffix("\"."))
                .filter(|current| !current.is_empty())
            {
                return replace_placeholders_once(
                    &text(id, english),
                    &[("{requested}", requested), ("{current}", current)],
                );
            }
        }
    }

    if let Some((requested, current)) = reason
        .strip_prefix('"')
        .and_then(|rest| {
            rest.split_once(
                "\" isn't allowed by your allowed_models setting, so this session is using \"",
            )
        })
        .and_then(|(requested, rest)| rest.strip_suffix("\".").map(|current| (requested, current)))
        .filter(|(requested, current)| !requested.is_empty() && !current.is_empty())
    {
        return replace_placeholders_once(
            &text(
                "scrollback.session_event.model_unavailable.allowlist_user",
                "\"{requested}\": This model isn't allowed by your allowed_models setting. This session is using \"{current}\".",
            ),
            &[("{requested}", requested), ("{current}", current)],
        );
    }

    for (suffix, id, english) in [
        (
            "\" is no longer available for your account.",
            "scrollback.session_event.model_unavailable.account",
            "Model \"{model}\" is no longer available for your account.",
        ),
        (
            "\" is no longer available. Please start a new session.",
            "scrollback.session_event.model_unavailable.new_session",
            "Model \"{model}\" is no longer available. Please start a new session.",
        ),
    ] {
        if let Some(model) = reason
            .strip_prefix("Model \"")
            .and_then(|rest| rest.strip_suffix(suffix))
            .filter(|model| !model.is_empty())
        {
            return text(id, english).replace("{model}", model);
        }
    }

    reason.to_owned()
}

fn localized_hook_annotation(
    locale: &crate::locale::LocaleContext,
    kind: Option<HookAnnotationKind>,
    message: &str,
) -> String {
    let text = |id: &str, english: &str| locale.named_text(id, english).into_owned();

    match kind {
        Some(HookAnnotationKind::PromptBlockRequestedNotEnforced) => message
            .strip_prefix("⚠ Prompt block requested by ")
            .and_then(|rest| rest.split_once(" (not enforced for this origin): "))
            .map(|(hook, reason)| {
                replace_placeholders_once(
                    &text(
                        "scrollback.hook_annotation.block_requested_not_enforced",
                        "⚠ Prompt block requested by {hook} (not enforced for this origin): {reason}",
                    ),
                    &[("{hook}", hook), ("{reason}", reason)],
                )
            }),
        Some(HookAnnotationKind::PromptBlocked) => message
            .strip_prefix("⚠ Prompt blocked by ")
            .and_then(|rest| rest.split_once(": "))
            .map(|(hook, reason)| {
                replace_placeholders_once(
                    &text(
                        "scrollback.hook_annotation.prompt_blocked",
                        "⚠ Prompt blocked by {hook}: {reason}",
                    ),
                    &[("{hook}", hook), ("{reason}", reason)],
                )
            }),
        Some(HookAnnotationKind::QueuedPromptsHeld) => message
            .strip_prefix("⚠ ")
            .and_then(|rest| {
                rest.strip_suffix(
                    " queued prompt(s) on hold after the block. Edit or remove them, or send a prompt to resume.",
                )
            })
            .filter(|count| count.chars().all(|ch| ch.is_ascii_digit()))
            .map(|count| {
                text(
                    "scrollback.hook_annotation.queued_prompts_held",
                    "⚠ {count} queued prompt(s) on hold after the block. Edit or remove them, or send a prompt to resume.",
                )
                .replace("{count}", count)
            }),
        Some(HookAnnotationKind::InterjectionsJoined) => message
            .strip_prefix("⚠ ")
            .and_then(|rest| {
                rest.strip_suffix(
                    " interjection(s) joined the held queue. Send a prompt to resume.",
                )
            })
            .filter(|count| count.chars().all(|ch| ch.is_ascii_digit()))
            .map(|count| {
                text(
                    "scrollback.hook_annotation.interjections_joined",
                    "⚠ {count} interjection(s) joined the held queue. Send a prompt to resume.",
                )
                .replace("{count}", count)
            }),
        Some(HookAnnotationKind::Unknown | HookAnnotationKind::Note | HookAnnotationKind::ToolOutcome) | None => None,
    }
    .unwrap_or_else(|| message.to_owned())
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

/// Format a token count with "k" suffix for thousands.
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

/// Visually identical to [`super::SystemMessageBlock`] (muted text, compact, unselectable).
/// The structured `event` field is available for future styling differentiation (e.g., red text for failures).
#[derive(Debug, Clone)]
pub struct SessionEventBlock {
    pub event: SessionEvent,
    locale: crate::locale::LocaleContext,
}

impl SessionEventBlock {
    pub fn new(event: SessionEvent) -> Self {
        Self {
            event,
            locale: crate::locale::LocaleContext::default(),
        }
    }

    pub(crate) fn set_locale(&mut self, locale: crate::locale::LocaleContext) {
        self.locale = locale;
    }

    /// A recap with real body content, i.e. not the empty loading spinner or a stray empty recap.
    /// Gates the interactive affordances (folding and j/k selection) so navigation never lands on a recap that can't fold.
    fn recap_has_body(&self) -> bool {
        self.event
            .recap_summary()
            .is_some_and(|s| !s.trim().is_empty())
    }

    /// Render a recap event in the tool-call visual style. While the recap is still being generated the entry is
    /// `is_running`, so only the header is shown. [`BlockLine::separator`] / [`Selectable::None`] keep drag-highlight
    /// and copy on the summary body, never the chrome label.
    fn recap_output(&self, ctx: &BlockContext, summary: &str) -> BlockOutput {
        let theme = Theme::current();
        let muted_collapsed =
            ctx.mute_when_collapsed(ctx.appearance.scrollback.blocks.tool.muted_collapsed);

        // "Recap" header: bold, neutral primary text (like a tool-call header)
        // Dimmed to muted gray while collapsed-and-unselected.
        let header_text_style = if muted_collapsed {
            theme.muted()
        } else {
            theme.primary()
        };
        let header_style = header_text_style.add_modifier(Modifier::BOLD);
        let recap_label = self
            .locale
            .named_text("scrollback.session_event.recap_label", "Recap")
            .into_owned();
        // Non-selectable chrome (same as Thinking / tool label prefixes).
        let header_line =
            || BlockLine::separator(Line::from(Span::styled(recap_label.clone(), header_style)));

        // Loading: header only; the animated gray sidebar is the feedback.
        if ctx.is_running {
            return BlockOutput {
                lines: vec![header_line()],
            };
        }

        match ctx.mode {
            DisplayMode::Collapsed => {
                let mut spans = vec![Span::styled(recap_label.clone(), header_style)];
                let preview = summary.lines().next().unwrap_or(summary).trim();
                if !preview.is_empty() {
                    spans.push(Span::styled(format!("  {preview}"), theme.muted()));
                }
                let line = crate::render::line_utils::truncate_line(
                    Line::from(spans),
                    ctx.content_width(),
                );
                // Only the preview span is copyable, never the "Recap" label
                // No preview (empty after trim) means fully non-selectable
                let selectable = if preview.is_empty() {
                    Selectable::None
                } else {
                    Selectable::Spans(1..2)
                };
                BlockOutput {
                    lines: vec![BlockLine {
                        content: line,
                        selectable,
                        selection_range: (!preview.is_empty()).then_some(RECAP_BODY_RANGE),
                        selection_text: (!preview.is_empty()).then(|| preview.to_string()),
                        ..Default::default()
                    }],
                }
            }
            DisplayMode::Truncated | DisplayMode::Expanded => {
                let mut lines: Vec<BlockLine> = vec![header_line()];
                // Blank gap under the header is decoration, not copyable text.
                lines.push(BlockLine::separator(Line::from("")));

                let styled_lines = summary
                    .split('\n')
                    .map(|line| Line::from(Span::styled(line.to_string(), theme.muted())));
                let wrapped =
                    word_wrap_lines(styled_lines, (ctx.width as usize).saturating_sub(2).max(20));
                for wrapped_line in wrapped {
                    lines.push(
                        BlockLine::styled(wrapped_line)
                            .with_selection_range(Some(RECAP_BODY_RANGE)),
                    );
                }

                BlockOutput { lines }
            }
        }
    }
}

impl BlockContent for SessionEventBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        // Recap renders in the tool-call style (bullet, bold header, body)
        if let Some(summary) = self.event.recap_summary() {
            return self.recap_output(ctx, summary);
        }

        let theme = Theme::current();
        // Failures and re-auth / context-overflow prompts are actionable, not informational; render them in the warning color rather than muted
        let style = if self.event.is_warning_banner() {
            ratatui::style::Style::default().fg(theme.warning)
        } else {
            theme.muted()
        };

        let text = self.event.message_with_locale(&self.locale);
        let wrapped = if text.contains('\n') {
            let input_lines = text
                .split('\n')
                .map(|s| Line::from(Span::styled(s.to_owned(), style)));
            word_wrap_lines(input_lines, ctx.width as usize)
        } else {
            word_wrap_lines(
                std::iter::once(Line::from(Span::styled(text, style))),
                ctx.width as usize,
            )
        };
        let mut lines: Vec<BlockLine> = wrapped
            .into_iter()
            .map(|line| BlockLine::styled(line).with_selection_range(Some(0)))
            .collect();

        if lines.is_empty() {
            lines.push(BlockLine::styled(Line::from("")).with_selection_range(Some(0)));
        }
        BlockOutput { lines }
    }

    fn accent(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        if self.event.recap_summary().is_some() {
            // Loading: animated sidebar so there's feedback that the recap is being generated
            // Gray rather than the magenta `accent_running`: the recap is a passive marker, not an active tool turn
            if ctx.is_running {
                return Some(AccentStyle::animated(theme.gray));
            }
            // Finished: neutral tool accent bar when expanded (no special color).
            return (ctx.mode != DisplayMode::Collapsed)
                .then(|| AccentStyle::static_color(theme.accent_tool));
        }
        if self.event.is_warning_banner() {
            Some(AccentStyle::static_color(theme.warning))
        } else {
            None
        }
    }

    fn bullet(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        // Recap: animated dot while loading; default gray dot when collapsed-idle; accent color when expanded
        // A hook outcome keeps the default gray, matching its muted text; other events never show a bullet
        if matches!(self.event, SessionEvent::HookOutcome { .. })
            || (self.event.recap_summary().is_some()
                && !ctx.is_running
                && ctx.mode == DisplayMode::Collapsed)
        {
            return None;
        }
        self.accent(ctx)
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false // Compact like SystemMessageBlock
    }

    fn has_raw_mode(&self) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        // A recap with body content folds; other events are single informational lines with nothing to collapse
        self.recap_has_body()
    }

    fn is_selectable(&self) -> bool {
        // Recap is tool-like: navigable so it can be folded, but only once it has body content (mirrors `is_foldable`)
        // That way j/k never lands on the loading spinner or an empty recap. Other events stay non-interactive.
        self.recap_has_body()
    }

    fn default_display_mode(&self) -> DisplayMode {
        DisplayMode::Expanded
    }

    fn has_bullet(&self, ctx: &BlockContext) -> bool {
        // Recap and hook outcomes only, gated on the shared tool bullet so they track the tool rows' appearance setting
        (self.event.recap_summary().is_some()
            || matches!(self.event, SessionEvent::HookOutcome { .. }))
            && ctx
                .appearance
                .scrollback
                .blocks
                .tool
                .bullet
                .char()
                .is_some()
    }

    fn is_groupable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zh_locale() -> crate::locale::LocaleContext {
        crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::ZhCn,
            source: crate::locale::LocaleSource::Cli,
        })
    }

    #[test]
    fn zh_localization_new_session_events_preserve_payloads_and_permissions() {
        let zh = zh_locale();
        let en = crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::EnUs,
            source: crate::locale::LocaleSource::Cli,
        });
        let payload = "blocked by guard: {permission} /tmp/plan.md — 原始输出";
        let hook = SessionEvent::HookOutcome {
            message: payload.into(),
        };
        assert_eq!(hook.message_with_locale(&zh), payload);
        assert_eq!(hook.message_with_locale(&en), hook.message());
        for permission in [
            PermissionLabel::Ask,
            PermissionLabel::Auto,
            PermissionLabel::AlwaysApprove,
        ] {
            let entered = SessionEvent::PlanModeEnteredByAgent { permission };
            assert_eq!(entered.message_with_locale(&en), entered.message());
            assert_eq!(
                entered.message_with_locale(&zh),
                format!(
                    "代理已进入计划模式 · 当前权限模式：{} · 退出计划模式前，禁止编辑会话 plan.md 以外的文件",
                    permission.as_canonical(),
                )
            );
            for (outcome, verdict) in [
                (PlanReviewOutcome::Approved, "批准"),
                (PlanReviewOutcome::Abandoned, "放弃"),
            ] {
                let closed = SessionEvent::PlanReviewClosed {
                    outcome,
                    permission,
                };
                assert_eq!(closed.message_with_locale(&en), closed.message());
                assert_eq!(
                    closed.message_with_locale(&zh),
                    format!(
                        "计划已{verdict} · 计划模式已关闭 · 当前权限模式：{}",
                        permission.as_canonical(),
                    )
                );
            }
        }
    }

    #[test]
    fn turn_completed_message() {
        let event = SessionEvent::TurnCompleted {
            elapsed: Some(Duration::from_secs(125)),
        };
        assert_eq!(event.message(), "Worked for 2m5s");
    }

    #[test]
    fn localization_regression_session_event_localizes_chrome_only() {
        let event = SessionEvent::TurnCompleted {
            elapsed: Some(Duration::from_millis(3_200)),
        };
        assert_eq!(event.message(), "Worked for 3.2s");
        assert_eq!(event.message_with_locale(&zh_locale()), "工作用时 3.2s");

        let compaction = SessionEvent::CompactionCompleted {
            tokens_before: Some(20_000),
            tokens_after: 8_000,
            elapsed_ms: Some(1_500),
        };
        assert_eq!(
            compaction.message_with_locale(&zh_locale()),
            "上下文已压缩：20.0k → 8.0k Token（1.5s）"
        );

        let mut block = SessionEventBlock::new(event);
        block.set_locale(zh_locale());
        assert_eq!(plain(&block.output(&ctx()).lines[0]), "工作用时 3.2s");
    }

    #[test]
    fn hook_annotations_localize_known_shell_templates_and_preserve_opaque_values() {
        let blocked = SessionEvent::HookAnnotation {
            message: "⚠ Prompt blocked by global/guard: provider reason".into(),
            kind: Some(HookAnnotationKind::PromptBlocked),
        };
        assert_eq!(
            blocked.message_with_locale(&zh_locale()),
            "⚠ 提示被 global/guard 阻止：provider reason"
        );

        let held = SessionEvent::HookAnnotation {
            message: "⚠ 2 queued prompt(s) on hold after the block. Edit or remove them, or send a prompt to resume.".into(),
            kind: Some(HookAnnotationKind::QueuedPromptsHeld),
        };
        assert_eq!(
            held.message_with_locale(&zh_locale()),
            "⚠ 阻止后有 2 条排队提示处于保留状态。请编辑或移除它们，或发送提示以恢复。"
        );

        let requested = SessionEvent::HookAnnotation {
            message: "⚠ Prompt block requested by a user hook (not enforced for this origin): reason: {hook}".into(),
            kind: Some(HookAnnotationKind::PromptBlockRequestedNotEnforced),
        };
        assert_eq!(
            requested.message_with_locale(&zh_locale()),
            "⚠ a user hook 请求阻止提示（未对此来源强制执行）：reason: {hook}"
        );

        let interjections = SessionEvent::HookAnnotation {
            message: "⚠ 3 interjection(s) joined the held queue. Send a prompt to resume.".into(),
            kind: Some(HookAnnotationKind::InterjectionsJoined),
        };
        assert_eq!(
            interjections.message_with_locale(&zh_locale()),
            "⚠ 3 条插入消息已加入保留队列。请发送提示以恢复。"
        );

        let arbitrary = SessionEvent::HookAnnotation {
            message: "heads up from a custom hook".into(),
            kind: None,
        };
        assert_eq!(
            arbitrary.message_with_locale(&zh_locale()),
            "heads up from a custom hook"
        );
        let spoofed = SessionEvent::HookAnnotation {
            message: "⚠ Prompt blocked by custom/hook: dynamic text".into(),
            kind: None,
        };
        assert_eq!(
            spoofed.message_with_locale(&zh_locale()),
            "⚠ Prompt blocked by custom/hook: dynamic text"
        );
        assert_eq!(
            blocked.message(),
            "⚠ Prompt blocked by global/guard: provider reason"
        );
    }

    #[test]
    fn turn_cancelled_message() {
        let event = SessionEvent::TurnCancelled {
            elapsed: Duration::from_secs(10),
            cause: crate::scrollback::blocks::CancelledBy::User,
        };
        assert_eq!(event.message(), "Turn cancelled by user in 10s.");
    }

    #[test]
    fn hook_blocked_turn_is_distinct_and_localized() {
        let event = SessionEvent::TurnBlockedByHook {
            elapsed: Duration::from_secs(10),
        };
        assert_eq!(event.message(), "Turn blocked by a hook in 10s.");
        assert_eq!(
            event.message_with_locale(&zh_locale()),
            "钩子在 10s 后阻止了回合。"
        );
    }

    #[test]
    fn turn_cancelled_message_names_passive_cause() {
        let event = SessionEvent::TurnCancelled {
            elapsed: Duration::from_secs(10),
            cause: crate::scrollback::blocks::CancelledBy::SessionClosed,
        };
        assert_eq!(
            event.message(),
            "Turn cancelled because the session closed in 10s."
        );
        let event = SessionEvent::TurnCancelled {
            elapsed: Duration::from_secs(4),
            cause: crate::scrollback::blocks::CancelledBy::Unspecified,
        };
        assert_eq!(event.message(), "Turn cancelled in 4.0s.");
    }

    #[test]
    fn goal_completed_message_shows_end_to_end_time() {
        let event = SessionEvent::GoalCompleted {
            elapsed: Duration::from_secs(619),
        };
        assert_eq!(event.message(), "Goal complete in 10m19s end-to-end.");
    }

    #[test]
    fn turn_halted_message() {
        let event = SessionEvent::TurnHalted {
            elapsed: Duration::from_secs(45),
        };
        assert_eq!(
            event.message(),
            "Agent was unable to make progress. Turn ended in 45s."
        );
    }

    #[test]
    fn turn_failed_with_elapsed_message() {
        let event = SessionEvent::TurnFailed {
            error: "connection reset".into(),
            elapsed: Some(Duration::from_secs(3)),
        };
        assert_eq!(event.message(), "Turn failed in 3.0s: connection reset");
    }

    #[test]
    fn turn_failed_without_elapsed_message() {
        let event = SessionEvent::TurnFailed {
            error: "auth error".into(),
            elapsed: None,
        };
        assert_eq!(event.message(), "Turn failed: auth error");
    }

    #[test]
    fn model_unavailable_with_switch() {
        let event = SessionEvent::ModelUnavailable {
            previous_model_id: "grok-4.5".into(),
            new_model_id: "grok-build".into(),
            reason: "Model \"grok-4.5\" is no longer available.".into(),
        };
        assert_eq!(
            event.message(),
            "Model \"grok-4.5\" is no longer available. Switched to \"grok-build\"."
        );
    }

    #[test]
    fn model_unavailable_blocked_reprompt() {
        let event = SessionEvent::ModelUnavailable {
            previous_model_id: String::new(),
            new_model_id: String::new(),
            reason: "Your previous model is no longer available. Please start a new session."
                .into(),
        };
        assert_eq!(
            event.message(),
            "Your previous model is no longer available. Please start a new session."
        );
    }

    #[test]
    fn zh_localization_model_unavailable_known_templates_preserve_model_ids() {
        let org = SessionEvent::ModelUnavailable {
            previous_model_id: "custom-preview".into(),
            new_model_id: "grok-4.6".into(),
            reason: "\"custom-preview\": This model isn't allowed by your organization's policy. Contact your administrator. This session is using \"grok-4.6\".".into(),
        };
        assert_eq!(
            org.message_with_locale(&zh_locale()),
            "“custom-preview”：组织策略不允许使用此模型。请联系管理员。当前会话正使用“grok-4.6”。 已切换到“grok-4.6”。"
        );

        let account = SessionEvent::ModelUnavailable {
            previous_model_id: "grok-enterprise".into(),
            new_model_id: String::new(),
            reason: "Model \"grok-enterprise\" is no longer available for your account.".into(),
        };
        assert_eq!(
            account.message_with_locale(&zh_locale()),
            "你的账户已无法使用模型“grok-enterprise”。"
        );

        let legacy = SessionEvent::ModelUnavailable {
            previous_model_id: "preview".into(),
            new_model_id: "grok-4.6".into(),
            reason: "\"preview\" isn't allowed by your allowed_models setting, so this session is using \"grok-4.6\".".into(),
        };
        assert!(
            legacy
                .message_with_locale(&zh_locale())
                .starts_with("“preview”：此模型不在 allowed_models 设置的允许范围内。")
        );

        let blocked = SessionEvent::ModelUnavailable {
            previous_model_id: String::new(),
            new_model_id: String::new(),
            reason: "Your previous model is no longer available and could not be switched to a compatible model. Please start a new session.".into(),
        };
        assert_eq!(
            blocked.message_with_locale(&zh_locale()),
            "你之前使用的模型已不可用，且无法切换到兼容模型。请启动新会话。"
        );
    }

    #[test]
    fn zh_localization_model_unavailable_unknown_reason_is_not_rewritten() {
        let reason = "Provider said: Model \"x\" is unavailable?!";
        let event = SessionEvent::ModelUnavailable {
            previous_model_id: "x".into(),
            new_model_id: String::new(),
            reason: reason.into(),
        };
        assert_eq!(event.message_with_locale(&zh_locale()), reason);
    }

    #[test]
    fn zh_localization_model_unavailable_exact_policy_failures() {
        for (reason, expected) in [
            (
                "The organization model policy is invalid. Contact your administrator.",
                "组织模型策略无效。请联系管理员。",
            ),
            (
                "None of your models are allowed by your organization's policy. Contact your administrator.",
                "组织策略未允许你的任何模型。请联系管理员。",
            ),
            (
                "None of your models are allowed by allowed_models. Broaden it or remove it from your config, then restart.",
                "allowed_models 未允许任何模型。请扩大匹配范围或从配置中移除该项，然后重启。",
            ),
        ] {
            let event = SessionEvent::ModelUnavailable {
                previous_model_id: String::new(),
                new_model_id: String::new(),
                reason: reason.into(),
            };
            assert_eq!(event.message_with_locale(&zh_locale()), expected);
        }
    }

    #[test]
    fn zh_localization_model_unavailable_policy_near_miss_is_not_rewritten() {
        let reason = "None of your models are allowed by allowed_models. Broaden it or remove it from your config, then restart!";
        let event = SessionEvent::ModelUnavailable {
            previous_model_id: String::new(),
            new_model_id: String::new(),
            reason: reason.into(),
        };
        assert_eq!(event.message_with_locale(&zh_locale()), reason);
    }

    #[test]
    fn zh_localization_model_unavailable_unknown_reason_placeholders_are_not_rewritten() {
        let reason = "Provider returned the literal token {model}.";
        let event = SessionEvent::ModelUnavailable {
            previous_model_id: "x".into(),
            new_model_id: "grok-4.6".into(),
            reason: reason.into(),
        };
        assert_eq!(
            event.message_with_locale(&zh_locale()),
            "Provider returned the literal token {model}. 已切换到“grok-4.6”。"
        );
    }

    #[test]
    fn retry_failed_generic() {
        let event = SessionEvent::RetryFailed {
            error: "connection timeout".into(),
            error_type: None,
        };
        assert_eq!(event.message(), "Retry failed: connection timeout");
    }

    #[test]
    fn retry_failed_encrypted_content_mismatch() {
        let event = SessionEvent::RetryFailed {
            error: "raw API error message".into(),
            error_type: Some("encrypted_content_mismatch".into()),
        };
        assert_eq!(
            event.message(),
            "This session's conversation history is incompatible with the \
             current model. Please start a new session."
        );
    }

    #[test]
    fn retry_failed_other_error_type_shows_raw() {
        let event = SessionEvent::RetryFailed {
            error: "bad request".into(),
            error_type: Some("api_400".into()),
        };
        assert_eq!(event.message(), "Retry failed: bad request");
    }

    #[test]
    fn reauth_required_message_points_at_login() {
        let msg = SessionEvent::ReAuthRequired.message();
        assert!(msg.contains("/login"), "must tell the user to run /login");
        assert!(
            msg.to_lowercase().contains("authentication")
                || msg.to_lowercase().contains("credentials"),
            "must explain it is an auth problem: {msg}"
        );
    }

    #[test]
    fn reauth_required_has_warning_accent() {
        let block = SessionEventBlock::new(SessionEvent::ReAuthRequired);
        let theme = Theme::current();
        let accent = block.accent(&ctx());
        assert_eq!(
            accent.map(|a| a.color),
            Some(theme.warning),
            "re-auth prompt must stand out with a warning accent"
        );
    }

    #[test]
    fn request_failed_message_and_warning_accent() {
        let event = SessionEvent::RequestFailed {
            status: Some(500),
            headline: "Server error (500)".into(),
            detail: "upstream exploded".into(),
        };
        assert_eq!(event.message(), "Server error (500): upstream exploded");
        let block = SessionEventBlock::new(event);
        let theme = Theme::current();
        assert_eq!(
            block.accent(&ctx()).map(|a| a.color),
            Some(theme.warning),
            "request-failed banner must stand out like re-auth"
        );
    }

    #[test]
    fn context_too_large_message_is_actionable() {
        let msg = SessionEvent::ContextTooLarge.message();
        assert!(
            msg.to_lowercase().contains("too large"),
            "must explain the conversation is too large: {msg}"
        );
        assert!(
            msg.contains("/new"),
            "must offer /new as the recovery action: {msg}"
        );
    }

    #[test]
    fn context_too_large_has_warning_accent() {
        let block = SessionEventBlock::new(SessionEvent::ContextTooLarge);
        let theme = Theme::current();
        let accent = block.accent(&ctx());
        assert_eq!(
            accent.map(|a| a.color),
            Some(theme.warning),
            "context-too-large prompt must stand out with a warning accent"
        );
    }

    #[test]
    fn compaction_started_switch_reason_renders_reason() {
        let event = SessionEvent::CompactionStarted {
            percentage: 9,
            reason: MODEL_FAMILY_SWITCH_COMPACT_BANNER.into(),
        };
        assert_eq!(event.message(), MODEL_FAMILY_SWITCH_COMPACT_BANNER);
    }

    #[test]
    fn compaction_started_threshold_reason_renders_fullness() {
        for reason in ["Context window 9% full", ""] {
            let event = SessionEvent::CompactionStarted {
                percentage: 9,
                reason: reason.into(),
            };
            assert_eq!(event.message(), "Context 9% full. Compacting…");
        }
    }

    #[test]
    fn compaction_completed_renders_before_after_delta() {
        let event = SessionEvent::CompactionCompleted {
            tokens_before: Some(48_800),
            tokens_after: 27_100,
            elapsed_ms: Some(21_000),
        };
        assert_eq!(
            event.message(),
            "Context compacted: 48.8k → 27.1k tokens (21.0s)"
        );
    }

    #[test]
    fn compaction_completed_without_before_keeps_legacy_format() {
        let event = SessionEvent::CompactionCompleted {
            tokens_before: None,
            tokens_after: 27_100,
            elapsed_ms: None,
        };
        assert_eq!(event.message(), "Context compacted → 27.1k tokens");
    }

    #[test]
    fn compaction_failed_empty_error_is_terse() {
        let event = SessionEvent::CompactionFailed {
            error: String::new(),
        };
        assert_eq!(event.message(), "Compaction failed.");
    }

    #[test]
    fn compaction_failed_curated_error_is_appended() {
        let event = SessionEvent::CompactionFailed {
            error: "out of credits or over your spending limit. Add credits and retry.".into(),
        };
        assert_eq!(
            event.message(),
            "Compaction failed - out of credits or over your spending limit. Add credits and retry."
        );
    }

    #[test]
    fn compaction_failed_two_line_error_renders_headline_then_detail() {
        // A guidance headline and a detail line, as the shell composes them
        let block = SessionEventBlock::new(SessionEvent::CompactionFailed {
            error: "it'll retry on the next turn, or start a new session using /new.\n\
                    API error (status 400 Bad Request): invalid_image: too big"
                .into(),
        });
        // Wide enough that word-wrap cannot split the two logical lines.
        let out = block.output(&BlockContext {
            width: 200,
            ..ctx()
        });
        assert_eq!(out.lines.len(), 2, "one block line per message line");
        assert_eq!(
            plain(nth(&out, 0)),
            "Compaction failed - it'll retry on the next turn, or start a new session using /new."
        );
        assert_eq!(
            plain(nth(&out, 1)),
            "API error (status 400 Bad Request): invalid_image: too big"
        );
    }

    #[test]
    fn compaction_failed_has_warning_accent() {
        let block = SessionEventBlock::new(SessionEvent::CompactionFailed {
            error: "out of credits or over your spending limit. Add credits and retry.".into(),
        });
        let theme = Theme::current();
        assert_eq!(
            block.accent(&ctx()).map(|a| a.color),
            Some(theme.warning),
            "an actionable compaction failure must use a warning accent, not muted"
        );
    }

    fn ctx() -> BlockContext {
        BlockContext {
            mode: crate::scrollback::types::DisplayMode::Expanded,
            is_running: false,
            width: 80,
            raw: false,
            max_lines: None,
            appearance: crate::appearance::AppearanceConfig::default(),
            is_selected: false,
            cwd: None,
            locale: Default::default(),
        }
    }

    #[test]
    fn memory_capture_debug_block_is_collapsed_by_default() {
        let block = MemoryCaptureBlock::new(
            2,
            4,
            vec![MemoryCaptureDebugEntry {
                statement: "Use the focused test target.".into(),
                body: Some("The full suite is expensive.".into()),
                path: "/tmp/memory/observation.md".into(),
            }],
        );
        assert_eq!(block.default_display_mode(), DisplayMode::Collapsed);

        let mut collapsed = ctx();
        collapsed.mode = DisplayMode::Collapsed;
        let output = block.output(&collapsed);
        let [line] = output.lines.as_slice() else {
            panic!("expected one collapsed line, got {}", output.lines.len());
        };
        let text = crate::scrollback::types::line_plain_text(&line.content);
        assert_eq!(
            text,
            "Model-generated memory debug output: 1 memory for turns 2-4"
        );
        assert!(line.link_target.is_none());
    }

    #[test]
    fn expanded_memory_capture_debug_block_shows_content_and_file_link() {
        let path = "/tmp/memory/observation.md";
        let block = MemoryCaptureBlock::new(
            2,
            4,
            vec![MemoryCaptureDebugEntry {
                statement: "Use the focused test target.".into(),
                body: Some("The full suite is expensive.".into()),
                path: path.into(),
            }],
        );
        let output = block.output(&ctx());
        let text = output
            .lines
            .iter()
            .map(|line| crate::scrollback::types::line_plain_text(&line.content))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Use the focused test target."));
        assert!(text.contains("The full suite is expensive."));
        assert!(text.contains("Open file \u{2192} observation.md"));
        assert!(output.lines.iter().any(|line| {
            line.link_target.as_ref()
                == Some(&crate::render::osc8::LinkTarget::File(Arc::from(
                    Path::new(path),
                )))
        }));
    }

    #[test]
    fn memory_capture_debug_sanitizes_controls_and_disarms_remote_links() {
        let local_path = "/tmp/memory/observation.md";
        let block = MemoryCaptureBlock::new(
            2,
            4,
            vec![MemoryCaptureDebugEntry {
                statement: "\u{1b}]8;;https://evil.example\u{7}trusted\u{1b}]8;;\u{7}".into(),
                body: Some("Visit https://evil.example or attacker@example.com\u{202e}".into()),
                path: local_path.into(),
            }],
        );
        let output = block.output(&ctx());
        let text = output
            .lines
            .iter()
            .map(|line| crate::scrollback::types::line_plain_text(&line.content))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Untrusted model-generated observation 1"));
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{7}'));
        assert!(!text.contains("https://"));
        assert!(!text.contains("attacker@example.com"));
        assert!(output.lines.iter().all(|line| {
            line.link_target.is_none()
                || line.link_target.as_ref()
                    == Some(&crate::render::osc8::LinkTarget::File(Arc::from(
                        Path::new(local_path),
                    )))
        }));
    }

    #[test]
    fn memory_saved_message_formats_correctly() {
        let event = SessionEvent::MemorySaved {
            path: "/some/absolute/path/MEMORY.md".into(),
            trigger: "flush".into(),
        };
        let msg = event.message();
        assert!(msg.starts_with("Memory saved (flush)"));
        assert!(msg.contains("/memory to view"));
    }

    #[test]
    fn plan_mode_row_messages() {
        let cases = [
            (
                SessionEvent::PlanModeEnteredByAgent {
                    permission: PermissionLabel::Ask,
                },
                "Agent entered plan mode · active permission mode: ask · file edits outside session plan.md blocked until plan mode exits",
            ),
            (
                SessionEvent::PlanReviewClosed {
                    outcome: PlanReviewOutcome::Approved,
                    permission: PermissionLabel::Auto,
                },
                "Plan approved · plan mode off · active permission mode: auto",
            ),
            (
                SessionEvent::PlanReviewClosed {
                    outcome: PlanReviewOutcome::Abandoned,
                    permission: PermissionLabel::AlwaysApprove,
                },
                "Plan abandoned · plan mode off · active permission mode: always-approve",
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(event.message(), expected);
        }
    }

    #[test]
    fn recap_manual_and_auto_use_same_label() {
        let manual = SessionEvent::Recap {
            summary: "refactored the parser".into(),
            auto: false,
        };
        assert_eq!(manual.message(), "Recap: refactored the parser");

        let auto = SessionEvent::Recap {
            summary: "refactored the parser".into(),
            auto: true,
        };
        assert_eq!(auto.message(), "Recap: refactored the parser");
    }

    /// `ctx()` with an overridden display mode / selection state.
    fn recap_ctx(mode: DisplayMode, is_selected: bool) -> BlockContext {
        BlockContext {
            mode,
            is_selected,
            ..ctx()
        }
    }

    /// `ctx()` in the running/loading state (mode is `Expanded` from `ctx()`).
    fn recap_running_ctx() -> BlockContext {
        BlockContext {
            is_running: true,
            ..ctx()
        }
    }

    fn plain(line: &BlockLine) -> String {
        crate::scrollback::types::line_plain_text(&line.content)
    }

    fn nth<'a>(out: &'a BlockOutput, i: usize) -> &'a BlockLine {
        let Some(line) = out.lines.get(i) else {
            panic!("expected line {i}, got {} lines", out.lines.len());
        };
        line
    }

    #[test]
    fn recap_renders_tool_style_header_and_body() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "Refactored the parser and added tests.".into(),
            auto: false,
        });
        let out = block.output(&recap_ctx(DisplayMode::Expanded, false));
        assert_eq!(
            plain(nth(&out, 0)),
            "Recap",
            "header line is the 'Recap' label"
        );
        let body = out.lines.iter().map(plain).collect::<Vec<_>>().join("\n");
        assert!(
            body.contains("Refactored the parser and added tests."),
            "summary is shown as body text: {body}"
        );
    }

    /// The deny / failure line takes the tool rows' bullet (config-gated like theirs); a plain hook note stays unbulleted.
    #[test]
    fn hook_outcome_takes_the_tool_bullet_and_a_note_does_not() {
        let outcome = SessionEventBlock::new(SessionEvent::HookOutcome {
            message: "`web_fetch` blocked by global/qa: no".into(),
        });
        let note = SessionEventBlock::new(SessionEvent::HookAnnotation {
            message: "`web_fetch` blocked by global/qa: no".into(),
            kind: None,
        });
        assert!(outcome.has_bullet(&ctx()));
        assert!(!note.has_bullet(&ctx()));
        assert!(
            outcome.bullet(&ctx()).is_none(),
            "default gray, matching the muted text"
        );

        let mut no_bullet = ctx();
        no_bullet.appearance.scrollback.blocks.tool.bullet = crate::appearance::ToolBullet::None;
        assert!(
            !outcome.has_bullet(&no_bullet),
            "a user who turned tool bullets off gets none on hook lines either"
        );
        assert_eq!(
            outcome
                .output(&ctx())
                .lines
                .first()
                .map(|line| line.content.to_string()),
            Some("`web_fetch` blocked by global/qa: no".to_string()),
            "the message itself carries no glyph"
        );
    }

    #[test]
    fn recap_is_foldable_selectable_and_bulleted_open_by_default() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "did stuff".into(),
            auto: false,
        });
        assert!(
            block.is_foldable(),
            "recap collapses/expands like a tool call"
        );
        assert!(
            block.is_selectable(),
            "recap is navigable so it can be folded"
        );
        assert!(
            block.has_bullet(&ctx()),
            "recap shows the shared tool bullet under default appearance"
        );
        assert_eq!(
            block.default_display_mode(),
            DisplayMode::Expanded,
            "recap is open by default"
        );
    }

    #[test]
    fn recap_accent_and_bullet_use_neutral_tool_color_when_idle() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "did stuff".into(),
            auto: false,
        });
        let theme = Theme::current();
        // Expanded: static neutral tool accent bar (no special color).
        let expanded = block.accent(&recap_ctx(DisplayMode::Expanded, false));
        assert_eq!(expanded.map(|a| a.color), Some(theme.accent_tool));
        assert_eq!(expanded.map(|a| a.animated), Some(false));
        // Collapsed: no accent bar; bullet falls back to the default gray dot.
        assert_eq!(
            block.accent(&recap_ctx(DisplayMode::Collapsed, false)),
            None
        );
        assert_eq!(
            block.bullet(&recap_ctx(DisplayMode::Collapsed, false)),
            None
        );
    }

    #[test]
    fn recap_loading_shows_header_only_with_animated_sidebar() {
        // An empty summary on a running entry is the in-flight loading state
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: String::new(),
            auto: false,
        });
        let theme = Theme::current();
        let rc = recap_running_ctx();

        // Header only: no blank line or body while still generating
        let out = block.output(&rc);
        assert_eq!(out.lines.len(), 1, "loading recap is just the header");
        assert_eq!(plain(nth(&out, 0)), "Recap");

        // The sidebar and bullet animate in gray (the feedback), not the magenta running color used for active tool turns
        let accent = block.accent(&rc).expect("loading recap has an accent bar");
        assert_eq!(accent.color, theme.gray);
        assert!(accent.animated, "loading sidebar animates");
        assert_eq!(
            block.bullet(&rc),
            Some(accent),
            "bullet animates while loading"
        );
    }

    #[test]
    fn recap_collapsed_shows_header_with_first_line_preview() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "First line of recap.\nSecond line.".into(),
            auto: false,
        });
        let out = block.output(&recap_ctx(DisplayMode::Collapsed, false));
        assert_eq!(out.lines.len(), 1, "collapsed recap is a single line");
        let text = plain(nth(&out, 0));
        assert!(text.starts_with("Recap"), "starts with the header: {text}");
        assert!(
            text.contains("First line of recap."),
            "shows a preview: {text}"
        );
        assert!(
            !text.contains("Second line"),
            "preview is first line only: {text}"
        );
    }

    #[test]
    fn recap_loading_or_empty_is_not_selectable_or_foldable() {
        // The in-flight spinner (empty summary) and any stray empty recap must not be selectable or foldable
        // j/k must never stop on a block that can't fold and offers no interaction (mirrors `is_foldable`)
        for summary in ["", "   \n  "] {
            let block = SessionEventBlock::new(SessionEvent::Recap {
                summary: summary.into(),
                auto: false,
            });
            assert!(
                !block.is_selectable(),
                "empty/loading recap is not selectable: {summary:?}"
            );
            assert!(
                !block.is_foldable(),
                "empty/loading recap is not foldable: {summary:?}"
            );
        }
    }

    #[test]
    fn recap_expanded_header_is_not_text_selectable() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "We fixed the recap body selection.\nSecond line.".into(),
            auto: false,
        });
        let out = block.output(&recap_ctx(DisplayMode::Expanded, false));
        assert!(
            matches!(nth(&out, 0).selectable, Selectable::None),
            "header must be decoration, not copyable"
        );
        assert_eq!(nth(&out, 0).selection_range, None);
        assert!(
            matches!(nth(&out, 1).selectable, Selectable::None),
            "blank gap under header must not be selectable"
        );
        let body: Vec<_> = out.lines.iter().skip(2).collect();
        assert!(!body.is_empty(), "expected body lines");
        for line in body {
            assert!(
                matches!(line.selectable, Selectable::All),
                "body lines are fully selectable"
            );
            assert_eq!(
                line.selection_range,
                Some(0),
                "body shares one selection range so multi-line drag works"
            );
        }
    }

    #[test]
    fn recap_collapsed_only_preview_is_text_selectable() {
        let block = SessionEventBlock::new(SessionEvent::Recap {
            summary: "First line of recap.\nSecond line.".into(),
            auto: false,
        });
        let out = block.output(&recap_ctx(DisplayMode::Collapsed, false));
        assert_eq!(out.lines.len(), 1);
        let line = nth(&out, 0);
        assert!(
            matches!(&line.selectable, Selectable::Spans(r) if *r == (1..2)),
            "only the preview span is selectable, not the Recap label: {:?}",
            line.selectable
        );
        assert_eq!(line.selection_range, Some(0));
        assert_eq!(
            line.selection_text.as_deref(),
            Some("First line of recap."),
            "copy payload is the preview body only"
        );
    }

    #[test]
    fn non_recap_events_stay_non_interactive() {
        let block = SessionEventBlock::new(SessionEvent::TurnCompleted {
            elapsed: Some(Duration::from_secs(5)),
        });
        assert!(!block.is_foldable());
        assert!(!block.is_selectable());
        assert!(!block.has_bullet(&ctx()));
        assert_eq!(block.accent(&ctx()), None);
    }
    #[test]
    fn zh_localization_review135_memory_debug_retains_payload_and_local_link_only() {
        let path = "/tmp/memory/observation.md";
        let body = "literal {activity}, 原始正文 remains unchanged";
        let block = MemoryCaptureBlock::new(
            2,
            4,
            vec![MemoryCaptureDebugEntry {
                statement: "Original observation".into(),
                body: Some(body.into()),
                path: path.into(),
            }],
        );
        let output = block.output(&BlockContext {
            locale: zh_locale(),
            width: 160,
            ..ctx()
        });
        let text = output
            .lines
            .iter()
            .map(|line| crate::scrollback::types::line_plain_text(&line.content))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("模型生成的记忆调试输出：第 2–4 回合，共 1 条记忆"),
            "{text}"
        );
        assert!(text.contains("未经信任的模型生成观察记录 1"), "{text}");
        assert!(text.contains(body), "{text}");
        assert!(text.contains("Original observation"));
        assert!(text.contains("打开文件 → observation.md"), "{text}");
        let links = output
            .lines
            .iter()
            .filter_map(|line| line.link_target.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(
            links,
            vec![&crate::render::osc8::LinkTarget::File(Arc::from(
                Path::new(path)
            ))]
        );
        // The Chinese label is not a license to make model text or URLs trusted.
        let untrusted = MemoryCaptureBlock::new(
            2,
            4,
            vec![MemoryCaptureDebugEntry {
                statement: "\u{1b}]8;;https://evil.example\u{7}trusted\u{1b}]8;;\u{7}".into(),
                body: Some("attacker@example.com\u{202e}".into()),
                path: path.into(),
            }],
        );
        let output = untrusted.output(&BlockContext {
            locale: zh_locale(),
            ..ctx()
        });
        let text = output
            .lines
            .iter()
            .map(|line| crate::scrollback::types::line_plain_text(&line.content))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
        assert!(!text.contains("https://"));
        assert!(!text.contains("attacker@example.com"));
        assert!(output.lines.iter().all(|line| line.link_target.is_none()
            || line.link_target.as_ref()
                == Some(&crate::render::osc8::LinkTarget::File(Arc::from(
                    Path::new(path)
                )))));
    }
}
