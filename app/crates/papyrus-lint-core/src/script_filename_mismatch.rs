//! Flags a `.psc` file whose declared `ScriptName` doesn't match its own
//! file name, aside from casing, since Papyrus resolves/compiles a script by
//! matching the two and rejects a mismatch at compile time.
//!
//! Needs `script_path` to compare against, so — like [`crate::stale_pex`] —
//! this can't live in `papyrus-lints` alongside every other lint, which only
//! ever sees a script's source text. Works on lexer tokens (rather than the
//! parsed AST) so it still runs on scripts that don't parse cleanly,
//! matching every other token-based lint in `papyrus-lints`.

use std::path::Path;

use papyrus_lints::Diagnostic;
use papyrus_parser::token::{Keyword, TokenKind};

/// This diagnostic's [`Diagnostic::rule`] id, for `@disable` line comments
/// and the `rules.script_filename_mismatch` config key.
pub const RULE: &str = "script-filename-mismatch";

/// Checks `source`'s declared `ScriptName` against `script_path`'s own file
/// stem (case-insensitively). A script with no `ScriptName` statement, one
/// that fails to lex, or a `script_path` with no file stem to compare
/// against yields no diagnostic. A Fallout 4-style namespaced name (e.g.
/// `ScriptName User:MyScript`, stored at `Scripts/Source/User/MyScript.psc`)
/// is compared by its final `:`-separated segment only, since the leading
/// namespace segment(s) are encoded as the script's containing subfolder
/// rather than part of its file name. Unlike every rule in `papyrus-lints`
/// itself, this can't be filtered by `; @disable`/`; @disable-file` inside
/// [`papyrus_lints::lint`]/[`papyrus_lints::lint_with_external_arguments`]
/// directly (this check needs `script_path`, which neither ever sees), so
/// this always returns the diagnostic regardless of any directive; a caller
/// merges it in via
/// [`papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`]
/// instead, which honors a matching directive (and validates it as used)
/// the same way it does for every other diagnostic.
pub fn check(script_path: &Path, source: &str) -> Option<Diagnostic> {
    let file_stem = script_path.file_stem()?.to_str()?;
    let tokens = papyrus_parser::tokenize(source).ok()?;

    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        if token.kind != TokenKind::Keyword(Keyword::ScriptName) {
            continue;
        }
        let name_token = tokens.next()?;
        let TokenKind::Identifier(first_segment) = &name_token.kind else {
            return None;
        };
        let mut full_name = first_segment.clone();
        let mut last_segment = first_segment.clone();
        while tokens.peek().map(|t| &t.kind) == Some(&TokenKind::Colon) {
            tokens.next();
            let TokenKind::Identifier(segment) = &tokens.next()?.kind else {
                return None;
            };
            full_name.push(':');
            full_name.push_str(segment);
            last_segment = segment.clone();
        }
        if last_segment.eq_ignore_ascii_case(file_stem) {
            return None;
        }
        return Some(Diagnostic {
            line: name_token.line,
            column: name_token.col,
            rule: RULE,
            message: format!(
                "[error] Script name '{full_name}' does not match its file name '{file_stem}'; Papyrus requires them to match (aside from casing, and any leading 'Namespace:' segment)"
            ),
        });
    }
    None
}

#[cfg(test)]
#[path = "script_filename_mismatch_tests.rs"]
mod tests;
