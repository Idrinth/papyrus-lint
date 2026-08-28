//! Flags local variables that shadow a `Property` declared on the same
//! script or on a parent script, since reading the name inside the
//! function then reads the local rather than the property, which is a
//! common source of confusion (and, once the local goes out of scope
//! conceptually, bugs).
//!
//! Like [`crate::argument_types`], this works from the parsed AST rather
//! than raw tokens, and reuses that module's
//! [`argument_types::ExternalSignatures`] trait so a caller that can
//! resolve other scripts (e.g. the desktop app's `FunctionTable`) can also
//! check shadowing against a parent script's properties, not just the
//! linted script's own. A script that doesn't parse cleanly is left
//! unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{FunctionDecl, Script, Stmt, VariableDecl};

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "local-variable-shadowing";

/// Checks `source` for local variables that shadow a property declared on
/// the same script. Shadowing a property declared on a parent script isn't
/// checked this way, since resolving parent scripts to files requires
/// filesystem access this crate deliberately doesn't have; see
/// [`check_with`] for that. Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but also flags a local variable that shadows a property
/// declared on a parent script, resolved (including through `Extends`)
/// through `external`.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let own_properties: HashSet<String> = script
        .properties
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        for decl in collect_var_decls(&function.body) {
            diagnostics.extend(check_decl(decl, &script, &own_properties, external));
        }
    }
    diagnostics
}

fn check_decl<E: ExternalSignatures>(
    decl: &VariableDecl,
    script: &Script,
    own_properties: &HashSet<String>,
    external: &mut E,
) -> Option<Diagnostic> {
    let name_lower = decl.name.to_ascii_lowercase();

    if own_properties.contains(&name_lower) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows this script's own property '{}'",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    let parent = script.extends.as_ref()?;
    if external.has_property(parent, &decl.name) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows a property '{}' inherited from a parent script",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    None
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

/// Finds every `VariableDecl` in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies, since Papyrus locals
/// aren't block-scoped.
fn collect_var_decls(body: &[Stmt]) -> Vec<&VariableDecl> {
    let mut decls = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => decls.push(decl),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    decls.extend(collect_var_decls(&branch.body));
                }
                decls.extend(collect_var_decls(else_body));
            }
            Stmt::While { body, .. } => decls.extend(collect_var_decls(body)),
            _ => {}
        }
    }
    decls
}

#[cfg(test)]
mod tests {
    use super::*;
    use papyrus_parser::ast::TypeName;

    #[test]
    fn flags_local_variable_shadowing_own_property() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("own property"));
        assert!(diagnostics[0].message.contains("'MyValue'"));
    }

    #[test]
    fn matches_property_shadowing_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int myvalue = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_local_variable_with_no_matching_property() {
        let diagnostics =
            check("ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int total = 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_variable_declared_inside_if_block() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    If true\n        Int MyValue = 1\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn flags_variables_in_every_nested_control_flow_body() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    If true\n        While true\n            Int MyValue = 1\n        EndWhile\n    ElseIf false\n        Int myvalue = 2\n    Else\n        Int MYVALUE = 3\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.line)
                .collect::<Vec<_>>(),
            vec![8, 11, 13]
        );
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nState Active\n    Function Test()\n        Int MyValue = 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'MyValue'"));
    }

    #[test]
    fn does_not_flag_function_parameters_or_unrelated_properties() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test(Int MyValue)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_parent_shadowing_without_an_external_resolver() {
        let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    struct FakeExternalWithProperty;

    impl ExternalSignatures for FakeExternalWithProperty {
        fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<TypeName>> {
            None
        }

        fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("BaseScript")
                && property_name.eq_ignore_ascii_case("MyValue")
        }
    }

    #[test]
    fn flags_local_variable_shadowing_a_parent_property_through_external_resolver() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("inherited from a parent script"));
    }

    #[test]
    fn does_not_flag_unrelated_variable_through_external_resolver() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int total = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn own_property_takes_precedence_over_external_lookup() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("own property"));
    }
}
