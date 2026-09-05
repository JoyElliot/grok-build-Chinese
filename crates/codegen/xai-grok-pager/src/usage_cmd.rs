//! `grok usage <session-id> [turn]`: persisted token/cost usage.

use std::io::Write;

use anyhow::{Context, Result};
use xai_grok_shell::session::usage_file::{SessionUsageFile, UsageLoad};

use crate::locale::LocaleContext;

#[derive(Debug, clap::Args, Clone)]
pub struct UsageArgs {
    /// 会话 ID
    pub session_id: String,
    /// 回合编号；省略时显示会话总计及所有已记录回合
    pub turn: Option<u32>,
}

pub fn run(args: UsageArgs) -> Result<()> {
    run_with_locale(args, &LocaleContext::default())
}

pub fn run_with_locale(args: UsageArgs, locale: &LocaleContext) -> Result<()> {
    let payload = load_payload(&args.session_id, args.turn, locale)?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{}", serde_json::to_string_pretty(&payload)?)?;
    Ok(())
}

fn load_payload(
    session_id: &str,
    turn: Option<u32>,
    locale: &LocaleContext,
) -> Result<serde_json::Value> {
    match SessionUsageFile::load_for_session(session_id).with_context(|| {
        locale
            .named_text(
                "usage.cli.read_failed",
                "Failed to read usage for session '{session_id}'",
            )
            .replace("{session_id}", session_id)
    })? {
        UsageLoad::SessionNotFound => {
            anyhow::bail!(
                "{}",
                locale
                    .named_text(
                        "usage.cli.session_not_found",
                        "Session '{session_id}' not found."
                    )
                    .replace("{session_id}", session_id)
            )
        }
        UsageLoad::NoUsage => {
            anyhow::bail!(
                "{}",
                locale
                    .named_text(
                        "usage.cli.no_usage",
                        "No usage recorded for session '{session_id}'."
                    )
                    .replace("{session_id}", session_id)
            )
        }
        UsageLoad::Ready(file) => select_payload(&file, turn, session_id, locale),
    }
}

fn select_payload(
    file: &SessionUsageFile,
    turn: Option<u32>,
    session_id: &str,
    locale: &LocaleContext,
) -> Result<serde_json::Value> {
    match turn {
        None => Ok(serde_json::to_value(file)?),
        Some(turn_number) => {
            let Some(row) = file.turn(turn_number) else {
                anyhow::bail!(
                    "{}",
                    locale
                        .named_text(
                            "usage.cli.turn_not_found",
                            "Turn {turn_number} not found in session '{session_id}'."
                        )
                        .replace("{turn_number}", &turn_number.to_string())
                        .replace("{session_id}", session_id)
                );
            };
            Ok(serde_json::json!({
                "sessionId": file.session_id,
                "updatedAt": file.updated_at,
                "session": file.session,
                "turns": [row],
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LocaleSource, ResolvedLocale, UiLocale};
    use xai_grok_shell::session::usage_file::UsageSummary;

    fn zh_locale() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    fn file_with_turns() -> SessionUsageFile {
        let mut file = SessionUsageFile::new("sess-1");
        file.apply_turn(
            1,
            "t1",
            &UsageSummary {
                input_tokens: 10,
                output_tokens: 2,
                total_tokens: 12,
                model_calls: 1,
                turn_count: 1,
                ..Default::default()
            },
            None,
        );
        file.apply_turn(
            2,
            "t2",
            &UsageSummary {
                input_tokens: 25,
                output_tokens: 7,
                total_tokens: 32,
                model_calls: 2,
                ..Default::default()
            },
            Some(&UsageSummary {
                input_tokens: 10,
                output_tokens: 2,
                total_tokens: 12,
                model_calls: 1,
                ..Default::default()
            }),
        );
        file
    }

    #[test]
    fn omitted_turn_returns_session_and_all_turns() {
        let file = file_with_turns();
        let value = select_payload(&file, None, "sess-1", &LocaleContext::default()).unwrap();
        assert_eq!(value["sessionId"], "sess-1");
        assert_eq!(value["session"]["inputTokens"], 25);
        assert_eq!(value["turns"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn turn_index_returns_that_row() {
        let file = file_with_turns();
        let value = select_payload(&file, Some(2), "sess-1", &LocaleContext::default()).unwrap();
        assert_eq!(value["sessionId"], "sess-1");
        assert_eq!(value["session"]["inputTokens"], 25);
        assert_eq!(value["turns"].as_array().unwrap().len(), 1);
        assert_eq!(value["turns"][0]["turnNumber"], 2);
        assert_eq!(value["turns"][0]["inputTokens"], 15);
    }

    #[test]
    fn missing_turn_is_an_error() {
        let file = file_with_turns();
        let err = select_payload(&file, Some(9), "sess-1", &LocaleContext::default()).unwrap_err();
        assert!(err.to_string().contains("Turn 9 not found"));
        assert!(!err.to_string().contains("usage.json"));
    }

    #[test]
    fn missing_turn_is_localized_without_changing_dynamic_values() {
        let file = file_with_turns();
        let err = select_payload(&file, Some(9), "sess-1", &zh_locale()).unwrap_err();
        assert_eq!(err.to_string(), "会话“sess-1”中未找到第 9 回合。");
    }
}
