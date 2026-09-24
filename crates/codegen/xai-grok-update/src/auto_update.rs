use anyhow::{Context, Result};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::cleanup_downloads::cleanup_old_downloads;
use crate::version::{
    UpdateConfig, fetch_latest_version, get_installed_grok_version, get_latest_version,
    is_version_cache_fresh, try_fetch_stable_pointer, write_version_cache,
};
use semver::Version;
use xai_grok_shell::util::config;
use xai_grok_shell::util::grok_home::{grok_application, grok_home};
pub use xai_grok_telemetry::events::CliUpdateTrigger;
use xai_grok_telemetry::events::{
    CliUpdate, CliUpdateChannel, CliUpdateErrorKind, CliUpdateInstaller, CliUpdateOutcome,
};

#[derive(Clone, Copy, Debug)]
pub enum UpdateRunMode {
    Blocking,
    NonBlocking,
}

#[cfg(feature = "community-build")]
const PROMPT_UPDATE_NOW: &str = "现在更新？[Y/n/d]";
#[cfg(not(feature = "community-build"))]
const PROMPT_UPDATE_NOW: &str = "Update now? [Y/n/d]";
#[cfg(feature = "community-build")]
const MSG_AUTO_UPDATE_BACKGROUND: &str = "正在后台自动更新。";
#[cfg(not(feature = "community-build"))]
const MSG_AUTO_UPDATE_BACKGROUND: &str = "Auto-update running in background.";
#[cfg(feature = "community-build")]
const MSG_RUN_UPDATE_MANUAL: &str = "运行 `grok-zh update` 获取最新版本。";
#[cfg(not(feature = "community-build"))]
const MSG_RUN_UPDATE_MANUAL: &str = "Run `grok update` to get the latest version.";
fn human_channel_label(channel: &str) -> String {
    if cfg!(feature = "community-build") {
        let translated = match channel {
            "stable" => "稳定",
            "alpha" => "预览",
            "enterprise" => "企业",
            other => other,
        };
        format!(" [{translated}]")
    } else {
        format!(" [{channel}]")
    }
}

fn announce_update_available(current: &str, latest: &str, channel: &str) {
    if cfg!(feature = "community-build") {
        eprintln!(
            "发现 {} 新版本：{} → {}{}",
            xai_grok_product::DISPLAY_NAME,
            current,
            latest,
            human_channel_label(channel)
        );
    } else {
        eprintln!(
            "A new version of Grok Build is available: {} -> {}{}",
            current,
            latest,
            human_channel_label(channel)
        );
    }
}

fn announce_update_failed(error: &dyn std::fmt::Display) {
    if cfg!(feature = "community-build") {
        eprintln!("更新失败：{error}");
    } else {
        eprintln!("Update failed: {error}");
    }
}

/// An empty or `"stable"` channel means stable, the installers' default.
fn is_stable_channel(channel: &str) -> bool {
    channel.is_empty() || channel == "stable"
}

/// Manual-install one-liner for this platform's bootstrap installer. On Unix the variable must prefix `bash` (which runs
/// install.sh), not `curl`. In `VAR=x curl … | bash` the assignment applies to `curl` only and install.sh would fall back
/// to stable.
fn manual_install_cmd(channel: &str) -> String {
    if cfg!(feature = "community-build") {
        return xai_grok_product::COMMUNITY_RELEASES_URL.to_string();
    }
    // Only interpolate a well-formed channel ([A-Za-z0-9._-]) into the shell one-liner
    // Anything else falls back to stable (a working installer beats a broken quoted command)
    let channel = channel.trim();
    let safe = !channel.is_empty()
        && channel
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if channel == "enterprise" {
        return if cfg!(windows) {
            "irm https://x.ai/cli/enterprise-install.ps1 | iex".to_string()
        } else {
            "curl -fsSL https://x.ai/cli/enterprise-install.sh | bash".to_string()
        };
    }
    if is_stable_channel(channel) || !safe {
        return if cfg!(windows) {
            "irm https://x.ai/cli/install.ps1 | iex".to_string()
        } else {
            "curl -fsSL https://x.ai/cli/install.sh | bash".to_string()
        };
    }
    if cfg!(windows) {
        format!("$env:GROK_CHANNEL='{channel}'; irm https://x.ai/cli/install.ps1 | iex")
    } else {
        format!("curl -fsSL https://x.ai/cli/install.sh | GROK_CHANNEL='{channel}' bash")
    }
}

fn reinstall_hint(installer: &str, channel: &str) -> String {
    if cfg!(feature = "community-build") {
        return format!(
            "请从本项目 Releases 页面下载最新 grok-zh 安装包：\n  {}",
            manual_install_cmd(channel)
        );
    }
    match installer {
        "npm" => "Please reinstall via npm:\n  npm i -g @xai-official/grok".to_string(),
        "gh-release" => "Please reinstall via GitHub Releases:\n  gh release download --repo xai-org-shared/grok-build --pattern 'grok-*' --output grok && chmod +x grok".to_string(),
        _ => format!("Please reinstall via:\n  {}", manual_install_cmd(channel)),
    }
}

/// True when this process is an x86_64 build translated by Rosetta on an Apple Silicon host. `hw.optional.arm64` is 1 on
/// Apple Silicon, including from a translated process, where the compile-time arch says x86_64. A missing key (genuine
/// Intel Mac) or any error means not Apple Silicon: the probe fails open to the compile-time arch.
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn running_under_rosetta_on_apple_silicon() -> bool {
    static ROSETTA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ROSETTA.get_or_init(|| {
        let mut val: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>();
        // SAFETY: the name is a valid NUL-terminated C string; `val`/`len`
        // describe a properly sized c_int; sysctlbyname writes at most
        // `len` bytes into `val`.
        let rc = unsafe {
            libc::sysctlbyname(
                c"hw.optional.arm64".as_ptr(),
                (&raw mut val).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        rc == 0 && val == 1
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn running_under_rosetta_on_apple_silicon() -> bool {
    false
}

/// Arch to download artifacts for, given the compile-time arch and whether the host is Apple Silicon running this build under Rosetta.
/// Separated from [`detect_platform`] so the decision is unit-testable.
fn corrected_arch(
    os: &'static str,
    arch: &'static str,
    rosetta_on_apple_silicon: bool,
) -> &'static str {
    if os == "macos" && arch == "x86_64" && rosetta_on_apple_silicon {
        "aarch64"
    } else {
        arch
    }
}

/// Artifact platform from [`detect_platform`]; falls back to the compile-time values for combos the updater does not support.
fn platform_label() -> String {
    detect_platform()
        .map(|(os, arch)| format!("{os}-{arch}"))
        .unwrap_or_else(|_| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}

/// Typed phase marker for telemetry classification.
/// Deliberately no `source()`, so anyhow's `{:#}` does not print the chain twice.
#[derive(Debug, thiserror::Error)]
enum InstallPhaseError {
    #[error("{0:#}")]
    Download(anyhow::Error),
    #[error("{0:#}")]
    Activate(anyhow::Error),
}

/// Smoke failures stay unwrapped: already typed, and the base-retry abort in [`install_internal_from_bases`] must still downcast them.
fn wrap_download_err(e: anyhow::Error) -> anyhow::Error {
    if e.is::<SmokeTestFailure>() {
        e
    } else {
        InstallPhaseError::Download(e).into()
    }
}

#[doc(hidden)]
pub fn classify_install_error(err: &anyhow::Error) -> CliUpdateErrorKind {
    if let Some(smoke) = err.downcast_ref::<SmokeTestFailure>() {
        return match smoke {
            SmokeTestFailure::Timeout => CliUpdateErrorKind::SmokeTimeout,
            SmokeTestFailure::Spawn(_) => CliUpdateErrorKind::SmokeSpawn,
            SmokeTestFailure::NonZero { .. } => CliUpdateErrorKind::SmokeNonzero,
        };
    }
    match err.downcast_ref::<InstallPhaseError>() {
        Some(InstallPhaseError::Download(_)) => CliUpdateErrorKind::Download,
        Some(InstallPhaseError::Activate(_)) => CliUpdateErrorKind::Activate,
        // npm / gh-release failures carry no phase marker.
        None => CliUpdateErrorKind::Other,
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub installer: Option<String>,
    pub channel: String,
    pub auto_update: Option<bool>,
    pub error: Option<String>,
}

pub fn print_update_status(status: &UpdateStatus, json: bool) -> anyhow::Result<()> {
    if json {
        let payload = serde_json::to_string(status)?;
        println!("{payload}");
        return Ok(());
    }

    let product_name = if cfg!(feature = "community-build") {
        xai_grok_product::DISPLAY_NAME
    } else {
        "Grok Build"
    };

    if let Some(error) = status.error.as_deref() {
        println!(
            "{} - v{}{}",
            product_name,
            status.current_version,
            human_channel_label(&status.channel)
        );
        if cfg!(feature = "community-build") {
            println!("检查更新失败：{error}");
        } else {
            println!("Update check failed: {error}");
        }
        return Ok(());
    }

    let channel_label = human_channel_label(&status.channel);

    if status.update_available {
        if let Some(latest_version) = status.latest_version.as_deref() {
            if cfg!(feature = "community-build") {
                println!(
                    "发现 {} 新版本：{} → {}{}",
                    product_name, status.current_version, latest_version, channel_label
                );
            } else {
                println!(
                    "A new version of {} is available: {} -> {}{}",
                    product_name, status.current_version, latest_version, channel_label
                );
            }
        } else {
            if cfg!(feature = "community-build") {
                println!("发现 {product_name} 新版本。");
            } else {
                println!("A new version of {product_name} is available.");
            }
        }
        return Ok(());
    }

    if let Some(latest_version) = status.latest_version.as_deref() {
        if cfg!(feature = "community-build") {
            println!(
                "{} - v{}（最新：{}）{}",
                product_name, status.current_version, latest_version, channel_label
            );
        } else {
            println!(
                "{} - v{} (latest: {}){}",
                product_name, status.current_version, latest_version, channel_label
            );
        }
        return Ok(());
    }

    println!(
        "{} - v{}{}",
        product_name, status.current_version, channel_label
    );
    Ok(())
}

pub async fn check_update_status(update_config: &UpdateConfig) -> UpdateStatus {
    let current_version = get_installed_grok_version();
    let channel = update_config.channel.clone();

    if crate::ensure_selected_updates_enabled().is_err() {
        return UpdateStatus {
            current_version,
            latest_version: None,
            update_available: false,
            installer: None,
            channel,
            auto_update: Some(false),
            error: Some(crate::UPDATE_DISABLED_REASON.to_string()),
        };
    }

    let current_config = config::load_config().await;
    let auto_update = current_config.cli.auto_update;

    let installer = get_installer().await.map(|value| value.to_string());

    let Some(ref inst) = installer else {
        return UpdateStatus {
            current_version,
            latest_version: None,
            update_available: false,
            installer,
            channel,
            auto_update,
            error: None,
        };
    };

    match get_latest_version(inst, update_config).await {
        // --check shares the updater's decision, so it never advertises a version the policy would skip, clamp away, or can't satisfy
        Ok(latest) => match plan_for(&config::VersionPolicy::resolve(), latest) {
            UpdatePlan::Install { target, .. } => {
                let mut error = None;
                let update_available = match needs_update(
                    &current_version,
                    &target,
                    &channel,
                    false,
                ) {
                    Some(value) => value,
                    None => {
                        // Distinguish parse failure from unsupported channel.
                        let parse_ok = Version::parse(&current_version).is_ok()
                            && Version::parse(&target).is_ok();
                        error = Some(if cfg!(feature = "community-build") {
                            if parse_ok {
                                format!(
                                    "不支持发布通道“{channel}”（当前={current_version}，最新={target}）。支持的通道：stable、alpha、enterprise。"
                                )
                            } else {
                                format!("版本解析失败（当前={current_version}，最新={target}）")
                            }
                        } else if parse_ok {
                            format!(
                                "Unsupported release channel '{channel}' (current={current_version}, latest={target}). \
                                     Supported channels: stable, alpha, enterprise."
                            )
                        } else {
                            format!(
                                "Failed to parse versions (current={current_version}, latest={target})"
                            )
                        });
                        false
                    }
                };
                UpdateStatus {
                    current_version,
                    latest_version: Some(target),
                    update_available,
                    installer,
                    channel,
                    auto_update,
                    error,
                }
            }
            // Policy skips (anti-downgrade) or can't satisfy the floor: no upgrade.
            UpdatePlan::Skip { latest } | UpdatePlan::Unavailable { latest, .. } => UpdateStatus {
                current_version,
                latest_version: Some(latest),
                update_available: false,
                installer,
                channel,
                auto_update,
                error: None,
            },
        },
        Err(err) => UpdateStatus {
            current_version,
            latest_version: None,
            update_available: false,
            installer,
            channel,
            auto_update,
            error: Some(err.to_string()),
        },
    }
}

enum UpdatePlan {
    /// Anti-downgrade skip; `latest` is reported to the user.
    Skip {
        latest: String,
    },
    /// A hard `required_minimum` exceeds the latest release, so nothing satisfies it.
    Unavailable {
        latest: String,
        target: String,
    },
    Install {
        latest: String,
        target: String,
    },
}

/// Classify a fetched `latest` release under `policy`.
/// Pure; `fetch_update_plan` is the IO wrapper.
/// `--check` shares this so it can't diverge from the updater.
fn plan_for(policy: &config::VersionPolicy, latest: String) -> UpdatePlan {
    let Some(target) = policy.resolve_target(&latest) else {
        return UpdatePlan::Skip { latest };
    };
    // A hard `required_minimum` can clamp above the latest release; that version doesn't exist
    if matches!(
        (
            Version::parse(&target),
            Version::parse(&latest),
        ),
        (Ok(t), Ok(l)) if t > l
    ) {
        UpdatePlan::Unavailable { latest, target }
    } else {
        UpdatePlan::Install { latest, target }
    }
}

async fn fetch_update_plan(
    installer: &str,
    update_config: &UpdateConfig,
    policy: &config::VersionPolicy,
) -> Result<UpdatePlan> {
    let _check_span =
        xai_grok_telemetry::region!("update.check", xai_grok_telemetry::region::Parent::Inherit);
    let latest = fetch_latest_version(installer, update_config).await?;
    Ok(plan_for(policy, latest))
}

/// Installer and version the leader/background path should converge to: an upgrade OR an authoritative-installer rollback.
/// `None` means stay put.
/// Gates on the installer (via `installer_allows_downgrade`) so npm is never downgraded; the decision depends on the installer, never the caller.
pub async fn auto_update_target(update_config: &UpdateConfig) -> Option<(&'static str, String)> {
    let installer = get_installer().await?;
    let current = get_installed_grok_version();
    let policy = config::VersionPolicy::resolve();
    let UpdatePlan::Install { target, .. } = fetch_update_plan(installer, update_config, &policy)
        .await
        .ok()?
    else {
        return None;
    };
    needs_update(
        &current,
        &target,
        &update_config.channel,
        installer_allows_downgrade(installer),
    )
    .unwrap_or(false)
    .then_some((installer, target))
}

/// Outcome of [`ensure_latest_on_disk`].
#[derive(Debug)]
pub struct EnsureLatestOutcome {
    /// Version this call downloaded and installed; `None` when the disk was already current (or there was no installer).
    pub installed: Option<String>,
    /// The running process differs from what is now on disk in the channel's update direction.
    /// The caller should relaunch onto the on-disk binary.
    pub relaunch_needed: bool,
}

/// Unlike [`run_update`] this never uses the compiled-in version for the download decision. A binary already installed by
/// another process (TUI background download, explicit `grok update`) is reused as-is. On Windows a busy leader therefore
/// still re-downloads hourly; only the symlink layout can prove the disk is current without exec'ing the binary.
pub async fn ensure_latest_on_disk(update_config: &UpdateConfig) -> Result<EnsureLatestOutcome> {
    let _ensure_span = xai_grok_telemetry::region!(
        "update.ensure_latest",
        xai_grok_telemetry::region::Parent::Inherit
    );
    let mut outcome = EnsureLatestOutcome {
        installed: None,
        relaunch_needed: false,
    };
    let Some(installer) = get_installer().await else {
        return Ok(outcome);
    };
    heal_managed_install(installer).await;
    let allow_downgrade = installer_allows_downgrade(installer);
    let policy = config::VersionPolicy::resolve();
    let UpdatePlan::Install { target, .. } =
        fetch_update_plan(installer, update_config, &policy).await?
    else {
        return Ok(outcome);
    };

    let effective_current =
        disk_version_for_installer(installer).unwrap_or_else(get_installed_grok_version);
    if needs_update(
        &effective_current,
        &target,
        &update_config.channel,
        allow_downgrade,
    )
    .unwrap_or(false)
    {
        run_install_script(
            installer,
            Some(&target),
            update_config,
            CliUpdateTrigger::LeaderConverge,
        )
        .await?;
        // The leader relaunches right after a successful converge and would die with the event still in flight
        // Failures keep it alive, so successes would under-report. The install is already done.
        xai_grok_telemetry::session_ctx::drain_pending(xai_grok_telemetry::session_ctx::CLI_DRAIN)
            .await;
        outcome.installed = Some(target.clone());
    }

    // Relaunch when the running binary differs from what's on disk in the channel's update direction
    // This covers binaries installed by other processes, not just the install above
    let running = get_installed_grok_version();
    if let Some(disk_now) =
        disk_version_for_installer(installer).or_else(|| outcome.installed.clone())
    {
        outcome.relaunch_needed =
            needs_update(&running, &disk_now, &update_config.channel, allow_downgrade)
                .unwrap_or(false);
    }
    Ok(outcome)
}

/// Disk-version probe gated on the installer actually maintaining the managed `~/.grok/bin/grok` symlink. Only the
/// internal (install.sh / CDN) and gh-release installers write that symlink. npm manages its own global install, so a
/// symlink left over from a previous internal install would LIE about the npm install's version.
fn disk_version_for_installer(installer: &str) -> Option<String> {
    match installer {
        "internal" | "gh-release" => crate::version::installed_on_disk_version(),
        #[cfg(feature = "community-build")]
        crate::community_release::COMMUNITY_INSTALLER => {
            crate::version::installed_on_disk_version()
        }
        _ => None,
    }
}

fn env_installer() -> Option<&'static str> {
    if let Ok(v) = std::env::var("GROK_INSTALLER") {
        return match v.to_ascii_lowercase().as_str() {
            "npm" => Some("npm"),
            "internal" => Some("internal"),
            "gh-release" | "gh" => Some("gh-release"),
            _ => None,
        };
    }
    if std::env::var_os("GROK_MANAGED_BY_NPM").is_some() {
        return Some("npm");
    }
    if std::env::var_os("GROK_MANAGED_BY_INTERNAL").is_some() {
        return Some("internal");
    }
    if std::env::var_os("npm_config_user_agent").is_some() {
        return Some("npm");
    }
    None
}

pub async fn get_installer() -> Option<&'static str> {
    #[cfg(feature = "community-build")]
    {
        crate::ensure_community_updates_enabled().ok()?;
        return Some(crate::community_release::COMMUNITY_INSTALLER);
    }
    #[cfg(not(feature = "community-build"))]
    {
        crate::ensure_updates_enabled().ok()?;
        if let Some(i) = env_installer() {
            return Some(i);
        }
        let cfg = config::load_config().await;
        match cfg.cli.installer.as_deref() {
            Some("npm") => Some("npm"),
            Some("gh-release") => Some("gh-release"),
            Some(_) => Some("internal"),
            // A wiped config must not reclassify an npm install as internal:
            // that re-enables downgrades and updates npm never sees.
            None if path_resolves_to_npm_entry() => Some("npm"),
            None => Some("internal"),
        }
    }
}

/// The npm entry links to a binary inside the package, so the running
/// executable's real path names the installer.
fn path_resolves_to_npm_entry() -> bool {
    std::env::current_exe()
        .and_then(|exe| dunce::canonicalize(&exe))
        .is_ok_and(|exe| is_under_node_modules(&exe))
}

fn is_under_node_modules(exe: &std::path::Path) -> bool {
    exe.components().any(|c| c.as_os_str() == "node_modules")
}

fn needs_update(current: &str, target: &str, channel: &str, allow_downgrade: bool) -> Option<bool> {
    let current = Version::parse(current).ok()?;
    let target = Version::parse(target).ok()?;
    match channel {
        // Stable channels never accept pre-release candidates. A pre-release
        // current version still follows normal SemVer ordering, so a same-base
        // stable release is newer while a lower stable release is only accepted
        // when the installer explicitly allows rollback.
        "stable" | "enterprise" => {
            if !target.pre.is_empty() {
                tracing::warn!(
                    %current, %target,
                    channel = %channel,
                    "stable/enterprise channel received pre-release candidate, rejecting"
                );
                return Some(false);
            }
            #[cfg(not(feature = "community-build"))]
            if !current.pre.is_empty() {
                return Some(true);
            }
        }
        "alpha" => {}
        _ => return None,
    }
    Some(if allow_downgrade {
        target != current
    } else {
        target > current
    })
}

/// Returns `true` for installer backends whose version source is authoritative (managed by xAI directly). Returns `false`
/// for backends like npm where stale corporate registries/proxies can return arbitrarily old versions. Users who
/// installed via `install.sh` are classified as `"internal"` by `get_installer()`, so they also get rollback support.
fn installer_allows_downgrade(installer: &str) -> bool {
    match installer {
        "internal" | "gh-release" => true,
        #[cfg(feature = "community-build")]
        // GitHub's latest compatible Release can temporarily be older than a
        // CI/prerelease build. Do not turn that into an automatic downgrade;
        // explicit --stable/--alpha switches and --version retain their
        // intentional rollback paths in run_update. --force only reinstalls
        // the effective current version when the pointer is older.
        crate::community_release::COMMUNITY_INSTALLER => false,
        "npm" => false,
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct UpdateAvailable {
    /// The latest version string (e.g. "0.1.200").
    pub latest_version: String,
}

/// Outcome of [`check_update_background`].
pub struct BackgroundUpdateCheck {
    /// `Some` when the *running* binary is older than the channel pointer; drives the in-TUI restart hint regardless of who downloads the binary.
    pub update: Option<UpdateAvailable>,
    /// Handle to the background `grok update` child, `Some` only when a download was actually started (the on-disk install was behind the pointer).
    /// The TUI parks this and `wait()`s on it at quit-for-update time instead of spawning a second downloader.
    pub download: Option<tokio::process::Child>,
}

impl BackgroundUpdateCheck {
    fn none() -> Self {
        Self {
            update: None,
            download: None,
        }
    }
}

fn effective_auto_update_enabled(configured: Option<bool>) -> bool {
    configured.unwrap_or_else(crate::default_auto_update_enabled)
}

fn should_skip_cached_background_check(auto_update: bool, cache_fresh: bool) -> bool {
    auto_update && cache_fresh
}

fn should_start_background_download(auto_update: bool, disk_needs_download: bool) -> bool {
    auto_update && disk_needs_download
}

/// Check for available updates without blocking the TUI startup.
///
/// Sets [`BackgroundUpdateCheck::update`] when the running binary is older than the channel pointer.
/// If `auto_update` is enabled **and the on-disk install is also behind the pointer**, kicks off a download (a detached `grok update` child).
/// The new binary is then ready when the user quits and relaunches.
/// When another process (an earlier TUI, the leader's hourly checker) already put the target version on disk, no download is started.
/// Only the restart hint is shown.
pub async fn check_update_background(update_config: &UpdateConfig) -> BackgroundUpdateCheck {
    let Some(installer) = get_installer().await else {
        return BackgroundUpdateCheck::none();
    };

    heal_managed_install(installer).await;

    let current_config = config::load_config().await;
    let auto_update = effective_auto_update_enabled(current_config.cli.auto_update);
    #[cfg(not(feature = "community-build"))]
    if !auto_update {
        return BackgroundUpdateCheck::none();
    }

    // A fresh cache can suppress work after an automatic updater has already
    // prepared the target. In the community default-off mode we still query
    // metadata on each launch so dismissing one prompt cannot hide it for the
    // cache TTL while no package has been downloaded.
    if should_skip_cached_background_check(auto_update, is_version_cache_fresh().await) {
        return BackgroundUpdateCheck::none();
    }

    let current_version = get_installed_grok_version();
    let policy = config::VersionPolicy::resolve();
    let target_version = match fetch_update_plan(installer, update_config, &policy).await {
        Ok(UpdatePlan::Install { target, .. }) => target,
        Ok(UpdatePlan::Skip { .. } | UpdatePlan::Unavailable { .. }) | Err(_) => {
            return BackgroundUpdateCheck::none();
        }
    };

    let allow_downgrade = installer_allows_downgrade(installer);
    if !needs_update(
        &current_version,
        &target_version,
        &update_config.channel,
        allow_downgrade,
    )
    .unwrap_or(false)
    {
        let stable_ptr = try_fetch_stable_pointer().await;
        write_version_cache(&target_version, stable_ptr.as_deref()).await;
        return BackgroundUpdateCheck::none();
    }

    // Only download when the on-disk install is behind the pointer. The running process being stale (checked above) just
    // means "show the restart hint". The quit-for-update path's `grok update` child resolves to "Already up to date" against
    // the same disk state. For npm a leftover symlink would wrongly suppress the download (see `disk_version_for_installer`)
    let disk_needs_download = match disk_version_for_installer(installer) {
        Some(disk) => needs_update(
            &disk,
            &target_version,
            &update_config.channel,
            allow_downgrade,
        )
        .unwrap_or(true),
        None => true,
    };

    // Community builds default to a metadata-only check. They download here
    // only after the user explicitly enables auto-update; otherwise Ctrl+U
    // starts the same update subprocess after the TUI exits.
    let download = if should_start_background_download(auto_update, disk_needs_download) {
        match run_update_subcommand(UpdateRunMode::NonBlocking, CliUpdateTrigger::AutoBackground)
            .await
        {
            Ok(child) => child,
            Err(e) => {
                tracing::warn!("Background update download failed to start: {e}");
                None
            }
        }
    } else if !auto_update {
        tracing::info!(
            target_version = %target_version,
            "Background update: automatic download disabled; waiting for explicit user action"
        );
        None
    } else {
        tracing::info!(
            target_version = %target_version,
            "Background update: target already on disk, skipping download"
        );
        None
    };

    BackgroundUpdateCheck {
        update: Some(UpdateAvailable {
            latest_version: target_version,
        }),
        download,
    }
}

/// Returns Ok(true) if a blocking update ran; otherwise Ok(false).
pub async fn run_update_if_available(
    run_mode: UpdateRunMode,
    interactive: bool,
    trigger: CliUpdateTrigger,
    update_config: &UpdateConfig,
) -> Result<bool> {
    let Some(inst) = get_installer().await else {
        return Ok(false);
    };

    heal_managed_install(inst).await;

    if is_version_cache_fresh().await {
        return Ok(false);
    }

    let current_config = config::load_config().await;

    let auto_update = effective_auto_update_enabled(current_config.cli.auto_update);
    if !auto_update {
        return Ok(false);
    }

    #[cfg(not(feature = "community-build"))]
    if current_config.cli.auto_update.is_none()
        && let Err(error) = config::update_config(|stored| {
            if stored.cli.auto_update.is_none() {
                stored.cli.auto_update = Some(true);
            }
        })
        .await
    {
        tracing::warn!("Failed to save auto-update setting: {error}");
    }

    let current_version = get_installed_grok_version();
    let policy = config::VersionPolicy::resolve();
    // Don't write version.json here
    // Only cache after confirming no update is needed or after a successful install
    // Otherwise a failed background download would suppress retries for the TTL window
    let latest_version = match fetch_update_plan(inst, update_config, &policy).await {
        Ok(UpdatePlan::Install { target, .. }) => target,
        Ok(UpdatePlan::Skip { .. } | UpdatePlan::Unavailable { .. }) | Err(_) => return Ok(false),
    };
    if !needs_update(
        &current_version,
        &latest_version,
        &update_config.channel,
        installer_allows_downgrade(inst),
    )
    .unwrap_or(false)
    {
        let stable_ptr = try_fetch_stable_pointer().await;
        write_version_cache(&latest_version, stable_ptr.as_deref()).await;
        return Ok(false);
    }

    if auto_update {
        announce_update_available(&current_version, &latest_version, &update_config.channel);
        if interactive {
            if let Err(e) = run_update_subcommand(run_mode, trigger).await {
                announce_update_failed(&e);
            } else if matches!(run_mode, UpdateRunMode::Blocking) {
                return Ok(true);
            } else {
                eprintln!("{}", MSG_AUTO_UPDATE_BACKGROUND);
                return Ok(false);
            }
        } else if let Err(e) = run_update_subcommand(run_mode, trigger).await {
            announce_update_failed(&e);
        } else if matches!(run_mode, UpdateRunMode::Blocking) {
            return Ok(true);
        }
        return Ok(false);
    } else {
        if current_config
            .cli
            .dismissed_version
            .as_deref()
            .is_some_and(|v| v == latest_version)
        {
            return Ok(false);
        }
        announce_update_available(&current_version, &latest_version, &update_config.channel);
        if interactive {
            eprintln!("{}", PROMPT_UPDATE_NOW);
            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_ok() {
                let ans = line.trim().to_ascii_lowercase();
                if ans.is_empty() || ans == "y" || ans == "yes" {
                    // Accepting the prompt is consent, whatever the caller was
                    if let Err(e) =
                        run_update_subcommand(run_mode, CliUpdateTrigger::UserCommand).await
                    {
                        announce_update_failed(&e);
                    } else if matches!(run_mode, UpdateRunMode::Blocking) {
                        return Ok(true);
                    } else {
                        eprintln!("{}", MSG_AUTO_UPDATE_BACKGROUND);
                        return Ok(false);
                    }
                } else if ans == "d" || ans == "dismiss" {
                    let dismissed = latest_version.clone();
                    if let Err(e) = config::update_config(|st| {
                        st.cli.dismissed_version = Some(dismissed);
                    })
                    .await
                    {
                        tracing::warn!("Failed to save dismissed version: {}", e);
                    }
                }
            }
        } else {
            eprintln!("{}", MSG_RUN_UPDATE_MANUAL);
        }
    }
    Ok(false)
}

/// Launch "grok update" in blocking or non-blocking mode. `NonBlocking` mode returns the spawned child's handle. The
/// TUI's quit-for-update path `wait()`s on that in-flight download instead of spawning a second downloader. Dropping the
/// handle does not kill the child (`kill_on_drop` is off), so callers that don't care can ignore it.
async fn run_update_subcommand(
    run_mode: UpdateRunMode,
    trigger: CliUpdateTrigger,
) -> Result<Option<tokio::process::Child>> {
    let exe = std::env::current_exe()?;
    let mut cmd = tokio::process::Command::new(exe);
    // One trigger representation end to end: the enum crosses the process boundary as --trigger=<value> (FromStr on the other side)
    cmd.arg("update");
    cmd.arg(format!("--trigger={}", trigger.as_ref()));
    // Hand the resolved telemetry mode to the child, which cannot see the remote-settings layer (requirement pins still beat env)
    // None at the startup spawns: they run before the settings prefetch, when this process knows no more than the child
    // Waiting would let telemetry delay an update
    if let Some(mode) = xai_grok_telemetry::client::current_mode() {
        cmd.env("GROK_TELEMETRY_ENABLED", mode.to_string());
    }
    match run_mode {
        UpdateRunMode::Blocking => {
            // stderr must be null, not piped: `.status()` does not drain pipes, so if the child writes more than the OS pipe buffer
            // (~16 KB macOS / ~64 KB Linux) to stderr (e.g. download progress bars), the child blocks on the write while the parent
            // blocks on waitpid — deadlocking both processes. With `panic = "abort"`, the blocked child eventually receives SIGABRT.
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                // inherit, not piped: the TUI is already restored so the parent's stderr fd is a normal terminal inherit lets the
                // child's diagnostic output reach the user. With piped stderr, `status()` would immediately close the read end. The
                // child then hits EPIPE and panics, which is SIGABRT (signal 6) under panic=abort
                .stderr(Stdio::inherit());
            // No detach: the child must stay in the foreground process group so Ctrl+C cancels it with the parent
            // The atomic install protocol makes mid-download kills safe
            let status = cmd.status().await?;
            if !status.success() {
                if cfg!(feature = "community-build") {
                    anyhow::bail!("grok-zh update 执行失败：{}", status);
                }
                anyhow::bail!("grok update failed with {}", status);
            }
            Ok(None)
        }
        UpdateRunMode::NonBlocking => {
            #[cfg(feature = "community-build")]
            cmd.env("GROK_ZH_DEFER_UPDATE_NOTES", "1");
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            // Detach means a new session (Ctrl+C isolation), not handle abandonment: the child is still ours to wait() on
            xai_grok_tools::util::detach_command(&mut cmd);
            #[allow(clippy::disallowed_methods)] // the caller owns the returned handle
            let child = cmd.spawn()?;
            Ok(Some(child))
        }
    }
}

/// Run the explicit update command after the user accepts the welcome-screen
/// prompt. Unlike automatic startup paths, this intentionally ignores the
/// persisted `auto_update` preference: Ctrl+U is the user's authorization for
/// this one download and install.
pub async fn run_user_approved_update() -> Result<()> {
    run_update_subcommand(UpdateRunMode::Blocking, CliUpdateTrigger::UserCommand).await?;
    Ok(())
}

/// Resolve the grok binary path for re-execution after an update.
///
/// `current_exe()` resolves symlinks via `/proc/self/exe` (see proc(5)), so it returns the old versioned target after a symlink swap.
/// Prefer `~/.grok/bin/grok` which always points to the latest version.
fn resolve_restart_exe() -> Result<std::path::PathBuf> {
    let canonical = grok_application();
    if canonical.exists() {
        return Ok(canonical);
    }
    Ok(std::env::current_exe()?)
}

/// Restart grok with the original command-line arguments to pick up the update.
pub fn restart_grok() -> Result<()> {
    let exe = resolve_restart_exe()?;
    let mut cmd = Command::new(exe);
    for arg in std::env::args_os().skip(1) {
        cmd.arg(arg);
    }
    cmd.env_clear();
    cmd.envs(std::env::vars_os().filter(|(k, _)| k != "GROK_AUTO_UPDATE"));
    if cfg!(feature = "community-build") {
        eprintln!("正在重新启动 grok-zh…");
    } else {
        eprintln!("Restarting Grok...");
    }

    // Use exec on Unix to replace the current process, avoiding stdio issues when the parent exits
    // On Windows, fall back to spawn and exit
    #[cfg(unix)]
    {
        // Flush output before exec to ensure messages are visible
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        let err = cmd.exec();
        // exec only returns if there was an error
        anyhow::bail!("Failed to exec: {}", err);
    }

    #[cfg(not(unix))]
    {
        // Flush output before exit to ensure messages are visible
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        #[allow(clippy::disallowed_methods)] // the relaunched CLI replaces this process
        let _ = cmd.spawn()?;
        std::process::exit(0);
    }
}

pub async fn run_install_script(
    installer: &str,
    target: Option<&str>,
    update_config: &UpdateConfig,
    trigger: CliUpdateTrigger,
) -> Result<()> {
    crate::ensure_selected_updates_enabled()?;
    let from_version =
        disk_version_for_installer(installer).unwrap_or_else(get_installed_grok_version);
    let started = Instant::now();
    #[cfg(feature = "community-build")]
    let result: Result<Option<String>> =
        if installer == crate::community_release::COMMUNITY_INSTALLER {
            install_community_release(target, update_config)
                .await
                .map(Some)
        } else {
            Err(anyhow::anyhow!(
                "unsupported installer for the community distribution: {installer}"
            ))
        };
    #[cfg(not(feature = "community-build"))]
    // Internal reports the version it actually activated; npm/gh-release resolve their own artifact, so the requested target stands in
    let result: Result<Option<String>> = match installer {
        "npm" => install_npm(
            target,
            &update_config.channel,
            update_config.npm_registry.as_deref(),
        )
        .map(|()| None),
        "gh-release" => install_gh_release(target).await.map(|()| None),
        "internal" => install_internal(target, update_config).await.map(Some),
        _ => Err(anyhow::anyhow!("unsupported installer: {installer}")),
    };
    // Measured before the success-only cache sweep, so the sweep cannot inflate success durations
    let duration_ms = started.elapsed().as_millis() as u64;
    if result.is_ok() {
        remove_stale_models_cache().await;
    }
    let (outcome, error_kind) = match &result {
        Ok(_) => (CliUpdateOutcome::Success, None),
        Err(error) => (
            CliUpdateOutcome::Failed,
            Some(classify_install_error(error)),
        ),
    };
    let to_version = match &result {
        Ok(Some(installed)) => Some(installed.clone()),
        _ => target.map(str::to_string),
    };
    xai_grok_telemetry::session_ctx::log_event(CliUpdate {
        outcome,
        trigger,
        from_version,
        to_version,
        channel: CliUpdateChannel::from_channel_str(&update_config.channel),
        installer: CliUpdateInstaller::from_installer_str(installer),
        platform: platform_label(),
        rosetta: running_under_rosetta_on_apple_silicon(),
        duration_ms,
        error_kind,
    });
    result.map(|_| ()).map_err(|e| {
        #[cfg(feature = "community-build")]
        if e.is::<crate::community_update_cancel::UpdateCancelled>() {
            return e;
        }
        if cfg!(feature = "community-build") {
            anyhow::anyhow!(
                "自动更新失败：{:#}\n\n{}",
                e,
                reinstall_hint(installer, &update_config.channel)
            )
        } else {
            anyhow::anyhow!(
                "Auto-update failed: {:#}\n\n{}",
                e,
                reinstall_hint(installer, &update_config.channel)
            )
        }
    })
}

/// Install from the exact complete platform package recorded by an immutable
/// community Release. The archive is verified against GitHub metadata, its
/// layout and inner SHA256SUMS are checked, and the extracted executable is
/// smoke-tested before activation. The updater never accepts a separate raw
/// executable asset.
#[cfg(feature = "community-build")]
async fn install_community_release(
    target: Option<&str>,
    update_config: &UpdateConfig,
) -> Result<String> {
    crate::ensure_community_updates_enabled()?;

    let version = match target {
        Some(version) => version.to_string(),
        None => crate::community_release::fetch_latest_version(&update_config.channel).await?,
    };
    let parsed = Version::parse(&version)
        .with_context(|| format!("invalid community release version: {version}"))?;
    if parsed.to_string() != version || !parsed.build.is_empty() {
        anyhow::bail!("community release version is not canonical: {version}");
    }
    let asset = crate::community_release::fetch_asset(&version).await?;
    if asset.version != version {
        anyhow::bail!("release asset version does not match the requested version");
    }

    #[cfg(windows)]
    {
        let destination = std::env::current_exe().context("locating the running grok-zh.exe")?;
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

        let archive = unique_temp_sibling(&destination, "download.zip");
        let candidate = unique_temp_sibling(&destination, "candidate.exe");
        crate::community_release::download_verified(&asset, &archive).await?;

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
        crate::community_update_notes::record_success(&version, asset.release_notes.as_deref());
        eprintln!(
            "  已从 {}/releases 安装 {} v{}。",
            xai_grok_product::COMMUNITY_RELEASE_REPO,
            xai_grok_product::CLI_NAME,
            version
        );
        return Ok(version);
    }

    #[cfg(any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    ))]
    {
        return install_community_unix_release(&version, &asset).await;
    }

    #[cfg(not(any(
        windows,
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )))]
    {
        let _ = asset;
        anyhow::bail!(
            "community self-update supports only Windows x86_64 GNU or aarch64 MSVC, macOS x86_64 or aarch64, and Linux GNU x86_64 or aarch64"
        );
    }
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
fn ensure_private_community_dir(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "refusing to use a symlinked community install directory: {}",
                path.display()
            );
        }
        Ok(metadata) if !metadata.is_dir() => {
            anyhow::bail!(
                "community install path is not a directory: {}",
                path.display()
            );
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(path).with_context(|| {
                format!("creating community install directory {}", path.display())
            })?;
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!("checking community install directory {}", path.display())
            });
        }
    }

    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("re-checking community install directory {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!(
            "community install directory changed type: {}",
            path.display()
        );
    }
    if metadata.uid() != unsafe { libc::geteuid() } {
        anyhow::bail!(
            "community install directory is not owned by the current user: {}",
            path.display()
        );
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("securing community install directory {}", path.display()))?;
    let mode = std::fs::symlink_metadata(path)?.permissions().mode() & 0o777;
    if mode != 0o700 {
        anyhow::bail!(
            "community install directory is not private (mode {mode:o}): {}",
            path.display()
        );
    }
    Ok(())
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
fn validate_community_home_path(path: &std::path::Path) -> Result<()> {
    use std::path::Component;

    if !path.is_absolute() || path.parent().is_none() {
        anyhow::bail!(
            "community GROK_HOME must be an absolute non-root path: {}",
            path.display()
        );
    }
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => current.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(name) => current.push(name),
            _ => {
                anyhow::bail!(
                    "community GROK_HOME contains an unsafe path component: {}",
                    path.display()
                );
            }
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                anyhow::bail!(
                    "community GROK_HOME contains a symlink component: {}",
                    current.display()
                );
            }
            Ok(metadata) if !metadata.is_dir() => {
                anyhow::bail!(
                    "community GROK_HOME contains a non-directory component: {}",
                    current.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Only the final GROK_HOME component may be absent. Parent
                // creation is deliberately forbidden so no unresolved chain
                // can be followed by create_dir_all.
                if current != path {
                    anyhow::bail!(
                        "community GROK_HOME parent does not exist: {}",
                        current.display()
                    );
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "checking community GROK_HOME component {}",
                        current.display()
                    )
                });
            }
        }
    }
    Ok(())
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
async fn sync_directory(path: &std::path::Path) -> Result<()> {
    let dir = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("opening directory for sync {}", path.display()))?;
    dir.sync_all()
        .await
        .with_context(|| format!("syncing directory {}", path.display()))
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
struct CommunityInstallLock {
    file: std::fs::File,
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
impl Drop for CommunityInstallLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        // SAFETY: `file` remains open for this call and LOCK_UN does not
        // dereference any process memory.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
fn acquire_community_install_lock(bin_dir: &std::path::Path) -> Result<CommunityInstallLock> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let lock_path = bin_dir.join(".grok-zh-install.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&lock_path)
        .with_context(|| format!("opening community install lock {}", lock_path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("checking community install lock {}", lock_path.display()))?;
    if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
        anyhow::bail!(
            "community install lock is not a current-user regular file: {}",
            lock_path.display()
        );
    }
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("protecting community install lock {}", lock_path.display()))?;

    // SAFETY: the descriptor belongs to `file` and remains alive in the
    // returned guard. Kernel flock locks are released automatically on crash.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        anyhow::bail!(
            "another grok-zh install or automatic update is in progress ({}): {error}",
            lock_path.display()
        );
    }
    Ok(CommunityInstallLock { file })
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
fn reserve_unique_community_target(base: &std::path::Path) -> Result<std::path::PathBuf> {
    use std::os::unix::fs::OpenOptionsExt;

    for _ in 0..64 {
        let path = unique_temp_sibling(base, "installed");
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true).mode(0o600);
        match options.open(&path) {
            Ok(file) => {
                if let Err(error) = file.sync_all() {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error).with_context(|| {
                        format!("syncing reserved community target {}", path.display())
                    });
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("reserving community target {}", path.display()));
            }
        }
    }
    anyhow::bail!("unable to reserve a unique community Unix install target")
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
fn community_unix_platform_suffix() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-aarch64-gnu"
    } else {
        "linux-x86_64-gnu"
    }
}

#[cfg(all(
    feature = "community-build",
    any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    )
))]
async fn install_community_unix_release(
    version: &str,
    asset: &crate::community_release::VerifiedAsset,
) -> Result<String> {
    let home = xai_dirs::resolve_grok_home()
        .ok_or_else(|| anyhow::anyhow!("unable to resolve a user GROK_HOME"))?;
    validate_community_home_path(&home)?;
    let bin_dir = home.join("bin");
    let download_dir = home.join("grok-zh-downloads");
    for dir in [&home, &bin_dir, &download_dir] {
        ensure_private_community_dir(dir)?;
    }

    let archive_base = download_dir.join(&asset.name);
    let archive = unique_temp_sibling(&archive_base, "download");
    let platform_suffix = community_unix_platform_suffix();
    let target_base = download_dir.join(format!("grok-zh-{version}-{platform_suffix}"));
    let candidate = unique_temp_sibling(&target_base, "candidate");

    crate::community_release::download_verified(asset, &archive).await?;
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
            return Err(error).context("validating the community Unix release package");
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&candidate).await;
            anyhow::bail!("community Unix release verifier failed: {error}");
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
            anyhow::bail!("downloaded community Unix binary failed verification: {error}");
        }
    };
    if !community_version_output_matches(&version_output, version) {
        let _ = tokio::fs::remove_file(&candidate).await;
        anyhow::bail!(
            "downloaded community Unix binary reported an unexpected version: {}",
            truncate_err(&version_output, 200)
        );
    }

    if let Err(error) = crate::community_update_cancel::begin_activation() {
        let _ = tokio::fs::remove_file(&candidate).await;
        return Err(error);
    }
    let _install_lock = match acquire_community_install_lock(&bin_dir) {
        Ok(lock) => lock,
        Err(error) => {
            let _ = tokio::fs::remove_file(&candidate).await;
            return Err(error);
        }
    };
    let installed = match reserve_unique_community_target(&target_base) {
        Ok(path) => path,
        Err(error) => {
            let _ = tokio::fs::remove_file(&candidate).await;
            return Err(error);
        }
    };
    if let Err(error) = tokio::fs::rename(&candidate, &installed).await {
        let _ = tokio::fs::remove_file(&candidate).await;
        let _ = tokio::fs::remove_file(&installed).await;
        return Err(error).with_context(|| {
            format!(
                "publishing verified community binary {}",
                installed.display()
            )
        });
    }
    if let Err(error) = sync_directory(&download_dir).await {
        tracing::warn!(
            path = %download_dir.display(),
            "verified community Unix binary is installed but directory sync failed: {error:#}"
        );
    }

    // Every install gets a fresh immutable target. Prune only after both
    // entry points are active, retaining targets still used by live processes.
    if let Err(error) = swap_community_bin_links(&installed, &bin_dir).await {
        // Do not delete `installed`: a rollback failure may have left one link
        // pointing at it, and preserving a harmless orphan is safer than a
        // dangling canonical entry point.
        return Err(error).context("activating the community Unix entry points");
    }
    if let Err(error) = sync_directory(&bin_dir).await {
        tracing::warn!(
            path = %bin_dir.display(),
            "community Unix entry points are active but directory sync failed: {error:#}"
        );
    }

    cleanup_community_targets(&download_dir, &bin_dir).await;
    crate::community_update_notes::record_success(version, asset.release_notes.as_deref());

    let _ = config::update_config(|state| {
        state.cli.installer = Some(crate::community_release::COMMUNITY_INSTALLER.to_string());
    })
    .await;
    eprintln!(
        "  已从 {}/releases 安装 {} v{}。",
        xai_grok_product::COMMUNITY_RELEASE_REPO,
        xai_grok_product::CLI_NAME,
        version
    );
    Ok(version.to_string())
}

/// Every update path converges to the native build instead of perpetuating the translated one. That covers interactive
/// `grok update`, background `--auto` children, the leader's hourly converge, and forced minimum-version installs.
/// Without it, a lingering x86_64 process would reinstall x86_64 right over a fresh native install.
pub(crate) fn detect_platform() -> Result<(&'static str, &'static str)> {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        anyhow::bail!("Unsupported OS");
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        anyhow::bail!("Unsupported architecture");
    };
    Ok((
        os,
        corrected_arch(os, arch, running_under_rosetta_on_apple_silicon()),
    ))
}

/// Age past which a leftover `.tmp` download file or freshly-renamed versioned binary counts as abandoned (crashed or
/// killed updater). The per-request budget is [`DOWNLOAD_REQUEST_TIMEOUT`] and the leader's check-and-download pass
/// matches it. So a concurrent updater's in-flight or just-landed file is never deleted out from under it.
pub(crate) const STALE_TMP_AGE: Duration = Duration::from_secs(60 * 60);

/// Total timeout for a CLI artifact download request (including body).
/// Tighter budgets abort slow-link transfers mid-body and restart them from zero.
const DOWNLOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(20 * 60);

fn download_client() -> reqwest::Result<reqwest::Client> {
    xai_grok_extra_ca::build_reqwest_client(|builder| builder.timeout(DOWNLOAD_REQUEST_TIMEOUT))
}

/// Unique temp path for an in-flight download of `dest`. Two updaters may race in the same instant; that residual race is
/// accepted instead of taking a lock. The unique name means neither racer can rename the other's half-written temp file
/// into place. Leftovers older than [`STALE_TMP_AGE`] are swept by `cleanup_old_downloads`.
fn tmp_download_path(dest: &std::path::Path) -> std::path::PathBuf {
    unique_temp_sibling(dest, "tmp")
}

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

/// Set `+x` on the temp file before renaming onto `dest`, so a concurrent same-version installer never execs `dest` while it is still 0644.
async fn publish_downloaded_artifact(tmp: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(tmp, std::fs::Permissions::from_mode(0o755)).await?;
    }
    tokio::fs::rename(tmp, dest).await?;
    Ok(())
}

/// Files smaller than this are not worth fragmenting across parallel chunks.
const PARALLEL_DOWNLOAD_MIN_BYTES: u64 = 16 * 1024 * 1024;

/// Pick chunk count from file size: 1 chunk per 16 MiB, capped at 8.
fn parallel_chunk_count(size: u64) -> u64 {
    let size_mb = size / (1024 * 1024);
    (size_mb / 16).clamp(1, 8)
}

/// Try a parallel byte-range download to `dest`.
/// Returns Err when the server doesn't advertise a Content-Length, the file is too small to split, the range request is rejected, or a chunk fails.
/// The caller is expected to fall back to a single-connection download on Err.
async fn try_parallel_download(
    url: &str,
    dest: &std::path::Path,
    with_progress: bool,
) -> Result<()> {
    let client = download_client()?;

    let head = client.head(url).send().await?;
    if !head.status().is_success() {
        anyhow::bail!("HEAD failed: HTTP {}", head.status());
    }
    let size = head
        .content_length()
        .ok_or_else(|| anyhow::anyhow!("response missing Content-Length"))?;
    if size < PARALLEL_DOWNLOAD_MIN_BYTES {
        anyhow::bail!("file too small for parallel download ({} bytes)", size);
    }

    let _dl_span = xai_grok_telemetry::region::Region::from_span(tracing::info_span!(
        "update.download",
        bytes = size as i64,
    ));

    let n_chunks = parallel_chunk_count(size);
    if n_chunks < 2 {
        anyhow::bail!(
            "file size yields {} chunk(s); not worth parallelizing",
            n_chunks
        );
    }
    let chunk_size = size.div_ceil(n_chunks);

    let pb = if with_progress {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {bar:30.cyan/dim} {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("━╸─"),
        );
        Some(pb)
    } else {
        None
    };

    let tmp = tmp_download_path(dest);
    // Pre-allocate so each task can seek and write to its own range concurrently
    // One spawn_blocking call costs one blocking-pool hop; separate tokio::fs calls would each cost their own
    let tmp_for_alloc = tmp.clone();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let f = std::fs::File::create(&tmp_for_alloc)?;
        f.set_len(size)?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("blocking pre-allocate task panicked: {e}"))??;

    let tasks = (0..n_chunks).map(|i| {
        let start = i * chunk_size;
        let end = std::cmp::min(start + chunk_size, size) - 1;
        let url = url.to_string();
        let tmp = tmp.clone();
        let client = client.clone();
        let pb = pb.clone();
        async move { download_range(&client, &url, &tmp, start, end, pb.as_ref()).await }
    });
    let result = futures::future::try_join_all(tasks).await;

    if let Some(pb) = &pb {
        pb.finish_and_clear();
    }

    match result {
        Ok(_) => {
            publish_downloaded_artifact(&tmp, dest).await?;
            Ok(())
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

/// Streams from the network into a `Vec<u8>` so progress ticks smoothly as bytes arrive. A single `spawn_blocking` per
/// chunk then does the open, seek, and write_all in `std::fs`. This avoids the per-write hop into tokio's blocking pool
/// that `tokio::fs::File::write_all` performs on every ~8 KiB Bytes item.
async fn download_range(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
    start: u64,
    end: u64,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    let resp = client
        .get(url)
        .header("Range", format!("bytes={}-{}", start, end))
        .send()
        .await?;
    if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        anyhow::bail!("range request rejected: HTTP {}", resp.status());
    }
    let mut buf = Vec::with_capacity((end - start + 1) as usize);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if let Some(pb) = progress {
            pb.inc(chunk.len() as u64);
        }
        buf.extend_from_slice(&chunk);
    }
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(&dest)?;
        f.seek(SeekFrom::Start(start))?;
        f.write_all(&buf)?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("blocking write task panicked: {e}"))??;
    Ok(())
}

/// Download a file from `url` to `dest` with a terminal progress bar. If the server provides a `Content-Length` header, a
/// determinate bar is shown with bytes downloaded, total size, and ETA. Otherwise a spinner with a byte counter is used
/// as a fallback.
#[doc(hidden)]
pub async fn download_with_progress(url: &str, dest: &std::path::Path) -> Result<()> {
    crate::ensure_updates_enabled()?;
    // Try parallel byte-range first. Falls through to single-connection on any
    // failure (HEAD missing Content-Length, ranges rejected, partial-fetch error).
    match try_parallel_download(url, dest, true).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            tracing::debug!("parallel download failed, falling back to single connection: {e}")
        }
    }

    let client = download_client()?;
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let total_size = resp.content_length();

    let pb = if let Some(size) = total_size {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {bar:30.cyan/dim} {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("━╸─"),
        );
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} {bytes} downloaded")
                .unwrap(),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    };

    // Stream to a temp file, then rename atomically
    let tmp = tmp_download_path(dest);
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    file.flush().await?;
    drop(file);

    pb.finish_and_clear();

    publish_downloaded_artifact(&tmp, dest).await?;
    Ok(())
}

/// Download a file silently (no progress bar).
#[doc(hidden)]
pub async fn download_silent(url: &str, dest: &std::path::Path) -> Result<()> {
    crate::ensure_updates_enabled()?;
    match try_parallel_download(url, dest, false).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            tracing::debug!("parallel download failed, falling back to single connection: {e}")
        }
    }

    let client = download_client()?;
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let tmp = tmp_download_path(dest);
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    publish_downloaded_artifact(&tmp, dest).await?;
    Ok(())
}

/// Delete `~/.grok/models_cache.json` after a successful update. The cache embeds the binary version, so the new binary
/// would treat it as a miss anyway. Removing it eagerly avoids a wasted disk read and deserialize on first launch.
async fn remove_stale_models_cache() {
    let cache = grok_home().join("models_cache.json");
    match tokio::fs::remove_file(&cache).await {
        Ok(()) => tracing::debug!("removed stale models_cache.json after update"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::debug!("failed to remove stale models cache: {e}"),
    }
}

/// Remove the stale `grok-pager` symlink/binary from `~/.grok/bin/` left by
/// older installations that shipped a separate pager binary.
async fn remove_stale_pager(bin_dir: &std::path::Path) {
    let name = if cfg!(windows) {
        "grok-pager.exe"
    } else {
        "grok-pager"
    };
    let link = bin_dir.join(name);
    if link.exists() || link.is_symlink() {
        let _ = tokio::fs::remove_file(&link).await;
    }
}

async fn download_plain(url: &str, dest: &std::path::Path, with_progress: bool) -> Result<()> {
    if with_progress {
        download_with_progress(url, dest).await
    } else {
        download_silent(url, dest).await
    }
}

#[derive(Clone, Copy)]
enum Codec {
    Zstd,
    Gzip,
}

// Cap decode output so a crafted or corrupt archive cannot expand unbounded and fill the disk
// A real CLI binary is ~170 MiB; 512 MiB leaves 3x headroom
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

async fn download_and_decode(
    url: &str,
    dest: &std::path::Path,
    codec: Codec,
    with_progress: bool,
) -> Result<()> {
    let comp_tmp = tmp_download_path(dest);
    if let Err(e) = download_plain(url, &comp_tmp, with_progress).await {
        let _ = tokio::fs::remove_file(&comp_tmp).await;
        return Err(e);
    }

    let bin_tmp = tmp_download_path(dest);
    let (comp_in, bin_out) = (comp_tmp.clone(), bin_tmp.clone());
    let decoded = tokio::task::spawn_blocking(move || -> Result<()> {
        use std::io::Read as _;
        let src = std::fs::File::open(&comp_in)
            .with_context(|| format!("open compressed download {}", comp_in.display()))?;
        let decoder: Box<dyn std::io::Read> = match codec {
            Codec::Zstd => {
                Box::new(zstd::stream::read::Decoder::new(src).context("init zstd decoder")?)
            }
            Codec::Gzip => Box::new(flate2::read::GzDecoder::new(src)),
        };
        let mut out = std::fs::File::create(&bin_out)
            .with_context(|| format!("create decoded binary {}", bin_out.display()))?;
        let mut capped = decoder.take(MAX_DECODED_BYTES + 1);
        let written = std::io::copy(&mut capped, &mut out).context("decode")?;
        if written > MAX_DECODED_BYTES {
            anyhow::bail!("decoded artifact exceeds the {MAX_DECODED_BYTES}-byte cap");
        }
        Ok(())
    })
    .await;
    let _ = tokio::fs::remove_file(&comp_tmp).await;

    match decoded {
        Ok(Ok(())) => publish_downloaded_artifact(&bin_tmp, dest).await,
        Ok(Err(e)) => {
            let _ = tokio::fs::remove_file(&bin_tmp).await;
            Err(e)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&bin_tmp).await;
            Err(anyhow::anyhow!("decode task panicked: {e}"))
        }
    }
}

/// Object-name candidates in fetch order; on Windows the `.exe` name comes first.
fn cli_object_candidates(object_name: &str, windows: bool) -> Vec<String> {
    if windows {
        vec![format!("{object_name}.exe"), object_name.to_string()]
    } else {
        vec![object_name.to_string()]
    }
}

/// Download a CLI object from GCS, preferring a `.zst`/`.gz` sidecar over the plain object.
async fn download_cli_artifact_from_gcs(
    gcs_base_url: &str,
    object_name: &str,
    dest: &std::path::Path,
    with_progress: bool,
) -> Result<()> {
    let base = gcs_base_url.trim_end_matches('/');
    let names = cli_object_candidates(object_name, cfg!(windows));

    let mut last_err = None;
    for name in &names {
        for (suffix, codec) in [("zst", Codec::Zstd), ("gz", Codec::Gzip)] {
            let url = format!("{base}/{name}.{suffix}");
            match download_and_decode(&url, dest, codec, with_progress).await {
                Ok(()) => return Ok(()),
                Err(e) => tracing::debug!("compressed {name}.{suffix} unusable, trying next: {e}"),
            }
        }
        match download_plain(&format!("{base}/{name}"), dest, with_progress).await {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no artifact at {base}/{object_name}")))
}

/// Returns the version that was actually activated.
async fn install_internal(target: Option<&str>, update_config: &UpdateConfig) -> Result<String> {
    let bases = crate::version::cli_base_urls();
    let base_refs: Vec<&str> = bases.iter().map(String::as_str).collect();
    install_internal_from_bases(target, update_config, &base_refs).await
}

/// Download-phase side effects (download dir creation, binary fetch) are idempotent, so retrying with a different base
/// after a partial failure is safe. Its failures are not base-dependent, so they abort the install instead of triggering
/// a pointless re-download from the next base.
#[doc(hidden)]
pub async fn install_internal_from_bases(
    target: Option<&str>,
    update_config: &UpdateConfig,
    bases: &[&str],
) -> Result<String> {
    crate::ensure_updates_enabled()?;
    let mut last_err: Option<anyhow::Error> = None;
    for (i, base) in bases.iter().enumerate() {
        match download_verified_from_base(target, update_config, base).await {
            Ok(download) => {
                return activate_verified_download(&download)
                    .await
                    .map(|()| download.version)
                    .map_err(|e| InstallPhaseError::Activate(e).into());
            }
            Err(e) if e.is::<SmokeTestFailure>() => {
                // Same published artifact on every base; retrying will not change a --version timeout or crash
                // Left unwrapped so telemetry classification sees the typed failure
                return Err(e);
            }
            Err(e) => {
                let e = wrap_download_err(e);
                if i + 1 < bases.len() {
                    tracing::warn!(
                        "install via {} failed ({:#}); trying next base URL",
                        base,
                        e
                    );
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no CLI base URLs to try")))
}

/// First-launch of a freshly downloaded macOS binary can exceed 10s (Rosetta AOT and Gatekeeper on ~140MB).
/// A short cap would fail a good artifact.
const SMOKE_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Retry budget for exec attempts refused with ETXTBSY. The failure window is normally the microseconds another spawn in
/// this process sits between fork and exec (see [`smoke_test_binary`]). On a heavily loaded machine that window can
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

async fn smoke_test_binary(binary_path: &std::path::Path) -> Result<String, SmokeTestFailure> {
    // ETXTBSY race: while a concurrent updater in this process is between
    // fork and exec (pre_exec in detach_command forces the fork/exec path),
    // its child briefly holds every open fd — including the write-side fd of
    // a download that has just been renamed onto `binary_path`. Exec'ing a
    // binary whose inode is still open for write fails with ETXTBSY even
    // though the file is complete and healthy, so retry instead of failing
    // the install (and deleting a racer's freshly installed binary).
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
            Err(_) => return Err(SmokeTestFailure::Timeout),
            Ok(Ok(output)) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            Ok(Ok(output)) => {
                let status = output
                    .status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| output.status.to_string());
                let stderr = truncate_err(&String::from_utf8_lossy(&output.stderr), 400);
                return Err(SmokeTestFailure::NonZero { status, stderr });
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                last_spawn = e.to_string();
                if attempt < SMOKE_TEST_ETXTBSY_ATTEMPTS {
                    tokio::time::sleep(SMOKE_TEST_ETXTBSY_BACKOFF * attempt).await;
                }
            }
            Ok(Err(e)) => return Err(SmokeTestFailure::Spawn(e.to_string())),
        }
    }
    // Reached only when every attempt hit ETXTBSY; `last_spawn` holds the final spawn error
    Err(SmokeTestFailure::Spawn(last_spawn))
}

#[cfg(feature = "community-build")]
fn community_version_output_matches(output: &str, expected_version: &str) -> bool {
    let expected_prefix = format!("{} {expected_version} (", xai_grok_product::CLI_NAME);
    output
        .lines()
        .next()
        .is_some_and(|line| line.trim().starts_with(&expected_prefix))
}

/// Test-only entry point: same as [`install_internal`] but reads from
/// `gcs_base_url` instead of the hardcoded GCS bucket. Persists installer
/// config and writes to `~/.grok/bin/`, so callers must isolate
/// `GROK_HOME`.
#[doc(hidden)]
pub async fn install_internal_from_base(
    target: Option<&str>,
    update_config: &UpdateConfig,
    gcs_base_url: &str,
) -> Result<String> {
    crate::ensure_updates_enabled()?;
    let download = download_verified_from_base(target, update_config, gcs_base_url)
        .await
        .map_err(wrap_download_err)?;
    activate_verified_download(&download)
        .await
        .map_err(|e| InstallPhaseError::Activate(e).into())
        .map(|()| download.version)
}

/// A downloaded and smoke-tested binary in `~/.grok/downloads/`, not yet
/// activated as the managed `grok`/`agent`.
struct VerifiedDownload {
    version: String,
    binary_path: std::path::PathBuf,
    /// Windows: the grove hook exes and MinGit archive fetched for this release (empty elsewhere).
    #[cfg_attr(not(windows), allow(dead_code))]
    payload: windows_payload::Payload,
}

/// Base-dependent install phase: resolve the version (per base when no target is pinned), download the binary, and smoke-test it.
/// Network / fetch failures here are worth retrying against another base URL.
/// [`SmokeTestFailure`] is not; see [`install_internal_from_bases`].
async fn download_verified_from_base(
    target: Option<&str>,
    update_config: &UpdateConfig,
    gcs_base_url: &str,
) -> Result<VerifiedDownload> {
    let (os, arch) = detect_platform()?;
    let platform = format!("{}-{}", os, arch);

    let version = match target {
        Some(v) => {
            Version::parse(v).map_err(|_| anyhow::anyhow!("invalid version format: '{}'", v))?;
            v.to_string()
        }
        None => {
            crate::version::fetch_gcs_version_from_base(&update_config.channel, gcs_base_url)
                .await?
        }
    };

    let grok_home = grok_home();
    let download_dir = grok_home.join("downloads");
    tokio::fs::create_dir_all(&download_dir).await?;

    let binary_name = format!("grok-{}-{}", version, platform);
    let binary_path = download_dir.join(&binary_name);

    eprintln!("  Downloading grok v{} ({})...", version, platform);

    // The downloaded binary is already +x (see `publish_downloaded_artifact`)
    download_cli_artifact_from_gcs(gcs_base_url, &binary_name, &binary_path, true).await?;

    // Smoke-test: run the binary before activating it
    // A truncated or corrupt download is caught here and never becomes the active grok
    let smoke_span = xai_grok_telemetry::region::Region::from_span(tracing::info_span!(
        "update.smoke_test",
        elapsed_ms = tracing::field::Empty,
    ));
    let smoke_started = Instant::now();
    let smoke_result = smoke_test_binary(&binary_path).await;
    smoke_span
        .span()
        .record("elapsed_ms", smoke_started.elapsed().as_millis() as i64);
    smoke_span.close();
    if let Err(fail) = smoke_result {
        let _ = tokio::fs::remove_file(&binary_path).await;
        // No prefix: run_install_script's wrap adds "Auto-update failed:".
        return Err(fail.into());
    }

    // Best-effort and base-dependent, so it belongs to this phase; a miss never fails the install.
    #[cfg(windows)]
    let payload = windows_payload::download(gcs_base_url, &version, &platform, &download_dir).await;
    #[cfg(not(windows))]
    let payload = windows_payload::Payload::default();

    Ok(VerifiedDownload {
        version,
        binary_path,
        payload,
    })
}

/// Local activation phase: swap the managed bin links to the downloaded binary and finish bookkeeping.
/// Nothing here depends on which base URL served the download, so callers must not retry another base on failure.
async fn activate_verified_download(download: &VerifiedDownload) -> Result<()> {
    let activate_span = xai_grok_telemetry::region::Region::from_span(tracing::info_span!(
        "update.install",
        elapsed_ms = tracing::field::Empty,
    ));
    let activate_started = Instant::now();
    let grok_home = grok_home();
    let download_dir = grok_home.join("downloads");
    let bin_dir = grok_home.join("bin");
    tokio::fs::create_dir_all(&bin_dir).await?;

    // Atomic swap of ~/.grok/bin/{grok,agent} -> downloaded binary.
    let link_path = swap_managed_bin_links(&download.binary_path, &bin_dir).await?;

    remove_stale_pager(&bin_dir).await;

    // Hook exes beside grok.exe and the bundled MinGit; grok is already live, so a failure here is only logged.
    #[cfg(windows)]
    windows_payload::activate(&download.payload, &bin_dir, &download.version).await;

    eprintln!();

    // Current, N-1, and any leftover a live process is still executing.
    cleanup_old_downloads(&download_dir, "grok", &download.version).await;
    cleanup_old_downloads(&download_dir, "grok-pager", &download.version).await;

    // Persist installer to config.toml so future runs auto-detect internal.
    let _ = config::update_config(|st| {
        st.cli.installer = Some("internal".to_string());
    })
    .await;

    regenerate_completions(&link_path, &grok_home).await;

    activate_span
        .span()
        .record("elapsed_ms", activate_started.elapsed().as_millis() as i64);
    Ok(())
}

/// Regenerate shell completions after a binary update (best-effort). Spawns the newly-installed binary with `completions
/// <shell>` for each supported shell and writes the output to the standard completion paths. Failures are silently
/// ignored: completions are a nice-to-have, not a requirement for a successful update.
async fn regenerate_completions(binary: &std::path::Path, grok_home: &std::path::Path) {
    // Derive $HOME independently: grok_home may be overridden via GROK_HOME env var, so grok_home.parent() isn't necessarily the user's home dir
    let user_home = xai_dirs::home_dir().unwrap_or_default();

    let completions: &[(&str, std::path::PathBuf)] = &[
        ("bash", grok_home.join("completions/bash/grok.bash")),
        ("zsh", grok_home.join("completions/zsh/_grok")),
        ("fish", user_home.join(".config/fish/completions/grok.fish")),
    ];

    for (shell, dest) in completions {
        if let Some(parent) = dest.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(["completions", shell])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        xai_grok_tools::util::detach_command(&mut cmd);
        let Ok(output) = cmd.output().await else {
            continue;
        };
        if output.status.success() && !output.stdout.is_empty() {
            let _ = tokio::fs::write(dest, &output.stdout).await;
        }
    }
}

/// When both paths share a grandparent (e.g. `~/.grok/bin/grok` and `~/.grok/downloads/grok-0.1.203-linux-x86_64`),
/// returns a relative path like `../downloads/grok-0.1.203-linux-x86_64`. Relative symlinks survive Docker bind-mounts
/// where `~/.grok/` is mapped into a container with a different `$HOME` (and thus a different absolute prefix).
#[cfg(unix)]
fn relative_symlink_target(target: &std::path::Path, link: &std::path::Path) -> std::path::PathBuf {
    let (Some(target_parent), Some(link_parent)) = (target.parent(), link.parent()) else {
        return target.to_path_buf();
    };
    // Same directory: just the filename (e.g. grok-latest -> grok-0.1.203-…)
    if target_parent == link_parent
        && let Some(name) = target.file_name()
    {
        return std::path::PathBuf::from(name);
    }
    // Sibling directories: ../target_dir/filename (e.g. bin/grok -> ../downloads/grok-…)
    if let (Some(tp), Some(lp)) = (target_parent.parent(), link_parent.parent())
        && tp == lp
        && let (Some(dir_name), Some(file_name)) = (target_parent.file_name(), target.file_name())
    {
        return std::path::Path::new("..").join(dir_name).join(file_name);
    }
    target.to_path_buf()
}

/// The bootstrap installers (`install.sh`, `install.ps1`, `install-enterprise.sh`) maintain `grok` and `agent` in
/// lockstep, and so must the updater. Otherwise `grok update` leaves `agent` pinned at the previous version. Any earlier
/// successful swaps are rolled back if a later one fails, including *removing* a link that didn't exist before.
async fn swap_managed_bin_links(
    binary_path: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let grok_name = if cfg!(windows) { "grok.exe" } else { "grok" };
    let agent_name = if cfg!(windows) { "agent.exe" } else { "agent" };
    swap_managed_entrypoint_links(binary_path, bin_dir, grok_name, agent_name, None).await
}

#[cfg(all(unix, feature = "community-build"))]
fn managed_community_target_path(target: &std::path::Path) -> bool {
    !target.is_absolute()
        && target.parent() == Some(std::path::Path::new("../grok-zh-downloads"))
        && target
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(crate::version::version_from_current_community_unix_binary_name)
            .is_some()
}

#[cfg(all(unix, feature = "community-build"))]
async fn validate_existing_community_entrypoint(
    path: &std::path::Path,
    allow_canonical_indirection: bool,
) -> Result<()> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("checking entry point {}", path.display()));
        }
    };
    if !metadata.file_type().is_symlink() {
        anyhow::bail!(
            "refusing to replace an unmanaged entry point: {}",
            path.display()
        );
    }
    let target = tokio::fs::read_link(path)
        .await
        .with_context(|| format!("reading entry point {}", path.display()))?;
    if allow_canonical_indirection && target == std::path::Path::new("grok-zh") {
        return Ok(());
    }
    if !managed_community_target_path(&target) {
        anyhow::bail!(
            "refusing to replace an unmanaged community link: {} -> {}",
            path.display(),
            target.display()
        );
    }
    let resolved = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("entry point has no parent: {}", path.display()))?
        .join(&target);
    let target_metadata = tokio::fs::symlink_metadata(&resolved)
        .await
        .with_context(|| format!("checking managed target {}", resolved.display()))?;
    if target_metadata.file_type().is_symlink() || !target_metadata.is_file() {
        anyhow::bail!(
            "managed community target is not a regular file: {}",
            resolved.display()
        );
    }
    Ok(())
}

#[cfg(all(unix, feature = "community-build"))]
async fn swap_community_bin_links(
    binary_path: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    // `agent-zh` follows the canonical `grok-zh` link instead of duplicating
    // its version target. Concurrent updaters can then race only on one
    // version pointer; both command names always converge on the same inode.
    let binary_target = relative_symlink_target(binary_path, &bin_dir.join("grok-zh"));
    if !managed_community_target_path(&binary_target) {
        anyhow::bail!(
            "refusing to activate a community binary outside the managed download directory: {}",
            binary_path.display()
        );
    }
    let binary_metadata = tokio::fs::symlink_metadata(binary_path)
        .await
        .with_context(|| format!("checking community binary {}", binary_path.display()))?;
    if binary_metadata.file_type().is_symlink() || !binary_metadata.is_file() {
        anyhow::bail!(
            "community binary is not a regular file: {}",
            binary_path.display()
        );
    }
    validate_existing_community_entrypoint(&bin_dir.join("grok-zh"), false).await?;
    validate_existing_community_entrypoint(&bin_dir.join("agent-zh"), true).await?;

    swap_managed_entrypoint_links(
        binary_path,
        bin_dir,
        "grok-zh",
        "agent-zh",
        Some(std::path::Path::new("grok-zh")),
    )
    .await
}

async fn swap_managed_entrypoint_links(
    binary_path: &std::path::Path,
    bin_dir: &std::path::Path,
    primary_name: &str,
    secondary_name: &str,
    secondary_target: Option<&std::path::Path>,
) -> Result<std::path::PathBuf> {
    let primary_link = bin_dir.join(primary_name);
    let secondary_source = secondary_target
        .map(|target| bin_dir.join(target))
        .unwrap_or_else(|| binary_path.to_path_buf());
    let pairs = [
        (binary_path.to_path_buf(), primary_link.clone()),
        (secondary_source, bin_dir.join(secondary_name)),
    ];
    replace_managed_bins(&pairs).await?;
    Ok(primary_link)
}

/// Point every `dest` in `pairs` at its `src` (a symlink on Unix, a copy through
/// `windows_replace_exe` on Windows) as one unit: every `dest` is captured
/// up-front, then replaced in order, and a failure restores the completed ones in
/// reverse, including removing a `dest` that did not exist before.
async fn replace_managed_bins(pairs: &[(std::path::PathBuf, std::path::PathBuf)]) -> Result<()> {
    // Capture every dest up-front so a later capture failure can't strand an earlier one mid-swap
    let mut captured: Vec<LinkRollback> = Vec::with_capacity(pairs.len());
    for (_, dest) in pairs {
        match LinkRollback::capture(dest).await {
            Ok(rb) => captured.push(rb),
            Err(e) => {
                // Nothing swapped yet; drop any Windows .rollback.bak files.
                for prior in &captured {
                    prior.cleanup().await;
                }
                return Err(e)
                    .with_context(|| format!("capturing rollback state for {}", dest.display()));
            }
        }
    }

    let mut completed: Vec<(&LinkRollback, Option<std::path::PathBuf>)> =
        Vec::with_capacity(captured.len());
    for (i, ((src, dest), rollback)) in pairs.iter().zip(captured.iter()).enumerate() {
        #[cfg(unix)]
        let swap_result = {
            let rel_target = relative_symlink_target(src, dest);
            atomic_symlink_swap(&rel_target, dest).await
        };
        #[cfg(unix)]
        let rollback_expected = Some(relative_symlink_target(src, dest));
        #[cfg(windows)]
        let swap_result = windows_replace_exe(src, dest).await;
        #[cfg(windows)]
        let rollback_expected = None;
        #[cfg(not(any(unix, windows)))]
        let swap_result: Result<()> = {
            // No managed bin layout on this target; no-op.
            let _ = (src, dest);
            Ok(())
        };
        #[cfg(not(any(unix, windows)))]
        let rollback_expected = None;

        match swap_result {
            Ok(()) => completed.push((rollback, rollback_expected)),
            Err(e) => {
                // Restore each successful swap in reverse. On restore
                // failure keep the .rollback.bak as a recovery artifact
                // (Windows only) and warn!; the swap error propagates so
                // `reinstall_hint` is the user-visible message.
                for (prior, expected) in completed.iter().rev() {
                    if let Err(restore_err) = prior.restore(expected.as_deref()).await {
                        let backup_note = prior.backup_path().map_or(String::new(), |p| {
                            format!(" (prior binary preserved at {})", p.display())
                        });
                        tracing::warn!(
                            "failed to roll back managed bin link {}: {restore_err:#}{backup_note}",
                            prior.link_path().display(),
                        );
                        continue;
                    }
                    prior.cleanup().await;
                }
                // Failed swap had no active state to restore; drop its backup.
                rollback.cleanup().await;
                // Drop backups for never-attempted later captures (Windows orphans).
                for later in captured.get(i + 1..).unwrap_or(&[]) {
                    later.cleanup().await;
                }
                return Err(e).with_context(|| format!("replacing {}", dest.display()));
            }
        }
    }

    for cap in &captured {
        cap.cleanup().await;
    }
    Ok(())
}

/// Snapshot of a managed-bin link's prior state for rollback in [`replace_managed_bins`].
/// `Absent` vs `Present` is discriminated up front via `symlink_metadata` so capture errors never get misread as "link was absent".
enum LinkRollback {
    /// Link was absent before the swap; rollback removes the one we created.
    Absent { link_path: std::path::PathBuf },
    /// Link existed before the swap; rollback restores its prior contents.
    Present {
        link_path: std::path::PathBuf,
        /// Unix: prior symlink target (relative or absolute).
        #[cfg(unix)]
        prior_target: std::path::PathBuf,
        /// Windows: `.rollback.bak` copy of the previous binary.
        #[cfg(windows)]
        backup_path: std::path::PathBuf,
    },
}

impl LinkRollback {
    async fn capture(link_path: &std::path::Path) -> Result<Self> {
        let lp = link_path.to_path_buf();

        // `symlink_metadata` (lstat) handles valid symlinks, broken symlinks, and regular files alike
        // Any IO error other than NotFound aborts the swap before mutation
        match tokio::fs::symlink_metadata(&lp).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LinkRollback::Absent { link_path: lp });
            }
            Err(e) => {
                return Err(e).with_context(|| format!("stat {} before swap", lp.display()));
            }
        }

        #[cfg(unix)]
        {
            let prior_target = tokio::fs::read_link(&lp)
                .await
                .with_context(|| format!("reading prior symlink target {}", lp.display()))?;
            Ok(LinkRollback::Present {
                link_path: lp,
                prior_target,
            })
        }
        #[cfg(windows)]
        {
            // The backup name comes from `unique_temp_sibling` (PID and sequence), so concurrent updaters can't clobber each other's backups
            let backup_path = unique_temp_sibling(&lp, "rollback.bak");
            tokio::fs::copy(&lp, &backup_path).await.with_context(|| {
                format!(
                    "backing up {} to {} before swap",
                    lp.display(),
                    backup_path.display(),
                )
            })?;
            Ok(LinkRollback::Present {
                link_path: lp,
                backup_path,
            })
        }
    }

    fn link_path(&self) -> &std::path::Path {
        match self {
            LinkRollback::Absent { link_path } => link_path,
            LinkRollback::Present { link_path, .. } => link_path,
        }
    }

    /// Path to the on-disk backup (Windows only; Unix is in-memory).
    #[cfg(windows)]
    fn backup_path(&self) -> Option<&std::path::Path> {
        match self {
            LinkRollback::Present { backup_path, .. } => Some(backup_path),
            LinkRollback::Absent { .. } => None,
        }
    }
    #[cfg(unix)]
    fn backup_path(&self) -> Option<&std::path::Path> {
        None
    }

    async fn restore(&self, expected_current: Option<&std::path::Path>) -> Result<()> {
        #[cfg(not(unix))]
        let _ = expected_current;
        #[cfg(unix)]
        if let Some(expected) = expected_current {
            match tokio::fs::read_link(self.link_path()).await {
                Ok(current) if current == expected => {}
                Ok(_) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "checking current link before rollback {}",
                            self.link_path().display()
                        )
                    });
                }
            }
        }
        match self {
            LinkRollback::Absent { link_path } => {
                // Remove the link we created
                // NotFound (someone else cleaned up) is fine; anything else is a real failure
                match tokio::fs::remove_file(link_path).await {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e).with_context(|| {
                        format!("removing rolled-back link {}", link_path.display())
                    }),
                }
            }
            #[cfg(unix)]
            LinkRollback::Present {
                link_path,
                prior_target,
            } => atomic_symlink_swap(prior_target, link_path)
                .await
                .with_context(|| {
                    format!("restoring prior symlink target for {}", link_path.display())
                }),
            #[cfg(windows)]
            LinkRollback::Present {
                link_path,
                backup_path,
            } => {
                // Route through `windows_replace_exe`
                // Rollback then inherits the same ERROR_SHARING_VIOLATION rename-aside fallback as the forward path
                windows_replace_exe(backup_path, link_path)
                    .await
                    .with_context(|| {
                        format!(
                            "restoring {} from {}",
                            link_path.display(),
                            backup_path.display()
                        )
                    })
            }
        }
    }

    async fn cleanup(&self) {
        #[cfg(windows)]
        if let LinkRollback::Present { backup_path, .. } = self {
            let _ = tokio::fs::remove_file(backup_path).await;
        }
        #[cfg(unix)]
        let _ = self; // no on-disk backup on Unix
    }
}

/// Creates a temporary symlink next to `link_path`, then renames it over the old symlink. This avoids the
/// remove-then-create race where the path briefly doesn't exist, and never deletes the old target file. On macOS
/// (especially Apple Silicon), deleting a binary that a running process has mmap'd causes SIGKILL.
#[cfg(unix)]
async fn atomic_symlink_swap(target: &std::path::Path, link_path: &std::path::Path) -> Result<()> {
    // Per-racer temp name: a shared one makes remove_file then symlink racy (EEXIST, or ENOENT when another racer renames the link away)
    sweep_stale_tmp_links(link_path, STALE_TMP_AGE).await;
    let tmp_link = unique_temp_sibling(link_path, "tmp-link");
    let _ = tokio::fs::remove_file(&tmp_link).await;
    tokio::fs::symlink(target, &tmp_link).await?;
    tokio::fs::rename(&tmp_link, link_path).await?;
    Ok(())
}

/// Remove `<link>.*.tmp-link` siblings left by a swap that crashed between symlink and rename.
/// Only those older than `max_age` are removed, so a concurrent racer's in-flight link is never deleted out from under it.
#[cfg(unix)]
async fn sweep_stale_tmp_links(link_path: &std::path::Path, max_age: Duration) {
    let (Some(dir), Some(name)) = (
        link_path.parent(),
        link_path.file_name().and_then(|n| n.to_str()),
    ) else {
        return;
    };
    let prefix = format!("{name}.");
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let fname = entry.file_name();
        let Some(fname) = fname.to_str() else {
            continue;
        };
        if !fname.starts_with(&prefix) || !fname.ends_with(".tmp-link") {
            continue;
        }
        let stale = tokio::fs::symlink_metadata(entry.path())
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > max_age);
        if stale {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

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

fn installer_manages_bin_entrypoints(installer: &str) -> bool {
    if matches!(installer, "internal" | "gh-release") {
        return true;
    }
    #[cfg(feature = "community-build")]
    {
        return installer == crate::community_release::COMMUNITY_INSTALLER;
    }
    #[cfg(not(feature = "community-build"))]
    false
}

#[cfg_attr(not(any(unix, windows)), allow(clippy::unused_async))]
async fn heal_managed_install(installer: &str) {
    if !installer_manages_bin_entrypoints(installer) {
        return;
    }

    #[cfg(any(unix, windows))]
    {
        #[cfg(feature = "community-build")]
        if installer == crate::community_release::COMMUNITY_INSTALLER {
            #[cfg(any(
                all(
                    target_os = "macos",
                    any(target_arch = "aarch64", target_arch = "x86_64")
                ),
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64"),
                    target_env = "gnu"
                )
            ))]
            let bin_dir = {
                let Some(home) = xai_dirs::resolve_grok_home() else {
                    return;
                };
                if let Err(error) = validate_community_home_path(&home) {
                    tracing::warn!("refusing to reconcile an unsafe community home: {error:#}");
                    return;
                }
                let bin_dir = home.join("bin");
                let download_dir = home.join("grok-zh-downloads");
                for dir in [&home, &bin_dir, &download_dir] {
                    if let Err(error) = ensure_private_community_dir(dir) {
                        tracing::warn!(
                            path = %dir.display(),
                            "refusing to reconcile an unsafe community install directory: {error:#}"
                        );
                        return;
                    }
                }
                bin_dir
            };
            #[cfg(all(
                unix,
                not(any(
                    all(
                        target_os = "macos",
                        any(target_arch = "aarch64", target_arch = "x86_64")
                    ),
                    all(
                        target_os = "linux",
                        any(target_arch = "x86_64", target_arch = "aarch64"),
                        target_env = "gnu"
                    )
                ))
            ))]
            let bin_dir = grok_home().join("bin");
            #[cfg(unix)]
            reconcile_secondary_to_fixed_target(&bin_dir, "grok-zh", "agent-zh").await;
            return;
        }

        let bin_dir = grok_home().join("bin");
        #[cfg(unix)]
        reconcile_agent_to_grok(&bin_dir).await;

        #[cfg(windows)]
        reconcile_agent_exe_to_grok(&bin_dir).await;
    }
}

#[cfg(unix)]
async fn reconcile_agent_to_grok(bin_dir: &std::path::Path) {
    reconcile_secondary_to_primary(bin_dir, "grok", "agent").await;
}

#[cfg(all(unix, feature = "community-build"))]
async fn reconcile_secondary_to_fixed_target(
    bin_dir: &std::path::Path,
    primary_name: &str,
    secondary_name: &str,
) {
    let primary_link = bin_dir.join(primary_name);
    let secondary_link = bin_dir.join(secondary_name);
    if let Err(error) = validate_existing_community_entrypoint(&primary_link, false).await {
        tracing::warn!(
            "refusing to reconcile {secondary_name}: canonical {primary_name} is unmanaged: {error:#}"
        );
        return;
    }
    if tokio::fs::metadata(&primary_link).await.is_err() {
        return;
    }
    let desired_target = std::path::Path::new(primary_name);
    if let Ok(current_target) = tokio::fs::read_link(&secondary_link).await
        && current_target == desired_target
    {
        return;
    }
    if let Err(error) = validate_existing_community_entrypoint(&secondary_link, true).await {
        tracing::warn!(
            "refusing to replace unmanaged {secondary_name} during reconciliation: {error:#}"
        );
        return;
    }
    match atomic_symlink_swap(desired_target, &secondary_link).await {
        Ok(()) => tracing::info!(
            primary = primary_name,
            secondary = secondary_name,
            "reconciled secondary bin symlink to canonical primary entry point"
        ),
        Err(error) => tracing::warn!(
            "failed to reconcile {secondary_name} bin symlink to {primary_name}: {error:#}"
        ),
    }
}

#[cfg(unix)]
async fn reconcile_secondary_to_primary(
    bin_dir: &std::path::Path,
    primary_name: &str,
    secondary_name: &str,
) {
    let primary_link = bin_dir.join(primary_name);
    let secondary_link = bin_dir.join(secondary_name);

    let Ok(primary_target) = tokio::fs::read_link(&primary_link).await else {
        return;
    };
    if tokio::fs::metadata(&primary_link).await.is_err() {
        return;
    }
    if let Ok(secondary_target) = tokio::fs::read_link(&secondary_link).await
        && secondary_target == primary_target
    {
        return;
    }
    match atomic_symlink_swap(&primary_target, &secondary_link).await {
        Ok(()) => tracing::info!(
            primary = primary_name,
            secondary = secondary_name,
            target = %primary_target.display(),
            "reconciled secondary bin symlink to primary target"
        ),
        Err(e) => tracing::warn!(
            "failed to reconcile {secondary_name} bin symlink to {primary_name}: {e:#}"
        ),
    }
}

#[cfg(windows)]
async fn reconcile_agent_exe_to_grok(bin_dir: &std::path::Path) {
    let grok_exe = bin_dir.join("grok.exe");
    let agent_exe = bin_dir.join("agent.exe");

    if tokio::fs::metadata(&grok_exe).await.is_err() {
        return;
    }
    match agent_exe_differs(&grok_exe, &agent_exe).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            tracing::debug!("agent.exe reconcile: compare failed: {e:#}");
            return;
        }
    }
    match windows_replace_exe(&grok_exe, &agent_exe).await {
        Ok(()) => tracing::info!("reconciled agent.exe to grok.exe"),
        Err(e) => tracing::warn!("failed to reconcile agent.exe to grok.exe: {e:#}"),
    }
}

#[cfg(windows)]
async fn agent_exe_differs(
    grok: &std::path::Path,
    agent: &std::path::Path,
) -> std::io::Result<bool> {
    use tokio::io::{AsyncReadExt, BufReader};
    let grok_len = tokio::fs::metadata(grok).await?.len();
    match tokio::fs::metadata(agent).await {
        Ok(m) if m.len() != grok_len => return Ok(true),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    }
    let mut rg = BufReader::new(tokio::fs::File::open(grok).await?);
    let mut ra = BufReader::new(tokio::fs::File::open(agent).await?);
    let mut bg = [0u8; 64 * 1024];
    let mut ba = [0u8; 64 * 1024];
    loop {
        let n = rg.read(&mut bg).await?;
        if n == 0 {
            return Ok(false);
        }
        let Some(dst) = ba.get_mut(..n) else {
            return Ok(true);
        };
        ra.read_exact(dst).await?;
        if bg.get(..n) != ba.get(..n) {
            return Ok(true);
        }
    }
}

/// Download a single asset from a GitHub release via `gh release download`.
async fn gh_release_download(tag: &str, pattern: &str, dest: &std::path::Path) -> Result<()> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} Downloading from GitHub Releases...")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut cmd = tokio::process::Command::new("gh");
    cmd.args([
        "release",
        "download",
        tag,
        "--repo",
        crate::version::GH_RELEASE_REPO,
        "--pattern",
        pattern,
        "--output",
        &dest.to_string_lossy(),
        "--clobber",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
    xai_grok_tools::util::detach_command(&mut cmd);
    cmd.envs(xai_grok_tools::util::pager_env());
    let output = cmd.output().await?;

    pb.finish_and_clear();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "gh release download failed for {} tag {} from {}: {}",
            pattern,
            tag,
            crate::version::GH_RELEASE_REPO,
            stderr.trim()
        );
    }
    Ok(())
}

/// Download and install grok from GitHub Releases (xai-org-shared/grok-build). Uses `gh release download` to fetch the
/// binary matching the current platform. This works anywhere the `gh` CLI is authenticated, without needing npm or
/// internal network access.
async fn install_gh_release(target: Option<&str>) -> Result<()> {
    let (os, arch) = detect_platform()?;
    let platform = format!("{}-{}", os, arch);

    let version = match target {
        Some(v) => v.to_string(),
        None => crate::version::fetch_gh_release_version("stable").await?,
    };

    let grok_home = grok_home();
    let download_dir = grok_home.join("downloads");
    let bin_dir = grok_home.join("bin");
    tokio::fs::create_dir_all(&download_dir).await?;
    tokio::fs::create_dir_all(&bin_dir).await?;

    let binary_name = format!("grok-{}-{}", version, platform);
    let binary_path = download_dir.join(&binary_name);
    let tag = format!("v{}", version);

    eprintln!(
        "  Downloading grok v{} ({}) from GitHub Releases...",
        version, platform
    );

    gh_release_download(&tag, &binary_name, &binary_path).await?;

    // chmod +x
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).await?;
    }

    // Atomic swap of ~/.grok/bin/{grok,agent} -> downloaded binary.
    swap_managed_bin_links(&binary_path, &bin_dir).await?;

    // Update grok-latest -> versioned binary so any existing symlinks that route
    // through it (e.g. /usr/local/bin/grok -> ~/.grok/downloads/grok-latest)
    // resolve to the newly installed version.
    #[cfg(unix)]
    {
        let latest_path = download_dir.join("grok-latest");
        let rel_target = relative_symlink_target(&binary_path, &latest_path);
        if let Err(e) = atomic_symlink_swap(&rel_target, &latest_path).await {
            tracing::warn!("Failed to update grok-latest symlink: {e}");
        }
    }

    // Also update /usr/local/bin/{grok,agent} if either points directly into
    // ~/.grok/downloads/ (legacy layout — skips the grok-latest indirection).
    // Permission errors are ignored
    #[cfg(unix)]
    for name in ["grok", "agent"] {
        let system_link = std::path::PathBuf::from(format!("/usr/local/bin/{name}"));
        if let Ok(existing_target) = tokio::fs::read_link(&system_link).await {
            let target_str = existing_target.to_string_lossy();
            if target_str.contains(".grok/downloads/") && !target_str.ends_with("grok-latest") {
                let _ = atomic_symlink_swap(&binary_path, &system_link).await;
            }
        }
    }

    remove_stale_pager(&bin_dir).await;

    eprintln!();

    // Current, N-1, and any leftover a live process is still executing.
    cleanup_old_downloads(&download_dir, "grok", &version).await;
    cleanup_old_downloads(&download_dir, "grok-pager", &version).await;

    // Persist installer to config.toml so future runs auto-detect gh-release.
    let _ = config::update_config(|st| {
        st.cli.installer = Some("gh-release".to_string());
    })
    .await;

    Ok(())
}

/// Creates a temporary .npmrc file with the NPM token if present.
/// Returns the path to the created file, or None if no token was set.
fn create_temp_npmrc(npm_registry: Option<&str>) -> Result<Option<std::path::PathBuf>> {
    if let Ok(token) = std::env::var("NPM_TOKEN") {
        let token = token.trim();
        if !token.is_empty() {
            let dir = std::env::temp_dir();
            let npmrc_path = dir.join(format!(".npmrc-{}-install", std::process::id()));
            let registry_host = npm_registry
                .and_then(|r| reqwest::Url::parse(r).ok())
                .map(|u| {
                    let host = u.host_str().unwrap_or("registry.npmjs.org");
                    let port_suffix = u.port().map(|p| format!(":{}", p)).unwrap_or_default();
                    format!("{}{}{}", host, port_suffix, u.path().trim_end_matches('/'))
                })
                .unwrap_or_else(|| "registry.npmjs.org".to_string());
            let npmrc_content = format!("//{}/:_authToken={}\n", registry_host, token);
            std::fs::write(&npmrc_path, npmrc_content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&npmrc_path, std::fs::Permissions::from_mode(0o600))?;
            }
            return Ok(Some(npmrc_path));
        }
    }
    Ok(None)
}

/// Check if other grok processes are running (macOS only). Any grok process running from that vendored path will be
/// SIGKILL'd by the kernel. macOS (Apple Silicon in particular) can no longer verify the code signature of the mmap'd
/// executable pages once the backing inode is unlinked.
#[cfg(target_os = "macos")]
fn warn_if_other_grok_processes_running() {
    let my_pid = std::process::id().to_string();
    let mut cmd = Command::new("pgrep");
    cmd.args(["-f", "grok"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    xai_grok_tools::util::detach_std_command(&mut cmd);
    if let Ok(output) = cmd.output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let other_pids: Vec<&str> = stdout
            .lines()
            .map(|l| l.trim())
            .filter(|pid| !pid.is_empty() && *pid != my_pid)
            .collect();
        if !other_pids.is_empty() {
            eprintln!(
                "  ⚠ Warning: {} other grok process(es) detected.",
                other_pids.len()
            );
            eprintln!("    Processes running from the npm vendored binary path may be");
            eprintln!("    killed by macOS when npm replaces the package files.");
            eprintln!("    Consider closing other grok sessions before updating.");
            eprintln!();
        }
    }
}

/// Test-only entry point: invokes the private [`install_npm`] for tests that swap in a fake `npm` via PATH.
#[doc(hidden)]
pub fn install_npm_for_test(
    target: Option<&str>,
    channel: &str,
    npm_registry: Option<&str>,
) -> Result<()> {
    crate::ensure_updates_enabled()?;
    install_npm(target, channel, npm_registry)
}

fn install_npm(target: Option<&str>, channel: &str, npm_registry: Option<&str>) -> Result<()> {
    #[cfg(target_os = "macos")]
    warn_if_other_grok_processes_running();

    let version_arg = match target {
        Some(ver) => format!("@xai-official/grok@{ver}"),
        None => {
            // All current callers resolve the version via get_latest_version (max(stable, alpha) for the alpha channel) before reaching here
            // Falling back to a raw dist-tag would bypass that logic, so warn loudly if this path is ever hit
            tracing::warn!(
                channel,
                "install_npm called without a resolved version, falling back to dist-tag"
            );
            format!(
                "@xai-official/grok@{}",
                if channel == "alpha" {
                    "alpha"
                } else {
                    "latest"
                }
            )
        }
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} Installing via npm...")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut cmd = Command::new("npm");
    cmd.args(["i", "-g", &version_arg]);
    if let Some(registry) = npm_registry {
        cmd.arg(format!("--registry={}", registry));
    }

    // Use a temporary .npmrc to avoid exposing the token in process lists or shell history.
    let temp_npmrc = create_temp_npmrc(npm_registry)?;
    if let Some(ref npmrc_path) = temp_npmrc {
        cmd.arg(format!("--userconfig={}", npmrc_path.display()));
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        // inherit, not piped; same rationale as run_update_subcommand
        .stderr(Stdio::inherit());
    xai_grok_tools::util::detach_std_command(&mut cmd);
    let status = cmd.status()?;

    if let Some(path) = temp_npmrc
        && let Err(e) = std::fs::remove_file(&path)
    {
        tracing::warn!("Failed to remove temp .npmrc file: {}", e);
    }

    pb.finish_and_clear();

    if !status.success() {
        anyhow::bail!("npm install failed. Please try again.");
    }
    eprintln!();
    Ok(())
}

pub async fn apply_channel_switch(channel_switch: Option<&str>, update_config: &mut UpdateConfig) {
    if crate::ensure_selected_updates_enabled().is_err() {
        return;
    }
    if let Some(ch) = channel_switch
        && update_config.channel != ch
    {
        let _ = config::update_config(|st| {
            st.cli.channel = Some(ch.to_string());
        })
        .await;
        update_config.channel = ch.to_string();
        if cfg!(feature = "community-build") {
            eprintln!("已切换到 {ch} 通道。");
        } else {
            eprintln!("Switched to {} channel.", ch);
        }
    }
}

/// Returns `Ok(Some(version))` when the target version is present on disk afterwards. Callers use the returned version to
/// signal a running leader to relaunch onto the new binary (see the pager's post-update leader relaunch). That signal
/// must fire even when the download itself was skipped, so a stale leader still picks up a binary someone else installed.
pub async fn run_update(
    force: bool,
    pinned_version: Option<&str>,
    channel_switch: Option<&str>,
    update_config: &mut UpdateConfig,
    trigger: CliUpdateTrigger,
) -> Result<Option<String>> {
    crate::ensure_selected_updates_enabled()?;
    apply_channel_switch(channel_switch, update_config).await;
    let installer = match get_installer().await {
        Some(i) => i,
        None => {
            if cfg!(feature = "community-build") {
                eprintln!("当前安装方式无法使用自动更新。");
            } else {
                eprintln!("Auto-update is not available for manual installations.");
            }
            return Ok(None);
        }
    };
    // Persist installer if not already saved
    #[cfg(not(feature = "community-build"))]
    {
        let cfg = config::load_config().await;
        if cfg.cli.installer.is_none() {
            let _ = config::update_config(|st| {
                st.cli.installer = Some(installer.to_string());
            })
            .await;
        }
    }

    heal_managed_install(installer).await;

    let current_version = get_installed_grok_version();
    let policy = config::VersionPolicy::resolve();

    // When --version is given, skip the latest-version check and install directly
    if let Some(version) = pinned_version {
        if let Err(e) = crate::version_policy::check_install_target(&policy, version) {
            anyhow::bail!("{e}");
        }
        if cfg!(feature = "community-build") {
            eprintln!("正在安装 grok-zh {version}（当前：{current_version}）…");
        } else {
            eprintln!(
                "Installing Grok {} (current: {})...",
                version, current_version
            );
        }
        eprintln!();
        run_install_script(installer, Some(version), update_config, trigger).await?;
        refresh_deployment_config().await;
        if let Err(e) = config::update_config(|st| {
            st.cli.auto_update = Some(false);
        })
        .await
        {
            tracing::warn!("Failed to persist auto_update=false for pinned install: {e}");
        }
        #[cfg(feature = "community-build")]
        {
            let commands = crate::community_command_names();
            eprintln!("  ✓ grok-zh v{version} 安装成功！");
            eprintln!("  请重新启动 `{}` 或 `{}`。", commands.grok, commands.agent);
            if std::env::var_os("GROK_ZH_DEFER_UPDATE_NOTES").is_none() {
                crate::community_update_notes::print_pending(Some(version));
            }
        }
        #[cfg(not(feature = "community-build"))]
        {
            eprintln!("  ✓ grok v{} installed successfully!", version);
            eprintln!("  Please restart Grok.");
        }
        return Ok(Some(version.to_string()));
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template(if cfg!(feature = "community-build") {
                "  {spinner:.cyan} 正在检查更新…"
            } else {
                "  {spinner:.cyan} Checking for updates..."
            })
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    let plan = fetch_update_plan(installer, update_config, &policy).await;
    pb.finish_and_clear();
    let plan = plan?;

    let (latest_version, install_target) = match plan {
        UpdatePlan::Skip { latest } => {
            // Cache so an explicit `grok update` doesn't re-prompt every run.
            let stable_ptr = try_fetch_stable_pointer().await;
            #[cfg(feature = "community-build")]
            crate::community_update_cancel::check()?;
            write_version_cache(&latest, stable_ptr.as_deref()).await;
            if cfg!(feature = "community-build") {
                eprintln!(
                    "最新发布版本（{latest}）不在允许的更新范围内；保持当前版本（{current_version}）。"
                );
            } else {
                eprintln!(
                    "The latest release ({latest}) is not an allowed update; \
                     keeping the current version ({current_version})."
                );
            }
            refresh_deployment_config().await;
            return Ok(None);
        }
        UpdatePlan::Unavailable { latest, target } => {
            if cfg!(feature = "community-build") {
                anyhow::bail!(
                    "要求的最低版本（{target}）高于最新可用版本（{latest}）。请联系管理员。"
                );
            }
            anyhow::bail!(
                "The required minimum version ({target}) is newer than the latest \
                 available release ({latest}). Contact your administrator."
            );
        }
        UpdatePlan::Install { latest, target } => (latest, target),
    };
    if install_target != latest_version {
        if cfg!(feature = "community-build") {
            eprintln!(
                "最新可用版本为 {latest_version}，但配置的版本范围允许安装 {install_target}；将安装该版本。"
            );
        } else {
            eprintln!(
                "Latest available is {latest_version}, but your configured version range \
                 allows {install_target}; installing that instead."
            );
        }
    }

    // What's on disk wins over this process's compiled-in version. A concurrent or earlier updater (TUI background download,
    // leader hourly checker) may already have installed the target. In that case there is nothing to download. Gated on the
    // installer maintaining the managed symlink. For npm a leftover symlink would lie (see `disk_version_for_installer`)
    let effective_current =
        disk_version_for_installer(installer).unwrap_or_else(|| current_version.clone());

    if !force {
        match needs_update(
            &effective_current,
            &install_target,
            &update_config.channel,
            installer_allows_downgrade(installer),
        ) {
            Some(true) => {}
            Some(false) => {
                // An explicit channel switch (--stable / --alpha) can target a different version
                // Install it even though the current version is "newer" by semver
                // This handles switching from alpha 0.2.X back to stable 0.1.220 where 0.2.X > 0.1.220
                if channel_switch.is_some() && effective_current != install_target {
                    // Fall through to install
                } else {
                    let stable_ptr = try_fetch_stable_pointer().await;
                    #[cfg(feature = "community-build")]
                    crate::community_update_cancel::check()?;
                    write_version_cache(&install_target, stable_ptr.as_deref()).await;
                    if cfg!(feature = "community-build") {
                        eprintln!("已是最新版本（{effective_current}）。");
                    } else {
                        eprintln!("Already up to date ({}).", effective_current);
                    }
                    // Retry if a prior sync failed.
                    refresh_deployment_config().await;
                    // The target is on disk even though this call installed nothing
                    // Report it so the caller still signals stale leaders to relaunch onto it
                    // Signalling is directional and skips leaders already at/after this version
                    return Ok(Some(install_target));
                }
            }
            None => {
                // Distinguish parse failure from unsupported channel.
                let parse_ok = Version::parse(&effective_current).is_ok()
                    && Version::parse(&install_target).is_ok();
                if parse_ok {
                    if cfg!(feature = "community-build") {
                        anyhow::bail!(
                            "不支持发布通道“{}”（当前={}，目标={}）。支持的通道：stable、alpha、enterprise。可使用 --stable 或 --alpha 覆盖，或在 config.toml 的 [cli] 中设置通道。",
                            update_config.channel,
                            effective_current,
                            install_target
                        );
                    }
                    anyhow::bail!(
                        "Unsupported release channel '{}' (current={}, target={}). \
                         Supported channels: stable, alpha, enterprise. \
                         Use --stable or --alpha to override, or set [cli] channel in config.toml.",
                        update_config.channel,
                        effective_current,
                        install_target
                    );
                } else {
                    if cfg!(feature = "community-build") {
                        anyhow::bail!(
                            "版本解析失败（当前={}，目标={}）",
                            effective_current,
                            install_target
                        );
                    }
                    anyhow::bail!(
                        "Failed to parse versions (current={}, target={})",
                        effective_current,
                        install_target
                    );
                }
            }
        }
    }

    let target_version = if force
        && !needs_update(
            &effective_current,
            &install_target,
            &update_config.channel,
            installer_allows_downgrade(installer),
        )
        .unwrap_or(true)
    {
        if cfg!(feature = "community-build") {
            eprintln!("正在强制重新安装 grok-zh {effective_current}（当前已是最新版本）");
        } else {
            eprintln!(
                "Forcing reinstall of Grok {} (already up to date)",
                effective_current
            );
        }
        &effective_current
    } else {
        if cfg!(feature = "community-build") {
            eprintln!("正在更新 grok-zh {effective_current} → {install_target}");
        } else {
            eprintln!("Updating Grok {} → {}", effective_current, install_target);
        }
        &install_target
    };

    eprintln!();
    run_install_script(installer, Some(target_version), update_config, trigger).await?;
    // Fetch the stable pointer now so the new binary has it immediately for channel_label() display
    // Otherwise it would wait for the next TTL-gated update check (~30 min)
    let stable_ptr = try_fetch_stable_pointer().await;
    write_version_cache(target_version, stable_ptr.as_deref()).await;
    refresh_deployment_config().await;
    if cfg!(feature = "community-build") {
        eprintln!("  ✓ grok-zh v{target_version} 安装成功！");
    } else {
        eprintln!("  ✓ grok v{} installed successfully!", target_version);
    }

    if !force && std::env::var_os("GROK_AUTO_UPDATE").is_none() {
        #[cfg(feature = "community-build")]
        {
            let commands = crate::community_command_names();
            eprintln!("  请重新启动 `{}` 或 `{}`。", commands.grok, commands.agent);
        }
        #[cfg(not(feature = "community-build"))]
        {
            eprintln!("  Please restart Grok.");
        }
    }
    #[cfg(feature = "community-build")]
    if std::env::var_os("GROK_ZH_DEFER_UPDATE_NOTES").is_none() {
        crate::community_update_notes::print_pending(Some(target_version));
    }
    Ok(Some(target_version.to_string()))
}

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

/// Best-effort retry for files that were locked when activation completed.
/// This never touches shared conversation data or official-command backups.
#[cfg(feature = "community-build")]
pub async fn cleanup_community_update_backups() {
    #[cfg(windows)]
    if let Ok(exe) = std::env::current_exe()
        && exe
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("grok-zh.exe"))
        && let Ok(_lock) = acquire_windows_update_lock(&exe)
    {
        sweep_old_exe_backups(&exe.with_file_name("grok-zh.exe.old")).await;
    }
    #[cfg(any(
        all(
            target_os = "macos",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_env = "gnu"
        )
    ))]
    if let Some(home) = xai_dirs::resolve_grok_home() {
        let bin = home.join("bin");
        let downloads = home.join("grok-zh-downloads");
        if validate_community_home_path(&bin).is_ok()
            && validate_community_home_path(&downloads).is_ok()
            && bin.is_dir()
            && downloads.is_dir()
            && let Ok(_lock) = acquire_community_install_lock(&bin)
        {
            cleanup_community_targets(&downloads, &bin).await;
        }
    }
}

#[cfg(all(unix, feature = "community-build"))]
async fn cleanup_community_targets(downloads: &std::path::Path, bin: &std::path::Path) {
    cleanup_community_targets_with(
        downloads,
        bin,
        crate::cleanup_downloads::executable_is_in_use_conservative,
    )
    .await;
}

#[cfg(all(unix, feature = "community-build"))]
async fn cleanup_community_targets_with(
    downloads: &std::path::Path,
    bin: &std::path::Path,
    is_in_use: impl Fn(&std::path::Path) -> bool,
) {
    use std::os::unix::fs::MetadataExt;
    // Refuse redirected directories. Only plain files with our exact installed
    // naming contract are eligible; all live launch aliases retain their target.
    for directory in [downloads, bin] {
        if !std::fs::symlink_metadata(directory).is_ok_and(|metadata| {
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && metadata.uid() == unsafe { libc::geteuid() }
        }) {
            return;
        }
    }
    let Ok(active) = std::fs::canonicalize(bin.join("grok-zh")) else {
        return;
    };
    let Ok(root) = std::fs::canonicalize(downloads) else {
        return;
    };
    if active.parent() != Some(root.as_path()) {
        return;
    }
    let protected: Vec<_> = ["grok-zh", "agent-zh", "grok", "agent"]
        .iter()
        .filter_map(|name| std::fs::canonicalize(bin.join(name)).ok())
        .collect();
    let Ok(mut entries) = tokio::fs::read_dir(&root).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = entry.file_name();
        if !name.to_str().is_some_and(is_community_installed_target)
            || protected.contains(&path)
            || !entry.file_type().await.is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        // A standalone installer may still be publishing a fresh immutable
        // target. Leave those for a subsequent startup instead of racing it.
        let stale = entry
            .metadata()
            .await
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > STALE_TMP_AGE);
        if stale && !is_in_use(&path) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

#[cfg(all(feature = "community-build", any(unix, test)))]
fn is_community_installed_target(name: &str) -> bool {
    let Some(rest) = name
        .strip_prefix("grok-zh-")
        .and_then(|rest| rest.strip_suffix(".installed"))
    else {
        return false;
    };
    [
        "-linux-x86_64-gnu.",
        "-linux-aarch64-gnu.",
        "-macos-aarch64.",
        "-macos-x86_64.",
    ]
    .iter()
    .any(|platform| {
        let Some((version, nonce)) = rest.rsplit_once(platform) else {
            return false;
        };
        semver::Version::parse(version)
            .is_ok_and(|parsed| parsed.to_string() == version && parsed.build.is_empty())
            && !nonce.is_empty()
            && nonce
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

/// Refresh managed config post-update (best-effort, staleness-gated), for deployment-key and team principals alike.
async fn refresh_deployment_config() {
    if !xai_grok_shell::managed_config::has_principal() {
        return;
    }
    if !xai_grok_shell::managed_config::is_fetch_enabled() {
        return;
    }
    // Clear a logged-out team's files before deciding to fetch (mirrors the loop).
    xai_grok_shell::managed_config::clear_orphan();
    if !xai_grok_shell::config::is_managed_config_stale_for(
        &xai_grok_shell::managed_config::current_serving_identity(),
    ) {
        return;
    }
    match xai_grok_shell::managed_config::sync().await {
        Ok(true) => {
            if cfg!(feature = "community-build") {
                eprintln!("  已应用托管配置。");
            } else {
                eprintln!("  Applied managed configuration.");
            }
        }
        Ok(false) => tracing::debug!("no managed configuration to apply"),
        // Auth issues aren't actionable mid-update: quiet here, loud on `grok setup`.
        Err(e) if e.is_auth_rejection() => tracing::debug!("managed config not applied: {e}"),
        Err(e) if e.is_retryable() => {
            tracing::debug!("managed config refresh failed: {e}");
            if cfg!(feature = "community-build") {
                eprintln!("  无法应用托管配置。运行 `grok-zh setup` 重试。");
            } else {
                eprintln!("  Couldn't apply managed configuration. Run `grok setup` to retry.");
            }
        }
        Err(e) => {
            if cfg!(feature = "community-build") {
                eprintln!("  无法应用托管配置：{e}");
            } else {
                eprintln!("  Couldn't apply managed configuration. {e}");
            }
        }
    }
}

#[path = "windows_payload.rs"]
mod windows_payload;

#[cfg(test)]
#[path = "auto_update_tests.rs"]
mod tests;
