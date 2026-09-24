//! Flags a `.psc` whose declared `ScriptName` doesn't match its file stem,
//! aside from casing. Papyrus resolves and compiles a script by matching the
//! two, and rejects a mismatch at compile time.
//!
//! The `ScriptName` line is a run of lexer tokens (`ScriptName`, an
//! identifier, then any `:segment` pieces). The file stem is not in the
//! source, so a caller passes it in. This stays out of `collect_diagnostics`
//! for that reason — the same way [`crate::conflicting_script_versions`]
//! takes a project snapshot the dispatch never has.

use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::Diagnostic;

/// This diagnostic's [`Diagnostic::rule`] id, for `@disable` line comments
/// and the `rules.script_filename_mismatch` config key.
pub const RULE: &str = "script-filename-mismatch";

/// Checks `tokens` for a `ScriptName` whose final `:`-separated segment
/// differs from `file_stem` (case-insensitively).
///
/// `file_stem` is the `.psc` file's stem (`Path::file_stem`), not a path and
/// not the `.psc` suffix. A Fallout 4-style name (`User:MyScript`, stored at
/// `Scripts/Source/User/MyScript.psc`) is compared by that final segment
/// only — the namespace is the containing folder, not part of the file name.
/// No `ScriptName`, a `ScriptName` that isn't followed by an identifier (and
/// optional `:identifier` pieces), or an empty `file_stem` yields nothing.
///
/// This does not apply `; @disable` / `; @disable-file`. Callers merge the
/// diagnostic through
/// [`crate::lint_with_external_arguments_and_extra_diagnostics`], which
/// honors a matching directive and counts it as used.
pub fn check(file_stem: &str, tokens: &[Token]) -> Option<Diagnostic> {
    if file_stem.is_empty() {
        return None;
    }
    let mut tokens = tokens.iter().peekable();
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
        while tokens.peek().map(|next| &next.kind) == Some(&TokenKind::Colon) {
            tokens.next();
            let TokenKind::Identifier(segment) = &tokens.next()?.kind else {
                return None;
            };
            full_name.push(':');
            full_name.push_str(segment);
            last_segment.clone_from(segment);
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
