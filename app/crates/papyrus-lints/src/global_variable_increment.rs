//! Flags a `SetValue` call on a `GlobalVariable`-like receiver whose sole
//! argument adds something to that very same receiver's own `GetValue()`
//! (in either operand order):
//!
//! ```papyrus
//! gv.SetValue(gv.GetValue() + x)
//! gv.SetValue(x + gv.GetValue())
//! ```
//!
//! `GlobalVariable` exposes that exact operation as a single native call:
//! `Mod(afValue)` adds `afValue` to the global's current value (and returns
//! the new value), so either call above does the same thing as `gv.Mod(x)`
//! with one native call and one addition instead of two native calls and an
//! addition.
//!
//! Like the "Slow function usage" lint (`slow_functions`), a call's receiver
//! can't generally be resolved back to a `GlobalVariable`-typed script (the
//! lexer/parser have no type/symbol resolution), so this matches
//! structurally instead: a `SetValue` call standing alone as its own
//! statement, taking exactly one argument that's a top-level `+` between
//! `<receiver>.GetValue()` and some other expression, where `<receiver>` is
//! the exact same receiver (see [`receiver_key`]) the `SetValue` call itself
//! targets. Anything less direct — a different operator, `SetValueInt`, a
//! `GetValue()` nested deeper than the addition's own two operands, a
//! receiver that isn't a simple identifier/`Self`/member chain, ... — is
//! left unflagged rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "global-variable-increment";

/// Checks every `SetValue` call in `source` for the `SetValue(GetValue() +
/// x)`/`SetValue(x + GetValue())` pattern described above.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_body(&function.body, &mut diagnostics);
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

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            Stmt::Expr { value, line } => check_setvalue_call(value, *line, diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn check_setvalue_call(expr: &Expr, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    let Expr::Call { callee, args, .. } = expr else {
        return;
    };
    if args.len() != 1 {
        return;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return;
    };
    if !property.eq_ignore_ascii_case("SetValue") {
        return;
    }
    let Some(receiver) = receiver_key(object) else {
        return;
    };
    let Expr::Binary {
        left,
        op: BinaryOp::Add,
        right,
    } = &args[0]
    else {
        return;
    };
    if !is_getvalue_on(left, &receiver) && !is_getvalue_on(right, &receiver) {
        return;
    }
    let Some(display) = receiver_display(object) else {
        return;
    };
    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[info] {display}.SetValue({display}.GetValue() + x) is slower than necessary; use \
             `{display}.Mod(x)` instead, which adds x to the current value in one native call"
        ),
        rule: RULE,
    });
}

/// Whether `expr` is exactly `<receiver>.GetValue()` (no arguments) on the
/// receiver identified by `receiver` (see [`receiver_key`]).
fn is_getvalue_on(expr: &Expr, receiver: &str) -> bool {
    let Expr::Call { callee, args, .. } = expr else {
        return false;
    };
    if !args.is_empty() {
        return false;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return false;
    };
    property.eq_ignore_ascii_case("GetValue") && receiver_key(object).as_deref() == Some(receiver)
}

/// A canonical, case-insensitive key for a "simple" receiver expression (an
/// identifier, `Self`, or a chain of member accesses built from those), used
/// to tell whether a `SetValue` call's receiver is the same one its
/// argument's `GetValue()` call reads from. Anything less direct (a call, an
/// index, a cast, ...) returns `None` rather than being guessed at.
fn receiver_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            receiver_key(object)?,
            property.to_lowercase()
        )),
        _ => None,
    }
}

/// Like [`receiver_key`], but preserves the receiver's original casing for
/// use in a diagnostic message instead of comparison.
fn receiver_display(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.clone()),
        Expr::Self_ => Some("Self".to_string()),
        Expr::Member { object, property } => {
            Some(format!("{}.{}", receiver_display(object)?, property))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_getvalue_plus_other_operand() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].column, 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[info]"));
        assert!(diagnostics[0].message.contains("gv.Mod(x)"));
    }

    #[test]
    fn flags_other_operand_plus_getvalue() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(x + gv.GetValue())\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.setvalue(gv.getvalue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_setvalueint() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int x)\n    gv.SetValueInt(gv.GetValueInt() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_different_operator() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() - x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_getvalue_call_on_a_different_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    gv.SetValue(other.GetValue() + 1.0)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_plain_setvalue_with_no_addition() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(1.0)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_addition_of_two_unrelated_values() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float a, Float b)\n    gv.SetValue(a + b)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn resolves_self_and_chained_member_receivers() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.SetValue(Self.GetValue() + x)\n    quest.MyGlobal.SetValue(quest.MyGlobal.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("Self.Mod")));
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("quest.MyGlobal.Mod")));
    }

    #[test]
    fn does_not_flag_a_receiver_that_is_neither_an_identifier_self_nor_member_chain() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs, Int i, Float x)\n    gvs[i].SetValue(gvs[i].GetValue() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv, Float x)\n        gv.SetValue(gv.GetValue() + x)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_nested_if_while_and_else_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.SetValue(gv.GetValue() + x)\n        Else\n            gv.SetValue(x + gv.GetValue())\n        EndIf\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_a_call_with_more_than_one_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x, x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unqualified_setvalue_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float x)\n    SetValue(GetValue() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn receiver_key_and_display_reject_expressions_that_are_not_a_simple_chain() {
        let not_a_receiver = Expr::Literal(papyrus_parser::ast::Literal::int(1));
        assert_eq!(receiver_key(&not_a_receiver), None);
        assert_eq!(receiver_display(&not_a_receiver), None);
    }
}
