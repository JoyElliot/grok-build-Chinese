use anyhow::Result;
use clap::Subcommand;
use xai_grok_login::{AuthManager, try_ensure_fresh_auth};
use xai_grok_shell::agent::config::Config as AgentConfig;
use xai_grok_shell::session::merge::MergedSession;
use xai_grok_shell::util::grok_home::grok_home;

use crate::locale::LocaleContext;

#[derive(Debug, clap::Args, Clone)]
pub struct SessionsArgs {
    #[command(subcommand)]
    command: SessionsCommand,
}

#[derive(Debug, Subcommand, Clone)]
enum SessionsCommand {
    /// 列出最近会话（等同于无查询词的搜索）
    List {
        /// 最多显示的会话数
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },
    /// 按关键词搜索会话
    Search {
        /// 搜索词（搜索摘要和首条提示词）。
        query: String,
        /// 最多显示的会话数
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },
    /// 从历史记录中永久删除会话
    Delete {
        /// 要删除的会话 ID。
        id: String,
    },
}

pub async fn run(
    args: SessionsArgs,
    agent_config: &AgentConfig,
    locale: &LocaleContext,
) -> Result<()> {
    // Best-effort only: never force an interactive public login here. Enterprise deployments may configure only a
    // deployment_key and a custom xai_api_base_url. Otherwise we still proceed so the SessionRegistryClient can use
    // the deployment_key when talking to the custom proxy.
    let auth = try_ensure_fresh_auth(
        &agent_config.grok_com_config,
        agent_config.endpoints.proxy_url(),
    )
    .await;

    let auth_manager = std::sync::Arc::new(AuthManager::new_with_proxy_base_url(
        &grok_home(),
        agent_config.grok_com_config.clone(),
        agent_config.endpoints.proxy_url(),
    ));

    let client = xai_grok_shell::agent::session_registry_client::SessionRegistryClient::new(
        agent_config.endpoints.proxy_url(),
        String::new(),
    )
    .with_deployment_key(agent_config.endpoints.deployment_key.clone())
    .with_alpha_test_key(agent_config.endpoints.alpha_test_key.clone())
    .with_auth(auth_manager.clone());

    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());

    match args.command {
        SessionsCommand::List { limit } => {
            let sessions = xai_grok_shell::session::merge::fetch_merged(
                Some(&client),
                cwd.to_str(),
                xai_grok_shell::session::merge::CwdScope::WithSiblings,
                None,
                limit,
                // The CLI listing is an inventory, not the resume picker.
                xai_grok_shell::session::visibility::HeadlessPolicy::Include,
            )
            .await;
            print_sessions_grouped(&sessions, locale);
        }
        SessionsCommand::Search { query, limit } => {
            use std::collections::HashSet;
            use xai_grok_shell::session::merge::REMOTE_TIMEOUT;
            use xai_grok_shell::session::storage::search::{
                IndexDecision, SessionSearchRequest, execute_search,
            };

            // Search is the only subcommand that reads the index, so it is the only one to start one
            let search = xai_grok_shell::session::storage::search::start_if_enabled(agent_config);

            let req = SessionSearchRequest {
                query,
                cwd: Some(cwd.to_string_lossy().to_string()),
                limit,
                offset: 0,
                include_content: true,
            };
            let root = grok_home();

            let remote_limit = (limit * 3).max(100) as i64;
            let (local_resp, remote_results) = tokio::join!(
                execute_search(IndexDecision::settled(&search), &root, &req),
                async {
                    tokio::time::timeout(
                        REMOTE_TIMEOUT,
                        client.search(Some(&req.query), remote_limit),
                    )
                    .await
                    .unwrap_or_else(|_| {
                        eprintln!(
                            "{}",
                            locale.named_text(
                                "sessions.search.remote_timeout",
                                "warning: remote session search timed out, showing local results only"
                            )
                        );
                        Ok(Vec::new())
                    })
                    .unwrap_or_else(|e| {
                        eprintln!(
                            "{}",
                            locale
                                .named_text(
                                    "sessions.search.remote_failed",
                                    "warning: remote session search failed: {error}"
                                )
                                .replace("{error}", &e.to_string())
                        );
                        Vec::new()
                    })
                }
            );

            let resp = local_resp?;
            if let Some(by) = search.off_reason() {
                eprintln!(
                    "{}",
                    locale
                        .named_text(
                            "sessions.search.local_off",
                            "warning: local session search is off ({reason}); searched remote sessions only."
                        )
                        .replace("{reason}", by)
                );
            }
            let local_ids: HashSet<&str> =
                resp.results.iter().map(|r| r.session_id.as_str()).collect();

            for hit in &resp.results {
                let title = if hit.title.is_empty() {
                    locale.named_static_text("sessions.common.untitled", "(untitled)")
                } else {
                    &hit.title
                };
                let time = chrono::DateTime::from_timestamp(hit.updated_at_unix, 0)
                    .map(|dt| {
                        dt.with_timezone(&chrono::Local)
                            .format("%b %d, %l:%M%P")
                            .to_string()
                    })
                    .unwrap_or_default();
                println!(
                    "{} ({}: {:.2})  {}\n  {}\n  {}",
                    hit.session_id,
                    locale.named_text("sessions.search.score", "score"),
                    hit.score,
                    time,
                    title,
                    hit.snippet.as_deref().unwrap_or("")
                );
            }

            let remaining = limit.saturating_sub(resp.results.len());
            let mut remote_shown = 0usize;
            for r in &remote_results {
                if remote_shown >= remaining {
                    break;
                }
                if local_ids.contains(r.session_id.as_str()) {
                    continue;
                }
                let title = if r.summary.is_empty() {
                    locale.named_static_text("sessions.common.untitled", "(untitled)")
                } else {
                    &r.summary
                };
                let time = chrono::DateTime::parse_from_rfc3339(&r.updated_at)
                    .map(|dt| {
                        dt.with_timezone(&chrono::Local)
                            .format("%b %d, %l:%M%P")
                            .to_string()
                    })
                    .unwrap_or_default();
                let snippet: String = r
                    .first_prompt
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect();
                println!(
                    "{} ({})  {}\n  {}\n  {}",
                    r.session_id,
                    locale.named_text("sessions.common.remote", "remote"),
                    time,
                    title,
                    snippet
                );
                remote_shown += 1;
            }

            println!(
                "\n{} {}",
                locale.named_text("sessions.search.total", "Total:"),
                resp.results.len() + remote_shown
            );
        }
        SessionsCommand::Delete { id } => {
            let local_matches = xai_grok_shell::session::persistence::list_summaries(None)
                .await?
                .into_iter()
                .filter(|summary| summary.info.id.0.as_ref() == id.as_str())
                .count();
            if local_matches > 1 {
                anyhow::bail!(
                    "{}",
                    locale
                        .named_text(
                            "sessions.delete.ambiguous_id",
                            "Session id {id} exists in multiple workspaces. Delete it from a specific workspace to avoid removing the wrong local or cloud copy."
                        )
                        .replace("{id}", &id)
                );
            }
            // Always attempt the remote delete when authenticated and not
            // ZDR — `list` / `search` likewise query remote unconditionally
            // rather than gating on storage mode (which the CLI cannot
            // resolve here: it builds config without remote settings). The
            // backend delete is idempotent (a `404` is treated as success),
            // so this is safe for local-only sessions with no remote copy.
            // ZDR teams never upload, so there is nothing remote to delete.
            let needs_remote = auth.as_ref().is_some_and(|a| !a.is_zdr_team());

            // Pass `cwd = None` so the session is found by id regardless of which workspace it was created in
            // The local delete still uses the resolved per-session cwd
            // No search handle: the eviction inside prunes the row from another process's index, so a delete never needs one of its own
            let deletion = xai_grok_shell::session::persistence::delete_session_history(
                &id,
                None,
                needs_remote,
                auth_manager.clone(),
                None,
            )
            .await?;

            if deletion.any_removed() {
                println!(
                    "{}",
                    locale
                        .named_text("sessions.delete.deleted", "Deleted session {id}")
                        .replace("{id}", &id)
                );
            } else {
                println!(
                    "{}",
                    locale
                        .named_text(
                            "sessions.delete.not_found",
                            "No session found with id {id}."
                        )
                        .replace("{id}", &id)
                );
            }
        }
    }

    Ok(())
}

/// Print sessions grouped by worktree label, preserving the original table
/// format with a `Label: <label>` header before each group.
fn print_sessions_grouped(sessions: &[MergedSession], locale: &LocaleContext) {
    if sessions.is_empty() {
        println!(
            "{}",
            locale.named_text("sessions.list.empty", "No sessions found.")
        );
        return;
    }

    // Group by worktree_label, sort alphabetically, None last.
    let mut groups: std::collections::BTreeMap<Option<&str>, Vec<&MergedSession>> =
        std::collections::BTreeMap::new();
    for s in sessions {
        groups
            .entry(s.worktree_label.as_deref())
            .or_default()
            .push(s);
    }

    let header = format!(
        "{:<36}  {:<10}  {:<10}  {:<10}  {}",
        locale.named_text("sessions.list.column.id", "SESSION ID"),
        locale.named_text("sessions.list.column.created", "CREATED"),
        locale.named_text("sessions.list.column.updated", "UPDATED"),
        locale.named_text("sessions.list.column.source", "SOURCE"),
        locale.named_text("sessions.list.column.summary", "SUMMARY")
    );

    // Labeled groups first (alphabetical), then unlabeled last.
    let none_group = groups.remove(&None);
    let print_group = |label_line: &str, members: &[&MergedSession]| {
        println!("\n{label_line}");
        println!("{header}");
        for s in members {
            let first_line;
            let summary: &str = if !s.summary.is_empty() {
                &s.summary
            } else if let Some(ref fp) = s.first_prompt
                && let Some(line) = fp.lines().find(|l| !l.trim().is_empty())
            {
                first_line = line.trim().to_string();
                &first_line
            } else {
                locale.named_static_text("sessions.list.no_summary", "(no summary)")
            };
            let truncated: String = summary.chars().take(50).collect();
            let created = s
                .created_at
                .get(..s.created_at.len().min(10))
                .unwrap_or(s.created_at.as_str());
            let updated = s
                .updated_at
                .get(..s.updated_at.len().min(10))
                .unwrap_or(s.updated_at.as_str());
            let source = match s.source.as_str() {
                "local" => locale.named_static_text("sessions.source.local", "local"),
                "remote" => locale.named_static_text("sessions.source.remote", "remote"),
                "both" => locale.named_static_text("sessions.source.both", "both"),
                _ => &s.source,
            };
            println!(
                "{}  {}  {}  {}  {}",
                s.session_id, created, updated, source, truncated
            );
        }
    };

    for (label, members) in &groups {
        let line = locale
            .named_text("sessions.list.label", "Label: {label}")
            .replace("{label}", label.unwrap_or(""));
        print_group(&line, members);
    }
    if let Some(members) = &none_group {
        print_group(
            locale.named_static_text("sessions.list.no_label", "(no label)"),
            members,
        );
    }
}
