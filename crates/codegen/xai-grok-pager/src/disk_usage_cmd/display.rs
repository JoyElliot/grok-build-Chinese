//! Pure renderer over [`DiskUsageReport`].
//! `xai_grok_config::grok_home()`, whose first call creates the home, must stay out of this module.

use std::io::Write;
use std::path::Path;

use unicode_width::UnicodeWidthStr;
use xai_fast_worktree::WorktreeStatus;

use super::{DiskUsageReport, Registration, RegistryState, WorktreeUsage};
use crate::util::{format_age, format_bytes, pad_to_width, truncate_to_width};

const SIZE_WIDTH: usize = 10;
const AGE_WIDTH: usize = 10;
const TYPE_HEADER: &str = "TYPE";
const LABEL_HEADER: &str = "LABEL";
const LABEL_WIDTH_MAX: usize = 24;

pub fn print_report(
    report: &DiskUsageReport,
    now: i64,
    out: &mut impl Write,
) -> std::io::Result<()> {
    print_report_with_locale(report, now, out, None)
}

pub fn print_report_with_locale(
    report: &DiskUsageReport,
    now: i64,
    out: &mut impl Write,
    locale: Option<&crate::locale::LocaleContext>,
) -> std::io::Result<()> {
    let home_label = home_prefix_label(&report.grok_home);
    writeln!(
        out,
        "{}",
        text(locale, "du.title", "Disk usage for {home}").replace("{home}", &home_label)
    )?;
    for entry in &report.top_level_dirs {
        writeln!(
            out,
            "  {:>SIZE_WIDTH$}  {}",
            size_cell(entry.bytes),
            entry.name
        )?;
    }
    if report.root_files_bytes > 0 {
        writeln!(
            out,
            "  {}  {}",
            pad_left_to_width(&format_bytes(report.root_files_bytes), SIZE_WIDTH),
            text(locale, "du.top_level_files", "(top-level files)")
        )?;
    }
    writeln!(
        out,
        "  {}  {}",
        pad_left_to_width(&format_bytes(report.total_bytes), SIZE_WIDTH),
        text(locale, "du.total", "total")
    )?;
    if report.skips.unreadable_dirs > 0 {
        let count = report.skips.unreadable_dirs;
        let english = if count == 1 {
            "{count} directory could not be read; what is under it may be missing from the total. RUST_LOG=debug names it."
        } else {
            "{count} directories could not be read; what is under them may be missing from the total. RUST_LOG=debug names them."
        };
        writeln!(
            out,
            "  {}",
            text(locale, "du.skip.unreadable", english).replace("{count}", &count.to_string())
        )?;
    }
    if report.skips.unstatable_entries > 0 {
        let count = report.skips.unstatable_entries;
        let english = format!(
            "{{count}} {} could not be read and {} not counted.",
            count_noun(count, "entry", "entries"),
            count_verb(count)
        );
        writeln!(
            out,
            "  {}",
            text(locale, "du.skip.unstatable", &english).replace("{count}", &count.to_string())
        )?;
    }
    if report.skips.other_filesystem_dirs > 0 {
        let count = report.skips.other_filesystem_dirs;
        let english = format!(
            "{{count}} {} on another filesystem and {} not counted, here or in any row.",
            count_noun(count, "directory is", "directories are"),
            count_verb(count)
        );
        writeln!(
            out,
            "  {}",
            text(locale, "du.skip.other_filesystem", &english)
                .replace("{count}", &count.to_string())
        )?;
    }
    if report.unfollowed_dir_symlinks > 0 {
        let count = report.unfollowed_dir_symlinks;
        let english = if count == 1 {
            "{count} top-level symlink to a directory is not followed, so its contents are missing from the total."
        } else {
            "{count} top-level symlinks to directories are not followed, so their contents are missing from the total."
        };
        writeln!(
            out,
            "  {}",
            text(locale, "du.skip.unfollowed_symlink", english)
                .replace("{count}", &count.to_string())
        )?;
    }
    // The proven statement replaces the general note rather than joining it.
    if report.total_exceeds_volume_used() {
        writeln!(
            out,
            "  {}",
            text(
                locale,
                "du.volume.shared_blocks",
                "Total exceeds the used space on this volume, so shared blocks are counted once per path."
            )
        )?;
    } else if cfg!(unix) && !report.worktrees.is_empty() {
        writeln!(
            out,
            "  {}",
            text(
                locale,
                "du.volume.clone_shared_storage",
                "Worktree clones share storage with their source, so the total can exceed real disk use."
            )
        )?;
    }

    writeln!(out)?;
    writeln!(out, "{}", text(locale, "du.worktrees.title", "Worktrees"))?;
    if report.worktrees_outside_managed_roots > 0 {
        let count = report.worktrees_outside_managed_roots;
        let english = format!(
            "{{count}} {} outside the managed worktree dirs {} not shown here.",
            count_noun(count, "worktree", "worktrees"),
            count_verb(count)
        );
        writeln!(
            out,
            "  {}",
            text(locale, "du.worktrees.outside_managed", &english)
                .replace("{count}", &count.to_string())
        )?;
    }
    match report.registry {
        RegistryState::Read => {}
        RegistryState::Absent => {
            if !report.worktrees.is_empty() {
                writeln!(
                    out,
                    "  {}",
                    text(
                        locale,
                        "du.registry.absent",
                        "Worktree registry not found; rows may show as untracked."
                    )
                )?;
            }
        }
        RegistryState::Busy => {
            writeln!(
                out,
                "  {}",
                text(
                    locale,
                    "du.registry.busy",
                    "Worktree registry is in use by another process; rows show as untracked. Retry in a moment."
                )
            )?;
        }
        RegistryState::Unopenable => {
            let path = abbreviate(&report.registry_path, &report.grok_home, &home_label);
            writeln!(
                out,
                "  {}",
                text(
                    locale,
                    "du.registry.unopenable",
                    "Worktree registry at {path} could not be opened; rows show as untracked. Check its permissions."
                )
                .replace("{path}", &path)
            )?;
        }
        RegistryState::Corrupt => {
            let path = abbreviate(&report.registry_path, &report.grok_home, &home_label);
            writeln!(
                out,
                "  {}",
                text(
                    locale,
                    "du.registry.corrupt",
                    "Worktree registry is damaged; rows show as untracked. Remove {path} and run `grok worktree db rebuild` to recreate it."
                )
                .replace("{path}", &path)
            )?;
        }
    }
    if report.worktrees.is_empty() {
        writeln!(
            out,
            "  {}",
            text(locale, "du.worktrees.empty", "No worktrees found.")
        )?;
    } else {
        let size_header = text(locale, "du.columns.size", "SIZE");
        let type_header = text(locale, "du.columns.type", TYPE_HEADER);
        let age_header = text(locale, "du.columns.age", "AGE");
        let label_header = text(locale, "du.columns.label", LABEL_HEADER);
        let path_header = text(locale, "du.columns.path", "PATH");
        let kind_cells: Vec<String> = report
            .worktrees
            .iter()
            .map(|wt| kind_cell(wt, locale))
            .collect();
        let kind_width = kind_cells
            .iter()
            .map(|k| UnicodeWidthStr::width(k.as_str()))
            .fold(UnicodeWidthStr::width(type_header.as_str()), usize::max);
        let label_width = report
            .worktrees
            .iter()
            .map(|w| UnicodeWidthStr::width(w.label()))
            .fold(UnicodeWidthStr::width(label_header.as_str()), usize::max)
            .min(LABEL_WIDTH_MAX);
        writeln!(
            out,
            "  {}  {} {} {} {}",
            pad_left_to_width(&size_header, SIZE_WIDTH),
            pad_to_width(&type_header, kind_width),
            pad_to_width(&age_header, AGE_WIDTH),
            pad_to_width(&label_header, label_width),
            path_header,
        )?;
        for (wt, kind) in report.worktrees.iter().zip(&kind_cells) {
            let age = wt.age_stamp().map_or_else(
                || "-".to_owned(),
                |ts| format_age_for_locale(ts, now, locale),
            );
            let label = truncate_to_width(wt.label(), label_width);
            writeln!(
                out,
                "  {}  {} {} {} {}",
                pad_left_to_width(&size_cell(wt.bytes), SIZE_WIDTH),
                pad_to_width(kind, kind_width),
                pad_to_width(&age, AGE_WIDTH),
                pad_to_width(&label, label_width),
                abbreviate(&wt.path, &report.grok_home, &home_label),
            )?;
        }
    }

    // gc's age pass needs `--max-age` and walks registry records, so neither half of the hint holds for both row kinds
    if report.worktrees_dominate() && !report.worktrees.is_empty() {
        writeln!(out)?;
        if report.worktrees.iter().any(WorktreeUsage::is_tracked) {
            writeln!(
                out,
                "{}",
                text(
                    locale,
                    "du.hint.reclaim_tracked",
                    "To reclaim space, run `grok-zh worktree gc --max-age 7d --dry-run`, then the same command without `--dry-run`. Without `--max-age`, gc expires nothing, and it keeps a worktree whose work it cannot find elsewhere, naming each one."
                )
            )?;
        }
        if !report.worktrees.iter().all(WorktreeUsage::is_tracked) {
            writeln!(
                out,
                "{}",
                text(
                    locale,
                    "du.hint.untracked",
                    "Untracked rows are not in the registry, so gc never visits them. Remove one with `grok-zh worktree rm --dry-run <path>`, then without `--dry-run`."
                )
            )?;
        }
    }
    Ok(())
}

pub(crate) fn print_redirect_rows(
    report: &DiskUsageReport,
    out: &mut impl Write,
) -> std::io::Result<()> {
    if report.redirections_bytes == 0
        && report.orphaned_redirections.is_empty()
        && report.unattributed_redirect_dirs.is_empty()
    {
        return Ok(());
    }
    writeln!(out)?;
    writeln!(out, "Redirections")?;
    writeln!(
        out,
        "  {:>SIZE_WIDTH$}  Redirections",
        format_bytes(report.redirections_bytes)
    )?;
    let orphaned_bytes = report
        .orphaned_redirections
        .iter()
        .map(|row| row.bytes)
        .fold(0, u64::saturating_add);
    if !report.orphaned_redirections.is_empty() {
        writeln!(
            out,
            "  {:>SIZE_WIDTH$}  Orphaned redirections",
            format_bytes(orphaned_bytes)
        )?;
        for row in &report.orphaned_redirections {
            writeln!(
                out,
                "  {:>SIZE_WIDTH$}  {}",
                format_bytes(row.bytes),
                row.id
            )?;
        }
    }
    let unattributed_bytes = report
        .unattributed_redirect_dirs
        .iter()
        .map(|row| row.bytes)
        .fold(0, u64::saturating_add);
    if !report.unattributed_redirect_dirs.is_empty() {
        writeln!(
            out,
            "  {:>SIZE_WIDTH$}  Unattributed redirect directories",
            format_bytes(unattributed_bytes)
        )?;
        for row in &report.unattributed_redirect_dirs {
            writeln!(
                out,
                "  {:>SIZE_WIDTH$}  {}",
                format_bytes(row.bytes),
                row.path
            )?;
        }
    }
    Ok(())
}

pub fn print_missing_home(grok_home: &str, out: &mut impl Write) -> std::io::Result<()> {
    print_missing_home_with_locale(grok_home, out, None)
}

pub fn print_missing_home_with_locale(
    grok_home: &str,
    out: &mut impl Write,
    locale: Option<&crate::locale::LocaleContext>,
) -> std::io::Result<()> {
    let home = home_prefix_label(grok_home);
    writeln!(
        out,
        "{}",
        text(locale, "du.empty_home", "Nothing on disk yet at {home}.").replace("{home}", &home)
    )
}

pub(crate) fn print_missing_home_report(
    report: &DiskUsageReport,
    out: &mut impl Write,
) -> std::io::Result<()> {
    print_missing_home(&report.grok_home, out)?;
    print_redirect_rows(report, out)?;
    print_skip_notes(report, out)
}

fn print_skip_notes(report: &DiskUsageReport, out: &mut impl Write) -> std::io::Result<()> {
    if report.skips.unreadable_dirs > 0 {
        let (noun, pronoun) = if report.skips.unreadable_dirs == 1 {
            ("directory", "it")
        } else {
            ("directories", "them")
        };
        writeln!(
            out,
            "  {} {noun} could not be read; what is under {pronoun} may be missing from the total. RUST_LOG=debug names {pronoun}.",
            report.skips.unreadable_dirs,
        )?;
    }
    if report.skips.unstatable_entries > 0 {
        writeln!(
            out,
            "  {} {} could not be read and {} not counted.",
            report.skips.unstatable_entries,
            count_noun(report.skips.unstatable_entries, "entry", "entries"),
            count_verb(report.skips.unstatable_entries)
        )?;
    }
    if report.skips.other_filesystem_dirs > 0 {
        writeln!(
            out,
            "  {} {} on another filesystem and {} not counted, here or in any row.",
            report.skips.other_filesystem_dirs,
            count_noun(
                report.skips.other_filesystem_dirs,
                "directory is",
                "directories are"
            ),
            count_verb(report.skips.other_filesystem_dirs),
        )?;
    }
    if report.unfollowed_dir_symlinks > 0 {
        let (noun, pronoun) = if report.unfollowed_dir_symlinks == 1 {
            ("symlink to a directory is", "its")
        } else {
            ("symlinks to directories are", "their")
        };
        writeln!(
            out,
            "  {} top-level {noun} not followed, so {pronoun} contents are missing from the total.",
            report.unfollowed_dir_symlinks,
        )?;
    }
    Ok(())
}

/// A dash where nothing was measured, which is not zero bytes.
fn size_cell(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "-".to_owned(), format_bytes)
}

fn count_noun(n: u64, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

fn count_verb(n: u64) -> &'static str {
    if n == 1 { "is" } else { "are" }
}

fn kind_cell(wt: &WorktreeUsage, locale: Option<&crate::locale::LocaleContext>) -> String {
    let kind = localized_kind(wt.kind.as_ref(), locale);
    match &wt.registration {
        Registration::Untracked => {
            text(locale, "du.kind.untracked", "untracked ({kind})").replace("{kind}", &kind)
        }
        Registration::Tracked(rec) => match rec.status {
            WorktreeStatus::Dead => {
                text(locale, "du.kind.dead", "{kind} (dead)").replace("{kind}", &kind)
            }
            WorktreeStatus::Alive => kind,
        },
    }
}

fn localized_kind(kind: &str, locale: Option<&crate::locale::LocaleContext>) -> String {
    let id = match kind {
        "session" => "du.kind.session",
        "ab" => "du.kind.ab",
        "pool" => "du.kind.pool",
        "fork" => "du.kind.fork",
        "manual" => "du.kind.manual",
        "subagent" => "du.kind.subagent",
        _ => return kind.to_string(),
    };
    text(locale, id, kind)
}

fn format_age_for_locale(
    created_at: i64,
    now: i64,
    locale: Option<&crate::locale::LocaleContext>,
) -> String {
    let Some(locale) = locale else {
        return format_age(created_at, now);
    };
    let delta = now.saturating_sub(created_at).max(0);
    let (id, count, english) = if delta < 60 {
        ("du.age.seconds", delta, "{count}s ago")
    } else if delta < 3600 {
        ("du.age.minutes", delta / 60, "{count}m ago")
    } else if delta < 86400 {
        ("du.age.hours", delta / 3600, "{count}h ago")
    } else {
        ("du.age.days", delta / 86400, "{count}d ago")
    };
    locale
        .named_text(id, english)
        .replace("{count}", &count.to_string())
}

fn text(locale: Option<&crate::locale::LocaleContext>, id: &str, english: &str) -> String {
    locale
        .map(|locale| locale.named_text(id, english).into_owned())
        .unwrap_or_else(|| english.to_string())
}

fn pad_left_to_width(s: &str, width: usize) -> String {
    let current = UnicodeWidthStr::width(s);
    format!("{}{}", " ".repeat(width.saturating_sub(current)), s)
}

fn abbreviate(path: &str, home: &str, label: &str) -> String {
    match Path::new(path).strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => label.to_owned(),
        Ok(rest) => format!("{label}/{}", rest.display()),
        Err(_) => path.to_owned(),
    }
}

fn home_prefix_label(grok_home: &str) -> String {
    crate::util::display_grok_home_prefix_for(Path::new(grok_home))
}
