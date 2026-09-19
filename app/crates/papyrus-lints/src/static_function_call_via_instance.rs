//! Flags a call reaching a script's `Global` function through an actual
//! object reference (a local variable, parameter, property, cast, or array
//! element — e.g. `akOtherActor.MyGlobalHelper()`) rather than Papyrus's
//! static/global call syntax (`ScriptName.MyGlobalHelper()`). Papyrus
//! allows this — a `Global` function ignores whatever reference it's
//! called through — but it can read as a mistake: a reader expecting an
//! instance method (or the author themselves, if the call was copy-pasted
//! from one) may not realize the reference is never actually used.
//!
//! The mirror image of [`crate::non_global_function_call`], which instead
//! flags a *non*-`Global` function reached through the static syntax.
//! `Self`/`Parent` are deliberately left unflagged: a script calling its
//! own `Global` function through `Self` for symmetry with its other
//! `Self.Whatever()` calls is a reasonable, common style rather than a
//! likely mistake. Whether a resolved function is declared `Global`
//! depends on the project's own scripts, which this crate has no
//! filesystem access to on its own; a caller that can resolve it (e.g. the
//! desktop app's `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and calling [`check_with`]
//! instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "static-function-call-via-instance";

#[derive(Default)]
struct Collect {
    store: crate::visitor::Store,
}

impl crate::visitor::AstLint for Collect {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn visit_script(
        &mut self,
        script: &papyrus_parser::ast::Script,
        ctx: &mut crate::visitor::VisitCtx<'_>,
    ) {
        self.store.extend(lint_issues(
            ctx.source,
            Some(script),
            ctx.tokens,
            ctx.config,
            ctx.external,
        ));
    }

    fn finish(&mut self, ctx: &mut crate::visitor::VisitCtx<'_>) {
        if ctx.ast.is_none() {
            self.store.extend(lint_issues(
                ctx.source,
                None,
                ctx.tokens,
                ctx.config,
                ctx.external,
            ));
        }
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for calls reaching a `Global` function through an
/// object reference. Since this crate has no filesystem access on its own,
/// no such call can ever be confirmed this way; see [`check_with`] to
/// actually resolve function signatures.
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

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    check_with(ast, external)
}

/// Like [`check`], but resolves each call's target function through
/// `external`, flagging one that resolves and is declared `Global`.
pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    for function in all_functions(script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, external, &mut diagnostics);
            }
        });
    }

    diagnostics
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn walk_stmt<E: ExternalSignatures + ?Sized>(
    stmt: &Stmt,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, env, external, diagnostics);
            walk_expr(value, env, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, env, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, env, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, env, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, env, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures + ?Sized>(
    expr: &Expr,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if let Expr::Member { object, property } = &**callee {
                if !matches!(**object, Expr::Self_ | Expr::Parent) {
                    if let Some(object_type) = infer_type(object, env) {
                        if !object_type.is_array
                            && external.is_global_function(&object_type.name, property)
                                == Some(true)
                        {
                            diagnostics.push(called_via_instance(
                                *line,
                                *col,
                                &object_type.name,
                                property,
                            ));
                        }
                    }
                }
            }
            walk_expr(callee, env, external, diagnostics);
            for arg in args {
                walk_expr(arg, env, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, external, diagnostics);
            walk_expr(right, env, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, external, diagnostics);
            walk_expr(index, env, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, external, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn called_via_instance(
    line: usize,
    col: usize,
    type_name: &str,
    function_name: &str,
) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[warning] '{function_name}' is declared Global on '{type_name}', so it doesn't need an object reference — calling it as '{type_name}.{function_name}()' avoids the possibly-confusing implication that the object matters, but calling it through an instance still works"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "static_function_call_via_instance_tests.rs"]
mod tests;
