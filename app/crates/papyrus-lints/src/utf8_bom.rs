//! Flags a leading UTF-8 BOM according to `utf8_bom_mode`.

use crate::config::Utf8BomMode;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` comments.
pub const RULE: &str = "utf8-bom";

const BOM: char = '\u{FEFF}';

/// Checks the leading UTF-8 BOM against `config.utf8_bom_mode`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, external);
    let has_bom = source.starts_with(BOM);

    match config.utf8_bom_mode {
        Utf8BomMode::Allowed => Vec::new(),
        Utf8BomMode::Forbidden if has_bom => vec![Diagnostic {
            line: 1,
            column: 1,
            message: "[warning] File starts with a UTF-8 BOM".to_string(),
            rule: RULE,
        }],
        Utf8BomMode::Required if !has_bom => vec![Diagnostic {
            line: 1,
            column: 1,
            message: "[warning] File is missing a UTF-8 BOM".to_string(),
            rule: RULE,
        }],
        _ => Vec::new(),
    }
}

/// Inserts or strips a leading UTF-8 BOM to match `config.utf8_bom_mode`.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let rest = source.strip_prefix(BOM).unwrap_or(source);
    match config.utf8_bom_mode {
        Utf8BomMode::Required => {
            let mut out = String::with_capacity(rest.len() + BOM.len_utf8());
            out.push(BOM);
            out.push_str(rest);
            out
        }
        Utf8BomMode::Forbidden | Utf8BomMode::Allowed => rest.to_string(),
    }
}

#[cfg(test)]
#[path = "utf8_bom_tests.rs"]
mod tests;
