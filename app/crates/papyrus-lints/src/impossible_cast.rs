//! Flags an explicit `as` cast that can never succeed: the value's known
//! type and the cast's target type are *proven* unrelated — neither
//! extends the other, directly or transitively — so the cast always
//! evaluates to `None` at runtime no matter what the value actually holds
//! (e.g. `Armor a` then `Weapon b = a as Weapon`, since `Armor` and
//! `Weapon` are unrelated siblings both directly extending `Form`).
//!
//! Like [`crate::useless_downcast`], this works from the parsed AST (via
//! [`papyrus_parser::types`]) to know a cast's value's declared type, and
//! only checks a cast whose value's type can be determined locally
//! (locals, parameters, properties, `Self`/`Parent`, literals, and other
//! resolvable expressions) — a member access or function call result is
//! left unflagged rather than guessed at. Primitive types (`Int`, `Float`,
//! `Bool`, `String`) are never flagged, since Papyrus's conversions between
//! those (and between a primitive and an object type) are a different
//! concern entirely from object-type subtyping.
//!
//! Papyrus scripts have single inheritance, so two types are unrelated
//! exactly when neither's `Extends` chain reaches the other — but a
//! negative [`ExternalSignatures::is_subtype`] result alone doesn't prove
//! that: it's also what an *unresolvable* chain (an unknown type this
//! crate simply has no data for) returns, and flagging on that would be
//! guessing. [`ExternalSignatures::ancestry_fully_known`] is what
//! distinguishes the two: a cast is only ever flagged once both the
//! value's and the target's `Extends` chains are confirmed to resolve all
//! the way to a definite root (a resolved script with no `Extends` at all)
//! without ever reaching each other.

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::is_primitive;
use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "impossible-cast";

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
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let Expr::Cast { value, type_name } = expr else {
            return;
        };
        let Some(value_type) = infer_type(value, env) else {
            return;
        };
        if value_type.is_array {
            return;
        }
        if impossible(&value_type.name, type_name, ctx.external) {
            self.store.emit(
                ctx.line,
                1,
                format!(
                    "[warning] cast to '{type_name}' can never succeed: '{}' and '{type_name}' are unrelated types, so this always evaluates to None",
                    value_type.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for an `as` cast proven impossible using only
/// same-script information. Since that alone can never confirm a type's
/// full ancestry resolves to a definite root (see the module docs), this
/// never actually flags anything on its own — see [`check_with`].
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

/// Like [`check`], but resolves both the value's and the target's full
/// `Extends` ancestry through `external`, the same way
/// [`crate::useless_downcast::check_with`] resolves ancestor-type casts.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures>(ast: Option<&Script>, external: &mut E) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

/// Whether a cast from `value_type_name` to `target_type_name` is proven
/// impossible: neither extends the other (an exact match is handled by
/// [`ExternalSignatures::is_subtype`] returning `true` for equal names),
/// neither is a primitive type, and both types' full `Extends` ancestry is
/// confirmed to resolve to a definite root per `external`, per the module
/// docs.
fn impossible<E: ExternalSignatures + ?Sized>(
    value_type_name: &str,
    target_type_name: &str,
    external: &mut E,
) -> bool {
    if is_primitive(value_type_name) || is_primitive(target_type_name) {
        return false;
    }
    if external.is_subtype(value_type_name, target_type_name)
        || external.is_subtype(target_type_name, value_type_name)
    {
        return false;
    }
    external.ancestry_fully_known(value_type_name)
        && external.ancestry_fully_known(target_type_name)
}

#[cfg(test)]
#[path = "impossible_cast_tests.rs"]
mod tests;
