//! Single table of built-in lints.
//!
//! Adding a source-level rule should mean: a new module, one entry in
//! [`collect_diagnostics`] (and [`apply_repairs`] if it has a fix), the
//! matching [`crate::config::Rules`] field + default, and a README row.
//! [`KNOWN_RULE_IDS`] / [`FIXABLE_RULE_IDS`] are derived from the same
//! lists so dispatch and the public id tables cannot drift.

use crate::argument_types::ExternalSignatures;
use crate::config::{Config, Rules};
use crate::{
    actor_value, argument_naming, argument_override_types, argument_types, array_bounds,
    array_size_range, assignment_operator_spacing, chain_whitespace, comma_spacing,
    cyclomatic_complexity, default_property_value, division_by_zero, empty_body, event_signature,
    exclamation_spacing, explicit_return, float_equality, float_int_conversion,
    forbidden_functions, formid_hex_notation, function_override, get_state_comparison,
    global_variable_increment, global_variable_setvalue, goto_state, identifier_casing,
    impossible_cast, indentation, int_division_to_float, invalid_random_range,
    invariant_loop_condition, local_variable_shadowing, magic_numbers, missing_doc_comment,
    missing_update_handler, named_arguments, native_function_usage, non_global_function_call,
    none_form_usage, numeric_comparison, operator_spacing, parameter_reassignment,
    property_sorting, readonly_property_write, repeated_getvalue, repeated_setoutfit, return_types,
    script_name_collision, self_assignment, semicolon, setvalue_in_loop, short_wait_interval,
    slow_functions, state_count, state_function_signature, static_condition,
    static_function_call_via_instance, strict_boolean, trailing_whitespace, type_casing,
    unchecked_array_element, unchecked_cast, unchecked_form_parameter, unguarded_self_recursion,
    unnecessary_function, unreachable_elseif, unreachable_statement, unresolved_script,
    unused_disable, unused_getter, unused_import, unused_local_variable, unused_property,
    useless_downcast, variable_used_before_assignment, Diagnostic,
};

/// Project-level ids that are not dispatched inside this crate. Callers
/// pass matching diagnostics in as `extra_diagnostics`.
pub const EXTRA_RULE_IDS: &[&str] = &[
    "conflicting-script-versions",
    "stale-compiled-output",
    "script-filename-mismatch",
];

/// Every rule id [`crate::lint`] can report.
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
    int_division_to_float::RULE,
    strict_boolean::RULE,
    argument_types::RULE,
    return_types::RULE,
    function_override::RULE,
    argument_naming::RULE,
    argument_override_types::RULE,
    state_function_signature::RULE,
    numeric_comparison::RULE,
    indentation::RULE,
    cyclomatic_complexity::RULE,
    unreachable_statement::RULE,
    static_condition::RULE,
    unreachable_elseif::RULE,
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
    assignment_operator_spacing::RULE,
    property_sorting::RULE,
    explicit_return::RULE,
    unchecked_form_parameter::RULE,
    unchecked_array_element::RULE,
    unchecked_cast::RULE,
    useless_downcast::RULE,
    impossible_cast::RULE,
    unresolved_script::RULE,
    non_global_function_call::RULE,
    static_function_call_via_instance::RULE,
    short_wait_interval::RULE,
    goto_state::RULE,
    get_state_comparison::RULE,
    state_count::TOO_MANY_STATES_RULE,
    state_count::MULTIPLE_AUTO_STATES_RULE,
    EXTRA_RULE_IDS[0],
    EXTRA_RULE_IDS[1],
    EXTRA_RULE_IDS[2],
    unused_disable::RULE,
    magic_numbers::RULE,
    variable_used_before_assignment::RULE,
    native_function_usage::RULE,
    repeated_getvalue::RULE,
    global_variable_setvalue::RULE,
    global_variable_increment::RULE,
    setvalue_in_loop::RULE,
    invariant_loop_condition::RULE,
    script_name_collision::RULE,
    array_bounds::RULE,
    array_size_range::RULE,
    readonly_property_write::RULE,
    default_property_value::RULE,
    unguarded_self_recursion::RULE,
    self_assignment::RULE,
    unnecessary_function::RULE,
    actor_value::RULE,
    repeated_setoutfit::RULE,
    missing_doc_comment::RULE,
    invalid_random_range::RULE,
    float_equality::RULE,
    missing_update_handler::RULE,
    unused_import::RULE,
    event_signature::RULE,
];

/// Rule ids with an automatic fix, in the order [`apply_repairs`] applies them.
pub const FIXABLE_RULE_IDS: &[&str] = &[
    identifier_casing::RULE,
    slow_functions::RULE,
    semicolon::RULE,
    indentation::RULE,
    property_sorting::RULE,
    comma_spacing::RULE,
    chain_whitespace::RULE,
    exclamation_spacing::RULE,
    operator_spacing::RULE,
    assignment_operator_spacing::RULE,
    type_casing::RULE,
    trailing_whitespace::RULE,
    global_variable_increment::RULE,
    named_arguments::RULE,
    unnecessary_function::RULE,
    unused_import::RULE,
];

/// Default enable/disable flags for [`Rules`]. Kept next to the dispatch
/// table so a new field is added in one crate module.
pub fn default_rules() -> Rules {
    Rules {
        trailing_whitespace: true,
        comma_spacing: true,
        forbidden_functions: true,
        formid_hex_notation: true,
        slow_functions: true,
        unused_getter: true,
        unused_property: true,
        semicolon: true,
        float_int_conversion: true,
        int_division_to_float: true,
        strict_boolean: true,
        argument_types: true,
        return_types: true,
        function_override: true,
        argument_naming: true,
        argument_override_types: true,
        numeric_comparison: true,
        indentation: true,
        cyclomatic_complexity: true,
        unreachable_statement: true,
        static_condition: true,
        unreachable_elseif: true,
        division_by_zero: true,
        empty_body: true,
        unused_local_variable: true,
        variable_used_before_assignment: true,
        none_form_usage: true,
        local_variable_shadowing: true,
        parameter_reassignment: true,
        chain_whitespace: true,
        exclamation_spacing: true,
        identifier_casing: true,
        type_casing: true,
        named_arguments: true,
        operator_spacing: true,
        assignment_operator_spacing: true,
        property_sorting: false,
        explicit_return: true,
        unchecked_form_parameter: false,
        unchecked_array_element: false,
        unchecked_cast: true,
        useless_downcast: true,
        impossible_cast: true,
        unresolved_script: true,
        non_global_function_call: true,
        static_function_call_via_instance: true,
        short_wait_interval: true,
        state_function_signature: true,
        goto_state: true,
        get_state_comparison: true,
        too_many_states: true,
        multiple_auto_states: true,
        conflicting_script_versions: true,
        stale_compiled_output: true,
        script_filename_mismatch: true,
        unused_disable: false,
        magic_numbers: false,
        native_function_usage: false,
        repeated_getvalue: false,
        global_variable_setvalue: false,
        global_variable_increment: true,
        setvalue_in_loop: true,
        invariant_loop_condition: true,
        script_name_collision: true,
        array_bounds: true,
        array_size_range: true,
        readonly_property_write: true,
        default_property_value: false,
        unguarded_self_recursion: true,
        self_assignment: true,
        unnecessary_function: true,
        unknown_actor_value: false,
        repeated_setoutfit: true,
        missing_doc_comment: false,
        invalid_random_range: true,
        float_equality: false,
        missing_update_handler: false,
        unused_import: true,
        event_signature_mismatch: false,
    }
}

pub fn collect_diagnostics<E: ExternalSignatures>(
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
    if rules.int_division_to_float {
        diagnostics.extend(int_division_to_float::check(source));
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
    if rules.non_global_function_call {
        diagnostics.extend(non_global_function_call::check_with(source, external));
    }
    if rules.static_function_call_via_instance {
        diagnostics.extend(static_function_call_via_instance::check_with(
            source, external,
        ));
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
    if rules.argument_override_types {
        diagnostics.extend(argument_override_types::check_with(source, external));
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
    if rules.unreachable_elseif {
        diagnostics.extend(unreachable_elseif::check(source));
    }
    if rules.division_by_zero {
        diagnostics.extend(division_by_zero::check(source));
    }
    if rules.invalid_random_range {
        diagnostics.extend(invalid_random_range::check(source));
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
        diagnostics.extend(none_form_usage::check(
            source,
            config.assume_auto_properties_filled,
        ));
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
    if rules.assignment_operator_spacing {
        diagnostics.extend(assignment_operator_spacing::check(source));
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
    if rules.unchecked_array_element {
        diagnostics.extend(unchecked_array_element::check(source));
    }
    if rules.unchecked_cast {
        diagnostics.extend(unchecked_cast::check(source));
    }
    if rules.useless_downcast {
        diagnostics.extend(useless_downcast::check_with(source, external));
    }
    if rules.impossible_cast {
        diagnostics.extend(impossible_cast::check_with(source, external));
    }
    if rules.short_wait_interval {
        diagnostics.extend(short_wait_interval::check(source, config.min_wait_interval));
    }
    if rules.goto_state {
        diagnostics.extend(goto_state::check_with(source, external));
    }
    if rules.get_state_comparison {
        diagnostics.extend(get_state_comparison::check_with(source, external));
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
    if rules.native_function_usage {
        diagnostics.extend(native_function_usage::check(source));
    }
    if rules.repeated_getvalue {
        diagnostics.extend(repeated_getvalue::check(source));
    }
    if rules.global_variable_setvalue {
        diagnostics.extend(global_variable_setvalue::check(source));
    }
    if rules.global_variable_increment {
        diagnostics.extend(global_variable_increment::check(source));
    }
    if rules.setvalue_in_loop {
        diagnostics.extend(setvalue_in_loop::check(source));
    }
    if rules.invariant_loop_condition {
        diagnostics.extend(invariant_loop_condition::check(source));
    }
    if rules.script_name_collision {
        diagnostics.extend(script_name_collision::check(source));
    }
    if rules.array_bounds {
        diagnostics.extend(array_bounds::check(source));
    }
    if rules.array_size_range {
        diagnostics.extend(array_size_range::check(source));
    }
    if rules.readonly_property_write {
        diagnostics.extend(readonly_property_write::check(source));
    }
    if rules.default_property_value {
        diagnostics.extend(default_property_value::check(source));
    }
    if rules.unguarded_self_recursion {
        diagnostics.extend(unguarded_self_recursion::check(source));
    }
    if rules.self_assignment {
        diagnostics.extend(self_assignment::check(source));
    }
    if rules.unnecessary_function {
        diagnostics.extend(unnecessary_function::check(source));
    }
    if rules.unknown_actor_value {
        diagnostics.extend(actor_value::check(source));
    }
    if rules.repeated_setoutfit {
        diagnostics.extend(repeated_setoutfit::check(source));
    }
    if rules.missing_doc_comment {
        diagnostics.extend(missing_doc_comment::check(source));
    }
    if rules.float_equality {
        diagnostics.extend(float_equality::check(source));
    }
    if rules.missing_update_handler {
        diagnostics.extend(missing_update_handler::check(source));
    }
    if rules.unused_import {
        diagnostics.extend(unused_import::check_with(source, external));
    }
    if rules.event_signature_mismatch {
        diagnostics.extend(event_signature::check(source));
    }
    diagnostics
}

pub fn apply_repairs(source: &str, config: &Config, applies: impl Fn(&str) -> bool) -> String {
    let rules = &config.rules;
    let source = if rules.identifier_casing && applies(identifier_casing::RULE) {
        identifier_casing::repair(source, config.identifier_casing)
    } else {
        source.to_string()
    };
    let source = if rules.slow_functions && applies(slow_functions::RULE) {
        slow_functions::repair(&source)
    } else {
        source
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
    let source = if rules.assignment_operator_spacing && applies(assignment_operator_spacing::RULE)
    {
        assignment_operator_spacing::repair(&source)
    } else {
        source
    };
    let source = if rules.type_casing && applies(type_casing::RULE) {
        type_casing::repair(&source, config.type_casing)
    } else {
        source
    };
    let source = if rules.trailing_whitespace && applies(trailing_whitespace::RULE) {
        trailing_whitespace::repair(&source)
    } else {
        source
    };
    let source = if rules.global_variable_increment && applies(global_variable_increment::RULE) {
        global_variable_increment::repair(&source)
    } else {
        source
    };
    let source = if rules.named_arguments && applies(named_arguments::RULE) {
        named_arguments::repair(&source, config.named_arguments)
    } else {
        source
    };
    if rules.unnecessary_function && applies(unnecessary_function::RULE) {
        unnecessary_function::repair(&source)
    } else {
        source
    }
}
