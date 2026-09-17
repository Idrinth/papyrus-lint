import {
  type Diagnostic,
  type PscParseOutcome,
  currentPscOutcomes,
  hasNoAutomaticFix,
  hasFixableFindings,
  isFixableFinding,
  levelOf,
  loadAppVersion,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
} from "./main";
import { type LintConfig, currentLintConfig } from "./config";
import { currentProjectDir } from "./project";
import { relativePath } from "./path";
import { compileAndShowOutput, openCodeViewer } from "./code-viewer";
import { downloadTextFile } from "./download-text-file";
import { formatIssuesAsText } from "./results-export-text";
import { formatIssuesAsJson } from "./results-export-json";
import { formatIssuesForAi as formatIssuesForAiDocument, readIssueFileSources } from "./results-export-ai";
import { type AiSource, type FilteredIssuesFile } from "./results-export-types";
import {
  activeFiltersForExport,
  bindResultsFilters,
  collectFilteredIssues,
  filterOutcomes,
  tagsForFinding,
} from "./results-filter";

export { downloadTextFile } from "./download-text-file";
export { formatIssuesAsText } from "./results-export-text";
export { formatIssuesAsJson } from "./results-export-json";
export { aiConfiguration } from "./results-export-ai";
export type { ActiveFilters, AiSource, FilteredIssuesFile } from "./results-export-types";
export {
  collectFilteredIssues,
  filterOutcomes,
  matchesFilenameFilter,
  matchesTagFilters,
  populateRuleFilterGroups,
  tagsForFinding,
  filenameFilterEl,
  autoFixableFilterEl,
  ruleFilterSelectEls,
  severityFilterEls,
  tagKindFilterEls,
  tagImportanceFilterEls,
  type FilteredPscResult,
} from "./results-filter";

export let pscResultEl: HTMLElement | null;
export let pscResultListEl: HTMLElement | null;
export let pscResultMassFixEl: HTMLElement | null;
export let pscResultMassFixListEl: HTMLElement | null;
export let exportFormatEl: HTMLSelectElement | null;
export let exportIssuesButtonEl: HTMLButtonElement | null;
export let exportAiButtonEl: HTMLButtonElement | null;
export let exportAiHashSourceEl: HTMLInputElement | null;

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

// Builds the list item for an already-filtered file. `findings` is the
// subset filterOutcomes selected for display; `outcome` is the original
// parse/lint result, used by file-level actions (View code, Apply fixes)
// so those still see every finding in the file. Returns null if there's
// nothing to show (a successfully parsed file with no findings to list).
export function buildPscResultItem(
  outcome: PscParseOutcome,
  findings: Diagnostic[] = outcome.findings,
): HTMLLIElement | null {
  const { path, ok, detail } = outcome;

  if (ok && findings.length === 0) {
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

  if (hasFixableFindings(outcome.findings)) {
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

  if (findings.length > 0) {
    const findingsList = document.createElement("ul");
    findingsList.classList.add("psc-result__findings");
    findingsList.replaceChildren(
      ...findings.map((finding) => {
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

  const items = filterOutcomes(outcomes)
    .map(({ outcome, findings }) => buildPscResultItem(outcome, findings))
    .filter((item): item is HTMLLIElement => item !== null);
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
  exportFormatEl = document.querySelector("#export-format");
  exportIssuesButtonEl = document.querySelector("#export-issues-button");
  exportAiButtonEl = document.querySelector("#export-ai-button");
  exportAiHashSourceEl = document.querySelector("#export-ai-hash-source");

  bindResultsFilters(() => renderPscResults(currentPscOutcomes));

  exportIssuesButtonEl?.addEventListener("click", () => handleExportIssuesClick());
  exportAiButtonEl?.addEventListener("click", () => void handleExportAiClick());
}
