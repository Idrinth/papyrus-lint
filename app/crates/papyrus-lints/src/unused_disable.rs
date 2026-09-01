//! Reports `@disable` directives which do not suppress a diagnostic.

use crate::{disable_comments::LineDisable, Diagnostic};

/// This lint's [`Diagnostic::rule`] id.
pub const RULE: &str = "unused-disable";

pub(crate) fn check(
    disables: &crate::disable_comments::Disables,
    diagnostics: &[Diagnostic],
    known_rules: &[&str],
) -> Vec<Diagnostic> {
    let mut unused = Vec::new();
    for (line, disable) in disables.iter() {
        match disable {
            LineDisable::All { column } => {
                if !diagnostics.iter().any(|diagnostic| diagnostic.line == line) {
                    unused.push(Diagnostic {
                        line,
                        column: *column,
                        message:
                            "[warning] Unused @disable: this line does not produce any diagnostics"
                                .into(),
                        rule: RULE,
                    });
                }
            }
            LineDisable::Rules(rules) => {
                for disabled in rules {
                    let known = known_rules.contains(&disabled.id.as_str());
                    let triggered = diagnostics.iter().any(|diagnostic| {
                        diagnostic.line == line
                            && diagnostic.rule.eq_ignore_ascii_case(&disabled.id)
                    });
                    if !known || !triggered {
                        let reason = if known {
                            "this line does not produce a diagnostic from that rule"
                        } else {
                            "the rule id is unknown"
                        };
                        unused.push(Diagnostic {
                            line,
                            column: disabled.column,
                            message: format!(
                                "[warning] Unused @disable `{}`: {reason}",
                                disabled.id
                            ),
                            rule: RULE,
                        });
                    }
                }
            }
        }
    }
    unused.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));
    unused
}
