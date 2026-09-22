//! Display-only snapshots and process-local signals from existing official loads.
//! There is deliberately no network client, timer, or account state here.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Models,
    Settings,
    Mcp,
    Skills,
    Marketplace,
}

impl Domain {
    pub const ALL: [Self; 5] = [
        Self::Models,
        Self::Settings,
        Self::Mcp,
        Self::Skills,
        Self::Marketplace,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Models => "models",
            Self::Settings => "settings",
            Self::Mcp => "mcp",
            Self::Skills => "skills",
            Self::Marketplace => "marketplace",
        }
    }

    fn events(self) -> &'static watch::Sender<bool> {
        static EVENTS: OnceLock<BTreeMap<Domain, watch::Sender<bool>>> = OnceLock::new();
        EVENTS
            .get_or_init(|| {
                Self::ALL
                    .into_iter()
                    .map(|domain| (domain, watch::channel(false).0))
                    .collect()
            })
            .get(&self)
            .expect("all display domains registered")
    }

    /// Notify at an existing load boundary, never from a render or a new timer.
    pub fn notify_load(self) {
        self.events().send_replace(true);
    }

    /// Retain startup loads that happened before the frontend subscribed.
    pub fn subscribe(self) -> watch::Receiver<bool> {
        let mut receiver = self.events().subscribe();
        if *receiver.borrow_and_update() {
            receiver.mark_changed();
        }
        receiver
    }

    pub fn context_len(self, field: &str) -> Option<usize> {
        match (self, field) {
            (Self::Models, "description") => Some(1),
            (Self::Models, "effort_label" | "effort_description") => Some(2),
            (Self::Settings, "tip" | "gate_message" | "gate_label") => Some(0),
            (Self::Settings, "command_tag") => Some(1),
            (Self::Mcp, "tool_label" | "tool_description") => Some(3),
            (Self::Mcp, "connector_label") => Some(1),
            (Self::Skills, "label" | "description") => Some(1),
            (Self::Marketplace, "description" | "category") => Some(2),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayEntry {
    pub field: String,
    pub context: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_string"
    )]
    pub source: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_string"
    )]
    pub source_sha256: Option<String>,
    pub translation: String,
}

// Omission is allowed; an explicitly present null is not an exact source.
fn present_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

pub fn digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_text(value: &str, multiline: bool) -> bool {
    !value.trim().is_empty()
        && value.len() <= 16 * 1024
        && !value
            .chars()
            .any(|c| c.is_control() && !(multiline && c == '\n'))
}

/// Keys include identity and the exact source digest, so changed server copy
/// cannot inherit an old translation. Values never enter protocol messages.
#[derive(Debug)]
pub struct DisplayCatalog {
    pub domain: Domain,
    entries: BTreeMap<(String, Vec<String>, String), String>,
}

impl DisplayCatalog {
    pub fn from_entries(domain: Domain, entries: Vec<DisplayEntry>) -> Result<Self, &'static str> {
        if entries.len() > 512 {
            return Err("too many display translations");
        }
        let mut mapped = BTreeMap::new();
        for entry in entries {
            if domain.context_len(&entry.field) != Some(entry.context.len())
                || entry
                    .context
                    .iter()
                    .any(|s| !valid_text(s, false) || s.len() > 1024)
            {
                return Err("invalid display field or identity");
            }
            let multiline = matches!(
                entry.field.as_str(),
                "description" | "effort_description" | "tool_description" | "tip" | "gate_message"
            );
            if !valid_text(&entry.translation, multiline) {
                return Err("invalid display translation");
            }
            let hash = match (&entry.source, &entry.source_sha256) {
                (Some(source), None) if valid_text(source, multiline) => digest(source),
                (None, Some(hash)) if valid_digest(hash) => hash.clone(),
                _ => return Err("exactly one valid source or source_sha256 is required"),
            };
            if domain == Domain::Mcp && entry.context.len() == 3 && !valid_digest(&entry.context[2])
            {
                return Err("invalid authoritative MCP description digest");
            }
            if domain == Domain::Mcp
                && entry.field == "tool_description"
                && hash != entry.context[2]
            {
                return Err("MCP description does not match its authoritative digest");
            }
            if mapped
                .insert((entry.field, entry.context, hash), entry.translation)
                .is_some()
            {
                return Err("duplicate display translation");
            }
        }
        Ok(Self {
            domain,
            entries: mapped,
        })
    }

    pub fn lookup(&self, field: &str, context: &[&str], source: &str) -> Option<&str> {
        self.entries
            .get(&(
                field.to_owned(),
                context.iter().map(|s| (*s).to_owned()).collect(),
                digest(source),
            ))
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DisplayCatalogs(Arc<RwLock<BTreeMap<Domain, Arc<DisplayCatalog>>>>);

/// Source selection belongs to one application context, shared only with its
/// render clones. Keep the selected occurrence, not a set of matching tip text.
#[derive(Clone, Debug, Default)]
pub(crate) struct SettingsSources(Arc<RwLock<SettingsSelection>>);

#[derive(Debug, Default)]
struct SettingsSelection {
    tip: Option<String>,
    command_tags: BTreeMap<String, String>,
}

impl SettingsSources {
    pub fn set_tip(&self, tip: Option<String>) {
        if let Ok(mut state) = self.0.write() {
            state.tip = tip;
        }
    }
    pub fn set_tags(&self, tags: BTreeMap<String, String>) {
        if let Ok(mut state) = self.0.write() {
            state.command_tags = tags;
        }
    }
    pub fn permits(&self, field: &str, context: &[&str], source: &str) -> bool {
        let Ok(state) = self.0.read() else {
            return false;
        };
        match (field, context) {
            ("tip", []) => state.tip.as_deref() == Some(source),
            ("command_tag", [command]) => {
                state.command_tags.get(*command).map(String::as_str) == Some(source)
            }
            _ => false,
        }
    }
}

impl DisplayCatalogs {
    pub fn contains(&self, domain: Domain) -> bool {
        self.0
            .read()
            .is_ok_and(|catalogs| catalogs.contains_key(&domain))
    }
    pub fn install(&self, catalog: Arc<DisplayCatalog>) {
        if let Ok(mut snapshots) = self.0.write() {
            snapshots.insert(catalog.domain, catalog);
        }
    }

    pub fn lookup(
        &self,
        domain: Domain,
        field: &str,
        context: &[&str],
        source: &str,
    ) -> Option<String> {
        let snapshot = self.0.read().ok()?.get(&domain).cloned()?;
        snapshot.lookup(field, context, source).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_changed_source_never_reuse_old_copy() {
        let catalog = DisplayCatalog::from_entries(
            Domain::Models,
            vec![DisplayEntry {
                field: "description".into(),
                context: vec!["grok-4.7".into()],
                source: Some("Fast. 2x the price.".into()),
                source_sha256: None,
                translation: "快速版本，价格为两倍。".into(),
            }],
        )
        .unwrap();
        assert!(
            catalog
                .lookup("description", &["grok-4.7"], "Fast. 2x the price.")
                .is_some()
        );
        assert!(
            catalog
                .lookup("description", &["custom"], "Fast. 2x the price.")
                .is_none()
        );
        assert!(
            catalog
                .lookup("description", &["grok-4.7"], "Fast. 3x the price.")
                .is_none()
        );
    }

    #[test]
    fn snapshots_are_shared_by_locale_clones_but_not_other_contexts() {
        let locale = crate::LocaleContext::new(crate::ResolvedLocale {
            locale: crate::UiLocale::ZhCn,
            source: crate::LocaleSource::Cli,
        });
        let clone = locale.clone();
        let entries = vec![DisplayEntry {
            field: "tip".into(),
            context: vec![],
            source: Some("Tip".into()),
            source_sha256: None,
            translation: "提示".into(),
        }];
        locale.install_display_catalog(Arc::new(
            DisplayCatalog::from_entries(Domain::Settings, entries).unwrap(),
        ));
        assert_eq!(
            clone
                .display_translation(Domain::Settings, "tip", &[], "Tip")
                .as_deref(),
            Some("提示")
        );
        assert!(
            crate::LocaleContext::default()
                .display_translation(Domain::Settings, "tip", &[], "Tip")
                .is_none()
        );
        locale.install_display_catalog(Arc::new(
            DisplayCatalog::from_entries(Domain::Settings, vec![]).unwrap(),
        ));
        assert!(
            clone
                .display_translation(Domain::Settings, "tip", &[], "Tip")
                .is_none()
        );
    }

    #[test]
    fn remote_settings_origin_does_not_leak_into_local_occurrences_or_other_apps() {
        let locale = crate::LocaleContext::new(crate::ResolvedLocale {
            locale: crate::UiLocale::ZhCn,
            source: crate::LocaleSource::Cli,
        });
        let entry = DisplayEntry {
            field: "tip".into(),
            context: vec![],
            source: Some("Same text".into()),
            source_sha256: None,
            translation: "中文提示".into(),
        };
        locale.install_display_catalog(Arc::new(
            DisplayCatalog::from_entries(Domain::Settings, vec![entry]).unwrap(),
        ));
        locale.set_remote_tip(Some("Same text".into()));
        assert_eq!(
            locale
                .clone()
                .remote_settings_translation("tip", &[], "Same text")
                .as_deref(),
            Some("中文提示")
        );
        locale.set_remote_tip(None); // The next selected occurrence is local but has identical text.
        assert!(
            locale
                .remote_settings_translation("tip", &[], "Same text")
                .is_none()
        );
        let other = crate::LocaleContext::new(locale.resolved());
        assert!(
            other
                .remote_settings_translation("tip", &[], "Same text")
                .is_none()
        );
    }
}
