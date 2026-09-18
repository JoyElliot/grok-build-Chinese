mod display;
use agent_client_protocol as acp;
use anyhow::{Result, bail};
use clap::Subcommand;
use std::io::Write;
use tokio_util::sync::CancellationToken;
use xai_acp_lib::acp_send;
use xai_fast_worktree::WorktreeRecord;
use xai_grok_shell::agent::config::Config as AgentConfig;

fn localized_named(
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
/// Read the agent's own report types rather than copies, so a field added
/// there cannot go missing here.
pub use xai_fast_worktree::{DbStats, GcReport, KeptWorktree, RebuildReport};
#[derive(Debug, clap::Args, Clone)]
pub struct WorktreeArgs {
    #[command(subcommand)]
    command: WorktreeCommand,
}
#[derive(Debug, Subcommand, Clone)]
enum WorktreeCommand {
    /// 列出已跟踪的工作树
    #[command(visible_alias = "ls")]
    List {
        #[arg(long)]
        repo: Option<String>,
        #[arg(long, value_delimiter = ',')]
        r#type: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        all: bool,
    },
    /// 显示指定工作树的详细信息
    Show { id_or_path: String },
    /// 移除工作树
    Rm {
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(short, long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// 清理孤立或过期的工作树；无法确认工作仍有其他副本时会保留
    #[command(alias = "prune")]
    Gc {
        /// 仅报告将被移除的项目，不实际移除。
        #[arg(long)]
        dry_run: bool,
        /// 使闲置时间超过该值的工作树过期，例如 `7d`；未指定时不会使任何项过期。
        #[arg(long)]
        max_age: Option<String>,
        /// 跳过活动进程与受保护路径检查；不会绕过工作内容安全检查，强制移除请使用
        /// `grok-zh worktree rm`。
        #[arg(short, long)]
        force: bool,
    },
    /// 数据库维护
    Db {
        #[command(subcommand)]
        command: WorktreeDbCommand,
    },
}
#[derive(Debug, Subcommand, Clone)]
enum WorktreeDbCommand {
    /// 通过扫描文件系统重建数据库
    Rebuild,
    /// 显示数据库统计信息
    Stats,
    /// 输出数据库文件路径
    Path,
}
pub async fn run(args: WorktreeArgs, agent_config: &AgentConfig) -> Result<()> {
    run_with_locale(args, agent_config, &crate::locale::LocaleContext::default()).await
}

pub async fn run_with_locale(
    args: WorktreeArgs,
    agent_config: &AgentConfig,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    let cancel = CancellationToken::new();
    xai_grok_telemetry::startup::mark_utility_process();
    let spawned = crate::acp::spawn::spawn_grok_shell(agent_config.clone(), &cancel, None).await?;
    // Cancel + join on every return path, including the `?` below.
    let _agent_guard = crate::acp::spawn::AgentShutdownGuard::new_with_locale(
        cancel.clone(),
        Some(spawned.thread_handle),
        locale,
    );
    let _init: acp::InitializeResponse = acp_send(
        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .client_capabilities(
                acp::ClientCapabilities::new()
                    .fs(acp::FileSystemCapabilities::new())
                    .terminal(false),
            )
            .meta(
                serde_json::json!({
                    "clientType": crate::client_identity::HEADLESS_CLIENT_TYPE,
                    "clientVersion": crate::client_identity::PAGER_CLIENT_VERSION
                })
                .as_object()
                .cloned(),
            ),
        &spawned.channel.tx,
    )
    .await?;
    dispatch(args.command, &spawned.channel.tx, locale).await
}

async fn dispatch(
    command: WorktreeCommand,
    tx: &xai_acp_lib::AcpAgentTx,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    match command {
        WorktreeCommand::List {
            repo,
            r#type,
            json,
            all,
        } => cmd_list(tx, repo, r#type, json, all, locale).await,
        WorktreeCommand::Show { id_or_path } => cmd_show(tx, &id_or_path, locale).await,
        WorktreeCommand::Rm {
            ids,
            force,
            dry_run,
        } => cmd_rm(tx, ids, force, dry_run, locale).await,
        WorktreeCommand::Gc {
            dry_run,
            max_age,
            force,
        } => cmd_gc(tx, dry_run, max_age, force, locale).await,
        WorktreeCommand::Db { command } => cmd_db(tx, command, locale).await,
    }
}
fn ext_request<T: serde::Serialize>(
    method: &str,
    params: &T,
) -> Result<acp::ExtRequest, serde_json::Error> {
    let params = serde_json::value::to_raw_value(params)?;
    Ok(acp::ExtRequest::new(method, params.into()))
}
/// ACP extension responses are wrapped in `{ "result": T, "error": ... }`.
#[derive(serde::Deserialize)]
struct ExtEnvelope<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}
async fn ext_call<T: serde::de::DeserializeOwned>(
    tx: &xai_acp_lib::AcpAgentTx,
    method: &str,
    params: &impl serde::Serialize,
) -> Result<T> {
    let req =
        ext_request(method, params).map_err(|e| anyhow::anyhow!("failed to build request: {e}"))?;
    let resp = acp_send(req, tx)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let envelope: ExtEnvelope<T> = serde_json::from_str(resp.0.get())
        .map_err(|e| anyhow::anyhow!("response parse error: {e}"))?;
    if let Some(err) = envelope.error {
        bail!("ACP error: {err}");
    }
    envelope
        .result
        .ok_or_else(|| anyhow::anyhow!("ACP response missing result field"))
}
async fn cmd_list(
    tx: &xai_acp_lib::AcpAgentTx,
    repo: Option<String>,
    types: Vec<String>,
    json: bool,
    all: bool,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    let records: Vec<WorktreeRecord> = ext_call(
        tx,
        "x.ai/git/worktree/list",
        &serde_json::json!({
            "repo": repo,
            "type": types,
            "includeAll": all,
        }),
    )
    .await?;
    let mut out = std::io::stdout().lock();
    let written = if json {
        display::print_json(&records, &mut out)
    } else {
        display::print_table_with_locale(&records, &mut out, locale)
    };
    Ok(crate::util::ignore_broken_pipe(written)?)
}
async fn cmd_show(
    tx: &xai_acp_lib::AcpAgentTx,
    id_or_path: &str,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    let rec: Option<WorktreeRecord> = ext_call(
        tx,
        "x.ai/git/worktree/show",
        &serde_json::json!({ "idOrPath" : id_or_path }),
    )
    .await?;
    match rec {
        Some(r) => {
            let written =
                display::print_show_with_locale(&r, &mut std::io::stdout().lock(), locale);
            Ok(crate::util::ignore_broken_pipe(written)?)
        }
        None => bail!(localized_named(
            locale,
            "worktree.error.not_found",
            "worktree not found: {id_or_path}",
            &[("id_or_path", id_or_path)],
        )),
    }
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveResponse {
    removed: bool,
    #[serde(default)]
    resolved_path: Option<String>,
}
async fn cmd_rm(
    tx: &xai_acp_lib::AcpAgentTx,
    ids: Vec<String>,
    force: bool,
    dry_run: bool,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    for id_or_path in &ids {
        let resp: Result<RemoveResponse> = ext_call(
            tx,
            "x.ai/git/worktree/remove",
            &serde_json::json!({
                "idOrPath": id_or_path,
                "force": force,
                "dryRun": dry_run,
            }),
        )
        .await;
        match resp {
            Ok(r) => {
                let path = r.resolved_path.as_deref().unwrap_or(id_or_path);
                if dry_run {
                    println!(
                        "  {}",
                        localized_named(
                            locale,
                            "worktree.rm.would_remove",
                            "would remove: {path}",
                            &[("path", path)],
                        )
                    );
                } else if r.removed {
                    println!(
                        "  {}",
                        localized_named(
                            locale,
                            "worktree.rm.removed",
                            "removed: {path}",
                            &[("path", path)],
                        )
                    );
                }
            }
            Err(e) => eprintln!(
                "  {}",
                localized_named(
                    locale,
                    "worktree.rm.error",
                    "error removing {id_or_path}: {error}",
                    &[("id_or_path", id_or_path), ("error", &e.to_string())],
                )
            ),
        }
    }
    Ok(())
}
async fn cmd_gc(
    tx: &xai_acp_lib::AcpAgentTx,
    dry_run: bool,
    max_age: Option<String>,
    force: bool,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    let report: GcReport = ext_call(
        tx,
        "x.ai/git/worktree/gc",
        &serde_json::json!({
            "dryRun": dry_run,
            "maxAge": max_age,
            "force": force,
        }),
    )
    .await?;
    let mut out = std::io::stdout().lock();
    let written = (|| {
        if dry_run {
            writeln!(
                out,
                "{}",
                locale.named_text("worktree.gc.dry_run", "Dry run \u{2014} no changes made.",)
            )?;
        }
        display::print_gc_with_locale(&report, &mut out, locale)
    })();
    Ok(crate::util::ignore_broken_pipe(written)?)
}
async fn cmd_db(
    tx: &xai_acp_lib::AcpAgentTx,
    command: WorktreeDbCommand,
    locale: &crate::locale::LocaleContext,
) -> Result<()> {
    match command {
        WorktreeDbCommand::Stats => {
            let stats: DbStats = ext_call(tx, "x.ai/git/worktree/db/stats", &()).await?;
            let written =
                display::print_stats_with_locale(&stats, &mut std::io::stdout().lock(), locale);
            Ok(crate::util::ignore_broken_pipe(written)?)
        }
        WorktreeDbCommand::Path => {
            #[derive(serde::Deserialize)]
            struct PathResp {
                path: String,
            }
            let resp: PathResp = ext_call(tx, "x.ai/git/worktree/db/path", &()).await?;
            println!("{}", resp.path);
            Ok(())
        }
        WorktreeDbCommand::Rebuild => {
            let report: RebuildReport = ext_call(tx, "x.ai/git/worktree/db/rebuild", &()).await?;
            let written =
                display::print_rebuild_with_locale(&report, &mut std::io::stdout().lock(), locale);
            Ok(crate::util::ignore_broken_pipe(written)?)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ext_request_builds_list_with_filters() {
        let req = ext_request(
            "x.ai/git/worktree/list",
            &serde_json::json!({
                "repo": "xai",
                "type": ["session"],
                "includeAll": true,
            }),
        )
        .unwrap();
        assert_eq!(req.method.as_ref(), "x.ai/git/worktree/list");
        let params: serde_json::Value = serde_json::from_str(req.params.get()).unwrap();
        assert_eq!(params.get("repo").and_then(|v| v.as_str()), Some("xai"));
        assert_eq!(params.get("includeAll"), Some(&serde_json::json!(true)));
    }
    #[test]
    fn ext_request_builds_gc_with_max_age_string() {
        let req = ext_request(
            "x.ai/git/worktree/gc",
            &serde_json::json!({
                "dryRun": true,
                "maxAge": "7d",
                "force": false,
            }),
        )
        .unwrap();
        let params: serde_json::Value = serde_json::from_str(req.params.get()).unwrap();
        assert_eq!(params.get("maxAge").and_then(|v| v.as_str()), Some("7d"));
        assert_eq!(params.get("dryRun"), Some(&serde_json::json!(true)));
    }
    #[test]
    fn ext_request_builds_remove_with_id_or_path() {
        let req = ext_request(
            "x.ai/git/worktree/remove",
            &serde_json::json!({
                "idOrPath": "wt-abc123",
                "force": true,
                "dryRun": false,
            }),
        )
        .unwrap();
        let params: serde_json::Value = serde_json::from_str(req.params.get()).unwrap();
        assert_eq!(
            params.get("idOrPath").and_then(|v| v.as_str()),
            Some("wt-abc123")
        );
    }
    #[test]
    fn ext_request_builds_show() {
        let req = ext_request(
            "x.ai/git/worktree/show",
            &serde_json::json!({ "idOrPath": "/some/path" }),
        )
        .unwrap();
        let params: serde_json::Value = serde_json::from_str(req.params.get()).unwrap();
        assert_eq!(
            params.get("idOrPath").and_then(|v| v.as_str()),
            Some("/some/path")
        );
    }
    #[test]
    fn ext_request_builds_detach_salvage_clean() {
        let d = ext_request(
            "x.ai/git/worktree/detach",
            &serde_json::json!({ "idOrPath": "/wt", "allowCopy": false }),
        )
        .unwrap();
        assert_eq!(d.method.as_ref(), "x.ai/git/worktree/detach");
        let s = ext_request(
            "x.ai/git/worktree/salvage",
            &serde_json::json!({ "idOrPath": "/wt", "out": "/out" }),
        )
        .unwrap();
        assert_eq!(s.method.as_ref(), "x.ai/git/worktree/salvage");
        let c = ext_request(
            "x.ai/git/worktree/clean-artifacts",
            &serde_json::json!({ "idOrPath": "/wt" }),
        )
        .unwrap();
        assert_eq!(c.method.as_ref(), "x.ai/git/worktree/clean-artifacts");
    }
    #[test]
    fn ext_request_builds_db_stats_empty_params() {
        let req = ext_request("x.ai/git/worktree/db/stats", &()).unwrap();
        assert_eq!(req.method.as_ref(), "x.ai/git/worktree/db/stats");
    }
    #[test]
    fn remove_response_deserializes_with_resolved_path() {
        let json = r#"{"removed": true, "resolvedPath": "/resolved"}"#;
        let resp: RemoveResponse = serde_json::from_str(json).unwrap();
        assert!(resp.removed);
        assert_eq!(resp.resolved_path.as_deref(), Some("/resolved"));
    }
    #[test]
    fn remove_response_deserializes_without_resolved_path() {
        let json = r#"{"removed": true}"#;
        let resp: RemoveResponse = serde_json::from_str(json).unwrap();
        assert!(resp.removed);
        assert!(resp.resolved_path.is_none());
    }
    #[test]
    fn ext_envelope_unwraps_success_result() {
        let json = r#"{"result": {"path": "/home/user/.grok/worktrees.db"}, "error": null}"#;
        #[derive(serde::Deserialize)]
        struct PathResp {
            path: String,
        }
        let envelope: ExtEnvelope<PathResp> = serde_json::from_str(json).unwrap();
        assert!(envelope.error.is_none());
        let inner = envelope.result.unwrap();
        assert_eq!(inner.path, "/home/user/.grok/worktrees.db");
    }
    #[test]
    fn ext_envelope_unwraps_error_result() {
        let json = r#"{"result": null, "error": "something went wrong"}"#;
        let envelope: ExtEnvelope<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(envelope.result.is_none());
        assert!(envelope.error.is_some());
    }
    #[test]
    fn ext_envelope_unwraps_list_of_records() {
        let json = r#"{"result": [], "error": null}"#;
        let envelope: ExtEnvelope<Vec<WorktreeRecord>> = serde_json::from_str(json).unwrap();
        assert!(envelope.error.is_none());
        assert!(envelope.result.unwrap().is_empty());
    }
    #[test]
    fn ext_envelope_unwraps_db_stats() {
        let json = r#"{"result": {"total_records": 5, "alive_count": 3, "dead_count": 2, "db_file_bytes": 1024}}"#;
        let envelope: ExtEnvelope<DbStats> = serde_json::from_str(json).unwrap();
        let stats = envelope.result.unwrap();
        assert_eq!(stats.total_records, 5);
        assert_eq!(stats.alive_count, 3);
    }
    #[test]
    fn ext_envelope_unwraps_gc_report() {
        let json = r#"{"result": {"dead_removed": 2, "expired_removed": 1, "skipped_alive": 0}}"#;
        let envelope: ExtEnvelope<GcReport> = serde_json::from_str(json).unwrap();
        let report = envelope.result.unwrap();
        assert_eq!(report.dead_removed, 2);
        assert_eq!(report.expired_removed, 1);
        assert_eq!(report.remove_failed, 0);
    }
    /// The GC report prints kept (not reclaimable) worktrees apart from guarded (in use) ones, and non-repository paths apart from removals.
    #[test]
    fn kept_worktree_prints_apart_from_a_busy_one_and_from_a_removal() {
        let json = r#"{"result": {"dead_removed": 0, "expired_removed": 3, "skipped_alive": 0,
            "kept_unsafe": 2, "no_repo_paths": 1, "kept_reasons": {"dirty": 2},
            "kept": [{"path": "/wt", "reason": "dirty"}], "not_judged": 4, "unnamed": 5}}"#;
        let envelope: ExtEnvelope<GcReport> = serde_json::from_str(json).unwrap();
        let mut out = Vec::new();
        display::print_gc(&envelope.result.unwrap(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            [
                "GC report:",
                "  Dead records removed:      0",
                "  Expired worktrees removed: 3",
                "  Non-repository paths:      1",
                "  Skipped (guarded):         0",
                "  Kept (not reclaimable):    2",
                "    dirty: 2",
                "      /wt  (dirty)",
                "      and 1 more, named in the log",
                "  Not judged this pass:      4",
                "  Naming failed (kept):      5",
            ]
        );
    }
    #[test]
    fn rm_parses_short_force_flag() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: WorktreeCommand,
        }
        let cli = Cli::parse_from(["test", "rm", "-f", "wt-1"]);
        match cli.command {
            WorktreeCommand::Rm {
                ids,
                force,
                dry_run,
            } => {
                assert!(force);
                assert!(!dry_run);
                assert_eq!(ids, vec!["wt-1"]);
            }
            _ => panic!("expected Rm variant"),
        }
    }
    #[test]
    fn rm_parses_long_force_flag() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: WorktreeCommand,
        }
        let cli = Cli::parse_from(["test", "rm", "--force", "a", "b"]);
        match cli.command {
            WorktreeCommand::Rm {
                ids,
                force,
                dry_run,
            } => {
                assert!(force);
                assert!(!dry_run);
                assert_eq!(ids, vec!["a", "b"]);
            }
            _ => panic!("expected Rm variant"),
        }
    }
    #[test]
    fn gc_parses_short_force_flag() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: WorktreeCommand,
        }
        let cli = Cli::parse_from(["test", "gc", "-f"]);
        match cli.command {
            WorktreeCommand::Gc {
                force,
                dry_run,
                max_age,
            } => {
                assert!(force);
                assert!(!dry_run);
                assert!(max_age.is_none());
            }
            _ => panic!("expected Gc variant"),
        }
    }
    #[test]
    fn list_accepts_ls_alias() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: WorktreeCommand,
        }
        let cli = Cli::parse_from(["test", "ls", "--json"]);
        match cli.command {
            WorktreeCommand::List {
                repo,
                r#type,
                json,
                all,
            } => {
                assert!(repo.is_none());
                assert!(r#type.is_empty());
                assert!(json);
                assert!(!all);
            }
            _ => panic!("expected List variant"),
        }
    }
}
