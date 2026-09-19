//! Flags, as an `[error]`, a call to `Utility.RandomInt` or
//! `Utility.RandomFloat` whose first two arguments both fold to
//! compile-time-constant numbers with the first not smaller than the
//! second, since both native functions require their first argument (the
//! minimum) to be smaller than their second (the maximum) — a call with the
//! bounds equal or reversed never produces any actual randomness.
//!
//! Like [`crate::division_by_zero`], this only folds an argument built
//! entirely from literals (optionally combined with arithmetic, comparison,
//! logical, and unary operators); an argument that depends on an identifier,
//! a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array
//! is left unflagged rather than guessed at. A named argument is matched by its
//! position in the call, not by its name, the same way every other
//! argument-inspecting lint in this crate does.

use papyrus_parser::ast::Expr;

use crate::const_eval::{as_number, eval_const};
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "invalid-random-range";

/// The native `Utility` singleton functions this lint checks, both only
/// ever called through that literal script name (see
/// `shared/rules/data/native-globals.yaml`), the same way [`crate::short_wait_interval`]
/// treats `Utility.Wait`.
const RANDOM_FUNCTIONS: &[&str] = &["RandomInt", "RandomFloat"];

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        let Some(name) = matching_function(callee) else {
            return;
        };
        let (Some(min_arg), Some(max_arg)) = (args.first(), args.get(1)) else {
            return;
        };
        let (Some(min_literal), Some(max_literal)) = (
            eval_const(unwrap_value(min_arg)),
            eval_const(unwrap_value(max_arg)),
        ) else {
            return;
        };
        let (Some((min, _)), Some((max, _))) = (as_number(&min_literal), as_number(&max_literal))
        else {
            return;
        };
        if min < max {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[error] Utility.{name}({min}, {max}): the first argument \
                 must be smaller than the second, or the call never \
                 produces any actual randomness"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every call to `Utility.RandomInt`/`Utility.RandomFloat` in
/// `source`, flagging one whose first two arguments both fold to constant
/// numbers with the first not smaller than the second.
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

/// Unwraps a call argument's underlying value, ignoring a `NamedArg` name.
fn unwrap_value(expr: &Expr) -> &Expr {
    match expr {
        Expr::NamedArg { value, .. } => value,
        other => other,
    }
}

/// Whether `callee` is a call to one of [`RANDOM_FUNCTIONS`], only matching
/// when explicitly qualified by the literal `Utility` script name, the same
/// way [`crate::short_wait_interval::matching_function`] treats
/// `Utility.Wait`.
fn matching_function(callee: &Expr) -> Option<&'static str> {
    let Expr::Member { object, property } = callee else {
        return None;
    };
    let name = *RANDOM_FUNCTIONS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(property))?;
    let Expr::Identifier(qualifier) = object.as_ref() else {
        return None;
    };
    if !qualifier.eq_ignore_ascii_case("Utility") {
        return None;
    }
    Some(name)
}

#[cfg(test)]
#[path = "invalid_random_range_tests.rs"]
mod tests;
