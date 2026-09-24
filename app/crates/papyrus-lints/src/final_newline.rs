//! Flags non-empty files that do not end with a newline.

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` comments.
pub const RULE: &str = "final-newline";

/// Checks that a non-empty `source` ends with a newline.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, config, external);

    if source.is_empty() || source.ends_with('\n') {
        return Vec::new();
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

/// Appends a newline to a non-empty `source` that does not already end with
/// one. Uses `\r\n` when the file already contains a CR so a CRLF script
/// stays CRLF; otherwise `\n`. Empty source is left untouched.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    if source.is_empty() || source.ends_with('\n') {
        return source.to_string();
    }

    let mut repaired = String::with_capacity(source.len() + 2);
    repaired.push_str(source);
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
