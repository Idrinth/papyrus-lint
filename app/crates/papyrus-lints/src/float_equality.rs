//! Flags a direct `==`/`!=` comparison between two `Float` values.
//!
//! Floating-point rounding error can make two values that are
//! conceptually "the same" (e.g. one computed via a different, but
//! mathematically equivalent, sequence of operations) compare unequal at
//! runtime, or vice versa. Comparing against a small epsilon (or an
//! inequality) is usually more robust than a direct `==`/`!=`.
//!
//! Like [`crate::numeric_comparison`], this works on the parsed AST (see
//! `papyrus_parser::types`) rather than raw tokens, so a script that fails
//! to parse simply isn't checked. Disabled by default: see
//! `crate::config::Rules::float_equality`.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, Script, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "float-equality";

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.enter_function(function);
        }
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Binary { left, op, right } = expr else {
            return;
        };
        if !is_equality(*op) {
            return;
        }
        let Some(env) = self.env.as_ref() else {
            return;
        };
        check_comparison(left, right, env, ctx.line, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Float`/`Float` `==`/`!=` comparisons.
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

fn is_equality(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// Flags `left op right` when both sides are `Float`. Flagged as an
/// `[info]`.
fn check_comparison(left: &Expr, right: &Expr, env: &TypeEnv, line: usize, store: &mut Store) {
    let Some(left_ty) = infer_type(left, env) else {
        return;
    };
    let Some(right_ty) = infer_type(right, env) else {
        return;
    };

    if is_float(&left_ty) && is_float(&right_ty) {
        store.emit(
            line,
            1,
            "[info] Comparing two Float values with '==' or '!=' directly; \
                      floating-point rounding error can make this comparison unreliable",
            RULE,
        );
    }
}

#[cfg(test)]
#[path = "float_equality_tests.rs"]
mod tests;
