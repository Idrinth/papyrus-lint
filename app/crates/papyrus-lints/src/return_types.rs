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
//! value there is a script author error of a different kind). A `Return`
//! whose value's type can't be determined from the script alone is still
//! checked when the value is a call whose return type can be resolved
//! locally or through [`ExternalSignatures::function_return_type`]; any
//! other unresolved value is skipped rather than guessed at, to keep
//! false positives rare.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, Literal, Script, Stmt, TypeName};
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
    locals: LocalReturns,
    return_type: Option<TypeName>,
    function_name: String,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
        self.locals = LocalReturns::from_script(script);
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
        if matches!(value, Expr::Literal(Literal::None)) {
            if !argument_types::accepts_none(return_type) {
                diagnostics.push(mismatch(*line, &self.function_name, return_type, "None"));
            }
        } else if let Some(value_type) =
            infer_returned_type(value, env, &self.locals, ctx.external)
        {
            let form_as_bool = ctx.config.treat_form_as_bool_for_returns
                && !return_type.is_array
                && return_type.name.eq_ignore_ascii_case("bool")
                && !value_type.is_array
                && !argument_types::is_primitive(&value_type.name);
            if !form_as_bool
                && !argument_types::is_compatible(return_type, &value_type, ctx.external)
            {
                diagnostics.push(mismatch(
                    *line,
                    &self.function_name,
                    return_type,
                    &argument_types::format_type(&value_type),
                ));
            }
        }
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
pub fn check_with<E: ExternalSignatures>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

/// Return types of the functions declared in the script being linted,
/// keyed by lowercased name. A name declared more than once (e.g.
/// overridden in a state) with differing return types is stored as
/// missing, since which declaration applies at a given call site can't be
/// determined here.
#[derive(Default)]
struct LocalReturns {
    by_name: HashMap<String, Option<Option<TypeName>>>,
}

impl LocalReturns {
    fn from_script(script: &Script) -> Self {
        let mut grouped: HashMap<String, Vec<Option<TypeName>>> = HashMap::new();
        for function in all_functions(script) {
            grouped
                .entry(function.name.to_ascii_lowercase())
                .or_default()
                .push(function.return_type.clone());
        }

        let by_name = grouped
            .into_iter()
            .map(|(name, types)| {
                let first = types[0].clone();
                let consistent = types.iter().all(|return_type| return_type == &first);
                (name, consistent.then_some(first))
            })
            .collect();

        LocalReturns { by_name }
    }

    fn lookup(&self, name: &str) -> Option<&TypeName> {
        self.by_name
            .get(&name.to_ascii_lowercase())?
            .as_ref()?
            .as_ref()
    }
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

fn infer_returned_type<E: ExternalSignatures + ?Sized>(
    value: &Expr,
    env: &TypeEnv,
    locals: &LocalReturns,
    external: &mut E,
) -> Option<TypeName> {
    if let Some(value_type) = infer_type(value, env) {
        return Some(value_type);
    }
    match value {
        Expr::Call { callee, .. } => resolve_call_return_type(callee, env, locals, external),
        Expr::NamedArg { value, .. } => infer_returned_type(value, env, locals, external),
        _ => None,
    }
}

fn resolve_call_return_type<E: ExternalSignatures + ?Sized>(
    callee: &Expr,
    env: &TypeEnv,
    locals: &LocalReturns,
    external: &mut E,
) -> Option<TypeName> {
    let (object_type, function_name) = match callee {
        Expr::Identifier(name) => {
            if let Some(return_type) = locals.lookup(name) {
                return Some(return_type.clone());
            }
            (infer_type(&Expr::Self_, env)?, name.as_str())
        }
        Expr::Member { object, property } => {
            if matches!(**object, Expr::Self_) {
                if let Some(return_type) = locals.lookup(property) {
                    return Some(return_type.clone());
                }
            }
            (infer_type(object, env)?, property.as_str())
        }
        _ => return None,
    };
    if object_type.is_array {
        return None;
    }
    external.function_return_type(&object_type.name, function_name)
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
