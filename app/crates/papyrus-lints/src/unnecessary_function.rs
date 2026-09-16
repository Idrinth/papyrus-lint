//! Flags a `Function` whose body consists of exactly one statement, since
//! it adds an indirection without doing enough on its own to justify a
//! separate declaration — a caller could just as well inline that one
//! statement instead.
//!
//! Only `Function`s are checked. `Event`s are always left alone: they're
//! declared by the engine rather than the script's own author, so a
//! single-statement handler may well be forwarding to shared logic used by
//! other events too, which is a reasonable reason for it to exist on its
//! own. This works from the parsed AST, so a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unnecessary-function";

/// Checks `source` for `Function`s whose body is exactly one statement long.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_functions(&script)
        .filter(|function| !function.is_event && function.body.len() == 1)
        .map(|function| Diagnostic {
            line: function.line,
            column: 1,
            message: format!(
                "[info] Function '{}' contains only a single statement; consider inlining it \
                 at its call site(s) instead of keeping it as a separate function",
                function.name
            ),
            rule: RULE,
        })
        .collect()
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_function_with_a_single_statement() {
        let diagnostics = check("ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("'A'"));
    }

    #[test]
    fn does_not_flag_an_event_with_a_single_statement() {
        let diagnostics = check("ScriptName Example\n\nEvent OnInit()\n    B()\nEndEvent\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_with_no_statements() {
        let diagnostics = check("ScriptName Example\n\nFunction A()\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_with_more_than_one_statement() {
        let diagnostics =
            check("ScriptName Example\n\nFunction A()\n    B()\n    C()\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_single_if_statement_still_counts_as_one_statement_even_when_nested_is_bigger() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction A()\n    If ready\n        B()\n        C()\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function A()\n        B()\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'A'"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction A(\nEndFunction\n").is_empty());
    }
}
