import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { highlightPapyrusLines } from "./highlight";

let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
let pscResultEl: HTMLElement | null;
let pscResultListEl: HTMLElement | null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let currentPscOutcomes: PscParseOutcome[] = [];
let compilerPathEl: HTMLInputElement | null;
let semicolonStyleEl: HTMLSelectElement | null;
let cyclomaticComplexityWarningEl: HTMLInputElement | null;
let cyclomaticComplexityErrorEl: HTMLInputElement | null;
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
let codeViewerCancelButtonEl: HTMLButtonElement | null;
let codeViewerFullscreenEl: HTMLButtonElement | null;

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

export const TAB_IDS = ["import", "settings", "files", "lint"] as const;
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
  numeric_comparison: boolean;
  indentation: boolean;
  cyclomatic_complexity: boolean;
  unreachable_statement: boolean;
  static_condition: boolean;
  unused_local_variable: boolean;
  none_form_usage: boolean;
}

export interface LintConfig {
  semicolon: boolean;
  indentation: "tab" | "space";
  indentation_width: number;
  cyclomatic_complexity_warning: number;
  cyclomatic_complexity_error: number;
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
  numeric_comparison: true,
  indentation: true,
  cyclomatic_complexity: true,
  unreachable_statement: true,
  static_condition: true,
  unused_local_variable: true,
  none_form_usage: true,
};

export const DEFAULT_LINT_CONFIG: LintConfig = {
  semicolon: false,
  indentation: "tab",
  indentation_width: 4,
  cyclomatic_complexity_warning: 10,
  cyclomatic_complexity_error: 20,
  rules: DEFAULT_RULES,
};
const LAST_PROJECT_DIR_KEY = "papyrus-lint:last-project-dir";
export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];

let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;
// The project root (the directory containing the dropped .achlist file),
// also used by the "Argument type check" lint to resolve calls to
// functions declared on other scripts under it.
let currentProjectDir: string | null = null;
// The PapyrusCompiler.exe path to use for the "Compile" button, kept in
// sync with the Settings tab's input (see handleCompilerPathChanged).
let currentCompilerPath = "";

const TRAILING_WHITESPACE_MESSAGE = "Line contains trailing whitespace";

export function dirnameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(0, index);
}

// Formats `path` relative to `base` (the project directory containing the
// dropped .achlist file) for display in the lint results list, so long
// absolute paths stay readable. Falls back to the absolute path if `base`
// isn't known yet or `path` doesn't live under it.
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
    cyclomatic_complexity_warning: Math.max(1, cyclomaticComplexityWarningEl?.valueAsNumber || 10),
    cyclomatic_complexity_error: Math.max(1, cyclomaticComplexityErrorEl?.valueAsNumber || 20),
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
  });
}

async function writePscFile(path: string, contents: string): Promise<void> {
  await invoke("write_psc_file", { path, contents });
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

// Diagnostic messages are prefixed with `[level] ` by the lints that care
// about severity (e.g. `forbidden_functions`); others have no prefix and
// are treated as the "other" severity.
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

function lineSeverityOf(lineFindings: Diagnostic[] | undefined): "error" | "warning" | "info" | "flagged" | null {
  if (!lineFindings || lineFindings.length === 0) {
    return null;
  }
  const levels = new Set(lineFindings.map((finding) => levelOf(finding.message)));
  if (levels.has("error")) return "error";
  if (levels.has("warning")) return "warning";
  if (levels.has("info")) return "info";
  // Lints like trailing-whitespace don't tag a severity level; still mark
  // their line so the finding is visible in the viewer.
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
  if (codeViewerViewEl) codeViewerViewEl.hidden = mode !== "view";
  if (codeViewerEditEl) codeViewerEditEl.hidden = mode !== "edit";
  if (codeViewerEditButtonEl) codeViewerEditButtonEl.hidden = mode !== "view";
  if (codeViewerSaveButtonEl) codeViewerSaveButtonEl.hidden = mode !== "edit";
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
  setCodeViewerMode("edit");
  codeViewerEditTextareaEl.focus();
}

export function cancelCodeViewerEditMode() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  setCodeViewerMode("view");
}

export async function saveCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  const contents = codeViewerEditTextareaEl.value;

  codeViewerSaveButtonEl.disabled = true;
  try {
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

export function showResult(path: string, entries: string[]) {
  if (!resultEl || !resultTitleEl || !resultListEl) {
    return;
  }

  resultTitleEl.textContent = `Loaded ${path}`;
  resultListEl.replaceChildren(
    ...entries.map((entry) => {
      const item = document.createElement("li");
      item.textContent = entry;
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

// Compiles `path` via PapyrusCompiler.exe when the "Compile" button is
// clicked, reporting both a successful compile and a compiler-reported
// failure (syntax errors, missing imports, etc.) as well as a failure to
// run the compiler at all (e.g. no path configured).
export async function handleCompileClick(path: string, button: HTMLButtonElement, outputEl: HTMLElement) {
  button.disabled = true;
  const originalLabel = button.textContent;
  button.textContent = "Compiling…";
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

export async function useProjectDir(dir: string) {
  currentProjectDir = dir;
  currentLintConfig = await loadLintConfig(dir);
  applyLintConfigToUI(currentLintConfig);
  currentCompilerPath = await loadCompilerPath(dir);
  if (compilerPathEl) {
    compilerPathEl.value = currentCompilerPath;
  }
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

// Compiles the `.psc` file at `path` with the currently configured
// PapyrusCompiler.exe path, reproducing the invocation Creation Kit
// tooling uses to compile a single script out of its source directory.
export async function compilePscFile(path: string): Promise<CompileOutcome> {
  return invoke<CompileOutcome>("compile_psc_file", {
    path,
    compilerPath: currentCompilerPath,
  });
}

export async function handleDroppedPaths(paths: string[]) {
  const achlistPath = paths.find(isAchlistPath);

  if (!achlistPath) {
    showError("Please drop a single .achlist file.");
    return;
  }

  try {
    const entries = await invoke<string[]>("parse_achlist_file", {
      path: achlistPath,
    });
    clearError();
    showResult(achlistPath, entries);

    await useProjectDir(dirnameOf(achlistPath));
    currentPscOutcomes = await parsePscFiles(entries.filter(isPscPath));
    renderPscResults(currentPscOutcomes);
  } catch (error) {
    showError("Failed to read that .achlist file. Please try again.");
    console.error(error);
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
  compilerPathEl = document.querySelector("#compiler-path");
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");
  cyclomaticComplexityWarningEl = document.querySelector("#cyclomatic-complexity-warning");
  cyclomaticComplexityErrorEl = document.querySelector("#cyclomatic-complexity-error");
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
  codeViewerCancelButtonEl = document.querySelector("#code-viewer-cancel");
  codeViewerFullscreenEl = document.querySelector("#code-viewer-fullscreen");

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
  codeViewerEditTextareaEl?.addEventListener("input", () => updateCodeViewerEditHighlight());
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

  compilerPathEl?.addEventListener("change", handleCompilerPathChanged);
  semicolonStyleEl?.addEventListener("change", handleLintConfigChanged);
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
    handleLintConfigChanged();
  });
  indentationWidthEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityWarningEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityErrorEl?.addEventListener("change", handleLintConfigChanged);
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
