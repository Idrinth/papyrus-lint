//! Flags, as a `[warning]`, an `Event` declaration whose name matches one of
//! the engine's own native events (per `shared/rules/data/skyrim/known-events.yaml`, which also
//! notes the Form that first declares each one) but whose parameter list
//! doesn't match the signature the engine actually calls it with.
//!
//! Papyrus never validates an `Event`'s signature against what the engine
//! calls it with — a mismatched declaration still compiles fine, but the
//! engine then never invokes it (or invokes it with arguments the script
//! doesn't expect), so the script silently never receives that event.
//!
//! Event/signature pairs are compiled into the `KNOWN_EVENTS` array below by
//! `build.rs` at build time, so this never parses YAML at runtime. Unlike
//! most of the other lints in this crate, this works from the parsed AST
//! rather than raw tokens, since it needs each declared `Event`'s parameter
//! list; a script that doesn't parse cleanly is left unchecked rather than
//! guessed at.
//!
//! Matches an `Event` by name alone (case-insensitively), regardless of
//! which Form the enclosing script actually `Extends`, the same way
//! [`crate::native_function_usage`] matches by (script, function) name
//! alone. Disabled by default: `shared/rules/data/skyrim/known-events.yaml` only lists a
//! curated subset of the engine's native events, and a script that declares
//! an `Event` sharing one of those names without actually extending the
//! listed Form (e.g. its own unrelated event handler that happens to reuse
//! a common name) would otherwise be misreported here.

use papyrus_parser::ast::{FunctionDecl, Param, TypeName};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// One parameter of a [`KnownEventRule`]'s expected signature.
pub struct EventArg {
    pub type_name: &'static str,
    pub name: &'static str,
}

/// One `shared/rules/data/skyrim/known-events.yaml` entry: an event's name, the Form that
/// first declares it, and its exact expected parameter list.
pub struct KnownEventRule {
    pub event: &'static str,
    pub form: &'static str,
    pub args: &'static [EventArg],
}

include!(concat!(env!("OUT_DIR"), "/known_events_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "event-signature-mismatch";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if !function.is_event {
            return;
        }
        let Some(rule) = find_rule(&function.name) else {
            return;
        };
        if signature_matches(&function.params, rule.args) {
            return;
        }
        self.store.emit(
            function.line,
            1,
            format!(
                "[warning] Event {}({}) does not match the signature {}({}) declared on {}; the engine will not invoke it correctly",
                function.name,
                describe_params(&function.params),
                rule.event,
                describe_args(rule.args),
                rule.form
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Event` declarations whose name matches a known
/// native event but whose parameter list doesn't match its signature.
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

/// Looks up a known event's signature by name, case-insensitively (Papyrus
/// identifiers are case-insensitive).
fn find_rule(name: &str) -> Option<&'static KnownEventRule> {
    KNOWN_EVENTS
        .iter()
        .find(|rule| rule.event.eq_ignore_ascii_case(name))
}

/// Whether `params` matches `expected` by count and, position by position,
/// by type name (case-insensitively); parameter names and default values
/// aren't compared, since the engine calls an event by position alone.
fn signature_matches(params: &[Param], expected: &[EventArg]) -> bool {
    params.len() == expected.len()
        && params.iter().zip(expected).all(|(param, arg)| {
            !param.type_name.is_array && param.type_name.name.eq_ignore_ascii_case(arg.type_name)
        })
}

/// Renders a declared `Event`'s parameter list as `Type name, ...`, for the
/// mismatch diagnostic's message.
fn describe_params(params: &[Param]) -> String {
    params
        .iter()
        .map(|param| format!("{} {}", type_display(&param.type_name), param.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Renders a [`KnownEventRule`]'s expected parameter list the same way
/// [`describe_params`] renders a declared one, so both can be shown side by
/// side in the mismatch diagnostic's message.
fn describe_args(args: &[EventArg]) -> String {
    args.iter()
        .map(|arg| format!("{} {}", arg.type_name, arg.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Renders a parsed [`TypeName`] the way it's written in Papyrus source
/// (`Type` or `Type[]`).
fn type_display(type_name: &TypeName) -> String {
    if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name.clone()
    }
}

#[cfg(test)]
#[path = "event_signature_tests.rs"]
mod tests;
