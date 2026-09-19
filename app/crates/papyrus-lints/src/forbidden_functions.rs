//! Flags calls to functions listed in `shared/rules/data/forbidden-functions.yaml`
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

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
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

#[derive(Default)]
struct Collect {
    store: Store,
    if_stack: Vec<bool>,
    debug_guard_depth: usize,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        match &token.kind {
            TokenKind::Keyword(Keyword::If) => {
                let guarded = is_simple_debug_guard(tokens, index + 1);
                self.if_stack.push(guarded);
                if guarded {
                    self.debug_guard_depth += 1;
                }
            }
            TokenKind::Keyword(Keyword::ElseIf) => {
                replace_current_branch(
                    &mut self.if_stack,
                    &mut self.debug_guard_depth,
                    is_simple_debug_guard(tokens, index + 1),
                );
            }
            TokenKind::Keyword(Keyword::Else) => {
                replace_current_branch(&mut self.if_stack, &mut self.debug_guard_depth, false);
            }
            TokenKind::Keyword(Keyword::EndIf) => {
                if let Some(guarded) = self.if_stack.pop() {
                    if guarded {
                        self.debug_guard_depth = self.debug_guard_depth.saturating_sub(1);
                    }
                }
            }
            TokenKind::Identifier(name)
                if tokens
                    .get(index + 1)
                    .is_some_and(|token| matches!(token.kind, TokenKind::LParen)) =>
            {
                let Some(rule) = find_rule(name) else {
                    return;
                };
                if rule.global && !qualifier_matches(tokens, index, rule.script) {
                    return;
                }
                if is_debug_script(rule) && self.debug_guard_depth > 0 {
                    return;
                }
                self.store.emit(
                    token.line,
                    token.col,
                    format!(
                        "[{}] {}.{}: {}",
                        rule.level, rule.script, rule.function, rule.message
                    ),
                    RULE,
                );
            }
            _ => {}
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

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
#[path = "forbidden_functions_tests.rs"]
mod tests;
