//! Flags an unresolved parent script, declared type, or call through
//! Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `Utility.Wait(1.0)` or
//! `MyMissingScript.DoThing()`) whose target script can't be located,
//! since Papyrus resolves that name against a script file at compile time
//! and a call through a script that doesn't exist can never compile.
//!
//! Only a call whose object is a bare identifier not already known as a
//! local variable, parameter, or property (i.e. definitely not an
//! instance the script already has a handle to) is treated as a script
//! reference at all — anything resolvable locally is left to the
//! "Argument type check"/"Return type check" lints instead. Whether such a
//! name, an `Extends` parent, or a type annotation can be located depends
//! on the project's own scripts and built-in native type data,
//! neither of which this crate has access to on its own; a caller that can
//! resolve them (e.g. the desktop app's `FunctionTable`) does so by
//! implementing [`ExternalSignatures::script_exists`] and
//! [`ExternalSignatures::type_exists`] and calling
//! [`check_with`] instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unresolved-script";

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

/// Checks `source` for calls through a script name that can't be
/// resolved. Since this crate has no filesystem access on its own, no
/// script can ever be confirmed missing this way; see [`check_with`] to
/// actually resolve script names.
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

/// Like [`check`], but resolves each call's target script through
/// `external`, flagging one that can't be located.
pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    if let Some(parent) = &script.extends {
        if !external.type_exists(parent) {
            diagnostics.push(missing_type(script.line, 1, parent, "Parent script"));
        }
    }

    for property in &script.properties {
        check_type(
            &property.type_name,
            property.line,
            external,
            &mut diagnostics,
        );
        if let Some(value) = &property.value {
            walk_expr(value, property.line, &env, external, &mut diagnostics);
        }
    }
    for variable in &script.variables {
        check_type(
            &variable.type_name,
            variable.line,
            external,
            &mut diagnostics,
        );
        if let Some(value) = &variable.value {
            walk_expr(value, variable.line, &env, external, &mut diagnostics);
        }
    }

    for function in all_functions(script) {
        if let Some(return_type) = &function.return_type {
            check_type(return_type, function.line, external, &mut diagnostics);
        }
        for param in &function.params {
            check_type(&param.type_name, function.line, external, &mut diagnostics);
        }
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
            check_type(&decl.type_name, decl.line, external, diagnostics);
            if let Some(value) = &decl.value {
                walk_expr(value, decl.line, env, external, diagnostics);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            walk_expr(target, *line, env, external, diagnostics);
            walk_expr(value, *line, env, external, diagnostics);
        }
        Stmt::Expr { value, line } => walk_expr(value, *line, env, external, diagnostics),
        Stmt::Return { value, line } => {
            if let Some(value) = value {
                walk_expr(value, *line, env, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition,
                body,
                line,
                ..
            } in branches
            {
                walk_expr(condition, *line, env, external, diagnostics);
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
            walk_expr(condition, stmt_line(stmt), env, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures + ?Sized>(
    expr: &Expr,
    line: usize,
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
            if let Expr::Member { object, .. } = &**callee {
                if let Expr::Identifier(name) = &**object {
                    if env.lookup(name).is_none() && !external.script_exists(name) {
                        diagnostics.push(missing(*line, *col, name));
                    }
                }
            }
            walk_expr(callee, *line, env, external, diagnostics);
            for arg in args {
                walk_expr(arg, *line, env, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, line, env, external, diagnostics);
            walk_expr(right, line, env, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, line, env, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, line, env, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, line, env, external, diagnostics);
            walk_expr(index, line, env, external, diagnostics);
        }
        Expr::Cast { value, type_name } => {
            check_type_name(type_name, line, external, diagnostics);
            walk_expr(value, line, env, external, diagnostics);
        }
        Expr::NewArray { type_name, size } => {
            check_type(type_name, line, external, diagnostics);
            walk_expr(size, line, env, external, diagnostics);
        }
        Expr::NamedArg { value, .. } => walk_expr(value, line, env, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.line,
        Stmt::Assign { line, .. }
        | Stmt::Expr { line, .. }
        | Stmt::Return { line, .. }
        | Stmt::If { line, .. }
        | Stmt::While { line, .. } => *line,
    }
}

fn check_type<E: ExternalSignatures + ?Sized>(
    type_name: &TypeName,
    line: usize,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_type_name(&type_name.name, line, external, diagnostics);
}

fn check_type_name<E: ExternalSignatures + ?Sized>(
    type_name: &str,
    line: usize,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !external.type_exists(type_name) {
        diagnostics.push(missing_type(line, 1, type_name, "Type"));
    }
}

fn missing_type(line: usize, col: usize, name: &str, kind: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] {kind} '{name}' could not be located"),
        rule: RULE,
    }
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] Script '{name}' could not be located"),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "unresolved_script_tests.rs"]
mod tests;
