//! Shared reasoning-effort dropdown levels for `/model` and `/effort`.

use xai_grok_shell::sampling::types::{ReasoningEffort, ReasoningEffortOption};

use crate::slash::command::{ArgItem, ArgPresentation};

pub(crate) fn is_official_model(info: &agent_client_protocol::ModelInfo) -> bool {
    info.meta
        .as_ref()
        .and_then(|meta| meta.get(xai_grok_shell::agent::config::OFFICIAL_MODEL_META_KEY))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

pub(crate) fn stamp_effort_presentations(
    items: &mut [ArgItem],
    options: &[ReasoningEffortOption],
    model_id: &agent_client_protocol::ModelId,
    info: &agent_client_protocol::ModelInfo,
) {
    let official = is_official_model(info);
    let remote_options = info
        .meta
        .as_ref()
        .is_some_and(|meta| meta.contains_key("reasoningEfforts"));
    for (item, option) in items.iter_mut().zip(options) {
        if official {
            item.presentation = Some(ArgPresentation::OfficialEffort {
                model_id: model_id.0.to_string(),
                option_id: option.id.clone(),
                is_current: item.display.ends_with(" (active)"),
            });
        } else if remote_options {
            item.presentation = Some(ArgPresentation::Opaque);
        }
    }
}

/// Effort levels in the built-in fallback menu (strongest first).
/// `none`/`minimal` are still accepted by `ReasoningEffort::from_str` for power users.
pub(crate) const EFFORT_LEVELS: &[ReasoningEffort] = &[
    ReasoningEffort::Xhigh,
    ReasoningEffort::High,
    ReasoningEffort::Medium,
    ReasoningEffort::Low,
];

pub(crate) fn effort_description(level: ReasoningEffort) -> &'static str {
    match level {
        ReasoningEffort::None => "No reasoning",
        ReasoningEffort::Minimal => "Minimal reasoning",
        ReasoningEffort::Low => "Faster, lighter reasoning",
        ReasoningEffort::Medium => "Balanced reasoning",
        ReasoningEffort::High => "Heavy reasoning",
        ReasoningEffort::Xhigh => "Extended reasoning",
        ReasoningEffort::Max => "Maximum reasoning",
    }
}

/// The built-in menu used when the server sends no `reasoningEfforts`.
/// Reproduces the historical rows: labels are the lowercase level (via `Display`), descriptions from `effort_description`.
/// The active row is matched by value against the session effort at render time, so `default` is left unset here.
pub(crate) fn legacy_effort_options() -> Vec<ReasoningEffortOption> {
    EFFORT_LEVELS
        .iter()
        .map(|&level| ReasoningEffortOption {
            id: level.as_ref().to_string(),
            value: level,
            label: level.to_string(),
            description: Some(effort_description(level).to_string()),
            default: false,
        })
        .collect()
}

/// Build effort rows for autocomplete from a per-model option list. `match_text` gets an `a `/`b `/…` sort prefix
/// so the matcher's alphabetical tiebreak preserves the option order.
pub(crate) fn build_effort_arg_items(
    options: &[ReasoningEffortOption],
    current_effort: Option<ReasoningEffort>,
    mark_active: bool,
    insert_text_for: impl Fn(&ReasoningEffortOption) -> String,
) -> Vec<ArgItem> {
    options
        .iter()
        .enumerate()
        .map(|(idx, option)| {
            let active = mark_active && current_effort == Some(option.value);
            let active_suffix = if active { " (active)" } else { "" };
            let insert_text = insert_text_for(option);
            // Sort-key prefix: 'a' for top row, 'b' for next, etc
            // Only affects matcher tiebreak ordering, never rendered
            let sort_prefix = char::from(b'a' + idx as u8);
            ArgItem {
                display: format!("{}{active_suffix}", option.label),
                match_text: format!("{sort_prefix} {insert_text}"),
                insert_text,
                description: option.description.clone().unwrap_or_default(),
                presentation: Some(ArgPresentation::ReasoningEffort(option.value)),
            }
        })
        .collect()
}
