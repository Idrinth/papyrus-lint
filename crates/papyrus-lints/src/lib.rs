//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod forbidden_functions;
pub mod trailing_whitespace;

use serde::Serialize;

/// A single lint finding, pointing at the 1-indexed line and column it applies to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Runs every lint rule against `source` and returns all diagnostics found.
pub fn lint(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = trailing_whitespace::check(source);
    diagnostics.extend(forbidden_functions::check(source));
    diagnostics
}
