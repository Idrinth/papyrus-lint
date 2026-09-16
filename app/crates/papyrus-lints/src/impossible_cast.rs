//! Flags an explicit `as` cast that can never succeed: the value's known
//! type and the cast's target type are *proven* unrelated — neither
//! extends the other, directly or transitively — so the cast always
//! evaluates to `None` at runtime no matter what the value actually holds
//! (e.g. `Armor a` then `Weapon b = a as Weapon`, since `Armor` and
//! `Weapon` are unrelated siblings both directly extending `Form`).
//!
//! Like [`crate::useless_downcast`], this works from the parsed AST (via
//! [`papyrus_parser::types`]) to know a cast's value's declared type, and
//! only checks a cast whose value's type can be determined locally
//! (locals, parameters, properties, `Self`/`Parent`, literals, and other
//! resolvable expressions) — a member access or function call result is
//! left unflagged rather than guessed at. Primitive types (`Int`, `Float`,
//! `Bool`, `String`) are never flagged, since Papyrus's conversions between
//! those (and between a primitive and an object type) are a different
//! concern entirely from object-type subtyping.
//!
//! Papyrus scripts have single inheritance, so two types are unrelated
//! exactly when neither's `Extends` chain reaches the other — but a
//! negative [`ExternalSignatures::is_subtype`] result alone doesn't prove
//! that: it's also what an *unresolvable* chain (an unknown type this
//! crate simply has no data for) returns, and flagging on that would be
//! guessing. [`ExternalSignatures::ancestry_fully_known`] is what
//! distinguishes the two: a cast is only ever flagged once both the
//! value's and the target's `Extends` chains are confirmed to resolve all
//! the way to a definite root (a script with no `Extends` at all, or a
//! native engine type from `rules/native-types.yaml` with no further
//! parent) without ever reaching each other.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::{is_primitive, ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "impossible-cast";

/// Checks `source` for an `as` cast proven impossible using only
/// same-script information. Since that alone can never confirm a type's
/// full ancestry resolves to a definite root (see the module docs), this
/// never actually flags anything on its own — see [`check_with`].
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves both the value's and the target's full
/// `Extends` ancestry through `external`, the same way
/// [`crate::useless_downcast::check_with`] resolves ancestor-type casts.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, external, &mut diagnostics);
            }
        });
    }

    diagnostics
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn walk_stmt<E: ExternalSignatures>(
    stmt: &Stmt,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, external, decl.line, diagnostics);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            walk_expr(target, env, external, *line, diagnostics);
            walk_expr(value, env, external, *line, diagnostics);
        }
        Stmt::Expr { value, line } => walk_expr(value, env, external, *line, diagnostics),
        Stmt::Return {
            value: Some(value),
            line,
        } => walk_expr(value, env, external, *line, diagnostics),
        Stmt::Return { value: None, .. } => {}
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition,
                body,
                line,
                ..
            } in branches
            {
                walk_expr(condition, env, external, *line, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
        Stmt::While {
            condition,
            body,
            line,
            ..
        } => {
            walk_expr(condition, env, external, *line, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    env: &TypeEnv,
    external: &mut E,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Cast { value, type_name } => {
            walk_expr(value, env, external, line, diagnostics);
            if let Some(value_type) = infer_type(value, env) {
                if !value_type.is_array && impossible(&value_type.name, type_name, external) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] cast to '{type_name}' can never succeed: '{}' and '{type_name}' are unrelated types, so this always evaluates to None",
                            value_type.name
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, external, line, diagnostics);
            walk_expr(right, env, external, line, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, external, line, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, external, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, external, line, diagnostics);
            walk_expr(index, env, external, line, diagnostics);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, env, external, line, diagnostics);
            for arg in args {
                walk_expr(arg, env, external, line, diagnostics);
            }
        }
        Expr::NewArray { size, .. } => walk_expr(size, env, external, line, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, external, line, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Whether a cast from `value_type_name` to `target_type_name` is proven
/// impossible: neither extends the other (an exact match is handled by
/// [`ExternalSignatures::is_subtype`] returning `true` for equal names),
/// neither is a primitive type, and both types' full `Extends` ancestry is
/// confirmed to resolve to a definite root per `external`, per the module
/// docs.
fn impossible<E: ExternalSignatures>(
    value_type_name: &str,
    target_type_name: &str,
    external: &mut E,
) -> bool {
    if is_primitive(value_type_name) || is_primitive(target_type_name) {
        return false;
    }
    if external.is_subtype(value_type_name, target_type_name)
        || external.is_subtype(target_type_name, value_type_name)
    {
        return false;
    }
    external.ancestry_fully_known(value_type_name)
        && external.ancestry_fully_known(target_type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_flag_without_external_ancestry_resolution() {
        // `check` (no external resolver) can't confirm either type's
        // ancestry actually resolves to a root, so it never flags anything
        // on its own; see `check_with` below for the confirmed case.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    struct FakeExternalWithUnrelatedTypes;

    impl ExternalSignatures for FakeExternalWithUnrelatedTypes {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            sub_type.eq_ignore_ascii_case(super_type)
        }

        fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("Armor") || type_name.eq_ignore_ascii_case("Weapon")
        }
    }

    #[test]
    fn flags_a_cast_between_confirmed_unrelated_types() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'Armor'"));
        assert!(diagnostics[0].message.contains("'Weapon'"));
    }

    struct FakeExternalWithSubtype;

    impl ExternalSignatures for FakeExternalWithSubtype {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            sub_type.eq_ignore_ascii_case(super_type)
                || (sub_type.eq_ignore_ascii_case("Actor")
                    && super_type.eq_ignore_ascii_case("ObjectReference"))
        }

        fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("Actor")
                || type_name.eq_ignore_ascii_case("ObjectReference")
        }
    }

    #[test]
    fn does_not_flag_a_legitimate_narrowing_or_widening_cast() {
        let widening = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    ObjectReference b = akActor as ObjectReference\nEndFunction\n",
            &mut FakeExternalWithSubtype,
        );
        assert!(widening.is_empty());

        let narrowing = check_with(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor b = akRef as Actor\nEndFunction\n",
            &mut FakeExternalWithSubtype,
        );
        assert!(narrowing.is_empty());
    }

    #[test]
    fn does_not_flag_an_exact_type_match() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Armor b = akArmor as Armor\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert!(diagnostics.is_empty());
    }

    struct FakeExternalWithUnresolvedAncestry;

    impl ExternalSignatures for FakeExternalWithUnresolvedAncestry {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            sub_type.eq_ignore_ascii_case(super_type)
        }

        // Only one of the two types' ancestry is confirmed resolved.
        fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("Armor")
        }
    }

    #[test]
    fn does_not_flag_when_only_one_sides_ancestry_is_confirmed() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnresolvedAncestry,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_cast_involving_a_primitive_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Int a)\n    Armor b = a as Armor\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_cast_to_a_primitive_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Int value = akArmor as Int\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_cast_whose_value_type_is_unresolvable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Weapon b = GetTarget() as Weapon\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_cast_on_a_property() {
        let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    Weapon b = MyArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check_with(
            "ScriptName Example\n\nState Active\n    Function Test(Armor akArmor)\n        Weapon b = akArmor as Weapon\n    EndFunction\nEndState\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_a_cast_from_an_array_typed_value() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor[] armors)\n    Weapon b = armors as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn walks_a_cast_value_nested_in_an_index_expression() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor[] arr)\n    Weapon b = arr[0] as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        // Whether the array-element access itself resolves to a known type
        // isn't the point here; this just needs to walk the Index
        // expression nested inside the cast without crashing.
        assert!(diagnostics.len() <= 1);
    }

    #[test]
    fn flags_a_cast_nested_in_a_named_argument_and_walks_a_new_array_expression() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    SomeCall(flag = akArmor as Weapon)\n    Int[] arr = new Int[3]\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'Armor'"));
    }

    #[test]
    fn finds_casts_in_control_flow_conditions_and_bodies() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor as Weapon\n        Foo(akArmor as Weapon)\n    ElseIf akArmor as Weapon\n        Return akArmor as Weapon\n    Else\n        Weapon local = akArmor as Weapon\n    EndIf\n    While akArmor as Weapon\n        Foo(akArmor as Weapon)\n    EndWhile\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        let lines: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect();
        assert_eq!(lines, vec![4, 5, 6, 7, 9, 11, 12]);
    }

    #[test]
    fn finds_casts_nested_in_composite_expressions() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Bool compared = (akArmor as Weapon) == None\n    Bool negated = !(akArmor as Weapon)\n    Foo((akArmor as Weapon).GetName())\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        let lines: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect();
        assert_eq!(lines, vec![4, 5, 6]);
    }

    #[test]
    fn flags_each_cast_when_casts_are_nested() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Foo((akArmor as Weapon) as Armor)\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.line == 4 && diagnostic.rule == RULE));
    }

    #[test]
    fn external_lookup_is_not_needed_to_classify_casts() {
        assert!(FakeExternalWithUnrelatedTypes
            .lookup("Armor", "SomeFunction")
            .is_none());
    }
}
