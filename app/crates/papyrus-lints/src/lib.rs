//! Lint rules for Bethesda's Papyrus scripting language.
//!
//! Each rule inspects raw Papyrus source text and reports [`Diagnostic`]s
//! for lines that violate it. Rules work on the source text directly
//! (rather than the parsed AST) so they still run on scripts that don't
//! parse cleanly.

mod actor_value;
mod argument_naming;
mod argument_override_types;
mod argument_types;
mod array_bounds;
mod array_size_range;
mod assignment_operator_spacing;
mod chain_whitespace;
mod circular_dependency;
mod comma_spacing;
pub mod config;
mod cyclomatic_complexity;
mod default_property_value;
mod disable_comments;
mod division_by_zero;
mod empty_body;
mod event_signature;
mod exclamation_spacing;
mod explicit_return;
mod float_equality;
mod float_int_conversion;
mod forbidden_functions;
mod formid_hex_notation;
mod getform_from_skyrim_esm;
mod fragment_code;
mod function_override;
mod get_state_comparison;
mod global_variable_increment;
mod global_variable_setvalue;
mod goto_state;
mod identifier_casing;
mod impossible_cast;
mod indentation;
mod int_division_to_float;
mod invalid_random_range;
mod invariant_loop_condition;
mod local_variable_shadowing;
mod magic_numbers;
mod missing_doc_comment;
mod missing_update_handler;
mod named_arguments;
mod native_function_usage;
mod non_global_function_call;
mod none_form_usage;
mod numeric_comparison;
mod operator_spacing;
mod parameter_reassignment;
mod property_sorting;
mod readonly_property_write;
mod repeated_getvalue;
mod repeated_setoutfit;
mod return_types;
mod script_name_collision;
mod self_assignment;
mod semicolon;
mod setvalue_in_loop;
mod short_wait_interval;
mod slow_functions;
mod state_count;
mod state_function_signature;
mod static_condition;
mod static_function_call_via_instance;
mod strict_boolean;
pub mod tags;
mod trailing_whitespace;
mod type_casing;
mod unchecked_array_element;
mod unchecked_cast;
mod unchecked_form_parameter;
mod unguarded_self_recursion;
mod unnecessary_function;
mod unreachable_elseif;
mod unreachable_statement;
mod unresolved_script;
mod unused_disable;
mod unused_getter;
mod unused_import;
mod unused_local_variable;
mod unused_property;
mod useless_downcast;
mod variable_used_before_assignment;

mod registry;

/// Every rule id [`lint`]/[`lint_with_external_arguments`] can report,
/// matched against `; @disable <rule-id>` directives (see
/// [`disable_comments`]) and validated against by callers (e.g. the CLI's
/// `fix --type <rule-id>`) that need to tell an unknown rule id apart from
/// a known one with no automatic fix (see [`FIXABLE_RULE_IDS`]).
pub use registry::{FIXABLE_RULE_IDS, KNOWN_RULE_IDS};

use serde::Serialize;

pub use argument_types::{ExternalSignatures, NoExternalSignatures, ParamInfo};
pub use config::{Config, MagicNumbers, NamedArguments, TypeCasing};

/// Runs the "Argument type check" lint against `source`, resolving calls
/// declared on other scripts through `external`.
///
/// This is the public entry for callers that already hold an
/// [`ExternalSignatures`] resolver (e.g. `papyrus-lint-core`'s
/// `FunctionTable`) and need that rule in isolation. Prefer
/// [`lint_with_external_arguments`] when you want every enabled rule.
pub fn check_argument_types<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Vec<Diagnostic> {
    argument_types::check_with(source, external)
}
