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
//!   trailing_whitespace: true
//!   comma_spacing: true
//!   forbidden_functions: true
//!   formid_hex_notation: true
//!   slow_functions: true
//!   unused_getter: true
//!   unused_property: true
//!   semicolon: true
//!   float_int_conversion: true
//!   int_division_to_float: true
//!   strict_boolean: true
//!   argument_types: true
//!   return_types: true
//!   function_override: true
//!   argument_naming: true
//!   argument_override_types: true
//!   numeric_comparison: true
//!   indentation: true
//!   cyclomatic_complexity: true
//!   unreachable_statement: true
//!   static_condition: true
//!   unreachable_elseif: true
//!   division_by_zero: true
//!   empty_body: true
//!   unused_local_variable: true
//!   variable_used_before_assignment: true
//!   none_form_usage: true
//!   local_variable_shadowing: true
//!   parameter_reassignment: true
//!   chain_whitespace: true
//!   exclamation_spacing: true
//!   identifier_casing: true
//!   type_casing: true
//!   named_arguments: true
//!   operator_spacing: true
//!   assignment_operator_spacing: true
//!   property_sorting: false
//!   explicit_return: true
//!   unchecked_form_parameter: false
//!   unchecked_array_element: false
//!   unchecked_cast: true
//!   useless_downcast: true
//!   impossible_cast: true
//!   unresolved_script: true
//!   non_global_function_call: true
//!   static_function_call_via_instance: true
//!   short_wait_interval: true
//!   state_function_signature: true
//!   goto_state: true
//!   get_state_comparison: true
//!   too_many_states: true
//!   multiple_auto_states: true
//!   conflicting_script_versions: true
//!   stale_compiled_output: true
//!   script_filename_mismatch: true
//!   magic_numbers: false
//!   native_function_usage: false
//!   repeated_getvalue: false
//!   global_variable_setvalue: false
//!   global_variable_increment: true
//!   setvalue_in_loop: true
//!   invariant_loop_condition: true
//!   script_name_collision: true
//!   array_bounds: true
//!   array_size_range: true
//!   readonly_property_write: true
//!   default_property_value: false
//!   unguarded_self_recursion: true
//!   self_assignment: true
//!   unnecessary_function: true
//!   unknown_actor_value: false
//!   repeated_setoutfit: true
//!   missing_doc_comment: false
//!   invalid_random_range: true
//!   float_equality: false
//!   missing_update_handler: false
//!   unused_import: true
//!   event_signature_mismatch: false
//!   circular_dependency: false
//! ```
//!
//! Every entry under `rules` is enabled by default; set one to `false` to
//! disable that lint (and its automatic fix, if it has one) entirely. As
//! with the top-level keys, `rules` and any key within it may be omitted
//! and falls back to its default. `property_sorting`,
//! `unchecked_form_parameter`, `unchecked_array_element`, `magic_numbers`,
//! `native_function_usage`,
//! `repeated_getvalue`, `global_variable_setvalue`,
//! `default_property_value`, `unknown_actor_value`,
//! `missing_doc_comment`, `float_equality`, `missing_update_handler`,
//! `event_signature_mismatch`, and `circular_dependency` are the
//! exceptions: they default to `false`.
//! `property_sorting` reorders a script's declared properties, a more
//! invasive change than the rest of these rules; `unchecked_form_parameter`
//! defaults off because many scripts intentionally accept a possibly-`None`
//! Form and defer the check to a caller or a later branch;
//! `unchecked_array_element` defaults off for the same reason, extended to
//! array elements instead of parameters; `magic_numbers`
//! defaults off because many existing scripts contain plenty of
//! unremarkable literal numbers a project may not want flagged all at
//! once; `native_function_usage` defaults off because plenty of mods
//! intentionally depend on SKSE/F4SE or another native extension and don't
//! need to be warned about it; `repeated_getvalue` defaults off because a
//! chain that reads the same global more than once is often written that
//! way deliberately for readability, and the performance cost is usually
//! negligible outside a hot code path; `global_variable_setvalue` defaults
//! off since its heuristic `Else`-branch check can't actually prove the
//! write it flags is redundant, only that the branch never checked;
//! `default_property_value` defaults off because many existing scripts
//! already rely on Papyrus's own implicit per-type defaults for some or
//! all of their properties; `unknown_actor_value` defaults off because a
//! project's own plugin can define additional, custom Actor Values that
//! have no way to appear in `rules/actor-values.yaml`; `missing_doc_comment`
//! defaults off because most existing scripts have no documentation
//! comments at all, and enabling it would otherwise flag literally every
//! `ScriptName`/`Property`/`Function`/`Event` declaration in such a project
//! at once; `float_equality` defaults off because a project may
//! deliberately compare two `Float` values it knows are computed the exact
//! same way, and enabling it by default would flag every such comparison
//! as a false positive; `missing_update_handler` defaults off because it
//! only ever sees a single script's own source, so a `RegisterFor*` call
//! whose matching `Event` is instead declared on a script it `Extends`
//! would otherwise be misreported as having no handler at all;
//! `event_signature_mismatch` defaults off because `rules/known-events.yaml`
//! only lists a curated subset of the engine's native events, and this
//! lint matches an `Event`'s name alone, regardless of whether the
//! enclosing script actually extends the Form that declares it, which
//! would otherwise misreport a same-named custom event; `circular_dependency`
//! defaults off because two scripts intentionally holding `Property`
//! references to each other for two-way communication (e.g. a manager and a
//! worker script) is a common, legitimate design, not a mistake, and
//! enabling this by default would flag it as one. All fourteen need a
//! project to opt in explicitly.
//!
//! `assume_auto_properties_filled` (a top-level key, not a `rules` entry)
//! is `false` by default: see [`Config::assume_auto_properties_filled`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Diagnostic;

pub use crate::magic_numbers::MagicNumbers;
pub use crate::named_arguments::NamedArguments;
pub use crate::type_casing::Style as TypeCasing;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// default; see [`Rules`].
    pub rules: Rules,
}

impl Default for Config {
    fn default() -> Self {
        Self {
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
    /// The "FormID hex notation" lint.
    pub formid_hex_notation: bool,
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
    /// The "Int/Int division widened to Float" lint.
    pub int_division_to_float: bool,
    /// The "Strict boolean check" lint.
    pub strict_boolean: bool,
    /// The "Argument type check" lint.
    pub argument_types: bool,
    /// The "Return type check" lint.
    pub return_types: bool,
    /// The "Inherited function override" lint.
    pub function_override: bool,
    /// The "Argument naming consistency" lint.
    pub argument_naming: bool,
    /// The "Argument override type check" lint.
    pub argument_override_types: bool,
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
    /// The "Unreachable elseif" lint.
    pub unreachable_elseif: bool,
    /// The "Division by zero" lint.
    pub division_by_zero: bool,
    /// The "Empty loop/conditional body" lint.
    pub empty_body: bool,
    /// The "Unused or write-only local variables" lint.
    pub unused_local_variable: bool,
    /// The "Local variable used before assignment" lint.
    pub variable_used_before_assignment: bool,
    /// The "None used as an existing Form" lint.
    pub none_form_usage: bool,
    /// The "Local variable shadowing" lint.
    pub local_variable_shadowing: bool,
    /// The "Parameter reassignment" lint.
    pub parameter_reassignment: bool,
    /// The "Whitespace interrupting property/method chaining" lint/fix.
    pub chain_whitespace: bool,
    /// The "Exclamation mark spacing" lint/fix.
    pub exclamation_spacing: bool,
    /// The "Identifier casing" lint.
    pub identifier_casing: bool,
    /// The "Type name casing" lint.
    pub type_casing: bool,
    /// The "Prefer named arguments" lint.
    pub named_arguments: bool,
    /// The "Spacing around logical/comparison operators" lint/fix.
    pub operator_spacing: bool,
    /// The "Spacing around assignment operators" lint/fix.
    pub assignment_operator_spacing: bool,
    /// The "Property sorting" lint/fix. Unlike every other field here,
    /// this defaults to `false`: see [`crate::property_sorting`].
    pub property_sorting: bool,
    /// The "Explicit return on every path" lint.
    pub explicit_return: bool,
    /// The "Form parameter used without a None check" lint. Like
    /// [`Self::property_sorting`], this defaults to `false`: see
    /// [`crate::unchecked_form_parameter`].
    pub unchecked_form_parameter: bool,
    /// The "Array element used without a None check" lint. Like
    /// [`Self::property_sorting`] and [`Self::unchecked_form_parameter`],
    /// this defaults to `false`: see [`crate::unchecked_array_element`].
    pub unchecked_array_element: bool,
    /// The "Unchecked cast" lint.
    pub unchecked_cast: bool,
    /// The "Useless downcast" lint.
    pub useless_downcast: bool,
    /// The "Impossible cast" lint.
    pub impossible_cast: bool,
    /// The "Unresolved script reference" lint.
    pub unresolved_script: bool,
    /// The "Non-static function call" lint.
    pub non_global_function_call: bool,
    /// The "Static function called via instance" lint.
    pub static_function_call_via_instance: bool,
    /// The "Short wait/update interval" lint.
    pub short_wait_interval: bool,
    /// The "State function signature mismatch" lint.
    pub state_function_signature: bool,
    /// The "GoToState state reference" lint.
    pub goto_state: bool,
    /// The "GetState() comparison" lint.
    pub get_state_comparison: bool,
    /// The "Total named state count" lint.
    pub too_many_states: bool,
    /// The "Multiple Auto states" lint.
    pub multiple_auto_states: bool,
    /// The "Conflicting script versions" project lint.
    pub conflicting_script_versions: bool,
    /// The "Stale compiled output" project lint: flags a `.psc` whose
    /// compiled `.pex` output is older than the source itself. Only
    /// available when linting a file with project context in the desktop
    /// app or CLI, the same as [`Self::conflicting_script_versions`].
    pub stale_compiled_output: bool,
    /// The "ScriptName/filename mismatch" project lint: flags a `.psc`
    /// whose declared `ScriptName` doesn't match its own file name, aside
    /// from casing. Only available when linting a file with a known path in
    /// the desktop app or CLI, the same as [`Self::stale_compiled_output`],
    /// but unlike it doesn't need project context.
    pub script_filename_mismatch: bool,
    /// The "Unused disable directive" lint. Defaults to `false`.
    pub unused_disable: bool,
    /// The "Magic numbers" lint. Like [`Self::property_sorting`] and
    /// [`Self::unchecked_form_parameter`], this defaults to `false`: see
    /// [`crate::magic_numbers`].
    pub magic_numbers: bool,
    /// The "Non-base-game native function usage" lint. Like
    /// [`Self::property_sorting`], [`Self::unchecked_form_parameter`], and
    /// [`Self::magic_numbers`], this defaults to `false`: see
    /// [`crate::native_function_usage`].
    pub native_function_usage: bool,
    /// The "Repeated GlobalVariable.GetValue() calls" lint. Like
    /// [`Self::property_sorting`], [`Self::unchecked_form_parameter`],
    /// [`Self::magic_numbers`], and [`Self::native_function_usage`], this
    /// defaults to `false`: see [`crate::repeated_getvalue`].
    pub repeated_getvalue: bool,
    /// The "GlobalVariable no-op write" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`], and
    /// [`Self::native_function_usage`], this defaults to `false`: see
    /// [`crate::global_variable_setvalue`].
    pub global_variable_setvalue: bool,
    /// The "GlobalVariable increment via SetValue(GetValue() + x)" lint.
    pub global_variable_increment: bool,
    /// The "Repeated GlobalVariable.SetValue() calls in a loop" lint.
    pub setvalue_in_loop: bool,
    /// The "Invariant loop condition" lint.
    pub invariant_loop_condition: bool,
    /// The "Property/variable named as script" lint.
    pub script_name_collision: bool,
    /// The "Array bounds" lint.
    pub array_bounds: bool,
    /// The "Array size range" lint.
    pub array_size_range: bool,
    /// The "Read-only (AutoReadOnly) property write" lint.
    pub readonly_property_write: bool,
    /// The "Default property value" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`], and
    /// [`Self::global_variable_setvalue`], this defaults to `false`: see
    /// [`crate::default_property_value`].
    pub default_property_value: bool,
    /// The "Unguarded self-recursion" lint.
    pub unguarded_self_recursion: bool,
    /// The "Self-assignment" lint.
    pub self_assignment: bool,
    /// The "Unnecessary function" lint.
    pub unnecessary_function: bool,
    /// The "Unknown Actor Value" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`],
    /// [`Self::global_variable_setvalue`], and
    /// [`Self::default_property_value`], this defaults to `false`: see
    /// [`crate::actor_value`].
    pub unknown_actor_value: bool,
    /// The "Repeated Actor.SetOutfit() calls" lint.
    pub repeated_setoutfit: bool,
    /// The "Missing documentation comment" lint. Like
    /// [`Self::property_sorting`], [`Self::unchecked_form_parameter`],
    /// [`Self::magic_numbers`], [`Self::native_function_usage`],
    /// [`Self::repeated_getvalue`], [`Self::global_variable_setvalue`],
    /// [`Self::default_property_value`], and [`Self::unknown_actor_value`],
    /// this defaults to `false`: see [`crate::missing_doc_comment`].
    pub missing_doc_comment: bool,
    /// The "Invalid random range" lint.
    pub invalid_random_range: bool,
    /// The "Float equality comparison" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`],
    /// [`Self::global_variable_setvalue`], [`Self::default_property_value`],
    /// [`Self::unknown_actor_value`], and [`Self::missing_doc_comment`],
    /// this defaults to `false`: see [`crate::float_equality`].
    pub float_equality: bool,
    /// The "Missing update event handler" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`],
    /// [`Self::global_variable_setvalue`], [`Self::default_property_value`],
    /// [`Self::unknown_actor_value`], [`Self::missing_doc_comment`], and
    /// [`Self::float_equality`], this defaults to `false`: see
    /// [`crate::missing_update_handler`].
    pub missing_update_handler: bool,
    /// The "Unused import" lint.
    pub unused_import: bool,
    /// The "Event signature mismatch" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`],
    /// [`Self::global_variable_setvalue`], [`Self::default_property_value`],
    /// [`Self::unknown_actor_value`], [`Self::missing_doc_comment`],
    /// [`Self::float_equality`], and [`Self::missing_update_handler`], this
    /// defaults to `false`: see [`crate::event_signature`].
    pub event_signature_mismatch: bool,
    /// The "Circular script dependency" lint. Like [`Self::property_sorting`],
    /// [`Self::unchecked_form_parameter`], [`Self::magic_numbers`],
    /// [`Self::native_function_usage`], [`Self::repeated_getvalue`],
    /// [`Self::global_variable_setvalue`], [`Self::default_property_value`],
    /// [`Self::unknown_actor_value`], [`Self::missing_doc_comment`],
    /// [`Self::float_equality`], [`Self::missing_update_handler`], and
    /// [`Self::event_signature_mismatch`], this defaults to `false`: see
    /// [`crate::circular_dependency`].
    pub circular_dependency: bool,
}

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
        crate::registry::default_rules()
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

/// An error parsing a lint config file.
#[derive(Debug)]
pub enum ConfigError {
    Yaml(serde_norway::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Yaml(err) => write!(f, "failed to parse lint config: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<serde_norway::Error> for ConfigError {
    fn from(err: serde_norway::Error) -> Self {
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
    Ok(serde_norway::from_str(yaml)?)
}

/// Serializes a [`Config`] back into the YAML document format read by
/// [`parse`], so the desktop app can persist the formatting selected in
/// its UI.
pub fn to_yaml(config: &Config) -> Result<String, ConfigError> {
    Ok(serde_norway::to_string(config)?)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
