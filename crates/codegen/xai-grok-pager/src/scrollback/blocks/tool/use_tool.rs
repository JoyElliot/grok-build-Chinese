//! UseToolCallBlock: MCP integration tool dispatch.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span, Text};
use xai_grok_workspace::permission::{MCP_TOOL_NAME_DELIMITER, mcp_titleize_segment};

use crate::appearance::AppearanceConfig;
use crate::render::line_utils::truncate_str;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{
    AccentStyle, BlockBackground, BlockContext, BlockLine, BlockOutput, DisplayMode,
};
use crate::theme::Theme;

const MAX_INLINE_LINES: usize = 10;
const TRUNCATED_INLINE_LINES: usize = 3;

/// Use tool call: dispatching to an MCP integration tool.
#[derive(Debug, Clone)]
pub struct UseToolCallBlock {
    /// The qualified tool name (e.g. "linear__save_issue").
    pub tool_name: String,
    /// Input arguments as key-value pairs (extracted from tool_input JSON).
    pub input_args: Vec<(String, String)>,
    /// Output text from the dispatched tool.
    pub output: Option<String>,
    /// Error message if the tool call failed.
    pub error: Option<String>,
    /// When the tool started running.
    pub started_at: Option<std::time::Instant>,
    /// Elapsed time in ms after completion.
    pub elapsed_ms: Option<i64>,
    /// ACP-only provenance emitted after an actual managed-gateway dispatch.
    pub managed_gateway_tool: Option<xai_grok_tools::types::resources::ManagedGatewayToolIdentity>,
}

impl UseToolCallBlock {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            input_args: Vec::new(),
            output: None,
            error: None,
            started_at: None,
            elapsed_ms: None,
            managed_gateway_tool: None,
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    pub fn set_error(&mut self, error: Option<String>) {
        if self.elapsed_ms.is_none()
            && let Some(start) = self.started_at
        {
            self.elapsed_ms = Some(start.elapsed().as_millis() as i64);
        }
        self.error = error;
    }

    pub fn finish(&mut self) {
        if self.elapsed_ms.is_some() {
            return;
        }
        if let Some(start) = self.started_at {
            self.elapsed_ms = Some(start.elapsed().as_millis() as i64);
        }
    }

    pub fn elapsed_ms(&self) -> Option<i64> {
        self.elapsed_ms.or_else(|| {
            self.started_at
                .map(|start| start.elapsed().as_millis() as i64)
        })
    }

    pub fn copy_text(&self) -> String {
        let mut out = format!("tool: {}\n", self.tool_name);
        for (k, v) in &self.input_args {
            out.push_str(&format!("{k}: {v}\n"));
        }
        out.push('\n');
        out.push_str(self.output.as_deref().unwrap_or("(no output)"));
        out
    }

    /// Split `tool_name` on `MCP_TOOL_NAME_DELIMITER` (validated to be unambiguous) and title-case each segment.
    /// Returns `(server_title, action_title)` for qualified names, or `("", titleized_tool_name)` for unqualified ones.
    /// Unqualified names fall through to a single-span render in `header_line`.
    fn split_name(&self) -> (String, String) {
        match self.tool_name.split_once(MCP_TOOL_NAME_DELIMITER) {
            Some((server, action)) => (mcp_titleize_segment(server), mcp_titleize_segment(action)),
            None => (String::new(), mcp_titleize_segment(&self.tool_name)),
        }
    }

    /// Render the header line: **Server** `Action`
    fn header_line(
        &self,
        theme: &Theme,
        locale: &crate::locale::LocaleContext,
        muted: bool,
        max_width: Option<usize>,
    ) -> Line<'static> {
        let text_style = if muted {
            theme.muted()
        } else {
            theme.primary()
        };
        let bold_style = text_style.add_modifier(Modifier::BOLD);
        let action_style = if muted {
            theme.muted()
        } else {
            theme.fg(theme.command)
        };

        let managed_localized = self.managed_gateway_tool.as_ref().and_then(|identity| {
            crate::views::managed_mcp_localization::localized_verified_managed_mcp_tool_name(
                &self.tool_name,
                identity,
                locale,
            )
        });
        if let Some(localized) = managed_localized
            .or_else(|| super::localized_known_mcp_tool_name(&self.tool_name, locale))
        {
            let display = match max_width {
                Some(w) => truncate_str(localized, w),
                None => localized.to_string(),
            };
            return Line::from(vec![Span::styled(display, bold_style)]);
        }

        let (server, action) = self.split_name();

        if server.is_empty() {
            let display = match max_width {
                Some(w) => truncate_str(&action, w),
                None => action,
            };
            return Line::from(vec![Span::styled(display, bold_style)]);
        }

        let prefix = format!("{server} ");

        match max_width {
            Some(w) => {
                let budget = w.saturating_sub(prefix.len());
                let display_action = truncate_str(&action, budget);
                Line::from(vec![
                    Span::styled(prefix, bold_style),
                    Span::styled(display_action, action_style),
                ])
            }
            None => Line::from(vec![
                Span::styled(prefix, bold_style),
                Span::styled(action, action_style),
            ]),
        }
    }
}

impl BlockContent for UseToolCallBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        let theme = Theme::current();
        let muted_collapsed =
            ctx.mute_when_collapsed(ctx.appearance.scrollback.blocks.tool.muted_collapsed);

        match ctx.mode {
            DisplayMode::Collapsed => BlockOutput {
                lines: vec![
                    self.header_line(
                        &theme,
                        &ctx.locale,
                        muted_collapsed,
                        Some(ctx.content_width()),
                    )
                    .into(),
                ],
            },
            DisplayMode::Truncated | DisplayMode::Expanded => {
                let header = self.header_line(&theme, &ctx.locale, false, None);
                let wrapped = crate::render::wrapping::wrap_header_flush(
                    header,
                    ctx.width as usize,
                    ctx.bullet_indent(),
                );
                let mut lines: Vec<BlockLine> = wrapped.into_iter().map(BlockLine::from).collect();

                // Input arguments
                if !self.input_args.is_empty() {
                    lines.push(Line::from("").into());
                    for (key, val) in &self.input_args {
                        lines.push(BlockLine::styled(Line::from(vec![
                            Span::styled(format!("  {key}: "), theme.muted()),
                            Span::styled(val.clone(), theme.primary()),
                        ])));
                    }
                }

                let max_inline = if ctx.mode == DisplayMode::Truncated {
                    TRUNCATED_INLINE_LINES
                } else {
                    MAX_INLINE_LINES
                };
                if let Some(ref output) = self.output {
                    lines.push(Line::from("").into());
                    lines
                        .push(BlockLine::from(Line::from("")).with_panel_background(theme.bg_dark));

                    let indent = "  ";
                    let content_lines: Vec<&str> = output.lines().collect();

                    for (i, line) in content_lines.iter().enumerate() {
                        if i >= max_inline {
                            let remaining = content_lines.len() - max_inline;
                            let truncated = if ctx.locale.locale() == crate::locale::UiLocale::ZhCn
                            {
                                format!("{indent}…（还有 {remaining} 行，按 Enter 查看）")
                            } else {
                                format!("{indent}... ({remaining} more lines, press Enter to view)",)
                            };
                            lines.push(
                                BlockLine::from(Line::from(Span::styled(truncated, theme.dim())))
                                    .with_panel_background(theme.bg_dark),
                            );
                            break;
                        }
                        lines.push(
                            BlockLine::from(Line::from(Span::styled(
                                format!("{indent}{line}"),
                                theme.primary(),
                            )))
                            .with_panel_background(theme.bg_dark),
                        );
                    }

                    lines
                        .push(BlockLine::from(Line::from("")).with_panel_background(theme.bg_dark));
                }

                if let Some(ref err) = self.error {
                    lines.push(Line::from("").into());
                    lines.push(
                        Line::from(Span::styled(
                            format!("  {err}"),
                            theme.fg(theme.accent_error),
                        ))
                        .into(),
                    );
                }

                BlockOutput { lines }
            }
        }
    }

    fn accent(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        if ctx.mode == DisplayMode::Collapsed {
            return None;
        }
        let theme = Theme::current();
        if self.error.is_some() {
            Some(AccentStyle::static_color(theme.accent_error))
        } else if ctx.is_running {
            Some(AccentStyle::animated(theme.accent_running))
        } else {
            Some(AccentStyle::static_color(theme.accent_tool))
        }
    }

    fn bullet(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        if self.error.is_some() {
            let theme = Theme::current();
            Some(AccentStyle::static_color(theme.accent_error))
        } else if ctx.mode == DisplayMode::Collapsed {
            None
        } else {
            self.accent(ctx)
        }
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false
    }

    fn background(&self, _ctx: &BlockContext) -> BlockBackground {
        BlockBackground::None
    }

    fn has_raw_mode(&self) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        !self.input_args.is_empty() || self.output.is_some() || self.error.is_some()
    }

    fn default_display_mode(&self) -> DisplayMode {
        DisplayMode::Collapsed
    }

    fn next_fold_mode(&self, current: DisplayMode, _is_running: bool) -> DisplayMode {
        match current {
            DisplayMode::Collapsed => DisplayMode::Expanded,
            _ => DisplayMode::Collapsed,
        }
    }

    fn preamble(&self, ctx: &BlockContext) -> Option<Text<'static>> {
        let theme = Theme::current();
        Some(Text::from(vec![self.header_line(
            &theme,
            &ctx.locale,
            false,
            None,
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LocaleContext, LocaleSource, ResolvedLocale, UiLocale};
    use crate::scrollback::types::BlockContext;

    fn ctx_with_locale(mode: DisplayMode, locale: LocaleContext) -> BlockContext {
        BlockContext {
            width: 80,
            mode,
            is_running: false,
            raw: false,
            max_lines: None,
            appearance: Default::default(),
            is_selected: false,
            cwd: None,
            locale,
        }
    }

    fn zh_locale() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    fn rendered_text(block: &UseToolCallBlock, mode: DisplayMode) -> String {
        rendered_text_with_locale(block, mode, LocaleContext::default())
    }

    fn rendered_text_with_locale(
        block: &UseToolCallBlock,
        mode: DisplayMode,
        locale: LocaleContext,
    ) -> String {
        block
            .output(&ctx_with_locale(mode, locale))
            .lines
            .iter()
            .map(|l| {
                l.content
                    .spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn zh_localization_translates_known_tasks_and_voice_titles() {
        let cases = [
            ("tasks__list", "list", "List", "列出自动化"),
            ("tasks__create", "create", "Create", "创建自动化"),
            ("tasks__update", "update", "Update", "更新自动化"),
            (
                "tasks__get_results",
                "get_results",
                "Get Results",
                "获取自动化结果",
            ),
            ("tasks__run_now", "run_now", "Run Now", "立即运行自动化"),
            ("tasks__pause", "pause", "Pause", "暂停自动化"),
            ("tasks__delete", "delete", "Delete", "删除自动化"),
            (
                "tasks__list_trigger_catalog",
                "list_trigger_catalog",
                "List Trigger Catalog",
                "列出触发器目录",
            ),
            (
                "tasks__list_trigger_resources",
                "list_trigger_resources",
                "List Trigger Resources",
                "列出触发器资源",
            ),
            ("tasks__validate", "validate", "Validate", "验证自动化"),
        ];

        for (tool_name, tool_id, display_name, expected) in cases {
            let mut block = UseToolCallBlock::new(tool_name);
            block.managed_gateway_tool = Some(
                xai_grok_tools::types::resources::ManagedGatewayToolIdentity {
                    qualified_name: tool_name.to_owned(),
                    connector_id: "tasks".to_owned(),
                    tool_id: tool_id.to_owned(),
                    display_name: display_name.to_owned(),
                    description_sha256: match tool_id {
                        "create" => {
                            "bd70559a0f8696630e9bf97cb571d50a48cbb487f90fc1b75a9e6e32ebb65570"
                        }
                        "delete" => {
                            "127ff8c35578884847a40616c03d5bdf3b44785b3927f40859f8cddbb82bcbf2"
                        }
                        "get_results" => {
                            "fcde7ab85e189428c7507a7cfbfc68e06a869f6c2e9841cd58a8315fce15dfa4"
                        }
                        "list" => {
                            "d92bdcbd0f8b0a9b2d010d43e72bf3f29b7044d929dcedac4822d91770a292fc"
                        }
                        "list_trigger_catalog" => {
                            "c5291881d5fba7ee86c831d90105a9e78d5394c7d1a10323d1b91a4bb3dd8a14"
                        }
                        "list_trigger_resources" => {
                            "d5b460c32291fdd8a23c041c40aca4e7e2ba451cccd323940c129047b36276cc"
                        }
                        "pause" => {
                            "24aa2383616a0ed4f3a8db305fbd5ccf2731f92dab024b038d4f70e920a92940"
                        }
                        "run_now" => {
                            "cb624d3983f786b70574c451502ea4e008f49ec30fb48d45f44288346e7dc4e5"
                        }
                        "update" => {
                            "49f0dc9572c909d58bcbf64f72f9e00740f606c658251fdc578704b31bba4fae"
                        }
                        "validate" => {
                            "0f6673236d56ecef82e3bd08901c60a78ecb98b217dd5c3d2c7514d8490c364a"
                        }
                        _ => unreachable!("covered task tool"),
                    }
                    .to_owned(),
                },
            );
            assert_eq!(
                rendered_text_with_locale(&block, DisplayMode::Collapsed, zh_locale()),
                expected,
                "tool_name={tool_name}"
            );
        }

        let voice = UseToolCallBlock::new("voice__list_voices");
        assert_eq!(
            rendered_text_with_locale(&voice, DisplayMode::Collapsed, zh_locale()),
            "列出可用语音"
        );
    }

    #[test]
    fn zh_localization_does_not_translate_unverified_managed_name_collisions() {
        let missing_provenance = UseToolCallBlock::new("tasks__list");
        assert_eq!(
            rendered_text_with_locale(&missing_provenance, DisplayMode::Collapsed, zh_locale()),
            "Tasks List"
        );

        let mut spoofed = UseToolCallBlock::new("tasks__list");
        spoofed.managed_gateway_tool = Some(
            xai_grok_tools::types::resources::ManagedGatewayToolIdentity {
                qualified_name: "tasks__list".to_owned(),
                connector_id: "custom".to_owned(),
                tool_id: "list".to_owned(),
                display_name: "List".to_owned(),
                description_sha256:
                    "d92bdcbd0f8b0a9b2d010d43e72bf3f29b7044d929dcedac4822d91770a292fc".to_owned(),
            },
        );
        assert_eq!(
            rendered_text_with_locale(&spoofed, DisplayMode::Collapsed, zh_locale()),
            "Tasks List"
        );
    }

    #[test]
    fn mcp_title_localization_preserves_english_unknown_and_raw_identity() {
        let mut tasks = UseToolCallBlock::new("tasks__list");
        tasks
            .input_args
            .push(("workspace".to_string(), r"C:\repo\API_KEY".to_string()));
        tasks.output = Some("opaque server result".to_string());
        assert_eq!(rendered_text(&tasks, DisplayMode::Collapsed), "Tasks List");
        assert_eq!(
            tasks.copy_text(),
            "tool: tasks__list\nworkspace: C:\\repo\\API_KEY\n\nopaque server result"
        );

        let unknown = UseToolCallBlock::new("linear__list_issues");
        assert_eq!(
            rendered_text_with_locale(&unknown, DisplayMode::Collapsed, zh_locale()),
            "Linear List Issues"
        );
    }

    #[test]
    fn zh_localization_applies_to_fullscreen_preamble() {
        let block = UseToolCallBlock::new("voice__list_voices");
        let text = block
            .preamble(&ctx_with_locale(DisplayMode::Expanded, zh_locale()))
            .expect("preamble")
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text, "列出可用语音");
    }

    #[test]
    fn truncated_caps_inline_output_tighter_than_expanded() {
        let mut block = UseToolCallBlock::new("linear__list_issues");
        let content: Vec<String> = (1..=12).map(|i| format!("l{i:02} row")).collect();
        block.output = Some(content.join("\n"));

        let truncated = rendered_text(&block, DisplayMode::Truncated);
        assert!(truncated.contains("l03"), "truncated:\n{truncated}");
        assert!(!truncated.contains("l04"), "truncated:\n{truncated}");
        assert!(
            truncated.contains("(9 more lines"),
            "truncated:\n{truncated}"
        );

        let expanded = rendered_text(&block, DisplayMode::Expanded);
        assert!(expanded.contains("l10"), "expanded:\n{expanded}");
        assert!(!expanded.contains("l11"), "expanded:\n{expanded}");
        assert!(expanded.contains("(2 more lines"), "expanded:\n{expanded}");
    }
}
