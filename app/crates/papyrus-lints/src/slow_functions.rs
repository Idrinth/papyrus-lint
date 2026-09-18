//! Flags calls to functions listed in `rules/slow-functions.yaml` that
//! have a faster equivalent available, and suggests that replacement.
//!
//! Rules are compiled into the `SLOW_FUNCTIONS` array below by `build.rs`
//! at build time, so this never parses YAML at runtime. Like the other
//! lints in this crate, it works on tokens rather than the parsed AST, so
//! it still runs on scripts that don't parse cleanly.

use crate::Diagnostic;
use papyrus_parser::token::TokenKind;

pub struct SlowFunctionRule {
    pub object: &'static str,
    pub function: &'static str,
    pub replacement: &'static str,
    /// Whether `object` is a native singleton (e.g. `Game`, `Utility`)
    /// always called through its literal script name, rather than a base
    /// type (e.g. `GlobalVariable`) called through a variable of some
    /// subclass. See `check` for how this is used.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/slow_functions_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "slow-functions";

/// Checks `source` for calls to functions with a faster equivalent.
///
/// A call site is any identifier immediately followed by `(`. The lexer
/// has no type/symbol resolution, so — like the receiver in
/// `akGlobal.GetValueInt()` — a call's qualifier can't generally be
/// resolved back to the script that declares the function; matching is
/// therefore done by function name alone, case-insensitively (Papyrus
/// identifiers are case-insensitive). Flagged as an `[info]`, since it's a
/// performance suggestion rather than a correctness issue.
///
/// The exception is a rule whose `object` is a native singleton
/// (`global: true` in the YAML, e.g. `Utility`) rather than a base type
/// used through a variable: those scripts are never subclassed, so a
/// qualified call to one of their functions is only a real match when the
/// qualifier is literally that script's name.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, ast, config, external);

    check_with_rules(tokens, SLOW_FUNCTIONS)
}

/// Replaces slow calls with the faster expression supplied by their rule.
///
/// A replacement containing `(` describes the complete call. The word
/// `value` in such a replacement is a placeholder for the original call's
/// argument. A bare replacement names the faster function and retains the
/// original argument list. Calls whose parentheses are unbalanced are left
/// untouched.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    repair_with_rules(source, SLOW_FUNCTIONS)
}

fn repair_with_rules(source: &str, rules: &'static [SlowFunctionRule]) -> String {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let mut replacements = Vec::new();

    for (call_index, token) in tokens.iter().enumerate() {
        let TokenKind::Identifier(name) = &token.kind else {
            continue;
        };
        if !matches!(
            tokens.get(call_index + 1).map(|token| &token.kind),
            Some(TokenKind::LParen)
        ) {
            continue;
        }
        let Some(rule) = find_rule(rules, name) else {
            continue;
        };
        if rule.global && !qualifier_matches(&tokens, call_index, rule.object) {
            continue;
        }
        let Some(close_index) = matching_close_paren(&tokens, call_index + 1) else {
            continue;
        };

        let start = token_offset(&line_starts, token);
        let argument_start = token_offset(&line_starts, &tokens[call_index + 1]) + 1;
        let close = token_offset(&line_starts, &tokens[close_index]);
        let end = close + 1;
        // An outer slow call owns its entire source range. Repair its argument
        // recursively and don't also queue an overlapping inner edit against
        // offsets from the original string.
        if replacements
            .iter()
            .any(|(outer_start, outer_end, _)| *outer_start <= start && end <= *outer_end)
        {
            continue;
        }
        let argument = repair_with_rules(source[argument_start..close].trim(), rules);
        let replacement = if rule.replacement.contains('(') {
            rule.replacement.replace("value", &argument)
        } else {
            format!("{}({argument})", rule.replacement)
        };
        replacements.push((start, end, replacement));
    }

    let mut repaired = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

fn matching_close_paren(
    tokens: &[papyrus_parser::token::Token],
    open_index: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

fn token_offset(line_starts: &[usize], token: &papyrus_parser::token::Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

fn check_with_rules(
    tokens: Option<&[papyrus_parser::token::Token]>,
    rules: &'static [SlowFunctionRule],
) -> Vec<Diagnostic> {
    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for (i, window) in tokens.windows(2).enumerate() {
        let TokenKind::Identifier(name) = &window[0].kind else {
            continue;
        };
        if !matches!(window[1].kind, TokenKind::LParen) {
            continue;
        }
        let Some(rule) = find_rule(rules, name) else {
            continue;
        };
        if rule.global && !qualifier_matches(tokens, i, rule.object) {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: window[0].line,
            column: window[0].col,
            message: format!(
                "[info] {}.{} is slower than necessary; use `{}` instead",
                rule.object, rule.function, rule.replacement
            ),
            rule: RULE,
        });
    }
    diagnostics
}

/// Whether the call at `tokens[call_index]` is qualified with `object`
/// (case-insensitively), i.e. preceded by `object.`.
fn qualifier_matches(
    tokens: &[papyrus_parser::token::Token],
    call_index: usize,
    object: &str,
) -> bool {
    if call_index < 2 {
        return false;
    }
    if !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(object)
}

fn find_rule(rules: &'static [SlowFunctionRule], name: &str) -> Option<&'static SlowFunctionRule> {
    rules
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "slow_functions_tests.rs"]
mod tests;
