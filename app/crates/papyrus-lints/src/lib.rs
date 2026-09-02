//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod argument_naming;
pub mod argument_types;
pub mod chain_whitespace;
pub mod comma_spacing;
pub mod config;
pub mod cyclomatic_complexity;
mod disable_comments;
pub mod division_by_zero;
pub mod empty_body;
pub mod exclamation_spacing;
pub mod explicit_return;
pub mod float_int_conversion;
pub mod forbidden_functions;
pub mod formid_hex_notation;
pub mod fragment_code;
pub mod function_override;
pub mod goto_state;
pub mod identifier_casing;
pub mod indentation;
pub mod local_variable_shadowing;
pub mod magic_numbers;
pub mod named_arguments;
pub mod none_form_usage;
pub mod numeric_comparison;
pub mod operator_spacing;
pub mod parameter_reassignment;
pub mod property_sorting;
pub mod return_types;
pub mod semicolon;
pub mod short_wait_interval;
pub mod slow_functions;
pub mod state_count;
pub mod state_function_signature;
pub mod static_condition;
pub mod strict_boolean;
pub mod trailing_whitespace;
pub mod type_casing;
pub mod unchecked_cast;
pub mod unchecked_form_parameter;
pub mod unreachable_statement;
pub mod unresolved_script;
pub mod unused_disable;
pub mod unused_getter;
pub mod unused_local_variable;
pub mod unused_property;
pub mod useless_downcast;
pub mod variable_used_before_assignment;

/// Every rule id [`lint`]/[`lint_with_external_arguments`] can report,
/// matched against `; @disable <rule-id>` directives (see
/// [`disable_comments`]) and validated against by callers (e.g. the CLI's
/// `fix --type <rule-id>`) that need to tell an unknown rule id apart from
/// a known one with no automatic fix (see [`FIXABLE_RULE_IDS`]).
pub const KNOWN_RULE_IDS: &[&str] = &[
    trailing_whitespace::RULE,
    comma_spacing::RULE,
    forbidden_functions::RULE,
    formid_hex_notation::RULE,
    slow_functions::RULE,
    unused_getter::RULE,
    unused_property::RULE,
    semicolon::RULE,
    float_int_conversion::RULE,
    strict_boolean::RULE,
    argument_types::RULE,
    return_types::RULE,
    function_override::RULE,
    argument_naming::RULE,
    state_function_signature::RULE,
    numeric_comparison::RULE,
    indentation::RULE,
    cyclomatic_complexity::RULE,
    unreachable_statement::RULE,
    static_condition::RULE,
    division_by_zero::RULE,
    empty_body::RULE,
    unused_local_variable::RULE,
    none_form_usage::RULE,
    local_variable_shadowing::RULE,
    parameter_reassignment::RULE,
    chain_whitespace::RULE,
    exclamation_spacing::RULE,
    identifier_casing::RULE,
    type_casing::RULE,
    named_arguments::RULE,
    operator_spacing::RULE,
    property_sorting::RULE,
    explicit_return::RULE,
    unchecked_form_parameter::RULE,
    unchecked_cast::RULE,
    useless_downcast::RULE,
    unresolved_script::RULE,
    short_wait_interval::RULE,
    goto_state::RULE,
    state_count::TOO_MANY_STATES_RULE,
    state_count::MULTIPLE_AUTO_STATES_RULE,
    "conflicting-script-versions",
    unused_disable::RULE,
    magic_numbers::RULE,
    variable_used_before_assignment::RULE,
];

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

impl Diagnostic {
    /// The severity level tagged onto the front of [`Self::message`] (e.g.
    /// `"[warning] ..."`), matching the `^\[(error|warning|info)\]`
    /// convention the frontend's `levelOf` parses the same messages with.
    /// Every built-in lint tags one. Untagged diagnostics are classified as
    /// errors so every diagnostic has a level and an accidentally omitted tag
    /// cannot make a potentially serious finding less visible.
    pub fn level(&self) -> &'static str {
        if self.message.starts_with("[error]") {
            "error"
        } else if self.message.starts_with("[warning]") {
            "warning"
        } else if self.message.starts_with("[info]") {
            "info"
        } else {
            "error"
        }
    }
}

/// Runs every lint rule against `source` and returns all diagnostics found.
///
/// `config` carries the project's lint configuration (see [`Config`]), read
/// from its YAML config file (and, in the desktop app, kept in sync with
/// its UI). It selects the "Semicolon at end of line" policy and the
/// "Formatting checks"/"Indentation" style checked here.
///
/// The "Argument type check", "Return type check", and "Useless downcast"
/// lints only resolve object-type subtyping through scripts declared in
/// `source` itself this way; see [`lint_with_external_arguments`] to also
/// resolve it through other scripts' `Extends` chains. The "Inherited
/// function override" lint can never find anything to flag this way, since
/// it always needs to resolve `source`'s own `Extends` chain.
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
/// "Argument type check" lint can check those call sites too, so the
/// "Return type check" lint accepts a returned value whose script extends
/// (directly or transitively) the declared return type, so the
/// "Inherited function override" lint can resolve `source`'s own `Extends`
/// chain to flag a function that overrides an inherited one, so the
/// "Argument naming consistency" lint can compare an overriding function's
/// parameter names against the inherited declaration's, so the "Total
/// named state count"/"Multiple Auto states" lint pair can tally `State`s
/// declared anywhere in `source`'s ancestry alongside its own, and so the
/// "Useless downcast" lint recognizes a cast to an ancestor of the value's
/// script (not just an exact-type match). See
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
    if rules.formid_hex_notation {
        diagnostics.extend(formid_hex_notation::check(source));
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
        diagnostics.extend(strict_boolean::check(source, config.bool_like_int));
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
    if rules.unresolved_script {
        diagnostics.extend(unresolved_script::check_with(source, external));
    }
    if rules.local_variable_shadowing {
        diagnostics.extend(local_variable_shadowing::check_with(source, external));
    }
    if rules.parameter_reassignment {
        diagnostics.extend(parameter_reassignment::check(source));
    }
    if rules.function_override {
        diagnostics.extend(function_override::check_with(source, external));
    }
    if rules.argument_naming {
        diagnostics.extend(argument_naming::check_with(source, external));
    }
    if rules.state_function_signature {
        diagnostics.extend(state_function_signature::check(source));
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
    if rules.division_by_zero {
        diagnostics.extend(division_by_zero::check(source));
    }
    if rules.empty_body {
        diagnostics.extend(empty_body::check(source));
    }
    if rules.unused_local_variable {
        diagnostics.extend(unused_local_variable::check(source));
    }
    if rules.variable_used_before_assignment {
        diagnostics.extend(variable_used_before_assignment::check(source));
    }
    if rules.none_form_usage {
        diagnostics.extend(none_form_usage::check(source));
    }
    if rules.chain_whitespace {
        diagnostics.extend(chain_whitespace::check(source));
    }
    if rules.exclamation_spacing {
        diagnostics.extend(exclamation_spacing::check(source));
    }
    if rules.operator_spacing {
        diagnostics.extend(operator_spacing::check(source));
    }
    if rules.named_arguments {
        diagnostics.extend(named_arguments::check(source, config.named_arguments));
    }
    if rules.identifier_casing {
        diagnostics.extend(identifier_casing::check(source, config.identifier_casing));
    }
    if rules.type_casing {
        diagnostics.extend(type_casing::check(source, config.type_casing));
    }
    if rules.property_sorting {
        diagnostics.extend(property_sorting::check(source));
    }
    if rules.explicit_return {
        diagnostics.extend(explicit_return::check(source));
    }
    if rules.unchecked_form_parameter {
        diagnostics.extend(unchecked_form_parameter::check(source));
    }
    if rules.unchecked_cast {
        diagnostics.extend(unchecked_cast::check(source));
    }
    if rules.useless_downcast {
        diagnostics.extend(useless_downcast::check_with(source, external));
    }
    if rules.short_wait_interval {
        diagnostics.extend(short_wait_interval::check(source, config.min_wait_interval));
    }
    if rules.goto_state {
        diagnostics.extend(goto_state::check_with(source, external));
    }
    if rules.too_many_states {
        diagnostics.extend(state_count::check_too_many_states_with(source, external));
    }
    if rules.multiple_auto_states {
        diagnostics.extend(state_count::check_multiple_auto_states_with(
            source, external,
        ));
    }
    if rules.magic_numbers {
        diagnostics.extend(magic_numbers::check(source, config.magic_numbers));
    }
    let disables = disable_comments::Disables::scan(source);
    let unused_disables = rules
        .unused_disable
        .then(|| unused_disable::check(&disables, &diagnostics, KNOWN_RULE_IDS));
    diagnostics.retain(|diagnostic| !disables.is_disabled(diagnostic.line, diagnostic.rule));
    diagnostics.extend(unused_disables.into_iter().flatten());
    diagnostics
}

/// Rule ids with an automatic fix, in the order [`repair`] applies them.
/// Every other id in [`KNOWN_RULE_IDS`] can only be reported, never fixed.
pub const FIXABLE_RULE_IDS: &[&str] = &[
    identifier_casing::RULE,
    semicolon::RULE,
    indentation::RULE,
    property_sorting::RULE,
    comma_spacing::RULE,
    chain_whitespace::RULE,
    exclamation_spacing::RULE,
    operator_spacing::RULE,
    type_casing::RULE,
    trailing_whitespace::RULE,
];

/// Applies every automatic fix to `source`, including the semicolon and
/// indentation style selected by `config`, and returns the repaired text.
/// A ruleset disabled via `config.rules` has its fix skipped too.
pub fn repair(source: &str, config: &Config) -> String {
    repair_filtered(source, config, None)
}

/// Like [`repair`], but when `rule_filter` is `Some(rule)`, skips every fix
/// except the one whose rule id (see [`FIXABLE_RULE_IDS`]) equals `rule`.
/// `rule_filter` of `None` behaves exactly like [`repair`]. This lets a
/// caller (e.g. the CLI's `fix --type <rule-id>`) apply just one automatic
/// fix without disabling every other rule in `config` (which would also
/// suppress their diagnostics from the report).
pub fn repair_filtered(source: &str, config: &Config, rule_filter: Option<&str>) -> String {
    let rules = &config.rules;
    let applies = |rule: &str| rule_filter.is_none_or(|filter| filter == rule);
    let source = if rules.identifier_casing && applies(identifier_casing::RULE) {
        identifier_casing::repair(source, config.identifier_casing)
    } else {
        source.to_string()
    };
    let source = if rules.semicolon && applies(semicolon::RULE) {
        semicolon::repair(&source, config.semicolon_style())
    } else {
        source
    };
    let source = if rules.indentation && applies(indentation::RULE) {
        indentation::repair(&source, config.indentation_unit())
    } else {
        source
    };
    let source = if rules.property_sorting && applies(property_sorting::RULE) {
        property_sorting::repair(&source)
    } else {
        source
    };
    let source = if rules.comma_spacing && applies(comma_spacing::RULE) {
        comma_spacing::repair(&source)
    } else {
        source
    };
    let source = if rules.chain_whitespace && applies(chain_whitespace::RULE) {
        chain_whitespace::repair(&source)
    } else {
        source
    };
    let source = if rules.exclamation_spacing && applies(exclamation_spacing::RULE) {
        exclamation_spacing::repair(&source)
    } else {
        source
    };
    let source = if rules.operator_spacing && applies(operator_spacing::RULE) {
        operator_spacing::repair(&source)
    } else {
        source
    };
    let source = if rules.type_casing && applies(type_casing::RULE) {
        type_casing::repair(&source, config.type_casing)
    } else {
        source
    };
    if rules.trailing_whitespace && applies(trailing_whitespace::RULE) {
        trailing_whitespace::repair(&source)
    } else {
        source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_level_parses_the_leading_tag() {
        let tagged = |message: &str| Diagnostic {
            line: 1,
            column: 1,
            message: message.to_string(),
            rule: "some-rule",
        };

        assert_eq!(tagged("[error] boom").level(), "error");
        assert_eq!(tagged("[warning] hmm").level(), "warning");
        assert_eq!(tagged("[info] fyi").level(), "info");
        assert_eq!(tagged("No recognized level prefix here").level(), "error");
    }

    #[test]
    fn combined_repair_applies_comma_spacing_and_trailing_whitespace() {
        let config = Config::default();
        let repaired = repair("Call(1,2)  \r\n", &config);

        assert_eq!(repaired, "Call(1, 2)\r\n");
        assert!(lint(&repaired, &config).is_empty());
    }

    #[test]
    fn combined_repair_closes_whitespace_interrupting_a_chain() {
        let config = Config::default();
        let repaired = repair("SomeProperty . DoThing() .Other()  \r\n", &config);

        assert_eq!(repaired, "SomeProperty.DoThing().Other()\r\n");
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
    fn unused_disable_defaults_to_off() {
        let diagnostics = lint("Foo(1, 2) ; @disable made-up-rule\n", &Config::default());

        assert!(diagnostics.iter().all(|d| d.rule != unused_disable::RULE));
    }

    #[test]
    fn unused_disable_reports_unknown_and_untriggered_rule_ids() {
        let config = Config {
            rules: config::Rules {
                unused_disable: true,
                ..config::Rules::default()
            },
            ..Config::default()
        };
        let source = "Foo(1,2) ; @disable comma-spacing, made-up, float-to-int\n";

        let diagnostics = lint(source, &config);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.rule == unused_disable::RULE)
            .collect();

        assert_eq!(unused.len(), 2);
        assert!(unused
            .iter()
            .any(|d| d.message.contains("made-up") && d.message.contains("unknown")));
        assert!(unused
            .iter()
            .any(|d| d.message.contains("float-to-int") && d.message.contains("does not produce")));
        assert!(unused.iter().all(|d| !d.message.contains("comma-spacing")));
    }

    #[test]
    fn unused_disable_reports_a_bare_directive_on_a_clean_line() {
        let config = Config {
            rules: config::Rules {
                unused_disable: true,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        let diagnostics = lint("; @disable\n", &config);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, unused_disable::RULE);
        assert_eq!(diagnostics[0].column, 3);
    }

    #[test]
    fn unused_disable_columns_count_characters_instead_of_utf8_bytes() {
        let config = Config {
            rules: config::Rules {
                unused_disable: true,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        let diagnostics = lint("String value = \"é\" ; @disable unknown\n", &config);
        let diagnostic = diagnostics
            .iter()
            .find(|d| d.rule == unused_disable::RULE)
            .unwrap();

        assert_eq!(diagnostic.column, 31);
    }

    #[test]
    fn repair_filtered_applies_only_the_named_rule() {
        let config = Config::default();
        let source = "Call(1,2)  \r\n";

        let comma_only = repair_filtered(source, &config, Some(comma_spacing::RULE));
        assert_eq!(comma_only, "Call(1, 2)  \r\n");

        let whitespace_only = repair_filtered(source, &config, Some(trailing_whitespace::RULE));
        assert_eq!(whitespace_only, "Call(1,2)\r\n");

        assert_eq!(
            repair_filtered(source, &config, None),
            repair(source, &config)
        );
    }

    #[test]
    fn repair_filtered_matches_nothing_for_an_unknown_rule_id() {
        let config = Config::default();
        let source = "Call(1,2)  \r\n";

        assert_eq!(
            repair_filtered(source, &config, Some("made-up-rule")),
            source
        );
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
    fn repair_skips_chain_whitespace_fix_when_disabled() {
        let source = "SomeProperty . DoThing()\n";
        let config = Config {
            rules: config::Rules {
                chain_whitespace: false,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        assert_eq!(repair(source, &config), source);
    }

    #[test]
    fn repair_skips_exclamation_spacing_fix_when_disabled() {
        let source = "If !bReady\nEndIf\n";
        let config = Config {
            rules: config::Rules {
                exclamation_spacing: false,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        assert_eq!(repair(source, &config), source);
    }

    #[test]
    fn combined_repair_fixes_exclamation_spacing() {
        let config = Config::default();
        let repaired = repair("If !bReady\nEndIf\n", &config);

        assert_eq!(repaired, "If ! bReady\nEndIf\n");
        assert!(lint(&repaired, &config).is_empty());
    }

    #[test]
    fn combined_repair_fixes_configured_type_casing() {
        let config = Config::default();
        let repaired = repair("ScriptName myScript\n", &config);

        assert_eq!(repaired, "ScriptName MyScript\n");
        assert!(lint(&repaired, &config).is_empty());
    }

    #[test]
    fn repair_skips_type_casing_fix_when_disabled() {
        let source = "ScriptName myScript\n";
        let config = Config {
            rules: config::Rules {
                type_casing: false,
                ..config::Rules::default()
            },
            ..Config::default()
        };

        assert_eq!(repair(source, &config), source);
    }

    #[test]
    fn combined_repair_does_not_fight_trailing_whitespace_over_a_line_ending_negation() {
        // A `!` with nothing but a line ending after it would need a
        // trailing space to satisfy exclamation-spacing, but the trailing
        // whitespace fix runs after it and would strip that space right
        // back off; exclamation-spacing leaves it alone rather than
        // fighting that fix on every `repair()` call.
        let config = Config::default();
        let source = "If !\nEndIf\n";
        let repaired = repair(source, &config);

        assert_eq!(repaired, source);
        assert!(repair(&repaired, &config) == repaired);
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

    /// `lint_with_external_arguments` gates each ruleset behind its own `if
    /// rules.<field>` check (see [`config::Rules`]). This walks every
    /// ruleset except `function_override`, one at a time, confirming that:
    /// (a) its own flag being on lets it fire on a source crafted to
    /// trigger it, and (b) flipping only that flag off suppresses that
    /// rule's diagnostics. `function_override` can never fire through bare
    /// `lint()` (it always needs an `external` resolver for the script's
    /// `Extends` chain), so its gate is exercised separately by
    /// `function_override_flag_gates_only_its_own_lint`, below — a gate
    /// accidentally wired to the wrong `Rules` field, or hardcoded to
    /// always run, would fail
    /// here even though it wouldn't fail any other existing test.
    #[test]
    fn each_rule_flag_gates_only_its_own_lint() {
        let many_states_source: String = {
            let mut source = "ScriptName Example\n\n".to_string();
            for index in 0..128 {
                source.push_str(&format!("State State{index}\nEndState\n"));
            }
            source
        };
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
                "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
                variable_used_before_assignment::RULE,
                Config::default(),
                config_with(|c| c.rules.variable_used_before_assignment = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\n    Debug.Trace(MyValue)\nEndFunction\n",
                local_variable_shadowing::RULE,
                Config::default(),
                config_with(|c| c.rules.local_variable_shadowing = false),
            ),
            (
                "SomeProperty . DoThing()\n",
                chain_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.chain_whitespace = false),
            ),
            (
                "If !bReady\nEndIf\n",
                exclamation_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.exclamation_spacing = false),
            ),
            (
                "If a==b\nEndIf\n",
                operator_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.operator_spacing = false),
            ),
            (
                "ScriptName Example\n\nInt Property bad_name = 1 Auto\n",
                identifier_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.identifier_casing = false),
            ),
            (
                "ScriptName myExample\n",
                type_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.type_casing = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
                named_arguments::RULE,
                config_with(|c| c.named_arguments = named_arguments::NamedArguments::Always),
                config_with(|c| {
                    c.named_arguments = named_arguments::NamedArguments::Always;
                    c.rules.named_arguments = false;
                }),
            ),
            (
                "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
                property_sorting::RULE,
                config_with(|c| c.rules.property_sorting = true),
                config_with(|c| c.rules.property_sorting = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Int i = 1\nEndFunction\n",
                explicit_return::RULE,
                Config::default(),
                config_with(|c| c.rules.explicit_return = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
                unchecked_form_parameter::RULE,
                config_with(|c| c.rules.unchecked_form_parameter = true),
                config_with(|c| c.rules.unchecked_form_parameter = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).GetActorValue(\"Health\")\nEndFunction\n",
                unchecked_cast::RULE,
                Config::default(),
                config_with(|c| c.rules.unchecked_cast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as Actor)\nEndFunction\n",
                useless_downcast::RULE,
                Config::default(),
                config_with(|c| c.rules.useless_downcast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / 0\nEndFunction\n",
                division_by_zero::RULE,
                Config::default(),
                config_with(|c| c.rules.division_by_zero = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
                empty_body::RULE,
                Config::default(),
                config_with(|c| c.rules.empty_body = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.01)\nEndFunction\n",
                short_wait_interval::RULE,
                Config::default(),
                config_with(|c| c.rules.short_wait_interval = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
                formid_hex_notation::RULE,
                Config::default(),
                config_with(|c| c.rules.formid_hex_notation = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n",
                state_function_signature::RULE,
                Config::default(),
                config_with(|c| c.rules.state_function_signature = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
                goto_state::RULE,
                Config::default(),
                config_with(|c| c.rules.goto_state = false),
            ),
            (
                many_states_source.as_str(),
                state_count::TOO_MANY_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.too_many_states = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
                magic_numbers::RULE,
                config_with(|c| c.rules.magic_numbers = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n",
                state_count::MULTIPLE_AUTO_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.multiple_auto_states = false),
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

    struct FakeExternalWithParentFunction;

    impl argument_types::ExternalSignatures for FakeExternalWithParentFunction {
        fn lookup(
            &mut self,
            type_name: &str,
            function_name: &str,
        ) -> Option<Vec<argument_types::ParamInfo>> {
            if type_name.eq_ignore_ascii_case("ParentScript")
                && function_name.eq_ignore_ascii_case("DoThing")
            {
                Some(Vec::new())
            } else {
                None
            }
        }
    }

    /// See the note on `each_rule_flag_gates_only_its_own_lint` above:
    /// `function_override` needs `lint_with_external_arguments`'s
    /// `external` resolver to ever fire, so its own `rules.function_override`
    /// gate is checked here instead of in that loop.
    #[test]
    fn function_override_flag_gates_only_its_own_lint() {
        let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n";

        let enabled = lint_with_external_arguments(
            source,
            &Config::default(),
            &mut FakeExternalWithParentFunction,
        );
        assert!(enabled.iter().any(|d| d.rule == function_override::RULE));

        let disabled_config = config_with(|c| c.rules.function_override = false);
        let disabled = lint_with_external_arguments(
            source,
            &disabled_config,
            &mut FakeExternalWithParentFunction,
        );
        assert!(disabled.iter().all(|d| d.rule != function_override::RULE));
    }

    struct FakeExternalWithMissingScript;

    impl argument_types::ExternalSignatures for FakeExternalWithMissingScript {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<argument_types::ParamInfo>> {
            None
        }

        fn script_exists(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("KnownScript")
        }
    }

    /// Like `function_override_flag_gates_only_its_own_lint` above:
    /// `unresolved_script` also needs `lint_with_external_arguments`'s
    /// `external` resolver to ever fire, so its own
    /// `rules.unresolved_script` gate is checked here instead of in the
    /// main loop.
    #[test]
    fn unresolved_script_flag_gates_only_its_own_lint() {
        let source =
            "ScriptName Example\n\nFunction Test()\n    MissingScript.DoThing()\nEndFunction\n";

        let enabled = lint_with_external_arguments(
            source,
            &Config::default(),
            &mut FakeExternalWithMissingScript,
        );
        assert!(enabled.iter().any(|d| d.rule == unresolved_script::RULE));

        let disabled_config = config_with(|c| c.rules.unresolved_script = false);
        let disabled = lint_with_external_arguments(
            source,
            &disabled_config,
            &mut FakeExternalWithMissingScript,
        );
        assert!(disabled.iter().all(|d| d.rule != unresolved_script::RULE));
    }

    struct FakeExternalWithRenamedParentParam;

    impl argument_types::ExternalSignatures for FakeExternalWithRenamedParentParam {
        fn lookup(
            &mut self,
            type_name: &str,
            function_name: &str,
        ) -> Option<Vec<argument_types::ParamInfo>> {
            if type_name.eq_ignore_ascii_case("ParentScript")
                && function_name.eq_ignore_ascii_case("DoThing")
            {
                Some(vec![argument_types::ParamInfo {
                    name: "akTarget".to_string(),
                    type_name: papyrus_parser::ast::TypeName {
                        name: "ObjectReference".to_string(),
                        is_array: false,
                    },
                }])
            } else {
                None
            }
        }
    }

    /// Like `function_override_flag_gates_only_its_own_lint` above:
    /// `argument_naming` also needs `lint_with_external_arguments`'s
    /// `external` resolver to ever fire, so its own `rules.argument_naming`
    /// gate is checked here instead of in the main loop.
    #[test]
    fn argument_naming_flag_gates_only_its_own_lint() {
        let source =
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n";

        let enabled = lint_with_external_arguments(
            source,
            &Config::default(),
            &mut FakeExternalWithRenamedParentParam,
        );
        assert!(enabled.iter().any(|d| d.rule == argument_naming::RULE));

        let disabled_config = config_with(|c| c.rules.argument_naming = false);
        let disabled = lint_with_external_arguments(
            source,
            &disabled_config,
            &mut FakeExternalWithRenamedParentParam,
        );
        assert!(disabled.iter().all(|d| d.rule != argument_naming::RULE));
    }
}
