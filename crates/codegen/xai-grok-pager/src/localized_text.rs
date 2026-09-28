//! Display-template interpolation that leaves inserted user and provider text opaque.

/// Replace full placeholders such as `{path}` only in the original template.
/// Unknown placeholders remain visible; inserted values are never scanned again.
pub(crate) fn format_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut remaining = template;
    let mut output = String::with_capacity(template.len());
    loop {
        let next = replacements
            .iter()
            .filter(|(placeholder, _)| !placeholder.is_empty())
            .filter_map(|(placeholder, value)| {
                remaining
                    .find(placeholder)
                    .map(|index| (index, *placeholder, *value))
            })
            .min_by_key(|(index, _, _)| *index);
        let Some((index, placeholder, value)) = next else {
            output.push_str(remaining);
            break;
        };
        let (before, matched) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(value);
        remaining = matched.strip_prefix(placeholder).unwrap_or_default();
    }
    output
}
