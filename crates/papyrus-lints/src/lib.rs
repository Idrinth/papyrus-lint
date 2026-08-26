//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod argument_types;
pub mod comma_spacing;
pub mod config;
pub mod cyclomatic_complexity;
pub mod float_int_conversion;
pub mod forbidden_functions;
pub mod indentation;
pub mod numeric_comparison;
pub mod semicolon;
pub mod slow_functions;
pub mod strict_boolean;
pub mod trailing_whitespace;
pub mod unused_getter;
pub mod unused_property;

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
/// its UI). It selects the "Semicolon at end of line" policy and the
/// "Formatting checks"/"Indentation" style checked here.
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
    let rules = &config.rules;
    let mut diagnostics = Vec::new();
    if rules.trailing_whitespace {
        diagnostics.extend(trailing_whitespace::check(source));
    }
    if rules.comma_spacing {
        diagnostics.extend(comma_spacing::check(source));
    }
    if rules.forbidden_functions {
        diagnostics.extend(forbidden_functions::check(source));
    }
    if rules.slow_functions {
        diagnostics.extend(slow_functions::check(source));
    }
    if rules.unused_getter {
        diagnostics.extend(unused_getter::check(source));
    }
    if rules.float_int_conversion {
        diagnostics.extend(float_int_conversion::check(source));
    }
    if rules.unused_property {
        diagnostics.extend(unused_property::check(source));
    }
    if rules.strict_boolean {
        diagnostics.extend(strict_boolean::check(source));
    }
    if rules.numeric_comparison {
        diagnostics.extend(numeric_comparison::check(source));
    }
    if rules.semicolon {
        diagnostics.extend(semicolon::check(source, config.semicolon_style()));
    }
    if rules.indentation {
        diagnostics.extend(indentation::check(source, config.indentation_unit()));
    }
    if rules.argument_types {
        diagnostics.extend(argument_types::check_with(source, external));
    }
    if rules.cyclomatic_complexity {
        diagnostics.extend(cyclomatic_complexity::check(
            source,
            config.cyclomatic_complexity_warning,
            config.cyclomatic_complexity_error,
        ));
    }
    diagnostics
}

/// Applies every automatic fix to `source`, including the semicolon and
/// indentation style selected by `config`, and returns the repaired text.
/// A ruleset disabled via `config.rules` has its fix skipped too.
pub fn repair(source: &str, config: &Config) -> String {
    let rules = &config.rules;
    let source = if rules.semicolon {
        semicolon::repair(source, config.semicolon_style())
    } else {
        source.to_string()
    };
    let source = if rules.indentation {
        indentation::repair(&source, config.indentation_unit())
    } else {
        source
    };
    let source = if rules.comma_spacing {
        comma_spacing::repair(&source)
    } else {
        source
    };
    if rules.trailing_whitespace {
        trailing_whitespace::repair(&source)
    } else {
        source
    }
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
    fn lint_skips_disabled_rules() {
        let source = "Foo(1,2)   \n";
        let config = Config::default();
        let baseline = lint(source, &config);
        assert_eq!(baseline.len(), 2);

        let config = Config {
            rules: config::Rules {
                trailing_whitespace: false,
                comma_spacing: false,
                ..config::Rules::default()
            },
            ..Config::default()
        };
        assert!(lint(source, &config).is_empty());
    }

    #[test]
    fn repair_skips_disabled_rules() {
        let source = "Call(1,2)  \r\n";
        let config = Config {
            rules: config::Rules {
                trailing_whitespace: false,
                comma_spacing: false,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        assert_eq!(repair(source, &config), source);
    }

    #[test]
    fn repair_honors_configured_semicolon_and_indentation_style() {
        let config = Config {
            semicolon: true,
            indentation: config::Indentation::Space,
            indentation_width: 2,
            ..Config::default()
        };
        let source = "Function Run()\nIf ready\nDoThing()\nEndIf\nEndFunction\n";

        let repaired = repair(source, &config);

        assert_eq!(
            repaired,
            "Function Run();\n  If ready;\n    DoThing();\n  EndIf;\nEndFunction;\n"
        );
    }
}
