import { invoke } from "@tauri-apps/api/core";
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
let filenameFilterEl: HTMLInputElement | null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let typeCasingStyleEl: HTMLSelectElement | null;
let identifierCasingStyleEl: HTMLSelectElement | null;
let namedArgumentsStyleEl: HTMLSelectElement | null;
let currentPscOutcomes: PscParseOutcome[] = [];
let compilerPathEl: HTMLInputElement | null;
let scriptRootsEl: HTMLTextAreaElement | null;
let detectedScriptRootsEl: HTMLOutputElement | null;
let usedConfigurationFileEl: HTMLOutputElement | null;
let semicolonStyleEl: HTMLSelectElement | null;
let cyclomaticComplexityWarningEl: HTMLInputElement | null;
let cyclomaticComplexityErrorEl: HTMLInputElement | null;
let minWaitIntervalEl: HTMLInputElement | null;
let failOnWarningEl: HTMLInputElement | null;
let failOnInfoEl: HTMLInputElement | null;
let ruleEls: Partial<Record<keyof LintRules, HTMLInputElement>> = {};
let codeViewerEl: HTMLDialogElement | null;
let codeViewerTitleEl: HTMLElement | null;
let codeViewerCloseEl: HTMLButtonElement | null;
let codeViewerViewEl: HTMLElement | null;
let codeViewerEditEl: HTMLElement | null;
let codeViewerEditHighlightEl: HTMLElement | null;
let codeViewerEditTextareaEl: HTMLTextAreaElement | null;
let codeViewerEditButtonEl: HTMLButtonElement | null;
let codeViewerSaveButtonEl: HTMLButtonElement | null;
let codeViewerSaveCompileButtonEl: HTMLButtonElement | null;
let codeViewerCancelButtonEl: HTMLButtonElement | null;
let codeViewerCompileOutputEl: HTMLElement | null;
let codeViewerFullscreenEl: HTMLButtonElement | null;
let codeViewerAutocompleteEl: HTMLUListElement | null;
let themeSelectEl: HTMLSelectElement | null;

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

export const TAB_IDS = ["import", "settings", "files", "lint", "contact"] as const;
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
}

export interface PscParseOutcome {
  path: string;
  ok: boolean;
  detail: string;
  findings: Diagnostic[];
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

export interface LintRules {
  trailing_whitespace: boolean;
  comma_spacing: boolean;
  forbidden_functions: boolean;
  slow_functions: boolean;
  unused_getter: boolean;
  unused_property: boolean;
  semicolon: boolean;
  float_int_conversion: boolean;
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
  none_form_usage: boolean;
  local_variable_shadowing: boolean;
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
  short_wait_interval: boolean;
  state_function_signature: boolean;
  goto_state: boolean;
  too_many_states: boolean;
  multiple_auto_states: boolean;
}

export type TypeCasingStyle = "PascalCase" | "camelCase" | "lowercase" | "UPPERCASE";
export type IdentifierCasingStyle = "camelCase" | "PascalCase" | "snake_case" | "CONSTANT_CASE";
export type NamedArgumentsStyle = "always" | "instead_of_defaults" | "never";

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
  fail_on_warning: boolean;
  fail_on_info: boolean;
  rules: LintRules;
}

export const DEFAULT_RULES: LintRules = {
  trailing_whitespace: true,
  comma_spacing: true,
  forbidden_functions: true,
  slow_functions: true,
  unused_getter: true,
  unused_property: true,
  semicolon: true,
  float_int_conversion: true,
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
  none_form_usage: true,
  local_variable_shadowing: true,
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
  short_wait_interval: true,
  state_function_signature: true,
  goto_state: true,
  too_many_states: true,
  multiple_auto_states: true,
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
  fail_on_warning: false,
  fail_on_info: false,
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
// Extra directories (besides scripts/source and source/scripts under the
// project root) to search for .psc files when resolving cross-script
// lookups, kept in sync with the Settings tab's textarea (see
// handleScriptRootsChanged).
let currentScriptRoots: string[] = [];

const TRAILING_WHITESPACE_MESSAGE = "[warning] Line contains trailing whitespace";

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

// Persists `roots` as `dir`'s configured additional script root
// directories.
export async function saveScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_script_roots", { dir, roots });
  } catch (error) {
    console.error(error);
  }
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
  if (failOnWarningEl) {
    failOnWarningEl.checked = config.fail_on_warning;
  }
  if (failOnInfoEl) {
    failOnInfoEl.checked = config.fail_on_info;
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
    fail_on_warning: failOnWarningEl?.checked ?? false,
    fail_on_info: failOnInfoEl?.checked ?? false,
    rules,
  };
}

// Called whenever a formatting control changes: updates the in-memory
// config and, if a project directory is known, persists it to disk.
export function handleLintConfigChanged() {
  currentLintConfig = lintConfigFromUI();
  if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

export async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      root: currentProjectDir ?? "",
      config: currentLintConfig,
      additionalRoots: currentScriptRoots,
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
    additionalRoots: currentScriptRoots,
  });
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
      additionalRoots: currentScriptRoots,
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) =>
    finding.message === TRAILING_WHITESPACE_MESSAGE || finding.message.includes("end with a semicolon"),
  );
}

export async function parsePscFiles(paths: string[]): Promise<PscParseOutcome[]> {
  return Promise.all(
    paths.map(async (path) => {
      try {
        const script = await invoke<PapyrusScript>("parse_psc_file", { path });
        const findings = await lintPscFile(path);
        return { path, ok: true, detail: `parsed as "${script.name}"`, findings };
      } catch (error) {
        return { path, ok: false, detail: String(error), findings: [] };
      }
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

// Renders `source`'s syntax-highlighted, read-only table view with
// `findings` marked on their lines. If `focusLine` is given, scrolls that
// line into view and briefly flashes it, so a click on a specific finding
// jumps straight to it.
function renderCodeViewerView(source: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerViewEl) {
    return;
  }

  const findingsByLine = new Map<number, Diagnostic[]>();
  for (const finding of findings) {
    const forLine = findingsByLine.get(finding.line) ?? [];
    forLine.push(finding);
    findingsByLine.set(finding.line, forLine);
  }

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
}

// Re-renders the edit mode's syntax-highlighted overlay from the
// textarea's current value, keeping it in sync as the user types.
function updateCodeViewerEditHighlight() {
  const code = codeViewerEditHighlightEl?.querySelector("code");
  if (!code || !codeViewerEditTextareaEl) {
    return;
  }
  code.innerHTML = highlightPapyrusLines(codeViewerEditTextareaEl.value).join("\n");
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

export function isCodeViewerEditDirty(): boolean {
  return (
    codeViewerMode === "edit" &&
    codeViewerState !== null &&
    codeViewerEditTextareaEl !== null &&
    codeViewerEditTextareaEl.value !== codeViewerState.source
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

  const visibleFindings = findings.filter((finding) => activeSeverities.has(severityOf(finding.message)));

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
        findingItem.textContent = `line ${finding.line}, col ${finding.column}: ${finding.message}`;
        findingItem.classList.add("psc-result__finding");
        const level = levelOf(finding.message);
        if (level) {
          findingItem.classList.add(`psc-result__finding--${level}`);
        }
        findingItem.addEventListener("click", () => void openCodeViewer(path, outcome.findings, finding.line));
        return findingItem;
      }),
    );
    item.append(findingsList);
  }

  item.append(compileOutputEl);

  return item;
}

export function renderPscResults(outcomes: PscParseOutcome[]) {
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
  currentLintConfig = await loadLintConfig(dir);
  applyLintConfigToUI(currentLintConfig);
  currentCompilerPath = await loadCompilerPath(dir);
  if (compilerPathEl) {
    compilerPathEl.value = currentCompilerPath;
  }
  currentScriptRoots = await loadScriptRoots(dir);
  applyScriptRootsToUI(currentScriptRoots);
  applyProjectInfoToUI(await loadProjectInfo(dir));
  rememberProjectDir(dir);
}

// Called when the PapyrusCompiler.exe path input changes: updates the path
// used by the "Compile" button and persists it to the current project's
// config file (if a project is loaded).
export function handleCompilerPathChanged() {
  currentCompilerPath = compilerPathEl?.value ?? "";
  if (currentProjectDir && compilerPathEl) {
    void saveCompilerPath(currentProjectDir, compilerPathEl.value);
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
    additionalRoots: currentScriptRoots,
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
      const projectDir = projectDirForAchlist(achlistPath, entries);
      showResult(achlistPath, entries, projectDir);

      await useProjectDir(projectDir);
      currentPscOutcomes = await parsePscFiles(entries.filter(isPscPath));
      renderPscResults(currentPscOutcomes);
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
    showResult(pscPath, [pscPath], projectDirForPscPath(pscPath));

    await useProjectDir(projectDirForPscPath(pscPath));
    currentPscOutcomes = await parsePscFiles([pscPath]);
    renderPscResults(currentPscOutcomes);
    return;
  }

  showError("Please drop a single .achlist or .psc file.");
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
  filenameFilterEl = document.querySelector("#filename-filter");
  compilerPathEl = document.querySelector("#compiler-path");
  scriptRootsEl = document.querySelector("#script-roots");
  detectedScriptRootsEl = document.querySelector("#detected-script-roots");
  usedConfigurationFileEl = document.querySelector("#used-configuration-file");
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");
  typeCasingStyleEl = document.querySelector("#type-casing-style");
  identifierCasingStyleEl = document.querySelector("#identifier-casing-style");
  namedArgumentsStyleEl = document.querySelector("#named-arguments-style");
  cyclomaticComplexityWarningEl = document.querySelector("#cyclomatic-complexity-warning");
  cyclomaticComplexityErrorEl = document.querySelector("#cyclomatic-complexity-error");
  minWaitIntervalEl = document.querySelector("#min-wait-interval");
  failOnWarningEl = document.querySelector("#fail-on-warning");
  failOnInfoEl = document.querySelector("#fail-on-info");
  ruleEls = Object.fromEntries(
    RULE_KEYS.map((key) => [key, document.querySelector<HTMLInputElement>(`#rule-${key}`)]),
  ) as Partial<Record<keyof LintRules, HTMLInputElement>>;
  codeViewerEl = document.querySelector("#code-viewer");
  codeViewerTitleEl = document.querySelector("#code-viewer-title");
  codeViewerCloseEl = document.querySelector("#code-viewer-close");
  codeViewerViewEl = document.querySelector("#code-viewer-view");
  codeViewerEditEl = document.querySelector("#code-viewer-editor");
  codeViewerEditHighlightEl = document.querySelector("#code-viewer-editor-highlight");
  codeViewerEditTextareaEl = document.querySelector("#code-viewer-editor-textarea");
  codeViewerEditButtonEl = document.querySelector("#code-viewer-edit");
  codeViewerSaveButtonEl = document.querySelector("#code-viewer-save");
  codeViewerSaveCompileButtonEl = document.querySelector("#code-viewer-save-compile");
  codeViewerCancelButtonEl = document.querySelector("#code-viewer-cancel");
  codeViewerCompileOutputEl = document.querySelector("#code-viewer-compile-output");
  codeViewerFullscreenEl = document.querySelector("#code-viewer-fullscreen");
  codeViewerAutocompleteEl = document.querySelector("#code-viewer-autocomplete");
  themeSelectEl = document.querySelector("#theme-select");

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
  codeViewerCancelButtonEl?.addEventListener("click", () => cancelCodeViewerEditMode());
  codeViewerSaveButtonEl?.addEventListener("click", () => void saveCodeViewerEdits());
  codeViewerSaveCompileButtonEl?.addEventListener("click", () => void saveAndCompileCodeViewerEdits());
  codeViewerEditTextareaEl?.addEventListener("input", () => updateCodeViewerEditHighlight());
  codeViewerEditTextareaEl?.addEventListener("input", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("click", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleAutocompleteKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("blur", () => hideAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("scroll", () => {
    if (codeViewerEditHighlightEl && codeViewerEditTextareaEl) {
      codeViewerEditHighlightEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
      codeViewerEditHighlightEl.scrollLeft = codeViewerEditTextareaEl.scrollLeft;
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

  compilerPathEl?.addEventListener("change", handleCompilerPathChanged);
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
  cyclomaticComplexityWarningEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityErrorEl?.addEventListener("change", handleLintConfigChanged);
  minWaitIntervalEl?.addEventListener("change", handleLintConfigChanged);
  failOnWarningEl?.addEventListener("change", handleLintConfigChanged);
  failOnInfoEl?.addEventListener("change", handleLintConfigChanged);
  for (const key of RULE_KEYS) {
    ruleEls[key]?.addEventListener("change", handleLintConfigChanged);
  }

  for (const id of TAB_IDS) {
    document.querySelector<HTMLButtonElement>(`#tab-${id}`)?.addEventListener("click", () => switchTab(id));
  }
  switchTab("import");

  void loadAppVersion().then((version) => {
    if (appVersionEl && version) {
      appVersionEl.textContent = `v${version}`;
    }
  });

  const lastDir = lastProjectDir();
  if (lastDir) {
    void useProjectDir(lastDir);
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
});
