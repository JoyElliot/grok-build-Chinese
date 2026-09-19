//! Fork dispatchers and fork placeholder builders.
use super::lifecycle::{dispatch_new_session_inner_with_id, refuse_chat_mode_build_agent};
use super::load::session_opens_as_chat;
use crate::acp::tracker::AcpUpdateTracker;
use crate::app::actions::Effect;
use crate::app::agent::{AgentCommand, AgentId, AgentSession, AgentState};
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView};
use crate::app::cancel_latency::TurnEnd;
use crate::app::dispatch::ctx::{SwitchCause, switch_to_agent};
use crate::app::dispatch::modes::inherit_auto_mode;
use crate::app::dispatch::prompt::supersede_open_reload_window;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SessionEvent;
use crate::scrollback::state::ScrollbackState;
use agent_client_protocol as acp;
use std::time::Instant;

fn localized_template(
    locale: &crate::locale::LocaleContext,
    id: &str,
    english: &str,
    replacements: &[(&str, &str)],
) -> String {
    let mut message = locale.named_text(id, english).into_owned();
    for (placeholder, value) in replacements {
        message = message.replace(placeholder, value);
    }
    message
}

/// Top-level `/fork` dispatcher. Resolves the worktree decision: an
/// explicit `--worktree` / `--no-worktree` flag short-circuits to
/// [`dispatch_fork_resolved`]. When no flag is given and a persisted
/// `fork_worktree_mode` preference is set (`Always` / `Never`), the
/// popup is skipped and the corresponding path is taken directly. The
/// `Ask` default opens the [`open_fork_question`] modal so the user is
/// asked.
///
/// When the parent session's cwd is not inside a git repository (no `git_head_changed` notification, so `current_branch` is `None`):
/// - `--worktree` is rejected with a toast (nothing to create a worktree from).
/// - No flag (regardless of `fork_worktree_mode`): the worktree question is skipped and the fork proceeds with `worktree = false`.
///
/// If the notification has not arrived yet (the user forks before the shell sends `git_head_changed`), the `worktree = false` fallback is safe.
pub(in crate::app::dispatch) fn dispatch_fork(
    app: &mut AppView,
    args: crate::slash::commands::fork::ForkArgs,
) -> Vec<Effect> {
    let ActiveView::Agent(parent_id) = app.active_view else {
        let toast = app
            .locale
            .named_text(
                "session.fork.only_inside_session",
                "/fork only works inside a session",
            )
            .into_owned();
        app.show_toast(&toast);
        return vec![];
    };
    let (has_session, in_git_repo) = app
        .agents
        .get(&parent_id)
        .map(|a| (a.session.session_id.is_some(), a.current_branch.is_some()))
        .unwrap_or((false, false));
    if !has_session {
        let toast = app
            .locale
            .named_text(
                "session.fork.still_creating",
                "Cannot fork: session is still being created",
            )
            .into_owned();
        app.show_toast(&toast);
        return vec![];
    }
    match args.worktree_override {
        Some(true) if !in_git_repo => {
            let toast = app
                .locale
                .named_text(
                    "session.worktree.not_git_repo",
                    "Cannot create worktree: not in a git repository",
                )
                .into_owned();
            app.show_toast(&toast);
            vec![]
        }
        Some(worktree) => dispatch_fork_resolved(app, worktree, args.directive),
        None => {
            if in_git_repo {
                use crate::app::app_view::WorktreeMode;
                match app.fork_worktree_mode {
                    WorktreeMode::Always => dispatch_fork_resolved(app, true, args.directive),
                    WorktreeMode::Never => dispatch_fork_resolved(app, false, args.directive),
                    WorktreeMode::Ask => open_fork_question(app, args.directive),
                }
            } else {
                dispatch_fork_resolved(app, false, args.directive)
            }
        }
    }
}
/// If `persist_mode` is `Some`, write `mode` into `*field` and append a [`Effect::PersistWorktreeMode`] to `effects` with the given `config_key`.
pub(in crate::app::dispatch) fn apply_persist_worktree_mode(
    field: &mut crate::app::app_view::WorktreeMode,
    effects: &mut Vec<Effect>,
    persist_mode: Option<crate::app::app_view::WorktreeMode>,
    config_key: &'static str,
) {
    if let Some(mode) = persist_mode {
        *field = mode;
        effects.push(Effect::PersistWorktreeMode { mode, config_key });
    }
}
/// Build the two persistence options shared by the fork and new-session
/// worktree question modals ("Always worktree" / "Never worktree").
pub(super) fn worktree_persist_options(
    locale: &crate::locale::LocaleContext,
) -> [xai_grok_tools::implementations::grok_build::ask_user_question::QuestionOption; 2] {
    use xai_grok_tools::implementations::grok_build::ask_user_question::QuestionOption;
    [
        QuestionOption {
            label: locale
                .named_text("session.worktree.option.always.label", "Always worktree")
                .into_owned(),
            description: locale
                .named_text(
                    "session.worktree.option.always.description",
                    "Use worktree and stop asking (reset in config.toml)",
                )
                .into_owned(),
            preview: None,
            id: None,
        },
        QuestionOption {
            label: locale
                .named_text("session.worktree.option.never.label", "Never worktree")
                .into_owned(),
            description: locale
                .named_text(
                    "session.worktree.option.never.description",
                    "Skip worktree and stop asking (reset in config.toml)",
                )
                .into_owned(),
            preview: None,
            id: None,
        },
    ]
}
/// Open the local worktree question modal on the active agent.
/// Refuses with a toast if a question (ACP or local) is already on screen, so two questions never collide.
fn open_fork_question(app: &mut AppView, directive: Option<String>) -> Vec<Effect> {
    use crate::views::question_view::{LocalQuestionKind, QuestionViewState};
    use xai_grok_tools::implementations::grok_build::ask_user_question::{
        Question, QuestionOption,
    };
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let locale = app.locale.clone();
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    if agent.question_view.is_some() {
        let toast = locale
            .named_text(
                "question.finish_current_first",
                "Finish answering the current question first",
            )
            .into_owned();
        app.show_toast(&toast);
        return vec![];
    }
    let mut options = vec![
        QuestionOption {
            label: locale.named_text("question.option.yes", "Yes").into_owned(),
            description: locale
                .named_text(
                    "session.fork.option.worktree",
                    "Fork in a new isolated git worktree",
                )
                .into_owned(),
            preview: None,
            id: None,
        },
        QuestionOption {
            label: locale.named_text("question.option.no", "No").into_owned(),
            description: locale
                .named_text("session.fork.option.current_cwd", "Fork in the current cwd")
                .into_owned(),
            preview: None,
            id: None,
        },
    ];
    options.extend(worktree_persist_options(locale.as_ref()));
    let question = Question {
        question: locale
            .named_text(
                "session.fork.question.worktree",
                "Run this fork in an isolated git worktree?",
            )
            .into_owned(),
        id: None,
        options,
        multi_select: Some(false),
    };
    let agent = app.agents.get_mut(&id).expect("agent present (re-borrow)");
    let stashed = agent.prompt.stash();
    let state = QuestionViewState::new(
        format!("fork-{}", uuid::Uuid::new_v4()),
        vec![question],
        stashed,
    )
    .with_local_kind(LocalQuestionKind::Fork { directive });
    agent.install_local_question(state);
    agent.prompt.set_text("");
    vec![]
}
/// Construct the placeholder agent, push discoverability markers, flip the discovery gate, switch to the new agent, and emit the fork effect.
/// `worktree == true` reuses the [`Effect::CreateWorktreeSession`] pipeline (with `load_session_id` set to the parent session id).
/// `worktree == false` emits [`Effect::ForkSession`], which calls `x.ai/session/fork` directly.
pub(in crate::app::dispatch) fn dispatch_fork_resolved(
    app: &mut AppView,
    worktree: bool,
    directive: Option<String>,
) -> Vec<Effect> {
    let ActiveView::Agent(parent_id) = app.active_view else {
        return vec![];
    };
    let locale = app.locale.clone();
    let Some(parent) = app.agents.get(&parent_id) else {
        return vec![];
    };
    let Some(parent_session_id) = parent.session.session_id.clone() else {
        let toast = locale
            .named_text(
                "session.fork.not_yet_created",
                "Cannot fork: session not yet created",
            )
            .into_owned();
        app.show_toast(&toast);
        return vec![];
    };
    let parent_cwd = parent.session.cwd.clone();
    let parent_is_worktree = parent.session.is_worktree;
    let new_id = AgentId(app.next_agent_id);
    app.next_agent_id += 1;
    let new_agent = build_fork_placeholder(app, new_id, parent_id, &parent_cwd, worktree);
    let parent_marker = match directive.as_deref() {
        Some(d) => localized_template(
            locale.as_ref(),
            "session.fork.parent_marker.directive",
            "Forked: {directive}",
            &[("{directive}", d)],
        ),
        None => locale
            .named_text("session.fork.parent_marker", "Forked")
            .into_owned(),
    };
    let parent_chat_kind = parent.chat_kind || app.chat_mode;
    let parent_conversation_entry = parent.conversation_entry;
    app.agents.insert(new_id, new_agent);
    {
        let agent = app
            .agents
            .get_mut(&new_id)
            .expect("just-inserted agent missing");
        agent.prompt.set_compact(app.appearance.prompt.compact);
        agent.prompt.adopt_slash_mru(app.slash_mru.clone());
        agent.prompt.adopt_command_tags(app.command_tags.clone());
        agent
            .prompt
            .set_contextual_hints(app.contextual_hints.undo, app.contextual_hints.plan_mode);
        agent.set_session_recap_available(app.session_recap_available);
        agent.set_voice_mode_available(app.voice_mode_enabled);
        agent.apply_app_scoped_gates(
            app.sharing_enabled,
            app.usage_visible,
            !app.has_external_auth_provider,
            app.chat_mode,
            app.screen_mode,
            &app.active_announcements,
            &app.tier_restricted_commands,
        );
        agent.chat_kind = parent_chat_kind;
        agent.conversation_entry = parent_conversation_entry;
        agent.apply_credit_balance(app.credit_balance.clone(), app.auto_topup.clone());
        agent.set_plugins_visible_recursive(!app.appearance.disable_plugins);
        agent.pending_fork_banner = Some(crate::app::agent_view::PendingForkBanner {
            parent_sid: parent_session_id.0.to_string(),
            worktree,
        });
        if worktree {
            let message = locale
                .named_text("session.worktree.creating", "Creating worktree\u{2026}")
                .into_owned();
            agent.scrollback.push_block(RenderBlock::system(message));
        }
        agent.pending_first_prompt = directive;
    }
    if let Some(parent_mut) = app.agents.get_mut(&parent_id) {
        parent_mut
            .scrollback
            .push_block(RenderBlock::system(parent_marker));
    }
    switch_to_agent(app, new_id, SwitchCause::Fork);
    if worktree {
        vec![Effect::CreateWorktreeSession {
            agent_id: new_id,
            load_session_id: Some(parent_session_id.0.to_string()),
            label: None,
            git_ref: None,
            // Fork resumes the parent session, which carries its own model.
            model_id: None,
            permission_mode_override: None,
            preferred_session_id: None,
            minted_session_id: None,
            chat_kind: parent_chat_kind,
        }]
    } else {
        vec![Effect::ForkSession {
            agent_id: new_id,
            parent_session_id,
            parent_cwd,
            parent_is_worktree,
            new_session_id: None,
        }]
    }
}
/// Build the placeholder [`AgentView`] for a fork.
/// Centralises the `AgentSession`/spinner construction shared by the worktree and no-worktree branches so the parallel struct literal does not drift.
fn build_fork_placeholder(
    app: &AppView,
    new_id: AgentId,
    parent_id: AgentId,
    parent_cwd: &std::path::Path,
    worktree: bool,
) -> AgentView {
    let mut scrollback = ScrollbackState::new();
    scrollback.set_appearance(app.appearance.clone());
    let mut agent = AgentView::from_app(
        app,
        AgentSession {
            id: new_id,
            acp_tx: app.acp_tx.clone(),
            session_id: None,
            models: app.models.clone(),
            state: AgentState::Idle,
            tracker: AcpUpdateTracker::new(),
            cwd: parent_cwd.to_path_buf(),
            is_worktree: false,
            forked_from: Some(parent_id),
            pending_prompts: std::collections::VecDeque::new(),
            next_queue_id: 0,
            yolo_mode: app.default_yolo,
            auto_mode: inherit_auto_mode(app),
            prompt_history: Vec::new(),
            prompt_history_loading: false,
            loading_replay: false,
            restore_degree: None,
            rate_limited: false,
            model_incompatible: false,
            credit_limit_blocked: false,
            free_usage_blocked: false,
            available_commands: app.bootstrap_acp_commands.clone(),
            available_commands_generation: 1,
            available_tools: None,
            model_switch_pending: false,
            hook_block_hold: false,
            blocked_prompt: None,
            user_model_preference: None,
            deferred_model_switch: app.deferred_model_switch_from_cli(),
            bg_tasks: std::collections::BTreeMap::new(),
            bg_tool_call_to_task: std::collections::HashMap::new(),
            scheduled_tasks: std::collections::HashMap::new(),
            in_flight_prompt: None,
            compact_held_prompt: None,
            current_prompt_id: None,
            created_via_new: false,
        },
        scrollback,
    );
    let cmd = if worktree {
        AgentCommand::CreateWorktree
    } else {
        AgentCommand::ForkSession
    };
    agent.session.start_command(cmd);
    agent.turn_started_at = Some(Instant::now());
    agent
}
/// Build the discoverability banner for the child agent: the child's session id, the full parent session id, and optionally a session-switch tip.
/// The tip appears when `switch_hint` names a command: `/dashboard` normally, `/resume` in minimal mode where the dashboard is refused.
/// Called in `TaskResult::SessionLoaded` (not at dispatch time) because the child's session id is not known until the backend responds.
pub(in crate::app::dispatch) fn build_child_fork_marker(
    session_id: &str,
    parent_sid: &str,
    worktree: bool,
    switch_hint: Option<&str>,
) -> String {
    build_child_fork_marker_with_locale(session_id, parent_sid, worktree, switch_hint, None)
}

pub(in crate::app::dispatch) fn build_child_fork_marker_with_locale(
    session_id: &str,
    parent_sid: &str,
    worktree: bool,
    switch_hint: Option<&str>,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let header = if let Some(cmd) = switch_hint {
        let english = "Session {session_id} (forked from {parent_sid}), use {command} to switch between sessions";
        if let Some(locale) = locale {
            localized_template(
                locale,
                "session.fork.child_marker.switch",
                english,
                &[
                    ("{session_id}", session_id),
                    ("{parent_sid}", parent_sid),
                    ("{command}", cmd),
                ],
            )
        } else {
            english
                .replace("{session_id}", session_id)
                .replace("{parent_sid}", parent_sid)
                .replace("{command}", cmd)
        }
    } else {
        let english = "Session {session_id} (forked from {parent_sid})";
        if let Some(locale) = locale {
            localized_template(
                locale,
                "session.fork.child_marker",
                english,
                &[("{session_id}", session_id), ("{parent_sid}", parent_sid)],
            )
        } else {
            english
                .replace("{session_id}", session_id)
                .replace("{parent_sid}", parent_sid)
        }
    };
    if worktree {
        header
    } else {
        let suffix = locale
            .map(|locale| {
                locale
                    .named_text(
                        "session.fork.child_marker.shared_cwd",
                        "(both agents share cwd)",
                    )
                    .into_owned()
            })
            .unwrap_or_else(|| "(both agents share cwd)".to_string());
        format!("{header}\n  {suffix}")
    }
}
pub(in crate::app::dispatch) fn dispatch_startup_fork_session(
    app: &mut AppView,
    parent_session_id: String,
    parent_cwd: Option<std::path::PathBuf>,
    new_session_id: Option<String>,
) -> Vec<Effect> {
    if !app.session_startup_allowed() {
        app.deferred_startup.session =
            Some(crate::app::session_startup::DeferredSessionStartup::Fork {
                parent_session_id,
                parent_cwd,
                new_session_id,
            });
        return vec![];
    }
    let (_agent_id, mut effects) = dispatch_new_session_inner_with_id(app, None, false);
    let agent_id = app
        .agents
        .keys()
        .next_back()
        .copied()
        .expect("fork placeholder agent");
    effects.retain(|e| !matches!(e, Effect::CreateSession { .. }));
    let cwd = parent_cwd.unwrap_or_else(|| app.cwd.clone());
    let parent_is_worktree =
        crate::app::session_startup::parent_session_is_worktree(&parent_session_id, &cwd);
    effects.push(Effect::ForkSession {
        agent_id,
        parent_session_id: acp::SessionId::new(parent_session_id),
        parent_cwd: cwd,
        parent_is_worktree,
        new_session_id,
    });
    effects
}
#[allow(clippy::too_many_arguments)]
pub(in crate::app::dispatch) fn handle_worktree_forked(
    app: &mut AppView,
    agent_id: AgentId,
    session_id: acp::SessionId,
    worktree_path: std::path::PathBuf,
    session_cwd: std::path::PathBuf,
    code_restored: bool,
    restore_summary: Option<String>,
    restore_degree: Option<xai_grok_workspace::session::git::RestoreDegree>,
    resume_session_id: Option<String>,
    strategy_summary: Option<String>,
) -> Vec<Effect> {
    let session_id_str = session_id.0.to_string();
    let locale = app.locale.clone();
    let pending_entry = std::mem::take(&mut app.deferred_startup.pending_chat);
    let agent_entry = app
        .agents
        .get(&agent_id)
        .is_some_and(|a| a.conversation_entry);
    let conversation_entry = pending_entry || agent_entry;
    if crate::app::session_startup::chat_mode_refuses_local_build_load(
        app.chat_mode,
        conversation_entry,
        &session_id_str,
        &app.cwd,
    ) {
        refuse_chat_mode_build_agent(app, agent_id);
        return vec![];
    }
    let rename_entry = session_opens_as_chat(app, conversation_entry);
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        supersede_open_reload_window(agent, agent_id, "WorktreeForked");
        agent.session.finish_command();
        agent.mark_turn_finished(TurnEnd::Aborted);
        agent.bind_session_id(session_id);
        agent.scrollback.begin_batch();
        agent.begin_replay_window();
        agent.session.restore_degree = restore_degree;
        agent.session.cwd = session_cwd.clone();
        agent.session.is_worktree = true;
        agent.current_branch = None;
        agent.main_repo = None;
        agent.is_worktree = true;
        crate::git_info::populate_from_cwd_async(session_cwd.clone());
        app.restore_code = None;
        agent.prompt.file_search.retarget(&session_cwd);
        let worktree_path = worktree_path.display().to_string();
        let message = localized_template(
            locale.as_ref(),
            "session.worktree.ready",
            "Worktree ready: {path}",
            &[("{path}", &worktree_path)],
        );
        agent.scrollback.push_block(RenderBlock::system(message));
        if let Some(summary) = strategy_summary {
            agent.scrollback.push_block(RenderBlock::system(summary));
        }
        match (code_restored, restore_summary.as_deref()) {
            (true, Some(s)) => {
                let message = localized_template(
                    locale.as_ref(),
                    "session.load.code_restored",
                    "\u{2713} Code restored: {summary}",
                    &[("{summary}", s)],
                );
                agent.scrollback.push_block(RenderBlock::system(message));
            }
            (false, Some(s)) => {
                let message = localized_template(
                    locale.as_ref(),
                    "session.load.code_restore_failed",
                    "\u{26A0} Code restore failed: {summary}",
                    &[("{summary}", s)],
                );
                agent.scrollback.push_block(RenderBlock::system(message));
            }
            _ => {}
        }
        let effective_chat = conversation_entry || app.chat_mode;
        agent.chat_kind = effective_chat;
        agent.conversation_entry = rename_entry;
        agent.apply_credit_balance(app.credit_balance.clone(), app.auto_topup.clone());
    } else {
        return vec![];
    }
    if let Some(resume_id) = resume_session_id.as_deref() {
        crate::app::event_loop::retarget_suppress_code_restore(
            app,
            resume_id,
            session_id_str.clone(),
        );
    }
    vec![Effect::LoadSession {
        agent_id,
        session_id: session_id_str,
        session_cwd: Some(session_cwd),
        chat_kind: conversation_entry,
    }]
}
pub(in crate::app::dispatch) fn handle_fork_session_ready(
    app: &mut AppView,
    agent_id: AgentId,
    new_session_id: acp::SessionId,
    cwd: std::path::PathBuf,
    parent_session_id: acp::SessionId,
) -> Vec<Effect> {
    let session_id_str = new_session_id.0.to_string();
    let pending_entry = std::mem::take(&mut app.deferred_startup.pending_chat);
    let agent_entry = app
        .agents
        .get(&agent_id)
        .is_some_and(|a| a.conversation_entry);
    let conversation_entry = pending_entry || agent_entry;
    if crate::app::session_startup::chat_mode_refuses_local_build_load(
        app.chat_mode,
        conversation_entry,
        &session_id_str,
        &app.cwd,
    ) {
        refuse_chat_mode_build_agent(app, agent_id);
        return vec![];
    }
    let rename_entry = session_opens_as_chat(app, conversation_entry);
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        supersede_open_reload_window(agent, agent_id, "ForkSessionReady");
        agent.session.finish_command();
        agent.mark_turn_finished(TurnEnd::Aborted);
        agent.bind_session_id(new_session_id);
        agent.scrollback.begin_batch();
        agent.begin_replay_window();
        agent.session.cwd = cwd.clone();
        let effective_chat = conversation_entry || app.chat_mode;
        agent.chat_kind = effective_chat;
        agent.conversation_entry = rename_entry;
    } else {
        return vec![];
    }
    crate::app::event_loop::retarget_suppress_code_restore(
        app,
        parent_session_id.0.as_ref(),
        session_id_str.clone(),
    );
    vec![Effect::LoadSession {
        agent_id,
        session_id: session_id_str,
        session_cwd: Some(cwd),
        chat_kind: conversation_entry,
    }]
}
pub(in crate::app::dispatch) fn handle_fork_session_failed(
    app: &mut AppView,
    agent_id: AgentId,
    error: String,
) -> Vec<Effect> {
    tracing::error!(agent = ?agent_id, error = %error, "Fork session failed");
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        agent.pending_extensions_fetch = false;
        agent.session_starting_since = None;
        agent.session.finish_command();
        let elapsed = agent.turn_elapsed();
        agent.mark_turn_finished(TurnEnd::Aborted);
        agent.pending_first_prompt = None;
        agent.pending_fork_banner = None;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnFailed {
                error,
                elapsed,
            }));
    }
    vec![]
}
