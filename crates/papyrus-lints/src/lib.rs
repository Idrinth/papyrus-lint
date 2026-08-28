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
mod disable_comments;
pub mod float_int_conversion;
pub mod forbidden_functions;
pub mod fragment_code;
pub mod indentation;
pub mod local_variable_shadowing;
pub mod none_form_usage;
pub mod numeric_comparison;
pub mod return_types;
pub mod semicolon;
pub mod slow_functions;
pub mod static_condition;
pub mod strict_boolean;
pub mod trailing_whitespace;
pub mod unreachable_statement;
pub mod unused_getter;
pub mod unused_local_variable;
pub mod unused_property;

use serde::Serialize;

pub use config::Config;

/// A single lint finding, pointing at the 1-indexed line and column it applies to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
    /// The hyphenated id of the rule that raised this diagnostic (e.g.
    /// `"float-to-int"`), matched against `@disable <rule-id>` line-comment
    /// directives (see [`disable_comments`]) to suppress specific lints on
    /// a specific line.
    pub rule: &'static str,
}

/// Runs every lint rule against `source` and returns all diagnostics found.
///
/// `config` carries the project's lint configuration (see [`Config`]), read
/// from its YAML config file (and, in the desktop app, kept in sync with
/// its UI). It selects the "Semicolon at end of line" policy and the
/// "Formatting checks"/"Indentation" style checked here.
///
/// The "Argument type check" and "Return type check" lints only resolve
/// object-type subtyping through scripts declared in `source` itself this
/// way; see [`lint_with_external_arguments`] to also resolve it through
/// other scripts' `Extends` chains.
///
/// A line carrying a trailing `; @disable <rule-id>[, <rule-id>...]`
/// comment (e.g. `action = 1 ; @disable float-to-int`) has diagnostics from
/// the named rule(s) suppressed on that line only; `; @disable` with no
/// rule ids suppresses every lint on that line. See [`disable_comments`]
/// for the rule ids each lint is matched against. This does not affect
/// [`repair`], which still applies its fixes regardless of `@disable`
/// comments.
pub fn lint(source: &str, config: &Config) -> Vec<Diagnostic> {
    lint_with_external_arguments(source, config, &mut argument_types::NoExternalSignatures)
}

/// Like [`lint`], but resolves calls to functions declared on other
/// scripts (e.g. `SomeProperty.DoThing(...)`) through `external`, so the
/// "Argument type check" lint can check those call sites too, and so the
/// "Return type check" lint accepts a returned value whose script extends
/// (directly or transitively) the declared return type. See
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
    if rules.return_types {
        diagnostics.extend(return_types::check_with(source, external));
    }
    if rules.local_variable_shadowing {
        diagnostics.extend(local_variable_shadowing::check_with(source, external));
    }
    if rules.cyclomatic_complexity {
        diagnostics.extend(cyclomatic_complexity::check(
            source,
            config.cyclomatic_complexity_warning,
            config.cyclomatic_complexity_error,
        ));
    }
    if rules.unreachable_statement {
        diagnostics.extend(unreachable_statement::check(source));
    }
    if rules.static_condition {
        diagnostics.extend(static_condition::check(source));
    }
    if rules.unused_local_variable {
        diagnostics.extend(unused_local_variable::check(source));
    }
    if rules.none_form_usage {
        diagnostics.extend(none_form_usage::check(source));
    }
    let disables = disable_comments::Disables::scan(source);
    diagnostics.retain(|diagnostic| !disables.is_disabled(diagnostic.line, diagnostic.rule));
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
    fn lint_honors_disable_comments_on_the_flagged_line_only() {
        let source = "Foo(1,2)   \nBar(3,4)   \n";
        let config = Config::default();
        let baseline = lint(source, &config);
        assert_eq!(baseline.len(), 4);

        let source = "Foo(1,2) ; @disable comma-spacing \nBar(3,4)   \n";
        let diagnostics = lint(source, &config);

        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics
            .iter()
            .all(|d| !(d.line == 1 && d.rule == comma_spacing::RULE)));
        assert!(diagnostics
            .iter()
            .any(|d| d.line == 2 && d.rule == comma_spacing::RULE));
    }

    #[test]
    fn lint_bare_disable_comment_suppresses_every_rule_on_the_line() {
        let source = "Foo(1,2)   ; @disable\n";
        assert!(lint(source, &Config::default()).is_empty());
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

    fn config_with(tweak: impl FnOnce(&mut Config)) -> Config {
        let mut config = Config::default();
        tweak(&mut config);
        config
    }

    /// `lint_with_external_arguments` gates each of the 15 rulesets behind
    /// its own `if rules.<field>` check (see [`config::Rules`]). This walks
    /// every ruleset, one at a time, confirming that: (a) its own flag
    /// being on lets it fire on a source crafted to trigger it, and (b)
    /// flipping only that flag off suppresses that rule's diagnostics.
    /// This is the only test that exercises 13 of the 15 gates (only
    /// `trailing_whitespace` and `comma_spacing` were previously covered,
    /// by `lint_skips_disabled_rules` above) — a gate accidentally wired to
    /// the wrong `Rules` field, or hardcoded to always run, would fail
    /// here even though it wouldn't fail any other existing test.
    #[test]
    fn each_rule_flag_gates_only_its_own_lint() {
        let cases: Vec<(&str, &str, Config, Config)> = vec![
            (
                "ScriptName Example  \n",
                trailing_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.trailing_whitespace = false),
            ),
            (
                "Function Add(Int left,Int right)\n  Use(Add(1,2),3)\nEndFunction\n",
                comma_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.comma_spacing = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
                forbidden_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.forbidden_functions = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
                slow_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.slow_functions = false),
            ),
            (
                "Function Test()\n  GetValue()\nEndFunction\n",
                unused_getter::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_getter = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
                unused_property::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_property = false),
            ),
            (
                // Default config forbids trailing semicolons, so a semicolon here violates it.
                "ScriptName Example\n\nInt value = 1;\n",
                semicolon::RULE,
                Config::default(),
                config_with(|c| c.rules.semicolon = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int x = 1.5\nEndFunction\n",
                float_int_conversion::RULE,
                Config::default(),
                config_with(|c| c.rules.float_int_conversion = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\nEndFunction\n",
                strict_boolean::RULE,
                Config::default(),
                config_with(|c| c.rules.strict_boolean = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(1)\nEndFunction\n",
                argument_types::RULE,
                Config::default(),
                config_with(|c| c.rules.argument_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
                numeric_comparison::RULE,
                Config::default(),
                config_with(|c| c.rules.numeric_comparison = false),
            ),
            (
                // Default config expects tab indentation; this uses spaces instead.
                "Function Run()\n  If ready\nDoThing()\nEndIf\nEndFunction\n",
                indentation::RULE,
                Config::default(),
                config_with(|c| c.rules.indentation = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                cyclomatic_complexity::RULE,
                // The default warning threshold (10) wouldn't flag this trivial
                // function, so lower it — independent of the `rules` flag under test.
                config_with(|c| c.cyclomatic_complexity_warning = 0),
                config_with(|c| {
                    c.cyclomatic_complexity_warning = 0;
                    c.rules.cyclomatic_complexity = false;
                }),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\nEndFunction\n",
                unreachable_statement::RULE,
                Config::default(),
                config_with(|c| c.rules.unreachable_statement = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n",
                return_types::RULE,
                Config::default(),
                config_with(|c| c.rules.return_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                unused_local_variable::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_local_variable = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
                none_form_usage::RULE,
                Config::default(),
                config_with(|c| c.rules.none_form_usage = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\n    Debug.Trace(MyValue)\nEndFunction\n",
                local_variable_shadowing::RULE,
                Config::default(),
                config_with(|c| c.rules.local_variable_shadowing = false),
            ),
        ];

        for (source, rule, enabled_config, disabled_config) in cases {
            let baseline = lint(source, &enabled_config);
            assert!(
                baseline.iter().any(|d| d.rule == rule),
                "expected rule {rule:?} to fire on {source:?}, got {baseline:?}"
            );

            let with_rule_disabled = lint(source, &disabled_config);
            assert!(
                with_rule_disabled.iter().all(|d| d.rule != rule),
                "disabling {rule:?} should suppress its own diagnostics, got {with_rule_disabled:?}"
            );
        }
    }
}
