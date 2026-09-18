//! Flags `Game.GetFormFromFile(x, "Skyrim.esm")` and suggests
//! `Game.GetForm(x)` instead.
//!
//! `Skyrim.esm` is always loaded at index `00`, so a FormID resolved from
//! that file through `GetFormFromFile` is the same FormID `Game.GetForm`
//! looks up directly. The file-name lookup is the slower path, and writing
//! it for the base game is never necessary.
//!
//! Like [`crate::formid_hex_notation`], this works on lexer tokens rather
//! than the parsed AST so it still runs on scripts that don't parse
//! cleanly. Only a call qualified with the literal `Game` singleton is
//! matched (`Game` is never subclassed). The filename argument must be a
//! string literal equal to `Skyrim.esm` (case-insensitively), passed
//! positionally or by name (`asFilename`). A filename reached through a
//! variable is left unflagged rather than guessed at.
//!
//! [`check`] and [`repair`] share [`visit_skyrim_get_form_from_file`] so the
//! set of calls the fix rewrites can never drift from the set the
//! diagnostic flags.

use papyrus_parser::token::{Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "getform-from-skyrim-esm";

const MESSAGE: &str = "[warning] Game.GetFormFromFile(x, \"Skyrim.esm\") looks the form up by \
     file name; use `Game.GetForm(x)` instead, since Skyrim.esm is always \
     loaded at index 00 and GetForm resolves the same FormID directly";

/// Checks `source` for `Game.GetFormFromFile` calls whose filename argument
/// is the `Skyrim.esm` string literal.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    visit_skyrim_get_form_from_file(&tokens, source, |call| {
        diagnostics.push(Diagnostic {
            line: tokens[call.name_index].line,
            column: tokens[call.name_index].col,
            message: MESSAGE.to_string(),
            rule: RULE,
        });
    });
    diagnostics
}

/// Rewrites every `Game.GetFormFromFile(x, "Skyrim.esm")` call [`check`]
/// would flag into `Game.GetForm(x)`, leaving every other token exactly as
/// it was.
pub fn repair(source: &str) -> String {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);

    let mut edits = Vec::new();
    visit_skyrim_get_form_from_file(&tokens, source, |call| {
        let start = token_offset(&line_starts, &tokens[call.game_index]);
        let end = token_offset(&line_starts, &tokens[call.close_index]) + 1;
        let form_id = source[call.form_id_start..call.form_id_end].trim();
        let game = match &tokens[call.game_index].kind {
            TokenKind::Identifier(name) => name.as_str(),
            _ => "Game",
        };
        edits.push((start, end, format!("{game}.GetForm({form_id})"));
    });
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));

    let mut repaired = source.to_string();
    for (start, end, replacement) in edits {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

struct SkyrimCall {
    game_index: usize,
    name_index: usize,
    close_index: usize,
    form_id_start: usize,
    form_id_end: usize,
}

fn visit_skyrim_get_form_from_file(
    tokens: &[Token],
    source: &str,
    mut on_match: impl FnMut(SkyrimCall),
) {
    let line_starts = line_starts(source);
    for i in 0..tokens.len() {
        if !is_game_get_form_from_file_call(tokens, i) {
            continue;
        }
        let Some(close_index) = matching_close_paren(tokens, i + 1) else {
            continue;
        };
        let Some((form_id_lo, form_id_hi)) = form_id_arg_range(tokens, i + 2, close_index) else {
            continue;
        };
        if !filename_is_skyrim_esm(tokens, i + 2, close_index) {
            continue;
        }
        on_match(SkyrimCall {
            game_index: i - 2,
            name_index: i,
            close_index,
            form_id_start: token_offset(&line_starts, &tokens[form_id_lo]),
            form_id_end: token_end(&line_starts, source, &tokens[form_id_hi]),
        });
    }
}

fn is_identifier(token: &Token, name: &str) -> bool {
    matches!(&token.kind, TokenKind::Identifier(actual) if actual.eq_ignore_ascii_case(name))
}

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

fn matching_close_paren(tokens: &[Token], open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

struct Arg<'a> {
    name: Option<&'a str>,
    first: usize,
    last: usize,
}

fn arguments(tokens: &[Token], inner_start: usize, close_index: usize) -> Vec<Arg<'_>> {
    let mut args = Vec::new();
    let mut i = inner_start;
    while i < close_index {
        if matches!(tokens[i].kind, TokenKind::Comma | TokenKind::Newline) {
            i += 1;
            continue;
        }
        let mut name = None;
        let mut first = i;
        if let (Some(ident), Some(assign)) = (tokens.get(i), tokens.get(i + 1)) {
            if let TokenKind::Identifier(ident_name) = &ident.kind {
                if matches!(assign.kind, TokenKind::Assign) {
                    name = Some(ident_name.as_str());
                    first = i + 2;
                    i += 2;
                }
            }
        }
        if i >= close_index {
            break;
        }
        let mut depth = 0usize;
        let mut last = first;
        while i < close_index {
            match tokens[i].kind {
                TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RParen | TokenKind::RBracket => depth = depth.saturating_sub(1),
                TokenKind::Comma if depth == 0 => break,
                _ => {}
            }
            last = i;
            i += 1;
        }
        args.push(Arg {
            name,
            first,
            last,
        });
    }
    args
}

fn form_id_arg_range(
    tokens: &[Token],
    inner_start: usize,
    close_index: usize,
) -> Option<(usize, usize)> {
    let args = arguments(tokens, inner_start, close_index);
    for (position, arg) in args.iter().enumerate() {
        let is_form_id = match arg.name {
            Some(name) => {
                name.eq_ignore_ascii_case("aiFormID") || name.eq_ignore_ascii_case("auiFormID")
            }
            None => position == 0,
        };
        if is_form_id {
            return Some((arg.first, arg.last));
        }
    }
    None
}

fn filename_is_skyrim_esm(tokens: &[Token], inner_start: usize, close_index: usize) -> bool {
    let args = arguments(tokens, inner_start, close_index);
    for (position, arg) in args.iter().enumerate() {
        let is_filename = match arg.name {
            Some(name) => name.eq_ignore_ascii_case("asFilename"),
            None => position == 1,
        };
        if !is_filename {
            continue;
        }
        if arg.first != arg.last {
            return false;
        }
        return matches!(
            &tokens[arg.first].kind,
            TokenKind::StringLiteral(value) if value.eq_ignore_ascii_case("Skyrim.esm")
        );
    }
    false
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

fn token_end(line_starts: &[usize], source: &str, token: &Token) -> usize {
    let start = token_offset(line_starts, token);
    match &token.kind {
        TokenKind::Identifier(name) => start + name.len(),
        TokenKind::StringLiteral(value) => start + value.len() + 2,
        TokenKind::IntLiteral(value, _) => {
            let rest = &source[start..];
            if rest.len() >= 2
                && rest.as_bytes()[0] == b'0'
                && rest.as_bytes()[1].eq_ignore_ascii_case(&b'x')
            {
                let digits = rest[2..]
                    .bytes()
                    .take_while(|b| b.is_ascii_hexdigit())
                    .count();
                start + 2 + digits
            } else {
                start + value.abs().to_string().len()
            }
        }
        TokenKind::FloatLiteral(_) => source[start..]
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .map(|n| start + n)
            .unwrap_or(source.len()),
        _ => start + 1,
    }
}

#[cfg(test)]
#[path = "getform_from_skyrim_esm_tests.rs"]
mod tests;
