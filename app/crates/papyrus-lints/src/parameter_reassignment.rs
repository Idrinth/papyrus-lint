//! Flags a function/event parameter that gets assigned a new value inside
//! its own body, since reusing the parameter's name for a different value
//! shadows what the caller passed in and can confuse a reader who expects
//! it to still reflect the original argument at any later point in the
//! function.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a parameter's own name apart from an unrelated local or
//! property with the same name; a script that doesn't parse cleanly is
//! left unchecked rather than guessed at.

use papyrus_parser::ast::{Expr, FunctionDecl, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "parameter-reassignment";

#[derive(Default)]
struct Collect {
    store: Store,
    protected: Vec<bool>,
    params: Vec<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.params = function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.params.clear();
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let Stmt::Assign { target, .. } = stmt else {
            return;
        };
        let Expr::Identifier(name) = target else {
            return;
        };
        if self.protected.get(ctx.line).copied().unwrap_or(false) {
            return;
        }
        let Some(param) = self
            .params
            .iter()
            .find(|param| param.eq_ignore_ascii_case(name))
        else {
            return;
        };

        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] Parameter '{param}' is reassigned inside its function; consider using a local variable instead"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a function/event parameter reassigned somewhere in
/// its own body. Flagged as a `[warning]`.
///
/// A reassignment inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// is never flagged: it's generated boilerplate the user can't edit.
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

#[cfg(test)]
#[path = "parameter_reassignment_tests.rs"]
mod tests;
