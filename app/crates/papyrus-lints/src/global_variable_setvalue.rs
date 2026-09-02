//! Flags a `SetValue`/`SetValueInt` call on a `GlobalVariable`-like receiver
//! (`gv.SetValue(2.0)`) that writes a value the surrounding `If`/`ElseIf`/
//! `Else` chain never actually proves is different from the value already
//! there, since that write is at best redundant and at worst a sign the
//! author meant to branch on something else:
//!
//! ```papyrus
//! If gv.GetValue() == 1.0
//!     gv.SetValue(2.0)
//! Else
//!     gv.SetValue(0.0)
//! EndIf
//! ```
//!
//! Two shapes are flagged, both scoped to receivers this chain already
//! reads via `<receiver>.GetValue()`/`GetValueInt() == <literal>` somewhere
//! in one of its conditions, so an unrelated `SetValue` call is left alone:
//!
//! - A branch whose own condition is exactly that equality check, and whose
//!   body then calls `SetValue`/`SetValueInt` on the same receiver with the
//!   very same literal the condition just confirmed is already current —
//!   definitely a no-op write.
//! - The trailing `Else` of such a chain calling `SetValue`/`SetValueInt` on
//!   a receiver the chain reads elsewhere, since an `Else` has no condition
//!   of its own to rule out the value it's about to write already being
//!   current (the fix is usually to turn it into an explicit
//!   `ElseIf receiver.GetValue() != literal` branch instead).
//!
//! Only a call standing alone as its own statement (not nested inside a
//! further `If`/`While` in the branch, which would be its own guard) is
//! considered; only an equality (`==`) condition against a literal
//! establishes a value, and only a literal argument to `SetValue`/
//! `SetValueInt` is compared against it — anything less direct is left
//! unflagged rather than guessed at. Disabled by default: a project opts in
//! via `rules.global_variable_setvalue`.

use std::collections::HashSet;

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "global-variable-setvalue";

/// Checks every `If`/`ElseIf`/`Else` chain in `source` for a `SetValue`/
/// `SetValueInt` write that doesn't provably change the value.
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
            } => check_if(branches, else_body, diagnostics),
            Stmt::While { body, .. } => check_body(body, diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// A `<receiver>.GetValue()`/`GetValueInt() == <literal>` established by a
/// branch's own condition.
struct ValueRead {
    key: String,
    value: f64,
}

/// A `<receiver>.SetValue()`/`SetValueInt(<literal>)` call standing alone as
/// its own statement.
struct ValueWrite<'a> {
    key: String,
    display: String,
    method: &'a str,
    literal: &'a Literal,
    value: f64,
    line: usize,
}

fn check_if(branches: &[IfBranch], else_body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    let reads: Vec<Option<ValueRead>> = branches
        .iter()
        .map(|branch| value_read(&branch.condition))
        .collect();
    let chain_keys: HashSet<&str> = reads
        .iter()
        .flatten()
        .map(|read| read.key.as_str())
        .collect();

    for (branch, read) in branches.iter().zip(reads.iter()) {
        if let Some(read) = read {
            for write in find_setvalue_calls(&branch.body) {
                if write.key == read.key && write.value == read.value {
                    diagnostics.push(Diagnostic {
                        line: write.line,
                        column: 1,
                        message: format!(
                            "[warning] {}.{}({}) does not change the value: this branch's \
                             condition already established the current value is {}",
                            write.display,
                            write.method,
                            literal_display(write.literal),
                            literal_display(write.literal)
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        check_body(&branch.body, diagnostics);
    }

    for write in find_setvalue_calls(else_body) {
        if chain_keys.contains(write.key.as_str()) {
            diagnostics.push(Diagnostic {
                line: write.line,
                column: 1,
                message: format!(
                    "[warning] {}.{}({}) may be an unnecessary write: this Else branch doesn't \
                     check whether the value already differs from {} before writing it; \
                     consider an ElseIf {}.GetValue() != {} branch instead",
                    write.display,
                    write.method,
                    literal_display(write.literal),
                    literal_display(write.literal),
                    write.display,
                    literal_display(write.literal)
                ),
                rule: RULE,
            });
        }
    }
    check_body(else_body, diagnostics);
}

/// If `condition` is exactly `<receiver>.GetValue()`/`GetValueInt() ==
/// <literal>` (in either operand order), returns the receiver's key (see
/// [`receiver_key`]) and the literal's numeric value.
fn value_read(condition: &Expr) -> Option<ValueRead> {
    let Expr::Binary {
        left,
        op: BinaryOp::Eq,
        right,
    } = condition
    else {
        return None;
    };
    read_from_operands(left, right).or_else(|| read_from_operands(right, left))
}

fn read_from_operands(call_side: &Expr, literal_side: &Expr) -> Option<ValueRead> {
    let Expr::Call { callee, args, .. } = call_side else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return None;
    };
    if !is_value_getter(property) {
        return None;
    }
    let Expr::Literal(literal) = literal_side else {
        return None;
    };
    Some(ValueRead {
        key: receiver_key(object)?,
        value: literal_to_f64(literal)?,
    })
}

/// Finds every `<receiver>.SetValue()`/`SetValueInt(<literal>)` call
/// standing alone as its own statement directly in `body` (not nested
/// inside a further `If`/`While`, which would be its own guard).
fn find_setvalue_calls(body: &[Stmt]) -> Vec<ValueWrite<'_>> {
    body.iter()
        .filter_map(|stmt| {
            let Stmt::Expr { value, line } = stmt else {
                return None;
            };
            let Expr::Call { callee, args, .. } = value else {
                return None;
            };
            if args.len() != 1 {
                return None;
            }
            let Expr::Member { object, property } = callee.as_ref() else {
                return None;
            };
            if !is_value_setter(property) {
                return None;
            }
            let Expr::Literal(literal) = &args[0] else {
                return None;
            };
            Some(ValueWrite {
                key: receiver_key(object)?,
                display: receiver_display(object)?,
                method: property.as_str(),
                literal,
                value: literal_to_f64(literal)?,
                line: *line,
            })
        })
        .collect()
}

fn is_value_getter(name: &str) -> bool {
    name.eq_ignore_ascii_case("GetValue") || name.eq_ignore_ascii_case("GetValueInt")
}

fn is_value_setter(name: &str) -> bool {
    name.eq_ignore_ascii_case("SetValue") || name.eq_ignore_ascii_case("SetValueInt")
}

fn literal_to_f64(literal: &Literal) -> Option<f64> {
    match literal {
        Literal::Int { value, .. } => Some(*value as f64),
        Literal::Float(value) => Some(*value),
        Literal::String(_) | Literal::Bool(_) | Literal::None => None,
    }
}

fn literal_display(literal: &Literal) -> String {
    match literal {
        Literal::Int { value, .. } => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::String(_) | Literal::Bool(_) | Literal::None => String::new(),
    }
}

/// A canonical, case-insensitive key for a "simple" receiver expression
/// (an identifier, `Self`, or a chain of member accesses built from those),
/// used to tell whether a `SetValue` call's receiver is the same one a
/// condition elsewhere in the chain read from. Anything less direct (a call,
/// an index, a cast, ...) returns `None` rather than being guessed at.
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
    fn flags_unguarded_else_write_on_a_receiver_the_chain_reads() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("gv.SetValue(0)"));
        assert!(diagnostics[0].message.contains("ElseIf gv.GetValue() != 0"));
    }

    #[test]
    fn flags_a_branch_that_writes_back_the_value_its_own_condition_confirmed() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 2.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert!(diagnostics[0].message.contains("does not change the value"));
    }

    #[test]
    fn does_not_flag_a_branch_that_writes_a_different_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_the_corrected_pattern_using_a_saved_local_and_explicit_elseif() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    Float current = gv.GetValue()\n    If current == 1.0\n        gv.SetValue(2.0)\n    ElseIf current != 0.0\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_else_write_on_an_unrelated_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        other.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_setvalue_call_guarded_by_its_own_nested_if() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        If gv.GetValue() != 0.0\n            gv.SetValue(0.0)\n        EndIf\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_mixed_int_and_float_forms_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.getvalueint() == 1\n        gv.setvalue(1.0)\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_condition_depending_on_more_than_a_bare_equality() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Bool flag)\n    If gv.GetValue() == 1.0 && flag\n        gv.SetValue(1.0)\n    Else\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv)\n        If gv.GetValue() == 2.0\n            gv.SetValue(2.0)\n        EndIf\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
