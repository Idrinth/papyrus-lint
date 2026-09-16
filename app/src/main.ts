import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { type Member } from "./autocomplete";
import { bindPresets, applyConfigPreset, promptForConfigSelection, refreshPresetManagementTab } from "./presets";
import { bindCodeViewer, openCodeViewer } from "./code-viewer";
import { bindLiveEdit } from "./live-edit";
import { bindResultsList, populateRuleFilterGroups, renderPscResults } from "./results-list";


export {
  applyConfigPreset,
  deleteUserPreset,
  exportUserPreset,
  getPresetLintConfig,
  handleDeletePresetClick,
  handleExportPresetClick,
  handleRenamePresetClick,
  handleResetToPresetClick,
  handleSaveConfigAsPresetClick,
  isCustomPreset,
  loadConfigPresets,
  populateResetPresetSelect,
  promptForConfigSelection,
  refreshPresetManagementTab,
  renameUserPreset,
  renderPresetManagementTab,
} from "./presets";
export {
  cancelLiveEditLint,
  applyAutocompleteSelection,
  enterCodeViewerEditMode,
  handleAutocompleteKeydown,
  handleEditorTabKeydown,
  hideAutocomplete,
  isCodeViewerEditDirty,
  cancelCodeViewerEditMode,
  saveAndCompileCodeViewerEdits,
  saveCodeViewerEdits,
  updateAutocomplete,
} from "./live-edit";
export {
  handleCodeViewerFixClick,
  handleCodeViewerFixLineClick,
  handleCodeViewerIgnoreLineClick,
  handleCodeViewerPreviewFixClick,
  openCodeViewer,
  requestCloseCodeViewer,
  toggleCodeViewerFullscreen,
} from "./code-viewer";
export {
  type ActiveFilters,
  type AiSource,
  type FilteredIssuesFile,
  aiConfiguration,
  buildPscResultItem,
  collectFilteredIssues,
  formatIssuesAsJson,
  formatIssuesAsText,
  formatIssuesForAi,
  handleCompileClick,
  handleExportAiClick,
  handleExportIssuesClick,
  handleFixClick,
  handleFixIssueClick,
  handleMassFixClick,
  massFixRuleCounts,
  massFixRuleDisplayName,
  matchesFilenameFilter,
  matchesTagFilters,
  renderMassFixList,
  renderPscResults,
  tagsForFinding,
  updateExportIssuesButtonState,
} from "./results-list";


let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
let lintProgressEl: HTMLElement | null;
let lintProgressLabelEl: HTMLElement | null;
let lintProgressBarEl: HTMLProgressElement | null;
let lintProgressHideTimer: ReturnType<typeof setTimeout> | null = null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let typeCasingStyleEl: HTMLSelectElement | null;
let identifierCasingStyleEl: HTMLSelectElement | null;
let namedArgumentsStyleEl: HTMLSelectElement | null;
let magicNumbersModeEl: HTMLSelectElement | null;
export let currentPscOutcomes: PscParseOutcome[] = [];
// Set whenever a setting affecting lint output (formatting/rule config,
// compiler path, compile-check toggle, additional/lookup script roots, or the
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
let lookupScriptRootsEl: HTMLTextAreaElement | null;
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
let themeSelectEl: HTMLSelectElement | null;
let settingsFieldsetEl: HTMLFieldSetElement | null;
let settingsLockedNoticeEl: HTMLElement | null;

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
  // The rule's detailed description, copied from its row in README.md's
  // Implemented Lints tables (see papyrus_lints::tags::RuleTags).
  description: string;
  kinds: string[];
  importance: TagImportance;
  auto_fixable: boolean;
  // This rule's own documentation link (papyrus_lints::tags::RuleTags::doc_url),
  // for linking a finding straight to its explanation on the project website.
  doc_url: string;
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
  unknown_actor_value: boolean;
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
const THEME_KEY = "papyrus-lint:theme";
export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];

export type Theme = "system" | "light" | "dark";
const THEMES: Theme[] = ["system", "light", "dark"];

export let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;
// The project root (see projectDirForAchlist/projectDirForPscPath), also
// used by the "Argument type check" lint to resolve calls to functions
// declared on other scripts under it.
export let currentProjectDir: string | null = null;
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
// Extra directories searched only as a last-resort fallback when resolving
// a script by name for analysis (cross-script type/function lookups,
// Extends, autocompletion). Scripts found only here are never linted, and
// these directories are never considered by conflicting-script-versions.
// Kept in sync with the Settings tab's "Lookup script roots" textarea.
let currentLookupScriptRoots: string[] = [];
// Source directories inferred from the entries in the currently loaded
// achlist. These are runtime-only roots: unlike currentScriptRoots, they are
// not displayed as user configuration or persisted to papyrus-lint.yaml.
let currentAchlistScriptRoots: string[] = [];
// Every built-in lint rule's tag metadata, keyed by rule id, fetched once
// from the backend (see loadRuleTags) and used both to render each
// finding's tag badges and to drive the tag filters below.
export let ruleTagsByRule: Map<string, RuleTagsInfo> = new Map();

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

// Returns `dir`'s configured analysis-only lookup directories, if any.
// Returns an empty array if none are configured or the lookup fails.
export async function loadLookupScriptRoots(dir: string): Promise<string[]> {
  try {
    return await invoke<string[]>("load_lookup_script_roots", { dir });
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

// Persists `roots` as `dir`'s configured additional script root
// directories.
export async function saveScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_script_roots", { dir, roots });
  } catch (error) {
    console.error(error);
  }
}

// Persists `roots` as `dir`'s configured analysis-only lookup directories.
export async function saveLookupScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_lookup_script_roots", { dir, roots });
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
  lintResultsStale = true;
  const override = configPathOverride();
  if (override) {
    void saveLintConfigToPath(override, currentLintConfig);
  } else if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

// Lints `source` directly, in-process (the same `lint_papyrus_script`
// Tauri command `app/src-tauri/src/lib.rs` wraps around
// `papyrus_lints::lint`), instead of a `.psc` path on disk. Used by the
// code viewer's edit mode for live, as-you-type feedback on the textarea's
// current (possibly unsaved) contents - see `scheduleLiveEditLint` below.
// Unlike `lintPscFile`, this never resolves cross-script lookups (there's
// no project root to resolve them against), the same tradeoff the CLI's
// own `--blob` flag makes for editor extensions that only have the
// buffer's text in memory.
export async function lintPapyrusScript(source: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_papyrus_script", { source, config: currentLintConfig });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      root: currentProjectDir ?? "",
      config: currentLintConfig,
      additionalRoots: effectiveScriptRoots(),
      lookupRoots: currentLookupScriptRoots,
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
    lookupRoots: currentLookupScriptRoots,
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
  });
}

// Computes the same whole-file automatic fix repairPscFile would apply, but
// never writes it to disk: returns a standard unified diff of what would
// change (or an empty string if nothing would), the same output
// `PapyrusLinterCLI fix --dry-run` prints. Drives the code viewer's
// "Preview fixes" button.
export async function previewRepairPscFile(path: string): Promise<string> {
  return invoke<string>("preview_repair_psc_file", {
    path,
    config: currentLintConfig,
  });
}

// Computes just `rule`'s automatic fix and returns what `line` (1-indexed)
// would look like afterward, without writing anything to disk or touching
// any other line - `null` when there's nothing meaningful to show (the fix
// doesn't change the file, would shift the line count elsewhere, or simply
// doesn't touch `line`). Drives formatIssuesForAi's per-finding `repair`
// preview, below.
export async function previewRepairPscLine(path: string, rule: string, line: number): Promise<string | null> {
  try {
    return await invoke<string | null>("preview_repair_psc_line", {
      path,
      config: currentLintConfig,
      rule,
      line,
    });
  } catch (error) {
    console.error(error);
    return null;
  }
}

// Rule ids with an automatic fix (papyrus_lints::FIXABLE_RULE_IDS), used to
// decide which findings offer the per-finding "Fix this issue" button. Kept
// in sync by hand with FIXABLE_RULE_IDS in
// app/crates/papyrus-lints/src/lib.rs.
export const FIXABLE_RULE_IDS = new Set([
  "identifier-casing",
  "slow-functions",
  "semicolon",
  "indentation",
  "property-sorting",
  "comma-spacing",
  "chain-whitespace",
  "exclamation-spacing",
  "operator-spacing",
  "assignment-operator-spacing",
  "type-casing",
  "trailing-whitespace",
  "global-variable-increment",
  "unnecessary-function",
  "unused-import",
]);

// A rule in FIXABLE_RULE_IDS can still report a violation it can't actually
// repair without a substantive rename (e.g. type-casing on a name with
// underscores, such as a compiler-generated fragment script's ScriptName) --
// see papyrus_lints::type_casing::check, which appends this same note to
// such a finding's own message rather than letting a caller assume every
// finding from a "fixable" rule can be fixed.
const NO_AUTOMATIC_FIX_NOTE = "no automatic fix";

export function hasNoAutomaticFix(finding: Diagnostic): boolean {
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
    lookupRoots: currentLookupScriptRoots,
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
    lookupRoots: currentLookupScriptRoots,
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
    rule,
  });
}

// Adds (or extends) an `; @disable <rules>` comment on `line`, silencing
// every named rule there instead of fixing it — the code viewer's per-line
// "Ignore" button, the inverse of repairPscFinding's per-line "Fix" button.
// See add_disable_comment_to_psc_line/papyrus_lints::add_disable_comment on
// the backend for the exact merging rules.
export async function addDisableCommentToPscLine(path: string, rules: string[], line: number): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("add_disable_comment_to_psc_line", {
    path,
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additionalRoots: effectiveScriptRoots(),
    lookupRoots: currentLookupScriptRoots,
    compilerPath: currentCompilerPath,
    compileCheck: currentCompileCheck,
    rules,
    line,
  });
}


export async function writePscFile(path: string, contents: string): Promise<void> {
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
      lookupRoots: currentLookupScriptRoots,
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) => isFixableFinding(finding));
}

// How many scripts parsePscFiles works on at once, mirroring the CLI's own
// --threads default (papyrus_lint_core::parallel::default_thread_count): the
// machine's hardware concurrency, so dropping a large achlist doesn't fire
// every one of its scripts' worth of Tauri commands at the same instant —
// each is dispatched to its own thread, and Rust's own parsing/linting work
// is CPU-bound, so far more in flight than the machine has cores to run them
// on just adds contention without finishing any of them sooner. Falls back
// to 4 when the runtime doesn't report hardwareConcurrency (or reports an
// implausible non-positive value).
function parseConcurrencyLimit(): number {
  const cores = typeof navigator === "object" && navigator ? navigator.hardwareConcurrency : 0;
  return cores && cores > 0 ? cores : 4;
}

// Runs `fn` over every item in `items`, at most `limit` calls in flight at
// once, resolving to their results in `items`' own order regardless of which
// order they actually finish in — the same ordering guarantee
// Promise.all(items.map(fn)) gives, but without starting every call at once.
async function mapWithConcurrency<T, R>(
  items: T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let nextIndex = 0;
  async function worker() {
    while (nextIndex < items.length) {
      const index = nextIndex++;
      results[index] = await fn(items[index], index);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

// Parses and lints every path in `paths`, invoking `onOutcome` (if given) as
// each one finishes rather than waiting for the whole batch — the caller can
// use that to render results incrementally instead of freezing until the
// slowest file completes. Outcomes are otherwise still resolved concurrently
// (up to parseConcurrencyLimit() at once), so `onOutcome` fires in
// completion order, not necessarily `paths`' own order.
export async function parsePscFiles(
  paths: string[],
  onOutcome?: (outcome: PscParseOutcome) => void,
): Promise<PscParseOutcome[]> {
  return mapWithConcurrency(paths, parseConcurrencyLimit(), async (path) => {
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
  });
}

// Diagnostic messages are prefixed with `[level] `; every built-in lint
// tags one, so a message with no recognized prefix never actually occurs in
// practice, but severityOf still classifies it as "error" (matching
// Diagnostic::level()'s own fallback in papyrus-lints/src/lib.rs) rather
// than misclassifying it as something less visible.
export type Severity = "error" | "warning" | "info";
export const SEVERITIES: Severity[] = ["error", "warning", "info"];

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


export function severityOf(message: string): Severity {
  return levelOf(message) ?? "error";
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
  currentLookupScriptRoots = await loadLookupScriptRoots(dir);
  applyLookupScriptRootsToUI(currentLookupScriptRoots);
  applyProjectInfoToUI(projectInfo ?? (await loadProjectInfo(dir)));
  if (override && usedConfigurationFileEl) {
    usedConfigurationFileEl.textContent = override;
  }
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

// The entry point every real drop (handleDroppedPaths) calls instead of
// useProjectDir directly: it's what actually picks `dir`'s configuration
// (via promptForConfigSelection, unless `dir` was already confirmed this
// session) before handing off to useProjectDir to load and apply it,
// keeping the Settings tab locked for the whole of that pick (see
// setSettingsLocked) so it can never show/edit a configuration before one
// has actually been chosen for the project in play. useProjectDir itself
// stays reusable on its own (as plenty of tests do, and as the app's own
// startup restore of the last project directory does) for just loading an
// already-decided directory's configuration, without going through the
// picker at all - the picker is for a project the user is actively
// dropping into the app, not one merely remembered from a previous
// session.
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

// Splits the lookup script roots textarea's value into one directory
// per non-blank line.
export function lookupScriptRootsFromUI(): string[] {
  return (lookupScriptRootsEl?.value ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

// Reflects `roots` onto the lookup script roots textarea, one per line.
export function applyLookupScriptRootsToUI(roots: string[]) {
  if (lookupScriptRootsEl) {
    lookupScriptRootsEl.value = roots.join("\n");
  }
}

// Called when the lookup script roots textarea changes: updates the
// analysis-only fallback directories used to resolve cross-script lookups,
// and persists them to the current project's config file (if a project is
// loaded). These are never mixed into additional script roots, so they are
// not linted and are ignored by conflicting-script-versions.
export function handleLookupScriptRootsChanged() {
  currentLookupScriptRoots = lookupScriptRootsFromUI();
  lintResultsStale = true;
  if (currentProjectDir) {
    void saveLookupScriptRoots(currentProjectDir, currentLookupScriptRoots);
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
      switchTab("lint");
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
    switchTab("lint");
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
      switchTab("lint");
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
  lintProgressEl = document.querySelector("#lint-progress");
  lintProgressLabelEl = document.querySelector("#lint-progress-label");
  lintProgressBarEl = document.querySelector("#lint-progress-bar");
  configPathOverrideEl = document.querySelector("#config-path-override");
  compilerPathEl = document.querySelector("#compiler-path");
  compileCheckEl = document.querySelector("#compile-check");
  scriptRootsEl = document.querySelector("#script-roots");
  lookupScriptRootsEl = document.querySelector("#lookup-script-roots");
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
  bindPresets();
  bindCodeViewer();
  bindLiveEdit();
  bindResultsList();

  themeSelectEl = document.querySelector("#theme-select");
  settingsFieldsetEl = document.querySelector("#settings-fieldset");
  settingsLockedNoticeEl = document.querySelector("#settings-locked-notice");
  // No project's configuration is loaded yet at startup, so the Settings
  // tab starts locked (see setSettingsLocked); the markup itself also
  // starts with the wrapping fieldset disabled, so this just keeps the
  // notice paragraph in sync with it from the start. It stays locked until
  // the user actually drops something this session and loadProjectConfig
  // unlocks it - the app never restores a previous session's project.
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

  configPathOverrideEl?.addEventListener("change", handleConfigPathOverrideChanged);
  compilerPathEl?.addEventListener("change", handleCompilerPathChanged);
  compileCheckEl?.addEventListener("change", handleCompileCheckChanged);
  scriptRootsEl?.addEventListener("change", handleScriptRootsChanged);
  lookupScriptRootsEl?.addEventListener("change", handleLookupScriptRootsChanged);
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
