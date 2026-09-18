use super::LintVisitor;

fn assert_ast(visitor: LintVisitor) {
    assert!(matches!(visitor, LintVisitor::Ast(_)));
}

fn assert_tokens(visitor: LintVisitor) {
    assert!(matches!(visitor, LintVisitor::Tokens(_)));
}

#[test]
#[allow(clippy::too_many_lines)]
fn ast_rules_return_an_ast_visitor() {
    assert_ast(crate::argument_naming::visitor());
    assert_ast(crate::argument_override_types::visitor());
    assert_ast(crate::argument_types::visitor());
    assert_ast(crate::array_bounds::visitor());
    assert_ast(crate::array_size_range::visitor());
    assert_ast(crate::circular_dependency::visitor());
    assert_ast(crate::cyclomatic_complexity::visitor());
    assert_ast(crate::default_property_value::visitor());
    assert_ast(crate::division_by_zero::visitor());
    assert_ast(crate::empty_body::visitor());
    assert_ast(crate::event_signature::visitor());
    assert_ast(crate::explicit_return::visitor());
    assert_ast(crate::float_equality::visitor());
    assert_ast(crate::float_int_conversion::visitor());
    assert_ast(crate::function_override::visitor());
    assert_ast(crate::get_state_comparison::visitor());
    assert_ast(crate::global_variable_increment::visitor());
    assert_ast(crate::global_variable_setvalue::visitor());
    assert_ast(crate::goto_state::visitor());
    assert_ast(crate::identifier_casing::visitor());
    assert_ast(crate::impossible_cast::visitor());
    assert_ast(crate::int_division_to_float::visitor());
    assert_ast(crate::invalid_random_range::visitor());
    assert_ast(crate::invariant_loop_condition::visitor());
    assert_ast(crate::local_variable_shadowing::visitor());
    assert_ast(crate::magic_numbers::visitor());
    assert_ast(crate::missing_doc_comment::visitor());
    assert_ast(crate::multiple_auto_states::visitor());
    assert_ast(crate::named_arguments::visitor());
    assert_ast(crate::native_function_usage::visitor());
    assert_ast(crate::non_global_function_call::visitor());
    assert_ast(crate::none_form_usage::visitor());
    assert_ast(crate::numeric_comparison::visitor());
    assert_ast(crate::parameter_reassignment::visitor());
    assert_ast(crate::property_sorting::visitor());
    assert_ast(crate::readonly_property_write::visitor());
    assert_ast(crate::repeated_getvalue::visitor());
    assert_ast(crate::repeated_setoutfit::visitor());
    assert_ast(crate::return_types::visitor());
    assert_ast(crate::script_name_collision::visitor());
    assert_ast(crate::self_assignment::visitor());
    assert_ast(crate::setvalue_in_loop::visitor());
    assert_ast(crate::short_wait_interval::visitor());
    assert_ast(crate::state_function_signature::visitor());
    assert_ast(crate::static_condition::visitor());
    assert_ast(crate::static_function_call_via_instance::visitor());
    assert_ast(crate::strict_boolean::visitor());
    assert_ast(crate::too_many_states::visitor());
    assert_ast(crate::unchecked_array_element::visitor());
    assert_ast(crate::unchecked_cast::visitor());
    assert_ast(crate::unchecked_form_parameter::visitor());
    assert_ast(crate::unguarded_self_recursion::visitor());
    assert_ast(crate::unnecessary_function::visitor());
    assert_ast(crate::unreachable_elseif::visitor());
    assert_ast(crate::unreachable_statement::visitor());
    assert_ast(crate::unresolved_script::visitor());
    assert_ast(crate::unused_import::visitor());
    assert_ast(crate::unused_local_variable::visitor());
    assert_ast(crate::useless_downcast::visitor());
    assert_ast(crate::variable_used_before_assignment::visitor());
}

#[test]
fn token_rules_return_a_token_visitor() {
    assert_tokens(crate::assignment_operator_spacing::visitor());
    assert_tokens(crate::chain_whitespace::visitor());
    assert_tokens(crate::comma_spacing::visitor());
    assert_tokens(crate::debug_side_effects::visitor());
    assert_tokens(crate::exclamation_spacing::visitor());
    assert_tokens(crate::forbidden_functions::visitor());
    assert_tokens(crate::formid_hex_notation::visitor());
    assert_tokens(crate::get_form_from_file_skyrim_esm::visitor());
    assert_tokens(crate::indentation::visitor());
    assert_tokens(crate::missing_update_handler::visitor());
    assert_tokens(crate::operator_spacing::visitor());
    assert_tokens(crate::slow_functions::visitor());
    assert_tokens(crate::type_casing::visitor());
    assert_tokens(crate::actor_value::visitor());
    assert_tokens(crate::unused_getter::visitor());
    assert_tokens(crate::unused_nodiscard::visitor());
    assert_tokens(crate::unused_property::visitor());
}
