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

#[cfg(test)]
#[path = "unused_disable_tests.rs"]
mod tests;
