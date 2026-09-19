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
//!
//! A plain assignment (`outfit = OtherOutfit`) clears every tracked call in
//! the current statement list: it could reassign a local variable an
//! already-tracked receiver/argument expression reads, which a purely
//! structural AST comparison can't see through, so treating it the same as
//! any other "modification" would risk a false positive. A `Return`
//! statement stops scanning the rest of its own statement list outright,
//! since nothing after it can ever run. Receiver/argument expressions are
//! compared the way Papyrus itself would, matching identifiers and
//! member/property names case-insensitively.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "repeated-setoutfit";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        let mut applied = Vec::new();
        let mut diagnostics = Vec::new();
        check_body(&function.body, &mut applied, &mut diagnostics);
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event body in `source` for a `SetOutfit` call
/// repeating an earlier call's exact receiver and arguments with nothing in
/// between guaranteed to have changed the outfit.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
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
            // A plain assignment could reassign a local variable read by an
            // already-tracked receiver/outfit expression (e.g. `outfit =
            // OtherOutfit` between two `akActor.SetOutfit(outfit)` calls),
            // which a purely structural AST comparison can't see through;
            // clear every tracked call rather than risk comparing two calls
            // that no longer actually pass the same value.
            Stmt::Assign { .. } => applied.clear(),
            // Nothing after a Return in the same block can ever run, so
            // stop scanning this body rather than comparing dead code
            // against calls that already ran.
            Stmt::Return { .. } => break,
            Stmt::VarDecl(_) => {}
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
    if let Some(existing) = applied
        .iter_mut()
        .find(|entry| expr_eq(entry.receiver, receiver))
    {
        if args_eq(existing.args, args) {
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

/// Compares two expressions the way Papyrus itself would: identifiers and
/// member/property names are matched case-insensitively (Papyrus is a
/// case-insensitive language), recursing through a member-access chain;
/// anything else falls back to plain structural equality.
fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Identifier(x), Expr::Identifier(y)) => x.eq_ignore_ascii_case(y),
        (
            Expr::Member {
                object: oa,
                property: pa,
            },
            Expr::Member {
                object: ob,
                property: pb,
            },
        ) => pa.eq_ignore_ascii_case(pb) && expr_eq(oa, ob),
        _ => a == b,
    }
}

/// Compares two `SetOutfit(...)` call argument lists element-wise via
/// [`expr_eq`].
fn args_eq(a: &[Expr], b: &[Expr]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| expr_eq(x, y))
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
#[path = "repeated_setoutfit_tests.rs"]
mod tests;
