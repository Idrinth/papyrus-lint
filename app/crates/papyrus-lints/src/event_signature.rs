//! Flags, as a `[warning]`, an `Event` declaration whose name matches one of
//! the engine's own native events (per `rules/known-events.yaml`, which also
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
//! alone. Disabled by default: `rules/known-events.yaml` only lists a
//! curated subset of the engine's native events, and a script that declares
//! an `Event` sharing one of those names without actually extending the
//! listed Form (e.g. its own unrelated event handler that happens to reuse
//! a common name) would otherwise be misreported here.

use papyrus_parser::ast::{FunctionDecl, Param, Script, TypeName};

use crate::Diagnostic;

/// One parameter of a [`KnownEventRule`]'s expected signature.
pub struct EventArg {
    pub type_name: &'static str,
    pub name: &'static str,
}

/// One `rules/known-events.yaml` entry: an event's name, the Form that
/// first declares it, and its exact expected parameter list.
pub struct KnownEventRule {
    pub event: &'static str,
    pub form: &'static str,
    pub args: &'static [EventArg],
}

include!(concat!(env!("OUT_DIR"), "/known_events_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "event-signature-mismatch";

/// Checks `source` for `Event` declarations whose name matches a known
/// native event but whose parameter list doesn't match its signature.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_events(&script)
        .filter_map(|event| {
            let rule = find_rule(&event.name)?;
            if signature_matches(&event.params, rule.args) {
                return None;
            }
            Some(Diagnostic {
                line: event.line,
                column: 1,
                message: format!(
                    "[warning] Event {}({}) does not match the signature {}({}) declared on {}; the engine will not invoke it correctly",
                    event.name,
                    describe_params(&event.params),
                    rule.event,
                    describe_args(rule.args),
                    rule.form
                ),
                rule: RULE,
            })
        })
        .collect()
}

/// Iterates every `Event` declared directly on a script, plus every `Event`
/// declared in each of its states.
fn all_events(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script
        .functions
        .iter()
        .chain(
            script
                .states
                .iter()
                .flat_map(|state| state.functions.iter()),
        )
        .filter(|function| function.is_event)
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
mod tests {
    use super::*;

    #[test]
    fn compiled_known_events_are_loaded_from_yaml() {
        assert!(!KNOWN_EVENTS.is_empty());
        assert!(KNOWN_EVENTS
            .iter()
            .any(|rule| rule.event == "OnInit" && rule.form == "ScriptObject"));
    }

    #[test]
    fn does_not_flag_a_matching_no_argument_event() {
        let diagnostics = check("ScriptName Example\n\nEvent OnInit()\nEndEvent\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_matching_event_with_arguments() {
        let diagnostics = check(
            "ScriptName Example\n\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_case_insensitively() {
        let diagnostics = check("ScriptName Example\n\nevent oninit()\nendevent\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_missing_argument() {
        let diagnostics = check("ScriptName Example\n\nEvent OnActivate()\nEndEvent\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("OnActivate"));
        assert!(diagnostics[0].message.contains("ObjectReference"));
    }

    #[test]
    fn flags_a_mismatched_argument_type() {
        let diagnostics =
            check("ScriptName Example\n\nEvent OnActivate(Actor akActionRef)\nEndEvent\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("OnActivate"));
    }

    #[test]
    fn flags_an_extra_argument() {
        let diagnostics = check("ScriptName Example\n\nEvent OnInit(Int aiUnexpected)\nEndEvent\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("OnInit"));
    }

    #[test]
    fn flags_an_event_declared_inside_a_state() {
        let diagnostics = check(
            "ScriptName Example\n\nState Busy\n    Event OnActivate()\n    EndEvent\nEndState\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_an_unrelated_event_name() {
        let diagnostics =
            check("ScriptName Example\n\nEvent MyCustomEvent(Int aiValue)\nEndEvent\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_with_the_same_name_as_an_event() {
        let diagnostics = check("ScriptName Example\n\nFunction OnActivate()\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_every_known_event_with_its_own_correct_signature() {
        for rule in KNOWN_EVENTS {
            let params = rule
                .args
                .iter()
                .map(|arg| format!("{} {}", arg.type_name, arg.name))
                .collect::<Vec<_>>()
                .join(", ");
            let source = format!(
                "ScriptName Example\n\nEvent {}({params})\nEndEvent\n",
                rule.event
            );
            let diagnostics = check(&source);
            assert!(
                diagnostics.is_empty(),
                "{} was unexpectedly flagged",
                rule.event
            );
        }
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nEvent OnInit(\n");
        assert!(diagnostics.is_empty());
    }
}
