//! A success-only receipt bridges the updater child (whose output may be hidden)
//! and the foreground terminal. Release Markdown is display data, never a prompt.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use serde::{Deserialize, Serialize};

const RECEIPT: &str = ".grok-zh-update-result.json";
const MAX_BODY_BYTES: usize = 128 * 1024;
const MAX_RECEIPT_BYTES: u64 = 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
struct CompletedUpdate {
    version: String,
    body: String,
}

fn receipt_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Renaming the running EXE to .old does not change its parent directory.
        Some(std::env::current_exe().ok()?.parent()?.join(RECEIPT))
    }
    #[cfg(not(windows))]
    {
        Some(xai_dirs::resolve_grok_home()?.join("bin").join(RECEIPT))
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Keep authored notes, including compatibility warnings, links and examples.
/// The generated web-only footer has a separate Release link in terminal output.
fn changelog_body(body: &str) -> &str {
    let mut fence = None;
    let mut details_start = None;
    let mut offset = 0;
    let mut notes = body;
    for line in body.split_inclusive('\n') {
        let text = line.trim();
        let marker = text.as_bytes().first().copied();
        if matches!(marker, Some(b'`' | b'~')) {
            let marker = marker.unwrap();
            let count = text.bytes().take_while(|&byte| byte == marker).count();
            if count >= 3 {
                match fence {
                    None => fence = Some((marker, count)),
                    Some((open, width))
                        if marker == open && count >= width && text[count..].trim().is_empty() =>
                    {
                        fence = None;
                    }
                    _ => {}
                }
            }
        }
        if fence.is_none() {
            if let Some(start) = details_start
                && line.trim_end() == "<summary>下载与安装</summary>"
            {
                notes = &body[..start];
                break;
            }
            if !text.is_empty() {
                details_start = (line.trim_end() == "<details>").then_some(offset);
            }
        }
        offset += line.len();
    }
    let notes = notes.trim_end();
    let last_line = notes.rfind('\n').map_or(0, |index| index + 1);
    let footer = &notes[last_line..];
    if fence.is_none()
        && (footer.starts_with("[完整变更](") || footer.starts_with("[上游完整变更（"))
    {
        notes[..last_line].trim_end()
    } else {
        notes
    }
}

/// Never emit terminal controls from remote text. Bound the display/cache
/// without splitting UTF-8, including when reading a receipt from an older build.
fn display_body(body: &str) -> String {
    let normalized = body.replace("\r\n", "\n");
    let mut output = String::new();
    for character in changelog_body(&normalized).chars() {
        if character.is_control() && !matches!(character, '\n' | '\t') {
            continue;
        }
        if output.len() + character.len_utf8() > MAX_BODY_BYTES {
            output.push_str("\n\n（正文较长，完整内容见下方 Release 链接。）");
            break;
        }
        output.push(character);
    }
    output.trim().to_string()
}

fn save(path: &Path, version: &str, body: Option<&str>) -> Result<()> {
    let parsed = semver::Version::parse(version)?;
    anyhow::ensure!(parsed.to_string() == version && parsed.build.is_empty());
    let receipt = CompletedUpdate {
        version: version.to_string(),
        body: display_body(body.unwrap_or_default()),
    };
    let temporary = temporary_path(path);
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(&receipt)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

/// Call only after package verification and activation have both succeeded.
pub(crate) fn record_success(version: &str, body: Option<&str>) {
    if let Some(path) = receipt_path()
        && let Err(error) = save(&path, version, body)
    {
        tracing::warn!("could not save community update notes: {error:#}");
    }
}

fn read(path: &Path, expected_version: Option<&str>) -> Result<Option<String>> {
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(metadata.is_file() && !metadata.file_type().is_symlink());
    anyhow::ensure!(metadata.len() <= MAX_RECEIPT_BYTES);
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_RECEIPT_BYTES);
    let receipt: CompletedUpdate = serde_json::from_slice(&bytes)?;
    let version = semver::Version::parse(&receipt.version)?;
    anyhow::ensure!(version.to_string() == receipt.version && version.build.is_empty());
    if expected_version.is_some_and(|expected| expected != receipt.version) {
        return Ok(None);
    }
    let body = display_body(&receipt.body);
    let tag =
        if version.pre.is_empty() && (version.major, version.minor, version.patch) <= (1, 0, 8) {
            format!("v{version}")
        } else {
            format!("release-v{version}")
        };
    let url = format!("{}/tag/{tag}", xai_grok_product::COMMUNITY_RELEASES_URL);
    let body = if body.is_empty() {
        "此版本未提供更新日志，详情见 Release 页面。".to_string()
    } else {
        body
    };
    Ok(Some(format!(
        "\n更新日志 · v{version}\n\n{body}\n\nRelease：{url}\n"
    )))
}

/// Used only in human-facing success/exit paths, after restoring the terminal.
/// Background children retain the receipt for the foreground process to show.
pub fn print_pending(expected_version: Option<&str>) {
    let Some(path) = receipt_path() else { return };
    let mut output = std::io::stderr().lock();
    let _ = display_pending(&path, expected_version, &mut output);
}

fn display_pending(path: &Path, expected: Option<&str>, output: &mut impl Write) -> Result<bool> {
    // Atomic claim: another terminal cannot consume this receipt, and a new
    // install may publish its own receipt without our later deletion losing it.
    let claimed = temporary_path(path);
    if let Err(error) = std::fs::rename(path, &claimed) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(false);
        }
        return Err(error.into());
    }
    let result = match read(&claimed, expected) {
        Ok(Some(notes)) => output
            .write_all(notes.as_bytes())
            .and_then(|_| output.flush())
            .map(|_| true)
            .map_err(Into::into),
        Ok(None) => Ok(false),
        Err(error) => Err(error),
    };
    if !matches!(result, Ok(true)) {
        // Restore only into an empty slot. hard_link never overwrites a newer
        // receipt, unlike a check-then-rename sequence.
        let _ = std::fs::hard_link(&claimed, path);
    }
    let _ = std::fs::remove_file(claimed);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_receipt_preserves_changelog_and_target_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECEIPT);
        let body = "- 中文重点\n\n## 上游更新\n\n- 修复问题";
        save(&path, "1.0.35", Some(body)).unwrap();
        assert!(read(&path, Some("1.0.36")).unwrap().is_none());
        let text = read(&path, Some("1.0.35")).unwrap().unwrap();
        assert!(text.contains(body));
        assert!(text.contains("/releases/tag/release-v1.0.35"));
        save(&path, "1.0.36", Some("下一版")).unwrap();
        assert!(!read(&path, None).unwrap().unwrap().contains(body));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn display_keeps_notes_without_the_release_page_footer() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/release-notes-display.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let body = case["body"].as_str().unwrap();
            let expected = case["expected"].as_str().unwrap();
            for body in [body.to_string(), body.replace('\n', "\r\n")] {
                assert_eq!(display_body(&body), expected, "{}", case["name"]);
                assert_eq!(display_body(&display_body(&body)), expected);
            }
        }
    }

    #[test]
    fn legacy_receipt_filters_the_footer_before_display() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECEIPT);
        let body = "- 修复问题\n\n[完整变更](https://example.com/compare)\n\n<details>\n<summary>下载与安装</summary>\n```powershell\nInvoke-WebRequest ...\n```\n</details>";
        // Older clients saved the complete Release body. Do not rely on save()
        // having filtered it when consuming an existing success receipt.
        std::fs::write(
            &path,
            serde_json::to_vec(&CompletedUpdate {
                version: "1.0.35".to_string(),
                body: body.to_string(),
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        assert!(display_pending(&path, Some("1.0.35"), &mut output).unwrap());
        let text = String::from_utf8(output).unwrap();
        assert_eq!(
            text,
            "\n更新日志 · v1.0.35\n\n- 修复问题\n\nRelease：https://github.com/JoyElliot/grok-build-Chinese/releases/tag/release-v1.0.35\n"
        );
        assert!(!path.exists());
    }

    #[test]
    fn missing_body_has_release_link_and_invalid_receipts_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECEIPT);
        save(&path, "1.0.8", None).unwrap();
        let text = read(&path, None).unwrap().unwrap();
        assert!(text.contains("未提供更新日志"));
        assert!(text.contains("/tag/v1.0.8"));
        std::fs::write(&path, br#"{"version":"bad","body":"wrong"}"#).unwrap();
        assert!(read(&path, None).is_err());
        assert!(read(&dir.path().join("absent"), None).is_err());
    }

    #[test]
    fn notes_do_not_emit_terminal_controls_or_split_chinese_text() {
        let clean = display_body("中文\r\n\x1b[2J\x07\r继续\u{009b}31m");
        assert!(clean.starts_with("中文\n"));
        assert!(clean.chars().all(|c| !c.is_control() || c == '\n'));
        let long = display_body(&"中".repeat(MAX_BODY_BYTES));
        assert!(long.len() < MAX_BODY_BYTES + 150);
        assert!(long.ends_with("Release 链接。）"));
    }

    #[test]
    fn receipt_is_consumed_once_and_version_mismatch_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECEIPT);
        save(&path, "1.0.35", Some("中文正文")).unwrap();
        let mut output = Vec::new();
        assert!(!display_pending(&path, Some("1.0.36"), &mut output).unwrap());
        assert!(output.is_empty());
        assert!(path.exists());
        assert!(display_pending(&path, Some("1.0.35"), &mut output).unwrap());
        let written = output.len();
        assert!(!display_pending(&path, None, &mut output).unwrap());
        assert_eq!(written, output.len());
    }

    #[test]
    fn consuming_old_receipt_does_not_delete_a_concurrent_install_receipt() {
        struct PublishDuringWrite<'a>(&'a Path);
        impl Write for PublishDuringWrite<'_> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                save(self.0, "1.0.36", Some("new release")).unwrap();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECEIPT);
        save(&path, "1.0.35", Some("old release")).unwrap();
        assert!(display_pending(&path, None, &mut PublishDuringWrite(&path)).unwrap());
        assert!(
            read(&path, Some("1.0.36"))
                .unwrap()
                .unwrap()
                .contains("new release")
        );
    }
}
