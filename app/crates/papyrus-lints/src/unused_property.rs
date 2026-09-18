//! Flags script properties that are declared but never referenced.
//!
//! Like the other lints in this crate, this works on lexer tokens rather
//! than the parsed AST, so it still runs on scripts that don't parse
//! cleanly. Properties are matched by name alone (case-insensitively, as
//! Papyrus identifiers are), so a property that shares its name with a
//! member on some other object (e.g. `akRef.Foo` where `Foo` isn't this
//! script's property) is treated as used; this only produces false
//! negatives, never false positives.

use crate::Diagnostic;
use papyrus_parser::token::{Keyword, Token, TokenKind};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-property";

/// Checks `source` for `Property` declarations whose name is never used
/// anywhere else in the script. Flagged as a `[warning]`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, ast, config, external);

    let Some(tokens) = tokens else {
        return Vec::new();
    };

    property_declarations(tokens)
        .into_iter()
        .filter(|decl| !is_used_elsewhere(tokens, decl))
        .map(|decl| Diagnostic {
            line: decl.token.line,
            column: decl.token.col,
            message: format!(
                "[warning] Property '{}' is declared but never used",
                decl.name
            ),
            rule: RULE,
        })
        .collect()
}

struct PropertyDecl<'a> {
    name: &'a str,
    /// Index of the property's name token in `tokens`, excluded when
    /// searching for uses so the declaration doesn't count as a use.
    index: usize,
    token: &'a Token,
}

/// Finds every `Type PropertyName ... Property` declaration, matching the
/// grammar in `papyrus_parser::parser::Parser::parse_property` (an
/// identifier type name, with an optional `[]` array suffix, followed by
/// the `Property` keyword and the property's name).
fn property_declarations(tokens: &[Token]) -> Vec<PropertyDecl<'_>> {
    let mut decls = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Keyword(Keyword::Property)) {
            continue;
        }
        if !preceded_by_type_name(tokens, index) {
            continue;
        }
        let Some(name_token) = tokens.get(index + 1) else {
            continue;
        };
        let TokenKind::Identifier(name) = &name_token.kind else {
            continue;
        };

        decls.push(PropertyDecl {
            name,
            index: index + 1,
            token: name_token,
        });
    }

    decls
}

fn preceded_by_type_name(tokens: &[Token], property_index: usize) -> bool {
    if property_index == 0 {
        return false;
    }

    if matches!(tokens[property_index - 1].kind, TokenKind::Identifier(_)) {
        return true;
    }

    // An array type name: `Identifier [ ] Property`.
    property_index >= 3
        && matches!(tokens[property_index - 1].kind, TokenKind::RBracket)
        && matches!(tokens[property_index - 2].kind, TokenKind::LBracket)
        && matches!(tokens[property_index - 3].kind, TokenKind::Identifier(_))
}

fn is_used_elsewhere(tokens: &[Token], decl: &PropertyDecl) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        index != decl.index
            && matches!(&token.kind, TokenKind::Identifier(candidate) if candidate.eq_ignore_ascii_case(decl.name))
    })
}

#[cfg(test)]
#[path = "unused_property_tests.rs"]
mod tests;
