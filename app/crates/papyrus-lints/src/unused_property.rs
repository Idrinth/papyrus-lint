//! Flags script properties that are declared but never referenced.
//!
//! Like the other lints in this crate, this works on lexer tokens rather
//! than the parsed AST, so it still runs on scripts that don't parse
//! cleanly. Properties are matched by name alone (case-insensitively, as
//! Papyrus identifiers are), so a property that shares its name with a
//! member on some other object (e.g. `akRef.Foo` where `Foo` isn't this
//! script's property) is treated as used; this only produces false
//! negatives, never false positives.

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::token::{Keyword, Token, TokenKind};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-property";

#[derive(Default)]
struct Collect {
    store: Store,
    decls: Vec<(String, String, usize, usize, usize)>,
    uses: Vec<(String, usize)>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        if let TokenKind::Identifier(name) = &token.kind {
            self.uses.push((name.to_ascii_lowercase(), index));
        }
        if !matches!(token.kind, TokenKind::Keyword(Keyword::Property)) {
            return;
        }
        if !preceded_by_type_name(tokens, index) {
            return;
        }
        let Some(name_token) = tokens.get(index + 1) else {
            return;
        };
        let TokenKind::Identifier(name) = &name_token.kind else {
            return;
        };
        self.decls.push((
            name.to_ascii_lowercase(),
            name.clone(),
            index + 1,
            name_token.line,
            name_token.col,
        ));
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for (lower, name, decl_index, line, column) in &self.decls {
            let used = self
                .uses
                .iter()
                .any(|(candidate, index)| index != decl_index && candidate == lower);
            if used {
                continue;
            }
            self.store.emit(
                *line,
                *column,
                format!("[warning] Property '{name}' is declared but never used"),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for `Property` declarations whose name is never used
/// anywhere else in the script. Flagged as a `[warning]`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
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

#[cfg(test)]
#[path = "unused_property_tests.rs"]
mod tests;
