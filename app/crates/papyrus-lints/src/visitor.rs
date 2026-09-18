//! Per-rule visitors.
//!
//! Each ast/tokens rule exposes a `visitor()` function that returns an
//! [`LintVisitor`] for the tree it would walk. Not yet dispatched from
//! [`crate::registry`]; each rule's `check` remains the live entry.

#![allow(dead_code)] // not dispatched from collect_diagnostics yet

use papyrus_parser::visit::{TokenVisitor, Visitor};

struct NoopAst;

impl Visitor for NoopAst {}

struct NoopTokens;

impl TokenVisitor for NoopTokens {}

/// A rule's visitor, either over the parsed AST or the token stream.
pub enum LintVisitor {
    Ast(Box<dyn Visitor>),
    Tokens(Box<dyn TokenVisitor>),
}

impl LintVisitor {
    pub fn ast() -> Self {
        Self::Ast(Box::new(NoopAst))
    }

    pub fn tokens() -> Self {
        Self::Tokens(Box::new(NoopTokens))
    }
}

#[cfg(test)]
#[path = "visitor_tests.rs"]
mod tests;
