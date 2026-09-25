//! Flags functions/events whose cyclomatic complexity exceeds a configured
//! threshold.
//!
//! Complexity starts at 1 for the function itself and gains 1 for every
//! extra path through it: each `If`/`ElseIf` branch, each `While` loop, and
//! each short-circuiting `&&`/`||` operator (which itself introduces a
//! branch, since the right-hand side may or may not be evaluated). This
//! works from the parsed AST rather than raw tokens, since it needs the
//! block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "cyclomatic-complexity";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        let warning = ctx.config.cyclomatic_complexity_warning;
        let error = ctx.config.cyclomatic_complexity_error.max(warning);
        let complexity = complexity_of(function);
        let level = if complexity > error {
            "error"
        } else if complexity > warning {
            "warning"
        } else {
            return;
        };

        self.store.emit(
            ctx.line,
            1,
            format!(
                "[{}] Function '{}' has a cyclomatic complexity of {} (warning: {}, error: {})",
                level, function.name, complexity, warning, error
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for functions/events whose cyclomatic complexity exceeds
/// `warning` or `error`. A function at or below `warning` is not flagged; one
/// above `warning` but at or below `error` is flagged as `[warning]`; one
/// above `error` is flagged as `[error]`. `error` below `warning` (an
/// otherwise contradictory pair — an `[error]` kicking in before the
/// `[warning]` it's supposed to escalate) is treated as equal to `warning`
/// instead, so the misconfiguration can only ever make more functions read
/// `[error]` (by collapsing the `[warning]` band down to nothing), never
/// silently drop or downgrade a finding that a sane pair would have reported.
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

fn complexity_of(function: &FunctionDecl) -> usize {
    1 + function.body.iter().map(stmt_complexity).sum::<usize>()
}

fn stmt_complexity(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.value.as_ref().map_or(0, expr_complexity),
        Stmt::Assign { target, value, .. } => expr_complexity(target) + expr_complexity(value),
        Stmt::Expr { value, .. } => expr_complexity(value),
        Stmt::Return { value, .. } => value.as_ref().map_or(0, expr_complexity),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            branches
                .iter()
                .map(|branch| {
                    1 + expr_complexity(&branch.condition) + body_complexity(&branch.body)
                })
                .sum::<usize>()
                + body_complexity(else_body)
        }
        Stmt::While {
            condition, body, ..
        } => 1 + expr_complexity(condition) + body_complexity(body),
        Stmt::LockGuard {
            kind, body, else_body, ..
        } => {
            let decision = matches!(kind, papyrus_parser::ast::LockKind::Try) as usize;
            decision + body_complexity(body) + body_complexity(else_body)
        }
    }
}

fn body_complexity(body: &[Stmt]) -> usize {
    body.iter().map(stmt_complexity).sum()
}

fn expr_complexity(expr: &Expr) -> usize {
    match expr {
        Expr::Binary { left, op, right } => {
            let branch = matches!(op, BinaryOp::And | BinaryOp::Or) as usize;
            branch + expr_complexity(left) + expr_complexity(right)
        }
        Expr::Unary { operand, .. } => expr_complexity(operand),
        Expr::Call { callee, args, .. } => {
            expr_complexity(callee) + args.iter().map(expr_complexity).sum::<usize>()
        }
        Expr::Member { object, .. } => expr_complexity(object),
        Expr::Index { object, index } => expr_complexity(object) + expr_complexity(index),
        Expr::Cast { value, .. } | Expr::Is { value, .. } => expr_complexity(value),
        Expr::NewArray { size, .. } => expr_complexity(size),
        Expr::NamedArg { value, .. } => expr_complexity(value),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => 0,
        Expr::NewStruct { .. } => 0,
    }
}

#[cfg(test)]
#[path = "cyclomatic_complexity_tests.rs"]
mod tests;
