//! Flags source that cannot be represented in the configured file encoding.

use crate::config::EncodingEnforced;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` comments.
pub const RULE: &str = "forced-encoding";

/// Checks that `source` can be encoded as `config.encoding_enforced`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, external);

    if first_unencodable(source, config.encoding_enforced).is_none() {
        return Vec::new();
    }

    vec![Diagnostic {
        line: 1,
        column: 1,
        message: format!(
            "[warning] File is not valid {}",
            config.encoding_enforced.label()
        ),
        rule: RULE,
    }]
}

/// Rewrites unencodable characters to `?` so the file fits the configured encoding.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    source
        .chars()
        .map(|ch| {
            if encoding_allows(ch, config.encoding_enforced) {
                ch
            } else {
                '?'
            }
        })
        .collect()
}

fn first_unencodable(source: &str, encoding: EncodingEnforced) -> Option<char> {
    source.chars().find(|ch| !encoding_allows(*ch, encoding))
}

fn encoding_allows(ch: char, encoding: EncodingEnforced) -> bool {
    match encoding {
        EncodingEnforced::Utf8 => true,
        EncodingEnforced::Iso88591 => u32::from(ch) <= 0xFF,
        EncodingEnforced::Windows1252 => is_windows_1252(ch),
    }
}

fn is_windows_1252(ch: char) -> bool {
    matches!(
        u32::from(ch),
        0x00..=0x7F
            | 0xA0..=0xFF
            | 0x20AC
            | 0x201A
            | 0x0192
            | 0x201E
            | 0x2026
            | 0x2020
            | 0x2021
            | 0x02C6
            | 0x2030
            | 0x0160
            | 0x2039
            | 0x0152
            | 0x017D
            | 0x2018
            | 0x2019
            | 0x201C
            | 0x201D
            | 0x2022
            | 0x2013
            | 0x2014
            | 0x02DC
            | 0x2122
            | 0x0161
            | 0x203A
            | 0x0153
            | 0x017E
            | 0x0178
    )
}

#[cfg(test)]
#[path = "forced_encoding_tests.rs"]
mod tests;
