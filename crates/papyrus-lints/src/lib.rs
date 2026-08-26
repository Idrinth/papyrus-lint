//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod argument_types;
pub mod comma_spacing;
pub mod config;
pub mod forbidden_functions;
pub mod indentation;
pub mod semicolon;
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
///
/// The "Argument type check" lint only checks calls to functions declared
/// in `source` itself this way; see [`lint_with_external_arguments`] to
/// also check calls to functions declared on other scripts.
pub fn lint(source: &str, config: &Config) -> Vec<Diagnostic> {
    lint_with_external_arguments(source, config, &mut argument_types::NoExternalSignatures)
}

/// Like [`lint`], but resolves calls to functions declared on other
/// scripts (e.g. `SomeProperty.DoThing(...)`) through `external`, so the
/// "Argument type check" lint can check those call sites too. See
/// [`argument_types::ExternalSignatures`].
pub fn lint_with_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    _config: &Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    let mut diagnostics = trailing_whitespace::check(source);
    diagnostics.extend(comma_spacing::check(source));
    diagnostics.extend(forbidden_functions::check(source));
    diagnostics.extend(unused_getter::check(source));
    diagnostics.extend(argument_types::check_with(source, external));
    diagnostics
}

/// Applies every automatic fix to `source` and returns the repaired text.
///
/// See [`lint`] for `config`.
pub fn repair(source: &str, indentation: indentation::Indentation, _config: &Config) -> String {
    let source = indentation::repair(source, indentation);
    let source = comma_spacing::repair(&source);
    trailing_whitespace::repair(&source)
}

/// Runs every lint, including the configured semicolon rule.
pub fn lint_with_semicolons(
    source: &str,
    style: semicolon::Style,
    config: &Config,
) -> Vec<Diagnostic> {
    lint_with_semicolons_and_external_arguments(
        source,
        style,
        config,
        &mut argument_types::NoExternalSignatures,
    )
}

/// Combines [`lint_with_semicolons`] and [`lint_with_external_arguments`]:
/// every lint, including the configured semicolon rule, with the
/// "Argument type check" lint additionally resolving calls to other
/// scripts' functions through `external`.
pub fn lint_with_semicolons_and_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    style: semicolon::Style,
    config: &Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    let mut diagnostics = lint_with_external_arguments(source, config, external);
    diagnostics.extend(semicolon::check(source, style));
    diagnostics
}

/// Applies every automatic fix, including the configured semicolon fix.
pub fn repair_with_semicolons(
    source: &str,
    style: semicolon::Style,
    indentation: indentation::Indentation,
    config: &Config,
) -> String {
    let repaired = semicolon::repair(source, style);
    repair(&repaired, indentation, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_repair_applies_comma_spacing_and_trailing_whitespace() {
        let config = Config::default();
        let repaired = repair("Call(1,2)  \r\n", indentation::Indentation::Tabs, &config);

        assert_eq!(repaired, "Call(1, 2)\r\n");
        assert!(lint(&repaired, &config).is_empty());
    }
}
