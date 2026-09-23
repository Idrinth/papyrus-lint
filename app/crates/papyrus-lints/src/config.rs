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

/// Configuration for the lint/fix jobs, deserialized from a project's YAML
/// config file and, in the desktop app, kept in sync with the formatting
/// controls in the UI (loaded on startup, saved back to the file whenever
/// they change). Fields absent from the YAML fall back to their default.
/// File I/O for that YAML lives in `papyrus-lint-config`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The game whose Papyrus dialect and runtime APIs this project targets.
    /// Defaults to [`Game::Skyrim`] for compatibility with configurations
    /// created before this key existed.
    pub game: Game,
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
    /// The casing style enforced by the "Identifier casing" lint. See
    /// [`IdentifierCasing`].
    pub identifier_casing: IdentifierCasing,
    /// The cyclomatic complexity a function/event can reach before the
    /// "Cyclomatic complexity" lint flags it as a `[warning]`.
    pub cyclomatic_complexity_warning: usize,
    /// The cyclomatic complexity a function/event can reach before the
    /// "Cyclomatic complexity" lint flags it as an `[error]`. A value below
    /// [`Self::cyclomatic_complexity_warning`] is treated as equal to it
    /// instead (see [`crate::cyclomatic_complexity::check`]), since an
    /// `[error]` threshold lower than the `[warning]` one it's supposed to
    /// escalate would otherwise be contradictory.
    pub cyclomatic_complexity_error: usize,
    /// The casing convention required of a script's declared type name
    /// (the identifier following `ScriptName`), checked by the "Type name
    /// casing" lint.
    pub type_casing: TypeCasing,
    /// How strongly the "Prefer named arguments" lint prefers Papyrus's
    /// named-argument call syntax (`func(argB = 1)`) over positional
    /// arguments. See [`NamedArguments`].
    pub named_arguments: NamedArguments,
    /// The interval/duration argument a `Utility.Wait`, `RegisterForUpdate`,
    /// `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
    /// `RegisterForSingleUpdateGameTime` call can go below before the
    /// "Short wait/update interval" lint flags it as a `[warning]`.
    pub min_wait_interval: f64,
    /// Whether the "Magic numbers" lint also checks the interval argument
    /// of a `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/
    /// `RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call
    /// (`strict`), or leaves it unflagged since a hardcoded interval there
    /// is common and usually self-explanatory (`loose`, the default). See
    /// [`MagicNumbers`].
    pub magic_numbers: MagicNumbers,
    /// Whether the CLI (see `papyrus-lint-cli`) treats a `[warning]`-level
    /// diagnostic as a reason to exit non-zero. `false` by default, so a
    /// project only fails a lint run on `[error]`-level (and untagged)
    /// diagnostics unless it opts in. Has no effect on the desktop app,
    /// which always shows every diagnostic regardless of severity.
    pub fail_on_warning: bool,
    /// Like [`Self::fail_on_warning`], but for `[info]`-level diagnostics.
    /// `false` by default.
    pub fail_on_info: bool,
    /// Whether the "Strict boolean check" lint accepts an `Int` literal
    /// `1` or `0` used directly as an `If`/`ElseIf`/`While` condition,
    /// treating it as the common "bool-like" idiom rather than flagging
    /// it. `true` by default. Any other `Int` value (a variable, a
    /// property, or a literal other than `1`/`0`) is still flagged
    /// regardless of this setting.
    pub bool_like_int: bool,
    /// Whether the "None used as an existing Form" lint treats a
    /// script-level `Auto`/`AutoReadOnly` property as already filled in by
    /// the time a function runs, rather than possibly still `None` (see
    /// [`crate::none_form_usage`]). `false` by default, so such a property
    /// is treated the same as an uninitialized local unless proven
    /// otherwise. Many projects consider that noise, since in practice the
    /// CK's Property Manager (or another script's `PropertySet`) has
    /// already filled every listed property in by the time any function
    /// runs; setting this to `true` drops that initial assumption. A
    /// property is still tracked (and flagged) once script code assigns it
    /// `None` directly, the same as a local variable. Has no effect on
    /// `unchecked_form_parameter`, which never tracks properties at all.
    pub assume_auto_properties_filled: bool,
    /// Per-ruleset enable/disable switches. Every ruleset is enabled by
    /// default unless `shared/rules.json` sets `enabled_by_default: false`;
    /// see [`Rules`].
    pub rules: Rules,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game: Game::default(),
            semicolon: false,
            indentation: Indentation::default(),
            indentation_width: 4,
            identifier_casing: IdentifierCasing::default(),
            cyclomatic_complexity_warning: 10,
            cyclomatic_complexity_error: 20,
            type_casing: TypeCasing::default(),
            named_arguments: NamedArguments::default(),
            min_wait_interval: 0.1,
            magic_numbers: MagicNumbers::default(),
            fail_on_warning: false,
            fail_on_info: false,
            bool_like_int: true,
            assume_auto_properties_filled: false,
            rules: Rules::default(),
        }
    }
}

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
