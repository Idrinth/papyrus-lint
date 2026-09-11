import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { highlightPapyrusLines } from "./highlight";
import {
  type CompletionQuery,
  type Member,
  completionInsertText,
  completionLabel,
  completionQueryAt,
  filterMembers,
} from "./autocomplete";

let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
let pscResultEl: HTMLElement | null;
let pscResultListEl: HTMLElement | null;
let pscResultMassFixEl: HTMLElement | null;
let pscResultMassFixListEl: HTMLElement | null;
let lintProgressEl: HTMLElement | null;
let lintProgressLabelEl: HTMLElement | null;
let lintProgressBarEl: HTMLProgressElement | null;
let lintProgressHideTimer: ReturnType<typeof setTimeout> | null = null;
let filenameFilterEl: HTMLInputElement | null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let typeCasingStyleEl: HTMLSelectElement | null;
let identifierCasingStyleEl: HTMLSelectElement | null;
let namedArgumentsStyleEl: HTMLSelectElement | null;
let magicNumbersModeEl: HTMLSelectElement | null;
let currentPscOutcomes: PscParseOutcome[] = [];
// Set whenever a setting affecting lint output (formatting/rule config,
// compiler path, compile-check toggle, additional script roots, or the
// configuration file override) changes after currentPscOutcomes was last
// populated, so a currently showing lint results list no longer reflects
// the active settings. Checked by the Lint results tab button so switching
// to it re-lints the same files instead of silently showing stale findings.
let lintResultsStale = false;
// Bumped by handleDroppedPaths every time a new drop starts parsing/linting;
// a still-running drop's parsePscFiles callback checks its own snapshot of
// this against the current value before touching currentPscOutcomes, so a
// straggling outcome from a drop superseded by a newer one can't get mixed
// into the newer drop's results.
let currentParseGeneration = 0;
let configPathOverrideEl: HTMLInputElement | null;
let compilerPathEl: HTMLInputElement | null;
let compileCheckEl: HTMLInputElement | null;
let scriptRootsEl: HTMLTextAreaElement | null;
let detectedScriptRootsEl: HTMLOutputElement | null;
let usedConfigurationFileEl: HTMLOutputElement | null;
let semicolonStyleEl: HTMLSelectElement | null;
let cyclomaticComplexityWarningEl: HTMLInputElement | null;
let cyclomaticComplexityErrorEl: HTMLInputElement | null;
let minWaitIntervalEl: HTMLInputElement | null;
let failOnWarningEl: HTMLInputElement | null;
let failOnInfoEl: HTMLInputElement | null;
let boolLikeIntEl: HTMLInputElement | null;
let assumeAutoPropertiesFilledEl: HTMLInputElement | null;
let ruleEls: Partial<Record<keyof LintRules, HTMLInputElement>> = {};
let autoFixableFilterEl: HTMLInputElement | null;
// The per-tag-kind "Filter by rule" multiselects (one per TagKind; see
// populateRuleFilterGroups/wireRuleFilterGroups below), replacing what used
// to be a single flat multiselect spanning every rule.
let ruleFilterSelectEls: Partial<Record<TagKind, HTMLSelectElement>> = {};
let exportFormatEl: HTMLSelectElement | null;
let exportIssuesButtonEl: HTMLButtonElement | null;
let codeViewerEl: HTMLDialogElement | null;
let codeViewerTitleEl: HTMLElement | null;
let codeViewerCloseEl: HTMLButtonElement | null;
let codeViewerViewEl: HTMLElement | null;
let codeViewerEditEl: HTMLElement | null;
let codeViewerEditGutterEl: HTMLElement | null;
let codeViewerEditHighlightEl: HTMLElement | null;
let codeViewerEditTextareaEl: HTMLTextAreaElement | null;
let codeViewerEditButtonEl: HTMLButtonElement | null;
let codeViewerFixButtonEl: HTMLButtonElement | null;
let codeViewerSaveButtonEl: HTMLButtonElement | null;
let codeViewerSaveCompileButtonEl: HTMLButtonElement | null;
let codeViewerCancelButtonEl: HTMLButtonElement | null;
let codeViewerCompileOutputEl: HTMLElement | null;
let codeViewerFullscreenEl: HTMLButtonElement | null;
let codeViewerAutocompleteEl: HTMLUListElement | null;
let themeSelectEl: HTMLSelectElement | null;
let saveConfigAsPresetButtonEl: HTMLButtonElement | null;
let settingsFieldsetEl: HTMLFieldSetElement | null;
let settingsLockedNoticeEl: HTMLElement | null;
let configPickerEl: HTMLDialogElement | null;
let configPickerDetectedEl: HTMLElement | null;
let configPickerDetectedPathEl: HTMLElement | null;
let configPickerNoneEl: HTMLElement | null;
let configPickerPresetListEl: HTMLElement | null;
let configPickerPathInputEl: HTMLInputElement | null;
let configPickerUsePathButtonEl: HTMLButtonElement | null;
let configPickerContinueEl: HTMLButtonElement | null;
let presetManagementTabEl: HTMLButtonElement | null;
let presetManagementListEl: HTMLElement | null;

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

export const TAB_IDS = ["import", "settings", "presets", "files", "lint", "contact"] as const;
type TabId = (typeof TAB_IDS)[number];

// Shows `tab`'s panel and hides the others, updating the tab buttons'
// aria-selected/active state to match.
export function switchTab(tab: TabId) {
  for (const id of TAB_IDS) {
    const button = document.querySelector<HTMLButtonElement>(`#tab-${id}`);
    const panel = document.querySelector<HTMLElement>(`#panel-${id}`);
    const active = id === tab;
    button?.setAttribute("aria-selected", String(active));
    button?.classList.toggle("tabs__tab--active", active);
    if (panel) {
      panel.hidden = !active;
    }
  }
}

export function isAchlistPath(path: string): boolean {
  return path.toLowerCase().endsWith(ACHLIST_EXTENSION);
}

export function isPscPath(path: string): boolean {
  return path.toLowerCase().endsWith(PSC_EXTENSION);
}

export interface PapyrusScript {
  name: string;
}

export interface Diagnostic {
  line: number;
  column: number;
  message: string;
  // The lint rule that raised this finding (e.g. "trailing-whitespace"),
  // matching papyrus_lints::Diagnostic::rule. Optional here since not every
  // test fixture needs one; the backend always sends it.
  rule?: string;
}

export interface PscParseOutcome {
  path: string;
  ok: boolean;
  detail: string;
  findings: Diagnostic[];
}

// Mirrors papyrus_lints::tags::Importance's lowercase serde rename.
export type TagImportance = "low" | "medium" | "high";
export const TAG_IMPORTANCES: TagImportance[] = ["low", "medium", "high"];

// The kind keyword(s) papyrus_lints::tags currently tags every rule with.
// Kept in sync by hand with the "kinds" used across RULE_TAGS in
// app/crates/papyrus-lints/src/tags.rs, the same convention FIXABLE_RULE_IDS
// below follows.
export type TagKind = "style" | "performance" | "correctness" | "maintainability";
export const TAG_KINDS: TagKind[] = ["style", "performance", "correctness", "maintainability"];

// One rule's tag metadata, as returned by the backend's list_rule_tags
// command (papyrus_lints::tags::RuleTags, made JSON-friendly).
export interface RuleTagsInfo {
  rule: string;
  kinds: string[];
  importance: TagImportance;
  auto_fixable: boolean;
}

export interface CompileOutcome {
  success: boolean;
  stdout: string;
  stderr: string;
  personal_data_stripped: boolean;
}

export interface ProjectInfo {
  detected_script_roots: string[];
  used_configuration_file: string | null;
}

// One configuration preset's identity/description — a built-in one, or a
// user preset found under a presets directory next to the executable — as
// returned by the backend's list_config_presets command
// (papyrus_lint_core::presets::PresetInfo, made JSON-friendly). Offered
// inline in the config-picker dialog (see promptForConfigSelection) for a
// project directory that has no papyrus-lint.yaml/.yml of its own yet.
export interface ConfigPreset {
  id: string;
  label: string;
  description: string;
}

// What promptForConfigSelection resolved to (see useProjectDir): stick with
// whatever useProjectDir's own auto-detection would already do ("detected" —
// the project's existing papyrus-lint.yaml/.yml, or the engine's silent
// defaults if it has none), point at a specific configuration file instead
// ("path"), or seed a fresh one from a preset ("preset", handled the same
// way applyConfigPreset already is elsewhere).
export type ConfigSelectionResult =
  | { kind: "detected" }
  | { kind: "path"; path: string }
  | { kind: "preset"; preset: string };

export interface LintRules {
  trailing_whitespace: boolean;
  comma_spacing: boolean;
  forbidden_functions: boolean;
  formid_hex_notation: boolean;
  slow_functions: boolean;
  unused_getter: boolean;
  unused_property: boolean;
  semicolon: boolean;
  float_int_conversion: boolean;
  int_division_to_float: boolean;
  strict_boolean: boolean;
  argument_types: boolean;
  return_types: boolean;
  function_override: boolean;
  argument_naming: boolean;
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
  unchecked_cast: boolean;
  unresolved_script: boolean;
  non_global_function_call: boolean;
  static_function_call_via_instance: boolean;
  short_wait_interval: boolean;
  state_function_signature: boolean;
  goto_state: boolean;
  too_many_states: boolean;
  multiple_auto_states: boolean;
  conflicting_script_versions: boolean;
  unused_disable: boolean;
  magic_numbers: boolean;
  native_function_usage: boolean;
  repeated_getvalue: boolean;
  global_variable_setvalue: boolean;
  invariant_loop_condition: boolean;
  script_name_collision: boolean;
  array_bounds: boolean;
  default_property_value: boolean;
}

export type TypeCasingStyle = "PascalCase" | "camelCase" | "lowercase" | "UPPERCASE";
export type IdentifierCasingStyle = "camelCase" | "PascalCase" | "snake_case" | "CONSTANT_CASE";
export type NamedArgumentsStyle = "always" | "instead_of_defaults" | "never";
export type MagicNumbersMode = "loose" | "strict";

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
  unused_property: true,
  semicolon: true,
  float_int_conversion: true,
  int_division_to_float: true,
  strict_boolean: true,
  argument_types: true,
  return_types: true,
  function_override: true,
  argument_naming: true,
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
  unchecked_cast: true,
  unresolved_script: true,
  non_global_function_call: true,
  static_function_call_via_instance: true,
  short_wait_interval: true,
  state_function_signature: true,
  goto_state: true,
  too_many_states: true,
  multiple_auto_states: true,
  conflicting_script_versions: true,
  unused_disable: false,
  magic_numbers: false,
  native_function_usage: false,
  repeated_getvalue: false,
  global_variable_setvalue: false,
  invariant_loop_condition: true,
  script_name_collision: true,
  array_bounds: true,
  default_property_value: false,
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
const LAST_PROJECT_DIR_KEY = "papyrus-lint:last-project-dir";
const THEME_KEY = "papyrus-lint:theme";
export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];

export type Theme = "system" | "light" | "dark";
const THEMES: Theme[] = ["system", "light", "dark"];

let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;
// The project root (see projectDirForAchlist/projectDirForPscPath), also
// used by the "Argument type check" lint to resolve calls to functions
// declared on other scripts under it.
let currentProjectDir: string | null = null;
// The PapyrusCompiler.exe path to use for the "Compile" button, kept in
// sync with the Settings tab's input (see handleCompilerPathChanged).
let currentCompilerPath = "";
// Whether linting also runs PapyrusCompiler.exe against a dropped .psc,
// kept in sync with the Settings tab's checkbox (see
// handleCompileCheckChanged).
let currentCompileCheck = false;
// Extra directories (besides scripts/source and source/scripts under the
// project root) to search for .psc files when resolving cross-script
// lookups, kept in sync with the Settings tab's textarea (see
// handleScriptRootsChanged).
let currentScriptRoots: string[] = [];
// Source directories inferred from the entries in the currently loaded
// achlist. These are runtime-only roots: unlike currentScriptRoots, they are
// not displayed as user configuration or persisted to papyrus-lint.yaml.
let currentAchlistScriptRoots: string[] = [];
// Every built-in lint rule's tag metadata, keyed by rule id, fetched once
// from the backend (see loadRuleTags) and used both to render each
// finding's tag badges and to drive the tag filters below.
let ruleTagsByRule: Map<string, RuleTagsInfo> = new Map();

function effectiveScriptRoots(): string[] {
  return [...new Set([...currentScriptRoots, ...currentAchlistScriptRoots])];
}

export function scriptRootsForAchlist(entries: string[]): string[] {
  return [...new Set(entries.filter(isPscPath).map(dirnameOf))];
}

export function dirnameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(0, index);
}

function basenameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(index + 1);
}

// `path` itself, followed by each of its ancestor directories up to the
// root (where dirnameOf stops changing anything), mirroring Rust's
// `Path::ancestors()`.
function ancestorsOf(path: string): string[] {
  const ancestors = [path];
  let current = path;
  for (;;) {
    const parent = dirnameOf(current);
    if (parent === current) {
      return ancestors;
    }
    ancestors.push(parent);
    current = parent;
  }
}

// (outer, inner) pairs, mirroring papyrus-lint-cli's `CANDIDATE_DIRS`
// (`scripts/source`, `source/scripts`).
const CANDIDATE_DIR_PAIRS: readonly (readonly [string, string])[] = [
  ["scripts", "source"],
  ["source", "scripts"],
];

// Mirrors papyrus-lint-cli's `find_candidate_pair_root`: walks up `path`'s
// ancestors looking for a `scripts/source`/`source/scripts` directory pair
// (matched case-insensitively), and returns the directory above that pair,
// or null if no such pair appears anywhere in `path`'s ancestry.
export function findCandidatePairRoot(path: string): string | null {
  const ancestors = ancestorsOf(path);
  for (let i = 1; i < ancestors.length - 1; i++) {
    const innerName = basenameOf(ancestors[i]).toLowerCase();
    const outerName = basenameOf(ancestors[i + 1]).toLowerCase();
    const matches = CANDIDATE_DIR_PAIRS.some(([outer, inner]) => outer === outerName && inner === innerName);
    if (matches) {
      return dirnameOf(ancestors[i + 1]);
    }
  }
  return null;
}

// Finds the project root for a dropped `.achlist`: tries each of its
// resolved `.psc` entries' own position under a `scripts/source`/
// `source/scripts` directory pair first (see findCandidatePairRoot), so a
// project whose `.achlist` doesn't live in the project root itself (e.g. it
// was dropped next to a game's `Data` directory while the project lives in
// a subfolder) still resolves correctly. Falls back to the achlist's own
// parent directory (the conventional layout) if none of its entries match.
export function projectDirForAchlist(achlistPath: string, entries: string[]): string {
  for (const entry of entries) {
    if (!isPscPath(entry)) {
      continue;
    }
    const root = findCandidatePairRoot(entry);
    if (root) {
      return root;
    }
  }
  return dirnameOf(achlistPath);
}

// Finds the project root for a dropped directory (see handleDroppedPaths'
// directory-scan mode, for a project with no .achlist at all whose scripts
// are spread across arbitrarily nested subfolders, e.g. Requiem's own
// layout): tries each recursively-found .psc entry's own position under a
// `scripts/source`/`source/scripts` directory pair first (see
// findCandidatePairRoot), the same way projectDirForAchlist does for an
// achlist's entries. Falls back to the dropped directory itself if none of
// the entries match that layout, since there's no achlist file whose parent
// directory would otherwise apply.
export function projectDirForDirectory(dirPath: string, entries: string[]): string {
  for (const entry of entries) {
    const root = findCandidatePairRoot(entry);
    if (root) {
      return root;
    }
  }
  return dirPath;
}

// Formats `path` relative to `base` (the project root; see
// projectDirForAchlist/projectDirForPscPath) for display in the lint
// results list, so long absolute paths stay readable. Falls back to the
// absolute path if `base` isn't known yet or `path` doesn't live under it.
export function relativePath(path: string, base: string | null): string {
  if (!base) {
    return path;
  }
  for (const sep of ["/", "\\"]) {
    const prefix = base.endsWith(sep) ? base : `${base}${sep}`;
    if (path.startsWith(prefix)) {
      return path.slice(prefix.length);
    }
  }
  return path;
}

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

// Returns the PapyrusCompiler.exe path to use for `dir`'s project: an
// explicit override saved to its papyrus-lint config file, or, absent
// one, a path auto-detected at `../Papyrus Compiler/PapyrusCompiler.exe`
// relative to `dir`. Returns an empty string if neither is available or
// the lookup fails.
export async function loadCompilerPath(dir: string): Promise<string> {
  try {
    return (await invoke<string | null>("load_compiler_path", { dir })) ?? "";
  } catch (error) {
    console.error(error);
    return "";
  }
}

// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
// papyrus-lint config file. Passing an empty string clears the override,
// reverting to auto-detection.
export async function saveCompilerPath(dir: string, path: string): Promise<void> {
  try {
    await invoke("save_compiler_path", { dir, path });
  } catch (error) {
    console.error(error);
  }
}

// Returns whether `dir`'s project runs PapyrusCompiler.exe against a
// dropped .psc as part of linting it. Returns false if the lookup fails.
export async function loadCompileCheck(dir: string): Promise<boolean> {
  try {
    return await invoke<boolean>("load_compile_check", { dir });
  } catch (error) {
    console.error(error);
    return false;
  }
}

// Persists whether `dir`'s project runs PapyrusCompiler.exe against a
// dropped .psc as part of linting it.
export async function saveCompileCheck(dir: string, enabled: boolean): Promise<void> {
  try {
    await invoke("save_compile_check", { dir, enabled });
  } catch (error) {
    console.error(error);
  }
}

// Returns `dir`'s configured additional script root directories, if any.
// Returns an empty array if none are configured or the lookup fails.
export async function loadScriptRoots(dir: string): Promise<string[]> {
  try {
    return await invoke<string[]>("load_script_roots", { dir });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export async function loadProjectInfo(dir: string): Promise<ProjectInfo> {
  try {
    return await invoke<ProjectInfo>("load_project_info", { dir });
  } catch (error) {
    console.error(error);
    return { detected_script_roots: [], used_configuration_file: null };
  }
}

export function applyProjectInfoToUI(info: ProjectInfo) {
  if (detectedScriptRootsEl) {
    detectedScriptRootsEl.textContent = info.detected_script_roots.length
      ? info.detected_script_roots.join("\n")
      : "None detected";
  }
  if (usedConfigurationFileEl) {
    usedConfigurationFileEl.textContent = info.used_configuration_file ?? "None (using defaults)";
  }
}

// Fetches every configuration preset's identity/description — built-in
// plus any user preset (see ConfigPreset) — for the config-picker dialog's
// inline preset list (see promptForConfigSelection). Returns an empty
// array if the lookup fails, which promptForConfigSelection treats the
// same as "nothing to offer" and hides that list entirely.
export async function loadConfigPresets(): Promise<ConfigPreset[]> {
  try {
    return (await invoke<ConfigPreset[]>("list_config_presets")) ?? [];
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Seeds `dir`'s papyrus-lint config file from the named preset (built-in
// or user). Only called right after promptForConfigSelection resolves with
// a "preset" choice, while `dir` is still known to have no config file of
// its own.
export async function applyConfigPreset(dir: string, preset: string): Promise<void> {
  try {
    await invoke("apply_config_preset", { dir, preset });
  } catch (error) {
    console.error(error);
  }
}

// Prompts for a name and saves the Settings tab's currently edited lint
// configuration (currentLintConfig, kept in sync by handleLintConfigChanged)
// as a new user preset under it, via the backend's save_config_as_preset
// command (papyrus_lint_core::config::save_user_preset) — the same
// executable-adjacent "presets" directory loadConfigPresets/applyConfigPreset
// above already read from, so the saved preset is immediately selectable
// from the first-run picker (or the CLI's --preset <name>) afterward.
// Cancels silently if the prompt is left blank; if a preset (built-in or
// user) already exists under that name, asks to overwrite it and cancels
// silently if declined. Reports success/failure once the save itself is
// attempted, since unlike every other Settings tab field, this isn't an
// autosave the user can otherwise tell happened.
export async function handleSaveConfigAsPresetClick(): Promise<void> {
  const name = window.prompt("Save the current settings as a preset named:")?.trim();
  if (!name) {
    return;
  }

  const presets = await loadConfigPresets();
  const exists = presets.some((preset) => preset.id.toLowerCase() === name.toLowerCase());
  if (exists && !window.confirm(`A preset named "${name}" already exists. Overwrite it?`)) {
    return;
  }

  try {
    await invoke("save_config_as_preset", { config: currentLintConfig, name, overwrite: exists });
    await refreshPresetManagementTab();
    window.alert(`Saved preset "${name}".`);
  } catch (error) {
    console.error(error);
    window.alert(`Failed to save preset "${name}": ${error}`);
  }
}

// The three built-in presets' own ids (see papyrus_lint_core::config::PRESET_NAMES),
// kept in sync by hand the same way FIXABLE_RULE_IDS is: everything
// loadConfigPresets returns that isn't one of these is a user preset, since
// config::save_user_preset/rename_user_preset always refuse a name matching
// one of these case-insensitively.
const BUILTIN_PRESET_IDS = new Set(["strict", "standard", "careful"]);

export function isCustomPreset(preset: ConfigPreset): boolean {
  return !BUILTIN_PRESET_IDS.has(preset.id.toLowerCase());
}

// Renames the user preset `oldName` to `newName`, via the backend's
// rename_user_preset command (papyrus_lint_core::config::rename_user_preset).
export async function renameUserPreset(oldName: string, newName: string, overwrite: boolean): Promise<void> {
  await invoke("rename_user_preset", { oldName, newName, overwrite });
}

// Deletes the user preset `name`, via the backend's delete_user_preset
// command (papyrus_lint_core::config::delete_user_preset).
export async function deleteUserPreset(name: string): Promise<void> {
  await invoke("delete_user_preset", { name });
}

// Fetches the user preset `name`'s raw YAML content, via the backend's
// export_user_preset command (papyrus_lint_core::config::read_user_preset_yaml),
// for handleExportPresetClick to offer as a download.
export async function exportUserPreset(name: string): Promise<string> {
  return invoke<string>("export_user_preset", { name });
}

// Rebuilds the Presets tab's management list from `presets` (see
// loadConfigPresets), showing only the user (non-built-in) ones — built-in
// presets can't be renamed, exported, or deleted. The tab itself (its
// button and panel) is only shown while at least one user preset exists;
// if it was the active tab and its last preset just got deleted, switches
// back to the Settings tab instead of leaving an empty panel showing.
export function renderPresetManagementTab(presets: ConfigPreset[]) {
  const customPresets = presets.filter(isCustomPreset);
  const hasCustomPresets = customPresets.length > 0;
  const wasActive = presetManagementTabEl?.classList.contains("tabs__tab--active") ?? false;
  if (presetManagementTabEl) {
    presetManagementTabEl.hidden = !hasCustomPresets;
  }
  if (!hasCustomPresets && wasActive) {
    switchTab("settings");
  }

  if (!presetManagementListEl) {
    return;
  }
  presetManagementListEl.innerHTML = "";
  for (const preset of customPresets) {
    const item = document.createElement("li");
    item.className = "preset-management__item";

    const label = document.createElement("span");
    label.className = "preset-management__label";
    label.textContent = preset.label;

    const actions = document.createElement("span");
    actions.className = "preset-management__actions";

    const renameButton = document.createElement("button");
    renameButton.type = "button";
    renameButton.className = "preset-management__button";
    renameButton.textContent = "Rename";
    renameButton.addEventListener("click", () => void handleRenamePresetClick(preset));

    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.className = "preset-management__button";
    exportButton.textContent = "Export";
    exportButton.addEventListener("click", () => void handleExportPresetClick(preset));

    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.className = "preset-management__button";
    deleteButton.textContent = "Delete";
    deleteButton.addEventListener("click", () => void handleDeletePresetClick(preset));

    actions.append(renameButton, exportButton, deleteButton);
    item.append(label, actions);
    presetManagementListEl.appendChild(item);
  }
}

// Reloads every configuration preset and re-renders the Presets tab from
// it. Called on startup and after any action (saving, renaming, or
// deleting a user preset) that could change which presets exist.
export async function refreshPresetManagementTab(): Promise<void> {
  renderPresetManagementTab(await loadConfigPresets());
}

// Prompts for `preset`'s new name, confirming an overwrite the same way
// handleSaveConfigAsPresetClick does if one is already in use, then renames
// it via renameUserPreset and refreshes the tab. Cancels silently if the
// prompt is left blank, unchanged (ignoring case), or the overwrite
// confirmation is declined.
export async function handleRenamePresetClick(preset: ConfigPreset): Promise<void> {
  const name = window.prompt(`Rename preset "${preset.label}" to:`, preset.label)?.trim();
  if (!name || name.toLowerCase() === preset.id.toLowerCase()) {
    return;
  }

  const presets = await loadConfigPresets();
  const exists = presets.some((other) => other.id.toLowerCase() === name.toLowerCase());
  if (exists && !window.confirm(`A preset named "${name}" already exists. Overwrite it?`)) {
    return;
  }

  try {
    await renameUserPreset(preset.id, name, exists);
    await refreshPresetManagementTab();
  } catch (error) {
    console.error(error);
    window.alert(`Failed to rename preset "${preset.label}": ${error}`);
  }
}

// Confirms, then deletes `preset` via deleteUserPreset and refreshes the
// tab.
export async function handleDeletePresetClick(preset: ConfigPreset): Promise<void> {
  if (!window.confirm(`Delete preset "${preset.label}"? This can't be undone.`)) {
    return;
  }

  try {
    await deleteUserPreset(preset.id);
    await refreshPresetManagementTab();
  } catch (error) {
    console.error(error);
    window.alert(`Failed to delete preset "${preset.label}": ${error}`);
  }
}

// Downloads `preset`'s raw YAML content (via exportUserPreset) as
// `<id>.yaml`, the same browser-download technique handleExportIssuesClick
// uses for the Lint results tab's own export button.
export async function handleExportPresetClick(preset: ConfigPreset): Promise<void> {
  try {
    const yaml = await exportUserPreset(preset.id);
    downloadTextFile(`${preset.id}.yaml`, yaml, "application/x-yaml");
  } catch (error) {
    console.error(error);
    window.alert(`Failed to export preset "${preset.label}": ${error}`);
  }
}

// Shows the "select this project's configuration" dialog useProjectDir
// opens for every not-yet-confirmed project directory (see
// confirmedProjectDirs), so a project's configuration is always picked
// with the project itself already known, rather than the Settings tab
// showing/editing whatever configuration happened to be loaded previously
// (or the engine's silent defaults) before the user has even said which
// project it applies to. Resolves to `{ kind: "detected" }` for "Continue"
// (or Escape/a backdrop click), which leaves useProjectDir's own
// auto-detection to do the right thing whether or not the project already
// has a configuration file; to `{ kind: "path", path }` once a non-blank
// path is confirmed via the "different file" input; or to
// `{ kind: "preset", preset }` once one of the inline preset options -
// shown only when the project has no configuration file yet, since
// initializing from a preset requires there to be none (see
// applyConfigPreset/papyrus_lint_core::config::initialize_default_config)
// - is clicked. Resolves immediately with `{ kind: "detected" }` if the
// dialog isn't present in the DOM (e.g. a minimal test fixture).
export async function promptForConfigSelection(projectInfo: ProjectInfo): Promise<ConfigSelectionResult> {
  if (!configPickerEl) {
    return { kind: "detected" };
  }

  const detectedPath = projectInfo.used_configuration_file;
  const presets = detectedPath ? [] : await loadConfigPresets();
  const dialog = configPickerEl;

  return new Promise((resolve) => {
    if (configPickerDetectedEl) {
      configPickerDetectedEl.hidden = !detectedPath;
    }
    if (configPickerDetectedPathEl) {
      configPickerDetectedPathEl.textContent = detectedPath ?? "";
    }
    if (configPickerNoneEl) {
      configPickerNoneEl.hidden = Boolean(detectedPath);
    }
    if (configPickerPathInputEl) {
      configPickerPathInputEl.value = "";
    }

    let settled = false;
    // configPickerContinueEl/configPickerUsePathButtonEl are static
    // elements reused across every call (unlike the preset options below,
    // rebuilt fresh each time), so their listeners must be explicitly torn
    // down here - otherwise an earlier, already-resolved call's handler
    // (still bound, since a run that resolved via a different path/Escape
    // never fired it to trigger its own removal) would keep piling up
    // across every project dropped in the session.
    const cleanup = () => {
      dialog.removeEventListener("close", handleClose);
      configPickerContinueEl?.removeEventListener("click", handleContinue);
      configPickerUsePathButtonEl?.removeEventListener("click", handleUsePath);
    };
    const finish = (result: ConfigSelectionResult) => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      if (dialog.hasAttribute("open")) {
        dialog.close();
      }
      resolve(result);
    };
    const handleClose = () => finish({ kind: "detected" });
    const handleContinue = () => finish({ kind: "detected" });
    const handleUsePath = () => {
      const path = configPickerPathInputEl?.value.trim();
      if (path) {
        finish({ kind: "path", path });
      }
    };

    configPickerContinueEl?.addEventListener("click", handleContinue);
    configPickerUsePathButtonEl?.addEventListener("click", handleUsePath);

    if (configPickerPresetListEl) {
      configPickerPresetListEl.hidden = presets.length === 0;
      configPickerPresetListEl.innerHTML = "";
      for (const preset of presets) {
        const option = document.createElement("button");
        option.type = "button";
        option.className = "config-picker__preset-option";
        const label = document.createElement("strong");
        label.className = "config-picker__preset-option-label";
        label.textContent = preset.label;
        const description = document.createElement("span");
        description.className = "config-picker__preset-option-description";
        description.textContent = preset.description;
        option.append(label, description);
        option.addEventListener("click", () => finish({ kind: "preset", preset: preset.id }));
        configPickerPresetListEl.appendChild(option);
      }
    }

    dialog.addEventListener("close", handleClose, { once: true });
    dialog.showModal();
  });
}

// Persists `roots` as `dir`'s configured additional script root
// directories.
export async function saveScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_script_roots", { dir, roots });
  } catch (error) {
    console.error(error);
  }
}

// Fetches every built-in lint rule's tag metadata (kind(s), importance, and
// whether it's auto-fixable; see papyrus_lints::tags) from the Rust
// backend, for grouping/filtering the lint results by tag. Returns an
// empty array if the lookup fails.
export async function loadRuleTags(): Promise<RuleTagsInfo[]> {
  try {
    return (await invoke<RuleTagsInfo[]>("list_rule_tags")) ?? [];
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Indexes `tags` by rule id (for tagsForFinding/matchesTagFilters below),
// rebuilds each tag kind's "Filter by rule" multiselect from the same list,
// and re-renders the current lint results, so any already-listed findings
// pick up their tag badges/filtering once the lookup resolves.
export function applyRuleTags(tags: RuleTagsInfo[]) {
  ruleTagsByRule = new Map(tags.map((info) => [info.rule, info]));
  populateRuleFilterGroups(tags);
  renderPscResults(currentPscOutcomes);
}

// Renders a rule id like "trailing-whitespace" as "Trailing whitespace" for
// the rule filter's option labels; spelling a display name out by hand for
// every rule, the way FIXABLE_RULE_DISPLAY_NAMES does for the smaller set of
// fixable ones, doesn't scale to all of them.
function titleCaseRuleId(rule: string): string {
  return rule.charAt(0).toUpperCase() + rule.slice(1).replace(/-/g, " ");
}

// Rebuilds each tag kind's "Filter by rule" multiselect from the backend's
// full set of known rules - a rule tagged with more than one kind (e.g.
// "argument-types", tagged both "performance" and "correctness") appears in
// each of its kinds' multiselects, kept in sync with each other via the
// single activeRules set both read from/write to (see
// syncRuleFilterSelections below, and its own "change" listener set up in
// the DOMContentLoaded handler), so deselecting it in one group's list is
// reflected in the other's too. Every rule starts selected, so the filter
// starts as a no-op, the same way every other lint results filter does.
// activeRules is populated regardless of whether the <select> elements
// themselves are present, so matchesTagFilters below still works correctly
// (e.g. in a test fixture that doesn't include them).
function populateRuleFilterGroups(tags: RuleTagsInfo[]) {
  activeRules.clear();
  for (const tag of tags) {
    activeRules.add(tag.rule);
  }
  for (const kind of TAG_KINDS) {
    const select = ruleFilterSelectEls[kind];
    if (!select) {
      continue;
    }
    select.replaceChildren(
      ...tags
        .filter((tag) => tag.kinds.includes(kind))
        .sort((a, b) => a.rule.localeCompare(b.rule))
        .map((tag) => {
          const option = document.createElement("option");
          option.value = tag.rule;
          option.textContent = titleCaseRuleId(tag.rule);
          option.selected = true;
          return option;
        }),
    );
    updateTagKindHeaderCheckbox(kind);
  }
}

// Reflects activeRules onto every tag kind's "Filter by rule" multiselect
// (a rule shared by more than one kind's list needs both copies kept in
// sync) and updates each group's header checkbox to reflect whether all,
// some, or none of its own rules are currently active.
function syncRuleFilterSelections() {
  for (const kind of TAG_KINDS) {
    const select = ruleFilterSelectEls[kind];
    if (!select) {
      continue;
    }
    for (const option of select.options) {
      option.selected = activeRules.has(option.value);
    }
    updateTagKindHeaderCheckbox(kind);
  }
}

// Sets `kind`'s header checkbox to checked (every rule in its multiselect is
// active), unchecked (none are), or indeterminate (some are) - so it doubles
// as a "select all"/"select none" toggle for that group and as an at-a-glance
// summary of its current selection.
function updateTagKindHeaderCheckbox(kind: TagKind) {
  const checkbox = tagKindFilterEls[kind];
  const select = ruleFilterSelectEls[kind];
  if (!checkbox || !select) {
    return;
  }
  const options = [...select.options];
  const selectedCount = options.filter((option) => option.selected).length;
  checkbox.checked = options.length > 0 && selectedCount === options.length;
  checkbox.indeterminate = selectedCount > 0 && selectedCount < options.length;
}

// Fetches the desktop app's version from the Rust backend, so it can be
// shown to the user. Returns an empty string if the lookup fails.
export async function loadAppVersion(): Promise<string> {
  try {
    return await invoke<string>("get_app_version");
  } catch (error) {
    console.error(error);
    return "";
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
    cyclomatic_complexity_warning: Math.max(1, cyclomaticComplexityWarningEl?.valueAsNumber || 10),
    cyclomatic_complexity_error: Math.max(1, cyclomaticComplexityErrorEl?.valueAsNumber || 20),
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
  lintResultsStale = true;
  const override = configPathOverride();
  if (override) {
    void saveLintConfigToPath(override, currentLintConfig);
  } else if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

export async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      root: currentProjectDir ?? "",
      config: currentLintConfig,
      additionalRoots: effectiveScriptRoots(),
      compilerPath: currentCompilerPath,
      compileCheck: currentCompileCheck,
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export async function repairPscFile(path: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file", {
    path,
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additionalRoots: effectiveScriptRoots(),
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
  });
}

// Rule ids with an automatic fix (papyrus_lints::FIXABLE_RULE_IDS), used to
// decide which findings offer the per-finding "Fix this issue" button. Kept
// in sync by hand with FIXABLE_RULE_IDS in
// app/crates/papyrus-lints/src/lib.rs.
const FIXABLE_RULE_IDS = new Set([
  "identifier-casing",
  "slow-functions",
  "semicolon",
  "indentation",
  "property-sorting",
  "comma-spacing",
  "chain-whitespace",
  "exclamation-spacing",
  "operator-spacing",
  "type-casing",
  "trailing-whitespace",
]);

// A rule in FIXABLE_RULE_IDS can still report a violation it can't actually
// repair without a substantive rename (e.g. type-casing on a name with
// underscores, such as a compiler-generated fragment script's ScriptName) --
// see papyrus_lints::type_casing::check, which appends this same note to
// such a finding's own message rather than letting a caller assume every
// finding from a "fixable" rule can be fixed.
const NO_AUTOMATIC_FIX_NOTE = "no automatic fix";

function hasNoAutomaticFix(finding: Diagnostic): boolean {
  return finding.message.includes(NO_AUTOMATIC_FIX_NOTE);
}

export function isFixableFinding(finding: Diagnostic): boolean {
  return finding.rule !== undefined && FIXABLE_RULE_IDS.has(finding.rule) && !hasNoAutomaticFix(finding);
}

// Applies just `rule`'s own automatic fix, restricted to `line` (see
// repair_psc_finding/papyrus_lints::restrict_to_line on the backend),
// leaving every other line and finding untouched. Rejects (e.g. a fix that
// would change the file's line count elsewhere, like property-sorting
// relocating a declaration) rather than falling back to the whole-file
// "Apply fixes" behavior, so the caller can surface why this one issue
// couldn't be fixed on its own.
export async function repairPscFinding(path: string, rule: string, line: number): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_finding", {
    path,
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additionalRoots: effectiveScriptRoots(),
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
    rule,
    line,
  });
}

// Applies just `rule`'s own automatic fix across the whole of `path`, like
// repairPscFinding but without restricting it to a single line — the "mass
// fix" action below calls this once per file to clear every occurrence of
// one issue (e.g. every trailing-whitespace finding) project-wide in one go.
export async function repairPscFileRule(path: string, rule: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file_rule", {
    path,
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additionalRoots: effectiveScriptRoots(),
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
    rule,
  });
}

// Human-readable names for FIXABLE_RULE_IDS, used to label each rule in the
// "mass fix" panel instead of its raw id. Kept in sync by hand with each
// rule's own settings-tab checkbox label text in index.html.
const FIXABLE_RULE_DISPLAY_NAMES: Record<string, string> = {
  "identifier-casing": "Identifier casing",
  "slow-functions": "Slow function usage",
  semicolon: "Semicolon at end of line",
  indentation: "Formatting checks / Indentation",
  "property-sorting": "Property sorting",
  "comma-spacing": "Space after comma",
  "chain-whitespace": "Whitespace interrupting property/method chaining",
  "exclamation-spacing": "Exclamation mark spacing",
  "operator-spacing": "Spacing around logical/comparison operators",
  "type-casing": "Type name casing",
  "trailing-whitespace": "Trailing whitespace",
};

export function massFixRuleDisplayName(rule: string): string {
  return FIXABLE_RULE_DISPLAY_NAMES[rule] ?? rule;
}

// Counts, per rule id, how many currently fixable findings (see
// isFixableFinding) exist across every outcome, for the "mass fix" panel to
// list. A rule with no fixable finding anywhere is omitted entirely.
export function massFixRuleCounts(outcomes: PscParseOutcome[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const outcome of outcomes) {
    for (const finding of outcome.findings) {
      if (finding.rule && isFixableFinding(finding)) {
        counts.set(finding.rule, (counts.get(finding.rule) ?? 0) + 1);
      }
    }
  }
  return counts;
}

async function writePscFile(path: string, contents: string): Promise<void> {
  await invoke("write_psc_file", { path, contents });
}

// Fetches every function/property available on an object of type
// `typeName` (including those inherited via Extends), for the code
// viewer's `.`-triggered autocompletion. `root` is the project root (see
// projectDirForAchlist/projectDirForPscPath), the same as every other
// command that resolves scripts across a project.
export async function listScriptMembers(typeName: string): Promise<Member[]> {
  try {
    return await invoke<Member[]>("list_script_members", {
      root: currentProjectDir ?? "",
      typeName,
      additionalRoots: effectiveScriptRoots(),
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) => isFixableFinding(finding));
}

// Parses and lints every path in `paths`, invoking `onOutcome` (if given) as
// each one finishes rather than waiting for the whole batch — the caller can
// use that to render results incrementally instead of freezing until the
// slowest file completes. Outcomes are otherwise still resolved concurrently,
// so `onOutcome` fires in completion order, not necessarily `paths`' order.
export async function parsePscFiles(
  paths: string[],
  onOutcome?: (outcome: PscParseOutcome) => void,
): Promise<PscParseOutcome[]> {
  return Promise.all(
    paths.map(async (path) => {
      let outcome: PscParseOutcome;
      try {
        const script = await invoke<PapyrusScript>("parse_psc_file", { path });
        const findings = await lintPscFile(path);
        outcome = { path, ok: true, detail: `parsed as "${script.name}"`, findings };
      } catch (error) {
        outcome = { path, ok: false, detail: String(error), findings: [] };
      }
      onOutcome?.(outcome);
      return outcome;
    }),
  );
}

// Diagnostic messages are prefixed with `[level] `; every built-in lint
// tags one, but a message with no recognized prefix still falls back to
// the "other" severity rather than being misclassified.
export type Severity = "error" | "warning" | "info" | "other";
export const SEVERITIES: Severity[] = ["error", "warning", "info", "other"];

export function levelOf(message: string): "error" | "warning" | "info" | null {
  const match = /^\[(error|warning|info)\]/.exec(message);
  return match ? (match[1] as "error" | "warning" | "info") : null;
}

export function escapeAttr(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

interface CodeViewerState {
  path: string;
  source: string;
  findings: Diagnostic[];
}

let codeViewerState: CodeViewerState | null = null;
let codeViewerMode: "view" | "edit" = "view";

// The autocompletion dropdown's currently pending query/results, if any is
// showing. `autocompleteRequestId` guards against a stale
// `listScriptMembers` response (from an earlier keystroke) overwriting a
// newer one that resolves first.
let autocompleteQuery: CompletionQuery | null = null;
let autocompleteMembers: Member[] = [];
let autocompleteSelectedIndex = 0;
let autocompleteRequestId = 0;

function lineSeverityOf(lineFindings: Diagnostic[] | undefined): "error" | "warning" | "info" | "flagged" | null {
  if (!lineFindings || lineFindings.length === 0) {
    return null;
  }
  const levels = new Set(lineFindings.map((finding) => levelOf(finding.message)));
  if (levels.has("error")) return "error";
  if (levels.has("warning")) return "warning";
  if (levels.has("info")) return "info";
  // No recognized level prefix (not expected from any built-in lint, but
  // possible from a malformed diagnostic); still mark the line so the
  // finding is visible in the viewer.
  return "flagged";
}

function findingsGroupedByLine(findings: Diagnostic[]): Map<number, Diagnostic[]> {
  const findingsByLine = new Map<number, Diagnostic[]>();
  for (const finding of findings) {
    const forLine = findingsByLine.get(finding.line) ?? [];
    forLine.push(finding);
    findingsByLine.set(finding.line, forLine);
  }
  return findingsByLine;
}

// Renders `source`'s syntax-highlighted, read-only table view with
// `findings` marked on their lines. If `focusLine` is given, scrolls that
// line into view and briefly flashes it, so a click on a specific finding
// jumps straight to it.
function renderCodeViewerView(source: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerViewEl) {
    return;
  }

  const findingsByLine = findingsGroupedByLine(findings);

  const lines = highlightPapyrusLines(source);
  const rows = lines.map((lineHtml, index) => {
    const lineNumber = index + 1;
    const lineFindings = findingsByLine.get(lineNumber);
    const severity = lineSeverityOf(lineFindings);
    const rowClass = severity ? ` class="code-viewer__line--${severity}"` : "";
    const title = lineFindings
      ? ` title="${escapeAttr(lineFindings.map((f) => f.message).join("\n"))}"`
      : "";
    return (
      `<tr id="code-viewer-line-${lineNumber}"${rowClass}${title}>` +
      `<td class="code-viewer__line-number">${lineNumber}</td>` +
      `<td class="code-viewer__line-code">${lineHtml}</td>` +
      `</tr>`
    );
  });

  codeViewerViewEl.innerHTML = `<table class="code-viewer__table"><tbody>${rows.join("")}</tbody></table>`;

  if (focusLine) {
    const row = codeViewerViewEl.querySelector<HTMLElement>(`#code-viewer-line-${focusLine}`);
    row?.scrollIntoView({ block: "center" });
    row?.classList.add("code-viewer__line--flash");
  }
}

// Shows the view-mode table or the edit-mode textarea/highlight overlay,
// toggling the header's Edit/Save/Cancel buttons to match.
function setCodeViewerMode(mode: "view" | "edit") {
  codeViewerMode = mode;
  if (mode !== "edit") {
    hideAutocomplete();
  }
  if (codeViewerViewEl) codeViewerViewEl.hidden = mode !== "view";
  if (codeViewerEditEl) codeViewerEditEl.hidden = mode !== "edit";
  if (codeViewerEditButtonEl) codeViewerEditButtonEl.hidden = mode !== "view";
  if (codeViewerSaveButtonEl) codeViewerSaveButtonEl.hidden = mode !== "edit";
  if (codeViewerSaveCompileButtonEl) codeViewerSaveCompileButtonEl.hidden = mode !== "edit";
  if (codeViewerCancelButtonEl) codeViewerCancelButtonEl.hidden = mode !== "edit";
  updateCodeViewerFixButtonVisibility();
}

// Shows the "Apply fixes" button only in view mode, and only while the
// currently loaded file still has at least one fixable finding (the same
// check the Lint results list uses to decide whether to show its own
// per-file "Apply fixes" button), so it disappears on its own once nothing
// is left to fix. Called both on every mode change and whenever
// codeViewerState's findings change without a mode change (e.g. right
// after a fix is applied).
function updateCodeViewerFixButtonVisibility() {
  if (!codeViewerFixButtonEl) {
    return;
  }
  codeViewerFixButtonEl.hidden = codeViewerMode !== "view" || !hasFixableFindings(codeViewerState?.findings ?? []);
}

// Findings for the script currently open in edit mode, grouped by line -
// refreshed by every `updateCodeViewerEditHighlight()` call below, and read
// by `updateCodeViewerEditTooltip` so a mouse move doesn't have to regroup
// `codeViewerState.findings` on every event.
let codeViewerEditFindingsByLine: Map<number, Diagnostic[]> = new Map();

// Re-renders the edit mode's syntax-highlighted overlay and line-number
// gutter from the textarea's current value, keeping both in sync as the
// user types.
function updateCodeViewerEditHighlight() {
  const code = codeViewerEditHighlightEl?.querySelector("code");
  if (!code || !codeViewerEditTextareaEl) {
    return;
  }
  const findings = codeViewerState?.findings ?? [];
  const findingsByLine = findingsGroupedByLine(findings);
  codeViewerEditFindingsByLine = findingsByLine;
  const highlightedLines = highlightPapyrusLines(codeViewerEditTextareaEl.value);
  code.innerHTML = highlightedLines
    .map((lineHtml, index) => {
      const lineFindings = findingsByLine.get(index + 1);
      const severity = lineSeverityOf(lineFindings);
      const lineClass = severity
        ? `code-viewer__editor-line code-viewer__line--${severity}`
        : "code-viewer__editor-line";
      return `<span class="${lineClass}">${lineHtml}</span>`;
    })
    // Each line is already `display: block` (see styles.css), so it needs no
    // separator to end up on its own line; joining with "\n" here used to add
    // a literal newline character between spans that, because the highlight
    // layer is `white-space: pre`, rendered as its own extra blank line on
    // top of each blank line's own (empty, so zero-height) span - doubling
    // up and misaligning the overlay against the textarea underneath it.
    .join("");

  if (codeViewerEditGutterEl) {
    codeViewerEditGutterEl.innerHTML = highlightedLines
      .map((_, index) => `<span class="code-viewer__editor-gutter-line">${index + 1}</span>`)
      .join("");
  }

  // The textarea sits above the non-interactive highlighting layer, so its
  // per-line colours are the only ones actually visible; its own `title`
  // (see `updateCodeViewerEditTooltip`) is instead kept in sync with
  // whichever line the mouse currently hovers, the same as the read-only
  // view's per-row tooltips. The accessible description still summarizes
  // every finding at once, since a screen-reader user has no equivalent of
  // "hovering a line" to reveal them one at a time.
  const diagnosticSummary = findings
    .map((finding) => `Line ${finding.line}, column ${finding.column}: ${finding.message}`)
    .join("\n");
  codeViewerEditTextareaEl.setAttribute(
    "aria-label",
    diagnosticSummary ? `Papyrus source editor. Linter findings:\n${diagnosticSummary}` : "Papyrus source editor. No linter findings.",
  );
}

// Keeps the textarea's tooltip matched to the source line under the mouse
// cursor at `clientY`, since the textarea sits on top of (and intercepts
// every pointer event meant for) the highlighted overlay beneath it - whose
// own per-line findings would otherwise never actually be hoverable.
//
// Remembered so a scroll event (mouse wheel, keyboard navigation) can
// re-evaluate the tooltip against the pointer's last known position even
// though the pointer itself didn't move - otherwise scrolling a new line
// under a stationary mouse would leave the previous line's tooltip showing.
let codeViewerEditLastMouseY: number | null = null;

function updateCodeViewerEditTooltip(clientY: number) {
  if (!codeViewerEditTextareaEl) {
    return;
  }
  const computed = window.getComputedStyle(codeViewerEditTextareaEl);
  const lineHeight = parseFloat(computed.lineHeight);
  const paddingTop = parseFloat(computed.paddingTop);
  const rect = codeViewerEditTextareaEl.getBoundingClientRect();
  const offsetY = clientY - rect.top + codeViewerEditTextareaEl.scrollTop - paddingTop;
  const line = Math.floor(offsetY / lineHeight) + 1;
  const lineFindings = codeViewerEditFindingsByLine.get(line);
  codeViewerEditTextareaEl.title = lineFindings ? lineFindings.map((finding) => finding.message).join("\n") : "";
}

// Measures where the text caret currently renders inside `textarea`, using
// a hidden, identically-styled mirror element (the standard technique for
// this - a real caret rectangle isn't exposed by the DOM). Coordinates are
// relative to the textarea's own box, matching where the autocompletion
// dropdown (its sibling, absolutely positioned within the same container)
// should be placed.
function caretPixelPosition(textarea: HTMLTextAreaElement): { top: number; left: number } {
  const computed = window.getComputedStyle(textarea);
  const mirror = document.createElement("div");
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.top = "0";
  mirror.style.left = "-9999px";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.wordBreak = "break-word";
  mirror.style.boxSizing = computed.boxSizing;
  mirror.style.width = computed.width;
  mirror.style.padding = computed.padding;
  mirror.style.border = `${computed.borderWidth} solid transparent`;
  mirror.style.fontFamily = computed.fontFamily;
  mirror.style.fontSize = computed.fontSize;
  mirror.style.fontWeight = computed.fontWeight;
  mirror.style.lineHeight = computed.lineHeight;
  mirror.style.letterSpacing = computed.letterSpacing;

  const caretIndex = textarea.selectionStart;
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.append(textarea.value.slice(0, caretIndex), marker, textarea.value.slice(caretIndex) || " ");

  document.body.append(mirror);
  const top = marker.offsetTop - textarea.scrollTop + marker.offsetHeight;
  const left = marker.offsetLeft - textarea.scrollLeft;
  mirror.remove();

  return { top, left };
}

// Positions the autocompletion dropdown just below the text caret.
function positionAutocomplete() {
  if (!codeViewerAutocompleteEl || !codeViewerEditTextareaEl) {
    return;
  }
  const { top, left } = caretPixelPosition(codeViewerEditTextareaEl);
  codeViewerAutocompleteEl.style.top = `${top}px`;
  codeViewerAutocompleteEl.style.left = `${left}px`;
}

// Hides the autocompletion dropdown and clears its pending query/results.
export function hideAutocomplete() {
  // Invalidate a lookup that may still be awaiting the backend. Otherwise an
  // Escape press (or a cursor move away from member access) can hide the
  // dropdown only for the stale response to display it again.
  autocompleteRequestId += 1;
  autocompleteQuery = null;
  autocompleteMembers = [];
  autocompleteSelectedIndex = 0;
  if (codeViewerAutocompleteEl) {
    codeViewerAutocompleteEl.hidden = true;
    codeViewerAutocompleteEl.replaceChildren();
  }
}

// Renders `autocompleteMembers` into the dropdown (with the currently
// selected one highlighted), or hides it if there are none.
function renderAutocomplete() {
  if (!codeViewerAutocompleteEl) {
    return;
  }
  if (autocompleteMembers.length === 0) {
    codeViewerAutocompleteEl.hidden = true;
    codeViewerAutocompleteEl.replaceChildren();
    return;
  }

  codeViewerAutocompleteEl.replaceChildren(
    ...autocompleteMembers.map((member, index) => {
      const item = document.createElement("li");
      item.setAttribute("role", "option");
      item.classList.add("code-viewer__autocomplete-item");
      item.classList.toggle("code-viewer__autocomplete-item--active", index === autocompleteSelectedIndex);
      item.textContent = completionLabel(member);
      // mousedown (not click), and prevented from moving focus, so
      // accepting a completion by clicking it doesn't blur the textarea
      // first (which would otherwise close the dropdown before the click
      // that's meant to use it).
      item.addEventListener("mousedown", (event) => {
        event.preventDefault();
        applyAutocompleteSelection(index);
      });
      return item;
    }),
  );
  codeViewerAutocompleteEl.hidden = false;
  positionAutocomplete();
}

// Re-evaluates the autocompletion query at the textarea's current cursor
// position, fetching and showing matching members if the cursor is right
// after a "receiver.prefix" whose receiver's declared type is known.
// Hides the dropdown otherwise (including while a range is selected).
export async function updateAutocomplete() {
  if (!codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    hideAutocomplete();
    return;
  }
  const textarea = codeViewerEditTextareaEl;
  if (textarea.selectionStart !== textarea.selectionEnd) {
    hideAutocomplete();
    return;
  }

  const query = completionQueryAt(textarea.value, textarea.selectionStart);
  if (!query) {
    hideAutocomplete();
    return;
  }

  const requestId = ++autocompleteRequestId;
  const members = filterMembers(await listScriptMembers(query.receiverType), query.prefix);
  // A later keystroke may have started a new request (or left edit mode)
  // while this one was in flight; don't clobber it with a stale response.
  if (requestId !== autocompleteRequestId || !codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }

  autocompleteQuery = query;
  autocompleteMembers = members;
  autocompleteSelectedIndex = 0;
  renderAutocomplete();
}

// Splices the selected member's insertion text into the textarea in place
// of the typed prefix, then closes the dropdown.
export function applyAutocompleteSelection(index: number) {
  const member = autocompleteMembers[index];
  if (!codeViewerEditTextareaEl || !autocompleteQuery || !member) {
    return;
  }
  const textarea = codeViewerEditTextareaEl;
  const { prefixStart } = autocompleteQuery;
  textarea.setRangeText(completionInsertText(member), prefixStart, textarea.selectionStart, "end");
  hideAutocomplete();
  updateCodeViewerEditHighlight();
  textarea.focus();
}

// Handles the dropdown's navigation/acceptance/dismissal keys while it's
// open; every other key is left for the textarea to handle normally.
export function handleAutocompleteKeydown(event: KeyboardEvent) {
  if (autocompleteMembers.length === 0) {
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    autocompleteSelectedIndex = (autocompleteSelectedIndex + 1) % autocompleteMembers.length;
    renderAutocomplete();
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    autocompleteSelectedIndex = (autocompleteSelectedIndex - 1 + autocompleteMembers.length) % autocompleteMembers.length;
    renderAutocomplete();
  } else if (event.key === "Enter" || event.key === "Tab") {
    event.preventDefault();
    applyAutocompleteSelection(autocompleteSelectedIndex);
  } else if (event.key === "Escape") {
    event.preventDefault();
    hideAutocomplete();
  }
}

// A textarea's `value` getter always normalizes CR/CRLF line breaks to LF
// (per the HTML spec's "API value" transform), even though its `value`
// setter stores whatever was assigned verbatim. A CRLF-saved .psc file's
// `source` therefore no longer matches the textarea's own value right
// after `enterCodeViewerEditMode` sets it, with no edit having happened;
// normalizing both sides before comparing keeps that from reading as dirty.
function normalizeLineEndings(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

export function isCodeViewerEditDirty(): boolean {
  return (
    codeViewerMode === "edit" &&
    codeViewerState !== null &&
    codeViewerEditTextareaEl !== null &&
    codeViewerEditTextareaEl.value !== normalizeLineEndings(codeViewerState.source)
  );
}

export function enterCodeViewerEditMode() {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  codeViewerEditTextareaEl.value = codeViewerState.source;
  updateCodeViewerEditHighlight();
  hideCompileOutput(codeViewerCompileOutputEl);
  setCodeViewerMode("edit");
  codeViewerEditTextareaEl.focus();
}

export function cancelCodeViewerEditMode() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  setCodeViewerMode("view");
}

// Writes the editor's current contents to disk, re-lints the file, and
// refreshes both the code viewer's view mode and the Lint results list to
// match, switching the viewer back to view mode. Shared by the plain Save
// button and the Save & Compile button below; throws (without touching any
// UI) if the write itself fails, leaving the caller to report that.
async function persistCodeViewerEdits(): Promise<void> {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  const { path } = codeViewerState;
  const contents = codeViewerEditTextareaEl.value;

  await writePscFile(path, contents);
  const findings = await lintPscFile(path);
  codeViewerState = { path, source: contents, findings };

  const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
  if (outcome) {
    outcome.findings = findings;
    renderPscResults(currentPscOutcomes);
  }

  renderCodeViewerView(codeViewerState.source, codeViewerState.findings);
  setCodeViewerMode("view");
}

export async function saveCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveButtonEl) {
    return;
  }
  codeViewerSaveButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    const originalLabel = codeViewerSaveButtonEl.textContent;
    codeViewerSaveButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveButtonEl) {
        codeViewerSaveButtonEl.textContent = originalLabel;
      }
    }, 2000);
  } finally {
    codeViewerSaveButtonEl.disabled = false;
  }
}

// Saves the editor's contents (as saveCodeViewerEdits does) and, if that
// succeeds, immediately compiles the saved file, showing the compiler's
// output beneath the code viewer's view/editor area.
export async function saveAndCompileCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveCompileButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  const originalLabel = codeViewerSaveCompileButtonEl.textContent;

  codeViewerSaveCompileButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    codeViewerSaveCompileButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveCompileButtonEl) {
        codeViewerSaveCompileButtonEl.textContent = originalLabel;
      }
    }, 2000);
    codeViewerSaveCompileButtonEl.disabled = false;
    return;
  }

  if (codeViewerCompileOutputEl) {
    codeViewerSaveCompileButtonEl.textContent = "Compiling…";
    await compileAndShowOutput(path, codeViewerCompileOutputEl);
  }
  codeViewerSaveCompileButtonEl.disabled = false;
  codeViewerSaveCompileButtonEl.textContent = originalLabel;
}

// Closes the code viewer, confirming first if edit mode has unsaved changes.
export function requestCloseCodeViewer() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  codeViewerEl?.close();
}

// Reads and syntax-highlights `path`'s source, then opens the code viewer
// dialog with `findings` marked on their lines. If `focusLine` is given,
// scrolls that line into view and briefly flashes it, so a click on a
// specific finding jumps straight to it.
export async function openCodeViewer(path: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerEl || !codeViewerTitleEl || !codeViewerViewEl) {
    return;
  }

  codeViewerState = null;
  setCodeViewerMode("view");
  hideCompileOutput(codeViewerCompileOutputEl);
  codeViewerTitleEl.textContent = path;
  codeViewerViewEl.textContent = "Loading…";
  codeViewerEl.showModal();

  let source: string;
  try {
    source = await invoke<string>("read_psc_file", { path });
  } catch (error) {
    codeViewerViewEl.textContent = `Failed to read file: ${String(error)}`;
    return;
  }

  codeViewerState = { path, source, findings };
  updateCodeViewerFixButtonVisibility();
  renderCodeViewerView(source, findings, focusLine);
}

// Toggles the code viewer between its default size and filling the window,
// keeping the button's label/state in sync.
export function toggleCodeViewerFullscreen() {
  if (!codeViewerEl || !codeViewerFullscreenEl) {
    return;
  }
  const isFullscreen = codeViewerEl.classList.toggle("code-viewer--fullscreen");
  codeViewerFullscreenEl.setAttribute("aria-pressed", String(isFullscreen));
  codeViewerFullscreenEl.setAttribute("aria-label", isFullscreen ? "Exit fullscreen" : "Enter fullscreen");
}

export function severityOf(message: string): Severity {
  return levelOf(message) ?? "other";
}

// Which severities are currently shown in the lint results list; all are
// shown by default.
const activeSeverities = new Set<Severity>(SEVERITIES);
let severityFilterEls: Partial<Record<Severity, HTMLInputElement>> = {};

// Which importance levels are currently shown in the lint results list; all
// are shown by default, same as activeSeverities above. Tag kind filtering
// used to be a separate activeTagKinds set alongside this one, but each tag
// kind's checkbox is now just a "select all"/"select none" toggle for its
// own "Filter by rule" multiselect (see updateTagKindHeaderCheckbox above),
// so which rules of that kind are shown is tracked by activeRules alone.
const activeTagImportances = new Set<TagImportance>(TAG_IMPORTANCES);
// Whether only auto-fixable findings should be shown; off by default.
let onlyAutoFixable = false;
let tagKindFilterEls: Partial<Record<TagKind, HTMLInputElement>> = {};
let tagImportanceFilterEls: Partial<Record<TagImportance, HTMLInputElement>> = {};

// Which rule ids are currently shown in the lint results list, driven by the
// tag-grouped "Filter by rule" multiselects (see populateRuleFilterGroups/
// syncRuleFilterSelections above). Populated (with every known rule, i.e. no
// filtering) once the backend's rule list loads.
const activeRules = new Set<string>();

// Looks up `finding`'s own rule's tag metadata, if any. A finding with no
// rule (or one that isn't a papyrus-lints rule id at all, e.g. a
// compiler-reported diagnostic - see app/src-tauri/src/compile_diagnostics.rs)
// has none.
export function tagsForFinding(finding: Diagnostic): RuleTagsInfo | undefined {
  return finding.rule ? ruleTagsByRule.get(finding.rule) : undefined;
}

// Whether `finding` passes the active tag/rule, importance, and
// auto-fixable filters. A finding with no tag metadata always passes, the
// same way an unrecognized severity still falls back to the always-shown
// "other" bucket instead of being silently dropped. A finding whose rule is
// known always has a truthy `finding.rule` (tagsForFinding only returns tag
// metadata when it does), so once `tags` is present activeRules.has() below
// is checking the same rule id that produced it. Every finding also passes
// while the backend's rule list hasn't loaded yet, since tagsForFinding
// (and so `tags`) is undefined for all of them until then.
export function matchesTagFilters(finding: Diagnostic): boolean {
  const tags = tagsForFinding(finding);
  if (!tags) {
    return true;
  }
  if (!activeRules.has(finding.rule as string)) {
    return false;
  }
  if (!activeTagImportances.has(tags.importance)) {
    return false;
  }
  return !onlyAutoFixable || tags.auto_fixable;
}

// `findings` restricted to those passing every active severity/tag/rule
// filter, shared by buildPscResultItem (rendering the Lint results list)
// and collectFilteredIssues (the "Export issues" button below) so the two
// can never disagree about what "currently filtered" means.
function findingsPassingActiveFilters(findings: Diagnostic[]): Diagnostic[] {
  return findings.filter(
    (finding) => activeSeverities.has(severityOf(finding.message)) && matchesTagFilters(finding),
  );
}

// The current filename search pattern; an empty string matches every file.
let currentFilenameFilter = "";

// Tests whether `path` matches the user's filename search `pattern`,
// treating "*" and "%" as "match any run of characters" and "?" as "match
// exactly one character", the same way a shell glob or a SQL LIKE pattern
// would. Matching is case-insensitive and unanchored, so a plain pattern
// with no wildcards (e.g. "quest") behaves as a substring search, letting
// the user search the lint results by only part of a filename. An empty
// (or all-whitespace) pattern matches every path.
export function matchesFilenameFilter(path: string, pattern: string): boolean {
  const trimmed = pattern.trim();
  if (trimmed.length === 0) {
    return true;
  }
  const regexSource = trimmed
    .split(/([*%?])/)
    .map((part) => {
      if (part === "*" || part === "%") {
        return ".*";
      }
      if (part === "?") {
        return ".";
      }
      return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    })
    .join("");
  return new RegExp(regexSource, "i").test(path);
}

export function showError(message: string) {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = message;
  }
  resultEl?.setAttribute("hidden", "");
  switchTab("import");
}

export function clearError() {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = "";
  }
}

// `base` is the project root (see projectDirForAchlist/projectDirForPscPath),
// used to shorten each entry to a path relative to it so long absolute
// paths stay readable; entries outside `base` (or when it isn't known)
// fall back to their absolute path, per relativePath().
export function showResult(path: string, entries: string[], base: string | null) {
  if (!resultEl || !resultTitleEl || !resultListEl) {
    return;
  }

  resultTitleEl.textContent = `Loaded ${path}`;
  resultListEl.replaceChildren(
    ...entries.map((entry) => {
      const item = document.createElement("li");

      const label = document.createElement("span");
      label.textContent = relativePath(entry, base);
      item.append(label);

      if (isPscPath(entry)) {
        const viewButton = document.createElement("button");
        viewButton.type = "button";
        viewButton.textContent = "View";
        viewButton.classList.add("achlist-result__view-button");
        viewButton.addEventListener("click", () => {
          const outcome = currentPscOutcomes.find((candidate) => candidate.path === entry);
          void openCodeViewer(entry, outcome?.findings ?? []);
        });
        item.append(viewButton);
      }

      return item;
    }),
  );
  resultEl.removeAttribute("hidden");
  switchTab("files");
}

// Builds the small badge row surfacing `finding`'s own rule's tag metadata
// (kind(s), importance, and whether it has an automatic fix), or null if
// its rule carries no tag metadata (see tagsForFinding).
function buildFindingTagsEl(finding: Diagnostic): HTMLElement | null {
  const tags = tagsForFinding(finding);
  if (!tags) {
    return null;
  }

  const tagsEl = document.createElement("span");
  tagsEl.classList.add("psc-result__finding-tags");

  for (const kind of tags.kinds) {
    const badge = document.createElement("span");
    badge.classList.add("psc-result__tag-badge", "psc-result__tag-badge--kind");
    badge.textContent = kind;
    tagsEl.append(badge);
  }

  const importanceBadge = document.createElement("span");
  importanceBadge.classList.add("psc-result__tag-badge", `psc-result__tag-badge--importance-${tags.importance}`);
  importanceBadge.textContent = `${tags.importance} importance`;
  tagsEl.append(importanceBadge);

  if (tags.auto_fixable && !hasNoAutomaticFix(finding)) {
    const fixableBadge = document.createElement("span");
    fixableBadge.classList.add("psc-result__tag-badge", "psc-result__tag-badge--auto-fixable");
    fixableBadge.textContent = "auto-fixable";
    tagsEl.append(fixableBadge);
  }

  return tagsEl;
}

// Builds the list item for `outcome`, or null if it has no findings that
// pass the active severity filter and should therefore be skipped
// entirely (a file with nothing to show isn't worth a row). Files that
// failed to parse are always shown, since that failure is itself the
// result worth reporting.
export function buildPscResultItem(outcome: PscParseOutcome): HTMLLIElement | null {
  const { path, ok, detail, findings } = outcome;

  if (!matchesFilenameFilter(relativePath(path, currentProjectDir), currentFilenameFilter)) {
    return null;
  }

  const visibleFindings = findingsPassingActiveFilters(findings);

  if (ok && visibleFindings.length === 0) {
    return null;
  }

  const item = document.createElement("li");
  item.classList.add(ok ? "psc-result__item--ok" : "psc-result__item--error");

  const summary = document.createElement("span");
  summary.textContent = `${relativePath(path, currentProjectDir)}: ${detail}`;
  item.append(summary);

  const viewButton = document.createElement("button");
  viewButton.type = "button";
  viewButton.textContent = "View code";
  viewButton.classList.add("psc-result__view-button");
  viewButton.addEventListener("click", () => void openCodeViewer(path, outcome.findings));
  item.append(viewButton);

  if (hasFixableFindings(findings)) {
    const fixButton = document.createElement("button");
    fixButton.type = "button";
    fixButton.textContent = "Apply fixes";
    fixButton.classList.add("psc-result__fix-button");
    fixButton.addEventListener("click", () => void handleFixClick(path, outcome, fixButton));
    item.append(fixButton);
  }

  const compileButton = document.createElement("button");
  compileButton.type = "button";
  compileButton.textContent = "Compile";
  compileButton.classList.add("psc-result__compile-button");
  const compileOutputEl = document.createElement("pre");
  compileOutputEl.classList.add("psc-result__compile-output");
  compileOutputEl.hidden = true;
  compileButton.addEventListener("click", () => void handleCompileClick(path, compileButton, compileOutputEl));
  item.append(compileButton);

  if (visibleFindings.length > 0) {
    const findingsList = document.createElement("ul");
    findingsList.classList.add("psc-result__findings");
    findingsList.replaceChildren(
      ...visibleFindings.map((finding) => {
        const findingItem = document.createElement("li");
        findingItem.classList.add("psc-result__finding");
        const level = levelOf(finding.message);
        if (level) {
          findingItem.classList.add(`psc-result__finding--${level}`);
        }
        findingItem.addEventListener("click", () => void openCodeViewer(path, outcome.findings, finding.line));

        const label = document.createElement("span");
        label.textContent = `line ${finding.line}, col ${finding.column}: ${finding.message}`;
        findingItem.append(label);

        const tagsEl = buildFindingTagsEl(finding);
        if (tagsEl) {
          findingItem.append(tagsEl);
        }

        if (isFixableFinding(finding)) {
          const fixIssueButton = document.createElement("button");
          fixIssueButton.type = "button";
          fixIssueButton.textContent = "Fix this issue";
          fixIssueButton.classList.add("psc-result__finding-fix-button");
          const fixIssueErrorEl = document.createElement("span");
          fixIssueErrorEl.classList.add("psc-result__finding-fix-error");
          fixIssueErrorEl.hidden = true;
          fixIssueButton.addEventListener("click", (event) => {
            event.stopPropagation();
            void handleFixIssueClick(path, outcome, finding, fixIssueButton, fixIssueErrorEl);
          });
          findingItem.append(fixIssueButton, fixIssueErrorEl);
        }

        return findingItem;
      }),
    );
    item.append(findingsList);
  }

  item.append(compileOutputEl);

  return item;
}

// One file's worth of findings that currently pass every active filter
// (filename search, severity, tag, rule), as gathered by
// collectFilteredIssues below for the "Export issues" button.
export interface FilteredIssuesFile {
  path: string;
  findings: Diagnostic[];
}

// Gathers every finding currently visible in the Lint results list - i.e.
// the same set buildPscResultItem renders, grouped by file - for the
// "Export issues" button. A file that doesn't match the filename filter,
// or has no findings passing the severity/tag/rule filters (including one
// that failed to parse, which has none at all), is omitted entirely, since
// there's nothing to export for it.
export function collectFilteredIssues(outcomes: PscParseOutcome[]): FilteredIssuesFile[] {
  const files: FilteredIssuesFile[] = [];
  for (const outcome of outcomes) {
    const path = relativePath(outcome.path, currentProjectDir);
    if (!matchesFilenameFilter(path, currentFilenameFilter)) {
      continue;
    }
    const findings = findingsPassingActiveFilters(outcome.findings);
    if (findings.length === 0) {
      continue;
    }
    files.push({ path, findings });
  }
  return files;
}

// Renders `files` the same way the CLI's plain-text report does (see
// format_diagnostic_line in papyrus-lint-cli), one finding per line, so the
// exported text stays familiar to anyone who's already used the CLI's
// output.
export function formatIssuesAsText(files: FilteredIssuesFile[]): string {
  const lines: string[] = [];
  for (const file of files) {
    for (const finding of file.findings) {
      lines.push(`${file.path}:${finding.line}:${finding.column}: [${finding.rule ?? "unknown"}] ${finding.message}`);
    }
  }
  return lines.join("\n");
}

// Renders `files` as JSON, mirroring the shape of the CLI's own `--json`
// report (JsonReport/JsonFileReport/JsonDiagnostic in
// papyrus-lint-cli/src/lib.rs) so both can be consumed by the same tooling.
export function formatIssuesAsJson(files: FilteredIssuesFile[]): string {
  let totalDiagnostics = 0;
  const jsonFiles = files.map((file) => {
    totalDiagnostics += file.findings.length;
    return {
      path: file.path,
      diagnostics: file.findings.map((finding) => ({
        line: finding.line,
        column: finding.column,
        rule: finding.rule ?? "unknown",
        level: severityOf(finding.message),
        message: finding.message,
      })),
    };
  });
  return JSON.stringify(
    {
      files: jsonFiles,
      files_with_diagnostics: jsonFiles.length,
      total_diagnostics: totalDiagnostics,
    },
    null,
    2,
  );
}

// Triggers a browser "Save As" download of `contents` named `filename`, via
// a throwaway Blob URL and a clicked anchor element - the standard
// technique for a framework-free page, and one the desktop app's own
// WebView handles the same way an ordinary browser does, without needing a
// Tauri fs/dialog plugin.
function downloadTextFile(filename: string, contents: string, mimeType: string) {
  const blob = new Blob([contents], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  // Download processing can be asynchronous, so the WebView may still need
  // the URL after this task finishes; revoking it only once the event loop
  // is free again avoids racing an in-progress download.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

// Enables the "Export issues" button only while there's at least one
// currently filtered finding to export.
export function updateExportIssuesButtonState(outcomes: PscParseOutcome[]) {
  if (exportIssuesButtonEl) {
    exportIssuesButtonEl.disabled = collectFilteredIssues(outcomes).length === 0;
  }
}

// Downloads the currently filtered lint findings (see collectFilteredIssues)
// as a single text or JSON file, per the "Export format" selector.
export function handleExportIssuesClick() {
  const files = collectFilteredIssues(currentPscOutcomes);
  if (files.length === 0) {
    return;
  }
  if (exportFormatEl?.value === "json") {
    downloadTextFile("papyrus-lint-issues.json", formatIssuesAsJson(files), "application/json");
  } else {
    downloadTextFile("papyrus-lint-issues.txt", formatIssuesAsText(files), "text/plain");
  }
}

// Builds/refreshes the "mass fix" panel listing every rule with at least
// one fixable finding somewhere in `outcomes`, each with a button that
// clears every occurrence of that one rule across every file at once (see
// handleMassFixClick). Hides the panel entirely once no rule qualifies
// (e.g. right after the last one has been mass-fixed).
export function renderMassFixList(outcomes: PscParseOutcome[]) {
  if (!pscResultMassFixEl || !pscResultMassFixListEl) {
    return;
  }

  const counts = massFixRuleCounts(outcomes);
  if (counts.size === 0) {
    pscResultMassFixListEl.replaceChildren();
    pscResultMassFixEl.setAttribute("hidden", "");
    return;
  }

  const rules = [...counts.keys()].sort((a, b) =>
    massFixRuleDisplayName(a).localeCompare(massFixRuleDisplayName(b)),
  );
  pscResultMassFixListEl.replaceChildren(
    ...rules.map((rule) => {
      const count = counts.get(rule) ?? 0;
      const item = document.createElement("li");
      item.classList.add("psc-result__mass-fix-item");

      const label = document.createElement("span");
      label.textContent = `${massFixRuleDisplayName(rule)} (${count})`;
      item.append(label);

      const button = document.createElement("button");
      button.type = "button";
      button.textContent = count === 1 ? "Fix this issue everywhere" : `Fix all ${count} in project`;
      button.classList.add("psc-result__mass-fix-button");
      button.addEventListener("click", () => void handleMassFixClick(rule, outcomes, button));
      item.append(button);

      return item;
    }),
  );
  pscResultMassFixEl.removeAttribute("hidden");
}

export function renderPscResults(outcomes: PscParseOutcome[]) {
  renderMassFixList(outcomes);

  if (!pscResultEl || !pscResultListEl) {
    return;
  }

  if (outcomes.length === 0) {
    pscResultEl.setAttribute("hidden", "");
    return;
  }

  const items = outcomes.map(buildPscResultItem).filter((item): item is HTMLLIElement => item !== null);
  pscResultListEl.replaceChildren(...items);
  pscResultEl.removeAttribute("hidden");
  updateExportIssuesButtonState(outcomes);
  switchTab("lint");
}

export async function handleFixClick(path: string, outcome: PscParseOutcome, button: HTMLButtonElement) {
  button.disabled = true;
  try {
    outcome.findings = await repairPscFile(path);
  } catch (error) {
    console.error(error);
  } finally {
    renderPscResults(currentPscOutcomes);
  }
}

// The code viewer's own "Apply fixes" button: applies every automatic fix
// in the currently open file (the same repair handleFixClick performs from
// the Lint results list), then refreshes the viewer's source/findings in
// place and, since the fix also touches the file on disk, re-syncs the
// matching Lint results list entry - so acting on a file no longer requires
// closing the viewer first.
export async function handleCodeViewerFixClick() {
  if (!codeViewerState || !codeViewerFixButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  codeViewerFixButtonEl.disabled = true;
  try {
    const findings = await repairPscFile(path);
    const source = await invoke<string>("read_psc_file", { path });
    codeViewerState = { path, source, findings };
    renderCodeViewerView(source, findings);

    const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
    if (outcome) {
      outcome.findings = findings;
      renderPscResults(currentPscOutcomes);
    }
  } catch (error) {
    console.error(error);
  } finally {
    updateCodeViewerFixButtonVisibility();
    if (codeViewerFixButtonEl) {
      codeViewerFixButtonEl.disabled = false;
    }
  }
}

// Mass-fixes `rule` (an id from FIXABLE_RULE_IDS) across every file in
// `outcomes` that currently has a fixable finding for it, then re-renders
// the whole results list (including this panel) once every file has been
// repaired. A single file's fix failing (e.g. an I/O error) is logged and
// otherwise ignored, the same way handleFixClick treats a failed whole-file
// repair, so one bad file can't stop the rest of the project from being
// fixed.
export async function handleMassFixClick(rule: string, outcomes: PscParseOutcome[], button: HTMLButtonElement) {
  button.disabled = true;
  try {
    const targets = outcomes.filter((outcome) =>
      outcome.findings.some((finding) => finding.rule === rule && isFixableFinding(finding)),
    );
    await Promise.all(
      targets.map(async (outcome) => {
        try {
          outcome.findings = await repairPscFileRule(outcome.path, rule);
        } catch (error) {
          console.error(error);
        }
      }),
    );
  } finally {
    renderPscResults(currentPscOutcomes);
  }
}

// Applies just `finding`'s own automatic fix (see repairPscFinding), rather
// than every fixable finding in the file. On success, re-renders the whole
// results list like handleFixClick does. On failure (e.g. the fix would
// change the file's line count elsewhere, like property-sorting relocating
// a declaration), leaves the list as-is and shows `error` inline next to
// the finding instead, since a full re-render would just discard it.
export async function handleFixIssueClick(
  path: string,
  outcome: PscParseOutcome,
  finding: Diagnostic,
  button: HTMLButtonElement,
  errorEl: HTMLElement,
) {
  if (!finding.rule) {
    return;
  }
  button.disabled = true;
  errorEl.hidden = true;
  errorEl.textContent = "";
  try {
    outcome.findings = await repairPscFinding(path, finding.rule, finding.line);
    renderPscResults(currentPscOutcomes);
  } catch (error) {
    console.error(error);
    errorEl.textContent = String(error);
    errorEl.hidden = false;
    button.disabled = false;
  }
}

// Shows `text` in `outputEl`, styling it as a success or failure so a
// failed compile is easy to spot at a glance.
function showCompileOutput(outputEl: HTMLElement, text: string, success: boolean) {
  outputEl.textContent = text;
  outputEl.hidden = false;
  outputEl.classList.toggle("psc-result__compile-output--ok", success);
  outputEl.classList.toggle("psc-result__compile-output--error", !success);
}

// Hides and clears a previous compile result, if any is showing (e.g. from
// an earlier file in the same code viewer session).
function hideCompileOutput(outputEl: HTMLElement | null) {
  if (!outputEl) {
    return;
  }
  outputEl.hidden = true;
  outputEl.textContent = "";
  outputEl.classList.remove("psc-result__compile-output--ok", "psc-result__compile-output--error");
}

// Compiles `path` via PapyrusCompiler.exe and shows the result in
// `outputEl`, reporting both a successful compile and a compiler-reported
// failure (syntax errors, missing imports, etc.) as well as a failure to
// run the compiler at all (e.g. no path configured). Shared by the "Compile"
// button on the Lint results list and the code viewer's "Save & Compile"
// button.
async function compileAndShowOutput(path: string, outputEl: HTMLElement): Promise<void> {
  try {
    const outcome = await compilePscFile(path);
    const lines = [outcome.stdout, outcome.stderr].filter((text) => text.trim().length > 0);
    if (outcome.personal_data_stripped) {
      lines.push("Removed your username/computer name from the compiled script.");
    }
    const output = lines.join("\n");
    showCompileOutput(
      outputEl,
      output || (outcome.success ? "Compiled successfully." : "Compilation failed."),
      outcome.success,
    );
  } catch (error) {
    showCompileOutput(outputEl, String(error), false);
    console.error(error);
  }
}

// Compiles `path` via PapyrusCompiler.exe when the "Compile" button is
// clicked.
export async function handleCompileClick(path: string, button: HTMLButtonElement, outputEl: HTMLElement) {
  button.disabled = true;
  const originalLabel = button.textContent;
  button.textContent = "Compiling…";
  try {
    await compileAndShowOutput(path, outputEl);
  } finally {
    button.disabled = false;
    button.textContent = originalLabel;
  }
}

// Remembers `dir` as the last project opened, so its config file can be
// read again the next time the app starts.
export function rememberProjectDir(dir: string) {
  try {
    localStorage.setItem(LAST_PROJECT_DIR_KEY, dir);
  } catch (error) {
    console.error(error);
  }
}

export function lastProjectDir(): string | null {
  try {
    return localStorage.getItem(LAST_PROJECT_DIR_KEY);
  } catch (error) {
    console.error(error);
    return null;
  }
}

// Reads the Settings tab's "Configuration file" override input, trimmed. An
// empty string means no override is set, so the lint config is auto-detected
// from the current project directory as usual.
export function configPathOverride(): string {
  return configPathOverrideEl?.value.trim() ?? "";
}

// Locks (while `locked`) or unlocks the entire Settings tab. A project's
// configuration is picked per drop (see promptForConfigSelection/
// useProjectDir below), so until that pick is made for the
// currently-loading project, the Settings tab must not be shown/editable at
// all - otherwise it'd display (and let the user edit) the previous
// project's configuration, or the engine's silent defaults, before this
// drop's own configuration is even known, which doesn't make sense once
// more than one project is involved. The native <fieldset disabled>
// wrapping every Settings tab control (settingsFieldsetEl) handles
// keyboard/mouse interaction and accessibility on its own; the notice
// paragraph is a sibling of that fieldset (so it stays visible/announced
// while locked) explaining why the tab is inert.
export function setSettingsLocked(locked: boolean) {
  if (settingsFieldsetEl) {
    settingsFieldsetEl.disabled = locked;
  }
  if (settingsLockedNoticeEl) {
    settingsLockedNoticeEl.hidden = !locked;
  }
}

// Applies `theme` to the document: "system" removes any override, leaving
// the prefers-color-scheme media query in styles.css in control; "light"
// and "dark" set a data-theme attribute that overrides it.
export function applyTheme(theme: Theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

export function storeTheme(theme: Theme) {
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch (error) {
    console.error(error);
  }
}

// Reads the persisted theme choice, defaulting to "system" (also used when
// storage is unavailable or holds something unrecognized).
export function loadStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    return THEMES.includes(stored as Theme) ? (stored as Theme) : "system";
  } catch (error) {
    console.error(error);
    return "system";
  }
}

export async function useProjectDir(dir: string) {
  currentProjectDir = dir;
  const override = configPathOverride();
  const projectInfo = override ? null : await loadProjectInfo(dir);

  currentLintConfig = override ? await loadLintConfigFromPath(override) : await loadLintConfig(dir);
  applyLintConfigToUI(currentLintConfig);
  currentCompilerPath = await loadCompilerPath(dir);
  if (compilerPathEl) {
    compilerPathEl.value = currentCompilerPath;
  }
  currentCompileCheck = await loadCompileCheck(dir);
  if (compileCheckEl) {
    compileCheckEl.checked = currentCompileCheck;
  }
  currentScriptRoots = await loadScriptRoots(dir);
  applyScriptRootsToUI(currentScriptRoots);
  applyProjectInfoToUI(projectInfo ?? (await loadProjectInfo(dir)));
  if (override && usedConfigurationFileEl) {
    usedConfigurationFileEl.textContent = override;
  }
  rememberProjectDir(dir);
}

// Project directories already confirmed via promptForConfigSelection this
// session (see loadProjectConfig below), so re-linting the same project
// again (e.g. dropping the same achlist a second time) doesn't re-show the
// picker every time - only a directory not yet seen this session needs to
// go through it.
const confirmedProjectDirs = new Set<string>();

// Exposed for tests only: forgets every directory confirmed this session,
// so a test reusing the same directory string as an earlier one isn't
// short-circuited by that earlier test's confirmation.
export function resetConfirmedProjectDirs() {
  confirmedProjectDirs.clear();
}

// The entry point every real drop (handleDroppedPaths) and the app's own
// startup restore of the last project directory call instead of
// useProjectDir directly: it's what actually picks `dir`'s configuration
// (via promptForConfigSelection, unless `dir` was already confirmed this
// session) before handing off to useProjectDir to load and apply it,
// keeping the Settings tab locked for the whole of that pick (see
// setSettingsLocked) so it can never show/edit a configuration before one
// has actually been chosen for the project in play. useProjectDir itself
// stays reusable on its own (as plenty of tests do) for just loading an
// already-decided directory's configuration, without going through the
// picker at all.
export async function loadProjectConfig(dir: string): Promise<void> {
  if (!confirmedProjectDirs.has(dir)) {
    setSettingsLocked(true);
    const decision = await promptForConfigSelection(await loadProjectInfo(dir));
    if (decision.kind === "preset") {
      await applyConfigPreset(dir, decision.preset);
    }
    if (configPathOverrideEl) {
      configPathOverrideEl.value = decision.kind === "path" ? decision.path : "";
    }
    confirmedProjectDirs.add(dir);
  }
  await useProjectDir(dir);
  setSettingsLocked(false);
}

// Called when the "Configuration file" override input changes: reloads the
// current project's lint configuration from the new source (the override
// path, or back to auto-detection if it was cleared). Only reachable once
// the Settings tab is unlocked, i.e. after that project's configuration has
// already been picked via loadProjectConfig, so this never re-shows that
// picker - it's ordinary editing of an already-picked configuration.
export async function handleConfigPathOverrideChanged() {
  lintResultsStale = true;
  if (currentProjectDir) {
    await useProjectDir(currentProjectDir);
    // useProjectDir may have replaced currentLintConfig (and the other
    // settings it reloads) after a relint already ran against the old
    // values, if the Lint results tab was clicked while this reload was
    // still in flight (relintCurrentFiles clears lintResultsStale as soon
    // as it starts, well before this await resolves). Re-marking it stale
    // here, unconditionally, is what makes the next tab switch re-lint
    // against the config this reload actually settled on, regardless of
    // whether that race happened.
    lintResultsStale = true;
  }
}

// Called when the PapyrusCompiler.exe path input changes: updates the path
// used by the "Compile" button and persists it to the current project's
// config file (if a project is loaded).
export function handleCompilerPathChanged() {
  currentCompilerPath = compilerPathEl?.value ?? "";
  lintResultsStale = true;
  if (currentProjectDir && compilerPathEl) {
    void saveCompilerPath(currentProjectDir, compilerPathEl.value);
  }
}

// Called when the "Also check with PapyrusCompiler.exe while linting"
// checkbox changes: updates whether lintPscFile/repairPscFile include
// compiler-reported errors, and persists the choice to the current
// project's config file (if a project is loaded).
export function handleCompileCheckChanged() {
  currentCompileCheck = compileCheckEl?.checked ?? false;
  lintResultsStale = true;
  if (currentProjectDir) {
    void saveCompileCheck(currentProjectDir, currentCompileCheck);
  }
}

// Splits the additional script roots textarea's value into one directory
// per non-blank line.
export function scriptRootsFromUI(): string[] {
  return (scriptRootsEl?.value ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

// Reflects `roots` onto the additional script roots textarea, one per line.
export function applyScriptRootsToUI(roots: string[]) {
  if (scriptRootsEl) {
    scriptRootsEl.value = roots.join("\n");
  }
}

// Called when the additional script roots textarea changes: updates the
// roots used to resolve cross-script lookups/the compiler's -i argument,
// and persists them to the current project's config file (if a project is
// loaded).
export function handleScriptRootsChanged() {
  currentScriptRoots = scriptRootsFromUI();
  lintResultsStale = true;
  if (currentProjectDir) {
    void saveScriptRoots(currentProjectDir, currentScriptRoots);
  }
}

// Compiles the `.psc` file at `path` with the currently configured
// PapyrusCompiler.exe path, reproducing the invocation Creation Kit
// tooling uses to compile a single script out of its source directory.
export async function compilePscFile(path: string): Promise<CompileOutcome> {
  return invoke<CompileOutcome>("compile_psc_file", {
    path,
    compilerPath: currentCompilerPath,
    additionalRoots: effectiveScriptRoots(),
  });
}

// A bare `.psc` file conventionally lives two directories under the project
// root (e.g. `Data/Scripts/Source/abc.psc` under `Data`), matching
// papyrus-lint-cli's handling of a `.psc` path given directly (see its
// `root_ancestor_levels`) so a project's `papyrus-lint.yaml` and
// cross-script lookups are still found for a script dropped on its own,
// without an `.achlist`.
export function projectDirForPscPath(path: string): string {
  return dirnameOf(dirnameOf(dirnameOf(path)));
}

// How long the finished progress bar stays visible before
// scheduleHideLintProgress() hides it, so a run that finishes quickly
// doesn't just flash on and off.
const LINT_PROGRESS_HIDE_DELAY_MS = 2000;

// Shows the progress bar reset to 0/`total`, for a drop about to start
// parsing/linting `total` files.
export function showLintProgress(total: number) {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  if (total === 0) {
    lintProgressEl.hidden = true;
    return;
  }
  lintProgressBarEl.max = total;
  lintProgressBarEl.value = 0;
  lintProgressLabelEl.textContent = `Linting 0 / ${total} files`;
  lintProgressEl.hidden = false;
}

export function updateLintProgress(processed: number, total: number) {
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  lintProgressBarEl.value = processed;
  lintProgressLabelEl.textContent = `Linting ${processed} / ${total} files`;
}

export function hideLintProgress() {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
  if (lintProgressEl) {
    lintProgressEl.hidden = true;
  }
}

// Hides the progress bar after a short grace period instead of instantly,
// so the finished state stays visible long enough to register before it
// disappears. A drop that starts again in the meantime (showLintProgress)
// cancels this timer, so the bar isn't hidden out from under it.
export function scheduleHideLintProgress(delayMs = LINT_PROGRESS_HIDE_DELAY_MS) {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
  }
  lintProgressHideTimer = window.setTimeout(() => {
    lintProgressHideTimer = null;
    hideLintProgress();
  }, delayMs);
}

export async function handleDroppedPaths(paths: string[]) {
  const achlistPath = paths.find(isAchlistPath);

  if (achlistPath) {
    try {
      const entries = await invoke<string[]>("parse_achlist_file", {
        path: achlistPath,
      });
      clearError();
      // Cleared before rendering so a View click during the parse/lint
      // pass below can't show a previous drop's stale findings for a
      // path that happens to match one of this drop's entries.
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = projectDirForAchlist(achlistPath, entries);
      showResult(achlistPath, entries, projectDir);
      renderPscResults(currentPscOutcomes);

      await loadProjectConfig(projectDir);
      currentAchlistScriptRoots = scriptRootsForAchlist(entries);
      const pscEntries = entries.filter(isPscPath);
      showLintProgress(pscEntries.length);
      await parsePscFiles(pscEntries, (outcome) => {
        // A newer drop started (and so already reset currentPscOutcomes to
        // its own array) while this one was still parsing/linting; don't
        // mix this stale outcome into it.
        if (generation !== currentParseGeneration) {
          return;
        }
        currentPscOutcomes.push(outcome);
        renderPscResults(currentPscOutcomes);
        updateLintProgress(currentPscOutcomes.length, pscEntries.length);
      });
      if (generation === currentParseGeneration) {
        scheduleHideLintProgress();
      }
    } catch (error) {
      showError("Failed to read that .achlist file. Please try again.");
      console.error(error);
    }
    return;
  }

  if (paths.length === 1 && isPscPath(paths[0])) {
    const pscPath = paths[0];
    clearError();
    currentPscOutcomes = [];
    lintResultsStale = false;
    const generation = ++currentParseGeneration;
    showResult(pscPath, [pscPath], projectDirForPscPath(pscPath));
    renderPscResults(currentPscOutcomes);

    await loadProjectConfig(projectDirForPscPath(pscPath));
    currentAchlistScriptRoots = [];
    showLintProgress(1);
    await parsePscFiles([pscPath], (outcome) => {
      if (generation !== currentParseGeneration) {
        return;
      }
      currentPscOutcomes.push(outcome);
      renderPscResults(currentPscOutcomes);
      updateLintProgress(currentPscOutcomes.length, 1);
    });
    if (generation === currentParseGeneration) {
      scheduleHideLintProgress();
    }
    return;
  }

  // Neither an .achlist nor a single .psc: try treating the single dropped
  // path as a directory to scan recursively for .psc files, for a project
  // (e.g. Requiem's own layout) with no .achlist at all whose scripts are
  // spread across arbitrarily nested subfolders. list_psc_files_recursively
  // errors out if the path isn't actually a directory, so that case falls
  // through to the usual error message below.
  if (paths.length === 1) {
    const dirPath = paths[0];
    try {
      const entries = await invoke<string[]>("list_psc_files_recursively", {
        path: dirPath,
      });
      clearError();
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = projectDirForDirectory(dirPath, entries);
      showResult(dirPath, entries, projectDir);
      renderPscResults(currentPscOutcomes);

      await loadProjectConfig(projectDir);
      currentAchlistScriptRoots = scriptRootsForAchlist(entries);
      showLintProgress(entries.length);
      await parsePscFiles(entries, (outcome) => {
        if (generation !== currentParseGeneration) {
          return;
        }
        currentPscOutcomes.push(outcome);
        renderPscResults(currentPscOutcomes);
        updateLintProgress(currentPscOutcomes.length, entries.length);
      });
      if (generation === currentParseGeneration) {
        scheduleHideLintProgress();
      }
      return;
    } catch {
      // Not a directory either; fall through to the error below.
    }
  }

  showError("Please drop a single .achlist or .psc file, or a folder to scan recursively.");
}

// Re-lints the same set of files currently shown in the Lint results tab
// against the now-current settings. Called when that tab is switched to
// while lintResultsStale is set, so a settings change (formatting/rule
// config, compiler path, compile-check toggle, script roots, or the
// configuration file override) takes visible effect instead of leaving
// stale findings on screen. Clears the list first (rather than relinting
// in place) so it's obvious a fresh pass is running rather than silently
// showing results that may no longer match the current settings.
export async function relintCurrentFiles() {
  const paths = currentPscOutcomes.map((outcome) => outcome.path);
  if (paths.length === 0) {
    return;
  }
  lintResultsStale = false;
  currentPscOutcomes = [];
  const generation = ++currentParseGeneration;
  switchTab("lint");
  renderPscResults(currentPscOutcomes);

  showLintProgress(paths.length);
  await parsePscFiles(paths, (outcome) => {
    if (generation !== currentParseGeneration) {
      return;
    }
    currentPscOutcomes.push(outcome);
    renderPscResults(currentPscOutcomes);
    updateLintProgress(currentPscOutcomes.length, paths.length);
  });
  if (generation === currentParseGeneration) {
    scheduleHideLintProgress();
  }
}

window.addEventListener("DOMContentLoaded", () => {
  appVersionEl = document.querySelector("#app-version");
  dropZoneEl = document.querySelector("#drop-zone");
  dropZoneErrorEl = document.querySelector("#drop-zone-error");
  resultEl = document.querySelector("#achlist-result");
  resultTitleEl = document.querySelector("#achlist-result-title");
  resultListEl = document.querySelector("#achlist-result-list");
  pscResultEl = document.querySelector("#psc-result");
  pscResultListEl = document.querySelector("#psc-result-list");
  pscResultMassFixEl = document.querySelector("#psc-result-mass-fix");
  pscResultMassFixListEl = document.querySelector("#psc-result-mass-fix-list");
  lintProgressEl = document.querySelector("#lint-progress");
  lintProgressLabelEl = document.querySelector("#lint-progress-label");
  lintProgressBarEl = document.querySelector("#lint-progress-bar");
  filenameFilterEl = document.querySelector("#filename-filter");
  configPathOverrideEl = document.querySelector("#config-path-override");
  compilerPathEl = document.querySelector("#compiler-path");
  compileCheckEl = document.querySelector("#compile-check");
  scriptRootsEl = document.querySelector("#script-roots");
  detectedScriptRootsEl = document.querySelector("#detected-script-roots");
  usedConfigurationFileEl = document.querySelector("#used-configuration-file");
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
  codeViewerEl = document.querySelector("#code-viewer");
  codeViewerTitleEl = document.querySelector("#code-viewer-title");
  codeViewerCloseEl = document.querySelector("#code-viewer-close");
  codeViewerViewEl = document.querySelector("#code-viewer-view");
  codeViewerEditEl = document.querySelector("#code-viewer-editor");
  codeViewerEditGutterEl = document.querySelector("#code-viewer-editor-gutter");
  codeViewerEditHighlightEl = document.querySelector("#code-viewer-editor-highlight");
  codeViewerEditTextareaEl = document.querySelector("#code-viewer-editor-textarea");
  codeViewerEditButtonEl = document.querySelector("#code-viewer-edit");
  codeViewerFixButtonEl = document.querySelector("#code-viewer-fix");
  codeViewerSaveButtonEl = document.querySelector("#code-viewer-save");
  codeViewerSaveCompileButtonEl = document.querySelector("#code-viewer-save-compile");
  codeViewerCancelButtonEl = document.querySelector("#code-viewer-cancel");
  codeViewerCompileOutputEl = document.querySelector("#code-viewer-compile-output");
  codeViewerFullscreenEl = document.querySelector("#code-viewer-fullscreen");
  codeViewerAutocompleteEl = document.querySelector("#code-viewer-autocomplete");
  themeSelectEl = document.querySelector("#theme-select");
  autoFixableFilterEl = document.querySelector("#filter-auto-fixable-only");
  ruleFilterSelectEls = Object.fromEntries(
    TAG_KINDS.map((kind) => [kind, document.querySelector<HTMLSelectElement>(`#filter-rule-${kind}`)]),
  ) as Partial<Record<TagKind, HTMLSelectElement>>;
  exportFormatEl = document.querySelector("#export-format");
  exportIssuesButtonEl = document.querySelector("#export-issues-button");
  saveConfigAsPresetButtonEl = document.querySelector("#save-config-as-preset");
  saveConfigAsPresetButtonEl?.addEventListener("click", () => void handleSaveConfigAsPresetClick());
  presetManagementTabEl = document.querySelector("#tab-presets");
  presetManagementListEl = document.querySelector("#preset-management-list");

  settingsFieldsetEl = document.querySelector("#settings-fieldset");
  settingsLockedNoticeEl = document.querySelector("#settings-locked-notice");
  configPickerEl = document.querySelector("#config-picker");
  configPickerDetectedEl = document.querySelector("#config-picker-detected");
  configPickerDetectedPathEl = document.querySelector("#config-picker-detected-path");
  configPickerNoneEl = document.querySelector("#config-picker-none");
  configPickerPresetListEl = document.querySelector("#config-picker-preset-list");
  configPickerPathInputEl = document.querySelector("#config-picker-path-input");
  configPickerUsePathButtonEl = document.querySelector("#config-picker-use-path");
  configPickerContinueEl = document.querySelector("#config-picker-continue");
  configPickerEl?.addEventListener("click", (event) => {
    if (event.target === configPickerEl) {
      configPickerEl?.close();
    }
  });
  // No project's configuration has been picked yet at startup, so the
  // Settings tab starts locked (see setSettingsLocked/useProjectDir); the
  // markup itself also starts with the wrapping fieldset disabled, so this
  // just keeps the notice paragraph in sync with it from the start.
  setSettingsLocked(true);

  const initialTheme = loadStoredTheme();
  if (themeSelectEl) {
    themeSelectEl.value = initialTheme;
  }
  applyTheme(initialTheme);
  themeSelectEl?.addEventListener("change", () => {
    const theme = (themeSelectEl?.value ?? "system") as Theme;
    storeTheme(theme);
    applyTheme(theme);
  });

  codeViewerCloseEl?.addEventListener("click", () => requestCloseCodeViewer());
  codeViewerFullscreenEl?.addEventListener("click", toggleCodeViewerFullscreen);
  codeViewerEl?.addEventListener("click", (event) => {
    if (event.target === codeViewerEl) {
      requestCloseCodeViewer();
    }
  });
  codeViewerEl?.addEventListener("cancel", (event) => {
    if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
      event.preventDefault();
    }
  });
  codeViewerEl?.addEventListener("close", () => setCodeViewerMode("view"));

  codeViewerEditButtonEl?.addEventListener("click", () => enterCodeViewerEditMode());
  codeViewerFixButtonEl?.addEventListener("click", () => void handleCodeViewerFixClick());
  codeViewerCancelButtonEl?.addEventListener("click", () => cancelCodeViewerEditMode());
  codeViewerSaveButtonEl?.addEventListener("click", () => void saveCodeViewerEdits());
  codeViewerSaveCompileButtonEl?.addEventListener("click", () => void saveAndCompileCodeViewerEdits());
  codeViewerEditTextareaEl?.addEventListener("input", () => updateCodeViewerEditHighlight());
  codeViewerEditTextareaEl?.addEventListener("input", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("click", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleAutocompleteKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("blur", () => hideAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("mousemove", (event) => {
    codeViewerEditLastMouseY = event.clientY;
    updateCodeViewerEditTooltip(event.clientY);
  });
  codeViewerEditTextareaEl?.addEventListener("mouseleave", () => {
    codeViewerEditLastMouseY = null;
    if (codeViewerEditTextareaEl) {
      codeViewerEditTextareaEl.title = "";
    }
  });
  codeViewerEditTextareaEl?.addEventListener("scroll", () => {
    if (codeViewerEditHighlightEl && codeViewerEditTextareaEl) {
      codeViewerEditHighlightEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
      codeViewerEditHighlightEl.scrollLeft = codeViewerEditTextareaEl.scrollLeft;
    }
    if (codeViewerEditGutterEl && codeViewerEditTextareaEl) {
      // The gutter has no horizontal scrollbar of its own (line numbers
      // never need to scroll sideways), only vertical.
      codeViewerEditGutterEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
    }
    // A wheel scroll or keyboard navigation can bring a different source
    // line under a pointer that never itself moved, so re-evaluate the
    // tooltip against the pointer's last known position instead of leaving
    // it describing whichever line used to be underneath it.
    if (codeViewerEditLastMouseY !== null) {
      updateCodeViewerEditTooltip(codeViewerEditLastMouseY);
    }
  });
  codeViewerEl?.addEventListener("close", () => {
    codeViewerEl?.classList.remove("code-viewer--fullscreen");
    codeViewerFullscreenEl?.setAttribute("aria-pressed", "false");
    codeViewerFullscreenEl?.setAttribute("aria-label", "Enter fullscreen");
  });

  severityFilterEls = Object.fromEntries(
    SEVERITIES.map((severity) => [severity, document.querySelector<HTMLInputElement>(`#filter-${severity}`)]),
  ) as Partial<Record<Severity, HTMLInputElement>>;
  for (const severity of SEVERITIES) {
    severityFilterEls[severity]?.addEventListener("change", () => {
      const checked = severityFilterEls[severity]?.checked ?? true;
      if (checked) {
        activeSeverities.add(severity);
      } else {
        activeSeverities.delete(severity);
      }
      renderPscResults(currentPscOutcomes);
    });
  }

  filenameFilterEl?.addEventListener("input", () => {
    currentFilenameFilter = filenameFilterEl?.value ?? "";
    renderPscResults(currentPscOutcomes);
  });

  tagKindFilterEls = Object.fromEntries(
    TAG_KINDS.map((kind) => [kind, document.querySelector<HTMLInputElement>(`#filter-kind-${kind}`)]),
  ) as Partial<Record<TagKind, HTMLInputElement>>;
  for (const kind of TAG_KINDS) {
    const select = ruleFilterSelectEls[kind];

    // Selecting/deselecting an individual rule in this kind's multiselect.
    select?.addEventListener("change", () => {
      for (const option of select.options) {
        if (option.selected) {
          activeRules.add(option.value);
        } else {
          activeRules.delete(option.value);
        }
      }
      syncRuleFilterSelections();
      renderPscResults(currentPscOutcomes);
    });

    // The kind's own header checkbox: a "select all"/"select none" toggle
    // for every rule in its multiselect, rather than an independent filter
    // dimension of its own (see matchesTagFilters/updateTagKindHeaderCheckbox
    // above).
    tagKindFilterEls[kind]?.addEventListener("change", () => {
      const checked = tagKindFilterEls[kind]?.checked ?? true;
      for (const option of select?.options ?? []) {
        if (checked) {
          activeRules.add(option.value);
        } else {
          activeRules.delete(option.value);
        }
      }
      syncRuleFilterSelections();
      renderPscResults(currentPscOutcomes);
    });
  }

  tagImportanceFilterEls = Object.fromEntries(
    TAG_IMPORTANCES.map((importance) => [
      importance,
      document.querySelector<HTMLInputElement>(`#filter-importance-${importance}`),
    ]),
  ) as Partial<Record<TagImportance, HTMLInputElement>>;
  for (const importance of TAG_IMPORTANCES) {
    tagImportanceFilterEls[importance]?.addEventListener("change", () => {
      const checked = tagImportanceFilterEls[importance]?.checked ?? true;
      if (checked) {
        activeTagImportances.add(importance);
      } else {
        activeTagImportances.delete(importance);
      }
      renderPscResults(currentPscOutcomes);
    });
  }

  autoFixableFilterEl?.addEventListener("change", () => {
    onlyAutoFixable = autoFixableFilterEl?.checked ?? false;
    renderPscResults(currentPscOutcomes);
  });

  exportIssuesButtonEl?.addEventListener("click", () => handleExportIssuesClick());

  configPathOverrideEl?.addEventListener("change", handleConfigPathOverrideChanged);
  compilerPathEl?.addEventListener("change", handleCompilerPathChanged);
  compileCheckEl?.addEventListener("change", handleCompileCheckChanged);
  scriptRootsEl?.addEventListener("change", handleScriptRootsChanged);
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

  for (const id of TAB_IDS) {
    const button = document.querySelector<HTMLButtonElement>(`#tab-${id}`);
    if (id === "lint") {
      // A settings change since the results currently shown were linted
      // (lintResultsStale) means they no longer reflect the active
      // settings; re-lint the same files instead of just showing the tab.
      button?.addEventListener("click", () => {
        if (lintResultsStale && currentPscOutcomes.length > 0) {
          void relintCurrentFiles();
        } else {
          switchTab("lint");
        }
      });
    } else {
      button?.addEventListener("click", () => switchTab(id));
    }
  }
  switchTab("import");

  // Only reach for the Tauri bridge when actually running inside the
  // desktop app's webview: opened as a plain page (e.g. a browser preview,
  // or the CI Lighthouse check against the built frontend), none of these
  // calls have a backend to talk to and would otherwise throw/log errors.
  if (isTauri()) {
    void loadAppVersion().then((version) => {
      if (appVersionEl && version) {
        appVersionEl.textContent = `v${version}`;
      }
    });

    void loadRuleTags().then(applyRuleTags);
    void refreshPresetManagementTab();

    const lastDir = lastProjectDir();
    if (lastDir) {
      void loadProjectConfig(lastDir);
    }

    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        dropZoneEl?.classList.add("drop-zone--active");
      } else if (event.payload.type === "drop") {
        dropZoneEl?.classList.remove("drop-zone--active");
        void handleDroppedPaths(event.payload.paths);
      } else {
        dropZoneEl?.classList.remove("drop-zone--active");
      }
    });
  }
});
