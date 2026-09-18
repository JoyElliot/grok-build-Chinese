//! Wrapper that turns an ACP `AvailableCommand` into a `SlashCommand`.
//!
//! ACP-advertised commands appear in the dropdown but pass through to the shell for execution.
//! The wrapper stores `String` fields, consistent with the `&str` trait design.
//!
//! Skills (`SkillMeta::Skill`) are also passed through as `/name args` for the shell to expand, but marked `InjectSkill` for rendering.

use agent_client_protocol as acp;
use xai_grok_tools::implementations::skills::types::SkillScope;

use super::command::{CommandExecCtx, CommandProvenance, CommandResult, SlashCommand};

/// Identity of a skill as advertised in ACP `_meta`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIdentity {
    pub path: String,
    pub scope: SkillScope,
    /// Plugin install name when present (`acme`).
    pub plugin_name: Option<String>,
    /// Whether ACP metadata contained a `pluginName` key at all.
    ///
    /// Keep this separate from `plugin_name`: blank or mistyped values remain
    /// unusable as a display source, but their presence must still prevent the
    /// command from claiming trusted bundled/product provenance.
    pub plugin_name_key_present: bool,
}

impl SkillIdentity {
    /// Plugin install name, else the scope (plugin skills can carry any scope).
    pub fn source(&self) -> &str {
        self.plugin_name.as_deref().unwrap_or(self.scope.as_ref())
    }
}

/// Parsed ACP `_meta` skill fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillMeta {
    /// No skill keys.
    Absent,
    Skill(SkillIdentity),
    /// Unknown `scope` string (e.g. `"workflow"`). Pass through, don't error.
    Foreign,
    /// Skill-like keys present but invalid. Invocation errors rather than silently degrading.
    Malformed,
}

impl SkillMeta {
    pub fn parse(meta: Option<&serde_json::Map<String, serde_json::Value>>) -> Self {
        let Some(m) = meta else {
            return SkillMeta::Absent;
        };
        let path_val = m.get("path");
        let scope_val = m.get("scope");
        if path_val.is_none() && scope_val.is_none() {
            return SkillMeta::Absent;
        }
        let path = path_val.and_then(|v| v.as_str());
        let scope: Option<SkillScope> =
            scope_val.and_then(|v| serde_json::from_value(v.clone()).ok());
        match (path, scope) {
            (Some(path), Some(scope)) => SkillMeta::Skill(SkillIdentity {
                path: path.to_string(),
                scope,
                plugin_name: trimmed_string_field(m, "pluginName"),
                plugin_name_key_present: m.contains_key("pluginName"),
            }),
            (_, None) if scope_val.is_some_and(|v| v.is_string()) => SkillMeta::Foreign,
            _ => SkillMeta::Malformed,
        }
    }
}

fn trimmed_string_field(
    m: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    m.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Whether ACP metadata has a shell-owned shape that is safe for exact
/// presentation-localization allowlists.
///
/// Plain shell commands carry no metadata. Saved/bundled workflow projections
/// carry only `workflowSource` and an optional `workflowPath`; all other shapes
/// remain opaque even when their command text collides with an official shell
/// command.
fn has_trusted_shell_metadata(meta: Option<&serde_json::Map<String, serde_json::Value>>) -> bool {
    let Some(meta) = meta else {
        return true;
    };
    if !meta.contains_key("workflowSource")
        || meta
            .keys()
            .any(|key| !matches!(key.as_str(), "workflowSource" | "workflowPath"))
    {
        return false;
    }
    let Some(source) = meta.get("workflowSource").and_then(|value| value.as_str()) else {
        return false;
    };
    if !matches!(
        source,
        "builtin" | "bundled" | "project" | "user" | "inline"
    ) {
        return false;
    }
    matches!(
        meta.get("workflowPath"),
        None | Some(serde_json::Value::Null | serde_json::Value::String(_))
    )
}

/// A slash command backed by an ACP `AvailableCommand`.
pub struct AcpSlashCommand {
    name: String,
    description: String,
    has_args: bool,
    arg_hint: Option<String>,
    skill: SkillMeta,
    /// Whether `_meta` matches a shell-owned, presentation-safe shape.
    trusted_shell_metadata: bool,
    /// Trusted first-party product identity derived from ACP metadata.
    product_chat_skill: bool,
}

impl SlashCommand for AcpSlashCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn provenance(&self) -> CommandProvenance {
        match &self.skill {
            SkillMeta::Skill(identity) => CommandProvenance::Skill {
                source: identity.source().to_string(),
            },
            _ => CommandProvenance::Shell,
        }
    }

    fn usage(&self) -> &str {
        &self.name
    }

    fn takes_args(&self) -> bool {
        self.has_args
    }

    /// ACP commands always accept Enter; args are never required locally.
    /// The shell validates.
    fn args_required(&self) -> bool {
        false
    }

    fn arg_placeholder(&self) -> Option<&str> {
        self.arg_hint.as_deref()
    }

    fn is_skill(&self) -> bool {
        matches!(self.skill, SkillMeta::Skill(_))
    }

    fn has_trusted_shell_metadata(&self) -> bool {
        self.trusted_shell_metadata
    }

    fn is_bundled_skill(&self) -> bool {
        matches!(
            &self.skill,
            SkillMeta::Skill(identity)
                if identity.scope == SkillScope::Bundled
                    && !identity.plugin_name_key_present
        )
    }

    fn is_product_chat_skill(&self) -> bool {
        self.product_chat_skill
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let text = if args.trim().is_empty() {
            format!("/{}", self.name)
        } else {
            format!("/{} {}", self.name, args)
        };
        match self.skill {
            SkillMeta::Malformed => {
                CommandResult::Error(format!("Malformed skill metadata for /{}", self.name))
            }
            SkillMeta::Absent | SkillMeta::Foreign => CommandResult::PassThrough(text),
            SkillMeta::Skill(_) => CommandResult::InjectSkill {
                display_text: text.clone(),
                prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                display_as_skill: true,
                scheduled_task_preview: None,
            },
        }
    }
}

impl From<&acp::AvailableCommand> for AcpSlashCommand {
    fn from(cmd: &acp::AvailableCommand) -> Self {
        let arg_hint = cmd.input.as_ref().and_then(|input| match input {
            acp::AvailableCommandInput::Unstructured(u) => Some(u.hint.clone()),
            // TODO(acp-0.10): `AvailableCommandInput` is #[non_exhaustive].
            _ => None,
        });

        let skill = SkillMeta::parse(cmd.meta.as_ref());
        let trusted_shell_metadata = has_trusted_shell_metadata(cmd.meta.as_ref());
        let has_chat_product_marker = cmd
            .meta
            .as_ref()
            .and_then(|meta| meta.get("product"))
            .and_then(|value| value.as_str())
            == Some("chat");
        let expected_product_path = format!("chat-product://{}", cmd.name);
        let product_chat_skill = has_chat_product_marker
            && matches!(
                &skill,
                SkillMeta::Skill(identity)
                    if identity.scope == SkillScope::Server
                        && !identity.plugin_name_key_present
                        && identity.path == expected_product_path
            );
        Self {
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            // ACP commands always accept free-form input; the shell handles whatever text follows the command name
            // The `input` field only determines the placeholder hint, not whether args are allowed
            has_args: true,
            arg_hint,
            skill,
            trusted_shell_metadata,
            product_chat_skill,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmd(name: &str, meta: Option<serde_json::Value>) -> acp::AvailableCommand {
        let mut cmd = acp::AvailableCommand::new(name.to_string(), format!("{name} command"));
        if let Some(m) = meta.and_then(|v| v.as_object().cloned()) {
            cmd = cmd.meta(m);
        }
        cmd
    }

    fn parse(meta: serde_json::Value) -> SkillMeta {
        SkillMeta::parse(meta.as_object())
    }

    #[test]
    fn no_meta_or_unrelated_meta_is_absent() {
        assert_eq!(SkillMeta::parse(None), SkillMeta::Absent);
        assert_eq!(
            parse(serde_json::json!({"foo": "bar", "baz": 42})),
            SkillMeta::Absent
        );
    }

    #[test]
    fn zh_localization_product_chat_marker_is_preserved_for_display_provenance() {
        let meta = serde_json::json!({
            "scope": "server",
            "path": "chat-product://build-with-ai",
            "product": "chat"
        });
        let cmd = make_cmd("build-with-ai", Some(meta));
        let acp_cmd = AcpSlashCommand::from(&cmd);
        assert!(acp_cmd.is_product_chat_skill());
        assert!(matches!(
            acp_cmd.skill,
            SkillMeta::Skill(SkillIdentity {
                scope: SkillScope::Server,
                ..
            })
        ));
    }

    #[test]
    fn zh_localization_product_chat_display_provenance_rejects_untrusted_metadata() {
        for (label, meta) in [
            (
                "user product skill",
                serde_json::json!({
                    "scope": "user",
                    "path": "chat-product://build-with-ai",
                    "product": "chat"
                }),
            ),
            (
                "plugin spoof",
                serde_json::json!({
                    "scope": "plugin",
                    "path": "/plugins/acme/skills/build-with-ai/SKILL.md",
                    "pluginName": "acme",
                    "product": "chat"
                }),
            ),
            (
                "server path mismatch",
                serde_json::json!({
                    "scope": "server",
                    "path": "chat-product://another-skill",
                    "product": "chat"
                }),
            ),
            (
                "blank plugin marker",
                serde_json::json!({
                    "scope": "server",
                    "path": "chat-product://build-with-ai",
                    "pluginName": "  \t",
                    "product": "chat"
                }),
            ),
            (
                "mistyped plugin marker",
                serde_json::json!({
                    "scope": "server",
                    "path": "chat-product://build-with-ai",
                    "pluginName": 42,
                    "product": "chat"
                }),
            ),
            (
                "null plugin marker",
                serde_json::json!({
                    "scope": "server",
                    "path": "chat-product://build-with-ai",
                    "pluginName": null,
                    "product": "chat"
                }),
            ),
        ] {
            let cmd = make_cmd("build-with-ai", Some(meta));
            assert!(
                !AcpSlashCommand::from(&cmd).is_product_chat_skill(),
                "{label} must not gain first-party display provenance"
            );
        }
    }

    #[test]
    fn zh_localization_bundled_display_provenance_requires_plugin_key_absent() {
        let trusted = make_cmd(
            "design",
            Some(serde_json::json!({
                "scope": "bundled",
                "path": "/grok/bundled/skills/design/SKILL.md"
            })),
        );
        assert!(AcpSlashCommand::from(&trusted).is_bundled_skill());

        for (label, plugin_name) in [
            ("named", serde_json::json!("acme")),
            ("blank", serde_json::json!("  \t")),
            ("number", serde_json::json!(42)),
            ("null", serde_json::Value::Null),
            ("object", serde_json::json!({})),
        ] {
            let mut meta = serde_json::json!({
                "scope": "bundled",
                "path": "/grok/bundled/skills/design/SKILL.md"
            })
            .as_object()
            .cloned()
            .expect("object");
            meta.insert("pluginName".to_string(), plugin_name);
            let command = make_cmd("design", Some(serde_json::Value::Object(meta)));
            assert!(
                !AcpSlashCommand::from(&command).is_bundled_skill(),
                "{label} pluginName key must block bundled display provenance"
            );
        }
    }

    #[test]
    fn zh_localization_shell_display_provenance_requires_plain_skill_metadata() {
        let plain = AcpSlashCommand::from(&make_cmd("goal", None));
        assert!(plain.has_trusted_shell_metadata());

        for (label, meta) in [
            (
                "builtin workflow projection",
                serde_json::json!({"workflowSource": "builtin"}),
            ),
            (
                "saved workflow projection",
                serde_json::json!({
                    "workflowSource": "project",
                    "workflowPath": "/repo/.grok/workflows/review.rhai"
                }),
            ),
            (
                "inline workflow projection",
                serde_json::json!({"workflowSource": "inline", "workflowPath": null}),
            ),
        ] {
            let command = AcpSlashCommand::from(&make_cmd("workflow", Some(meta)));
            assert!(
                command.has_trusted_shell_metadata(),
                "{label} must retain shell display provenance"
            );
        }

        for (label, meta) in [
            (
                "foreign scope",
                serde_json::json!({"scope": "future", "path": "/x"}),
            ),
            (
                "malformed path",
                serde_json::json!({"scope": "local", "path": 42}),
            ),
            ("unknown metadata", serde_json::json!({"foo": "bar"})),
            (
                "workflow metadata with an extra key",
                serde_json::json!({"workflowSource": "user", "foo": "bar"}),
            ),
            (
                "unknown workflow source",
                serde_json::json!({"workflowSource": "remote"}),
            ),
            (
                "mistyped workflow path",
                serde_json::json!({"workflowSource": "user", "workflowPath": 42}),
            ),
        ] {
            let command = AcpSlashCommand::from(&make_cmd("goal", Some(meta)));
            assert!(
                !command.has_trusted_shell_metadata(),
                "{label} must not gain shell display provenance"
            );
        }
    }

    #[test]
    fn valid_skill_meta_parses_identity() {
        let meta = serde_json::json!({
            "scope": "local",
            "path": "/home/user/.grok/skills/commit/SKILL.md",
        });
        assert_eq!(
            parse(meta),
            SkillMeta::Skill(SkillIdentity {
                path: "/home/user/.grok/skills/commit/SKILL.md".to_string(),
                scope: SkillScope::Local,
                plugin_name: None,
                plugin_name_key_present: false,
            })
        );
    }

    #[test]
    fn plugin_meta_carries_plugin_name() {
        let meta = serde_json::json!({
            "scope": "plugin",
            "path": "/plugins/acme/skills/login/SKILL.md",
            "pluginName": "acme",
            "qualifiedName": "acme:login",
        });
        assert_eq!(
            parse(meta),
            SkillMeta::Skill(SkillIdentity {
                path: "/plugins/acme/skills/login/SKILL.md".to_string(),
                scope: SkillScope::Plugin,
                plugin_name: Some("acme".to_string()),
                plugin_name_key_present: true,
            })
        );
    }

    #[test]
    fn unknown_scope_string_is_foreign_not_malformed() {
        let meta = serde_json::json!({
            "scope": "workflow",
            "path": ".grok/workflows/pr-cleanup.rhai",
        });
        assert_eq!(parse(meta), SkillMeta::Foreign);
    }

    #[test]
    fn missing_or_mistyped_skill_keys_are_malformed() {
        for (label, meta) in [
            ("scope without path", serde_json::json!({"scope": "user"})),
            (
                "path without scope",
                serde_json::json!({"path": "/path/to/SKILL.md"}),
            ),
            (
                "path is not a string",
                serde_json::json!({"scope": "local", "path": 42}),
            ),
            (
                "scope is not a string",
                serde_json::json!({"scope": 42, "path": "/path/to/SKILL.md"}),
            ),
        ] {
            assert_eq!(parse(meta), SkillMeta::Malformed, "{label}");
        }
    }

    #[test]
    fn empty_plugin_name_is_dropped() {
        let meta = serde_json::json!({
            "scope": "plugin",
            "path": "/x/SKILL.md",
            "pluginName": "  ",
        });
        match parse(meta) {
            SkillMeta::Skill(identity) => {
                assert_eq!(identity.plugin_name, None);
                assert!(identity.plugin_name_key_present);
            }
            other => panic!("expected Skill, got {other:?}"),
        }
    }

    fn identity(scope: SkillScope, plugin_name: Option<&str>) -> SkillIdentity {
        SkillIdentity {
            path: "/x/SKILL.md".to_string(),
            scope,
            plugin_name: plugin_name.map(str::to_string),
            plugin_name_key_present: plugin_name.is_some(),
        }
    }

    #[test]
    fn source_prefers_plugin_name_over_scope() {
        assert_eq!(identity(SkillScope::Plugin, Some("acme")).source(), "acme");
        assert_eq!(identity(SkillScope::Repo, Some("acme")).source(), "acme");
        assert_eq!(identity(SkillScope::Plugin, None).source(), "plugin");
        assert_eq!(identity(SkillScope::Local, None).source(), "local");
    }

    #[test]
    fn provenance_distinguishes_skills_from_shell_commands() {
        let skill = AcpSlashCommand::from(&make_cmd(
            "login",
            Some(serde_json::json!({
                "scope": "plugin",
                "path": "/plugins/acme/skills/login/SKILL.md",
                "pluginName": "acme",
            })),
        ));
        assert!(skill.is_skill());
        assert_eq!(
            skill.provenance(),
            CommandProvenance::Skill {
                source: "acme".to_string()
            }
        );

        let shell_cmd = AcpSlashCommand::from(&make_cmd("flush", None));
        assert!(!shell_cmd.is_skill());
        assert_eq!(shell_cmd.provenance(), CommandProvenance::Shell);
    }

    fn make_skill_cmd(name: &str, path: &str, scope: SkillScope) -> AcpSlashCommand {
        AcpSlashCommand {
            name: name.to_string(),
            description: format!("{name} skill"),
            has_args: true,
            arg_hint: None,
            skill: SkillMeta::Skill(SkillIdentity {
                path: path.to_string(),
                scope,
                plugin_name: None,
                plugin_name_key_present: false,
            }),
            product_chat_skill: false,
            trusted_shell_metadata: false,
        }
    }

    fn make_exec_ctx() -> CommandExecCtx<'static> {
        use crate::acp::model_state::ModelState;
        let models = Box::leak(Box::new(ModelState::default()));
        let bundle = Box::leak(Box::new(crate::app::bundle::BundleState::default()));
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn run_non_skill_passes_through() {
        let cmd = AcpSlashCommand::from(&make_cmd("flush", None));
        let mut ctx = make_exec_ctx();
        let result = cmd.run(&mut ctx, "");
        assert!(matches!(result, CommandResult::PassThrough(t) if t == "/flush"));
    }

    #[test]
    fn run_foreign_kind_passes_through_with_args() {
        let cmd = AcpSlashCommand::from(&make_cmd(
            "pr-cleanup",
            Some(serde_json::json!({
                "scope": "workflow",
                "path": ".grok/workflows/pr-cleanup.rhai",
            })),
        ));
        let mut ctx = make_exec_ctx();
        match cmd.run(&mut ctx, "fix the branch") {
            CommandResult::PassThrough(text) => assert_eq!(text, "/pr-cleanup fix the branch"),
            other => panic!("expected PassThrough, got {other:?}"),
        }
    }

    #[test]
    fn run_malformed_meta_returns_error() {
        let cmd = AcpSlashCommand::from(&make_cmd(
            "broken",
            Some(serde_json::json!({"scope": "local", "path": 42})),
        ));
        let mut ctx = make_exec_ctx();
        let result = cmd.run(&mut ctx, "");
        assert!(matches!(result, CommandResult::Error(msg) if msg.contains("Malformed")));
    }

    #[test]
    fn run_skill_sends_raw_slash_text_to_shell() {
        for (name, args, expected) in [
            ("commit", "fix the auth bug", "/commit fix the auth bug"),
            ("deploy", "", "/deploy"),
            ("local:compact", "", "/local:compact"),
        ] {
            let cmd = make_skill_cmd(name, "/nonexistent/path/SKILL.md", SkillScope::Local);
            let mut ctx = make_exec_ctx();
            match cmd.run(&mut ctx, args) {
                CommandResult::InjectSkill {
                    display_text,
                    prompt_blocks,
                    ..
                } => {
                    assert_eq!(display_text, expected, "/{name} {args}");
                    let [acp::ContentBlock::Text(block)] = prompt_blocks.as_slice() else {
                        panic!("/{name}: expected a single Text block, got {prompt_blocks:?}");
                    };
                    assert_eq!(block.text, expected, "/{name} {args}");
                    assert!(
                        !block.text.contains('<'),
                        "/{name}: no client-side XML markup: {}",
                        block.text
                    );
                }
                other => panic!("/{name}: expected InjectSkill, got {other:?}"),
            }
        }
    }
}
