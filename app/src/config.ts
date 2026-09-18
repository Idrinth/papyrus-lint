import { invoke } from "@tauri-apps/api/core";
import { markLintResultsStale } from "./drop";
import { configPathOverride, currentProjectDir } from "./project";

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

let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let typeCasingStyleEl: HTMLSelectElement | null;
let identifierCasingStyleEl: HTMLSelectElement | null;
let namedArgumentsStyleEl: HTMLSelectElement | null;
let magicNumbersModeEl: HTMLSelectElement | null;
let semicolonStyleEl: HTMLSelectElement | null;
let cyclomaticComplexityWarningEl: HTMLInputElement | null;
let cyclomaticComplexityErrorEl: HTMLInputElement | null;
let minWaitIntervalEl: HTMLInputElement | null;
let failOnWarningEl: HTMLInputElement | null;
let failOnInfoEl: HTMLInputElement | null;
let boolLikeIntEl: HTMLInputElement | null;
let assumeAutoPropertiesFilledEl: HTMLInputElement | null;
let ruleEls: Partial<Record<keyof LintRules, HTMLInputElement>> = {};

// Looks for a papyrus-lint YAML config file in `dir`, falling back to the
// default configuration if none is found.
export async function loadLintConfig(dir: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config", { dir });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

// Persists `config` to `dir`'s papyrus-lint YAML config file so the
// formatting selected in the UI is remembered for next time.
export async function saveLintConfig(dir: string, config: LintConfig): Promise<void> {
  try {
    await invoke("save_lint_config", { dir, config });
  } catch (error) {
    console.error(error);
  }
}

// Reads and parses the config file at the exact `path` given, bypassing the
// project-directory discovery loadLintConfig does. Backs the Settings tab's
// "Configuration file" override.
export async function loadLintConfigFromPath(path: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config_from_path", { path });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

// Persists `config` to the exact file at `path`, creating it if it doesn't
// exist yet. The save-side counterpart of loadLintConfigFromPath, used
// while the Settings tab's "Configuration file" override is set.
export async function saveLintConfigToPath(path: string, config: LintConfig): Promise<void> {
  try {
    await invoke("save_lint_config_to_path", { path, config });
  } catch (error) {
    console.error(error);
  }
}

// Reflects `config` onto the formatting controls without firing their
// `change` listeners (assigning `.value` does not dispatch `change`).
export function applyLintConfigToUI(config: LintConfig) {
  if (semicolonStyleEl) {
    semicolonStyleEl.value = config.semicolon ? "require" : "forbid";
  }
  if (indentationStyleEl) {
    indentationStyleEl.value = config.indentation === "space" ? "spaces" : "tabs";
  }
  if (indentationWidthEl) {
    indentationWidthEl.value = String(config.indentation_width);
    indentationWidthEl.disabled = config.indentation !== "space";
  }
  if (cyclomaticComplexityWarningEl) {
    cyclomaticComplexityWarningEl.value = String(config.cyclomatic_complexity_warning);
  }
  if (cyclomaticComplexityErrorEl) {
    cyclomaticComplexityErrorEl.value = String(config.cyclomatic_complexity_error);
  }
  if (minWaitIntervalEl) {
    minWaitIntervalEl.value = String(config.min_wait_interval);
  }
  if (typeCasingStyleEl) {
    typeCasingStyleEl.value = config.type_casing;
  }
  if (identifierCasingStyleEl) {
    identifierCasingStyleEl.value = config.identifier_casing;
  }
  if (namedArgumentsStyleEl) {
    namedArgumentsStyleEl.value = config.named_arguments;
  }
  if (magicNumbersModeEl) {
    magicNumbersModeEl.value = config.magic_numbers;
  }
  if (failOnWarningEl) {
    failOnWarningEl.checked = config.fail_on_warning;
  }
  if (failOnInfoEl) {
    failOnInfoEl.checked = config.fail_on_info;
  }
  if (boolLikeIntEl) {
    boolLikeIntEl.checked = config.bool_like_int;
  }
  if (assumeAutoPropertiesFilledEl) {
    assumeAutoPropertiesFilledEl.checked = config.assume_auto_properties_filled;
  }
  for (const key of RULE_KEYS) {
    const el = ruleEls[key];
    if (el) {
      el.checked = config.rules[key];
    }
  }
}

// Reads the formatting controls' current values into a LintConfig.
export function lintConfigFromUI(): LintConfig {
  const indentation = indentationStyleEl?.value === "spaces" ? "space" : "tab";
  const cyclomaticComplexityWarning = Math.max(
    1,
    cyclomaticComplexityWarningEl?.valueAsNumber || 10,
  );
  const rules = { ...DEFAULT_RULES };
  for (const key of RULE_KEYS) {
    rules[key] = ruleEls[key]?.checked ?? DEFAULT_RULES[key];
  }
  return {
    semicolon: semicolonStyleEl?.value === "require",
    indentation,
    indentation_width: Math.min(16, Math.max(1, indentationWidthEl?.valueAsNumber || 4)),
    identifier_casing:
      (identifierCasingStyleEl?.value as IdentifierCasingStyle | undefined) ?? "PascalCase",
    cyclomatic_complexity_warning: cyclomaticComplexityWarning,
    // Never below the warning threshold: an error severity that kicks in
    // before the warning one would make the two settings contradict each
    // other.
    cyclomatic_complexity_error: Math.max(
      cyclomaticComplexityWarning,
      cyclomaticComplexityErrorEl?.valueAsNumber || 20,
    ),
    type_casing: (typeCasingStyleEl?.value as TypeCasingStyle | undefined) ?? "PascalCase",
    named_arguments: (namedArgumentsStyleEl?.value as NamedArgumentsStyle | undefined) ?? "never",
    min_wait_interval: Math.max(
      0,
      minWaitIntervalEl && Number.isFinite(minWaitIntervalEl.valueAsNumber)
        ? minWaitIntervalEl.valueAsNumber
        : 0.1,
    ),
    magic_numbers: (magicNumbersModeEl?.value as MagicNumbersMode | undefined) ?? "loose",
    fail_on_warning: failOnWarningEl?.checked ?? false,
    fail_on_info: failOnInfoEl?.checked ?? false,
    bool_like_int: boolLikeIntEl?.checked ?? true,
    assume_auto_properties_filled: assumeAutoPropertiesFilledEl?.checked ?? false,
    rules,
  };
}

// Called whenever a formatting control changes: updates the in-memory
// config and, if a project directory is known, persists it to disk.
export function handleLintConfigChanged() {
  currentLintConfig = lintConfigFromUI();
  markLintResultsStale();
  const override = configPathOverride();
  if (override) {
    void saveLintConfigToPath(override, currentLintConfig);
  } else if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

// Loads `dir`'s lint configuration (or the `overridePath` file instead, if
// set) into currentLintConfig and reflects it onto the Settings tab. The
// project.ts counterpart to handleLintConfigChanged: called by useProjectDir
// when a project is (re)loaded, rather than in response to editing a
// formatting control.
export async function loadAndApplyLintConfig(dir: string, overridePath: string): Promise<void> {
  currentLintConfig = overridePath ? await loadLintConfigFromPath(overridePath) : await loadLintConfig(dir);
  applyLintConfigToUI(currentLintConfig);
}

export function bindConfigSettings() {
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");
  typeCasingStyleEl = document.querySelector("#type-casing-style");
  identifierCasingStyleEl = document.querySelector("#identifier-casing-style");
  namedArgumentsStyleEl = document.querySelector("#named-arguments-style");
  magicNumbersModeEl = document.querySelector("#magic-numbers-mode");
  cyclomaticComplexityWarningEl = document.querySelector("#cyclomatic-complexity-warning");
  cyclomaticComplexityErrorEl = document.querySelector("#cyclomatic-complexity-error");
  minWaitIntervalEl = document.querySelector("#min-wait-interval");
  failOnWarningEl = document.querySelector("#fail-on-warning");
  failOnInfoEl = document.querySelector("#fail-on-info");
  boolLikeIntEl = document.querySelector("#bool-like-int");
  assumeAutoPropertiesFilledEl = document.querySelector("#assume-auto-properties-filled");
  ruleEls = Object.fromEntries(
    RULE_KEYS.map((key) => [key, document.querySelector<HTMLInputElement>(`#rule-${key}`)]),
  ) as Partial<Record<keyof LintRules, HTMLInputElement>>;

  semicolonStyleEl?.addEventListener("change", handleLintConfigChanged);
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
    handleLintConfigChanged();
  });
  indentationWidthEl?.addEventListener("change", handleLintConfigChanged);
  typeCasingStyleEl?.addEventListener("change", handleLintConfigChanged);
  identifierCasingStyleEl?.addEventListener("change", handleLintConfigChanged);
  namedArgumentsStyleEl?.addEventListener("change", handleLintConfigChanged);
  magicNumbersModeEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityWarningEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityErrorEl?.addEventListener("change", handleLintConfigChanged);
  minWaitIntervalEl?.addEventListener("change", handleLintConfigChanged);
  failOnWarningEl?.addEventListener("change", handleLintConfigChanged);
  failOnInfoEl?.addEventListener("change", handleLintConfigChanged);
  boolLikeIntEl?.addEventListener("change", handleLintConfigChanged);
  assumeAutoPropertiesFilledEl?.addEventListener("change", handleLintConfigChanged);
  for (const key of RULE_KEYS) {
    ruleEls[key]?.addEventListener("change", handleLintConfigChanged);
  }
}
