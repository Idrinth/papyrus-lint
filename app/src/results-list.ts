import {
  type Diagnostic,
  type PscParseOutcome,
  type RuleTagsInfo,
  type Severity,
  type TagImportance,
  type TagKind,
  SEVERITIES,
  TAG_IMPORTANCES,
  TAG_KINDS,
  currentPscOutcomes,
  hasNoAutomaticFix,
  hasFixableFindings,
  isFixableFinding,
  levelOf,
  loadAppVersion,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  ruleTagsByRule,
  severityOf,
} from "./main";
import { type LintConfig, currentLintConfig } from "./config";
import { currentProjectDir } from "./project";
import { relativePath } from "./path";
import { compileAndShowOutput, openCodeViewer } from "./code-viewer";
import { downloadTextFile } from "./download-text-file";
import { formatIssuesAsText } from "./results-export-text";
import { formatIssuesAsJson } from "./results-export-json";
import { formatIssuesForAi as formatIssuesForAiDocument, readIssueFileSources } from "./results-export-ai";
import { type ActiveFilters, type AiSource, type FilteredIssuesFile } from "./results-export-types";

export { downloadTextFile } from "./download-text-file";
export { formatIssuesAsText } from "./results-export-text";
export { formatIssuesAsJson } from "./results-export-json";
export { aiConfiguration } from "./results-export-ai";
export type { ActiveFilters, AiSource, FilteredIssuesFile } from "./results-export-types";

export let pscResultEl: HTMLElement | null;
export let pscResultListEl: HTMLElement | null;
export let pscResultMassFixEl: HTMLElement | null;
export let pscResultMassFixListEl: HTMLElement | null;
export let filenameFilterEl: HTMLInputElement | null;
export let autoFixableFilterEl: HTMLInputElement | null;
export let ruleFilterSelectEls: Partial<Record<TagKind, HTMLSelectElement>> = {};
export let exportFormatEl: HTMLSelectElement | null;
export let exportIssuesButtonEl: HTMLButtonElement | null;
export let exportAiButtonEl: HTMLButtonElement | null;
export let exportAiHashSourceEl: HTMLInputElement | null;
export let severityFilterEls: Partial<Record<Severity, HTMLInputElement>> = {};
export let tagKindFilterEls: Partial<Record<TagKind, HTMLInputElement>> = {};
export let tagImportanceFilterEls: Partial<Record<TagImportance, HTMLInputElement>> = {};

// Literal init rather than `new Set(SEVERITIES)` so this module can load
// while main.ts is still evaluating (circular import).
const activeSeverities = new Set<Severity>(["error", "warning", "info"]);
const activeTagImportances = new Set<TagImportance>(["low", "medium", "high"]);
let onlyAutoFixable = false;
const activeRules = new Set<string>();
let currentFilenameFilter = "";

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
export function populateRuleFilterGroups(tags: RuleTagsInfo[]) {
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
  "assignment-operator-spacing": "Spacing around assignment operators",
  "type-casing": "Type name casing",
  "trailing-whitespace": "Trailing whitespace",
  "global-variable-increment": "GlobalVariable increment via SetValue(GetValue() + x)",
  "unused-import": "Unused import",
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
function activeFiltersForExport(): ActiveFilters {
  return {
    filename_pattern: currentFilenameFilter,
    severities: SEVERITIES.filter((severity) => activeSeverities.has(severity)),
    importances: TAG_IMPORTANCES.filter((importance) => activeTagImportances.has(importance)),
    rules: [...activeRules].sort((a, b) => a.localeCompare(b)),
    auto_fixable_only: onlyAutoFixable,
  };
}
// Looks up `finding`'s own rule's tag metadata, if any. A finding with no
// rule (or one that isn't a papyrus-lints rule id at all, e.g. a
// compiler-reported diagnostic - see app/src-tauri/src/compile_diagnostics.rs)
// has none.
export function tagsForFinding(finding: Diagnostic): RuleTagsInfo | undefined {
  return finding.rule ? ruleTagsByRule.get(finding.rule) : undefined;
}

// Whether `finding` passes the active tag/rule, importance, and
// auto-fixable filters. A finding with no tag metadata always passes. A
// finding whose rule is known always has a truthy `finding.rule`
// (tagsForFinding only returns tag metadata when it does), so once `tags`
// is present activeRules.has() below is checking the same rule id that
// produced it. Every finding also passes
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

  const docsLink = document.createElement("a");
  docsLink.classList.add("psc-result__tag-badge", "psc-result__tag-badge--docs-link");
  docsLink.href = tags.doc_url;
  docsLink.target = "_blank";
  docsLink.rel = "noopener noreferrer";
  docsLink.textContent = "docs";
  tagsEl.append(docsLink);

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

export async function formatIssuesForAi(
  files: FilteredIssuesFile[],
  version: string,
  sources: Map<string, AiSource> = new Map(),
  configuration: LintConfig = currentLintConfig,
): Promise<string> {
  return formatIssuesForAiDocument(files, version, sources, configuration, activeFiltersForExport());
}

// Enables the "Export issues"/"Export for AI" buttons only while there's at
// least one currently filtered finding to export.
export function updateExportIssuesButtonState(outcomes: PscParseOutcome[]) {
  const disabled = collectFilteredIssues(outcomes).length === 0;
  if (exportIssuesButtonEl) {
    exportIssuesButtonEl.disabled = disabled;
  }
  if (exportAiButtonEl) {
    exportAiButtonEl.disabled = disabled;
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

// Downloads the currently filtered lint findings as a single "Export for
// AI" JSON document (see formatIssuesForAi), each file's current source
// attached (see readIssueFileSources) - as an md5 hash instead of its full
// content when the "Redact source" checkbox is checked - independent of the
// "Export format" selector above since this format is always JSON.
export async function handleExportAiClick(): Promise<void> {
  const files = collectFilteredIssues(currentPscOutcomes);
  if (files.length === 0) {
    return;
  }
  const hashSource = exportAiHashSourceEl?.checked ?? false;
  const [version, sources] = await Promise.all([
    loadAppVersion(),
    readIssueFileSources(files, currentPscOutcomes, hashSource),
  ]);
  downloadTextFile(
    "papyrus-lint-ai-export.json",
    await formatIssuesForAi(files, version, sources),
    "application/json",
  );
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

export function bindResultsList() {
  pscResultEl = document.querySelector("#psc-result");
  pscResultListEl = document.querySelector("#psc-result-list");
  pscResultMassFixEl = document.querySelector("#psc-result-mass-fix");
  pscResultMassFixListEl = document.querySelector("#psc-result-mass-fix-list");
  filenameFilterEl = document.querySelector("#filename-filter");
  autoFixableFilterEl = document.querySelector("#filter-auto-fixable-only");
  ruleFilterSelectEls = Object.fromEntries(
    TAG_KINDS.map((kind) => [kind, document.querySelector<HTMLSelectElement>(`#filter-rule-${kind}`)]),
  ) as Partial<Record<TagKind, HTMLSelectElement>>;
  exportFormatEl = document.querySelector("#export-format");
  exportIssuesButtonEl = document.querySelector("#export-issues-button");
  exportAiButtonEl = document.querySelector("#export-ai-button");
  exportAiHashSourceEl = document.querySelector("#export-ai-hash-source");

  severityFilterEls = Object.fromEntries(
    SEVERITIES.map((severity) => [severity, document.querySelector<HTMLInputElement>(`#filter-${severity}`)]),
  ) as Partial<Record<Severity, HTMLInputElement>>;
  for (const severity of SEVERITIES) {
    severityFilterEls[severity]?.addEventListener("change", () => {
      const checked = severityFilterEls[severity]?.checked ?? true;
      if (!checked && activeSeverities.size === 1) {
        severityFilterEls[severity]!.checked = true;
        return;
      }
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

  function applyRuleSelectionChange(apply: () => void) {
    const previousActiveRules = new Set(activeRules);
    apply();
    if (activeRules.size === 0) {
      activeRules.clear();
      for (const rule of previousActiveRules) {
        activeRules.add(rule);
      }
    }
    syncRuleFilterSelections();
    renderPscResults(currentPscOutcomes);
  }

  tagKindFilterEls = Object.fromEntries(
    TAG_KINDS.map((kind) => [kind, document.querySelector<HTMLInputElement>(`#filter-kind-${kind}`)]),
  ) as Partial<Record<TagKind, HTMLInputElement>>;
  for (const kind of TAG_KINDS) {
    const select = ruleFilterSelectEls[kind];

    select?.addEventListener("change", () => {
      applyRuleSelectionChange(() => {
        for (const option of select.options) {
          if (option.selected) {
            activeRules.add(option.value);
          } else {
            activeRules.delete(option.value);
          }
        }
      });
    });

    tagKindFilterEls[kind]?.addEventListener("change", () => {
      const checked = tagKindFilterEls[kind]?.checked ?? true;
      applyRuleSelectionChange(() => {
        for (const option of select?.options ?? []) {
          if (checked) {
            activeRules.add(option.value);
          } else {
            activeRules.delete(option.value);
          }
        }
      });
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
      if (!checked && activeTagImportances.size === 1) {
        tagImportanceFilterEls[importance]!.checked = true;
        return;
      }
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
  exportAiButtonEl?.addEventListener("click", () => void handleExportAiClick());
}
