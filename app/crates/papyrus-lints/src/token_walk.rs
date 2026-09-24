//! Shared walks over a Papyrus token stream.
//!
//! Several token lints need the same small primitives (physical-line
//! offsets, parenthesis matching, argument splitting, a `script.` qualifier
//! check). They live here so those copies cannot drift apart.

use papyrus_parser::token::{Token, TokenKind};

/// Byte offsets of each physical line start in `source` (line 1 at index 0).
pub(crate) fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

/// Index of the `)` that matches the `(` at `open_index`, or `None` if the
/// stream ends before the group is closed.
pub(crate) fn matching_close_paren(tokens: &[Token], open_index: usize) -> Option<usize> {
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

/// Index of the `(` that matches the `)` at `close_index`, or `None` if the
/// stream begins before the group is opened.
pub(crate) fn matching_open_paren(tokens: &[Token], close_index: usize) -> Option<usize> {
    let mut depth = 0;
    for index in (0..=close_index).rev() {
        match tokens[index].kind {
            TokenKind::RParen => depth += 1,
            TokenKind::LParen => {
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

/// Whether `token` is an identifier equal to `name`, ignoring ASCII case.
pub(crate) fn is_identifier(token: &Token, name: &str) -> bool {
    matches!(&token.kind, TokenKind::Identifier(actual) if actual.eq_ignore_ascii_case(name))
}

/// Whether the call at `tokens[call_index]` is qualified with `script`
/// (case-insensitively), i.e. preceded by `script.`.
pub(crate) fn qualifier_matches(tokens: &[Token], call_index: usize, script: &str) -> bool {
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

/// Whether `tokens[index]` starts a `GetFormFromFile(...)` call qualified by
/// the literal `Game` singleton. `Game` is never subclassed, so matching
/// the literal script name is the only way the call resolves to it.
pub(crate) fn is_game_get_form_from_file_call(tokens: &[Token], index: usize) -> bool {
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

/// Splits the arguments between `(` at `open` and its matching `)` at
/// `close` into top-level, comma-separated `(start, end)` token index
/// ranges (both inclusive), splitting only on commas outside any nested
/// parentheses. Returns an empty `Vec` for a call with no arguments at all.
pub(crate) fn split_arguments(tokens: &[Token], open: usize, close: usize) -> Vec<(usize, usize)> {
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

/// Skips a leading `identifier =` named-argument prefix within
/// `tokens[start..=end]`, returning the index the actual value starts at
/// (`start` itself when there's no such prefix).
pub(crate) fn skip_named_argument_prefix(tokens: &[Token], start: usize, end: usize) -> usize {
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

/// Splits `tokens` on top-level arithmetic, comparison, and boolean
/// operators (paren- and bracket-aware) into the operand slices between
/// them. A leading or doubled operator produces no empty slice.
pub(crate) fn top_level_operands(tokens: &[Token]) -> Vec<&[Token]> {
    let mut operands = Vec::new();
    let mut start = 0;
    let mut paren_depth: usize = 0;
    let mut bracket_depth: usize = 0;

    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Gt
            | TokenKind::Lt
            | TokenKind::GtEq
            | TokenKind::LtEq
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Not
                if paren_depth == 0 && bracket_depth == 0 =>
            {
                if index > start {
                    operands.push(&tokens[start..index]);
                }
                start = index + 1;
            }
            _ => {}
        }
    }

    if start < tokens.len() {
        operands.push(&tokens[start..]);
    }

    operands
}
