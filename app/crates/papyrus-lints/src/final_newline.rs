//! Flags non-empty files that do not end with exactly one newline.

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` comments.
pub const RULE: &str = "final-newline";

/// Checks that a non-empty `source` ends with exactly one newline.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, config, external);

    if source.is_empty() || ends_with_single_newline(source) {
        return Vec::new();
    }

    if source.ends_with('\n') {
        let line = source.bytes().filter(|byte| *byte == b'\n').count();
        return vec![Diagnostic {
            line,
            column: 1,
            message: "[warning] File ends with extra blank lines".to_string(),
            rule: RULE,
        }];
    }

    let line = source.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let final_line = source.rsplit_once('\n').map_or(source, |(_, line)| line);
    vec![Diagnostic {
        line,
        column: final_line.trim_end_matches('\r').chars().count() + 1,
        message: "[warning] File does not end with a newline".to_string(),
        rule: RULE,
    }]
}

/// True when `source` ends with a single `\n` or `\r\n` and that is the only
/// trailing blank line. Empty source is handled by the caller.
fn ends_with_single_newline(source: &str) -> bool {
    let without_one = if let Some(rest) = source.strip_suffix("\r\n") {
        rest
    } else if let Some(rest) = source.strip_suffix('\n') {
        rest
    } else {
        return false;
    };
    !without_one.ends_with('\n')
}

/// Ensures a non-empty `source` ends with exactly one newline. Uses `\r\n`
/// when the file already contains a CR so a CRLF script stays CRLF; otherwise
/// `\n`. Extra trailing newlines are collapsed. Empty source is left untouched.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    if source.is_empty() {
        return source.to_string();
    }

    let trimmed = source.trim_end_matches(['\n', '\r']);
    let mut repaired = String::with_capacity(trimmed.len() + 2);
    repaired.push_str(trimmed);
    if source.contains('\r') {
        repaired.push_str("\r\n");
    } else {
        repaired.push('\n');
    }
    repaired
}

#[cfg(test)]
#[path = "final_newline_tests.rs"]
mod tests;
