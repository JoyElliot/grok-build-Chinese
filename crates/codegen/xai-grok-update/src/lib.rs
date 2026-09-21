#![deny(clippy::indexing_slicing)]

#[cfg(feature = "community-build")]
pub mod announcement_translations;
pub mod auto_update;
mod cleanup_downloads;
#[cfg(feature = "community-build")]
mod community_release;
#[cfg(feature = "community-build")]
pub mod community_update_cancel;
#[cfg(feature = "community-build")]
pub mod community_update_notes;
pub mod version;
mod version_policy;

pub use auto_update::UpdateStatus;
pub use version::{UpdateConfig, channel_label, channel_name, write_version_cache};
pub use version_policy::enforce_version_policy_or_exit;

#[cfg(feature = "community-build")]
const COMMUNITY_INSTALL_MARKER: &str = ".grok-zh-install.json";
#[cfg(feature = "community-build")]
const MAX_COMMUNITY_INSTALL_MARKER_BYTES: u64 = 64 * 1024;

/// User-facing launch commands exposed by the community Windows installer.
#[cfg(feature = "community-build")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommunityCommandNames {
    pub grok: &'static str,
    pub agent: &'static str,
}

#[cfg(feature = "community-build")]
impl CommunityCommandNames {
    const COMMUNITY: Self = Self {
        grok: "grok-zh",
        agent: "agent-zh",
    };
    const COMPATIBILITY: Self = Self {
        grok: "grok",
        agent: "agent",
    };
}

#[cfg(feature = "community-build")]
#[derive(serde::Deserialize)]
struct CommunityInstallMarker {
    product: String,
    #[serde(default)]
    commands: Vec<String>,
}

#[cfg(feature = "community-build")]
fn same_search_path_dir(left: &std::path::Path, right: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(all(feature = "community-build", windows))]
fn command_search_suffixes(path_ext: Option<&std::ffi::OsStr>) -> Vec<String> {
    const DEFAULT_WINDOWS_PATHEXT: &str =
        ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC;.CPL";
    let path_ext = path_ext
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_WINDOWS_PATHEXT.to_string());
    let mut suffixes = vec![String::new(), ".ps1".to_string()];
    for raw_suffix in path_ext.split(';') {
        let raw_suffix = raw_suffix.trim();
        if raw_suffix.is_empty() {
            continue;
        }
        let suffix = if raw_suffix.starts_with('.') {
            raw_suffix.to_string()
        } else {
            format!(".{raw_suffix}")
        };
        if !suffixes
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&suffix))
        {
            suffixes.push(suffix);
        }
    }
    suffixes
}

#[cfg(feature = "community-build")]
fn search_path_resolves_shim(
    search_path: Option<&std::ffi::OsStr>,
    install_dir: &std::path::Path,
    command: &str,
) -> bool {
    let Some(search_path) = search_path else {
        return false;
    };
    let install_dir =
        std::fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());

    #[cfg(windows)]
    let command_suffixes = command_search_suffixes(std::env::var_os("PATHEXT").as_deref());
    #[cfg(windows)]
    if !command_suffixes
        .iter()
        .any(|suffix| suffix.eq_ignore_ascii_case(".cmd"))
    {
        return false;
    }
    for entry in std::env::split_paths(search_path) {
        let normalized_entry = std::fs::canonicalize(&entry).unwrap_or_else(|_| entry.clone());
        if same_search_path_dir(&normalized_entry, &install_dir) {
            #[cfg(windows)]
            {
                let expected_shim = entry.join(format!("{command}.cmd"));
                let has_conflicting_candidate = command_suffixes
                    .iter()
                    .filter(|suffix| !suffix.eq_ignore_ascii_case(".cmd"))
                    .any(|suffix| entry.join(format!("{command}{suffix}")).is_file());
                return expected_shim.is_file() && !has_conflicting_candidate;
            }
            #[cfg(unix)]
            {
                return entry.join(command).is_file();
            }
        }

        #[cfg(windows)]
        if command_suffixes
            .iter()
            .any(|suffix| entry.join(format!("{command}{suffix}")).is_file())
        {
            return false;
        }
        #[cfg(unix)]
        if entry.join(command).is_file() {
            return false;
        }
    }

    false
}

#[cfg(feature = "community-build")]
fn community_shim_path(install_dir: &std::path::Path, command: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        install_dir.join(format!("{command}.cmd"))
    } else {
        install_dir.join(command)
    }
}

#[cfg(feature = "community-build")]
fn community_command_names_in(
    install_dir: &std::path::Path,
    search_path: Option<&std::ffi::OsStr>,
) -> CommunityCommandNames {
    let marker_path = install_dir.join(COMMUNITY_INSTALL_MARKER);
    let Ok(metadata) = std::fs::metadata(&marker_path) else {
        return CommunityCommandNames::COMMUNITY;
    };
    if !metadata.is_file() || metadata.len() > MAX_COMMUNITY_INSTALL_MARKER_BYTES {
        return CommunityCommandNames::COMMUNITY;
    }
    let Ok(marker_bytes) = std::fs::read(marker_path) else {
        return CommunityCommandNames::COMMUNITY;
    };
    let marker_bytes = marker_bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&marker_bytes);
    let Ok(marker) = serde_json::from_slice::<CommunityInstallMarker>(marker_bytes) else {
        return CommunityCommandNames::COMMUNITY;
    };
    let exposes_compatibility_names = marker.product == "grok-build-zh"
        && marker.commands.iter().any(|command| command == "grok")
        && marker.commands.iter().any(|command| command == "agent")
        && community_shim_path(install_dir, "grok").is_file()
        && community_shim_path(install_dir, "agent").is_file()
        && search_path_resolves_shim(search_path, install_dir, "grok")
        && search_path_resolves_shim(search_path, install_dir, "agent");
    if exposes_compatibility_names {
        CommunityCommandNames::COMPATIBILITY
    } else {
        CommunityCommandNames::COMMUNITY
    }
}

/// Detect the launch commands that remain available after a community update.
///
/// The installer marker records whether the user enabled the optional
/// `grok`/`agent` compatibility shims. Both shim files must still exist so a
/// stale marker never recommends a command the user removed manually.
/// Both shims must resolve from the install directory before any earlier PATH
/// candidate so `-NoPathUpdate` and shadowed official commands never cause a
/// misleading recommendation.
#[cfg(feature = "community-build")]
pub fn community_command_names() -> CommunityCommandNames {
    #[cfg(windows)]
    let install_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf));
    #[cfg(unix)]
    let install_dir = xai_dirs::resolve_grok_home().map(|home| home.join("bin"));
    #[cfg(not(any(windows, unix)))]
    let install_dir: Option<std::path::PathBuf> = None;

    install_dir.map_or(CommunityCommandNames::COMMUNITY, |install_dir| {
        let search_path = std::env::var_os("PATH");
        community_command_names_in(&install_dir, search_path.as_deref())
    })
}

/// User-facing reason returned when the selected distribution source is not
/// enabled. Community builds never use the official xAI update sources.
#[cfg(feature = "community-build")]
pub const UPDATE_DISABLED_REASON: &str = "此版本未启用所选的更新来源。";
#[cfg(not(feature = "community-build"))]
pub const UPDATE_DISABLED_REASON: &str =
    "The selected update source is disabled for this distribution.";

/// Compile-time distribution policy. Keeping this in one leaf crate prevents
/// an upstream updater refactor from accidentally re-enabling official sources.
pub const fn updates_enabled() -> bool {
    if cfg!(feature = "community-build") {
        community_updates_enabled()
    } else {
        true
    }
}

/// Effective default for `[cli].auto_update` in the selected distribution.
/// Official builds retain their upstream default; the community build is
/// opt-in while still allowing metadata-only availability checks.
pub const fn default_auto_update_enabled() -> bool {
    if cfg!(feature = "community-build") {
        xai_grok_product::AUTO_UPDATE_DEFAULT_ENABLED
    } else {
        true
    }
}

/// The upstream updater implementation only contains official xAI backends.
/// Community builds keep them unavailable even after their own updater exists.
pub const fn official_update_sources_allowed() -> bool {
    !cfg!(feature = "community-build") || xai_grok_product::OFFICIAL_UPDATE_SOURCES_ALLOWED
}

/// Whether this binary is the community distribution with its fixed Releases
/// backend enabled.
pub const fn community_updates_enabled() -> bool {
    cfg!(feature = "community-build")
        && xai_grok_product::AUTO_UPDATE_ENABLED
        && (cfg!(all(
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "gnu"
        )) || cfg!(all(target_os = "macos", target_arch = "aarch64"))
            || cfg!(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_env = "gnu"
            )))
}

pub(crate) fn ensure_updates_enabled() -> anyhow::Result<()> {
    if !updates_enabled() || !official_update_sources_allowed() {
        anyhow::bail!(UPDATE_DISABLED_REASON);
    }
    Ok(())
}

pub(crate) fn ensure_community_updates_enabled() -> anyhow::Result<()> {
    if !community_updates_enabled() {
        anyhow::bail!(UPDATE_DISABLED_REASON);
    }
    Ok(())
}

/// Gate the backend selected at compile time. Official helper entry points keep
/// using [`ensure_updates_enabled`] so community builds cannot reach them.
pub(crate) fn ensure_selected_updates_enabled() -> anyhow::Result<()> {
    if cfg!(feature = "community-build") {
        ensure_community_updates_enabled()
    } else {
        ensure_updates_enabled()
    }
}

#[cfg(all(test, feature = "community-build"))]
mod community_build_tests {
    use super::*;

    fn write_install_marker(install_dir: &std::path::Path, commands: &[&str], bom: bool) {
        let json = serde_json::json!({
            "product": "grok-build-zh",
            "commands": commands,
        })
        .to_string();
        let bytes = if bom {
            [b"\xef\xbb\xbf".as_slice(), json.as_bytes()].concat()
        } else {
            json.into_bytes()
        };
        std::fs::write(install_dir.join(COMMUNITY_INSTALL_MARKER), bytes).unwrap();
    }

    #[test]
    fn community_build_enables_only_its_fixed_release_source() {
        let supported_target = cfg!(all(
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "gnu"
        )) || cfg!(all(target_os = "macos", target_arch = "aarch64"))
            || cfg!(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_env = "gnu"
            ));
        assert_eq!(updates_enabled(), supported_target);
        assert!(!default_auto_update_enabled());
        assert_eq!(community_updates_enabled(), supported_target);
        assert!(!official_update_sources_allowed());
        assert_eq!(ensure_community_updates_enabled().is_ok(), supported_target);
        assert_eq!(
            ensure_updates_enabled().unwrap_err().to_string(),
            UPDATE_DISABLED_REASON
        );
    }

    #[tokio::test]
    async fn official_download_is_blocked_and_fixed_installer_is_selected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let destination = temp.path().join("must-not-exist");
        let error =
            auto_update::download_silent("http://127.0.0.1:1/must-not-be-requested", &destination)
                .await
                .expect_err("community download must be disabled");
        assert_eq!(error.to_string(), UPDATE_DISABLED_REASON);
        assert!(!destination.exists());
        let expected_installer =
            community_updates_enabled().then_some(community_release::COMMUNITY_INSTALLER);
        assert_eq!(auto_update::get_installer().await, expected_installer);
    }

    #[test]
    fn community_command_names_require_marker_and_both_live_shims() {
        let temp = tempfile::tempdir().unwrap();
        let search_path = std::env::join_paths([temp.path()]).unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );

        write_install_marker(temp.path(), &["grok-zh", "agent-zh", "grok", "agent"], true);
        std::fs::write(community_shim_path(temp.path(), "grok"), b"shim").unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );

        std::fs::write(community_shim_path(temp.path(), "agent"), b"shim").unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), None),
            CommunityCommandNames::COMMUNITY
        );
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMPATIBILITY
        );

        std::fs::remove_file(community_shim_path(temp.path(), "grok")).unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );
    }

    #[test]
    fn community_command_names_reject_invalid_or_non_compatibility_markers() {
        let temp = tempfile::tempdir().unwrap();
        let search_path = std::env::join_paths([temp.path()]).unwrap();
        write_install_marker(temp.path(), &["grok-zh", "agent-zh"], false);
        std::fs::write(community_shim_path(temp.path(), "grok"), b"shim").unwrap();
        std::fs::write(community_shim_path(temp.path(), "agent"), b"shim").unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );

        std::fs::write(temp.path().join(COMMUNITY_INSTALL_MARKER), b"not json").unwrap();
        assert_eq!(
            community_command_names_in(temp.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );
    }

    #[test]
    fn community_command_names_reject_shadowed_compatibility_commands() {
        let install = tempfile::tempdir().unwrap();
        let shadow = tempfile::tempdir().unwrap();
        write_install_marker(
            install.path(),
            &["grok-zh", "agent-zh", "grok", "agent"],
            false,
        );
        std::fs::write(community_shim_path(install.path(), "grok"), b"shim").unwrap();
        std::fs::write(community_shim_path(install.path(), "agent"), b"shim").unwrap();
        let shadow_name = if cfg!(windows) { "grok.exe" } else { "grok" };
        std::fs::write(shadow.path().join(shadow_name), b"shadow").unwrap();
        let search_path = std::env::join_paths([shadow.path(), install.path()]).unwrap();

        assert_eq!(
            community_command_names_in(install.path(), Some(&search_path)),
            CommunityCommandNames::COMMUNITY
        );
    }

    #[cfg(windows)]
    #[test]
    fn command_search_suffixes_honor_pathext_and_powershell_scripts() {
        let suffixes =
            command_search_suffixes(Some(std::ffi::OsStr::new(".EXE;.CMD;.VBS;.JS;CMD")));

        for expected in ["", ".ps1", ".exe", ".cmd", ".vbs", ".js"] {
            assert!(
                suffixes
                    .iter()
                    .any(|suffix| suffix.eq_ignore_ascii_case(expected)),
                "missing command search suffix {expected:?}: {suffixes:?}",
            );
        }
        assert_eq!(
            suffixes
                .iter()
                .filter(|suffix| suffix.eq_ignore_ascii_case(".cmd"))
                .count(),
            1,
        );
    }
}
