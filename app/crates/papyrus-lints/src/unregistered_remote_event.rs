//! Flags a remote `Event Type.OnFoo(...)` handler with no matching
//! `RegisterForRemoteEvent` in the same script.
//!
//! Fallout 4 and Starfield remote handlers never fire unless something
//! registers them. A handler with no registration in this script is dead
//! code. Registration on a parent or a sibling script is a known false
//! positive; this rule starts with the same script only.
//!
//! Scoped to Fallout 4 / Starfield via the rule's `games` field.

use papyrus_parser::ast::{Expr, FunctionDecl, Literal};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unregistered-remote-event";

const REGISTER: &str = "RegisterForRemoteEvent";

struct RemoteHandler {
    line: usize,
    name: String,
    event: String,
}

#[derive(Default)]
struct Collect {
    store: Store,
    handlers: Vec<RemoteHandler>,
    /// Event-name leaves taken from a `RegisterForRemoteEvent` string
    /// argument (`"OnCellAttach"`). Empty means the call's event could
    /// not be read statically, so every remote handler is treated as
    /// registered.
    registered_events: Vec<String>,
    opaque_registration: bool,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if !function.is_event {
            return;
        }
        let Some((type_name, event)) = split_remote_event(&function.name) else {
            return;
        };
        self.handlers.push(RemoteHandler {
            line: function.line,
            name: format!("{type_name}.{event}"),
            event: event.to_string(),
        });
    }

    fn visit_expr(&mut self, expr: &Expr, _ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        if !is_register_for_remote_event(callee) {
            return;
        }
        match registered_event_name(args) {
            Some(name) => self.registered_events.push(name),
            None => self.opaque_registration = true,
        }
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        if self.opaque_registration {
            return;
        }
        for handler in &self.handlers {
            let registered = self
                .registered_events
                .iter()
                .any(|event| event.eq_ignore_ascii_case(&handler.event));
            if registered {
                continue;
            }
            self.store.emit(
                handler.line,
                1,
                format!(
                    "[warning] Remote event handler '{}' is never registered with \
                     RegisterForRemoteEvent in this script, so the engine will not \
                     call it",
                    handler.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for remote event handlers with no same-script
/// `RegisterForRemoteEvent`.
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

fn split_remote_event(name: &str) -> Option<(&str, &str)> {
    let (type_name, event) = name.rsplit_once('.')?;
    if type_name.is_empty() || event.is_empty() {
        return None;
    }
    Some((type_name, event))
}

fn is_register_for_remote_event(callee: &Expr) -> bool {
    let name = match callee {
        Expr::Identifier(name) => name.as_str(),
        Expr::Member { property, .. } => property.as_str(),
        _ => return false,
    };
    name.eq_ignore_ascii_case(REGISTER)
}

fn registered_event_name(args: &[Expr]) -> Option<String> {
    for arg in args {
        if let Expr::NamedArg { name, value } = arg {
            if name.eq_ignore_ascii_case("eventName") || name.eq_ignore_ascii_case("asEventName") {
                return string_literal(value);
            }
            continue;
        }
    }
    // RegisterForRemoteEvent(Form akSelf, ScriptEventName asEventName)
    args.get(1).and_then(string_literal)
}

fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "unregistered_remote_event_tests.rs"]
mod tests;
