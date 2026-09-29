//! Flags functions/events whose control-flow nesting exceeds a configured
//! threshold.
//!
//! Depth starts at 0 in the function or event body and gains 1 for every
//! nested `If` / `ElseIf` branch or `While` loop. `Else` does not add a
//! level of its own: it is an alternative at the same depth as the `If`
//! it belongs to. A `State` wrapper is ignored, so a function inside a
//! state is measured the same way as one declared on the script. This
//! works from the parsed AST rather than raw tokens; a script that
//! doesn't parse cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{FunctionDecl, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "nesting-depth";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        let info = ctx.config.nesting_depth_info.max(1);
        let warning = ctx.config.nesting_depth_warning.max(info);
        let error = ctx.config.nesting_depth_error.max(warning);
        let depth = max_depth(&function.body, 0);
        let level = if depth >= error {
            "error"
        } else if depth >= warning {
            "warning"
        } else if depth >= info {
            "info"
        } else {
            return;
        };

        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[{}] {kind} '{}' has a nesting depth of {} (info: {}, warning: {}, error: {})",
                level, function.name, depth, info, warning, error
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for functions/events whose control-flow nesting depth
/// reaches `info`, `warning`, or `error`. A function below `info` is not
/// flagged. `warning` below `info` is treated as equal to `info`, and
/// `error` below `warning` is treated as equal to `warning`, so a
/// contradictory pair can only collapse a milder band, never silently
/// drop a finding.
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

fn max_depth(body: &[Stmt], depth: usize) -> usize {
    body.iter().map(|stmt| stmt_depth(stmt, depth)).max().unwrap_or(depth)
}

fn stmt_depth(stmt: &Stmt, depth: usize) -> usize {
    match stmt {
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            let nested = depth + 1;
            let branch_depth = branches
                .iter()
                .map(|branch| max_depth(&branch.body, nested))
                .max()
                .unwrap_or(nested);
            let else_depth = if else_body.is_empty() {
                nested
            } else {
                max_depth(else_body, nested)
            };
            branch_depth.max(else_depth)
        }
        Stmt::While { body, .. } => max_depth(body, depth + 1),
        Stmt::LockGuard {
            body, else_body, ..
        } => {
            // Not part of the documented metric; walk children so nested
            // If/While inside a guard still count, without treating the
            // guard itself as a nesting level.
            max_depth(body, depth).max(max_depth(else_body, depth))
        }
        _ => depth,
    }
}

#[cfg(test)]
#[path = "nesting_depth_tests.rs"]
mod tests;
