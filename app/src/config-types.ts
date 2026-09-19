export type TypeCasingStyle = "PascalCase" | "camelCase" | "lowercase" | "UPPERCASE";
export type IdentifierCasingStyle = "camelCase" | "PascalCase" | "snake_case" | "CONSTANT_CASE";
export type NamedArgumentsStyle = "always" | "instead_of_defaults" | "never";
export type MagicNumbersMode = "loose" | "strict";

export interface LintRules {
  trailing_whitespace: boolean;
  comma_spacing: boolean;
  forbidden_functions: boolean;
  formid_hex_notation: boolean;
  slow_functions: boolean;
  unused_getter: boolean;
  unused_nodiscard: boolean;
  unused_property: boolean;
  semicolon: boolean;
  float_int_conversion: boolean;
  int_division_to_float: boolean;
  strict_boolean: boolean;
  argument_types: boolean;
  return_types: boolean;
  function_override: boolean;
  argument_naming: boolean;
  argument_override_types: boolean;
  numeric_comparison: boolean;
  indentation: boolean;
  cyclomatic_complexity: boolean;
  unreachable_statement: boolean;
  static_condition: boolean;
  division_by_zero: boolean;
  empty_body: boolean;
  unused_local_variable: boolean;
  variable_used_before_assignment: boolean;
  none_form_usage: boolean;
  local_variable_shadowing: boolean;
  parameter_reassignment: boolean;
  chain_whitespace: boolean;
  exclamation_spacing: boolean;
  identifier_casing: boolean;
  type_casing: boolean;
  named_arguments: boolean;
  operator_spacing: boolean;
  property_sorting: boolean;
  explicit_return: boolean;
  unchecked_form_parameter: boolean;
  unchecked_array_element: boolean;
  unchecked_cast: boolean;
  useless_downcast: boolean;
  impossible_cast: boolean;
  unresolved_script: boolean;
  non_global_function_call: boolean;
  static_function_call_via_instance: boolean;
  short_wait_interval: boolean;
  state_function_signature: boolean;
  goto_state: boolean;
  too_many_states: boolean;
  multiple_auto_states: boolean;
  conflicting_script_versions: boolean;
  stale_compiled_output: boolean;
  script_filename_mismatch: boolean;
  unused_disable: boolean;
  magic_numbers: boolean;
  native_function_usage: boolean;
  repeated_getvalue: boolean;
  global_variable_setvalue: boolean;
  global_variable_increment: boolean;
  setvalue_in_loop: boolean;
  invariant_loop_condition: boolean;
  script_name_collision: boolean;
  array_bounds: boolean;
  readonly_property_write: boolean;
  default_property_value: boolean;
  unguarded_self_recursion: boolean;
  self_assignment: boolean;
  debug_side_effects: boolean;
  unknown_actor_value: boolean;
}

export interface LintConfig {
  semicolon: boolean;
  indentation: "tab" | "space";
  indentation_width: number;
  identifier_casing: IdentifierCasingStyle;
  cyclomatic_complexity_warning: number;
  cyclomatic_complexity_error: number;
  type_casing: TypeCasingStyle;
  named_arguments: NamedArgumentsStyle;
  min_wait_interval: number;
  magic_numbers: MagicNumbersMode;
  fail_on_warning: boolean;
  fail_on_info: boolean;
  bool_like_int: boolean;
  assume_auto_properties_filled: boolean;
  rules: LintRules;
}

export const DEFAULT_RULES: LintRules = {
  trailing_whitespace: true,
  comma_spacing: true,
  forbidden_functions: true,
  formid_hex_notation: true,
  slow_functions: true,
  unused_getter: true,
  unused_nodiscard: true,
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
  readonly_property_write: true,
  default_property_value: false,
  unguarded_self_recursion: true,
  self_assignment: true,
  debug_side_effects: true,
  unknown_actor_value: false,
};

export const DEFAULT_LINT_CONFIG: LintConfig = {
  semicolon: false,
  indentation: "tab",
  indentation_width: 4,
  identifier_casing: "PascalCase",
  cyclomatic_complexity_warning: 10,
  cyclomatic_complexity_error: 20,
  type_casing: "PascalCase",
  named_arguments: "never",
  min_wait_interval: 0.1,
  magic_numbers: "loose",
  fail_on_warning: false,
  fail_on_info: false,
  bool_like_int: true,
  assume_auto_properties_filled: false,
  rules: DEFAULT_RULES,
};

export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];

export let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;

export function setCurrentLintConfig(config: LintConfig) {
  currentLintConfig = config;
}
