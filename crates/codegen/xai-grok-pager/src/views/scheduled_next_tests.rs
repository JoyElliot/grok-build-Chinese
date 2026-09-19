use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use pretty_assertions::assert_eq;

use super::suffix;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 10, 0, 0, 0).unwrap()
}

#[test]
fn rfc_future_is_next_in() {
    assert_eq!(
        " (next in 2m5s)",
        suffix(
            "every 30 minutes",
            Duration::ZERO,
            Some("2026-09-10T00:02:05Z"),
            now(),
        )
    );
}

#[test]
fn rfc_past_is_due_now() {
    assert_eq!(
        " (due now)",
        suffix(
            "every 30 minutes",
            Duration::ZERO,
            Some("2026-09-09T23:59:00Z"),
            now(),
        )
    );
}

#[test]
fn rfc_wins_over_elapsed_fallback() {
    assert_eq!(
        " (next in 2m5s)",
        suffix(
            "every 30 minutes",
            Duration::from_secs(10_000),
            Some("2026-09-10T00:02:05Z"),
            now(),
        )
    );
}

#[test]
fn bad_rfc_falls_back_to_created_interval() {
    assert_eq!(
        " (next in 28m30s)",
        suffix(
            "every 30 minutes",
            Duration::from_secs(90),
            Some("not-a-date"),
            now(),
        )
    );
}

#[test]
fn interval_without_rfc_counts_down() {
    assert_eq!(
        " (next in 28m30s)",
        suffix("every 30 minutes", Duration::from_secs(90), None, now())
    );
}

#[test]
fn elapsed_past_interval_is_due_now() {
    assert_eq!(
        " (due now)",
        suffix("every 30 minutes", Duration::from_secs(1_800), None, now())
    );
}

#[test]
fn unknown_schedule_without_rfc_is_empty() {
    assert_eq!("", suffix("soon", Duration::ZERO, None, now()));
}

#[test]
fn zh_localization_schedule_suffix_keeps_the_same_clocks_and_duration() {
    let locale = crate::locale::LocaleContext::new(crate::locale::ResolvedLocale {
        locale: crate::locale::UiLocale::ZhCn,
        source: crate::locale::LocaleSource::Cli,
    });
    assert_eq!(
        "（2m5s 后运行）",
        super::suffix_with_locale(
            "every 30 minutes",
            Duration::from_secs(10_000),
            Some("2026-09-10T00:02:05Z"),
            now(),
            Some(&locale),
        )
        .trim(),
    );
    assert_eq!(
        "（现在到期）",
        super::suffix_with_locale(
            "every 30 minutes",
            Duration::from_secs(1_800),
            None,
            now(),
            Some(&locale)
        )
        .trim(),
    );
    assert_eq!(
        "",
        super::suffix_with_locale("soon", Duration::ZERO, None, now(), Some(&locale)),
    );
}
