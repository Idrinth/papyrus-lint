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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disable_comments::Disables;

    const KNOWN_RULES: &[&str] = &["comma-spacing", "trailing-whitespace"];

    fn diagnostic(line: usize, rule: &'static str) -> Diagnostic {
        Diagnostic {
            line,
            column: 1,
            message: "[warning] test diagnostic".into(),
            rule,
        }
    }

    #[test]
    fn bare_disable_is_used_when_any_diagnostic_occurs_on_the_line() {
        let disables = Disables::scan("Call(1,2) ; @disable\n");
        let diagnostics = [diagnostic(1, "comma-spacing")];

        assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
    }

    #[test]
    fn bare_disable_is_unused_when_diagnostic_occurs_on_another_line() {
        let disables = Disables::scan("; @disable\nCall(1,2)\n");
        let diagnostics = [diagnostic(2, "comma-spacing")];

        let unused = check(&disables, &diagnostics, KNOWN_RULES);

        assert_eq!(unused.len(), 1);
        assert_eq!((unused[0].line, unused[0].column), (1, 3));
        assert!(unused[0]
            .message
            .contains("does not produce any diagnostics"));
    }

    #[test]
    fn named_disable_matches_rule_ids_case_insensitively() {
        let disables = Disables::scan("Call(1,2) ; @disable COMMA-SPACING\n");
        let diagnostics = [diagnostic(1, "comma-spacing")];

        assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
    }

    #[test]
    fn named_disables_distinguish_unknown_and_untriggered_rules() {
        let disables = Disables::scan("Call(1, 2) ; @disable mystery-rule, trailing-whitespace\n");

        let unused = check(&disables, &[], KNOWN_RULES);

        assert_eq!(unused.len(), 2);
        assert_eq!(unused[0].column, 23);
        assert!(unused[0].message.contains("mystery-rule"));
        assert!(unused[0].message.contains("unknown"));
        assert_eq!(unused[1].column, 37);
        assert!(unused[1].message.contains("trailing-whitespace"));
        assert!(unused[1].message.contains("does not produce"));
    }

    #[test]
    fn reports_are_sorted_by_source_location() {
        let disables = Disables::scan(
            "; @disable trailing-whitespace, mystery\nclean\n; @disable comma-spacing\n",
        );

        let unused = check(&disables, &[], KNOWN_RULES);
        let locations: Vec<_> = unused
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect();

        assert_eq!(locations, vec![(1, 12), (1, 33), (3, 12)]);
        assert!(unused.iter().all(|diagnostic| diagnostic.rule == RULE));
    }
}
