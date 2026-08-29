//! Flags a declared identifier (a function/event, property, state,
//! parameter, or local/script variable) whose name doesn't match the
//! project's configured casing style (see [`crate::config::IdentifierCasing`]).
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a declaration's name apart from any other identifier
//! in the script; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. `ScriptName` itself is never checked, since it
//! must match the script's filename regardless of casing style.
//!
//! A parameter has no line of its own in the AST, so it's reported on its
//! enclosing function's line.

use papyrus_parser::ast::{FunctionDecl, StateDecl, Stmt};

use crate::config::IdentifierCasing;
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "identifier-casing";

/// Checks `source` for declared identifiers that don't conform to `style`.
/// Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged: it's CreationKit-generated boilerplate the user can't
/// edit or rename.
pub fn check(source: &str, style: IdentifierCasing) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();

    for variable in &script.variables {
        check_name(
            &variable.name,
            "Variable",
            variable.line,
            style,
            &protected,
            &mut diagnostics,
        );
    }
    for property in &script.properties {
        check_name(
            &property.name,
            "Property",
            property.line,
            style,
            &protected,
            &mut diagnostics,
        );
    }
    for state in &script.states {
        check_state(state, style, &protected, &mut diagnostics);
    }
    for function in &script.functions {
        check_function(function, style, &protected, &mut diagnostics);
    }

    diagnostics
}

fn check_state(
    state: &StateDecl,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_name(
        &state.name,
        "State",
        state.line,
        style,
        protected,
        diagnostics,
    );
    for function in &state.functions {
        check_function(function, style, protected, diagnostics);
    }
}

fn check_function(
    function: &FunctionDecl,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = if function.is_event {
        "Event"
    } else {
        "Function"
    };
    check_name(
        &function.name,
        kind,
        function.line,
        style,
        protected,
        diagnostics,
    );
    for param in &function.params {
        check_name(
            &param.name,
            "Parameter",
            function.line,
            style,
            protected,
            diagnostics,
        );
    }
    for stmt in &function.body {
        check_stmt(stmt, style, protected, diagnostics);
    }
}

fn check_stmt(
    stmt: &Stmt,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => check_name(
            &decl.name,
            "Variable",
            decl.line,
            style,
            protected,
            diagnostics,
        ),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                for inner in &branch.body {
                    check_stmt(inner, style, protected, diagnostics);
                }
            }
            for inner in else_body {
                check_stmt(inner, style, protected, diagnostics);
            }
        }
        Stmt::While { body, .. } => {
            for inner in body {
                check_stmt(inner, style, protected, diagnostics);
            }
        }
        Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
    }
}

fn check_name(
    name: &str,
    kind: &str,
    line: usize,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if protected.get(line).copied().unwrap_or(false) {
        return;
    }
    if style.matches(name) {
        return;
    }

    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[warning] {kind} '{name}' does not match the configured {} casing style",
            style.label()
        ),
        rule: RULE,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_property_not_matching_pascal_case() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property myValue = 1 Auto\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("Property"));
        assert!(diagnostics[0].message.contains("myValue"));
    }

    #[test]
    fn does_not_flag_property_matching_pascal_case() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n",
            IdentifierCasing::PascalCase,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_function_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction do_thing()\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Function"));
        assert!(diagnostics[0].message.contains("do_thing"));
    }

    #[test]
    fn flags_event_name() {
        let diagnostics = check(
            "ScriptName Example\n\nEvent on_init()\nEndEvent\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Event"));
        assert!(diagnostics[0].message.contains("on_init"));
    }

    #[test]
    fn flags_parameter_on_the_function_line() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Int bad_name)\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.contains("Parameter"));
        assert!(diagnostics[0].message.contains("bad_name"));
    }

    #[test]
    fn flags_local_variable_inside_a_function_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Int bad_name = 1\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Variable"));
        assert!(diagnostics[0].message.contains("bad_name"));
    }

    #[test]
    fn flags_local_variable_nested_in_if_and_while_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    If true\n        Int bad_one = 1\n    Else\n        Int bad_two = 2\n    EndIf\n    While true\n        Int bad_three = 3\n    EndWhile\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_one")));
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_two")));
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_three")));
    }

    #[test]
    fn flags_state_name() {
        let diagnostics = check(
            "ScriptName Example\n\nState bad_state\nEndState\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("State"));
        assert!(diagnostics[0].message.contains("bad_state"));
    }

    #[test]
    fn flags_function_declared_inside_a_state() {
        let diagnostics = check(
            "ScriptName Example\n\nState Idle\n    Function do_thing()\n    EndFunction\nEndState\n",
            IdentifierCasing::PascalCase,
        );

        assert!(diagnostics.iter().any(|d| d.message.contains("do_thing")));
    }

    #[test]
    fn never_flags_script_name() {
        let diagnostics = check("ScriptName not_pascal_case\n", IdentifierCasing::PascalCase);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_against_the_configured_style() {
        let source = "ScriptName Example\n\nInt Property my_value = 1 Auto\n";

        assert!(check(source, IdentifierCasing::SnakeCase).is_empty());
        assert!(!check(source, IdentifierCasing::PascalCase).is_empty());
    }

    #[test]
    fn ignores_declarations_inside_a_fragment_wrapper() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname IDR__TIF__05000235 Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function fragment_0(ObjectReference akSpeakerRef)
Actor bad_local = akSpeakerRef as Actor
;BEGIN CODE
Int GoodStyle = 1
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";

        let diagnostics = check(source, IdentifierCasing::PascalCase);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property bad_name = \"unterminated\n",
            IdentifierCasing::PascalCase,
        );
        assert!(diagnostics.is_empty());
    }
}
