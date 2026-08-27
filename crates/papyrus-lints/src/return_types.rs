//! Flags `Return` statements whose value doesn't match the enclosing
//! function's declared return type.
//!
//! Like [`crate::argument_types`], this works from the parsed AST (via
//! `papyrus_parser::types`) since it needs to know declared types, and
//! reuses that module's [`argument_types::ExternalSignatures`] trait so a
//! caller that can resolve other scripts' `Extends` chains (e.g. the
//! desktop app's `FunctionTable`) lets a returned value whose type is a
//! *subtype* of the declared return type pass, the same way argument
//! type-checking accepts a child-type argument for a parameter typed as
//! one of its ancestors.
//!
//! A function with no declared return type isn't checked (`Return` with a
//! value there is a script author error of a different kind), and a
//! `Return` whose value's type can't be determined from the script alone
//! is skipped rather than guessed at, to keep false positives rare.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::{self, ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "return-types";

/// Checks `source` for `Return` values whose type doesn't match (or isn't a
/// subtype of) the enclosing function's declared return type. Subtype
/// relationships to scripts outside `source` are never resolved this way;
/// see [`check_with`] for that.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves object-type return values through
/// `external` so a value whose script extends (directly or transitively)
/// the declared return type is accepted.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        let Some(return_type) = function.return_type.clone() else {
            continue;
        };
        env.with_function_scope(function, |scoped| {
            check_body(
                &function.body,
                scoped,
                &return_type,
                &function.name,
                external,
                &mut diagnostics,
            );
        });
    }

    diagnostics
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

fn check_body<E: ExternalSignatures>(
    body: &[Stmt],
    env: &TypeEnv,
    return_type: &TypeName,
    function_name: &str,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                check_return(
                    *line,
                    value,
                    return_type,
                    function_name,
                    env,
                    external,
                    diagnostics,
                );
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, env, return_type, function_name, external, diagnostics);
                }
                check_body(
                    else_body,
                    env,
                    return_type,
                    function_name,
                    external,
                    diagnostics,
                );
            }
            Stmt::While { body, .. } => {
                check_body(body, env, return_type, function_name, external, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } => {}
        }
    }
}

fn check_return<E: ExternalSignatures>(
    line: usize,
    value: &Expr,
    return_type: &TypeName,
    function_name: &str,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(value, Expr::Literal(Literal::None)) {
        if !argument_types::accepts_none(return_type) {
            diagnostics.push(mismatch(line, function_name, return_type, "None"));
        }
        return;
    }

    let Some(value_type) = infer_type(value, env) else {
        return;
    };
    if !argument_types::is_compatible(return_type, &value_type, external) {
        diagnostics.push(mismatch(
            line,
            function_name,
            return_type,
            &argument_types::format_type(&value_type),
        ));
    }
}

fn mismatch(line: usize, function_name: &str, return_type: &TypeName, got: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: 1,
        message: format!(
            "[error] Function '{}' declares return type {} but returns {}",
            function_name,
            argument_types::format_type(return_type),
            got
        ),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mismatched_return_value() {
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("'Test'"));
        assert!(diagnostics[0].message.contains("declares return type Int"));
        assert!(diagnostics[0].message.contains("returns String"));
    }

    #[test]
    fn allows_matching_return_value() {
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Return 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_int_returned_from_float_function() {
        let diagnostics =
            check("ScriptName Example\n\nFloat Function Test()\n    Return 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_functions_with_no_declared_return_type() {
        let diagnostics = check("ScriptName Example\n\nFunction Test()\n    Return\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_bare_return_in_a_typed_function() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    If true\n        Return\n    EndIf\n    Return 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_none_returned_from_a_primitive_function() {
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Return None\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("returns None"));
    }

    #[test]
    fn allows_none_returned_from_an_object_typed_function() {
        let diagnostics =
            check("ScriptName Example\n\nActor Function Test()\n    Return None\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_returns_nested_in_if_and_while_blocks() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return \"hi\"\n    Else\n        While flag\n            Return \"bye\"\n        EndWhile\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_unresolvable_return_value() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Helper()\n    Return 1\nEndFunction\n\nInt Function Test()\n    Return Helper()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nInt Function Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    struct FakeExternalWithSubtypes;

    impl ExternalSignatures for FakeExternalWithSubtypes {
        fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<TypeName>> {
            None
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
        }
    }

    #[test]
    fn accepts_a_return_value_whose_script_extends_the_declared_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nForm Function Test()\n    Return MyArmor\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_an_unrelated_object_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nForm Function Test()\n    Return MyWeapon\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("declares return type Form"));
        assert!(diagnostics[0].message.contains("returns Weapon"));
    }
}
