//! The announcement update mechanism applied to other official display data.
use xai_grok_locale::{LocaleContext, UiLocale, dynamic::Domain};

use crate::announcement_translations::TranslationUpdates;

pub struct DisplayTranslationUpdates {
    workers: Vec<TranslationUpdates>,
}

impl DisplayTranslationUpdates {
    pub fn start(locale: &LocaleContext) -> Option<Self> {
        if locale.locale() != UiLocale::ZhCn {
            return None;
        }
        let offline = [
            "GROK_ZH_TRANSLATIONS_OFFLINE",
            "GROK_ZH_ANNOUNCEMENTS_OFFLINE",
            "GROK_CHANGELOG_OFFLINE",
        ]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.is_empty() && value != "0"));
        let workers: Vec<_> = Domain::ALL
            .into_iter()
            .map(|domain| TranslationUpdates::start_display(domain, !offline))
            .collect();
        for worker in &workers {
            if let Some(catalog) = worker.current().display_catalog() {
                locale.install_display_catalog(catalog);
            }
        }
        Some(Self { workers })
    }

    /// No polling: wake only on a verified cache/network snapshot from an
    /// existing official load. Closed offline workers are removed once.
    pub async fn changed(&mut self, locale: &LocaleContext) -> bool {
        loop {
            if self.workers.is_empty() {
                return false;
            }
            let pending: Vec<_> = self
                .workers
                .iter_mut()
                .map(|worker| Box::pin(worker.changed()))
                .collect();
            let (snapshot, index, remaining) = futures::future::select_all(pending).await;
            drop(remaining);
            if let Some(catalog) = snapshot.and_then(|snapshot| snapshot.display_catalog()) {
                locale.install_display_catalog(catalog);
                return true;
            }
            self.workers.remove(index);
        }
    }
}
