// Generated unchanged historical function bodies plus explicitly marked test seams.
use super::*;

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L1716-L1732
/// Unique temp path `<base>.{pid}-{seq}.{ext}`, appended to the full name.
/// A versioned base like `grok-0.1.181` would collide via `with_extension`, which treats everything after the last dot as the extension.
/// The PID and a per-process counter keep racing updaters from clobbering each other.
fn unique_temp_sibling(base: &std::path::Path, ext: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let mut name = base
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(
        ".{}-{}.{ext}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    base.with_file_name(name)
}

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L2156-L2273
/// First-launch of a freshly downloaded macOS binary can exceed 10s (Rosetta AOT and Gatekeeper on ~140MB).
/// A short cap would fail a good artifact.
pub(crate) const SMOKE_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Retry budget for exec attempts refused with ETXTBSY. The failure window is normally the microseconds another spawn in
/// this process sits between fork and exec (see [`probe_version`]). On a heavily loaded machine that window can
/// stretch, so the budget errs generous. A false "failed to run" both aborts this install and deletes the binary.
const SMOKE_TEST_ETXTBSY_ATTEMPTS: u32 = 8;
const SMOKE_TEST_ETXTBSY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(25);

fn truncate_err(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.saturating_sub(3);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", s.get(..end).unwrap_or(""))
}

#[derive(Debug, thiserror::Error)]
enum SmokeTestFailure {
    #[error(
        "downloaded binary failed to run (--version timed out after {}s).\n\
         Your current version is unchanged.",
        SMOKE_TEST_TIMEOUT.as_secs()
    )]
    Timeout,
    #[error(
        "downloaded binary failed to run (could not start: {0}).\n\
         Your current version is unchanged."
    )]
    Spawn(String),
    #[error("{}", nonzero_message(status, stderr))]
    NonZero { status: String, stderr: String },
}

/// `--version` exited nonzero; include stderr only when there is any.
fn nonzero_message(status: &str, stderr: &str) -> String {
    let stderr_line = if stderr.is_empty() {
        String::new()
    } else {
        format!("stderr: {stderr}\n")
    };
    format!(
        "downloaded binary failed to run (--version exited {status}).\n\
         {stderr_line}Your current version is unchanged."
    )
}

/// How `<binary> --version` failed to produce an exit status.
pub(crate) enum VersionProbeError {
    Timeout,
    Spawn(String),
}

/// Runs a fresh download's `--version`, detached, within [`SMOKE_TEST_TIMEOUT`]. Shared by the stock
/// updater and the private distributions, which judge the output differently.
pub(crate) async fn probe_version(
    binary_path: &std::path::Path,
) -> Result<std::process::Output, VersionProbeError> {
    // ETXTBSY race: a concurrent updater's child in this process briefly holds every open fd between fork and exec pre_exec
    // in detach_command forces the fork/exec path. The held fds include the write side of a download just renamed onto
    // `binary_path`. So retry instead of failing the install (and deleting a racer's freshly installed binary)
    let mut last_spawn = String::new();
    for attempt in 1..=SMOKE_TEST_ETXTBSY_ATTEMPTS {
        let mut cmd = tokio::process::Command::new(binary_path);
        cmd.arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        xai_grok_tools::util::detach_command(&mut cmd);
        match tokio::time::timeout(SMOKE_TEST_TIMEOUT, cmd.output()).await {
            Err(_) => return Err(VersionProbeError::Timeout),
            Ok(Ok(output)) => return Ok(output),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                last_spawn = e.to_string();
                if attempt < SMOKE_TEST_ETXTBSY_ATTEMPTS {
                    tokio::time::sleep(SMOKE_TEST_ETXTBSY_BACKOFF * attempt).await;
                }
            }
            Ok(Err(e)) => return Err(VersionProbeError::Spawn(e.to_string())),
        }
    }
    // Reached only when every attempt hit ETXTBSY; `last_spawn` holds the final spawn error
    Err(VersionProbeError::Spawn(last_spawn))
}

async fn smoke_test_binary(binary_path: &std::path::Path) -> Result<String, SmokeTestFailure> {
    let output = probe_version(binary_path)
        .await
        .map_err(|error| match error {
            VersionProbeError::Timeout => SmokeTestFailure::Timeout,
            VersionProbeError::Spawn(message) => SmokeTestFailure::Spawn(message),
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let status = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| output.status.to_string());
    let stderr = truncate_err(&String::from_utf8_lossy(&output.stderr), 400);
    Err(SmokeTestFailure::NonZero { status, stderr })
}

#[cfg(feature = "community-build")]
fn community_version_output_matches(output: &str, expected_version: &str) -> bool {
    let expected_prefix = format!("{} {expected_version} (", xai_grok_product::CLI_NAME);
    output
        .lines()
        .next()
        .is_some_and(|line| line.trim().starts_with(&expected_prefix))
}

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L2880-L3001
/// If the copy then fails, the rename is rolled back to avoid a broken install. In that case `dest` is renamed to a
/// unique `<dest>.old.{pid}-{seq}.old` sibling instead, so a locked leftover can never block the update. Still-locked
/// ones survive until a later update runs after those processes exit.
#[cfg(windows)]
async fn windows_replace_exe(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    let _install_lock = acquire_windows_update_lock(dest)?;
    windows_replace_exe_locked(src, dest).await
}

#[cfg(windows)]
async fn windows_replace_exe_locked(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    let file_name = dest
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("destination has no filename: {}", dest.display()))?
        .to_string_lossy();
    let old = dest.with_file_name(format!("{file_name}.old"));

    sweep_old_exe_backups(&old).await;

    match tokio::fs::copy(src, dest).await {
        Ok(_) => return Ok(()),
        // ERROR_SHARING_VIOLATION (32) / ERROR_ACCESS_DENIED (5): exe is locked by a running process
        // Fall through to rename-and-replace
        Err(e) if matches!(e.raw_os_error(), Some(32) | Some(5)) => {
            tracing::debug!("exe locked, falling back to rename: {e}");
        }
        Err(e) => return Err(e.into()),
    }

    // A .old that survived the sweep is locked; renaming onto it would need to delete-replace it and fail, so divert to a guaranteed-free name
    let old_is_free = matches!(
        tokio::fs::symlink_metadata(&old).await,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    );
    let mut aside = if old_is_free {
        old.clone()
    } else {
        let diverted = unique_temp_sibling(&old, "old");
        tracing::debug!(
            "stale {} is locked; diverting aside to {}",
            old.display(),
            diverted.display()
        );
        diverted
    };

    // Move the locked file aside, then copy the new binary into place.
    let mut rename_result = tokio::fs::rename(dest, &aside).await;
    // Pid reuse can collide a diverted name with a dead updater's still-locked leftover, and a racer can occupy a just-checked-free .old
    // A fresh unique sibling handles both (3 attempts total)
    for _ in 0..2 {
        match &rename_result {
            Err(e) if matches!(e.raw_os_error(), Some(32) | Some(5)) => {
                tracing::debug!(
                    "rename aside to {} failed; retrying with a fresh name: {e}",
                    aside.display()
                );
                aside = unique_temp_sibling(&old, "old");
                rename_result = tokio::fs::rename(dest, &aside).await;
            }
            _ => break,
        }
    }
    rename_result.map_err(|e| {
        anyhow::anyhow!(
            "cannot rename locked executable {}: {e}\n\
             Close all running grok sessions and retry.",
            dest.display(),
        )
    })?;
    match tokio::fs::copy(src, dest).await {
        Ok(_) => {
            sweep_old_exe_backups(&old).await;
            Ok(())
        }
        Err(e) => {
            // Rollback: restore the old binary so the install isn't broken.
            if let Err(rollback) = tokio::fs::rename(&aside, dest).await {
                anyhow::bail!(
                    "install failed: {e}; rollback failed: {rollback}; previous executable retained at {}",
                    aside.display()
                );
            }
            Err(e.into())
        }
    }
}

/// The `<exe>.old` prefix keeps the sweep away from `<exe>` itself, other executables' leftovers, and the `.rollback.bak`
/// / `.tmp` sibling shapes. Rename preserves mtime, so a racer's seconds-old aside already looks days old and age cannot
/// distinguish it. Deleting a racer's fresh unlocked aside is the same accepted race as in `tmp_download_path`.
#[cfg(windows)]
async fn sweep_old_exe_backups(old: &std::path::Path) {
    let _ = tokio::fs::remove_file(old).await;
    let (Some(dir), Some(old_name)) = (old.parent(), old.file_name().and_then(|n| n.to_str()))
    else {
        return;
    };
    let prefix = format!("{old_name}.");
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let is_owned_aside = name
            .strip_prefix(&prefix)
            .and_then(|suffix| suffix.strip_suffix(".old"))
            .and_then(|nonce| nonce.split_once('-'))
            .is_some_and(|(pid, sequence)| {
                !pid.is_empty()
                    && !sequence.is_empty()
                    && pid.bytes().all(|byte| byte.is_ascii_digit())
                    && sequence.bytes().all(|byte| byte.is_ascii_digit())
            });
        if is_owned_aside {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L3766-L3783
#[cfg(windows)]
fn acquire_windows_update_lock(exe: &std::path::Path) -> Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    let name = exe
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("executable has no filename"))?;
    let lock = exe.with_file_name(format!("{}.update.lock", name.to_string_lossy()));
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(&lock)
        .context(
            "another updater or backup cleanup is using this installation; retry after it finishes",
        )
}

// Preserve the historical tools re-export destination without the application dependency graph.
mod xai_grok_tools { pub mod util {

// Verbatim crates/codegen/xai-tty-utils/src/lib.rs:L171-L190
/// Detach a `tokio::process::Command` from the parent's controlling TTY/console. Unix: `pre_exec` hook calling `setsid`
/// (EPERM fallback: `setpgid`); Windows: `CREATE_NO_WINDOW`. Do NOT add `DETACHED_PROCESS` — it breaks stdio pipe
/// inheritance for grandchildren (`cmd.exe` → `node`). Detach only: the child's `PATH` is left alone, so a caller that
/// `env_clear`ed and installed its own keeps it. A spawn that resolves `git` by name adds [`prepend_bundled_git_path`]
/// itself.
pub fn detach_command(cmd: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        // SAFETY: every hook `detach_pre_exec_hook` can return is async-signal-safe,
        // and the env read that selects one happens here, before fork.
        unsafe {
            cmd.pre_exec(detach_pre_exec_hook());
        }
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::CREATE_NO_WINDOW;
        cmd.creation_flags(CREATE_NO_WINDOW.0);
    }
}

} }

// TEST SEAMS ONLY: fixture stream replaces HTTPS adapter; destination is an isolated genuine old EXE.
async fn isolated_transaction(asset: VerifiedAsset, destination: std::path::PathBuf, source: &Path, truncate_stream: bool) -> Result<String> {
    let version = asset.version.clone();

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L1161-L1171
        let destination_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("running executable has no valid filename"))?;
        if !destination_name.eq_ignore_ascii_case(xai_grok_product::executable_name()) {
            anyhow::bail!(
                "refusing to update renamed executable {}; expected {}",
                destination.display(),
                xai_grok_product::executable_name()
            );
        }

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L1173-L1174
        let archive = unique_temp_sibling(&destination, "download.zip");
        let candidate = unique_temp_sibling(&destination, "candidate.exe");

    // Replaces only original L1175 download_verified network adapter call.
    write_verified_download(&asset, &archive, &ProgressBar::hidden(), file_stream(source, truncate_stream)?).await?;

// Verbatim crates/codegen/xai-grok-update/src/auto_update.rs:L1177-L1231
        let archive_for_extract = archive.clone();
        let candidate_for_extract = candidate.clone();
        let asset_for_extract = asset.clone();
        let extract_result = tokio::task::spawn_blocking(move || {
            crate::community_release::extract_verified_executable(
                &asset_for_extract,
                &archive_for_extract,
                &candidate_for_extract,
            )
        })
        .await;
        let _ = tokio::fs::remove_file(&archive).await;
        match extract_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = tokio::fs::remove_file(&candidate).await;
                return Err(error).context("validating the community release ZIP");
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&candidate).await;
                anyhow::bail!("community release ZIP verifier failed: {error}");
            }
        }

        if let Err(error) = crate::community_update_cancel::check() {
            let _ = tokio::fs::remove_file(&candidate).await;
            return Err(error);
        }
        let version_output = match smoke_test_binary(&candidate).await {
            Ok(output) => output,
            Err(error) => {
                let _ = tokio::fs::remove_file(&candidate).await;
                anyhow::bail!("downloaded community binary failed verification: {error}");
            }
        };
        if !community_version_output_matches(&version_output, &version) {
            let _ = tokio::fs::remove_file(&candidate).await;
            anyhow::bail!(
                "downloaded community binary reported an unexpected version: {}",
                truncate_err(&version_output, 200)
            );
        }

        // Keep activation and its completion receipt in the same transaction.
        if let Err(error) = crate::community_update_cancel::begin_activation() {
            let _ = tokio::fs::remove_file(&candidate).await;
            return Err(error);
        }
        let install_lock = acquire_windows_update_lock(&destination);
        let replace_result = match &install_lock {
            Ok(_) => windows_replace_exe_locked(&candidate, &destination).await,
            Err(error) => Err(anyhow::anyhow!("{error:#}")),
        };
        let _ = tokio::fs::remove_file(&candidate).await;
        replace_result?;

    // Production success-receipt and UI block L1232-L1239 is deliberately excluded.
    Ok(version)
}

include!("transaction_tests.rs");
