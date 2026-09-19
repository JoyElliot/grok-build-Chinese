//! Picker rows for the extensions modal's Workflows tab.

use super::{
    LocaleContext, TabDataState, WorkflowInfo, cmp_str_ci, extension_text, fuzzy_matches,
    localized_source_display,
};

/// Placeholder row when the catalog comes back empty (also what a disabled workflows feature looks like on the wire, hence the hedged phrasing).
pub(super) const WORKFLOWS_EMPTY_PLACEHOLDER: &str =
    "No workflows available. Ask Grok to help make you one!";

const DEEP_RESEARCH_DESCRIPTION: &str =
    "Research a query with bounded parallelism, cross-check the evidence, and write a cited report";
const DEEP_RESEARCH_WHEN_TO_USE: &str = "Compare, investigate, or research a question that needs sourced claims. /deep-research, research this, write a cited report.";

/// Localize only the complete, product-owned workflow tuple.  The workflow
/// name, path, source, raw filtering, and model-facing listing stay canonical.
fn localized_workflow_metadata(
    workflow: &WorkflowInfo,
    locale: Option<&LocaleContext>,
) -> (String, Option<String>) {
    if workflow.source == "builtin"
        && workflow.name == "deep-research"
        && workflow.description == DEEP_RESEARCH_DESCRIPTION
        && workflow.when_to_use.as_deref() == Some(DEEP_RESEARCH_WHEN_TO_USE)
    {
        return (
            extension_text(
                locale,
                "extensions.workflows.deep_research.description",
                DEEP_RESEARCH_DESCRIPTION,
            ),
            Some(extension_text(
                locale,
                "extensions.workflows.deep_research.when_to_use",
                DEEP_RESEARCH_WHEN_TO_USE,
            )),
        );
    }
    (workflow.description.clone(), workflow.when_to_use.clone())
}

/// One picker row for the Workflows tab (flat, browse-only catalog).
#[derive(Debug)]
pub(super) struct WorkflowRow {
    pub(super) label: String,
    pub(super) right_label: String,
    pub(super) desc_lines: Vec<String>,
    pub(super) fields: Vec<(String, String)>,
    pub(super) dimmed: bool,
}

impl WorkflowRow {
    /// Dimmed single-label row (empty catalog or fetch error).
    fn notice(label: String) -> Self {
        Self {
            label,
            right_label: String::new(),
            desc_lines: Vec::new(),
            fields: Vec::new(),
            dimmed: true,
        }
    }
}

/// Build the Workflows-tab rows, A-Z by name, fuzzy-filtered on name and description like the Hooks/Plugins tabs.
/// An empty catalog yields a single dimmed placeholder row; an error yields a dimmed error row.
pub(super) fn build_workflows_picker_rows(
    data: &TabDataState<Vec<WorkflowInfo>>,
    query: &str,
) -> Vec<WorkflowRow> {
    build_workflows_picker_rows_with_locale(data, query, None)
}

pub(super) fn build_workflows_picker_rows_with_locale(
    data: &TabDataState<Vec<WorkflowInfo>>,
    query: &str,
    locale: Option<&LocaleContext>,
) -> Vec<WorkflowRow> {
    let workflows = match data {
        TabDataState::Loaded(workflows) => workflows,
        TabDataState::Error(msg) => {
            return vec![WorkflowRow::notice(
                extension_text(locale, "extensions.error.prefix", "Error: {error}")
                    .replace("{error}", msg),
            )];
        }
        // The render gate skips entry building while loading.
        TabDataState::Loading => return Vec::new(),
    };
    if workflows.is_empty() {
        return vec![WorkflowRow::notice(extension_text(
            locale,
            "extensions.workflows.empty",
            WORKFLOWS_EMPTY_PLACEHOLDER,
        ))];
    }
    let mut visible: Vec<&WorkflowInfo> = workflows
        .iter()
        .filter(|w| fuzzy_matches(&w.name, query) || fuzzy_matches(&w.description, query))
        .collect();
    visible.sort_by(|a, b| cmp_str_ci(&a.name, &b.name));
    visible
        .into_iter()
        .map(|wf| {
            let (description, when_to_use) = localized_workflow_metadata(wf, locale);
            let mut fields = Vec::new();
            if let Some(ref p) = wf.path {
                fields.push(("path".to_string(), p.clone()));
            }
            if let Some(w) = when_to_use {
                fields.push(("when to use".to_string(), w));
            }
            WorkflowRow {
                label: wf.name.clone(),
                right_label: format!("({})", localized_source_display(locale, &wf.source)),
                desc_lines: if description.is_empty() {
                    Vec::new()
                } else {
                    vec![description]
                },
                fields,
                dimmed: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zh() -> LocaleContext {
        LocaleContext::new(crate::locale::ResolvedLocale {
            locale: crate::locale::UiLocale::ZhCn,
            source: crate::locale::LocaleSource::Cli,
        })
    }

    fn labels(rows: &[WorkflowRow]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    #[test]
    fn filters_on_name_and_description() {
        let workflows = TabDataState::Loaded(vec![
            WorkflowInfo {
                name: "alpha-wf".into(),
                description: "touches ci".into(),
                when_to_use: None,
                source: "user".into(),
                path: Some("/home/u/.grok/workflows/alpha-wf.rhai".into()),
            },
            WorkflowInfo {
                name: "beta-wf".into(),
                description: "docs".into(),
                when_to_use: None,
                source: "user".into(),
                path: None,
            },
        ]);
        let by_desc = build_workflows_picker_rows(&workflows, "ci");
        assert_eq!(labels(&by_desc), ["alpha-wf"]);
        assert_eq!(
            by_desc.first().map(|r| r.fields.as_slice()),
            Some(
                [(
                    "path".to_string(),
                    "/home/u/.grok/workflows/alpha-wf.rhai".to_string()
                )]
                .as_slice()
            )
        );
        let by_name = build_workflows_picker_rows(&workflows, "beta");
        assert_eq!(labels(&by_name), ["beta-wf"]);
        // Subsequence match, same as the Hooks/Plugins tabs.
        let by_subsequence = build_workflows_picker_rows(&workflows, "alphawf");
        assert_eq!(labels(&by_subsequence), ["alpha-wf"]);
        let none = build_workflows_picker_rows(&workflows, "zzz");
        assert!(
            none.is_empty(),
            "query misses yield no rows (picker shows its No matches state)"
        );
    }

    #[test]
    fn error_and_loading_states_build_their_own_rows() {
        let error = build_workflows_picker_rows(&TabDataState::Error("boom".into()), "");
        assert_eq!(labels(&error), ["Error: boom"]);
        assert!(error.first().is_some_and(|r| r.dimmed));
        let loading = build_workflows_picker_rows(&TabDataState::Loading, "");
        assert!(loading.is_empty());
        let empty = build_workflows_picker_rows(&TabDataState::Loaded(vec![]), "");
        assert_eq!(labels(&empty), [WORKFLOWS_EMPTY_PLACEHOLDER]);
        assert!(empty.first().is_some_and(|r| r.dimmed));
    }

    #[test]
    fn zh_localization_workflow_overlay_requires_complete_builtin_tuple() {
        let locale = zh();
        let raw_path = "/opt/grok/src/session/workflows/deep_research.rhai";
        let exact = WorkflowInfo {
            name: "deep-research".into(),
            description: DEEP_RESEARCH_DESCRIPTION.into(),
            when_to_use: Some(DEEP_RESEARCH_WHEN_TO_USE.into()),
            source: "builtin".into(),
            path: Some(raw_path.into()),
        };
        let data = TabDataState::Loaded(vec![exact.clone()]);
        let rows = build_workflows_picker_rows_with_locale(&data, "Research", Some(&locale));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "deep-research");
        assert_eq!(rows[0].right_label, "(内置)");
        assert_eq!(
            rows[0].desc_lines,
            ["在受限并行下研究问题，交叉核验证据并撰写带引用的报告"]
        );
        assert_eq!(
            rows[0].fields,
            [
                ("path".into(), raw_path.into()),
                (
                    "when to use".into(),
                    "适用于比较、调查或研究需要有来源论据的问题。可使用 /deep-research、“研究这个”或“撰写带引用的报告”。".into()
                )
            ]
        );
        assert!(
            build_workflows_picker_rows_with_locale(&data, "受限并行", Some(&locale)).is_empty(),
            "localized display copy must not become search identity"
        );

        for source in ["bundled", "project", "user"] {
            let mut dynamic = exact.clone();
            dynamic.source = source.into();
            let rows = build_workflows_picker_rows_with_locale(
                &TabDataState::Loaded(vec![dynamic]),
                "",
                Some(&locale),
            );
            assert_eq!(rows[0].desc_lines, [DEEP_RESEARCH_DESCRIPTION]);
            assert_eq!(rows[0].fields[1].1, DEEP_RESEARCH_WHEN_TO_USE);
        }

        let mut drifted = exact;
        drifted.when_to_use = Some("Updated upstream metadata".into());
        let rows = build_workflows_picker_rows_with_locale(
            &TabDataState::Loaded(vec![drifted]),
            "",
            Some(&locale),
        );
        assert_eq!(rows[0].desc_lines, [DEEP_RESEARCH_DESCRIPTION]);
        assert_eq!(rows[0].fields[1].1, "Updated upstream metadata");
    }
}
