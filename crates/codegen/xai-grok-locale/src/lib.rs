//! Centralized UI localization for Grok Build.
//!
//! Catalogs are embedded in the binary and addressed through semantic,
//! typed keys. Consumers never hard-code translated text. The public API is
//! intentionally backend-neutral so the embedded JSON catalog can later be
//! replaced by Fluent without changing UI call sites.

use std::borrow::Cow;
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

const EN_US_SOURCE: &str = include_str!("../locales/en-US.json");
const ZH_CN_SOURCE: &str = include_str!("../locales/zh-CN.json");
const ZH_CN_METADATA_SOURCE: &str = include_str!("../locales/zh-CN-metadata.json");

static EN_US: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| parse_catalog("en-US", EN_US_SOURCE));
static ZH_CN: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| parse_catalog("zh-CN", ZH_CN_SOURCE));
static ZH_CN_METADATA: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| parse_catalog("zh-CN metadata", ZH_CN_METADATA_SOURCE));

fn parse_catalog(name: &str, source: &str) -> BTreeMap<String, String> {
    serde_json::from_str(source)
        .unwrap_or_else(|error| panic!("invalid built-in {name} locale catalog: {error}"))
}

/// UI locales shipped by the community build.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UiLocale {
    #[default]
    EnUs,
    ZhCn,
}

impl UiLocale {
    pub const fn as_bcp47(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }

    /// Canonicalize common BCP-47 and POSIX spellings.
    ///
    /// This deliberately does not reuse the voice/STT language catalog: that
    /// catalog has different supported languages and wire semantics.
    pub fn parse(raw: &str) -> Option<Self> {
        let normalized = raw
            .trim()
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .replace('_', "-")
            .to_ascii_lowercase();
        let primary = normalized.split('-').next().unwrap_or_default();
        match primary {
            "en" => Some(Self::EnUs),
            // This distribution intentionally provides Simplified Chinese.
            "zh" => Some(Self::ZhCn),
            _ => None,
        }
    }
}

impl fmt::Display for UiLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_bcp47())
    }
}

/// Source that selected the effective locale, in descending precedence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocaleSource {
    Requirement,
    Cli,
    Environment,
    Config,
    ManagedConfig,
    System,
    ProductDefault,
}

/// Inputs for deterministic locale resolution.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalePreferences<'a> {
    pub requirement: Option<&'a str>,
    pub cli: Option<&'a str>,
    pub environment: Option<&'a str>,
    pub config: Option<&'a str>,
    pub managed: Option<&'a str>,
    pub system: Option<&'a str>,
    pub product_default: Option<&'a str>,
}

/// Canonical locale plus the layer that selected it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedLocale {
    pub locale: UiLocale,
    pub source: LocaleSource,
}

impl ResolvedLocale {
    pub fn resolve(preferences: LocalePreferences<'_>) -> Self {
        let candidates = [
            (LocaleSource::Requirement, preferences.requirement),
            (LocaleSource::Cli, preferences.cli),
            (LocaleSource::Environment, preferences.environment),
            (LocaleSource::Config, preferences.config),
            (LocaleSource::ManagedConfig, preferences.managed),
            (LocaleSource::System, preferences.system),
            (LocaleSource::ProductDefault, preferences.product_default),
        ];
        candidates
            .into_iter()
            .find_map(|(source, value)| {
                value
                    .filter(|value| !value.trim().is_empty())
                    .and_then(UiLocale::parse)
                    .map(|locale| Self { locale, source })
            })
            .unwrap_or(Self {
                locale: UiLocale::EnUs,
                source: LocaleSource::ProductDefault,
            })
    }
}

/// Stable semantic message keys used by UI code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextKey {
    AppName,
    AuthConnecting,
    AuthCopyConfirmed,
    AuthCopyFailed,
    AuthCopyFallback,
    AuthCopyLink,
    AuthCopyPrefix,
    AuthCopySuffix,
    AuthCopyUnverified,
    AuthDeviceCaption,
    AuthDeviceCodeLabel,
    AuthDeviceHeader,
    AuthGoBack,
    AuthHeader,
    AuthOpenUrl,
    AuthOpeningBrowser,
    AuthQuit,
    AuthRawUrlHint,
    AuthSignIn,
    AuthSignInFailed,
    AuthStartingSession,
    AuthSubmit,
    AuthTokenPlaceholder,
    AuthWaitApproval,
    AuthWaitLogin,
    AuthWaitUrl,
    CommonCancel,
    CommonConfirm,
    MinimalHelpHint,
    MinimalModel,
    PermissionRejectFeedback,
    QuestionOtherPlaceholder,
    ReconnectAttempt,
    ReconnectConnected,
    ReconnectReinitialize,
    ReconnectReload,
    ReconnectRestoreFailed,
    ReconnectRestored,
    ReconnectWait,
    ScreenFullscreenEnabled,
    ScreenMinimalEnabled,
    SessionNewWorktree,
    SessionResume,
    SettingsCategoryAdvanced,
    SettingsCategoryAgent,
    SettingsCategoryAppearance,
    SettingsCategoryEditor,
    SettingsCategoryModels,
    SettingsCategoryMouse,
    SettingsCategoryPrivacy,
    SettingsCategorySession,
    SettingsDocsFooterLong,
    SettingsDocsFooterShort,
    SettingsNoMatches,
    SettingsTitle,
    ShortcutsTitle,
    TrustFooter,
    TrustNoQuit,
    TrustQuestion,
    TrustWarning1,
    TrustWarning2,
    TrustYesProceed,
    WelcomeChangelog,
    WelcomeImportClaudeSettings,
    WelcomeLoginWith,
    WelcomeLogout,
    WelcomeApiKeyAuth,
    WelcomeBeta,
    WelcomeChannelAlpha,
    WelcomeChannelStable,
    WelcomePromptPlaceholder,
    WelcomeProductName,
    WelcomeQuit,
    WelcomeSwitchAccount,
    WelcomeTier,
    WelcomeUnavailable,
    WelcomeUpgradeSubscription,
}

impl TextKey {
    pub const ALL: &[Self] = &[
        Self::AppName,
        Self::AuthConnecting,
        Self::AuthCopyConfirmed,
        Self::AuthCopyFailed,
        Self::AuthCopyFallback,
        Self::AuthCopyLink,
        Self::AuthCopyPrefix,
        Self::AuthCopySuffix,
        Self::AuthCopyUnverified,
        Self::AuthDeviceCaption,
        Self::AuthDeviceCodeLabel,
        Self::AuthDeviceHeader,
        Self::AuthGoBack,
        Self::AuthHeader,
        Self::AuthOpenUrl,
        Self::AuthOpeningBrowser,
        Self::AuthQuit,
        Self::AuthRawUrlHint,
        Self::AuthSignIn,
        Self::AuthSignInFailed,
        Self::AuthStartingSession,
        Self::AuthSubmit,
        Self::AuthTokenPlaceholder,
        Self::AuthWaitApproval,
        Self::AuthWaitLogin,
        Self::AuthWaitUrl,
        Self::CommonCancel,
        Self::CommonConfirm,
        Self::MinimalHelpHint,
        Self::MinimalModel,
        Self::PermissionRejectFeedback,
        Self::QuestionOtherPlaceholder,
        Self::ReconnectAttempt,
        Self::ReconnectConnected,
        Self::ReconnectReinitialize,
        Self::ReconnectReload,
        Self::ReconnectRestoreFailed,
        Self::ReconnectRestored,
        Self::ReconnectWait,
        Self::ScreenFullscreenEnabled,
        Self::ScreenMinimalEnabled,
        Self::SessionNewWorktree,
        Self::SessionResume,
        Self::SettingsCategoryAdvanced,
        Self::SettingsCategoryAgent,
        Self::SettingsCategoryAppearance,
        Self::SettingsCategoryEditor,
        Self::SettingsCategoryModels,
        Self::SettingsCategoryMouse,
        Self::SettingsCategoryPrivacy,
        Self::SettingsCategorySession,
        Self::SettingsDocsFooterLong,
        Self::SettingsDocsFooterShort,
        Self::SettingsNoMatches,
        Self::SettingsTitle,
        Self::ShortcutsTitle,
        Self::TrustFooter,
        Self::TrustNoQuit,
        Self::TrustQuestion,
        Self::TrustWarning1,
        Self::TrustWarning2,
        Self::TrustYesProceed,
        Self::WelcomeChangelog,
        Self::WelcomeImportClaudeSettings,
        Self::WelcomeLoginWith,
        Self::WelcomeLogout,
        Self::WelcomeApiKeyAuth,
        Self::WelcomeBeta,
        Self::WelcomeChannelAlpha,
        Self::WelcomeChannelStable,
        Self::WelcomePromptPlaceholder,
        Self::WelcomeProductName,
        Self::WelcomeQuit,
        Self::WelcomeSwitchAccount,
        Self::WelcomeTier,
        Self::WelcomeUnavailable,
        Self::WelcomeUpgradeSubscription,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::AppName => "app.name",
            Self::AuthConnecting => "auth.connecting",
            Self::AuthCopyConfirmed => "auth.copy.confirmed",
            Self::AuthCopyFailed => "auth.copy.failed",
            Self::AuthCopyFallback => "auth.copy.fallback",
            Self::AuthCopyLink => "auth.copy.link",
            Self::AuthCopyPrefix => "auth.copy.prefix",
            Self::AuthCopySuffix => "auth.copy.suffix",
            Self::AuthCopyUnverified => "auth.copy.unverified",
            Self::AuthDeviceCaption => "auth.device.caption",
            Self::AuthDeviceCodeLabel => "auth.device.code_label",
            Self::AuthDeviceHeader => "auth.device.header",
            Self::AuthGoBack => "auth.go_back",
            Self::AuthHeader => "auth.header",
            Self::AuthOpenUrl => "auth.open_url",
            Self::AuthOpeningBrowser => "auth.opening_browser",
            Self::AuthQuit => "auth.quit",
            Self::AuthRawUrlHint => "auth.raw_url_hint",
            Self::AuthSignIn => "auth.sign_in",
            Self::AuthSignInFailed => "auth.sign_in_failed",
            Self::AuthStartingSession => "auth.starting_session",
            Self::AuthSubmit => "auth.submit",
            Self::AuthTokenPlaceholder => "auth.token_placeholder",
            Self::AuthWaitApproval => "auth.wait_approval",
            Self::AuthWaitLogin => "auth.wait_login",
            Self::AuthWaitUrl => "auth.wait_url",
            Self::CommonCancel => "common.cancel",
            Self::CommonConfirm => "common.confirm",
            Self::MinimalHelpHint => "minimal.help_hint",
            Self::MinimalModel => "minimal.model",
            Self::PermissionRejectFeedback => "permission.reject_feedback",
            Self::QuestionOtherPlaceholder => "question.other_placeholder",
            Self::ReconnectAttempt => "reconnect.attempt",
            Self::ReconnectConnected => "reconnect.connected",
            Self::ReconnectReinitialize => "reconnect.reinitialize",
            Self::ReconnectReload => "reconnect.reload",
            Self::ReconnectRestoreFailed => "reconnect.restore_failed",
            Self::ReconnectRestored => "reconnect.restored",
            Self::ReconnectWait => "reconnect.wait",
            Self::ScreenFullscreenEnabled => "screen.fullscreen.enabled",
            Self::ScreenMinimalEnabled => "screen.minimal.enabled",
            Self::SessionNewWorktree => "session.new_worktree",
            Self::SessionResume => "session.resume",
            Self::SettingsCategoryAdvanced => "settings.category.advanced",
            Self::SettingsCategoryAgent => "settings.category.agent",
            Self::SettingsCategoryAppearance => "settings.category.appearance",
            Self::SettingsCategoryEditor => "settings.category.editor",
            Self::SettingsCategoryModels => "settings.category.models",
            Self::SettingsCategoryMouse => "settings.category.mouse",
            Self::SettingsCategoryPrivacy => "settings.category.privacy",
            Self::SettingsCategorySession => "settings.category.session",
            Self::SettingsDocsFooterLong => "settings.docs_footer.long",
            Self::SettingsDocsFooterShort => "settings.docs_footer.short",
            Self::SettingsNoMatches => "settings.no_matches",
            Self::SettingsTitle => "settings.title",
            Self::ShortcutsTitle => "shortcuts.title",
            Self::TrustFooter => "trust.footer",
            Self::TrustNoQuit => "trust.no_quit",
            Self::TrustQuestion => "trust.question",
            Self::TrustWarning1 => "trust.warning_1",
            Self::TrustWarning2 => "trust.warning_2",
            Self::TrustYesProceed => "trust.yes_proceed",
            Self::WelcomeChangelog => "welcome.changelog",
            Self::WelcomeImportClaudeSettings => "welcome.import_claude_settings",
            Self::WelcomeLoginWith => "welcome.login_with",
            Self::WelcomeLogout => "welcome.logout",
            Self::WelcomeApiKeyAuth => "welcome.api_key_auth",
            Self::WelcomeBeta => "welcome.beta",
            Self::WelcomeChannelAlpha => "welcome.channel.alpha",
            Self::WelcomeChannelStable => "welcome.channel.stable",
            Self::WelcomePromptPlaceholder => "welcome.prompt_placeholder",
            Self::WelcomeProductName => "welcome.product_name",
            Self::WelcomeQuit => "welcome.quit",
            Self::WelcomeSwitchAccount => "welcome.switch_account",
            Self::WelcomeTier => "welcome.tier",
            Self::WelcomeUnavailable => "welcome.unavailable",
            Self::WelcomeUpgradeSubscription => "welcome.upgrade_subscription",
        }
    }
}

/// Immutable localization context resolved once at the composition root.
#[derive(Clone, Debug)]
pub struct LocaleContext {
    resolved: ResolvedLocale,
}

impl Default for LocaleContext {
    fn default() -> Self {
        Self::new(ResolvedLocale {
            locale: UiLocale::EnUs,
            source: LocaleSource::ProductDefault,
        })
    }
}

impl LocaleContext {
    pub const fn new(resolved: ResolvedLocale) -> Self {
        Self { resolved }
    }

    pub const fn resolved(&self) -> ResolvedLocale {
        self.resolved
    }

    pub const fn locale(&self) -> UiLocale {
        self.resolved.locale
    }

    /// Display-only format for message timestamps, including their left padding.
    pub fn message_timestamp_format(&self, expanded: bool) -> &'static str {
        if expanded {
            self.named_static_text("timestamp.message.expanded_format", "  %H:%M:%S | %b %d")
        } else {
            self.named_static_text("timestamp.message.short_format", "  %-I:%M %p")
        }
    }

    /// Get a static message, falling back to the complete English catalog.
    pub fn text(&self, key: TextKey) -> &'static str {
        let id = key.id();
        selected_catalog(self.locale())
            .get(id)
            .or_else(|| EN_US.get(id))
            .map(String::as_str)
            .unwrap_or(id)
    }

    /// Look up a structured catalog entry while retaining an explicit English
    /// fallback at the call site.
    ///
    /// Stable, shared UI messages should keep using [`TextKey`]. This method is
    /// for large upstream-owned metadata catalogs (settings, command metadata,
    /// picker choices) where adding two enum variants for every label and
    /// description would make upstream merges unnecessarily noisy.
    pub fn named_text<'a>(&self, id: &str, english: &'a str) -> Cow<'a, str> {
        if self.locale() == UiLocale::ZhCn
            && let Some(value) = ZH_CN_METADATA.get(id)
        {
            return Cow::Borrowed(value.as_str());
        }
        selected_catalog(self.locale())
            .get(id)
            .or_else(|| EN_US.get(id))
            .map(|value| Cow::Borrowed(value.as_str()))
            .unwrap_or_else(|| Cow::Borrowed(english))
    }

    /// Static variant for UI metadata stored in structures that borrow their
    /// labels (for example modal shortcut rows). Both built-in catalogs and
    /// the English fallback live for the duration of the process.
    pub fn named_static_text(&self, id: &str, english: &'static str) -> &'static str {
        if self.locale() == UiLocale::ZhCn
            && let Some(value) = ZH_CN_METADATA.get(id)
        {
            return value.as_str();
        }
        selected_catalog(self.locale())
            .get(id)
            .or_else(|| EN_US.get(id))
            .map(String::as_str)
            .unwrap_or(english)
    }

    /// Localized display label for a stable setting key. The key itself remains
    /// the canonical config/TOML identifier and is never translated.
    pub fn setting_label<'a>(&self, setting_key: &str, english: &'a str) -> Cow<'a, str> {
        self.named_text(&format!("settings.setting.{setting_key}.label"), english)
    }

    /// Localized help text for a stable setting key.
    pub fn setting_description<'a>(&self, setting_key: &str, english: &'a str) -> Cow<'a, str> {
        self.named_text(
            &format!("settings.setting.{setting_key}.description"),
            english,
        )
    }

    /// Localized display label for a setting choice. `canonical` is still the
    /// persisted/wire value; unknown runtime values deliberately fall back to
    /// their original display text.
    pub fn setting_choice_label<'a>(
        &self,
        setting_key: &str,
        canonical: &str,
        english: &'a str,
    ) -> Cow<'a, str> {
        let setting_key = setting_choice_catalog_key(setting_key);
        let canonical = if canonical.is_empty() {
            "_none"
        } else {
            canonical
        };
        self.named_text(
            &format!("settings.setting.{setting_key}.choice.{canonical}.label"),
            english,
        )
    }

    /// Localized explanatory text for a setting choice.
    pub fn setting_choice_description<'a>(
        &self,
        setting_key: &str,
        canonical: &str,
        english: &'a str,
    ) -> Cow<'a, str> {
        let setting_key = setting_choice_catalog_key(setting_key);
        let canonical = if canonical.is_empty() {
            "_none"
        } else {
            canonical
        };
        self.named_text(
            &format!("settings.setting.{setting_key}.choice.{canonical}.description"),
            english,
        )
    }

    /// Format a message using named placeholders such as `{provider}`.
    ///
    /// Unknown arguments are ignored. Missing arguments intentionally remain
    /// visible in the returned string so catalog mistakes cannot silently erase
    /// user-visible context.
    pub fn format(&self, key: TextKey, arguments: &[(&str, &str)]) -> String {
        let mut output = self.text(key).to_owned();
        for (name, value) in arguments {
            output = output.replace(&format!("{{{name}}}"), value);
        }
        output
    }
}

fn setting_choice_catalog_key(setting_key: &str) -> &str {
    match setting_key {
        "auto_dark_theme" | "auto_light_theme" => "theme",
        "fork_secondary_model" => "default_model",
        other => other,
    }
}

fn selected_catalog(locale: UiLocale) -> &'static BTreeMap<String, String> {
    match locale {
        UiLocale::EnUs => &EN_US,
        UiLocale::ZhCn => &ZH_CN,
    }
}

/// Best-effort host UI locale. Explicit config and environment inputs should
/// be resolved before this value.
pub fn system_locale() -> Option<String> {
    let posix = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    if let Some(value) = posix
        && !value.eq_ignore_ascii_case("C")
        && !value.eq_ignore_ascii_case("POSIX")
    {
        return Some(value);
    }
    system_locale_platform()
}

#[cfg(windows)]
fn system_locale_platform() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

    let mut buffer = [0_u16; LOCALE_NAME_MAX_LENGTH as usize];
    // SAFETY: Windows writes at most `buffer.len()` UTF-16 code units to the
    // valid mutable buffer. The returned count includes the trailing NUL.
    let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 1 {
        return None;
    }
    String::from_utf16(&buffer[..length as usize - 1]).ok()
}

#[cfg(not(windows))]
fn system_locale_platform() -> Option<String> {
    None
}

#[cfg(test)]
fn placeholders(template: &str) -> BTreeSet<&str> {
    let mut result = BTreeSet::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('}') else {
            break;
        };
        let name = &rest[..close];
        if !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            result.insert(name);
        }
        rest = &rest[close + 1..];
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_supported_bcp47_and_posix_forms() {
        assert_eq!(UiLocale::parse("en_US.UTF-8"), Some(UiLocale::EnUs));
        assert_eq!(UiLocale::parse("zh-Hans-CN"), Some(UiLocale::ZhCn));
        assert_eq!(UiLocale::parse("ZH_cn"), Some(UiLocale::ZhCn));
        assert_eq!(UiLocale::parse("fr-FR"), None);
    }

    #[test]
    fn requirement_wins_and_invalid_values_fall_through() {
        let resolved = ResolvedLocale::resolve(LocalePreferences {
            requirement: Some("en-US"),
            cli: Some("zh-CN"),
            ..LocalePreferences::default()
        });
        assert_eq!(resolved.locale, UiLocale::EnUs);
        assert_eq!(resolved.source, LocaleSource::Requirement);

        let resolved = ResolvedLocale::resolve(LocalePreferences {
            cli: Some("unsupported"),
            environment: Some("zh_CN.UTF-8"),
            ..LocalePreferences::default()
        });
        assert_eq!(resolved.locale, UiLocale::ZhCn);
        assert_eq!(resolved.source, LocaleSource::Environment);
    }

    #[test]
    fn every_locale_layer_obeys_declared_precedence() {
        let candidates = [
            (LocaleSource::Requirement, "requirement"),
            (LocaleSource::Cli, "cli"),
            (LocaleSource::Environment, "environment"),
            (LocaleSource::Config, "config"),
            (LocaleSource::ManagedConfig, "managed"),
            (LocaleSource::System, "system"),
            (LocaleSource::ProductDefault, "product"),
        ];
        for (selected_index, (expected_source, _)) in candidates.iter().enumerate() {
            let values = candidates.map(|_| Some("unsupported"));
            let mut values = values;
            values[selected_index] = Some("zh-CN");
            let resolved = ResolvedLocale::resolve(LocalePreferences {
                requirement: values[0],
                cli: values[1],
                environment: values[2],
                config: values[3],
                managed: values[4],
                system: values[5],
                product_default: values[6],
            });
            assert_eq!(resolved.locale, UiLocale::ZhCn);
            assert_eq!(resolved.source, *expected_source);
        }
    }

    #[test]
    fn invalid_candidates_fall_back_to_product_default_then_english() {
        let unsupported = LocalePreferences {
            requirement: Some(""),
            cli: Some("fr-FR"),
            environment: Some("unsupported"),
            config: Some(" "),
            managed: Some("de-DE"),
            system: Some("ja-JP"),
            product_default: Some("zh-CN"),
        };
        assert_eq!(
            ResolvedLocale::resolve(unsupported),
            ResolvedLocale {
                locale: UiLocale::ZhCn,
                source: LocaleSource::ProductDefault,
            }
        );
        assert_eq!(
            ResolvedLocale::resolve(LocalePreferences {
                product_default: None,
                ..unsupported
            }),
            ResolvedLocale {
                locale: UiLocale::EnUs,
                source: LocaleSource::ProductDefault,
            }
        );
    }

    #[test]
    fn catalogs_have_matching_keys_and_placeholders_and_include_all_typed_keys() {
        let expected: BTreeSet<&str> = TextKey::ALL.iter().map(|key| key.id()).collect();
        let english: BTreeSet<&str> = EN_US.keys().map(String::as_str).collect();
        let chinese: BTreeSet<&str> = ZH_CN.keys().map(String::as_str).collect();
        assert_eq!(english, chinese, "locale catalog key drift");
        assert!(
            expected.is_subset(&english),
            "typed keys must all exist in both catalogs"
        );
        for id in english {
            assert_eq!(
                placeholders(EN_US.get(id).unwrap()),
                placeholders(ZH_CN.get(id).unwrap()),
                "placeholder drift for {}",
                id
            );
        }
    }

    #[test]
    fn structured_setting_lookup_localizes_display_text_without_touching_identity() {
        let context = LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        });
        assert_eq!(
            context.setting_label("compact_mode", "Compact mode"),
            "紧凑模式"
        );
        assert_eq!(
            context.setting_choice_label("permission_mode", "always-approve", "Always approve"),
            "始终批准"
        );
        assert_eq!(
            context.setting_choice_label("default_model", "grok-4.5", "grok-4.5"),
            "grok-4.5"
        );
        assert_eq!(context.named_text("context.tokens", "tokens"), "Token");
        assert_eq!(
            context.named_text("mode.always_approve.label", "always-approve"),
            "始终批准"
        );
    }

    #[test]
    fn structured_metadata_ids_and_values_are_non_empty() {
        assert!(!ZH_CN_METADATA.is_empty());
        for (id, value) in ZH_CN_METADATA.iter() {
            assert!(!id.trim().is_empty(), "blank metadata id");
            assert!(!value.trim().is_empty(), "blank metadata value for {id}");
        }
    }

    #[test]
    fn formatting_preserves_opaque_dynamic_values() {
        let context = LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        });
        assert_eq!(
            context.format(TextKey::WelcomeLoginWith, &[("provider", "grok.com")]),
            "使用 grok.com 登录"
        );
    }

    #[test]
    fn chinese_composer_and_shortcut_labels_are_catalog_backed() {
        let context = LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        });
        assert_eq!(
            context.named_text("prompt.placeholder.default", "Build anything"),
            "告诉我你想做些什么…"
        );
        assert_eq!(
            context.named_text("shortcut.clear_search", "clear search"),
            "清除搜索"
        );
        assert_eq!(
            context.named_text("shortcut.switch_tab", "switch tab"),
            "切换标签页"
        );
    }
}
