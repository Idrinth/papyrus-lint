//! Reports `@disable`/`@disable-file` directives which do not suppress a
//! diagnostic.

use crate::{disable_comments::Directive, Diagnostic};

/// This lint's [`Diagnostic::rule`] id.
pub const RULE: &str = "unused-disable";

pub(crate) fn check(
    disables: &crate::disable_comments::Disables,
    diagnostics: &[Diagnostic],
    known_rules: &[&str],
) -> Vec<Diagnostic> {
    let mut unused = Vec::new();
    for (line, disable) in disables.iter() {
        unused.extend(unused_directive(
            line,
            disable,
            known_rules,
            |id| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.line == line && diagnostic.rule.eq_ignore_ascii_case(id)
                })
            },
            |known| {
                if known {
                    "this line does not produce a diagnostic from that rule"
                } else {
                    "the rule id is unknown"
                }
            },
            diagnostics.iter().any(|diagnostic| diagnostic.line == line),
            "[warning] Unused @disable: this line does not produce any diagnostics",
            "@disable",
        ));
    }
    for (line, disable) in disables.file_iter() {
        unused.extend(unused_directive(
            line,
            disable,
            known_rules,
            |id| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule.eq_ignore_ascii_case(id))
            },
            |known| {
                if known {
                    "this file does not produce a diagnostic from that rule"
                } else {
                    "the rule id is unknown"
                }
            },
            !diagnostics.is_empty(),
            "[warning] Unused @disable-file: this file does not produce any diagnostics",
            "@disable-file",
        ));
    }
    unused.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));
    unused
}

#[allow(clippy::too_many_arguments)]
fn unused_directive<'a>(
    line: usize,
    disable: &'a Directive,
    known_rules: &'a [&str],
    triggered: impl Fn(&str) -> bool,
    reason: impl Fn(bool) -> &'static str,
    any_on_target: bool,
    all_message: &'static str,
    directive: &'static str,
) -> Vec<Diagnostic> {
    match disable {
        Directive::All { column } => {
            if any_on_target {
                Vec::new()
            } else {
                vec![Diagnostic {
                    line,
                    column: *column,
                    message: all_message.into(),
                    rule: RULE,
                }]
            }
        }
        Directive::Rules(rules) => rules
            .iter()
            .filter_map(|disabled| {
                let known = known_rules.contains(&disabled.id.as_str());
                if known && triggered(&disabled.id) {
                    return None;
                }
                Some(Diagnostic {
                    line,
                    column: disabled.column,
                    message: format!(
                        "[warning] Unused {directive} `{}`: {}",
                        disabled.id,
                        reason(known)
                    ),
                    rule: RULE,
                })
            })
            .collect(),
    }
}

/// Drops unused `@disable`/`@disable-file` rule ids (or the whole
/// directive when none of its ids suppress anything). Re-lints `source`
/// with [`crate::registry::collect_diagnostics`] so the set of comments
/// this removes matches what [`check`] would flag for the same file.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let diagnostics = crate::registry::collect_diagnostics(
        source,
        config,
        &mut crate::external_signatures::NoExternalSignatures,
    );
    let disables = crate::disable_comments::Disables::scan(source);
    let unused = check(&disables, &diagnostics, crate::KNOWN_RULE_IDS);
    if unused.is_empty() {
        return source.to_string();
    }

    let unused_ids_by_line = unused_ids_by_line(&unused);
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;
    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        let (content, ending) = if let Some(stripped) = line_and_ending.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = line_and_ending.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (line_and_ending, "")
        };
        if let Some(ids) = unused_ids_by_line.get(&line_number) {
            result.push_str(&rewrite_disable_line(content, ids));
            result.push_str(ending);
        } else {
            result.push_str(line_and_ending);
        }
        rest = remainder;
        line_number += 1;
    }
    result
}

fn unused_ids_by_line(unused: &[Diagnostic]) -> std::collections::HashMap<usize, Vec<String>> {
    let mut map: std::collections::HashMap<usize, Vec<String>> = std::collections::HashMap::new();
    for diagnostic in unused {
        let id = unused_rule_id(&diagnostic.message);
        map.entry(diagnostic.line).or_default().push(id);
    }
    map
}

/// The disabled rule id named in an unused-disable diagnostic, or `"*"` for
/// a bare `@disable`/`@disable-file` that suppresses nothing at all.
fn unused_rule_id(message: &str) -> String {
    const MARKER: &str = "Unused @disable";
    const FILE_MARKER: &str = "Unused @disable-file";
    let rest = message
        .split_once(FILE_MARKER)
        .or_else(|| message.split_once(MARKER))
        .map(|(_, rest)| rest)
        .unwrap_or(message);
    let rest = rest.trim_start_matches('-');
    let rest = rest.trim_start();
    if let Some(rest) = rest.strip_prefix('`') {
        rest.split('`').next().unwrap_or("*").to_ascii_lowercase()
    } else {
        "*".to_string()
    }
}

fn rewrite_disable_line(line: &str, unused_ids: &[String]) -> String {
    use papyrus_parser::comment_annotations::parse_line_annotations;

    let annotations = parse_line_annotations(line);
    let Some(annotation) = annotations.iter().find(|annotation| {
        annotation.name.eq_ignore_ascii_case("disable")
            || annotation.name.eq_ignore_ascii_case("disable-file")
            || annotation.name.eq_ignore_ascii_case("file-disable")
    }) else {
        return line.to_string();
    };

    let drop_all = unused_ids.iter().any(|id| id == "*");
    let remaining: Vec<&str> = if drop_all || annotation.arguments.is_empty() {
        Vec::new()
    } else {
        annotation
            .arguments
            .split(|character: char| character == ',' || character.is_whitespace())
            .filter(|part| !part.is_empty())
            .filter(|part| {
                !unused_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(part))
            })
            .collect()
    };

    if remaining.is_empty() {
        let before = line[..annotation.byte_start].trim_end();
        let after = line[annotation.byte_end..].trim_start();
        if after.is_empty() {
            if let Some(stripped) = before.strip_suffix(';') {
                return stripped.trim_end().to_string();
            }
            return before.to_string();
        }
        if let Some(stripped) = before.strip_suffix(';') {
            let code = stripped.trim_end();
            if code.is_empty() {
                return format!("; {after}");
            }
            return format!("{code} ; {after}");
        }
        return format!("{before}{after}");
    }

    format!(
        "{}@{} {}{}",
        &line[..annotation.byte_start],
        annotation.name,
        remaining.join(", "),
        &line[annotation.byte_end..]
    )
}

#[cfg(test)]
#[path = "unused_disable_tests.rs"]
mod tests;
