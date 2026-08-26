//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod config;
pub mod forbidden_functions;
pub mod indentation;
pub mod trailing_whitespace;
pub mod unused_getter;

use serde::Serialize;

pub use config::Config;

/// A single lint finding, pointing at the 1-indexed line and column it applies to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Runs every lint rule against `source` and returns all diagnostics found.
///
/// `config` carries the project's lint configuration (see [`Config`]), read
/// from its YAML config file. No rule reads it yet — it's threaded through
/// ready for the configurable "Semicolon at end of line" and
/// indentation lints listed as "Planned Lints" in README.md.
pub fn lint(source: &str, _config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = trailing_whitespace::check(source);
    diagnostics.extend(forbidden_functions::check(source));
    diagnostics.extend(unused_getter::check(source));
    diagnostics
}

/// Applies every automatic fix to `source` and returns the repaired text.
pub fn repair(source: &str, indentation: indentation::Indentation) -> String {
    let source = indentation::repair(source, indentation);
    trailing_whitespace::repair(&source)
///
/// See [`lint`] for `config`.
pub fn repair(source: &str, _config: &Config) -> String {
    trailing_whitespace::repair(source)
}
