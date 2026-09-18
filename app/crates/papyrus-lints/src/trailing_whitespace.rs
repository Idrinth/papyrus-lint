//! Flags lines that end with trailing spaces or tabs.

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "trailing-whitespace";

const TRAILING_WHITESPACE: [char; 2] = [' ', '\t'];

/// Checks `source` for lines ending in trailing spaces or tabs. Lines
/// inside a CreationKit fragment-code wrapper (see [`fragment_code`]),
/// outside of its `;BEGIN CODE`/`;END CODE` markers, are never flagged.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, config, external);

    let protected = fragment_code::protected_lines(source);

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if protected[index + 1] {
                return None;
            }

            let trimmed = line.trim_end_matches(TRAILING_WHITESPACE);
            if trimmed.len() == line.len() {
                return None;
            }

            Some(Diagnostic {
                line: index + 1,
                column: trimmed.chars().count() + 1,
                message: "[warning] Line contains trailing whitespace".to_string(),
                rule: RULE,
            })
        })
        .collect()
}

/// Strips trailing spaces/tabs from every line of `source`, preserving each
/// line's original ending (`\n`, `\r\n`, or none for a final line without a
/// trailing newline) and leaving lines that have no trailing whitespace
/// untouched. Lines protected by a CreationKit fragment-code wrapper (see
/// [`fragment_code`]) are left exactly as-is.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    let protected = fragment_code::protected_lines(source);
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;

    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };

        if protected[line_number] {
            result.push_str(line_and_ending);
        } else {
            let (content, ending) = if let Some(stripped) = line_and_ending.strip_suffix("\r\n") {
                (stripped, "\r\n")
            } else if let Some(stripped) = line_and_ending.strip_suffix('\n') {
                (stripped, "\n")
            } else {
                (line_and_ending, "")
            };

            result.push_str(content.trim_end_matches(TRAILING_WHITESPACE));
            result.push_str(ending);
        }

        rest = remainder;
        line_number += 1;
    }

    result
}

#[cfg(test)]
#[path = "trailing_whitespace_tests.rs"]
mod tests;
