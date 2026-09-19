//! Flags a call to a side-effecting function nested inside any `Debug.*`
//! argument list.
//!
//! `Debug.Trace` / `Debug.Notification` / other `Debug.*` calls are often
//! compiled out, gated behind a debug flag, or skipped in release-like
//! playthroughs. An argument that itself calls something like
//! `RemoveItem` or a same-script function that writes a property then
//! only runs when that debug call runs, which is almost never what the
//! author intended.
//!
//! Matching is token-based so a script that doesn't parse cleanly is
//! still checked. Same-script side effects come from the project function
//! index's canonical flag; calls that can't be resolved that way are
//! classified by a conservative name heuristic covering common
//! mutating native prefixes (`Set`, `Remove`, `Wait`, …).

use std::collections::HashMap;

use papyrus_parser::token::{Token, TokenKind};

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "debug-side-effects";

#[derive(Default)]
struct Collect {
    store: Store,
    same_script: HashMap<String, bool>,
    skip_until: usize,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.same_script.clear();
        if let Some(script) = ctx.ast {
            for function in script
                .functions
                .iter()
                .chain(script.states.iter().flat_map(|state| &state.functions))
            {
                let key = function.name.to_ascii_lowercase();
                if self.same_script.contains_key(&key) {
                    continue;
                }
                if let Some(has_side_effects) = ctx
                    .external
                    .function_has_side_effects(&script.name, &function.name)
                {
                    self.same_script.insert(key, has_side_effects);
                }
            }
        }
        self.skip_until = 0;
    }

    fn visit_token(
        &mut self,
        _token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        if index < self.skip_until {
            return;
        }
        if index + 3 >= tokens.len() || !is_debug_call(tokens, index) {
            return;
        }
        let method = match &tokens[index + 2].kind {
            TokenKind::Identifier(name) => name.clone(),
            _ => return,
        };
        let open = index + 3;
        let Some(close) = matching_rparen(tokens, open) else {
            return;
        };
        let mut diagnostics = Vec::new();
        collect_nested_calls(
            tokens,
            open + 1,
            close,
            &method,
            &self.same_script,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
        self.skip_until = close + 1;
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Native / conventional names that mutate game or script state even
/// when this script's own function list can't see their bodies.
const SIDE_EFFECT_PREFIXES: &[&str] = &[
    "set",
    "mod",
    "remove",
    "add",
    "enable",
    "disable",
    "force",
    "drop",
    "equip",
    "unequip",
    "delete",
    "reset",
    "start",
    "stop",
    "play",
    "interrupt",
    "evaluate",
    "move",
    "push",
    "apply",
    "cast",
    "dispel",
    "unlock",
    "lock",
    "kill",
    "resurrect",
    "place",
    "damage",
    "restore",
    "clear",
    "wait",
    "send",
    "register",
    "unregister",
    "goto",
    "fire",
];

/// Checks `source` for a side-effecting call nested inside any `Debug.*`
/// argument list. Flagged as a `[warning]`.
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

fn is_debug_call(tokens: &[Token], i: usize) -> bool {
    matches!(
        (
            &tokens[i].kind,
            &tokens[i + 1].kind,
            &tokens[i + 2].kind,
            &tokens[i + 3].kind,
        ),
        (
            TokenKind::Identifier(qualifier),
            TokenKind::Dot,
            TokenKind::Identifier(_),
            TokenKind::LParen,
        ) if qualifier.eq_ignore_ascii_case("Debug")
    )
}

fn matching_rparen(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_nested_calls(
    tokens: &[Token],
    from: usize,
    to: usize,
    debug_method: &str,
    same_script: &HashMap<String, bool>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut i = from;
    while i + 1 < to {
        if let TokenKind::Identifier(name) = &tokens[i].kind {
            if matches!(tokens[i + 1].kind, TokenKind::LParen)
                && looks_side_effecting(name, same_script)
            {
                diagnostics.push(Diagnostic {
                    line: tokens[i].line,
                    column: tokens[i].col,
                    message: format!(
                        "[warning] Call to '{name}' inside Debug.{debug_method} can run side \
                         effects only when that debug call executes; move the call out of the \
                         Debug argument list"
                    ),
                    rule: RULE,
                });
            }
        }
        i += 1;
    }
}

fn looks_side_effecting(name: &str, same_script: &HashMap<String, bool>) -> bool {
    let key = name.to_ascii_lowercase();
    if let Some(&proven) = same_script.get(&key) {
        return proven;
    }
    SIDE_EFFECT_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}


#[cfg(test)]
#[path = "debug_side_effects_tests.rs"]
mod tests;
