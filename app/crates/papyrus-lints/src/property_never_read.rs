//! Flags script properties that are written in this script but never read.
//!
//! `unused-property` stays silent when the name is referenced at all, even
//! if every reference is an assignment. This rule is the remaining gap:
//! a write-only property is leftover state or CK plumbing the script no
//! longer consumes. Compound assignments (`+=` and friends) count as
//! writes, not reads, matching the common "incremented counter nobody
//! inspects" case.
//!
//! Matching is by name alone (case-insensitive), so `akRef.Foo` where
//! `Foo` is also this script's property is treated as a use of that
//! property. That only produces false negatives, never false positives.
//! A `; @external` annotation on the property's declaration line opts the
//! property out, for MCM / other-script-consumed values.

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::token::{Keyword, Token, TokenKind};
use std::collections::HashMap;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "property-never-read";

#[derive(Default)]
struct Usage {
    written: bool,
    read: bool,
    external: bool,
}

struct Decl {
    lower: String,
    name: String,
    index: usize,
    line: usize,
    column: usize,
}

#[derive(Default)]
struct Collect {
    store: Store,
    decls: Vec<Decl>,
    usage: HashMap<String, Usage>,
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
        if let TokenKind::CommentAnnotation(name) = &token.kind {
            if name.eq_ignore_ascii_case("external") {
                if let Some(decl) = self.decls.iter().rev().find(|decl| decl.line == token.line) {
                    self.usage.entry(decl.lower.clone()).or_default().external = true;
                }
            }
            return;
        }

        if matches!(token.kind, TokenKind::Keyword(Keyword::Property))
            && preceded_by_type_name(tokens, index)
        {
            if let Some(name_token) = tokens.get(index + 1) {
                if let TokenKind::Identifier(name) = &name_token.kind {
                    let lower = name.to_ascii_lowercase();
                    self.decls.push(Decl {
                        lower: lower.clone(),
                        name: name.clone(),
                        index: index + 1,
                        line: name_token.line,
                        column: name_token.col,
                    });
                    self.usage.entry(lower).or_default();
                }
            }
            return;
        }

        let TokenKind::Identifier(name) = &token.kind else {
            return;
        };
        let lower = name.to_ascii_lowercase();
        if !self.usage.contains_key(&lower) {
            return;
        }
        if self
            .decls
            .iter()
            .any(|decl| decl.index == index && decl.lower == lower)
        {
            return;
        }

        let entry = self.usage.entry(lower).or_default();
        if next_significant(tokens, index).is_some_and(is_assignment) {
            entry.written = true;
        } else {
            entry.read = true;
        }
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for decl in &self.decls {
            let Some(usage) = self.usage.get(&decl.lower) else {
                continue;
            };
            if usage.external || !usage.written || usage.read {
                continue;
            }
            self.store.emit(
                decl.line,
                decl.column,
                format!("[info] Property '{}' is written but never read", decl.name),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for `Property` declarations that are assigned in this
/// script but whose value is never read. Flagged as an `[info]`.
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

fn is_assignment(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Assign
            | TokenKind::PlusAssign
            | TokenKind::MinusAssign
            | TokenKind::StarAssign
            | TokenKind::SlashAssign
            | TokenKind::PercentAssign
    )
}

fn next_significant(tokens: &[Token], index: usize) -> Option<&TokenKind> {
    tokens.get(index + 1..).and_then(|rest| {
        rest.iter()
            .map(|token| &token.kind)
            .find(|kind| {
                !matches!(
                    kind,
                    TokenKind::CommentAnnotation(_) | TokenKind::Newline | TokenKind::Eof
                )
            })
    })
}

fn preceded_by_type_name(tokens: &[Token], property_index: usize) -> bool {
    if property_index == 0 {
        return false;
    }

    if matches!(tokens[property_index - 1].kind, TokenKind::Identifier(_)) {
        return true;
    }

    property_index >= 3
        && matches!(tokens[property_index - 1].kind, TokenKind::RBracket)
        && matches!(tokens[property_index - 2].kind, TokenKind::LBracket)
        && matches!(tokens[property_index - 3].kind, TokenKind::Identifier(_))
}

#[cfg(test)]
#[path = "property_never_read_tests.rs"]
mod tests;
