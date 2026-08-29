//! Lint configuration, read from a project's YAML config file and passed
//! to every check/fix job (see [`crate::lint`] and [`crate::repair`]).
//!
//! A config file only needs to set the keys it wants to override — any
//! key it omits falls back to the default shown below:
//!
//! ```yaml
//! semicolon: false
//! indentation: tab
//! indentation_width: 4
//! cyclomatic_complexity_warning: 10
//! cyclomatic_complexity_error: 20
//! type_casing: PascalCase
//! fail_on_warning: false
//! fail_on_info: false
//! rules:
//!   trailing_whitespace: true
//!   comma_spacing: true
//!   forbidden_functions: true
//!   slow_functions: true
//!   unused_getter: true
//!   unused_property: true
//!   semicolon: true
//!   float_int_conversion: true
//!   strict_boolean: true
//!   argument_types: true
//!   return_types: true
//!   function_override: true
//!   numeric_comparison: true
//!   indentation: true
//!   cyclomatic_complexity: true
//!   unreachable_statement: true
//!   static_condition: true
//!   unused_local_variable: true
//!   none_form_usage: true
//!   local_variable_shadowing: true
//!   chain_whitespace: true
//!   type_casing: true
//! ```
//!
//! Every entry under `rules` is enabled by default; set one to `false` to
//! disable that lint (and its automatic fix, if it has one) entirely. As
//! with the top-level keys, `rules` and any key within it may be omitted
//! and falls back to `true`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// The indentation style a project expects, for the "Formatting checks"/
/// "Indentation" lint and automatic fix described in README.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Indentation {
    #[default]
    Tab,
    Space,
}

/// Configuration for the lint/fix jobs, deserialized from a project's YAML
/// config file and, in the desktop app, kept in sync with the formatting
/// controls in the UI (loaded on startup, saved back to the file whenever
/// they change). Fields absent from the YAML fall back to their default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Whether lines are required to end in a semicolon (`true`) or must
    /// not (`false`). See the "Semicolon at end of line" lint in
    /// README.md.
    pub semicolon: bool,
    /// The indentation style enforced by the "Formatting checks"/
    /// "Indentation" lint and automatic fix in README.md.
    pub indentation: Indentation,
    /// The number of spaces per indentation level, used only when
    /// `indentation` is [`Indentation::Space`].
    pub indentation_width: usize,
    /// The cyclomatic complexity a function/event can reach before the
    /// "Cyclomatic complexity" lint flags it as a `[warning]`.
    pub cyclomatic_complexity_warning: usize,
    /// The cyclomatic complexity a function/event can reach before the
    /// "Cyclomatic complexity" lint flags it as an `[error]`.
    pub cyclomatic_complexity_error: usize,
    /// The casing convention required of a script's declared type name
    /// (the identifier following `ScriptName`), checked by the "Type name
    /// casing" lint.
    pub type_casing: crate::type_casing::Style,
    /// Whether the CLI (see `papyrus-lint-cli`) treats a `[warning]`-level
    /// diagnostic as a reason to exit non-zero. `false` by default, so a
    /// project only fails a lint run on `[error]`-level (and untagged)
    /// diagnostics unless it opts in. Has no effect on the desktop app,
    /// which always shows every diagnostic regardless of severity.
    pub fail_on_warning: bool,
    /// Like [`Self::fail_on_warning`], but for `[info]`-level diagnostics.
    /// `false` by default.
    pub fail_on_info: bool,
    /// Per-ruleset enable/disable switches. Every ruleset is enabled by
    /// default; see [`Rules`].
    pub rules: Rules,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            semicolon: false,
            indentation: Indentation::default(),
            indentation_width: 4,
            cyclomatic_complexity_warning: 10,
            cyclomatic_complexity_error: 20,
            type_casing: crate::type_casing::Style::default(),
            fail_on_warning: false,
            fail_on_info: false,
            rules: Rules::default(),
        }
    }
}

/// Individual enable/disable switches for each lint ruleset, all `true`
/// (enabled) by default. A ruleset set to `false` here is skipped by both
/// [`crate::lint`]/[`crate::lint_with_external_arguments`] and, for
/// rulesets with an automatic fix, [`crate::repair`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// The "Trailing whitespace" lint/fix.
    pub trailing_whitespace: bool,
    /// The "Space after comma" lint/fix.
    pub comma_spacing: bool,
    /// The "Forbidden/discouraged function usage" lint.
    pub forbidden_functions: bool,
    /// The "Slow function usage" lint.
    pub slow_functions: bool,
    /// The "Getter usage without saving result" lint.
    pub unused_getter: bool,
    /// The "Unused script properties" lint.
    pub unused_property: bool,
    /// The "Semicolon at end of line" lint/fix.
    pub semicolon: bool,
    /// The "Implicit Float-to-Int conversion" lint.
    pub float_int_conversion: bool,
    /// The "Strict boolean check" lint.
    pub strict_boolean: bool,
    /// The "Argument type check" lint.
    pub argument_types: bool,
    /// The "Return type check" lint.
    pub return_types: bool,
    /// The "Inherited function override" lint.
    pub function_override: bool,
    /// The "Strict numeric type check" lint.
    pub numeric_comparison: bool,
    /// The "Formatting checks"/"Indentation" lint/fix.
    pub indentation: bool,
    /// The "Cyclomatic complexity" lint.
    pub cyclomatic_complexity: bool,
    /// The "Unreachable statement" lint.
    pub unreachable_statement: bool,
    /// The "Static condition" lint.
    pub static_condition: bool,
    /// The "Unused or write-only local variables" lint.
    pub unused_local_variable: bool,
    /// The "None used as an existing Form" lint.
    pub none_form_usage: bool,
    /// The "Local variable shadowing" lint.
    pub local_variable_shadowing: bool,
    /// The "Whitespace interrupting property/method chaining" lint/fix.
    pub chain_whitespace: bool,
    /// The "Type name casing" lint.
    pub type_casing: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            trailing_whitespace: true,
            comma_spacing: true,
            forbidden_functions: true,
            slow_functions: true,
            unused_getter: true,
            unused_property: true,
            semicolon: true,
            float_int_conversion: true,
            strict_boolean: true,
            argument_types: true,
            return_types: true,
            function_override: true,
            numeric_comparison: true,
            indentation: true,
            cyclomatic_complexity: true,
            unreachable_statement: true,
            static_condition: true,
            unused_local_variable: true,
            none_form_usage: true,
            local_variable_shadowing: true,
            chain_whitespace: true,
            type_casing: true,
        }
    }
}

impl Config {
    /// The trailing-semicolon policy this configuration selects, for use
    /// with [`crate::semicolon::check`]/[`crate::semicolon::repair`].
    pub fn semicolon_style(&self) -> crate::semicolon::Style {
        if self.semicolon {
            crate::semicolon::Style::Require
        } else {
            crate::semicolon::Style::Forbid
        }
    }

    /// The indentation unit this configuration selects, for use with
    /// [`crate::indentation::check`]/[`crate::indentation::repair`].
    /// `indentation_width` is clamped to `1..=16` to match the range
    /// accepted by the UI.
    pub fn indentation_unit(&self) -> crate::indentation::Indentation {
        match self.indentation {
            Indentation::Tab => crate::indentation::Indentation::Tabs,
            Indentation::Space => {
                crate::indentation::Indentation::Spaces(self.indentation_width.clamp(1, 16))
            }
        }
    }

    /// Whether `diagnostic` should count as a reason for the CLI to exit
    /// non-zero, per [`Self::fail_on_warning`]/[`Self::fail_on_info`]. An
    /// `[error]`-level diagnostic, or one tagged with no level at all (see
    /// [`Diagnostic::level`]), always counts.
    pub fn should_fail_on(&self, diagnostic: &Diagnostic) -> bool {
        match diagnostic.level() {
            Some("warning") => self.fail_on_warning,
            Some("info") => self.fail_on_info,
            _ => true,
        }
    }
}

/// An error parsing a lint config file.
#[derive(Debug)]
pub enum ConfigError {
    Yaml(serde_yaml::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Yaml(err) => write!(f, "failed to parse lint config: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<serde_yaml::Error> for ConfigError {
    fn from(err: serde_yaml::Error) -> Self {
        ConfigError::Yaml(err)
    }
}

/// Parses a YAML config document into a [`Config`]. An empty document
/// (including a missing/empty config file's contents) yields
/// [`Config::default`]; keys the document omits also fall back to their
/// default.
pub fn parse(yaml: &str) -> Result<Config, ConfigError> {
    if yaml.trim().is_empty() {
        return Ok(Config::default());
    }
    Ok(serde_yaml::from_str(yaml)?)
}

/// Serializes a [`Config`] back into the YAML document format read by
/// [`parse`], so the desktop app can persist the formatting selected in
/// its UI.
pub fn to_yaml(config: &Config) -> Result<String, ConfigError> {
    Ok(serde_yaml::to_string(config)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_yields_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
        assert_eq!(parse("   \n").unwrap(), Config::default());
    }

    #[test]
    fn all_rules_default_to_enabled() {
        let config = Config::default();
        assert_eq!(config.rules, Rules::default());
        assert!(config.rules.trailing_whitespace);
        assert!(config.rules.comma_spacing);
        assert!(config.rules.forbidden_functions);
        assert!(config.rules.slow_functions);
        assert!(config.rules.unused_getter);
        assert!(config.rules.unused_property);
        assert!(config.rules.semicolon);
        assert!(config.rules.float_int_conversion);
        assert!(config.rules.strict_boolean);
        assert!(config.rules.argument_types);
        assert!(config.rules.return_types);
        assert!(config.rules.function_override);
        assert!(config.rules.numeric_comparison);
        assert!(config.rules.indentation);
        assert!(config.rules.cyclomatic_complexity);
        assert!(config.rules.unreachable_statement);
        assert!(config.rules.static_condition);
        assert!(config.rules.unused_local_variable);
        assert!(config.rules.none_form_usage);
        assert!(config.rules.local_variable_shadowing);
        assert!(config.rules.chain_whitespace);
        assert!(config.rules.type_casing);
    }

    #[test]
    fn parses_individual_rule_overrides() {
        let config = parse("rules:\n  trailing_whitespace: false\n  indentation: false\n").unwrap();

        assert!(!config.rules.trailing_whitespace);
        assert!(!config.rules.indentation);
        // Omitted rule keys still default to enabled.
        assert!(config.rules.comma_spacing);
        assert!(config.rules.semicolon);
    }

    #[test]
    fn rules_round_trip_through_yaml() {
        let config = Config {
            rules: Rules {
                trailing_whitespace: false,
                argument_types: false,
                ..Rules::default()
            },
            ..Config::default()
        };

        let yaml = to_yaml(&config).unwrap();

        assert_eq!(parse(&yaml).unwrap(), config);
    }

    #[test]
    fn defaults_match_documented_default() {
        let config = Config::default();
        assert!(!config.semicolon);
        assert_eq!(config.indentation, Indentation::Tab);
        assert_eq!(config.indentation_width, 4);
        assert_eq!(config.cyclomatic_complexity_warning, 10);
        assert_eq!(config.cyclomatic_complexity_error, 20);
        assert_eq!(config.type_casing, crate::type_casing::Style::PascalCase);
        assert!(!config.fail_on_warning);
        assert!(!config.fail_on_info);
    }

    #[test]
    fn parses_full_config() {
        let config = parse("semicolon: true\nindentation: space\nindentation_width: 2\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
        assert_eq!(config.indentation_width, 2);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let config = parse("semicolon: true\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Tab);
        assert_eq!(config.indentation_width, 4);

        let config = parse("indentation: space\n").unwrap();
        assert!(!config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
        assert_eq!(config.indentation_width, 4);
    }

    #[test]
    fn parses_cyclomatic_complexity_thresholds() {
        let config =
            parse("cyclomatic_complexity_warning: 5\ncyclomatic_complexity_error: 15\n").unwrap();
        assert_eq!(config.cyclomatic_complexity_warning, 5);
        assert_eq!(config.cyclomatic_complexity_error, 15);
    }

    #[test]
    fn parses_fail_on_flags() {
        let config = parse("fail_on_warning: true\nfail_on_info: true\n").unwrap();
        assert!(config.fail_on_warning);
        assert!(config.fail_on_info);
    }

    #[test]
    fn should_fail_on_honors_fail_on_flags() {
        let error = Diagnostic {
            line: 1,
            column: 1,
            message: "[error] boom".to_string(),
            rule: "some-rule",
        };
        let warning = Diagnostic {
            line: 1,
            column: 1,
            message: "[warning] hmm".to_string(),
            rule: "some-rule",
        };
        let info = Diagnostic {
            line: 1,
            column: 1,
            message: "[info] fyi".to_string(),
            rule: "some-rule",
        };
        let untagged = Diagnostic {
            line: 1,
            column: 1,
            message: "Line contains trailing whitespace".to_string(),
            rule: "some-rule",
        };

        let default_config = Config::default();
        assert!(default_config.should_fail_on(&error));
        assert!(!default_config.should_fail_on(&warning));
        assert!(!default_config.should_fail_on(&info));
        assert!(default_config.should_fail_on(&untagged));

        let opted_in = Config {
            fail_on_warning: true,
            fail_on_info: true,
            ..Config::default()
        };
        assert!(opted_in.should_fail_on(&error));
        assert!(opted_in.should_fail_on(&warning));
        assert!(opted_in.should_fail_on(&info));
        assert!(opted_in.should_fail_on(&untagged));
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert!(parse("semicolon: [this is not a bool\n").is_err());
    }

    #[test]
    fn rejects_unknown_indentation_value() {
        assert!(parse("indentation: eight-spaces\n").is_err());
    }

    #[test]
    fn parses_type_casing_style() {
        let config = parse("type_casing: camelCase\n").unwrap();
        assert_eq!(config.type_casing, crate::type_casing::Style::CamelCase);

        let config = parse("type_casing: UPPERCASE\n").unwrap();
        assert_eq!(config.type_casing, crate::type_casing::Style::Uppercase);
    }

    #[test]
    fn rejects_unknown_type_casing_value() {
        assert!(parse("type_casing: snake_case\n").is_err());
    }

    #[test]
    fn semicolon_style_reflects_semicolon_flag() {
        assert_eq!(
            Config {
                semicolon: true,
                ..Config::default()
            }
            .semicolon_style(),
            crate::semicolon::Style::Require
        );
        assert_eq!(
            Config {
                semicolon: false,
                ..Config::default()
            }
            .semicolon_style(),
            crate::semicolon::Style::Forbid
        );
    }

    #[test]
    fn indentation_unit_reflects_indentation_and_width() {
        let config = Config {
            indentation: Indentation::Tab,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Tabs
        );

        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 2,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(2)
        );
    }

    #[test]
    fn indentation_unit_clamps_width_to_valid_range() {
        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 100,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(16)
        );

        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 0,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(1)
        );
    }

    #[test]
    fn to_yaml_round_trips_through_parse() {
        let config = Config {
            semicolon: true,
            indentation: Indentation::Space,
            indentation_width: 2,
            ..Config::default()
        };

        let yaml = to_yaml(&config).unwrap();

        assert_eq!(parse(&yaml).unwrap(), config);
    }
}
