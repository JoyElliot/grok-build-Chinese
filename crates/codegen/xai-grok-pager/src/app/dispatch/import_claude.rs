use crate::app::actions::Effect;
use crate::app::app_view::AppView;

fn import_text(locale: &crate::locale::LocaleContext, id: &str, english: &str) -> String {
    locale.named_text(id, english).into_owned()
}

fn localize_import_summary(locale: &crate::locale::LocaleContext, summary: &str) -> String {
    if locale.locale() != crate::locale::UiLocale::ZhCn {
        return summary.to_string();
    }

    summary
        .split_inclusive('\n')
        .map(|line| {
            let (body, newline) = line
                .strip_suffix('\n')
                .map_or((line, ""), |body| (body, "\n"));
            let localized = if body == "Found Claude settings to import:" {
                import_text(
                    locale,
                    "import.summary.found",
                    "Found Claude settings to import:",
                )
            } else if let Some(rest) = body.strip_prefix("Global (") {
                format!(
                    "{} ({rest}",
                    locale.named_static_text("import.scope.global", "Global")
                )
            } else if let Some(rest) = body.strip_prefix("Project (") {
                format!(
                    "{} ({rest}",
                    locale.named_static_text("import.scope.project", "Project")
                )
            } else if body.starts_with("  - ") {
                body.replace(
                    "permission rule(s)",
                    locale
                        .named_static_text("import.summary.permission_rules", "permission rule(s)"),
                )
                .replace(
                    "environment variable(s)",
                    locale.named_static_text(
                        "import.summary.environment_variables",
                        "environment variable(s)",
                    ),
                )
                .replace(
                    "MCP server(s)",
                    locale.named_static_text("import.summary.mcp_servers", "MCP server(s)"),
                )
                .replace(
                    "hook(s)",
                    locale.named_static_text("import.summary.hooks", "hook(s)"),
                )
                .replace(
                    "extra skill dir(s)",
                    locale
                        .named_static_text("import.summary.extra_skill_dirs", "extra skill dir(s)"),
                )
                .replace(
                    "extra rule dir(s)",
                    locale.named_static_text("import.summary.extra_rule_dirs", "extra rule dir(s)"),
                )
            } else if body.starts_with("      ") {
                body.replace(
                    "<redacted, ",
                    locale.named_static_text("import.summary.redacted_prefix", "<redacted, "),
                )
                .replace(
                    " chars>",
                    locale.named_static_text("import.summary.redacted_suffix", " chars>"),
                )
                .replace(
                    " (timeout: ",
                    locale.named_static_text("import.summary.timeout_prefix", " (timeout: "),
                )
            } else {
                body.to_string()
            };
            format!("{localized}{newline}")
        })
        .collect()
}

fn is_claude_import_warning(message: &str) -> bool {
    message.contains("Claude settings") || message.contains("Claude 设置")
}

/// Open the interactive Claude-import modal on the welcome screen.
///
/// When the scan finds nothing importable, records the dismissal and shows an info warning instead of the modal.
pub(super) fn dispatch_import_claude(app: &mut AppView) -> Vec<Effect> {
    let cwd = app.cwd.clone();
    let plan = xai_grok_shell::claude_import::scan_importable_settings(&cwd);

    if plan.is_empty() {
        xai_grok_shell::claude_import_state::mark_dismissed(&cwd);
        // Always write the [claude_compat] imported = true marker so the user's opt-in is recorded even on an empty plan
        if let Err(e) = xai_grok_shell::claude_import::mark_claude_imported() {
            tracing::warn!(error = %e, "Failed to write Claude import marker");
        }
        app.has_claude_import = false;
        app.startup_warnings
            .retain(|w| !is_claude_import_warning(&w.message));
        app.startup_warnings.push(crate::startup::StartupWarning {
            severity: crate::startup::WarningSeverity::Info,
            message: import_text(
                app.locale.as_ref(),
                "import.status.none_found",
                "No Claude settings found to import.",
            ),
            action: None,
        });
        return vec![];
    }

    app.import_claude_modal =
        Some(crate::views::import_claude_modal::ImportClaudeModalState::new(plan, cwd));
    vec![]
}

/// Apply the user's selection from the import modal and close it.
pub(super) fn dispatch_import_claude_confirm(app: &mut AppView) -> Vec<Effect> {
    let Some(modal) = app.import_claude_modal.take() else {
        return vec![];
    };
    let cwd = modal.cwd.clone();
    let total_in_modal = modal.total_count();
    let filtered = modal.filtered_plan();
    let selected_count = filtered.global_items.len() + filtered.project_items.len();

    let mut summary = if selected_count == 0 {
        import_text(
            app.locale.as_ref(),
            "import.status.none_selected",
            "No items selected.",
        )
    } else {
        localize_import_summary(app.locale.as_ref(), filtered.summary(&cwd).trim_end())
    };

    if selected_count > 0 {
        match xai_grok_external_agent_migration::apply_import(&filtered, &cwd) {
            Ok(result) => {
                let imported = import_text(
                    app.locale.as_ref(),
                    "import.status.imported",
                    "Imported {imported} of {total} setting(s).",
                )
                .replace("{imported}", &result.total().to_string())
                .replace("{total}", &total_in_modal.to_string());
                summary.push('\n');
                summary.push_str(&imported);
                for path in &result.modified_files {
                    let updated = import_text(
                        app.locale.as_ref(),
                        "import.status.updated",
                        "  Updated: {path}",
                    )
                    .replace("{path}", path);
                    summary.push('\n');
                    summary.push_str(&updated);
                }
            }
            Err(e) => {
                let message = import_text(
                    app.locale.as_ref(),
                    "import.status.failed",
                    "Failed to import Claude settings: {error}",
                )
                .replace("{error}", &e.to_string());
                app.startup_warnings.push(crate::startup::StartupWarning {
                    severity: crate::startup::WarningSeverity::Warning,
                    message,
                    action: None,
                });
                return vec![];
            }
        }
    }

    // Mark the current Claude state as seen so the startup warning won't re-fire for the same content
    // Skipped items remain importable via re-running the slash command
    xai_grok_shell::claude_import_state::mark_imported(&cwd);
    if let Err(e) = xai_grok_shell::claude_import::mark_claude_imported() {
        tracing::warn!(error = %e, "Failed to write Claude import marker");
    }
    app.has_claude_import = false;
    app.startup_warnings
        .retain(|w| !is_claude_import_warning(&w.message));
    app.startup_warnings.push(crate::startup::StartupWarning {
        severity: crate::startup::WarningSeverity::Info,
        message: summary,
        action: None,
    });
    vec![]
}

pub(super) fn dispatch_import_claude_cancel(app: &mut AppView) -> Vec<Effect> {
    app.import_claude_modal = None;
    vec![]
}

/// Hide the Claude-import menu row by recording the current `.claude/` content hash.
/// The startup detection compares the saved hash on the next launch; if it matches (no new Claude content), the menu stays hidden.
pub(super) fn dispatch_dismiss_claude_import(app: &mut AppView) -> Vec<Effect> {
    let cwd = app.cwd.clone();
    xai_grok_shell::claude_import_state::mark_dismissed(&cwd);
    // The imported = true marker also stops the runtime fallbacks (perms, env, MCP servers, hooks, plugins) reading .claude/ and ~/.claude.json
    // Dismiss means "I've decided I want nothing from .claude/", so don't keep silently reading it at runtime
    if let Err(e) = xai_grok_shell::claude_import::mark_claude_imported() {
        tracing::warn!(error = %e, "Failed to write Claude import marker on dismiss");
    }
    app.has_claude_import = false;
    // Reset the welcome menu selection: removing a row shifts indices
    // A stale selection (e.g. `Worktree mode` highlighted at index 1) would now point to a different row.
    app.welcome_menu_index = None;
    app.startup_warnings
        .retain(|w| !is_claude_import_warning(&w.message));
    vec![]
}
