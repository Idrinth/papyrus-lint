//! Flags implicit comparisons between different numeric types: an `Int`
//! value compared against a `Float` value (`==`, `!=`, `<`, `<=`, `>`, `>=`)
//! without an explicit cast making the comparison exact.
//!
//! Papyrus implicitly widens the `Int` side to `Float` for such a
//! comparison, which can produce surprising results (particularly for
//! `==`/`!=`) once floating-point precision is involved.
//!
//! Like the other type-aware lints in this crate, this one works on the
//! parsed AST (see `papyrus_parser::types`) rather than raw tokens. Scripts
//! that fail to parse simply aren't checked.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, Script, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "numeric-comparison";

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
        if !is_comparison(*op) {
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

/// Checks `source` for `Int`/`Float` comparisons that aren't already made
/// exact by an explicit cast.
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

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gt
            | BinaryOp::Lt
            | BinaryOp::GtEq
            | BinaryOp::LtEq
    )
}

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// Flags `left op right` when one side is `Int` and the other `Float`.
/// Flagged as a `[warning]`.
///
/// A side's inferred type already reflects any explicit cast it carries
/// (`someFloat as Int` infers as `Int`), so comparing the plain inferred
/// types is enough to let explicit casts through.
fn check_comparison(left: &Expr, right: &Expr, env: &TypeEnv, line: usize, store: &mut Store) {
    let Some(left_ty) = infer_type(left, env) else {
        return;
    };
    let Some(right_ty) = infer_type(right, env) else {
        return;
    };

    let mismatched =
        (is_int(&left_ty) && is_float(&right_ty)) || (is_float(&left_ty) && is_int(&right_ty));
    if mismatched {
        store.emit(
            line,
            1,
            "[warning] Comparison between Int and Float without an explicit cast; \
                      floating-point precision may make this inexact",
            RULE,
        );
    }
}

#[cfg(test)]
#[path = "numeric_comparison_tests.rs"]
mod tests;
