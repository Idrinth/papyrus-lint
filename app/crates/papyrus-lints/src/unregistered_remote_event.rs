//! Flags a remote `Event Type.OnFoo(...)` handler with no matching
//! `RegisterForRemoteEvent` in this script or any parent it `Extends`.
//!
//! Fallout 4 and Starfield remote handlers never fire unless something
//! registers them. A handler with no registration in this script or its
//! ancestry is dead code. Registration is matched case-insensitively by
//! event-name leaf (`"OnCellAttach"` covers `Event ObjectReference.OnCellAttach`).
//! Only unqualified `RegisterForRemoteEvent(...)` and `self.RegisterForRemoteEvent(...)`
//! count for this script; a call on another receiver registers that other script.
//!
//! Per-script registrations are indexed on the project function table (see
//! [`remote_event_registrations`]) so ancestry checks are a cheap
//! `Extends` walk via [`ExternalSignatures::registers_remote_event`].
//!
//! Scoped to Fallout 4 / Starfield via the rule's `games` field.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Literal, Script};
use papyrus_parser::visit::Visitor;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unregistered-remote-event";

const REGISTER: &str = "RegisterForRemoteEvent";

/// Literal `RegisterForRemoteEvent` event-name leaves collected from a
/// script, plus whether any call used a non-literal event name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteEventRegistrations {
    /// Lowercased event-name leaves from string-literal arguments.
    pub events: HashSet<String>,
    /// `true` when at least one `RegisterForRemoteEvent` call's event name
    /// could not be read statically, so every remote handler is treated as
    /// registered.
    pub opaque: bool,
}

/// Collects [`RemoteEventRegistrations`] from `script`. Used when indexing
/// a script into the project function table and by this lint's visitor.
pub fn remote_event_registrations(script: &Script) -> RemoteEventRegistrations {
    let mut collector = RegistrationCollector::default();
    collector.visit_script(script);
    collector.into_registrations()
}

#[derive(Default)]
struct RegistrationCollector {
    events: HashSet<String>,
    opaque: bool,
}

impl RegistrationCollector {
    fn into_registrations(self) -> RemoteEventRegistrations {
        RemoteEventRegistrations {
            events: self.events,
            opaque: self.opaque,
        }
    }

    fn note_call(&mut self, args: &[Expr]) {
        match registered_event_name(args) {
            Some(name) => {
                self.events.insert(name.to_ascii_lowercase());
            }
            None => self.opaque = true,
        }
    }
}

impl Visitor for RegistrationCollector {
    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::Call { callee, args, .. } = expr {
            if is_register_for_remote_event(callee) {
                self.note_call(args);
            }
        }
        papyrus_parser::visit::walk_expr(self, expr);
    }
}

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
    script_name: Option<String>,
    extends: Option<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.script_name = Some(script.name.clone());
        self.extends = script.extends.clone();
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

    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        if self.opaque_registration {
            return;
        }
        for handler in &self.handlers {
            let registered_here = self
                .registered_events
                .iter()
                .any(|event| event.eq_ignore_ascii_case(&handler.event));
            if registered_here {
                continue;
            }
            let ancestry = self
                .script_name
                .as_deref()
                .and_then(|name| ctx.external.registers_remote_event(name, &handler.event));
            match ancestry {
                Some(true) => continue,
                Some(false) => {}
                // Incomplete ancestry or no project resolver. Same-script
                // already missed; if this script Extends someone, registration
                // may live on a parent we cannot see — stay quiet.
                None if self.extends.is_some() => continue,
                None => {}
            }
            self.store.emit(
                handler.line,
                1,
                format!(
                    "[warning] Remote event handler '{}' is never registered with \
                     RegisterForRemoteEvent in this script or any parent it Extends, \
                     so the engine will not call it",
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

/// Checks `source` for remote event handlers with no
/// `RegisterForRemoteEvent` in this script or its ancestry.
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

/// Whether `callee` is a bare `RegisterForRemoteEvent(...)` call, or one
/// explicitly qualified with `self.RegisterForRemoteEvent(...)`. A call on
/// any other receiver registers that other script, not this one.
fn is_register_for_remote_event(callee: &Expr) -> bool {
    match callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case(REGISTER),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case(REGISTER)
        }
        _ => false,
    }
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
