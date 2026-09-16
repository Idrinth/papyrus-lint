//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

pub mod actor_value;
pub mod argument_naming;
pub mod argument_override_types;
pub mod argument_types;
pub mod array_bounds;
pub mod array_size_range;
pub mod assignment_operator_spacing;
pub mod chain_whitespace;
pub mod circular_dependency;
pub mod comma_spacing;
pub mod config;
pub mod cyclomatic_complexity;
pub mod default_property_value;
mod disable_comments;
pub mod division_by_zero;
pub mod empty_body;
pub mod event_signature;
pub mod exclamation_spacing;
pub mod explicit_return;
pub mod float_equality;
pub mod float_int_conversion;
pub mod forbidden_functions;
pub mod formid_hex_notation;
pub mod fragment_code;
pub mod function_override;
pub mod get_state_comparison;
pub mod global_variable_increment;
pub mod global_variable_setvalue;
pub mod goto_state;
pub mod identifier_casing;
pub mod impossible_cast;
pub mod indentation;
pub mod int_division_to_float;
pub mod invalid_random_range;
pub mod invariant_loop_condition;
pub mod local_variable_shadowing;
pub mod magic_numbers;
pub mod missing_doc_comment;
pub mod missing_update_handler;
pub mod named_arguments;
pub mod native_function_usage;
pub mod non_global_function_call;
pub mod none_form_usage;
pub mod numeric_comparison;
pub mod operator_spacing;
pub mod parameter_reassignment;
pub mod property_sorting;
pub mod readonly_property_write;
pub mod repeated_getvalue;
pub mod repeated_setoutfit;
pub mod return_types;
pub mod script_name_collision;
pub mod self_assignment;
pub mod semicolon;
pub mod setvalue_in_loop;
pub mod short_wait_interval;
pub mod slow_functions;
pub mod state_count;
pub mod state_function_signature;
pub mod static_condition;
pub mod static_function_call_via_instance;
pub mod strict_boolean;
pub mod tags;
pub mod trailing_whitespace;
pub mod type_casing;
pub mod unchecked_array_element;
pub mod unchecked_cast;
pub mod unchecked_form_parameter;
pub mod unguarded_self_recursion;
pub mod unnecessary_function;
pub mod unreachable_elseif;
pub mod unreachable_statement;
pub mod unresolved_script;
pub mod unused_disable;
pub mod unused_getter;
pub mod unused_import;
pub mod unused_local_variable;
pub mod unused_property;
pub mod useless_downcast;
pub mod variable_used_before_assignment;

mod registry;

/// Every rule id [`lint`]/[`lint_with_external_arguments`] can report,
/// matched against `; @disable <rule-id>` directives (see
/// [`disable_comments`]) and validated against by callers (e.g. the CLI's
/// `fix --type <rule-id>`) that need to tell an unknown rule id apart from
/// a known one with no automatic fix (see [`FIXABLE_RULE_IDS`]).
pub use registry::{FIXABLE_RULE_IDS, KNOWN_RULE_IDS};

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
/// rule ids suppresses every lint on that line. A `; @disable-file
/// <rule-id>[, <rule-id>...]` comment does the same across the entire
/// file, no matter which line it's written on; `; @disable-file` with no
/// rule ids suppresses every lint in the file. See [`disable_comments`]
/// for the rule ids each lint is matched against. This does not affect
/// [`repair`], which still applies its fixes regardless of
/// `@disable`/`@disable-file` comments.
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
/// parameter names against the inherited declaration's, so the "Argument
/// override type check" lint can compare an overriding function's
/// parameter count/types against the inherited declaration's, so the "Total
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
    lint_with_external_arguments_and_extra_diagnostics(source, config, external, Vec::new())
}

/// Like [`lint_with_external_arguments`], but merges `extra_diagnostics` in
/// *before* validating `@disable`/`@disable-file` directives and checking
/// for unused ones, rather than a caller appending them to this function's
/// own already-finalized result afterward.
///
/// `papyrus-lint-core`'s path-dependent project diagnostics
/// (`stale-compiled-output`, `conflicting-script-versions`,
/// `script-filename-mismatch`) need a file path or project root this crate
/// never sees, so a caller computes them separately and passes them in here
/// as `extra_diagnostics` instead of extending
/// [`lint_with_external_arguments`]'s own result with them. Appending them
/// afterward would mean [`unused_disable`]'s validation — which only ever
/// sees the diagnostics gathered before that result is returned — never
/// learns that a directive naming one of them actually suppressed
/// something, and reports it as an `unused-disable` even though [`is_disabled`]
/// does honor it.
pub fn lint_with_external_arguments_and_extra_diagnostics<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
    extra_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let mut diagnostics = registry::collect_diagnostics(source, config, external);
    diagnostics.extend(extra_diagnostics);
    let disables = disable_comments::Disables::scan(source);
    let unused_disables = config
        .rules
        .unused_disable
        .then(|| unused_disable::check(&disables, &diagnostics, KNOWN_RULE_IDS));
    diagnostics.retain(|diagnostic| !disables.is_disabled(diagnostic.line, diagnostic.rule));
    diagnostics.extend(unused_disables.into_iter().flatten());
    diagnostics
}

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
    repair_with(source, config, |rule| {
        rule_filter.is_none_or(|filter| filter == rule)
    })
}

/// Like [`repair_filtered`], but selects every fixable rule tagged (see
/// [`tags`]) with `tag` instead of a single rule id, matched
/// case-insensitively against each rule's kind keyword(s) (e.g. `"style"`
/// or `"performance"`). `tag` of `None` behaves exactly like [`repair`].
/// This lets a caller (e.g. the CLI's `fix --tag <kind>`) apply every
/// automatic fix in one class at once, the tag-based counterpart to
/// [`repair_filtered`]'s single-rule `fix --type <rule-id>`.
pub fn repair_filtered_by_tag(source: &str, config: &Config, tag: Option<&str>) -> String {
    repair_with(source, config, |rule| {
        tag.is_none_or(|tag| {
            tags::tags_for(rule).is_some_and(|rule_tags| {
                rule_tags
                    .kinds
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case(tag))
            })
        })
    })
}

/// Like [`repair`], but also applies the "unused-import" fix (see
/// [`unused_import::repair_with`]), resolving each `Import`'s usage through
/// `external` — the same [`argument_types::ExternalSignatures`] resolver
/// [`lint_with_external_arguments`] uses for that rule's own diagnostics.
/// Every other fix in [`FIXABLE_RULE_IDS`] behaves exactly as it does under
/// [`repair`], since only "unused-import" needs project-wide context to
/// resolve anything at all.
pub fn repair_with_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
) -> String {
    repair_filtered_with_external_arguments(source, config, external, None)
}

/// Like [`repair_filtered`], but also resolves "unused-import" through
/// `external`, the same way [`repair_with_external_arguments`] does.
pub fn repair_filtered_with_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
    rule_filter: Option<&str>,
) -> String {
    repair_with_external(source, config, external, |rule| {
        rule_filter.is_none_or(|filter| filter == rule)
    })
}

/// Like [`repair_filtered_by_tag`], but also resolves "unused-import"
/// through `external`, the same way [`repair_with_external_arguments`]
/// does.
pub fn repair_filtered_by_tag_with_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
    tag: Option<&str>,
) -> String {
    repair_with_external(source, config, external, |rule| {
        tag.is_none_or(|tag| {
            tags::tags_for(rule).is_some_and(|rule_tags| {
                rule_tags
                    .kinds
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case(tag))
            })
        })
    })
}

/// Combines [`repair_filtered_with_external_arguments`] and
/// [`repair_filtered_by_tag_with_external_arguments`] with an optional
/// [`restrict_to_line`] step, the exact selection both the CLI's `fix`
/// command and the desktop app's per-finding/per-rule repair commands need:
/// `tag_filter`, when `Some`, takes precedence over `rule_filter` (a caller
/// only ever sets one); `target_line`, when `Some`, restricts the result to
/// that line the same way [`restrict_to_line`] does. Returns `Err(())` in
/// `target_line`'s place — a fix that changes the file's line count (e.g.
/// `property-sorting` relocating a property's declaration) — the same way
/// [`restrict_to_line`] itself does, since the two callers report that
/// failure with different wording of their own.
pub fn repair_selected_with_external_arguments<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
    rule_filter: Option<&str>,
    tag_filter: Option<&str>,
    target_line: Option<usize>,
) -> Option<String> {
    let repaired = match tag_filter {
        Some(tag) => {
            repair_filtered_by_tag_with_external_arguments(source, config, external, Some(tag))
        }
        None => repair_filtered_with_external_arguments(source, config, external, rule_filter),
    };
    match target_line {
        Some(line) => restrict_to_line(source, &repaired, line),
        None => Some(repaired),
    }
}

/// Shared implementation behind the three `_with_external_arguments`
/// functions above: applies every self-contained fix via [`repair_with`]
/// (unaffected by `external`), then — if `config.rules.unused_import` is
/// enabled and `applies` accepts [`unused_import::RULE`] — removes every
/// `Import` line [`unused_import::check_with`] resolves as unused through
/// `external`.
fn repair_with_external<E: argument_types::ExternalSignatures>(
    source: &str,
    config: &Config,
    external: &mut E,
    applies: impl Fn(&str) -> bool,
) -> String {
    let source = repair_with(source, config, &applies);
    if config.rules.unused_import && applies(unused_import::RULE) {
        unused_import::repair_with(&source, external)
    } else {
        source
    }
}

/// Shared implementation behind [`repair_filtered`] and
/// [`repair_filtered_by_tag`]: applies every fix in [`FIXABLE_RULE_IDS`]
/// whose ruleset is enabled in `config.rules` and whose rule id `applies`
/// accepts.
fn repair_with(source: &str, config: &Config, applies: impl Fn(&str) -> bool) -> String {
    registry::apply_repairs(source, config, applies)
}

/// Rebuilds a repair result so only `target_line` (1-indexed) differs from
/// `original`, leaving every other line exactly as it was — even if
/// `repaired` (e.g. from [`repair_filtered`]) changed other lines too.
/// Returns `None` if `original` and `repaired` don't have the same number
/// of lines, since a fix that shifts the line count (e.g.
/// `property-sorting` relocating a property's declaration) means a single
/// original line number no longer identifies the same line in the result.
/// Lets a caller apply just one line's worth of an automatic fix — e.g. the
/// CLI's `fix --line <n>`, or the desktop app's per-finding "Fix this
/// issue" button — without touching lines the user didn't ask about.
pub fn restrict_to_line(original: &str, repaired: &str, target_line: usize) -> Option<String> {
    let original_lines: Vec<&str> = original.split('\n').collect();
    let repaired_lines: Vec<&str> = repaired.split('\n').collect();
    if original_lines.len() != repaired_lines.len() {
        return None;
    }
    Some(
        original_lines
            .iter()
            .zip(repaired_lines.iter())
            .enumerate()
            .map(|(i, (original_line, repaired_line))| {
                if i + 1 == target_line {
                    *repaired_line
                } else {
                    *original_line
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Applies just `rule`'s automatic fix (a [`FIXABLE_RULE_IDS`] id) to
/// `source` and returns what `target_line` (1-indexed) would look like
/// afterward, or `None` if there's nothing meaningful to show: `rule`'s fix
/// doesn't change `source` at all, applying it would shift the line count
/// (via [`restrict_to_line`], the same case that fix's own `Diagnostic`
/// shouldn't be treated as fixable in place), `target_line` doesn't exist in
/// `source`, or the fix doesn't actually touch `target_line` itself. Powers
/// the desktop app's "Export for AI" document, which attaches this preview
/// to each exported finding from an auto-fixable rule so an AI reading it
/// can see the fix without having to apply it first.
pub fn repaired_line(
    source: &str,
    config: &Config,
    rule: &str,
    target_line: usize,
) -> Option<String> {
    let repaired = repair_filtered(source, config, Some(rule));
    if repaired == source {
        return None;
    }
    let restricted = restrict_to_line(source, &repaired, target_line)?;
    let index = target_line.checked_sub(1)?;
    let original_line = source.split('\n').nth(index)?;
    let restricted_line = restricted.split('\n').nth(index)?;
    (restricted_line != original_line).then(|| restricted_line.to_string())
}

/// Adds (or extends) an `; @disable <rules>` line comment on `target_line`
/// (1-indexed) of `source`, covering every rule id in `rules` — the inverse
/// of [`repaired_line`]'s per-line "Fix": rather than fixing the findings on
/// that line, it silences them, driving the desktop app's per-line "Ignore"
/// button in the code viewer. See
/// [`crate::disable_comments::add_disable_directive`] for the exact
/// merging/formatting rules. A rule already covered by an existing
/// directive on that line, or a bare `@disable` already covering every
/// rule, is left as-is; an empty `rules` list or an out-of-range
/// `target_line` leaves `source` untouched.
pub fn add_disable_comment(source: &str, target_line: usize, rules: &[String]) -> String {
    disable_comments::add_disable_directive(source, target_line, rules)
}

/// Whether `rule` is suppressed on `line` (1-indexed) of `source` by an
/// `@disable`/`@disable-file` directive (see [`disable_comments`]), matched
/// the same case-insensitive way [`lint`]/[`lint_with_external_arguments`]
/// match their own diagnostics. Exposes the same check
/// [`lint_with_external_arguments_and_extra_diagnostics`] applies internally
/// to every diagnostic (including the `extra_diagnostics` a caller merges
/// in) for a caller that needs to ask about a single diagnostic in
/// isolation instead — namely `papyrus-lint-core`'s project-level lints
/// (`stale-compiled-output`, `conflicting-script-versions`,
/// `script-filename-mismatch`), which need more than just `source` (a file
/// path, or another script's contents) to run and so can't be dispatched
/// from inside this crate at all. Prefer merging such a diagnostic in via
/// [`lint_with_external_arguments_and_extra_diagnostics`] over filtering it
/// with this function by hand: only the former also validates the
/// directive as used for the `unused-disable` lint.
pub fn is_disabled(source: &str, line: usize, rule: &str) -> bool {
    disable_comments::Disables::scan(source).is_disabled(line, rule)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
