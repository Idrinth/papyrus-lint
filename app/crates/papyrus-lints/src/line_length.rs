//! Flags source lines that exceed the configured maximum character count.

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "line-length";

/// Checks every physical source line against [`crate::config::Config::max_line_length`].
/// Line endings are not included in the count, and Unicode scalar values count as one
/// character each.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, external);

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let length = line.chars().count();
            (length > config.max_line_length).then(|| Diagnostic {
                line: index + 1,
                column: config.max_line_length + 1,
                message: format!(
                    "[warning] Line is {length} characters long (maximum is {})",
                    config.max_line_length
                ),
                rule: RULE,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "line_length_tests.rs"]
mod tests;
