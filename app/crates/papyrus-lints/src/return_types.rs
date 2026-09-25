//! Flags `Return` statements whose value doesn't match the enclosing
//! function's declared return type.
//!
//! Like [`crate::argument_types`], this works from the parsed AST (via
//! `papyrus_parser::types`) since it needs to know declared types, and
//! reuses that module's [`external_signatures::ExternalSignatures`] trait so a
//! caller that can resolve other scripts' `Extends` chains (e.g. the
//! desktop app's `FunctionTable`) lets a returned value whose type is a
//! *subtype* of the declared return type pass, the same way argument
//! type-checking accepts a child-type argument for a parameter typed as
//! one of its ancestors.
//!
//! A function with no declared return type isn't checked (`Return` with a
//! value there is a script author error of a different kind), and a
//! `Return` whose value's type can't be determined from the script alone
//! is skipped rather than guessed at, to keep false positives rare.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types;
use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "return-types";

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
    return_type: Option<TypeName>,
    function_name: String,
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
        self.return_type = function.return_type.clone();
        self.function_name = function.name.clone();
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
        self.return_type = None;
        self.function_name.clear();
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let Some(return_type) = self.return_type.as_ref() else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let Stmt::Return {
            value: Some(value),
            line,
        } = stmt
        else {
            return;
        };
        let mut diagnostics = Vec::new();
        check_return(
            *line,
            value,
            return_type,
            &self.function_name,
            env,
            ctx.external,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Return` values whose type doesn't match (or isn't a
/// subtype of) the enclosing function's declared return type. Subtype
/// relationships to scripts outside `source` are never resolved this way;
/// see [`check_with`] for that.
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

/// Like [`check`], but resolves object-type return values through
/// `external` so a value whose script extends (directly or transitively)
/// the declared return type is accepted.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
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
        let Some(return_type) = function.return_type.clone() else {
            continue;
        };
        env.with_function_scope(function, |scoped| {
            check_body(
                &function.body,
                scoped,
                &return_type,
                &function.name,
                external,
                &mut diagnostics,
            );
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

fn check_body<E: ExternalSignatures + ?Sized>(
    body: &[Stmt],
    env: &TypeEnv,
    return_type: &TypeName,
    function_name: &str,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                check_return(
                    *line,
                    value,
                    return_type,
                    function_name,
                    env,
                    external,
                    diagnostics,
                );
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, env, return_type, function_name, external, diagnostics);
                }
                check_body(
                    else_body,
                    env,
                    return_type,
                    function_name,
                    external,
                    diagnostics,
                );
            }
            Stmt::While { body, .. } => {
                check_body(body, env, return_type, function_name, external, diagnostics);
            }
            Stmt::LockGuard { body, else_body, .. } => {
                check_body(body, env, return_type, function_name, external, diagnostics);
                check_body(
                    else_body,
                    env,
                    return_type,
                    function_name,
                    external,
                    diagnostics,
                );
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } => {}
        }
    }
}

fn check_return<E: ExternalSignatures + ?Sized>(
    line: usize,
    value: &Expr,
    return_type: &TypeName,
    function_name: &str,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(value, Expr::Literal(Literal::None)) {
        if !argument_types::accepts_none(return_type) {
            diagnostics.push(mismatch(line, function_name, return_type, "None"));
        }
        return;
    }

    let Some(value_type) = infer_type(value, env) else {
        return;
    };
    if !argument_types::is_compatible(return_type, &value_type, external) {
        diagnostics.push(mismatch(
            line,
            function_name,
            return_type,
            &argument_types::format_type(&value_type),
        ));
    }
}

fn mismatch(line: usize, function_name: &str, return_type: &TypeName, got: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: 1,
        message: format!(
            "[error] Function '{}' declares return type {} but returns {}",
            function_name,
            argument_types::format_type(return_type),
            got
        ),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "return_types_tests.rs"]
mod tests;
