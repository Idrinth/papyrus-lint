//! Flags a `While` loop whose condition depends on one or more local
//! variables/parameters that are never assigned (plainly or via a compound
//! `+=`/`-=`/etc.) anywhere within the loop's own body. A local variable or
//! parameter can only ever change through a direct assignment inside the
//! very function that owns it — nothing else (not a call, not another
//! script, not the engine) can reach in and modify it — so when nothing in
//! the loop body ever assigns any identifier the condition depends on, that
//! condition can never change once the loop starts: it either never runs at
//! all (if it starts out false) or never stops (if it starts out true).
//!
//! Only a condition built entirely from identifiers, literals, and
//! arithmetic/comparison/logical/unary operators is checked, the same
//! restriction [`crate::static_condition`] and [`crate::division_by_zero`]
//! place on the expressions they fold: one that reaches a call, a
//! member/index access, `Self`/`Parent`, a cast, or a `new` array is left
//! unflagged rather than guessed at, since evaluating it may depend on
//! state this lint has no way to see change (e.g. `a.IsDead()`, which can
//! start returning something different purely because of what happens
//! inside that call). Likewise, an identifier that isn't a known local
//! variable or parameter of the enclosing function — most notably a script
//! `Property`, which another function (or the engine) is free to change at
//! any time — disqualifies the whole condition from being checked, rather
//! than being assumed safe. This deliberately narrow scope means a loop
//! whose exit condition is reassigned to the very same value on every
//! iteration (e.g. re-fetching a reference that just happens to keep coming
//! back dead) is not caught here, since nothing short of running the script
//! could prove that.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "invariant-loop-condition";

/// Checks every `While` loop in `source` for a condition that can never
/// change across iterations, per the module documentation above.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut known: HashSet<String> = function
            .params
            .iter()
            .map(|param| param.name.to_lowercase())
            .collect();
        walk_body(&function.body, &mut known, &mut diagnostics);
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

/// Walks `body` in program order, growing `known` with every local variable
/// declared along the way (Papyrus has no block scoping, so a variable
/// declared inside a branch or loop is visible to code lexically after it
/// too), and checking each `While` loop's condition once its own known set
/// is up to date.
fn walk_body(body: &[Stmt], known: &mut HashSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                known.insert(decl.name.to_lowercase());
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    walk_body(&branch.body, known, diagnostics);
                }
                walk_body(else_body, known, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                col,
            } => {
                check_condition(condition, known, body, *line, *col, diagnostics);
                walk_body(body, known, diagnostics);
            }
            Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn check_condition(
    condition: &Expr,
    known: &HashSet<String>,
    body: &[Stmt],
    line: usize,
    column: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(names) = referenced_identifiers(condition) else {
        return;
    };
    if names.is_empty() || !names.iter().all(|name| known.contains(name)) {
        return;
    }
    if assigns_any(body, &names) {
        return;
    }

    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    let quoted: Vec<String> = sorted.iter().map(|name| format!("'{name}'")).collect();
    let (noun, verb) = if quoted.len() == 1 {
        ("Local variable", "is")
    } else {
        ("Local variables", "are")
    };
    diagnostics.push(Diagnostic {
        line,
        column,
        message: format!(
            "[warning] {noun} {} {verb} never assigned anywhere in this loop's body, so its condition can never change once the loop starts: it will either never run or never stop",
            quoted.join(", ")
        ),
        rule: RULE,
    });
}

/// Attempts to collect every identifier `expr` references, returning `None`
/// as soon as any part of it depends on something whose value this lint
/// can't be sure only changes through a direct assignment (a call, a
/// member/index access, `Self`/`Parent`, a cast, or a `new` array).
fn referenced_identifiers(expr: &Expr) -> Option<HashSet<String>> {
    match expr {
        Expr::Literal(_) => Some(HashSet::new()),
        Expr::Identifier(name) => Some(HashSet::from([name.to_lowercase()])),
        Expr::Unary { operand, .. } => referenced_identifiers(operand),
        Expr::Binary { left, right, .. } => {
            let mut names = referenced_identifiers(left)?;
            names.extend(referenced_identifiers(right)?);
            Some(names)
        }
        Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::Member { .. }
        | Expr::Index { .. }
        | Expr::Cast { .. }
        | Expr::NewArray { .. }
        | Expr::NamedArg { .. } => None,
    }
}

/// Whether any statement in `body` (recursing into every nested `If`/`While`
/// body, since even an assignment that doesn't always run still means the
/// condition *can* change) assigns one of `names` as a plain identifier
/// target — a compound assignment (`+=`, ...) counts too, since it still
/// writes a new value.
fn assigns_any(body: &[Stmt], names: &HashSet<String>) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Assign {
            target: Expr::Identifier(name),
            ..
        } => names.contains(&name.to_lowercase()),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            branches
                .iter()
                .any(|branch| assigns_any(&branch.body, names))
                || assigns_any(else_body, names)
        }
        Stmt::While { body, .. } => assigns_any(body, names),
        Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_loop_whose_counter_is_never_incremented() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 5\n    While n < 10\n        Debug.Trace(\"y\")\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("'n'"));
    }

    #[test]
    fn does_not_flag_a_loop_that_increments_its_counter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 0\n    While n < 10\n        n += 1\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_loop_that_plainly_reassigns_its_variable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 0\n    While n < 10\n        n = n + 1\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_the_loop_priming_idiom_from_issue_378() {
        // https://github.com/Idrinth/papyrus-lint/issues/378: `a` is
        // reassigned every iteration and the condition calls `a.IsDead()`,
        // so whether this actually terminates depends on what
        // Game.GetPlayer() returns at runtime — not something this lint
        // can prove either way, so it stays quiet rather than guess.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    Int c = 0\n    While a == None || a.IsDead()\n        a = Game.GetPlayer()\n        c += 1\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_condition_depending_on_a_property() {
        let diagnostics = check(
            "ScriptName Example\n\nBool Property Flag Auto\n\nFunction Test()\n    While Flag\n        Debug.Trace(\"waiting\")\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_condition_built_entirely_from_literals() {
        // Already covered by static-condition; this lint only fires when
        // the condition depends on at least one identifier.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_condition_reaching_a_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    While GetValue() > 0\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_parameter_never_reassigned_in_the_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int count)\n    While count > 0\n        Debug.Trace(\"spin\")\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'count'"));
    }

    #[test]
    fn does_not_flag_when_assignment_only_happens_inside_a_nested_if() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int n = 0\n    While n < 10\n        If flag\n            n = 5\n        EndIf\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_every_identifier_when_the_condition_uses_more_than_one() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 0\n    Int b = 0\n    While a < 10 && b < 10\n        Debug.Trace(\"spin\")\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'a'"));
        assert!(diagnostics[0].message.contains("'b'"));
        assert!(diagnostics[0].message.contains("Local variables"));
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int n = 5\n        While n < 10\n        EndWhile\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_variable_declared_inside_an_earlier_if_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Int n = 5\n        While n < 10\n            Debug.Trace(\"y\")\n        EndWhile\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
