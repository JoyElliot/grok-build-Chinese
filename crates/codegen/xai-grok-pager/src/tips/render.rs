use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::render::SafeBuf;
use crate::theme::Theme;

use super::EphemeralTip;

/// Columns between the composer's border and the text of the rows above it.
pub const HINT_INSET: u16 = 1;

/// Compute the number of rows a tip needs when rendered at the given `width`.
pub fn tip_height(width: u16, tip: &str) -> u16 {
    tip_height_with_locale(width, tip, None)
}

/// Locale-aware counterpart used by composition roots. Unknown server-authored
/// tips retain their original body; only stable known tips and the UI prefix
/// are translated.
pub fn tip_height_with_locale(
    width: u16,
    tip: &str,
    locale: Option<&crate::locale::LocaleContext>,
) -> u16 {
    if width == 0 {
        return 0;
    }
    let line = tip_line(tip, locale);
    let line_width = line.width() as u16;
    if line_width <= width {
        1
    } else {
        // Ceiling division: word wrapping may use slightly more rows than a naive character split, but this is a close-enough upper bound
        (line_width as u32)
            .div_ceil(width as u32)
            .min(u16::MAX as u32) as u16
    }
}

fn localized_tip_body<'a>(
    tip: &'a str,
    locale: Option<&crate::locale::LocaleContext>,
) -> std::borrow::Cow<'a, str> {
    let Some(locale) = locale else {
        return std::borrow::Cow::Borrowed(tip);
    };
    if locale.has_display_catalog(xai_grok_locale::dynamic::Domain::Settings) {
        return locale
            .remote_settings_translation("tip", &[], tip)
            .map(std::borrow::Cow::Owned)
            .unwrap_or(std::borrow::Cow::Borrowed(tip));
    }
    let catalog_id = match tip {
        "Use @ to attach files like @src/main.rs." => Some("tips.attach_files"),
        "Use @! for hidden or ignored files: @!.github/workflows." => Some("tips.hidden_files"),
        "Press Ctrl+O to toggle auto-approve mode." => Some("tips.toggle_auto_approve"),
        "Use Shift+Tab to cycle between modes like Plan mode." => Some("tips.cycle_modes"),
        "Run /compact [context] when chat gets long." => Some("tips.compact_context"),
        "Press Ctrl+B to background a running terminal command." => {
            Some("tips.background_terminal")
        }
        "Start Grok in a fresh worktree with `-w`; add `-r <session-id>` to resume an existing session there." => {
            Some("tips.fresh_worktree_resume")
        }
        "Use Ctrl+Enter to interject messages. Or just Enter to queue messages." => {
            Some("tips.interject_queue")
        }
        "Run /dashboard (or Ctrl+\\) to see and manage all your agents in one place." => {
            Some("tips.dashboard")
        }
        "Try out workflows using /workflows." => Some("tips.workflows"),
        "Use Ctrl+O or click [Click here to Upgrade] to subscribe." => {
            Some("tips.upgrade_subscription")
        }
        _ => None,
    };
    if let Some(catalog_id) = catalog_id {
        return locale.named_text(catalog_id, tip);
    }
    std::borrow::Cow::Borrowed(tip)
}

fn tip_line(tip: &str, locale: Option<&crate::locale::LocaleContext>) -> Line<'static> {
    let theme = Theme::current();
    let prefix = locale
        .map(|locale| locale.named_text("tips.prefix", "Tip: "))
        .unwrap_or_else(|| std::borrow::Cow::Borrowed("Tip: "));
    let body = localized_tip_body(tip, locale);
    Line::from(vec![
        Span::styled(
            prefix.into_owned(),
            Style::default().fg(theme.gray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(body.into_owned(), Style::default().fg(theme.gray)),
    ])
}

/// Rows above the composer line up one column inside its border, not out at its edge.
/// Callers paint the full slot and place text here, so the background still covers column zero.
pub fn hint_text_area(area: Rect) -> Rect {
    Rect {
        x: area.x + HINT_INSET,
        width: area.width.saturating_sub(HINT_INSET),
        ..area
    }
}

/// Render a tip into the provided area, word-wrapping if it exceeds the width.
pub fn render_tip(area: Rect, buf: &mut Buffer, tip: &str, inset: u16) {
    render_tip_with_locale(area, buf, tip, inset, None);
}

pub fn render_tip_with_locale(
    area: Rect,
    buf: &mut Buffer,
    tip: &str,
    inset: u16,
    locale: Option<&crate::locale::LocaleContext>,
) {
    if area.height == 0 {
        return;
    }

    let theme = Theme::current();
    let text = Rect {
        x: area.x + inset,
        width: area.width.saturating_sub(inset),
        ..area
    };

    clear_rect(buf, area, theme.bg_base);
    Paragraph::new(tip_line(tip, locale))
        .style(Style::default().bg(theme.bg_base))
        .wrap(Wrap { trim: false })
        .render(text, buf);
}

/// Blank every cell of `area` (chars, colors, and modifiers) in `color`. Modifiers MUST be reset here: ratatui's
/// `Cell::set_style` only *merges* modifiers (`insert(add)` / `remove(sub)`).
fn clear_rect(buf: &mut Buffer, area: Rect, color: Color) {
    for row in 0..area.height {
        for col in 0..area.width {
            if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
                cell.set_char(' ');
                cell.fg = color;
                cell.bg = color;
                cell.modifier = Modifier::empty();
            }
        }
    }
}

/// Render a pre-styled tip line into the banner rect.
/// The whole rect is cleared first; it can be taller than one row when a wrapped session tip reserved it.
/// The line paints on the first row, truncated at width.
pub fn render_ephemeral_tip(area: Rect, buf: &mut Buffer, line: &Line<'static>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let theme = Theme::current();
    clear_rect(buf, area, theme.bg_base);
    let text = hint_text_area(area);
    buf.set_line_safe(text.x, text.y, line, text.width);
}

struct TemplateToken<'a> {
    name: &'static str,
    span: &'a Span<'static>,
}

/// Rebuild a localized tip template while retaining the original shortcut or
/// command spans (content and style). A malformed catalog placeholder falls
/// back to the canonical pre-styled English line instead of dropping text.
fn localized_ephemeral_line(
    tip: &EphemeralTip,
    locale: Option<&crate::locale::LocaleContext>,
) -> Line<'static> {
    let Some(locale) = locale else {
        return tip.line.clone();
    };
    let Some(default_style) = tip.line.spans.first().map(|span| span.style) else {
        return tip.line.clone();
    };

    let token = |name, index| {
        tip.line
            .spans
            .get(index)
            .map(|span| TemplateToken { name, span })
    };
    let (catalog_id, english, tokens): (&str, &str, Vec<TemplateToken<'_>>) = match tip.key {
        super::clear_detector::UNDO_TIP_KEY => {
            let Some(chord) = token("chord", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.undo",
                "Input cleared · {chord} to undo",
                vec![chord],
            )
        }
        super::clipboard_focus::CLIPBOARD_IMAGE_TIP_KEY => {
            let Some(chord) = token("chord", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.clipboard_image",
                "Image in clipboard · {chord} to paste",
                vec![chord],
            )
        }
        super::plan_nudge::PLAN_NUDGE_KEY => {
            let Some(chord) = token("chord", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.plan_mode",
                "Planning? Check out plan mode via {chord}",
                vec![chord],
            )
        }
        super::send_now::SEND_NOW_TIP_KEY => {
            let Some(key) = token("key", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.send_now",
                "Queued · {key} to send now",
                vec![key],
            )
        }
        super::small_screen::SMALL_SCREEN_TIP_KEY => {
            let Some(command) = token("command", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.small_screen",
                "Tight on space? Try {command}",
                vec![command],
            )
        }
        super::ssh_wrap::SSH_WRAP_TIP_KEY => {
            let Some(command) = token("command", 1) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.ssh_wrap",
                "Run {command} for details and fixes.",
                vec![command],
            )
        }
        super::word_select::WORD_SELECT_TIP_KEY => {
            let (Some(settings), Some(chord)) = (token("settings", 1), token("chord", 3)) else {
                return tip.line.clone();
            };
            (
                "tips.ephemeral.word_select",
                "Want double-click to select? {settings} → Text selection · {chord}: enable now",
                vec![settings, chord],
            )
        }
        _ => return tip.line.clone(),
    };

    let template = locale.named_text(catalog_id, english);
    let mut rest = template.as_ref();
    let mut spans = Vec::new();
    let mut used_tokens = Vec::with_capacity(tokens.len());
    while let Some(open) = rest.find('{') {
        let Some(close_rel) = rest[open + 1..].find('}') else {
            return tip.line.clone();
        };
        let close = open + 1 + close_rel;
        if open > 0 {
            spans.push(Span::styled(rest[..open].to_string(), default_style));
        }
        let name = &rest[open + 1..close];
        let Some(token) = tokens.iter().find(|token| token.name == name) else {
            return tip.line.clone();
        };
        if used_tokens.contains(&name) {
            return tip.line.clone();
        }
        used_tokens.push(name);
        spans.push(token.span.clone());
        rest = &rest[close + 1..];
    }
    if used_tokens.len() != tokens.len() {
        return tip.line.clone();
    }
    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_string(), default_style));
    }
    Line::from(spans)
}

/// Render a client-owned contextual hint in the selected locale. Dynamic
/// shortcut and command tokens are preserved from the original styled line.
pub fn render_ephemeral_tip_with_locale(
    area: Rect,
    buf: &mut Buffer,
    tip: &EphemeralTip,
    locale: Option<&crate::locale::LocaleContext>,
) {
    let line = localized_ephemeral_line(tip, locale);
    render_ephemeral_tip(area, buf, &line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_localization_dynamic_tip_keeps_local_occurrence_and_shortcuts_unchanged() {
        use xai_grok_locale::dynamic::{DisplayCatalog, DisplayEntry, Domain};
        let locale = zh_cn_locale();
        let raw = "Try /new for a new session.";
        let entry = DisplayEntry {
            field: "tip".into(),
            context: vec![],
            source: Some(raw.into()),
            source_sha256: None,
            translation: "使用 /new 开始新会话。".into(),
        };
        locale.install_display_catalog(std::sync::Arc::new(
            DisplayCatalog::from_entries(Domain::Settings, vec![entry]).unwrap(),
        ));
        locale.set_remote_tip(Some(raw.into()));
        assert_eq!(
            localized_tip_body(raw, Some(&locale)),
            "使用 /new 开始新会话。"
        );
        locale.set_remote_tip(None);
        assert_eq!(localized_tip_body(raw, Some(&locale)), raw);
    }

    fn zh_cn_locale() -> crate::locale::LocaleContext {
        crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::ZhCn,
            source: crate::locale::LocaleSource::Cli,
        })
    }

    fn row_text(buf: &Buffer, area: Rect, y: u16) -> String {
        (0..area.width)
            .map(|x| buf.cell((area.x + x, y)).expect("cell in area").symbol())
            .collect()
    }

    #[test]
    fn clears_full_rect_and_truncates_to_width() {
        let area = Rect::new(0, 0, 8, 2);
        let mut buf = Buffer::empty(area);
        // Pre-dirty both rows to simulate stale banner content underneath.
        buf.set_string(0, 0, "XXXXXXXX", Style::default());
        buf.set_string(0, 1, "XXXXXXXX", Style::default());

        let line = Line::from("0123456789"); // wider than the rect
        render_ephemeral_tip(area, &mut buf, &line);

        assert_eq!(
            row_text(&buf, area, 0),
            " 0123456",
            "inset by one, truncated at width"
        );
        assert_eq!(
            row_text(&buf, area, 1),
            "        ",
            "stale rows below the line are cleared"
        );
    }

    #[test]
    fn zero_sized_area_is_a_noop() {
        let area = Rect::new(0, 0, 8, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "XXXXXXXX", Style::default());
        render_ephemeral_tip(Rect::new(0, 0, 8, 0), &mut buf, &Line::from("tip"));
        assert_eq!(row_text(&buf, area, 0), "XXXXXXXX", "untouched");
    }

    #[test]
    fn known_attach_files_tip_is_localized_without_rewriting_tokens() {
        let locale = zh_cn_locale();
        let line = tip_line("Use @ to attach files like @src/main.rs.", Some(&locale));
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text, "提示：可用 @ 附加文件，例如 @src/main.rs。");
    }

    #[test]
    fn localization_regression_compact_tip_unknown_remote_tip_is_opaque() {
        let locale = zh_cn_locale();
        let line = tip_line("Run /compact [context] when chat gets long.", Some(&locale));
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text, "提示：对话过长时，可运行 /compact [context]。");

        let unknown = tip_line("Review the current policy.", Some(&locale));
        let unknown_text = unknown
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(unknown_text, "提示：Review the current policy.");
    }

    #[test]
    fn zh_localization_remote_tip_catalog_preserves_shortcuts() {
        let locale = zh_cn_locale();
        let cases = [
            (
                "Use @! for hidden or ignored files: @!.github/workflows.",
                "提示：可用 @! 引用隐藏或被忽略的文件：@!.github/workflows。",
            ),
            (
                "Press Ctrl+O to toggle auto-approve mode.",
                "提示：按 Ctrl+O 切换自动批准模式。",
            ),
            (
                "Use Shift+Tab to cycle between modes like Plan mode.",
                "提示：使用 Shift+Tab 在不同模式之间循环切换，例如计划模式。",
            ),
            (
                "Press Ctrl+B to background a running terminal command.",
                "提示：按 Ctrl+B 将正在运行的终端命令发送到后台。",
            ),
            (
                "Start Grok in a fresh worktree with `-w`; add `-r <session-id>` to resume an existing session there.",
                "提示：使用 `-w` 在全新的工作树中启动 Grok；加上 `-r <session-id>` 可在其中恢复已有会话。",
            ),
            (
                "Use Ctrl+Enter to interject messages. Or just Enter to queue messages.",
                "提示：使用 Ctrl+Enter 插话；也可以直接按 Enter 将消息排入队列。",
            ),
            (
                "Run /dashboard (or Ctrl+\\) to see and manage all your agents in one place.",
                "提示：运行 /dashboard（或按 Ctrl+\\）即可集中查看和管理所有智能体。",
            ),
            (
                "Try out workflows using /workflows.",
                "提示：输入 /workflows 即可体验工作流。",
            ),
            (
                "Use Ctrl+O or click [Click here to Upgrade] to subscribe.",
                "提示：按 Ctrl+O 或点击[点击此处升级]进行订阅。",
            ),
        ];
        for (english, expected) in cases {
            let text = tip_line(english, Some(&locale))
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert_eq!(text, expected);
        }
    }

    #[test]
    fn zh_localization_ephemeral_tip_catalog_preserves_action_tokens() {
        let locale = zh_cn_locale();
        let cases = [
            (
                super::super::clear_detector::undo_tip(),
                "输入已清空 · ctrl+z 可撤销",
                "ctrl+z",
            ),
            (
                super::super::clipboard_focus::clipboard_image_tip(),
                "剪贴板中有图像 · ctrl+v 可粘贴",
                "ctrl+v",
            ),
            (
                super::super::plan_nudge::plan_nudge_tip(),
                "正在规划？可按 shift+tab 进入计划模式",
                "shift+tab",
            ),
            (
                super::super::send_now::send_now_tip(),
                "已排队 · 按 Enter 立即发送",
                "Enter",
            ),
            (
                super::super::small_screen::small_screen_tip(),
                "空间有限？试试 /compact-mode",
                "/compact-mode",
            ),
            (
                super::super::ssh_wrap::ssh_wrap_tip(),
                "运行 /doctor 查看详情和修复建议。",
                "/doctor",
            ),
            (
                super::super::word_select::word_select_tip(),
                "想通过双击选择文本？/settings → 文本选择 · Ctrl+Y：立即启用",
                "Ctrl+Y",
            ),
        ];

        for (tip, expected, token) in cases {
            let line = localized_ephemeral_line(&tip, Some(&locale));
            let text = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert_eq!(text, expected);
            let token_span = line
                .spans
                .iter()
                .find(|span| span.content.as_ref() == token)
                .expect("shortcut or command token remains a distinct span");
            assert!(token_span.style.add_modifier.contains(Modifier::BOLD));
        }

        let english = super::super::send_now::send_now_tip();
        let english_line =
            localized_ephemeral_line(&english, Some(&crate::locale::LocaleContext::default()));
        let english_text = english_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(english_text, "Queued · Enter to send now");
    }

    #[test]
    fn zh_localization_malformed_ephemeral_tip_falls_back_without_panicking() {
        let locale = zh_cn_locale();
        for key in [
            super::super::clear_detector::UNDO_TIP_KEY,
            super::super::word_select::WORD_SELECT_TIP_KEY,
        ] {
            let tip = EphemeralTip::new(key, Line::from("Keep original"));
            let line = localized_ephemeral_line(&tip, Some(&locale));
            assert_eq!(
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>(),
                "Keep original"
            );
        }
    }

    /// Regression: a bold underpaint in the banner rect (e.g. the welcome
    /// tip's `Tip: ` prefix painted the same frame) must not bleed BOLD into
    /// the ephemeral tip. `Cell::set_style` merges modifiers, so the clear
    /// pass has to reset them explicitly — otherwise `Queued · Enter …`
    /// rendered as bold `Queue` + regular `d` (5 leaked bold cells).
    #[test]
    fn clears_leaked_modifiers_from_underpaint() {
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(
            0,
            0,
            "Tip: never gonna give you up",
            Style::default().add_modifier(Modifier::BOLD),
        );

        let prefix = "Status · ";
        let chord = "Enter";
        let dim = Style::default();
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let line = Line::from(vec![
            Span::styled(prefix, dim),
            Span::styled(chord, bold),
            Span::styled(" to continue", dim),
        ]);
        render_ephemeral_tip(area, &mut buf, &line);

        assert_eq!(row_text(&buf, area, 0).trim(), "Status · Enter to continue");
        let bold_cols: Vec<u16> = (0..area.width)
            .filter(|&x| {
                buf.cell((x, 0))
                    .expect("cell in area")
                    .modifier
                    .contains(Modifier::BOLD)
            })
            .collect();
        let chord_start = 1 + prefix.chars().count() as u16;
        assert_eq!(
            bold_cols,
            (chord_start..chord_start + chord.chars().count() as u16).collect::<Vec<u16>>(),
            "only the Enter chord may be bold — no leak from the underpaint"
        );
    }
}
