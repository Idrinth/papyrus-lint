//! Flags a second `<receiver>.SetOutfit(<outfit>, ...)` call passing the
//! exact same argument(s) as an earlier call on the same receiver, with
//! nothing between them guaranteed to have changed what that receiver is
//! wearing, since Bethesda's engine is known to mishandle a redundant
//! repeated `SetOutfit` call — modders have reported it leaving the actor
//! with no visible equipment (or reverted to their default outfit) until
//! something re-equips them.
//!
//! Like [`crate::repeated_getvalue`]/[`crate::setvalue_in_loop`], a call's
//! receiver can't generally be resolved back to an `Actor`/`ActorBase`-typed
//! script, so this matches by the `SetOutfit` method name alone
//! (case-insensitively) rather than requiring the receiver's declared type.
//! The whole argument list is compared (not just the outfit itself), so a
//! call passing a different `abSleepOutfit` flag than the earlier one (e.g.
//! `akActor.SetOutfit(outfit, true)` after `akActor.SetOutfit(outfit)`) is
//! never flagged, since those two calls set different outfit slots.
//!
//! This only tracks calls appearing as direct statements within the same
//! straight-line statement list (a function/event body, or a single
//! `If`/`ElseIf`/`Else`/`While` body within it): entering a nested body
//! starts fresh from a copy of the outer state (so a call inside it can
//! still be compared against calls that already ran unconditionally before
//! it), but nothing learned inside a nested body — including a call that
//! sets a *different* outfit, which would otherwise count as the
//! "modification" that clears an earlier call from tracking — carries back
//! out to the statements that follow it, since neither an `If`'s branch nor
//! a `While`'s body is guaranteed to have run by the time execution reaches
//! there.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "repeated-setoutfit";

/// Checks every function/event body in `source` for a `SetOutfit` call
/// repeating an earlier call's exact receiver and arguments with nothing in
/// between guaranteed to have changed the outfit.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut applied = Vec::new();
        check_body(&function.body, &mut applied, &mut diagnostics);
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

/// One `<receiver>.SetOutfit(...)` call already seen earlier in the current
/// straight-line statement list, not yet superseded by a later call setting
/// a different outfit on the same receiver.
#[derive(Clone, Copy)]
struct AppliedOutfit<'a> {
    receiver: &'a Expr,
    args: &'a [Expr],
}

fn check_body<'a>(
    body: &'a [Stmt],
    applied: &mut Vec<AppliedOutfit<'a>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::Expr { value, line } => {
                if let Some((receiver, args)) = set_outfit_call(value) {
                    check_set_outfit_call(receiver, args, *line, applied, diagnostics);
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, &mut applied.clone(), diagnostics);
                }
                check_body(else_body, &mut applied.clone(), diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, &mut applied.clone(), diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Compares a single `SetOutfit` call's `receiver`/`args` against every
/// entry already tracked in `applied`: an exact match (same receiver, same
/// arguments) is flagged as a redundant repeat, a same-receiver call with
/// different arguments updates that entry instead (the outfit actually
/// changed), and a call on a not-yet-seen receiver is simply added.
fn check_set_outfit_call<'a>(
    receiver: &'a Expr,
    args: &'a [Expr],
    line: usize,
    applied: &mut Vec<AppliedOutfit<'a>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(existing) = applied.iter_mut().find(|entry| entry.receiver == receiver) {
        if existing.args == args {
            diagnostics.push(Diagnostic {
                line,
                column: 1,
                message: "[warning] This SetOutfit(...) call repeats the exact same outfit \
                          already applied earlier with nothing in between guaranteed to have \
                          changed it; calling SetOutfit twice in a row with the same outfit is \
                          redundant and has been reported to cause equipment/visibility issues"
                    .to_string(),
                rule: RULE,
            });
        } else {
            existing.args = args;
        }
    } else {
        applied.push(AppliedOutfit { receiver, args });
    }
}

/// Matches a `<receiver>.SetOutfit(<args>)` call, returning its receiver and
/// argument list. `SetOutfit` always takes at least one argument (the
/// `Outfit`), so a call with no arguments at all can't be this native
/// function and is never matched.
fn set_outfit_call(expr: &Expr) -> Option<(&Expr, &[Expr])> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    if args.is_empty() {
        return None;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return None;
    };
    if !property.eq_ignore_ascii_case("SetOutfit") {
        return None;
    }
    Some((object.as_ref(), args.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_the_same_outfit_applied_twice_in_a_row() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn flags_a_repeat_with_unrelated_statements_between() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    Debug.Trace(\"hi\")\n    Int a = 1\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn does_not_flag_a_single_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_different_outfits() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB)\n    akActor.SetOutfit(OutfitA)\n    akActor.SetOutfit(OutfitB)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_the_same_outfit_after_a_different_outfit_was_applied_between() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB)\n    akActor.SetOutfit(OutfitA)\n    akActor.SetOutfit(OutfitB)\n    akActor.SetOutfit(OutfitA)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_different_receivers() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActorA, Actor akActorB, Outfit MyOutfit)\n    akActorA.SetOutfit(MyOutfit)\n    akActorB.SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_different_sleep_outfit_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit, true)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unqualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Outfit MyOutfit)\n    SetOutfit(MyOutfit)\n    SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_different_method_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfitDefault(MyOutfit)\n    akActor.SetOutfitDefault(MyOutfit)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_the_method_name_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.setoutfit(MyOutfit)\n    akActor.SETOUTFIT(MyOutfit)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_repeat_nested_inside_an_if_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    akActor.SetOutfit(MyOutfit)\n    If bReady\n        akActor.SetOutfit(MyOutfit)\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_carry_a_conditional_change_past_the_if_statement() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB, Bool bReady)\n    akActor.SetOutfit(OutfitA)\n    If bReady\n        akActor.SetOutfit(OutfitB)\n    EndIf\n    akActor.SetOutfit(OutfitA)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 8);
    }

    #[test]
    fn checks_each_if_branch_independently_of_its_siblings() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    If bReady\n        akActor.SetOutfit(MyOutfit)\n    Else\n        akActor.SetOutfit(MyOutfit)\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_repeat_nested_inside_a_while_loop() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    akActor.SetOutfit(MyOutfit)\n    While bReady\n        akActor.SetOutfit(MyOutfit)\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test(Actor akActor, Outfit MyOutfit)\n        akActor.SetOutfit(MyOutfit)\n        akActor.SetOutfit(MyOutfit)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn resolves_self_and_chained_member_receivers() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Outfit MyOutfit)\n    Self.SetOutfit(MyOutfit)\n    Self.SetOutfit(MyOutfit)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
