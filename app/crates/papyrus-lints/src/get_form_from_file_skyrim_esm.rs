//! Flags `Game.GetFormFromFile(<id>, "Skyrim.esm")`, which always resolves
//! to exactly what `Game.GetForm(<id>)` already returns: `Skyrim.esm` is
//! always loaded as master index `0`, so a FormID that's valid for it never
//! needs the file name argument's extra resolution step at all.
//!
//! Like [`crate::forbidden_functions`]/[`crate::formid_hex_notation`], this
//! works on lexer tokens rather than the parsed AST, so it still runs on
//! scripts that don't parse cleanly. [`check`] and [`repair`] share
//! [`visit_matching_calls`] so the set of calls the fix rewrites can never
//! drift from the set the diagnostic flags.

use papyrus_parser::token::{Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-form-from-file-skyrim-esm";

/// Checks `source` for a qualified `Game.GetFormFromFile` call whose file
/// name argument is the literal `"Skyrim.esm"` (case-insensitively).
pub fn check(tokens: Option<&[Token]>) -> Vec<Diagnostic> {
    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    visit_matching_calls(tokens, |call_index, _close, _value_start, _delimiter| {
        let call_token = &tokens[call_index];
        diagnostics.push(Diagnostic {
            line: call_token.line,
            column: call_token.col,
            message: "[warning] Game.GetFormFromFile(id, \"Skyrim.esm\") always resolves to \
                      exactly what Game.GetForm(id) already returns, since Skyrim.esm is always \
                      loaded as master index 0; use Game.GetForm(id) instead and drop the file \
                      name argument"
                .to_string(),
            rule: RULE,
        });
    });
    diagnostics
}

/// Rewrites every flagged call into `Game.GetForm(<id>)`, keeping the
/// original FormID argument's source text (including any expression it's
/// part of) and dropping the `"Skyrim.esm"` file name argument entirely.
pub fn repair(source: &str) -> String {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);

    let mut edits = Vec::new();
    visit_matching_calls(
        &tokens,
        |call_index, close_index, value_start, delimiter| {
            let start = token_offset(&line_starts, &tokens[call_index]);
            let end = token_offset(&line_starts, &tokens[close_index]) + 1;
            let value_start_offset = token_offset(&line_starts, &tokens[value_start]);
            let value_end_offset = token_offset(&line_starts, &tokens[delimiter]);
            let form_id = source[value_start_offset..value_end_offset].trim();
            edits.push((start, end, format!("GetForm({form_id})")));
        },
    );
    // Applied back-to-front so an earlier edit's byte offsets stay valid
    // regardless of how a later edit on the same line changes its length.
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));

    let mut repaired = source.to_string();
    for (start, end, replacement) in edits {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

/// Walks `tokens` for every qualified `Game.GetFormFromFile(...)` call whose
/// two arguments are exactly a FormID expression and the literal
/// `"Skyrim.esm"` (in either order, positionally or by name), calling
/// `on_match` with the `GetFormFromFile` identifier's own token index, its
/// call's closing `)` token index, the FormID argument's first token index
/// (with any `name =` prefix already skipped), and the token index of the
/// delimiter (a `,` or the closing `)`) immediately following it. The single
/// place [`check`] and [`repair`] agree on which calls this lint flags.
fn visit_matching_calls(tokens: &[Token], mut on_match: impl FnMut(usize, usize, usize, usize)) {
    for call_index in 0..tokens.len() {
        if !is_game_get_form_from_file_call(tokens, call_index) {
            continue;
        }
        let open = call_index + 1;
        let Some(close) = matching_close_paren(tokens, open) else {
            continue;
        };
        let args = split_arguments(tokens, open, close);
        let [first, second] = args.as_slice() else {
            continue;
        };
        let form_id_arg = if is_skyrim_esm_literal(tokens, first.0, first.1) {
            *second
        } else if is_skyrim_esm_literal(tokens, second.0, second.1) {
            *first
        } else {
            continue;
        };
        let value_start = skip_named_argument_prefix(tokens, form_id_arg.0, form_id_arg.1);
        on_match(call_index, close, value_start, form_id_arg.1 + 1);
    }
}

/// Whether `tokens[index]` starts a `GetFormFromFile(...)` call qualified by
/// the literal `Game` singleton, the same way [`crate::formid_hex_notation`]
/// only matches it through its literal script name (`Game` is never
/// subclassed, so this is the only way the call resolves to it).
fn is_game_get_form_from_file_call(tokens: &[Token], index: usize) -> bool {
    if !is_identifier(&tokens[index], "GetFormFromFile") {
        return false;
    }
    if !matches!(
        tokens.get(index + 1).map(|t| &t.kind),
        Some(TokenKind::LParen)
    ) {
        return false;
    }
    if index < 2 || !matches!(tokens[index - 1].kind, TokenKind::Dot) {
        return false;
    }
    is_identifier(&tokens[index - 2], "Game")
}

fn is_identifier(token: &Token, name: &str) -> bool {
    matches!(&token.kind, TokenKind::Identifier(actual) if actual.eq_ignore_ascii_case(name))
}

/// Index of the `)` matching the `(` at `open_index`.
fn matching_close_paren(tokens: &[Token], open_index: usize) -> Option<usize> {
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

/// Splits the arguments between `(` at `open` and its matching `)` at
/// `close` into top-level, comma-separated `(start, end)` token index
/// ranges (both inclusive), splitting only on commas outside any nested
/// parentheses. Returns an empty `Vec` for a call with no arguments at all.
fn split_arguments(tokens: &[Token], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut args = Vec::new();
    if open + 1 >= close {
        return args;
    }

    let mut segment_start = open + 1;
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(close).skip(open + 1) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                args.push((segment_start, index - 1));
                segment_start = index + 1;
            }
            _ => {}
        }
    }
    args.push((segment_start, close - 1));
    args
}

/// Whether the argument spanning `tokens[start..=end]` is, once an optional
/// `name =` named-argument prefix is skipped, the bare string literal
/// `"Skyrim.esm"` (case-insensitively) and nothing else.
fn is_skyrim_esm_literal(tokens: &[Token], start: usize, end: usize) -> bool {
    let value_start = skip_named_argument_prefix(tokens, start, end);
    if value_start != end {
        return false;
    }
    matches!(
        &tokens[value_start].kind,
        TokenKind::StringLiteral(value) if value.eq_ignore_ascii_case("Skyrim.esm")
    )
}

/// Skips a leading `identifier =` named-argument prefix within
/// `tokens[start..=end]`, returning the index the actual value starts at
/// (`start` itself when there's no such prefix).
fn skip_named_argument_prefix(tokens: &[Token], start: usize, end: usize) -> usize {
    if start < end
        && matches!(tokens[start].kind, TokenKind::Identifier(_))
        && matches!(
            tokens.get(start + 1).map(|t| &t.kind),
            Some(TokenKind::Assign)
        )
    {
        start + 2
    } else {
        start
    }
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

fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

#[cfg(test)]
#[path = "get_form_from_file_skyrim_esm_tests.rs"]
mod tests;
