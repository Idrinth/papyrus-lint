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

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use crate::token_walk::{
    is_game_get_form_from_file_call, line_starts, matching_close_paren,
    skip_named_argument_prefix, split_arguments,
};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-form-from-file-skyrim-esm";

#[derive(Default)]
struct Collect {
    store: Store,
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
        if !is_matching_call(tokens, index) {
            return;
        }
        self.store.emit(
            token.line,
            token.col,
            "[warning] Game.GetFormFromFile(id, \"Skyrim.esm\") always resolves to \
             exactly what Game.GetForm(id) already returns, since Skyrim.esm is always \
             loaded as master index 0; use Game.GetForm(id) instead and drop the file \
             name argument",
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for a qualified `Game.GetFormFromFile` call whose file
/// name argument is the literal `"Skyrim.esm"` (case-insensitively).
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

/// Rewrites every flagged call into `Game.GetForm(<id>)`, keeping the
/// original FormID argument's source text (including any expression it's
/// part of) and dropping the `"Skyrim.esm"` file name argument entirely.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

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
        if let Some((close, value_start, delimiter)) = matching_call(tokens, call_index) {
            on_match(call_index, close, value_start, delimiter);
        }
    }
}

fn is_matching_call(tokens: &[Token], call_index: usize) -> bool {
    matching_call(tokens, call_index).is_some()
}

fn matching_call(tokens: &[Token], call_index: usize) -> Option<(usize, usize, usize)> {
        if !is_game_get_form_from_file_call(tokens, call_index) {
            return None;
        }
        let open = call_index + 1;
        let close = matching_close_paren(tokens, open)?;
        let args = split_arguments(tokens, open, close);
        let [first, second] = args.as_slice() else {
            return None;
        };
        let form_id_arg = if is_skyrim_esm_literal(tokens, first.0, first.1) {
            *second
        } else if is_skyrim_esm_literal(tokens, second.0, second.1) {
            *first
        } else {
            return None;
        };
        let value_start = skip_named_argument_prefix(tokens, form_id_arg.0, form_id_arg.1);
        Some((close, value_start, form_id_arg.1 + 1))
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

fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

#[cfg(test)]
#[path = "get_form_from_file_skyrim_esm_tests.rs"]
mod tests;
