//! Live-work chips collapse before dashboard titles truncate; right-hand controls stay reserved.
//! Chips use the dock's gray text, with no brackets: `Subagents 2 · Tasks 3`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::locale::LocaleContext;
use crate::render::line_utils::truncate_str;
use crate::theme::Theme;
use crate::views::dashboard::row::{DashboardRow, NEW_SESSION_LABEL, RowBadge};

pub(crate) struct RowTitle<'a> {
    pub row: &'a DashboardRow,
    pub theme: &'a Theme,
    pub bg: Color,
}

impl RowTitle<'_> {
    pub(crate) fn render_wide(&self, buf: &mut Buffer, area: Rect) -> u16 {
        self.render_wide_with_locale(buf, area, None)
    }

    pub(crate) fn render_wide_with_locale(
        &self,
        buf: &mut Buffer,
        area: Rect,
        locale: Option<&LocaleContext>,
    ) -> u16 {
        let new_session_label = super::row::new_session_label(locale);
        let failed_label = localized_chip(locale, "dashboard.row.failed", "failed");
        let failed_suffix_width = " · ".width() + failed_label.width();
        let subtitle_width = self
            .row
            .subtitle
            .as_deref()
            .map_or(0, |sub| 3 + sub.width());
        let failed_width = if self.row.badges.contains(&RowBadge::Failed) {
            failed_suffix_width
        } else {
            0
        };
        let text_width = self.row.label.width() + subtitle_width + failed_width;
        let (area, chip_w) = self.reserve_chips(buf, area, text_width, locale);
        let RowTitle { row, theme, bg } = *self;
        let mut cx = area.x;
        if area.width > 0 {
            let label_style = if row.is_more_placeholder {
                theme.dim().bg(bg)
            } else {
                Style::default().bg(bg).fg(theme.text_primary)
            };
            // The # distinguishes the fallback from user titles beginning with "New session".
            let dim_suffix = (!row.is_more_placeholder)
                .then(|| row.label.strip_prefix(new_session_label))
                .flatten()
                .filter(|rest| rest.starts_with(" #"));
            if let Some(suffix) = dim_suffix {
                let head = truncate_str(new_session_label, usize::from(area.width));
                cx = buf
                    .set_stringn(cx, area.y, head, usize::from(area.width), label_style)
                    .0;
                let remaining = area.right().saturating_sub(cx);
                if remaining > 0 {
                    let suffix = truncate_str(suffix, usize::from(remaining));
                    cx = buf
                        .set_stringn(
                            cx,
                            area.y,
                            suffix,
                            usize::from(remaining),
                            theme.dim().bg(bg),
                        )
                        .0;
                }
            } else {
                let label = truncate_str(&row.label, usize::from(area.width));
                cx = buf
                    .set_stringn(cx, area.y, label, usize::from(area.width), label_style)
                    .0;
            }
            if let Some(sub) = row.subtitle.as_deref()
                && area.right().saturating_sub(cx) > 2
            {
                let subtitle = format!(" · {sub}");
                let remaining = usize::from(area.right().saturating_sub(cx));
                let subtitle = truncate_str(&subtitle, remaining);
                cx = buf
                    .set_stringn(cx, area.y, subtitle, remaining, theme.dim().bg(bg))
                    .0;
            }
            if row.badges.contains(&RowBadge::Failed)
                && area.right().saturating_sub(cx) >= failed_suffix_width as u16
            {
                paint_failed_with_locale(buf, cx, area.y, theme, bg, locale);
            }
        }
        chip_w
    }

    pub(crate) fn render_narrow(&self, buf: &mut Buffer, area: Rect) -> u16 {
        self.render_narrow_with_locale(buf, area, None)
    }

    pub(crate) fn render_narrow_with_locale(
        &self,
        buf: &mut Buffer,
        area: Rect,
        locale: Option<&LocaleContext>,
    ) -> u16 {
        let (area, chip_w) = self.reserve_chips(buf, area, self.row.label.width(), locale);
        let label = truncate_str(&self.row.label, usize::from(area.width));
        buf.set_stringn(
            area.x,
            area.y,
            label,
            usize::from(area.width),
            Style::default().fg(self.theme.text_primary).bg(self.bg),
        );
        chip_w
    }

    fn reserve_chips(
        &self,
        buf: &mut Buffer,
        area: Rect,
        text_width: usize,
        locale: Option<&LocaleContext>,
    ) -> (Rect, u16) {
        let available = usize::from(area.width);
        let budget = available.saturating_sub(text_width.saturating_add(1));
        let chips = self.fit_chips_with_locale(budget, available, locale).style(
            Style::default()
                .fg(self.theme.gray)
                .bg(self.bg)
                .remove_modifier(Modifier::BOLD),
        );
        let width = chips.width() as u16;
        if width == 0 || area.height == 0 {
            return (area, 0);
        }
        buf.set_line(area.right().saturating_sub(width), area.y, &chips, width);
        (
            Rect {
                width: area.width.saturating_sub(width + 1),
                ..area
            },
            width,
        )
    }

    fn format_chips(
        &self,
        counts: &[usize; 4],
        labels: ChipLabels,
        locale: Option<&LocaleContext>,
    ) -> Line<'static> {
        let name_style = Style::default()
            .fg(self.theme.gray_bright)
            .add_modifier(Modifier::BOLD);
        let mut spans = Vec::with_capacity(counts.iter().filter(|count| **count > 0).count() * 3);
        for (count, (full, short, short_plural)) in counts
            .iter()
            .zip(CHIP_KINDS)
            .filter(|(count, _)| **count > 0)
        {
            if !spans.is_empty() {
                spans.push(Span::raw(CHIP_SEP));
            }
            let name = match labels {
                ChipLabels::Full => full,
                ChipLabels::Short if *count == 1 => short,
                ChipLabels::Short => short_plural,
            };
            let id = match (full, labels) {
                ("Subagents", ChipLabels::Full) => "dashboard.chip.subagents",
                ("Subagents", ChipLabels::Short) => "dashboard.chip.subagents_short",
                ("Tasks", ChipLabels::Full) => "dashboard.chip.tasks",
                ("Tasks", ChipLabels::Short) => "dashboard.chip.tasks_short",
                ("Watchers", ChipLabels::Full) => "dashboard.chip.watchers",
                ("Watchers", ChipLabels::Short) => "dashboard.chip.watchers_short",
                ("Workflows", ChipLabels::Full) => "dashboard.chip.workflows",
                ("Workflows", ChipLabels::Short) => "dashboard.chip.workflows_short",
                _ => "dashboard.chip.background",
            };
            let name = localized_chip(locale, id, name);
            spans.push(Span::styled(name, name_style));
            spans.push(Span::raw(format!(" {count}")));
        }
        Line::from(spans)
    }

    fn fit_chips(&self, budget: usize, available: usize) -> Line<'static> {
        self.fit_chips_with_locale(budget, available, None)
    }

    fn fit_chips_with_locale(
        &self,
        budget: usize,
        available: usize,
        locale: Option<&LocaleContext>,
    ) -> Line<'static> {
        let mut fallback = Line::default();
        let mut choose = |chips: Line<'static>| {
            let width = chips.width();
            if width <= budget {
                Some(chips)
            } else {
                if width <= available {
                    fallback = chips;
                }
                None
            }
        };
        let mut counts = [0; 4];
        {
            let [subagents, tasks, watchers, workflows] = &mut counts;
            for badge in &self.row.badges {
                match badge {
                    RowBadge::Subagents(count) => *subagents = *count,
                    RowBadge::Tasks(count) => *tasks = *count,
                    RowBadge::Watchers(count) => *watchers = *count,
                    RowBadge::Workflows(count) => *workflows = *count,
                    RowBadge::Worktree
                    | RowBadge::NeedsInput
                    | RowBadge::Pinned
                    | RowBadge::Failed => {}
                }
            }
        }
        let total: usize = counts.iter().sum();
        for labels in [ChipLabels::Full, ChipLabels::Short] {
            if let Some(chips) = choose(self.format_chips(&counts, labels, locale)) {
                return chips;
            }
        }
        if total > 0 {
            let bg = localized_chip(locale, "dashboard.chip.background", "bg");
            if let Some(chips) = choose(Line::raw(format!("{total} {bg}"))) {
                return chips;
            }
            if let Some(chips) = choose(Line::raw(bg)) {
                return chips;
            }
        }
        fallback
    }
}

#[derive(Clone, Copy)]
enum ChipLabels {
    Full,
    Short,
}

const CHIP_SEP: &str = " · ";
const FAILED_LABEL: &str = " · failed";

pub(crate) fn has_counted_live_work(badges: &[RowBadge]) -> bool {
    badges.iter().any(|badge| match badge {
        RowBadge::Subagents(count)
        | RowBadge::Tasks(count)
        | RowBadge::Watchers(count)
        | RowBadge::Workflows(count) => *count > 0,
        RowBadge::Worktree | RowBadge::NeedsInput | RowBadge::Pinned | RowBadge::Failed => false,
    })
}

fn localized_chip(locale: Option<&LocaleContext>, id: &str, english: &'static str) -> &'static str {
    locale
        .map(|l| l.named_static_text(id, english))
        .unwrap_or(english)
}

fn paint_failed_with_locale(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    theme: &Theme,
    bg: Color,
    locale: Option<&LocaleContext>,
) {
    let sep = " · ";
    let cx = buf
        .set_stringn(x, y, sep, sep.width(), theme.dim().bg(bg))
        .0;
    buf.set_string(
        cx,
        y,
        localized_chip(locale, "dashboard.row.failed", "failed"),
        Style::default().bg(bg).fg(theme.accent_error),
    );
}

const CHIP_KINDS: [(&str, &str, &str); 4] = [
    ("Subagents", "Sub", "Subs"),
    ("Tasks", "Task", "Tasks"),
    ("Watchers", "Watch", "Watch"),
    ("Workflows", "Flow", "Flows"),
];

#[cfg(test)]
#[path = "row_title_tests.rs"]
mod tests;
