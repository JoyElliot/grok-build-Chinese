//! Product identity and privacy policy for the Simplified Chinese community build.
//!
//! Keep distribution identity separate from UI localization. Protocol names,
//! server endpoints, model IDs, tool names, and wire fields must not depend on
//! this crate.

#![forbid(unsafe_code)]

/// Stable machine-readable identity used by packaging and release metadata.
pub const PRODUCT_ID: &str = "grok-build-zh";
/// Human-readable product name for client-owned UI chrome.
pub const DISPLAY_NAME: &str = "Grok Build 中文社区版";
/// Command and executable stem for the community distribution.
pub const CLI_NAME: &str = "grok-zh";
/// Shared per-user data directory, relative to the user's home directory.
///
/// The official and Simplified Chinese executables intentionally use the same
/// sessions, credentials, configuration, plugins, caches, and local state.
pub const DATA_DIR_NAME: &str = ".grok";
/// Shared user-data override used by both the official and Chinese executables.
pub const HOME_ENV: &str = "GROK_HOME";
/// Distribution-specific UI locale override.
pub const LOCALE_ENV: &str = "GROK_ZH_LOCALE";
/// Default UI locale for this distribution.
pub const DEFAULT_UI_LOCALE: &str = "zh-CN";

/// Build policy: product analytics, crash reports and OTLP exports are disabled.
/// Configuration, environment variables and remote feature flags cannot opt in.
pub const TELEMETRY_UPLOADS_ALLOWED: bool = false;
/// Build policy: auxiliary session, trace and research uploads are disabled.
/// This does not block model requests or tools explicitly requested by the user.
pub const SESSION_DATA_UPLOADS_ALLOWED: bool = false;
/// Build policy: feedback polling, submission and attached archives are disabled.
pub const FEEDBACK_UPLOADS_ALLOWED: bool = false;

/// Repository that owns every update accepted by the community distribution.
pub const COMMUNITY_RELEASE_REPO: &str = "JoyElliot/grok-build-Chinese";
/// Canonical download page for the community distribution.
pub const COMMUNITY_RELEASES_URL: &str = "https://github.com/JoyElliot/grok-build-Chinese/releases";
/// Independently versioned display translations; no release API or auth token
/// is involved. Catalog filenames are derived from a validated numeric version.
pub const COMMUNITY_ANNOUNCEMENTS_BASE_URL: &str = "https://raw.githubusercontent.com/JoyElliot/grok-build-Chinese/refs/heads/zh-dev/community/announcements";
/// Domain catalogs follow their corresponding official load events.
pub const COMMUNITY_DISPLAY_TRANSLATIONS_BASE_URL: &str = "https://raw.githubusercontent.com/JoyElliot/grok-build-Chinese/refs/heads/zh-dev/community/display-translations";
/// The community updater uses immutable GitHub Releases from the repository
/// above. Release ZIPs are selected by an exact platform-specific name and
/// verified against GitHub metadata plus the package's inner hashes before
/// activation.
pub const AUTO_UPDATE_ENABLED: bool = true;
/// Default for the user-controlled `[cli].auto_update` setting.
///
/// Availability checks remain enabled so the welcome screen can offer an
/// update, but community builds do not download or install in the background
/// until the user explicitly opts in.
pub const AUTO_UPDATE_DEFAULT_ENABLED: bool = false;
/// Whether the official npm/GitHub/CDN/GCS update sources may be consulted.
pub const OFFICIAL_UPDATE_SOURCES_ALLOWED: bool = false;
/// Whether release notes may be fetched from the official xAI changelog CDN.
pub const OFFICIAL_CHANGELOG_SOURCE_ALLOWED: bool = false;

/// Executable filename for the current platform.
pub const fn executable_name() -> &'static str {
    if cfg!(windows) {
        "grok-zh.exe"
    } else {
        CLI_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_ui_identity_uses_the_shared_official_data_home() {
        assert_eq!(PRODUCT_ID, "grok-build-zh");
        assert_eq!(DATA_DIR_NAME, ".grok");
        assert_eq!(HOME_ENV, "GROK_HOME");
        assert_eq!(LOCALE_ENV, "GROK_ZH_LOCALE");
        assert_ne!(
            executable_name(),
            if cfg!(windows) { "grok.exe" } else { "grok" }
        );
    }

    #[test]
    fn updater_uses_only_the_community_release_source() {
        assert!(AUTO_UPDATE_ENABLED);
        assert!(!AUTO_UPDATE_DEFAULT_ENABLED);
        assert_eq!(COMMUNITY_RELEASE_REPO, "JoyElliot/grok-build-Chinese");
        assert_eq!(
            COMMUNITY_RELEASES_URL,
            "https://github.com/JoyElliot/grok-build-Chinese/releases"
        );
        assert!(!OFFICIAL_UPDATE_SOURCES_ALLOWED);
        assert!(!OFFICIAL_CHANGELOG_SOURCE_ALLOWED);
        assert_eq!(
            COMMUNITY_ANNOUNCEMENTS_BASE_URL,
            "https://raw.githubusercontent.com/JoyElliot/grok-build-Chinese/refs/heads/zh-dev/community/announcements"
        );
    }
}
