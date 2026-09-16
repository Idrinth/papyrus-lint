//! Flags calls to functions listed in `rules/forbidden-functions.yaml`
//! (e.g. functions with known performance or reliability pitfalls).
//!
//! Rules are compiled into the `FORBIDDEN_FUNCTIONS` array below by
//! `build.rs` at build time, so this never parses YAML at runtime. Like
//! the other lints in this crate, it works on tokens rather than the
//! parsed AST, so it still runs on scripts that don't parse cleanly.
//!
//! `Debug.*` calls (`Trace`, `TraceStack`, `Notification`) are the entries
//! whose own messages describe diagnostic-only usage. A call nested inside
//! an `If`/`ElseIf` whose condition is a simple identifier
//! (optionally a `Self`/`Parent`/identifier member chain, optionally
//! wrapped in parentheses) whose name contains `debug` — e.g.
//! `If IsDebugMode` — is therefore left unflagged. An `Else` of that
//! chain, a negated or compound condition, or a name that doesn't look
//! like a debug flag is still flagged, as is every other forbidden
//! function even when it sits behind the same guard.

use crate::Diagnostic;
use papyrus_parser::token::{Keyword, Token, TokenKind};

pub struct ForbiddenFunctionRule {
    pub script: &'static str,
    pub function: &'static str,
    pub level: &'static str,
    pub message: &'static str,
    /// Whether `script` is a native singleton (e.g. `Game`, `Utility`)
    /// always called through its literal script name, rather than a base
    /// type (e.g. `ObjectReference`, `ScriptObject`) called through a
    /// variable of some subclass. See `check` for how this is used.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/forbidden_functions_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "forbidden-functions";

/// Checks `source` for calls to forbidden/discouraged functions.
///
/// A call site is any identifier immediately followed by `(`. The lexer
/// has no type/symbol resolution, so — like the receiver in
/// `akRef.GetLinkedRef()` — a call's qualifier can't generally be
/// resolved back to the script that declares the function; matching is
/// therefore done by function name alone, case-insensitively (Papyrus
/// identifiers are case-insensitive).
///
/// The exception is a rule whose `script` is a native singleton
/// (`global: true` in the YAML, e.g. `Utility`) rather than a base type
/// used through a variable: those scripts are never subclassed, so a
/// qualified call to one of their functions is only a real match when the
/// qualifier is literally that script's name. This is what keeps
/// `Utility.Wait()` flagged while `MyScript.Wait()` (a same-named function
/// on an unrelated script) is not.
///
/// A second exception is a `Debug.*` call already nested inside a simple
/// debug-flag `If`/`ElseIf` (see the module docs): those calls are the
/// guarded form the rules themselves recommend, so they are not
/// reported.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    let mut diagnostics = Vec::new();
    let mut if_stack: Vec<bool> = Vec::new();
    let mut debug_guard_depth = 0usize;

    for i in 0..tokens.len() {
        match &tokens[i].kind {
            TokenKind::Keyword(Keyword::If) => {
                let guarded = is_simple_debug_guard(&tokens, i + 1);
                if_stack.push(guarded);
                if guarded {
                    debug_guard_depth += 1;
                }
            }
            TokenKind::Keyword(Keyword::ElseIf) => {
                replace_current_branch(
                    &mut if_stack,
                    &mut debug_guard_depth,
                    is_simple_debug_guard(&tokens, i + 1),
                );
            }
            TokenKind::Keyword(Keyword::Else) => {
                replace_current_branch(&mut if_stack, &mut debug_guard_depth, false);
            }
            TokenKind::Keyword(Keyword::EndIf) => {
                if let Some(guarded) = if_stack.pop() {
                    if guarded {
                        debug_guard_depth = debug_guard_depth.saturating_sub(1);
                    }
                }
            }
            TokenKind::Identifier(name)
                if tokens
                    .get(i + 1)
                    .is_some_and(|token| matches!(token.kind, TokenKind::LParen)) =>
            {
                let Some(rule) = find_rule(name) else {
                    continue;
                };
                if rule.global && !qualifier_matches(&tokens, i, rule.script) {
                    continue;
                }
                if is_debug_script(rule) && debug_guard_depth > 0 {
                    continue;
                }
                diagnostics.push(Diagnostic {
                    line: tokens[i].line,
                    column: tokens[i].col,
                    message: format!(
                        "[{}] {}.{}: {}",
                        rule.level, rule.script, rule.function, rule.message
                    ),
                    rule: RULE,
                });
            }
            _ => {}
        }
    }
    diagnostics
}

/// Whether the call at `tokens[call_index]` is qualified with `script`
/// (case-insensitively), i.e. preceded by `script.`.
fn qualifier_matches(tokens: &[Token], call_index: usize, script: &str) -> bool {
    if call_index < 2 {
        return false;
    }
    if !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(script)
}

fn find_rule(name: &str) -> Option<&'static ForbiddenFunctionRule> {
    FORBIDDEN_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

fn is_debug_script(rule: &ForbiddenFunctionRule) -> bool {
    rule.script.eq_ignore_ascii_case("Debug")
}

fn replace_current_branch(if_stack: &mut [bool], debug_guard_depth: &mut usize, guarded: bool) {
    let Some(current) = if_stack.last_mut() else {
        return;
    };
    if *current {
        *debug_guard_depth = debug_guard_depth.saturating_sub(1);
    }
    *current = guarded;
    if guarded {
        *debug_guard_depth += 1;
    }
}

/// Whether the `If`/`ElseIf` condition starting at `start` is a simple
/// debug-flag identifier: a bare name or `Self`/`Parent`/identifier member
/// chain whose last identifier contains `debug` (case-insensitively),
/// optionally wrapped in parentheses, and nothing else before the
/// newline that ends the condition.
fn is_simple_debug_guard(tokens: &[Token], mut i: usize) -> bool {
    let mut opens = 0;
    while i < tokens.len() && matches!(tokens[i].kind, TokenKind::LParen) {
        opens += 1;
        i += 1;
    }

    let mut last_name = match tokens.get(i).map(|token| &token.kind) {
        Some(TokenKind::Keyword(Keyword::Self_)) => {
            i += 1;
            "self"
        }
        Some(TokenKind::Keyword(Keyword::Parent)) => {
            i += 1;
            "parent"
        }
        Some(TokenKind::Identifier(name)) => {
            i += 1;
            name.as_str()
        }
        _ => return false,
    };

    while i < tokens.len() && matches!(tokens[i].kind, TokenKind::Dot) {
        i += 1;
        match tokens.get(i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => {
                last_name = name;
                i += 1;
            }
            _ => return false,
        }
    }

    if !last_name.to_ascii_lowercase().contains("debug") {
        return false;
    }

    let mut closes = 0;
    while i < tokens.len() && matches!(tokens[i].kind, TokenKind::RParen) {
        closes += 1;
        i += 1;
    }
    if closes != opens {
        return false;
    }

    matches!(
        tokens.get(i).map(|token| &token.kind),
        Some(TokenKind::Newline | TokenKind::Eof) | None
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert_eq!(FORBIDDEN_FUNCTIONS.len(), 12);
        assert!(FORBIDDEN_FUNCTIONS
            .iter()
            .any(|r| r.script == "Game" && r.function == "GetPlayer" && r.level == "error"));
    }

    #[test]
    fn flags_qualified_call() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0]
            .message
            .starts_with("[error] Game.GetPlayer:"));
    }

    #[test]
    fn flags_code_before_an_inline_comment_but_ignores_calls_in_comment_text() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Actor a = Game.GetPlayer(); artificially slow Game.GetPlayer()\n    ; Game.GetPlayer() is forbidden\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].column, 20);
        assert!(diagnostics[0].message.contains("Game.GetPlayer"));
    }

    #[test]
    fn flags_unqualified_call() {
        let diagnostics = check(
            "ScriptName Example extends ObjectReference\n\nFunction DoThing()\n    GetLinkedRef()\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("GetLinkedRef"));
    }

    #[test]
    fn flags_call_on_arbitrary_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(ObjectReference akRef)\n    akRef.RegisterForUpdate(1.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    }

    #[test]
    fn does_not_flag_unrelated_calls() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"hi\")\n    self.DoOtherThing()\nEndFunction\n\nFunction DoOtherThing()\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_identifiers_that_are_not_calls() {
        let diagnostics = check("ScriptName Example\n\nInt GetPlayer = 1\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer(\"unterminated\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_global_singleton_call_qualified_by_its_own_name() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Utility.Wait(1.0)\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Utility.Wait"));
    }

    #[test]
    fn does_not_flag_same_named_function_on_a_different_script() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(MyScript akOther)\n    akOther.Wait(1.0)\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_global_function_called_on_an_expression_result() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    GetUtility().Wait(1.0)\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_unqualified_call_to_a_global_singleton_function() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Wait(1.0)\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_unguarded_debug_trace() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.Trace(\"left in\")\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Debug.Trace"));
    }

    #[test]
    fn does_not_flag_debug_trace_behind_a_simple_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nEvent OnEndState()\n  If IsDebugMode\n    Debug.Trace(\"State Transition -> Exited [Reverse]\", 0)\n  EndIf\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_debug_trace_behind_a_parenthesized_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If (IsDebugMode)\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_debug_trace_behind_a_member_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    If Self.IsDebugMode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_debug_trace_behind_an_elseif_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready, Bool IsDebugMode)\n    If ready\n        Return\n    ElseIf IsDebugMode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_nested_debug_trace_inside_a_debug_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready, Bool IsDebugMode)\n    If IsDebugMode\n        If ready\n            Debug.Trace(\"still guarded\")\n        EndIf\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_debug_trace_behind_an_unrelated_bool() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready)\n    If ready\n        Debug.Trace(\"not a debug flag\")\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Debug.Trace"));
    }

    #[test]
    fn flags_debug_trace_in_the_else_of_a_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Return\n    Else\n        Debug.Trace(\"not guarded\")\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Debug.Trace"));
    }

    #[test]
    fn flags_debug_trace_behind_a_negated_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If !IsDebugMode\n        Debug.Trace(\"inverted\")\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Debug.Trace"));
    }

    #[test]
    fn flags_debug_trace_behind_a_compound_debug_condition() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode, Bool ready)\n    If IsDebugMode && ready\n        Debug.Trace(\"not simple\")\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Debug.Trace"));
    }

    #[test]
    fn still_flags_other_forbidden_calls_behind_a_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Game.GetPlayer()\n        Utility.Wait(1.0)\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("Game.GetPlayer"));
        assert!(diagnostics[1].message.contains("Utility.Wait"));
    }

    #[test]
    fn does_not_flag_debug_tracestack_or_notification_behind_a_debug_flag() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Debug.Trace(\"log\")\n        Debug.TraceStack(\"dump\")\n        Debug.Notification(\"hi\")\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_unguarded_debug_tracestack_and_notification() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.TraceStack(\"dump\")\n    Debug.Notification(\"hi\")\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("Debug.TraceStack"));
        assert!(diagnostics[1].message.contains("Debug.Notification"));
    }

    #[test]
    fn debug_flag_match_is_case_insensitive() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool isdebugmode)\n    If isdebugmode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }
}
