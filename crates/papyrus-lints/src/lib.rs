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
pub mod strict_boolean;
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
/// from its YAML config file (and, in the desktop app, kept in sync with
/// its UI). It selects the "Semicolon at end of line" policy checked here;
/// the indentation it selects is only relevant to [`repair`], since there
/// is no "one true" indentation to flag as a lint finding.
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
    config: &Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    let mut diagnostics = trailing_whitespace::check(source);
    diagnostics.extend(comma_spacing::check(source));
    diagnostics.extend(forbidden_functions::check(source));
    diagnostics.extend(unused_getter::check(source));
    diagnostics.extend(strict_boolean::check(source));
    diagnostics.extend(semicolon::check(source, config.semicolon_style()));
    diagnostics.extend(argument_types::check_with(source, external));
    diagnostics
}

/// Applies every automatic fix to `source`, including the semicolon and
/// indentation style selected by `config`, and returns the repaired text.
pub fn repair(source: &str, config: &Config) -> String {
    let source = semicolon::repair(source, config.semicolon_style());
    let source = indentation::repair(&source, config.indentation_unit());
    let source = comma_spacing::repair(&source);
    trailing_whitespace::repair(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_repair_applies_comma_spacing_and_trailing_whitespace() {
        let config = Config::default();
        let repaired = repair("Call(1,2)  \r\n", &config);

        assert_eq!(repaired, "Call(1, 2)\r\n");
        assert!(lint(&repaired, &config).is_empty());
    }

    #[test]
    fn repair_honors_configured_semicolon_and_indentation_style() {
        let config = Config {
            semicolon: true,
            indentation: config::Indentation::Space,
            indentation_width: 2,
        };
        let source = "Function Run()\nIf ready\nDoThing()\nEndIf\nEndFunction\n";

        let repaired = repair(source, &config);

        assert_eq!(
            repaired,
            "Function Run();\n  If ready;\n    DoThing();\n  EndIf;\nEndFunction;\n"
        );
    }
}
