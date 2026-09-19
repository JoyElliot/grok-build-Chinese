//! `FEATURES` is the source of truth and the public operator table is a hand-maintained mirror.
//! Check the documentation shipped in the standalone repository, not monorepo-only internal docs.

use xai_grok_shell::agent::config::FEATURES;

const CONFIG_REFERENCE: &str = include_str!("../docs/user-guide/26-config-reference.md");

#[test]
fn every_registered_feature_reaches_the_operator() {
    for spec in FEATURES {
        let prefix = format!("| `{}` |", spec.path);
        let row = CONFIG_REFERENCE
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("{} has no row in 26-config-reference.md", spec.path));
        assert!(
            row.contains("| `pin` |"),
            "{} is missing its requirements pin in 26-config-reference.md",
            spec.path,
        );
        assert!(
            row.contains(&format!("`{}`", spec.env)),
            "{} is undocumented in the {} row of 26-config-reference.md",
            spec.env,
            spec.path,
        );
    }
}
