//! Flags `If`/`ElseIf`/`While` conditions that aren't already boolean,
//! instead of relying on Papyrus's implicit conversion to `Bool`.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "strict-boolean";

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

    fn visit_if_branch(&mut self, branch: &IfBranch, ctx: &mut VisitCtx<'_>) {
        let Some(env) = self.env.as_ref() else {
            return;
        };
        check_condition(
            &branch.condition,
            branch.line,
            branch.col,
            env,
            ctx.config.bool_like_int,
            &mut self.store,
        );
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let Stmt::While {
            condition,
            line,
            col,
            ..
        } = stmt
        else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        check_condition(
            condition,
            *line,
            *col,
            env,
            ctx.config.bool_like_int,
            &mut self.store,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every `If`/`ElseIf`/`While` condition in `source` and flags the
/// ones that don't resolve to `Bool`.
///
/// A condition whose type can't be determined locally (a function call or a
/// member access on another script, for instance) is left unflagged rather
/// than risk a false positive. When `allow_bool_like_int` is `true` (see
/// [`crate::config::Config::bool_like_int`]), a condition that's exactly
/// the `Int` literal `1` or `0` is also left unflagged, since that's a
/// common "bool-like" idiom; any other `Int` value is still flagged.
/// Flagged as a `[warning]`.
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

/// Whether `expr` is exactly the `Int` literal `1` or `0`, the "bool-like"
/// idiom [`check`] allows past when `allow_bool_like_int` is set.
fn is_bool_like_int(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Int { value: 0 | 1, .. }))
}

fn check_condition(
    condition: &Expr,
    line: usize,
    column: usize,
    env: &TypeEnv,
    allow_bool_like_int: bool,
    store: &mut Store,
) {
    let Some(type_name) = infer_type(condition, env) else {
        return;
    };

    if !type_name.is_array && type_name.name.eq_ignore_ascii_case("bool") {
        return;
    }

    if allow_bool_like_int && is_bool_like_int(condition) {
        return;
    }

    let found = if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name
    };

    store.emit(
        line,
        column,
        format!(
            "[warning] Condition must be a boolean value or expression, found '{found}'"
        ),
        RULE,
    );
}

#[cfg(test)]
#[path = "strict_boolean_tests.rs"]
mod tests;
