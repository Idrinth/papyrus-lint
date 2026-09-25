import { type Diagnostic, type PscParseOutcome, type RuleTagsInfo, type TagImportance, type TagKind, TAG_IMPORTANCES, TAG_KINDS } from "./backend-types";
import { type Severity, SEVERITIES, severityOf } from "./main-severity";
import { currentProjectDir } from "./project-state";
import { relativePath } from "./path";
import { type ActiveFilters, type FilteredIssuesFile } from "./results-export-types";
export const ruleTagsByRule: Map<string, RuleTagsInfo> = new Map();

export let filenameFilterEl: HTMLInputElement | null;
export let autoFixableFilterEl: HTMLInputElement | null;
export let ruleFilterSelectEls: Partial<Record<TagKind, HTMLSelectElement>> = {};
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

// One file after every active filter has been applied. `outcome` is the
// original parse/lint result (handlers mutate it); `findings` is the
// subset the results list is allowed to render. Shared by filterOutcomes
// (which produces these) and buildPscResultItem (which consumes them), so
// the list never re-decides what "currently filtered" means.
export interface FilteredPscResult {
  outcome: PscParseOutcome;
  findings: Diagnostic[];
}

function titleCaseRuleId(rule: string): string {
  return rule.charAt(0).toUpperCase() + rule.slice(1).replace(/-/g, " ");
}

// Rebuilds each tag kind's "Filter by rule" multiselect from the backend's
// full set of known rules - a rule tagged with more than one kind (e.g.
// "argument-types", tagged both "performance" and "correctness") appears in
// each of its kinds' multiselects, kept in sync with each other via the
// single activeRules set both read from/write to (see
// syncRuleFilterSelections below, and its own "change" listener set up in
// bindResultsFilters), so deselecting it in one group's list is
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

export function activeFiltersForExport(): ActiveFilters {
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
// filter, shared by filterOutcomes (the results list),
// collectFilteredIssues (the "Export issues" button below), and the
// code viewer/editor (highlights, tooltips, line actions) so none of
// them can disagree about what "currently filtered" means.
export function findingsPassingActiveFilters(findings: Diagnostic[]): Diagnostic[] {
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

// Applies every active filter (filename search, severity, tag, rule,
// auto-fixable) to `outcomes` and returns only what the results list
// should render: files that match the filename filter, each carrying just
// the findings that pass the rest. A successfully parsed file with
// nothing left to show is omitted; a file that failed to parse is kept
// even with no remaining findings, since that failure is itself the
// result worth reporting. The original `outcome` object is passed
// through (not copied) so file-level actions (View code, Apply fixes)
// still mutate the same object `currentPscOutcomes` holds.
export function filterOutcomes(outcomes: PscParseOutcome[]): FilteredPscResult[] {
  const filtered: FilteredPscResult[] = [];
  for (const outcome of outcomes) {
    if (!matchesFilenameFilter(relativePath(outcome.path, currentProjectDir), currentFilenameFilter)) {
      continue;
    }
    const findings = findingsPassingActiveFilters(outcome.findings);
    if (outcome.ok && findings.length === 0) {
      continue;
    }
    filtered.push({ outcome, findings });
  }
  return filtered;
}

// Gathers every finding currently visible in the Lint results list - i.e.
// the same set filterOutcomes hands the list, grouped by file - for the
// "Export issues" button. A file that doesn't match the filename filter,
// or has no findings passing the severity/tag/rule filters (including one
// that failed to parse, which has none at all), is omitted entirely, since
// there's nothing to export for it.
export function collectFilteredIssues(outcomes: PscParseOutcome[]): FilteredIssuesFile[] {
  const files: FilteredIssuesFile[] = [];
  for (const { outcome, findings } of filterOutcomes(outcomes)) {
    if (findings.length === 0) {
      continue;
    }
    files.push({ path: relativePath(outcome.path, currentProjectDir), findings });
  }
  return files;
}

// Wires the Lint results tab's filter controls. `onChange` is called after
// every successful filter mutation so the caller can re-render from
// currentPscOutcomes; this module never imports the results list, which
// imports this one.
export function bindResultsFilters(onChange: () => void) {
  filenameFilterEl = document.querySelector("#filename-filter");
  autoFixableFilterEl = document.querySelector("#filter-auto-fixable-only");
  ruleFilterSelectEls = Object.fromEntries(
    TAG_KINDS.map((kind) => [kind, document.querySelector<HTMLSelectElement>(`#filter-rule-${kind}`)]),
  ) as Partial<Record<TagKind, HTMLSelectElement>>;

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
      onChange();
    });
  }

  filenameFilterEl?.addEventListener("input", () => {
    currentFilenameFilter = filenameFilterEl?.value ?? "";
    onChange();
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
    onChange();
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
      onChange();
    });
  }

  autoFixableFilterEl?.addEventListener("change", () => {
    onlyAutoFixable = autoFixableFilterEl?.checked ?? false;
    onChange();
  });
}
