import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { type Member } from "./autocomplete";
import { bindPresets, refreshPresetManagementTab } from "./presets";
import { bindCodeViewer, openCodeViewer } from "./code-viewer";
import { bindLiveEdit } from "./live-edit";
import { bindResultsList, populateRuleFilterGroups, renderPscResults } from "./results-list";
import {
  isAchlistPath,
  isPscPath,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPscPath,
  relativePath,
  scriptRootsForAchlist,
} from "./path";
import { bindLintProgress, scheduleHideLintProgress, showLintProgress, updateLintProgress } from "./progress";
import { bindConfigSettings, currentLintConfig } from "./config";
import {
  bindProjectSettings,
  currentCompileCheck,
  currentCompilerPath,
  currentLookupScriptRoots,
  currentProjectDir,
  effectiveScriptRoots,
  loadProjectConfig,
  setAchlistScriptRoots,
} from "./project";


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
export {
  dirnameOf,
  findCandidatePairRoot,
  isAchlistPath,
  isPscPath,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPscPath,
  relativePath,
  scriptRootsForAchlist,
} from "./path";
export {
  hideLintProgress,
  scheduleHideLintProgress,
  showLintProgress,
  updateLintProgress,
} from "./progress";
export {
  applyLintConfigToUI,
  currentLintConfig,
  DEFAULT_LINT_CONFIG,
  DEFAULT_RULES,
  handleLintConfigChanged,
  lintConfigFromUI,
  loadLintConfig,
  loadLintConfigFromPath,
  RULE_KEYS,
  saveLintConfig,
  saveLintConfigToPath,
  type IdentifierCasingStyle,
  type LintConfig,
  type LintRules,
  type MagicNumbersMode,
  type NamedArgumentsStyle,
  type TypeCasingStyle,
} from "./config";
export {
  applyLookupScriptRootsToUI,
  applyProjectInfoToUI,
  applyScriptRootsToUI,
  configPathOverride,
  currentProjectDir,
  handleCompileCheckChanged,
  handleCompilerPathChanged,
  handleConfigPathOverrideChanged,
  handleLookupScriptRootsChanged,
  handleScriptRootsChanged,
  loadCompileCheck,
  loadCompilerPath,
  loadLookupScriptRoots,
  loadProjectConfig,
  loadProjectInfo,
  loadScriptRoots,
  lookupScriptRootsFromUI,
  resetConfirmedProjectDirs,
  saveCompileCheck,
  saveCompilerPath,
  saveLookupScriptRoots,
  saveScriptRoots,
  scriptRootsFromUI,
  setSettingsLocked,
  useProjectDir,
  type ProjectInfo,
} from "./project";


let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
export let currentPscOutcomes: PscParseOutcome[] = [];
// Set whenever a setting affecting lint output (formatting/rule config,
// compiler path, compile-check toggle, additional/lookup script roots, or the
// configuration file override) changes after currentPscOutcomes was last
// populated, so a currently showing lint results list no longer reflects
// the active settings. Checked by the Lint results tab button so switching
// to it re-lints the same files instead of silently showing stale findings.
let lintResultsStale = false;

// Marks the currently shown lint results as no longer reflecting the active
// settings (see lintResultsStale above). Called by config.ts/project.ts
// whenever a setting affecting lint output changes, since lintResultsStale
// itself stays private to the drop/lint orchestration in this file.
export function markLintResultsStale() {
  lintResultsStale = true;
}

// Bumped by handleDroppedPaths every time a new drop starts parsing/linting;
// a still-running drop's parsePscFiles callback checks its own snapshot of
// this against the current value before touching currentPscOutcomes, so a
// straggling outcome from a drop superseded by a newer one can't get mixed
// into the newer drop's results.
let currentParseGeneration = 0;
let themeSelectEl: HTMLSelectElement | null;

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

const THEME_KEY = "papyrus-lint:theme";

export type Theme = "system" | "light" | "dark";
const THEMES: Theme[] = ["system", "light", "dark"];

// Every built-in lint rule's tag metadata, keyed by rule id, fetched once
// from the backend (see loadRuleTags) and used both to render each
// finding's tag badges and to drive the tag filters below.
export let ruleTagsByRule: Map<string, RuleTagsInfo> = new Map();

// Lints `source` directly, in-process (the same `lint_papyrus_script`
// Tauri command `app/src-tauri/src/files.rs` wraps around
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

// Fetches every configuration preset's identity/description — built-in
// plus any user preset (see ConfigPreset) — for the config-picker dialog's
// inline preset list — indexes rule tags and re-renders the current lint
// results (see applyRuleTags below).

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
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
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
    setAchlistScriptRoots([]);
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
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
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
  themeSelectEl = document.querySelector("#theme-select");

  bindPresets();
  bindCodeViewer();
  bindLiveEdit();
  bindResultsList();
  bindProjectSettings();
  bindConfigSettings();
  bindLintProgress();

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
