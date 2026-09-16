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
mod tests {
    use super::*;

    #[test]
    fn flags_a_script_name_that_does_not_match_its_file_name() {
        let diagnostic = check(Path::new("Other.psc"), "ScriptName Example\n")
            .expect("mismatched script name should be flagged");

        assert_eq!(diagnostic.line, 1);
        assert_eq!(diagnostic.rule, RULE);
        assert!(diagnostic.message.starts_with("[error]"));
        assert!(diagnostic.message.contains("'Example'"));
        assert!(diagnostic.message.contains("'Other'"));
    }

    #[test]
    fn does_not_flag_a_matching_script_name() {
        assert!(check(Path::new("Example.psc"), "ScriptName Example\n").is_none());
    }

    #[test]
    fn matches_case_insensitively() {
        assert!(check(Path::new("example.psc"), "ScriptName EXAMPLE\n").is_none());
        assert!(check(Path::new("Example.psc"), "ScriptName example\n").is_none());
    }

    #[test]
    fn ignores_a_script_with_no_scriptname_statement() {
        assert!(check(Path::new("Example.psc"), "; comment only\n").is_none());
    }

    #[test]
    fn ignores_a_scriptname_without_a_declared_name() {
        assert!(check(Path::new("Example.psc"), "ScriptName").is_none());
    }

    #[test]
    fn ignores_a_scriptname_followed_by_a_non_identifier() {
        assert!(check(Path::new("Example.psc"), "ScriptName 123\n").is_none());
    }

    #[test]
    fn ignores_an_incomplete_namespace() {
        assert!(check(Path::new("Example.psc"), "ScriptName User:\n").is_none());
    }

    #[test]
    fn ignores_a_namespace_segment_that_is_not_an_identifier() {
        assert!(check(Path::new("Example.psc"), "ScriptName User:123\n").is_none());
    }

    #[test]
    fn ignores_a_script_that_fails_to_lex() {
        assert!(check(
            Path::new("Other.psc"),
            "ScriptName Example \"unterminated\n"
        )
        .is_none());
    }

    #[test]
    fn ignores_a_path_with_no_file_stem() {
        assert!(check(Path::new("/"), "ScriptName Example\n").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn ignores_a_path_with_a_non_utf8_file_stem() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new(OsStr::from_bytes(b"Example\xFF.psc"));

        assert!(check(path, "ScriptName Example\n").is_none());
    }

    #[test]
    fn compares_against_the_whole_stem_before_only_the_final_extension() {
        // `file_stem` only strips the last extension, so a file name with an
        // extra dot segment (e.g. a versioned file name) is compared against
        // its full stem, dot included — an identifier can't contain a dot,
        // so no `ScriptName` can ever match one anyway, but this documents
        // that `check` doesn't strip anything beyond the final extension.
        let diagnostic = check(Path::new("Quest.v2.psc"), "ScriptName Quest\n")
            .expect("mismatched script name should be flagged");

        assert!(diagnostic.message.contains("'Quest.v2'"));
    }

    #[test]
    fn does_not_flag_a_namespaced_script_name_matching_only_its_final_segment() {
        assert!(check(
            Path::new("Scripts/Source/User/MyScript.psc"),
            "ScriptName User:MyScript\n"
        )
        .is_none());
    }

    #[test]
    fn flags_a_namespaced_script_name_whose_final_segment_does_not_match() {
        let diagnostic = check(
            Path::new("Scripts/Source/User/Other.psc"),
            "ScriptName User:MyScript\n",
        )
        .expect("mismatched namespaced script name should be flagged");

        assert!(diagnostic.message.contains("'User:MyScript'"));
        assert!(diagnostic.message.contains("'Other'"));
    }

    #[test]
    fn reports_the_scriptname_identifiers_own_position() {
        let diagnostic = check(Path::new("Other.psc"), "\n\nScriptName   Example\n")
            .expect("mismatched script name should be flagged");

        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 14);
    }

    #[test]
    fn does_not_honor_a_disable_comment_itself_since_the_caller_filters_it_in() {
        // `check` no longer applies `; @disable`/`; @disable-file` itself —
        // see this module's own docs above. A caller merges its result into
        // `papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`
        // instead, which filters it (and validates the directive as used)
        // the same way it does for every other diagnostic; that's covered by
        // integration tests in `papyrus-lint-cli` and `src-tauri`.
        assert!(check(
            Path::new("Other.psc"),
            "ScriptName Example ; @disable script-filename-mismatch\n"
        )
        .is_some());
        assert!(check(Path::new("Other.psc"), "ScriptName Example ; @disable\n").is_some());
        assert!(check(
            Path::new("Other.psc"),
            "ScriptName Example\n; @disable-file script-filename-mismatch\n"
        )
        .is_some());
    }
}
