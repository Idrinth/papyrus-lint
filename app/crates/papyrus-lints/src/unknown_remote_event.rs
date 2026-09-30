//! Flags a remote-event registration whose string name is not an event
//! on the source type.
//!
//! `RegisterForRemoteEvent(actor, "OnLood")` compiles: the event name is a
//! string, and a misspelled one never fires. This checks the literal
//! against events declared on the source object's type and its `Extends`
//! chain, via [`ExternalSignatures::has_event`].
//!
//! Scoped to Fallout 4 / Starfield via the rule's `games` field.

use papyrus_parser::ast::{Expr, FunctionDecl, Literal, Script};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unknown-remote-event";

const REGISTER: &str = "RegisterForRemoteEvent";
const UNREGISTER: &str = "UnregisterForRemoteEvent";

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
        let Expr::Call {
            callee,
            args,
            line,
            col,
        } = expr
        else {
            return;
        };
        let Some(function) = remote_event_function(callee) else {
            return;
        };
        let (source, event_expr) = source_and_event(args);
        let Some(event_name) = event_expr.and_then(string_literal) else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let Some(source) = source else {
            return;
        };
        let Some(source_type) = infer_type(source, env) else {
            return;
        };
        if source_type.is_array {
            return;
        }
        if !matches!(
            ctx.external.has_event(&source_type.name, &event_name),
            Some(false)
        ) {
            return;
        }
        self.store.emit(
            *line,
            *col,
            format!(
                "[warning] '{event_name}' is not an event on {}, so {function} will not subscribe to it",
                source_type.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for remote-event registration strings that name no
/// event on the source object's type.
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

fn remote_event_function(callee: &Expr) -> Option<&'static str> {
    let name = match callee {
        Expr::Identifier(name) => name.as_str(),
        Expr::Member { property, .. } => property.as_str(),
        _ => return None,
    };
    if name.eq_ignore_ascii_case(REGISTER) {
        Some(REGISTER)
    } else if name.eq_ignore_ascii_case(UNREGISTER) {
        Some(UNREGISTER)
    } else {
        None
    }
}

/// Source object and event-name argument.
///
/// Positional form is `(akEventSource, asEventName)`. Named forms accept
/// the Creation Kit names and the shorter names already used by
/// `unregistered-remote-event`.
fn source_and_event(args: &[Expr]) -> (Option<&Expr>, Option<&Expr>) {
    let mut source = None;
    let mut event = None;
    let mut positional = Vec::new();
    for arg in args {
        if let Expr::NamedArg { name, value } = arg {
            if name.eq_ignore_ascii_case("asEventName") || name.eq_ignore_ascii_case("eventName") {
                event = Some(value.as_ref());
            } else if name.eq_ignore_ascii_case("akEventSource")
                || name.eq_ignore_ascii_case("akSelf")
            {
                source = Some(value.as_ref());
            }
        } else {
            positional.push(arg);
        }
    }
    if event.is_none() {
        event = if source.is_some() {
            positional.first().copied()
        } else {
            positional.get(1).copied()
        };
    }
    if source.is_none() {
        source = positional.first().copied();
    }
    (source, event)
}

fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "unknown_remote_event_tests.rs"]
mod tests;
