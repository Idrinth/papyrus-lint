//! Enforces a configured trailing-semicolon style.

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "semicolon";

/// The supported trailing-semicolon policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Require,
    Forbid,
}

/// Checks non-empty lines for the configured trailing-semicolon style.
/// Lines inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// are never flagged.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, external);
    let style = config.semicolon_style();

    let protected = fragment_code::protected_lines(source);

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if protected[index + 1] {
                return None;
            }

            let content = line.trim_end_matches([' ', '\t', '\r']);
            if content.is_empty() {
                return None;
            }

            let has_semicolon = content.ends_with(';');
            let message = match (style, has_semicolon) {
                (Style::Require, false) => "[warning] Line should end with a semicolon",
                (Style::Forbid, true) => "[warning] Line should not end with a semicolon",
                _ => return None,
            };

            Some(Diagnostic {
                line: index + 1,
                column: content.chars().count() + usize::from(!has_semicolon),
                message: message.to_string(),
                rule: RULE,
            })
        })
        .collect()
}

/// Adds or removes terminal semicolons while retaining line endings. In
/// forbid mode only terminal semicolons are removed, so comment text is never
/// discarded. Lines protected by a CreationKit fragment-code wrapper (see
/// [`fragment_code`]) are left exactly as-is.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let style = config.semicolon_style();

    let protected = fragment_code::protected_lines(source);
    let mut result = String::with_capacity(source.len());
    for (line_number, line_and_ending) in (1usize..).zip(source.split_inclusive('\n')) {
        if protected[line_number] {
            result.push_str(line_and_ending);
        } else {
            let (line, ending) = line_and_ending.strip_suffix("\r\n").map_or_else(
                || {
                    line_and_ending
                        .strip_suffix('\n')
                        .map_or((line_and_ending, ""), |line| (line, "\n"))
                },
                |line| (line, "\r\n"),
            );
            repair_line(&mut result, line, style);
            result.push_str(ending);
        }
    }

    result
}

fn repair_line(result: &mut String, line: &str, style: Style) {
    let content = line.trim_end_matches([' ', '\t']);
    let whitespace = &line[content.len()..];

    match style {
        Style::Require if !content.is_empty() && !content.ends_with(';') => {
            result.push_str(content);
            result.push(';');
            result.push_str(whitespace);
        }
        Style::Forbid if content.ends_with(';') => {
            result.push_str(&content[..content.len() - 1]);
            result.push_str(whitespace);
        }
        _ => result.push_str(line),
    }
}

#[cfg(test)]
#[path = "semicolon_tests.rs"]
mod tests;
