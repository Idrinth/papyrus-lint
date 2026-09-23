//! Flags a hex FormID literal passed to `Game.GetFormFromFile` that carries
//! data outside the local FormID range its file name argument's own plugin
//! type actually uses, or that pads the value with leading zero digits
//! beyond what's needed to write it.
//!
//! `Game.GetFormFromFile` always resolves the load index (a full plugin's
//! master index, or a light plugin's own index among other light plugins)
//! from the file name argument itself at runtime, discarding whatever the
//! FormID argument's own high bits say instead. A full plugin's local
//! FormID only ever needs the low 24 bits (up to `0xFFFFFF`); a light
//! plugin's (a file name literally ending in `.esl`) only the low 12
//! (`0xFFF`). A leading zero digit never changes a hex literal's value
//! either way, so it serves no use on top of that.
//!
//! Like [`crate::formid_hex_notation`]/[`crate::get_form_from_file_skyrim_esm`],
//! this works on lexer tokens rather than the parsed AST, and only flags a
//! FormID literal that is the argument's entire value (not part of a larger
//! expression) next to a file name argument that is itself a literal
//! string, so the local range being checked against is actually known
//! rather than guessed at.

use papyrus_parser::token::{IntFormat, Token, TokenKind};

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-form-from-file-load-index";

/// Bits of local FormID space a full plugin (`.esp`/`.esm`) gets; the rest
/// is the master index `Game.GetFormFromFile` resolves from the file name
/// argument instead.
const FULL_PLUGIN_LOCAL_BITS: u32 = 24;
/// Bits of local FormID space a light plugin (a file name literally ending
/// in `.esl`) gets; the rest is its own load index among other light
/// plugins, resolved from the file name argument the same way.
const LIGHT_PLUGIN_LOCAL_BITS: u32 = 12;

#[derive(Default)]
struct Collect {
    store: Store,
    line_starts: Vec<usize>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.line_starts = line_starts(ctx.source);
    }

    fn visit_token(
        &mut self,
        _token: &Token,
        index: usize,
        tokens: &[Token],
        ctx: &mut VisitCtx<'_>,
    ) {
        let Some((literal_index, filename_index)) = matching_call(tokens, index) else {
            return;
        };
        if let Some(diagnostic) = diagnostic_for(
            tokens,
            literal_index,
            filename_index,
            ctx.source,
            &self.line_starts,
        ) {
            self.store.push(diagnostic);
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for a hex FormID literal passed to a qualified
/// `Game.GetFormFromFile` call that carries load-index bits or leading zero
/// padding its file name argument's plugin type never uses.
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

/// A bare (not part of a larger expression) literal argument's kind and the
/// token index its value lives at.
enum ArgLiteral {
    Int(usize),
    Str(usize),
}

/// Whether `tokens[call_index]` starts a qualified `Game.GetFormFromFile`
/// call whose two arguments (in either order, positionally or by name) are
/// exactly a FormID literal and a file name string literal. Returns the
/// FormID literal's token index and the file name literal's token index.
fn matching_call(tokens: &[Token], call_index: usize) -> Option<(usize, usize)> {
    if !is_game_get_form_from_file_call(tokens, call_index) {
        return None;
    }
    let open = call_index + 1;
    let close = matching_close_paren(tokens, open)?;
    let args = split_arguments(tokens, open, close);
    let [first, second] = args.as_slice() else {
        return None;
    };
    match (
        bare_literal(tokens, first.0, first.1),
        bare_literal(tokens, second.0, second.1),
    ) {
        (Some(ArgLiteral::Int(literal)), Some(ArgLiteral::Str(filename))) => {
            Some((literal, filename))
        }
        (Some(ArgLiteral::Str(filename)), Some(ArgLiteral::Int(literal))) => {
            Some((literal, filename))
        }
        _ => None,
    }
}

/// Builds the diagnostic for the FormID literal at `literal_index`, given
/// the file name string literal at `filename_index`, or `None` when the
/// literal isn't written in hexadecimal (a decimal one is
/// [`crate::formid_hex_notation`]'s concern, not this rule's) or carries
/// neither load-index bits nor leading zero padding.
fn diagnostic_for(
    tokens: &[Token],
    literal_index: usize,
    filename_index: usize,
    source: &str,
    line_starts: &[usize],
) -> Option<Diagnostic> {
    let TokenKind::IntLiteral(value, format) = tokens[literal_index].kind else {
        return None;
    };
    if format != IntFormat::Hexadecimal || value < 0 {
        return None;
    }
    let TokenKind::StringLiteral(ref filename) = tokens[filename_index].kind else {
        return None;
    };
    let is_light_plugin = filename.trim_end().to_ascii_lowercase().ends_with(".esl");
    let local_bits = if is_light_plugin {
        LIGHT_PLUGIN_LOCAL_BITS
    } else {
        FULL_PLUGIN_LOCAL_BITS
    };
    let local_max = (1i64 << local_bits) - 1;
    let local_value = value & local_max;

    let literal = &tokens[literal_index];
    let message = if local_value != value {
        format!(
            "[warning] FormID {value:#X} passed to Game.GetFormFromFile carries a non-zero \
             load index; \"{filename}\" only gets a local FormID up to {local_max:#X} and \
             Game.GetFormFromFile always resolves the load index from the file name argument \
             itself, discarding whatever this literal's own upper bits say, so write it as \
             {local_value:#X}"
        )
    } else {
        let digits = hex_digits_text(source, line_starts, literal);
        let minimal_digits = format!("{value:X}").len();
        if digits.len() <= minimal_digits {
            return None;
        }
        format!(
            "[warning] FormID 0x{digits} passed to Game.GetFormFromFile is padded with leading \
             zero digits that don't change its value; write it as {value:#X}"
        )
    };

    Some(Diagnostic {
        line: literal.line,
        column: literal.col,
        message,
        rule: RULE,
    })
}

/// Whether the argument spanning `tokens[start..=end]` is, once an optional
/// `name =` named-argument prefix is skipped, a bare literal (an
/// `IntLiteral` or `StringLiteral`) and nothing else.
fn bare_literal(tokens: &[Token], start: usize, end: usize) -> Option<ArgLiteral> {
    let value_start = skip_named_argument_prefix(tokens, start, end);
    if value_start != end {
        return None;
    }
    match &tokens[value_start].kind {
        TokenKind::IntLiteral(_, _) => Some(ArgLiteral::Int(value_start)),
        TokenKind::StringLiteral(_) => Some(ArgLiteral::Str(value_start)),
        _ => None,
    }
}

/// The hex digit characters of `token` as written in `source` (everything
/// after its `0x`/`0X` prefix), so leading zero padding can be told apart
/// from a literal already written at its minimal length.
fn hex_digits_text<'a>(source: &'a str, line_starts: &[usize], token: &Token) -> &'a str {
    let start = token_offset(line_starts, token) + 2;
    let bytes = source.as_bytes();
    let mut end = start;
    while end < bytes.len() && (bytes[end] as char).is_ascii_hexdigit() {
        end += 1;
    }
    &source[start..end]
}

/// Whether `tokens[index]` starts a `GetFormFromFile(...)` call qualified by
/// the literal `Game` singleton, the same way
/// [`crate::get_form_from_file_skyrim_esm`]/[`crate::formid_hex_notation`]
/// only match it through its literal script name (`Game` is never
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
#[path = "get_form_from_file_load_index_tests.rs"]
mod tests;
