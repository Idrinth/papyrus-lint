//! Flags a function or event declared inside a `State` block whose
//! parameter list or return type doesn't match the same-named declaration
//! in the script's "empty state" (i.e. the one declared directly on the
//! script, outside any `State` block).
//!
//! Per the CreationKit wiki's [State
//! Reference](https://ck.uesp.net/wiki/State_Reference): "Every function
//! implemented in a state must also be implemented (with an identical
//! name, return type, and parameter list) in the empty state in either the
//! current script or a parent." A state function whose signature drifts
//! from that empty-state version isn't recognized as an override of it at
//! all, so it silently becomes a distinct, effectively unreachable
//! function instead of the behavior swap the author presumably intended.
//!
//! This only compares against an empty-state declaration already present
//! on the script being linted. Per the quote above, a state function may
//! instead match one declared on a *parent* script, which this lint has no
//! way to resolve (unlike e.g. [`crate::function_override`], there's no
//! `ExternalSignatures` lookup for "the parent's empty-state declaration of
//! this exact name"), so a state function with no local empty-state
//! counterpart is left unflagged rather than guessed at.

use papyrus_parser::ast::FunctionDecl;

use crate::argument_types::format_type;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "state-function-signature";

/// Checks `source` for state-declared functions/events whose parameter
/// list or return type doesn't match the same-named declaration in the
/// script's empty state.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for state in &script.states {
        for function in &state.functions {
            let Some(base) = script
                .functions
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(&function.name))
            else {
                continue;
            };

            check_function(function, base, state.name.as_str(), &mut diagnostics);
        }
    }

    diagnostics
}

fn check_function(
    state_fn: &FunctionDecl,
    base_fn: &FunctionDecl,
    state_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if state_fn.params.len() != base_fn.params.len() {
        diagnostics.push(Diagnostic {
            line: state_fn.line,
            column: 1,
            message: format!(
                "[error] Function '{}' in state '{}' declares {} but the empty state's declaration declares {}",
                state_fn.name,
                state_name,
                param_count(state_fn.params.len()),
                param_count(base_fn.params.len()),
            ),
            rule: RULE,
        });
    } else {
        for (index, (state_param, base_param)) in
            state_fn.params.iter().zip(&base_fn.params).enumerate()
        {
            if state_param.type_name != base_param.type_name {
                diagnostics.push(Diagnostic {
                    line: state_fn.line,
                    column: 1,
                    message: format!(
                        "[error] Parameter {} of '{}' in state '{}' is declared {} but the empty state's declaration declares {}",
                        index + 1,
                        state_fn.name,
                        state_name,
                        format_type(&state_param.type_name),
                        format_type(&base_param.type_name),
                    ),
                    rule: RULE,
                });
            }
        }
    }

    if state_fn.return_type != base_fn.return_type {
        diagnostics.push(Diagnostic {
            line: state_fn.line,
            column: 1,
            message: format!(
                "[error] Function '{}' in state '{}' declares return type {} but the empty state's declaration declares {}",
                state_fn.name,
                state_name,
                format_return_type(&state_fn.return_type),
                format_return_type(&base_fn.return_type),
            ),
            rule: RULE,
        });
    }
}

fn param_count(count: usize) -> String {
    if count == 1 {
        "1 parameter".to_string()
    } else {
        format!("{count} parameters")
    }
}

fn format_return_type(return_type: &Option<papyrus_parser::ast::TypeName>) -> String {
    match return_type {
        Some(type_name) => format_type(type_name),
        None => "no value".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_state_function_with_a_mismatched_parameter_type() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0]
            .message
            .contains("Parameter 1 of 'Greet' in state 'Loud'"));
        assert!(diagnostics[0].message.contains("is declared Int"));
        assert!(diagnostics[0].message.contains("declares String"));
    }

    #[test]
    fn flags_a_state_function_with_a_mismatched_parameter_count() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(String name, Int volume)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Function 'Greet' in state 'Loud'"));
        assert!(diagnostics[0].message.contains("declares 2 parameters"));
        assert!(diagnostics[0]
            .message
            .contains("empty state's declaration declares 1 parameter"));
    }

    #[test]
    fn flags_a_state_function_with_a_mismatched_return_type() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function MyFunction()\n    Return 1\nEndFunction\n\nState MyState\n    String Function MyFunction()\n        Return \"hi\"\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Function 'MyFunction' in state 'MyState'"));
        assert!(diagnostics[0]
            .message
            .contains("declares return type String"));
        assert!(diagnostics[0]
            .message
            .contains("empty state's declaration declares Int"));
    }

    #[test]
    fn flags_a_state_function_that_gained_a_return_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction()\nEndFunction\n\nState MyState\n    Int Function MyFunction()\n        Return 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("declares return type Int"));
        assert!(diagnostics[0]
            .message
            .contains("empty state's declaration declares no value"));
    }

    #[test]
    fn allows_a_matching_state_override() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function MyFunction()\n    Return 1\nEndFunction\n\nState MyState\n    Int Function MyFunction()\n        Return 2\n    EndFunction\nEndState\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_a_matching_state_override_with_parameters() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(String name)\n    EndFunction\nEndState\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_state_function_with_no_local_empty_state_counterpart() {
        // This might still be valid via a parent script's empty state,
        // which this lint has no way to resolve; see the module docs.
        let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function Greet(Int volume)\n    EndFunction\nEndState\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_function_names_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function GREET(Int name)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_each_state_separately() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n\nState Quiet\n    Function Greet(Bool name)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nState Loud\n    Function Greet(\n    EndFunction\nEndState\n",
        );
        assert!(diagnostics.is_empty());
    }
}
