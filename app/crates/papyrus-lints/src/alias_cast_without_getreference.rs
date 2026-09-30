//! Flags an explicit `as` cast that treats a Quest alias as if it were
//! the filled reference (`(MyAlias as Actor).Kill()`). Aliases are not
//! the filled `Actor` / `ObjectReference` / `Location`; the filled form
//! is reached with `GetReference()`, `GetActorReference()`, or
//! `GetLocation()`.
//!
//! Casting an alias to another alias type (`Alias as ReferenceAlias`,
//! `ReferenceAlias as MySpecialAlias`) is allowed — that is a real
//! subtype relationship between alias scripts, not a substitute for
//! `GetReference()`.

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::is_primitive;
use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "alias-cast-without-getreference";

const KNOWN_ALIAS_TYPES: &[&str] = &[
    "Alias",
    "ReferenceAlias",
    "LocationAlias",
    "RefCollectionAlias",
];

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
        if is_alias_type_name(type_name) {
            return;
        }
        if is_primitive(type_name) {
            return;
        }
        let Some(value_type) = infer_type(value, env) else {
            return;
        };
        if value_type.is_array {
            return;
        }
        if !is_alias_value(&value_type.name, ctx.external) {
            return;
        }
        let suggestion = suggested_api(&value_type.name, type_name);
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] do not cast alias type '{}' to '{type_name}': the alias is not the filled reference; use {suggestion} instead",
                value_type.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for an alias-to-non-alias `as` cast.
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

fn is_alias_type_name(name: &str) -> bool {
    KNOWN_ALIAS_TYPES
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
        || name.to_ascii_lowercase().ends_with("alias")
}

fn is_alias_value(type_name: &str, external: &mut dyn ExternalSignatures) -> bool {
    if is_alias_type_name(type_name) {
        return true;
    }
    KNOWN_ALIAS_TYPES
        .iter()
        .any(|known| external.is_subtype(type_name, known))
}

fn suggested_api(value_type: &str, target: &str) -> &'static str {
    if value_type.eq_ignore_ascii_case("LocationAlias")
        || target.eq_ignore_ascii_case("Location")
    {
        return "GetLocation()";
    }
    if target.eq_ignore_ascii_case("Actor") {
        return "GetActorReference()";
    }
    "GetReference() / GetActorReference()"
}

#[cfg(test)]
#[path = "alias_cast_without_getreference_tests.rs"]
mod tests;
