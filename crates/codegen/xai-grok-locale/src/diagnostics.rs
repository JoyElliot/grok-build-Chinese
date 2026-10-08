use crate::{LocaleContext, UiLocale};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticDomain {
    Mcp,
    Migration,
    Model,
}

pub fn localize(locale: &LocaleContext, domain: DiagnosticDomain, english: &str) -> String {
    if locale.locale() != UiLocale::ZhCn {
        return english.to_owned();
    }
    for &(source, key, template) in MESSAGES {
        if source != domain {
            continue;
        }
        if let Some(values) = capture(template, english) {
            let translated = locale.named_text(key, template);
            return interpolate(&translated, &values).unwrap_or_else(|| english.to_owned());
        }
    }
    english.to_owned()
}

const MESSAGES: &[(DiagnosticDomain, &str, &str)] = &[
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.empty",
        "bearer_token_file {0} is empty",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.meta_type",
        "`_meta[\"x.ai/mcp/bearerTokenFile\"]` must be a string",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.home_unknown",
        "bearer_token_file `{0}` starts with ~/ but the home directory is unknown",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.absolute",
        "bearer_token_file `{0}` must be an absolute or ~/ path",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.setup.unknown_field",
        "setup variable '{0}' references unknown field '{1}'",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.setup.unresolved",
        "unresolved setup variable '{0}'",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.invalid_toml",
        "{0}: existing config at {1} is not valid TOML ({2}). Fix the file (or move it aside) and retry.",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.not_array",
        "{0} is not an array",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.not_object",
        "{0} is not a JSON object",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.hooks_not_array",
        "{0}: hooks.{1} is not a JSON array",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.serialize_mcp",
        "failed to serialize MCP server {0}: {1}",
    ),
    (
        DiagnosticDomain::Migration,
        "diagnostic.migration.serialize_file",
        "failed to serialize {0}: {1}",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.pending",
        "an earlier read of bearer_token_file {0} has not returned",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.timeout",
        "reading bearer_token_file {0} timed out",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.header_characters",
        "bearer_token_file {0} holds characters not allowed in an HTTP header",
    ),
    (
        DiagnosticDomain::Mcp,
        "diagnostic.mcp.token_file.too_large",
        "bearer_token_file {0}: larger than 16384 bytes",
    ),
    (
        DiagnosticDomain::Model,
        "diagnostic.model.context_window.unsupported",
        "model '{0}' does not support context window {1}",
    ),
];

pub fn migration_io(locale: &LocaleContext, operation: &str, path: &str, source: &str) -> String {
    let english = format!("failed to {operation} {path}: {source}");
    if locale.locale() != UiLocale::ZhCn {
        return english;
    }
    let key = match operation {
        "read" => "diagnostic.migration.operation.read",
        "write" => "diagnostic.migration.operation.write",
        "resolve" => "diagnostic.migration.operation.resolve",
        "rename" => "diagnostic.migration.operation.rename",
        "create directory" => "diagnostic.migration.operation.create_directory",
        _ => return english,
    };
    let operation = locale.named_text(key, operation);
    let template = locale.named_text("diagnostic.migration.io", "failed to {0} {1}: {2}");
    interpolate(&template, &[&operation, path, source]).unwrap_or(english)
}

fn capture<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let mut remainder = text;
    let mut template = template;
    let mut values = Vec::new();
    while let Some((prefix, after_open)) = template.split_once('{') {
        remainder = remainder.strip_prefix(prefix)?;
        let (index, after_close) = after_open.split_once('}')?;
        if index.parse::<usize>().ok()? != values.len() {
            return None;
        }
        let literal = after_close
            .split_once('{')
            .map_or(after_close, |(literal, _)| literal);
        if after_close.contains('{') {
            if literal.is_empty() {
                return None;
            }
            let (value, rest) = remainder.split_once(literal)?;
            if rest.contains(literal) {
                return None;
            }
            values.push(value);
            remainder = &remainder[value.len()..];
        } else {
            let value = remainder.strip_suffix(literal)?;
            values.push(value);
            remainder = &remainder[value.len()..];
        }
        template = after_close;
    }
    (remainder == template).then_some(values)
}

// Insert captured paths and error details once; placeholder-like text in them stays opaque.
fn interpolate(template: &str, values: &[&str]) -> Option<String> {
    let mut result = String::new();
    let mut remaining = template;
    while let Some((prefix, after_open)) = remaining.split_once('{') {
        result.push_str(prefix);
        let (index, after_close) = after_open.split_once('}')?;
        result.push_str(values.get(index.parse::<usize>().ok()?)?);
        remaining = after_close;
    }
    result.push_str(remaining);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocaleSource, ResolvedLocale};

    fn zh() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Requirement,
        })
    }

    #[test]
    fn migration_io_keeps_paths_and_nested_errors_opaque() {
        let text = migration_io(&zh(), "create directory", "/tmp/a b/{2}", "raw: {1}");
        assert_eq!(text, "创建目录失败，路径：/tmp/a b/{2}。原因：raw: {1}");
        assert_eq!(
            migration_io(&zh(), "custom verb", "/tmp/a", "raw"),
            "failed to custom verb /tmp/a: raw"
        );
        assert_eq!(
            migration_io(&LocaleContext::default(), "read", "/tmp/a", "raw"),
            "failed to read /tmp/a: raw"
        );
    }

    #[test]
    fn known_diagnostics_translate_only_the_display_copy() {
        for &(domain, key, template) in MESSAGES {
            let values = ["opaque0", "opaque1", "opaque2", "opaque3"];
            let original = interpolate(template, &values).unwrap();
            let rendered = localize(&zh(), domain, &original);
            assert_ne!(rendered, original, "{key}");
            let captured = capture(template, &original).unwrap();
            for value in captured {
                assert!(rendered.contains(value), "{key}: {rendered}");
            }
            assert_eq!(
                localize(&LocaleContext::default(), domain, &original),
                original
            );
        }
    }

    #[test]
    fn unknown_messages_and_other_domains_remain_opaque() {
        let raw = "provider says: bearer_token_file /tmp/token is empty";
        assert_eq!(localize(&zh(), DiagnosticDomain::Mcp, raw), raw);
        let raw = "bearer_token_file /tmp/token is empty";
        assert_eq!(localize(&zh(), DiagnosticDomain::Migration, raw), raw);
        assert_eq!(
            localize(&zh(), DiagnosticDomain::Mcp, "arbitrary server error"),
            "arbitrary server error"
        );
    }

    #[test]
    fn payloads_are_not_interpreted_as_translation_placeholders() {
        let raw = "failed to serialize /tmp/{1}.toml: server detail {0}";
        let display = localize(&zh(), DiagnosticDomain::Migration, raw);
        assert!(display.contains("/tmp/{1}.toml"));
        assert!(display.contains("server detail {0}"));
        let ambiguous =
            "setup variable 'x' references unknown field 'y' references unknown field 'z'";
        assert_eq!(localize(&zh(), DiagnosticDomain::Mcp, ambiguous), ambiguous);
    }
}
