//! Flags a `.psc` whose declared `ScriptName` doesn't match its relative path,
//! aside from casing. Papyrus resolves and compiles a script by matching the
//! two, and rejects a mismatch at compile time.
//!
//! The `ScriptName` line is a run of lexer tokens (`ScriptName`, an
//! identifier, then any `:segment` pieces). The file stem is not in the
//! source, so a caller passes it in. This stays out of `collect_diagnostics`
//! for that reason — the same way [`crate::conflicting_script_versions`]
//! takes a project snapshot the dispatch never has.

use std::path::Path;

use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::Diagnostic;

/// This diagnostic's [`Diagnostic::rule`] id, for `@disable` line comments
/// and the `rules.script_filename_mismatch` config key.
pub const RULE: &str = "script-filename-mismatch";

/// Checks `tokens` for a `ScriptName` that differs from `relative_path`
/// (case-insensitively).
///
/// A qualified Fallout 4-style name (`User:MyScript`) must match the path
/// below its script search root (`User/MyScript.psc`). Leading folders that
/// are not part of that namespace (FO4 `Base/`, a DLC package folder the
/// ScriptName does not start with) are ignored so the comparison uses the
/// package root the ScriptName is written against. An unqualified name
/// keeps the Skyrim behavior and is compared only with the file stem. No
/// `ScriptName`, an invalid declaration, or a path without a UTF-8 stem
/// yields nothing.
///
/// This does not apply `; @disable` / `; @disable-file`. Callers merge the
/// diagnostic through
/// [`crate::lint_with_external_arguments_and_extra_diagnostics`], which
/// honors a matching directive and counts it as used.
pub fn check(relative_path: &Path, tokens: &[Token]) -> Option<Diagnostic> {
    let file_stem = relative_path.file_stem()?.to_str()?;
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
        let mut segments = vec![first_segment.as_str()];
        while tokens.peek().map(|next| &next.kind) == Some(&TokenKind::Colon) {
            tokens.next();
            let TokenKind::Identifier(segment) = &tokens.next()?.kind else {
                return None;
            };
            full_name.push(':');
            full_name.push_str(segment);
            segments.push(segment);
        }
        let matches = if segments.len() == 1 {
            first_segment.eq_ignore_ascii_case(file_stem)
        } else {
            path_matches_namespace(relative_path, file_stem, &segments)
        };
        if matches {
            return None;
        }
        return Some(Diagnostic {
            line: name_token.line,
            column: name_token.col,
            rule: RULE,
            message: format!(
                "[error] Script name '{full_name}' does not match its relative file path '{}'; Papyrus requires them to match aside from casing",
                relative_path.display()
            ),
        });
    }
    None
}

/// Whether `relative_path` ends with the ScriptName namespace folders.
///
/// Extra leading directories that do not appear in `segments` are layout
/// roots (`Base/`, a DLC pack folder the name does not include), not
/// namespace members. Folders after the first namespace segment still have
/// to match exactly.
fn path_matches_namespace(relative_path: &Path, file_stem: &str, segments: &[&str]) -> bool {
    let mut actual: Vec<_> = relative_path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| {
            let std::path::Component::Normal(part) = component else {
                return None;
            };
            part.to_str()
        })
        .collect();
    actual.push(file_stem);
    while actual.len() > segments.len() {
        let Some(leading) = actual.first() else {
            break;
        };
        if segments
            .iter()
            .any(|segment| segment.eq_ignore_ascii_case(leading))
        {
            break;
        }
        actual.remove(0);
    }
    segments.len() == actual.len()
        && segments
            .iter()
            .zip(actual)
            .all(|(expected, actual)| expected.eq_ignore_ascii_case(actual))
}

#[cfg(test)]
#[path = "script_filename_mismatch_tests.rs"]
mod tests;
