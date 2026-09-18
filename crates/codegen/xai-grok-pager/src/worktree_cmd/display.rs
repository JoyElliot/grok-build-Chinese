use super::{DbStats, GcReport, RebuildReport};
use crate::fs_size::{Volume, physical_dir_size};
use crate::locale::LocaleContext;
use crate::util::{format_bytes, pad_to_width, truncate_to_width, unix_now};
use std::io::Write;
use std::path::Path;
use unicode_width::UnicodeWidthStr;
use xai_fast_worktree::WorktreeRecord;

const REPO_WIDTH: usize = 6;
const BRANCH_WIDTH: usize = 20;
const AGE_WIDTH: usize = 10;
const GC_LABEL_WIDTH: usize = 26;
/// Truncate-then-pad to exactly `width` display columns; headers and data
/// share it so the two stay aligned.
fn cell(s: &str, width: usize) -> String {
    pad_to_width(&truncate_to_width(s, width), width)
}
fn localized_named(
    locale: &LocaleContext,
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

fn kind_label(kind: &str, locale: &LocaleContext) -> String {
    let id = match kind {
        "session" => "du.kind.session",
        "ab" => "du.kind.ab",
        "pool" => "du.kind.pool",
        "fork" => "du.kind.fork",
        "manual" => "du.kind.manual",
        "subagent" => "du.kind.subagent",
        _ => return kind.to_string(),
    };
    locale.named_text(id, kind).into_owned()
}

fn status_label(status: &str, locale: &LocaleContext) -> String {
    match status {
        "alive" => locale
            .named_text("worktree.status.alive", "alive")
            .into_owned(),
        "dead" => locale
            .named_text("worktree.status.dead", "dead")
            .into_owned(),
        _ => status.to_string(),
    }
}

fn format_age_with_locale(created_at: i64, now: i64, locale: &LocaleContext) -> String {
    let delta = now.saturating_sub(created_at).max(0);
    let (id, count, suffix) = if delta < 60 {
        ("du.age.seconds", delta, "s ago")
    } else if delta < 3600 {
        ("du.age.minutes", delta / 60, "m ago")
    } else if delta < 86400 {
        ("du.age.hours", delta / 3600, "h ago")
    } else {
        ("du.age.days", delta / 86400, "d ago")
    };
    let count = count.to_string();
    localized_named(
        locale,
        id,
        &format!("{{count}}{suffix}"),
        &[("count", &count)],
    )
}
pub fn print_table(records: &[WorktreeRecord], out: &mut impl Write) -> std::io::Result<()> {
    print_table_with_locale(records, out, &LocaleContext::default())
}

pub fn print_table_with_locale(
    records: &[WorktreeRecord],
    out: &mut impl Write,
    locale: &LocaleContext,
) -> std::io::Result<()> {
    if records.is_empty() {
        writeln!(
            out,
            "{}",
            locale.named_text("worktree.empty", "No worktrees found.")
        )?;
        return Ok(());
    }
    let id_header = locale.named_text("worktree.column.id", "ID");
    let type_header = locale.named_text("worktree.column.type", "TYPE");
    let repo_header = locale.named_text("worktree.column.repo", "REPO");
    let label_header = locale.named_text("worktree.column.label", "LABEL");
    let branch_header = locale.named_text("worktree.column.branch", "BRANCH");
    let age_header = locale.named_text("worktree.column.age", "AGE");
    let path_header = locale.named_text("worktree.column.path", "PATH");
    let id_width = records
        .iter()
        .map(|r| UnicodeWidthStr::width(r.id.as_str()))
        .max()
        .unwrap_or(0)
        .max(16);
    let label_width = records
        .iter()
        .map(|r| r.label().map_or(0, UnicodeWidthStr::width))
        .max()
        .unwrap_or(0)
        .clamp(5, 24);
    let type_width = records
        .iter()
        .map(|r| UnicodeWidthStr::width(kind_label(r.kind.as_ref(), locale).as_str()))
        .fold(UnicodeWidthStr::width(type_header.as_ref()), usize::max);
    writeln!(
        out,
        "  {} {} {} {} {} {} {}",
        pad_to_width(id_header.as_ref(), id_width),
        cell(type_header.as_ref(), type_width),
        cell(repo_header.as_ref(), REPO_WIDTH),
        cell(label_header.as_ref(), label_width),
        cell(branch_header.as_ref(), BRANCH_WIDTH),
        pad_to_width(age_header.as_ref(), AGE_WIDTH),
        path_header,
    )?;
    let now = unix_now();
    for rec in records {
        let age = format_age_with_locale(rec.created_at, now, locale);
        let detached = locale.named_text("worktree.detached", "(detached)");
        let branch = rec.git_ref.as_deref().unwrap_or(detached.as_ref());
        let label = rec.label().unwrap_or("");
        let path = abbreviate_home(&rec.path);
        let kind = kind_label(rec.kind.as_ref(), locale);
        // AGE is ASCII, so format-width padding is width-true; every other
        // cell pads by display width.
        writeln!(
            out,
            "  {} {} {} {} {} {:<AGE_WIDTH$} {}",
            pad_to_width(&rec.id, id_width),
            cell(&kind, type_width),
            cell(&rec.repo_name, REPO_WIDTH),
            cell(label, label_width),
            cell(branch, BRANCH_WIDTH),
            age,
            path,
        )?;
    }
    let total = records.len();
    let by_kind: std::collections::BTreeMap<&str, usize> =
        records
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut m, r| {
                *m.entry(r.kind.as_ref()).or_default() += 1;
                m
            });
    let breakdown: Vec<String> = by_kind
        .iter()
        .map(|(kind, count)| {
            let count = count.to_string();
            let kind = kind_label(kind, locale);
            localized_named(
                locale,
                "worktree.summary.kind",
                "{count} {kind}",
                &[("count", &count), ("kind", &kind)],
            )
        })
        .collect();
    let total = total.to_string();
    writeln!(
        out,
        "  {}",
        localized_named(
            locale,
            "worktree.summary",
            "{count} worktrees ({breakdown})",
            &[("count", &total), ("breakdown", &breakdown.join(", "))],
        )
    )
}
pub fn print_json(records: &[WorktreeRecord], out: &mut impl Write) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(records).unwrap_or_else(|_| "[]".to_string());
    writeln!(out, "{json}")
}
pub fn print_show(rec: &WorktreeRecord, out: &mut impl Write) -> std::io::Result<()> {
    print_show_with_locale(rec, out, &LocaleContext::default())
}

fn write_show_field(
    out: &mut impl Write,
    locale: &LocaleContext,
    id: &str,
    english: &str,
    value: impl std::fmt::Display,
) -> std::io::Result<()> {
    let label = format!("{}:", locale.named_text(id, english));
    writeln!(out, "  {} {value}", pad_to_width(&label, 16))
}

pub fn print_show_with_locale(
    rec: &WorktreeRecord,
    out: &mut impl Write,
    locale: &LocaleContext,
) -> std::io::Result<()> {
    write_show_field(
        out,
        locale,
        "worktree.show.path",
        "Path",
        rec.path.display(),
    )?;
    write_show_field(out, locale, "worktree.show.id", "ID", &rec.id)?;
    write_show_field(
        out,
        locale,
        "worktree.show.type",
        "Type",
        kind_label(rec.kind.as_ref(), locale),
    )?;
    write_show_field(
        out,
        locale,
        "worktree.show.source_repo",
        "Source Repo",
        rec.source_repo.display(),
    )?;
    write_show_field(
        out,
        locale,
        "worktree.show.creation_mode",
        "Creation Mode",
        &rec.creation_mode,
    )?;
    if let Some(ref git_ref) = rec.git_ref {
        write_show_field(out, locale, "worktree.show.git_ref", "Git Ref", git_ref)?;
    }
    if let Some(ref commit) = rec.head_commit {
        let short = commit.get(..12).unwrap_or(commit);
        write_show_field(out, locale, "worktree.show.head", "HEAD", short)?;
    }
    write_show_field(
        out,
        locale,
        "worktree.show.created",
        "Created",
        format_timestamp(rec.created_at),
    )?;
    if let Some(ts) = rec.last_accessed_at {
        write_show_field(
            out,
            locale,
            "worktree.show.last_accessed",
            "Last Accessed",
            format_timestamp(ts),
        )?;
    }
    if let Some(ref sid) = rec.session_id {
        write_show_field(out, locale, "worktree.show.session_id", "Session ID", sid)?;
    }
    if let Some(pid) = rec.creator_pid {
        write_show_field(out, locale, "worktree.show.creator_pid", "Creator PID", pid)?;
    }
    write_show_field(
        out,
        locale,
        "worktree.show.status",
        "Status",
        status_label(rec.status.as_ref(), locale),
    )?;
    if let Some(label) = rec.label() {
        write_show_field(out, locale, "worktree.show.label", "Label", label)?;
    }
    if rec.path.exists() {
        let size = physical_dir_size(&rec.path, Volume::of(&rec.path));
        let bytes = size.measure.bytes().unwrap_or_default();
        let label = format!(
            "{}:",
            locale.named_text("worktree.show.disk_usage", "Disk Usage")
        );
        write!(
            out,
            "  {} {}",
            pad_to_width(&label, 16),
            format_bytes(bytes)
        )?;
        let skipped = size.issues.skipped();
        if skipped > 0 {
            let skipped = skipped.to_string();
            write!(
                out,
                " {}",
                localized_named(
                    locale,
                    "worktree.disk_usage.skipped",
                    "({count} entries skipped)",
                    &[("count", &skipped)],
                )
            )?;
        }
        writeln!(out)?;
    }
    Ok(())
}
pub fn print_stats(stats: &DbStats, out: &mut impl Write) -> std::io::Result<()> {
    print_stats_with_locale(stats, out, &LocaleContext::default())
}

pub fn print_stats_with_locale(
    stats: &DbStats,
    out: &mut impl Write,
    locale: &LocaleContext,
) -> std::io::Result<()> {
    let title = locale.named_text("worktree.stats.title", "Worktree DB Statistics");
    writeln!(out, "{title}")?;
    writeln!(
        out,
        "{}",
        "=".repeat(UnicodeWidthStr::width(title.as_ref()))
    )?;
    write_show_field(
        out,
        locale,
        "worktree.stats.total_records",
        "Total records",
        stats.total_records,
    )?;
    write_show_field(
        out,
        locale,
        "worktree.stats.alive",
        "Alive",
        stats.alive_count,
    )?;
    write_show_field(out, locale, "worktree.stats.dead", "Dead", stats.dead_count)?;
    write_show_field(
        out,
        locale,
        "worktree.stats.db_size",
        "DB size",
        format_bytes(stats.db_file_bytes),
    )
}
pub fn print_gc(report: &GcReport, out: &mut impl Write) -> std::io::Result<()> {
    print_gc_with_locale(report, out, &LocaleContext::default())
}

fn write_gc_field(
    out: &mut impl Write,
    locale: &LocaleContext,
    id: &str,
    english: &str,
    value: impl std::fmt::Display,
) -> std::io::Result<()> {
    let label = format!("{}:", locale.named_text(id, english));
    writeln!(out, "  {} {value}", pad_to_width(&label, GC_LABEL_WIDTH))
}

pub fn print_gc_with_locale(
    report: &GcReport,
    out: &mut impl Write,
    locale: &LocaleContext,
) -> std::io::Result<()> {
    writeln!(
        out,
        "{}",
        locale.named_text("worktree.gc.title", "GC report:")
    )?;
    write_gc_field(
        out,
        locale,
        "worktree.gc.dead_removed",
        "Dead records removed",
        report.dead_removed,
    )?;
    write_gc_field(
        out,
        locale,
        "worktree.gc.expired_removed",
        "Expired worktrees removed",
        report.expired_removed,
    )?;
    if report.no_repo_paths > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.no_repo_paths",
            "Non-repository paths",
            report.no_repo_paths,
        )?;
    }
    write_gc_field(
        out,
        locale,
        "worktree.gc.skipped_alive",
        "Skipped (guarded)",
        report.skipped_alive,
    )?;
    if report.never_expiring > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.never_expiring",
            "Kept (never expires)",
            report.never_expiring,
        )?;
    }
    if report.kept_unsafe > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.kept_unsafe",
            "Kept (not reclaimable)",
            report.kept_unsafe,
        )?;
        for (reason, count) in &report.kept_reasons {
            let count = count.to_string();
            writeln!(
                out,
                "{}",
                localized_named(
                    locale,
                    "worktree.gc.kept_reason",
                    "    {reason}: {count}",
                    &[("reason", reason), ("count", &count)],
                )
            )?;
        }
        // The report carries only the first hundred entries; every retained
        // worktree is also named in the log for explicit follow-up.
        const MAX_KEPT_PRINTED: usize = 20;
        let printed = report.kept.len().min(MAX_KEPT_PRINTED);
        for kept in report.kept.iter().take(MAX_KEPT_PRINTED) {
            writeln!(
                out,
                "{}",
                localized_named(
                    locale,
                    "worktree.gc.kept_path",
                    "      {path}  ({reason})",
                    &[("path", &kept.path), ("reason", &kept.reason)],
                )
            )?;
        }
        let rest = usize::try_from(report.kept_unsafe)
            .unwrap_or(usize::MAX)
            .saturating_sub(printed);
        if rest > 0 {
            let rest = rest.to_string();
            writeln!(
                out,
                "{}",
                localized_named(
                    locale,
                    "worktree.gc.kept_more",
                    "      and {count} more, named in the log",
                    &[("count", &rest)],
                )
            )?;
        }
    }
    if report.names_collected > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.names_collected",
            "Reclaimed names dropped",
            report.names_collected,
        )?;
    }
    if report.not_judged > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.not_judged",
            "Not judged this pass",
            report.not_judged,
        )?;
    }
    if report.unnamed > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.unnamed",
            "Naming failed (kept)",
            report.unnamed,
        )?;
    }
    if report.remove_failed > 0 {
        write_gc_field(
            out,
            locale,
            "worktree.gc.remove_failed",
            "Removal failures",
            report.remove_failed,
        )?;
    }
    Ok(())
}
pub fn print_rebuild(report: &RebuildReport, out: &mut impl Write) -> std::io::Result<()> {
    print_rebuild_with_locale(report, out, &LocaleContext::default())
}

pub fn print_rebuild_with_locale(
    report: &RebuildReport,
    out: &mut impl Write,
    locale: &LocaleContext,
) -> std::io::Result<()> {
    writeln!(
        out,
        "{}",
        locale.named_text("worktree.rebuild.title", "Rebuild report:")
    )?;
    write_show_field(
        out,
        locale,
        "worktree.rebuild.discovered",
        "Discovered",
        report.discovered,
    )?;
    write_show_field(
        out,
        locale,
        "worktree.rebuild.registered",
        "Registered",
        report.registered,
    )?;
    write_show_field(
        out,
        locale,
        "worktree.rebuild.already_tracked",
        "Already tracked",
        report.already_tracked,
    )
}
fn format_timestamp(ts: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts, 0);
    match dt {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => ts.to_string(),
    }
}
fn abbreviate_home(path: &Path) -> String {
    crate::util::abbreviate_path(&path.to_string_lossy()).into_owned()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn print_table_never_truncates_long_ids() {
        let long_id = "a".repeat(40);
        let mut out = Vec::new();
        print_table(&[make_record(&long_id, "lbl")], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(&long_id), "full ID must be present: {text}");
    }
    fn make_record(id: &str, label: &str) -> WorktreeRecord {
        crate::test_util::make_worktree_record(
            id,
            std::path::Path::new(&format!("/tmp/wt-{id}")),
            label,
        )
    }
    #[test]
    fn print_show_non_nfs_omits_nfs_block() {
        let rec = make_record("wt-copy", "c");
        let mut out = Vec::new();
        print_show(&rec, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("Strategy:       nfs"), "{text}");
        assert!(!text.contains("clean-artifacts"), "{text}");
    }
    #[test]
    fn print_table_pads_cjk_labels_by_display_width() {
        let records = vec![
            make_record("wt-cjk", "组件更新"),
            make_record("wt-ascii", "plain-label"),
        ];
        let mut out = Vec::new();
        print_table(&records, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("组件更新"));
        crate::test_util::assert_path_column_aligned(&text, "/tmp/wt-");
    }
}
