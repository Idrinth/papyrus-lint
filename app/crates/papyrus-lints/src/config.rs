//! Lint configuration, deserialized from a project's YAML config file and
//! passed to every check/fix job (see [`crate::lint`] and [`crate::repair`]).
//!
//! Locating, loading, and saving that file is `papyrus-lint-config`'s job;
//! this module owns the settings types themselves. A config file only
//! needs to set the keys it wants to override — any key it omits falls
//! back to the default shown below:
//!
//! ```yaml
//! game: skyrim
//! semicolon: false
//! indentation: tab
//! indentation_width: 4
//! max_line_length: 120
//! identifier_casing: PascalCase
//! cyclomatic_complexity_warning: 10
//! cyclomatic_complexity_error: 20
//! type_casing: PascalCase
//! named_arguments: never
//! min_wait_interval: 0.1
//! magic_numbers: loose
//! fail_on_warning: false
//! fail_on_info: false
//! bool_like_int: true
//! assume_auto_properties_filled: false
//! rules:
//!   trailing_whitespace: true   # one boolean per lint; see [`Rules`]
//! ```
//!
//! Every key under `rules` may be omitted and falls back to
//! [`Rules::default`], which is generated from `shared/rules.json`
//! (`enabled_by_default`, defaulting to `true`). A ruleset set to `false`
//! disables that lint (and its automatic fix, if it has one) entirely.
//! The other [`Config`] fields are generated from
//! `configuration/lint-settings.json` the same way.
//!
//! `assume_auto_properties_filled` (a top-level key, not a `rules` entry)
//! is `false` by default: see [`Config::assume_auto_properties_filled`].

use serde::{Deserialize, Serialize};

use crate::Diagnostic;

pub use crate::magic_numbers::MagicNumbers;
pub use crate::named_arguments::NamedArguments;
pub use crate::type_casing::Style as TypeCasing;
pub use papyrus_lint_globals::Game;

/// The indentation style a project expects, for the "Formatting checks"/
/// "Indentation" lint and automatic fix described in README.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Indentation {
    #[default]
    Tab,
    Space,
}

/// The casing style a project expects declared identifiers (functions,
/// events, properties, states, parameters, and local variables) to use,
/// for the "Identifier casing" lint described in README.md. `ScriptName`
/// itself is never checked, since it must match the script's filename
/// regardless of casing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IdentifierCasing {
    #[serde(rename = "camelCase")]
    CamelCase,
    #[default]
    #[serde(rename = "PascalCase")]
    PascalCase,
    #[serde(rename = "snake_case")]
    SnakeCase,
    #[serde(rename = "CONSTANT_CASE")]
    ConstantCase,
}

impl IdentifierCasing {
    /// The name of this style, matching its YAML value.
    pub fn label(self) -> &'static str {
        match self {
            IdentifierCasing::CamelCase => "camelCase",
            IdentifierCasing::PascalCase => "PascalCase",
            IdentifierCasing::SnakeCase => "snake_case",
            IdentifierCasing::ConstantCase => "CONSTANT_CASE",
        }
    }

    /// Whether `name` conforms to this casing style.
    pub fn matches(self, name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return true;
        };

        match self {
            IdentifierCasing::CamelCase => {
                first.is_ascii_lowercase() && chars.all(|c| c.is_ascii_alphanumeric())
            }
            IdentifierCasing::PascalCase => {
                first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_alphanumeric())
            }
            IdentifierCasing::SnakeCase => {
                first.is_ascii_lowercase()
                    && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            }
            IdentifierCasing::ConstantCase => {
                first.is_ascii_uppercase()
                    && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            }
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/config_struct.rs"));

include!(concat!(env!("OUT_DIR"), "/rules_struct.rs"));

impl Rules {
    /// The hyphenated ids of every enabled rule here, alphabetically
    /// sorted. Property names are mapped to rule ids by replacing
    /// underscores with hyphens (see this struct's own docs above), so
    /// this stays correct without listing each field by hand as new rules
    /// are added. Used by the AI export's compact `enabled_rules` list
    /// (`papyrus-lint-cli`'s `AiReport`/the desktop app's
    /// `formatIssuesForAi`) instead of repeating this struct's full set of
    /// boolean flags on every export.
    pub fn enabled_ids(&self) -> Vec<String> {
        let value = serde_json::to_value(self).expect("Rules always serializes to an object");
        let object = value
            .as_object()
            .expect("Rules serializes as a JSON object");
        let mut ids: Vec<String> = object
            .iter()
            .filter(|(_, enabled)| enabled.as_bool() == Some(true))
            .map(|(name, _)| name.replace('_', "-"))
            .collect();
        ids.sort();
        ids
    }
}

impl Default for Rules {
    fn default() -> Self {
        default_rules()
    }
}

impl Config {
    /// The trailing-semicolon policy this configuration selects, for use
    /// with [`crate::semicolon::check`]/[`crate::semicolon::repair`].
    pub(crate) fn semicolon_style(&self) -> crate::semicolon::Style {
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
    pub(crate) fn indentation_unit(&self) -> crate::indentation::Indentation {
        match self.indentation {
            Indentation::Tab => crate::indentation::Indentation::Tabs,
            Indentation::Space => {
                crate::indentation::Indentation::Spaces(self.indentation_width.clamp(1, 16))
            }
        }
    }

    /// Whether `diagnostic` should count as a reason for the CLI to exit
    /// non-zero, per [`Self::fail_on_warning`]/[`Self::fail_on_info`]. An
    /// `[error]`-level diagnostic always counts. Untagged diagnostics are
    /// classified as errors by [`Diagnostic::level`] and therefore count too.
    pub fn should_fail_on(&self, diagnostic: &Diagnostic) -> bool {
        match diagnostic.level() {
            "warning" => self.fail_on_warning,
            "info" => self.fail_on_info,
            _ => true,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
