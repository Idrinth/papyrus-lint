//! Flags a script-level `Property` or variable (declared directly on the
//! script, outside any function) whose name matches (case-insensitively)
//! the name of the script it's declared in, since Papyrus doesn't allow a
//! declared identifier to collide with the script's own type name — such a
//! script fails to compile.
//!
//! Works from the parsed AST rather than raw tokens, since a script's own
//! declared name, properties, and variables are already tracked there. A
//! local variable declared inside a function isn't checked here — see
//! [`crate::local_variable_shadowing`] for shadowing concerns local to a
//! function body. A script that doesn't parse cleanly is left unchecked
//! rather than guessed at.

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "script-name-collision";

/// Checks `source` for a script-level `Property` or variable declaration
/// whose name matches (case-insensitively) the enclosing script's own
/// declared name. Flagged as an `[error]`, since Papyrus rejects such a
/// script at compile time.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for property in &script.properties {
        if property.name.eq_ignore_ascii_case(&script.name) {
            diagnostics.push(Diagnostic {
                line: property.line,
                column: 1,
                message: format!(
                    "[error] Property '{}' may not share its name with the script it's declared in ('{}')",
                    property.name, script.name
                ),
                rule: RULE,
            });
        }
    }
    for variable in &script.variables {
        if variable.name.eq_ignore_ascii_case(&script.name) {
            diagnostics.push(Diagnostic {
                line: variable.line,
                column: 1,
                message: format!(
                    "[error] Variable '{}' may not share its name with the script it's declared in ('{}')",
                    variable.name, script.name
                ),
                rule: RULE,
            });
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_property_named_identically_to_its_script() {
        let diagnostics =
            check("ScriptName Example\n\nInt Property Example Auto\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0].message.contains("Property 'Example'"));
    }

    #[test]
    fn flags_a_variable_named_identically_to_its_script() {
        let diagnostics = check("ScriptName Example\n\nInt Example = 1\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.contains("Variable 'Example'"));
    }

    #[test]
    fn matches_the_script_name_case_insensitively() {
        let diagnostics = check("ScriptName Example\n\nInt Property example Auto\n");

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_an_unrelated_property_or_variable_name() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nInt total = 1\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_every_colliding_declaration_on_the_same_script() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property Example Auto\n\nInt Example = 1\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_a_local_variable_sharing_the_script_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int Example = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nInt Property Example(\n").is_empty());
    }
}
