import { invoke } from "@tauri-apps/api/core";
import {
  type Diagnostic,
  type LintConfig,
  type PscParseOutcome,
  type RuleTagsInfo,
  type Severity,
  type TagImportance,
  type TagKind,
  FIXABLE_RULE_IDS,
  SEVERITIES,
  TAG_IMPORTANCES,
  TAG_KINDS,
  currentLintConfig,
  currentProjectDir,
  currentPscOutcomes,
  hasNoAutomaticFix,
  hasFixableFindings,
  isFixableFinding,
  levelOf,
  loadAppVersion,
  previewRepairPscLine,
  relativePath,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  ruleTagsByRule,
  severityOf,
} from "./main";
import { compileAndShowOutput, openCodeViewer } from "./code-viewer";

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
// A serializable snapshot of the GUI-only result filters. The AI export
// includes this alongside the already-filtered findings so its reader can
// distinguish a genuinely clean category from one the user excluded.
export interface ActiveFilters {
  filename_pattern: string;
  severities: Severity[];
  importances: TagImportance[];
  rules: string[];
  auto_fixable_only: boolean;
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

// Returns `files` with each file's findings sorted by (line, column), so a
// file's diagnostics list in reading order (top to bottom, left to right)
// rather than papyrus_lints::lint()'s own rule-registration order - the same
// ordering the CLI's `--json`/`--ai` output already gets from its own
// `diagnostics.sort_by_key(|d| (d.line, d.column))`
// (papyrus-lint-cli/src/lib.rs). A stable sort, so two findings at the exact
// same position (e.g. a lint diagnostic and a compiler-reported one) keep
// their original relative order.
function sortedByPosition(files: FilteredIssuesFile[]): FilteredIssuesFile[] {
  return files.map((file) => ({
    ...file,
    findings: [...file.findings].sort((a, b) => a.line - b.line || a.column - b.column),
  }));
}

// Shared by formatIssuesAsJson and formatIssuesForAi: `files` as a plain
// object mirroring the CLI's own `--json` report shape
// (JsonReport/JsonFileReport/JsonDiagnostic in papyrus-lint-cli/src/lib.rs).
function buildIssuesReport(files: FilteredIssuesFile[], stripSeverityPrefix = false) {
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
        message: stripSeverityPrefix
          ? finding.message.replace(/^\[(?:error|warning|info)\]\s*/, "")
          : finding.message,
        doc_url: (finding.rule ? ruleTagsByRule.get(finding.rule)?.doc_url : undefined) ?? null,
      })),
    };
  });
  return {
    files: jsonFiles,
    files_with_diagnostics: jsonFiles.length,
    total_diagnostics: totalDiagnostics,
  };
}

// Renders `files` as JSON, mirroring the shape of the CLI's own `--json`
// report so both can be consumed by the same tooling.
export function formatIssuesAsJson(files: FilteredIssuesFile[]): string {
  return JSON.stringify(buildIssuesReport(sortedByPosition(files)), null, 2);
}

// The AI export's own `configuration` shape (see formatIssuesForAi below):
// the same resolved LintConfig a lint run used, except its `rules` object
// (58 individual enable flags, each with its own description in the
// schema) is replaced with a compact, alphabetically sorted
// `enabled_rules` list of just the hyphenated ids that are currently on -
// no information is lost, since a rule absent from the list is simply
// disabled, but every export no longer repeats a large, mostly-constant
// block of booleans. Mirrors papyrus_lints::Rules::enabled_ids and
// papyrus-lint-cli's own ai_configuration in app/crates/papyrus-lint-cli/src/lib.rs.
export function aiConfiguration(config: LintConfig): Record<string, unknown> {
  const { rules, ...rest } = config;
  const enabledRules = Object.entries(rules)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name.replace(/_/g, "-"))
    .sort((a, b) => a.localeCompare(b));
  return { ...rest, enabled_rules: enabledRules };
}

// The desktop app's own homepage, where an AI reading an "Export for AI"
// document (see formatIssuesForAi) can look up rule/configuration
// documentation beyond what rule_details itself carries.
const WEBSITE_URL = "https://papyrus-lint.idrinth.de";
const AI_EXPORT_SCHEMA_URL =
  "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json";
const TOOL_NAME = "Papyrus Lint";
// The Papyrus dialect/engine version these findings were produced for, so
// an AI reading the export doesn't have to guess whether a suggestion (e.g.
// referencing a native type only added in a later game/edition) actually
// applies. Papyrus Lint has no per-project game/edition setting of its own
// (see rules/native-types.yaml's shared Skyrim/Fallout 4 fallback), so this
// is the fixed target its native rule data is written against.
const TARGET_GAME = "Skyrim SE/AE";

// The four explicit shapes an AI export's per-file `source` field can take
// (see formatIssuesForAi below): `null` when no source was attached at all;
// `content` carrying the script's full on-disk text; `hash` carrying only
// its md5 digest, selected via the "Redact source" checkbox next to the
// "Export for AI" button (or the CLI's --hash-source flag) so a report can
// be handed to an external AI without exposing proprietary script text
// while still letting it tell files apart, or notice a file changed between
// exports; and `error` describing why the source couldn't be read (e.g. the
// file was moved or deleted since linting). Kept as a discriminated union
// rather than a plain string so a consumer never has to guess which of
// "full content" or "error message" a given string represents.
export type AiSource =
  | { type: "content"; content: string }
  | { type: "hash"; algorithm: "md5"; hash: string }
  | { type: "error"; message: string };

// Reads each of `files`' current on-disk source (or, with `hashSource`, just
// its md5 digest - see AiSource above) via the read_psc_file/hash_psc_file_md5
// commands the code viewer and this redaction option use respectively, keyed
// by each file's display path (see FilteredIssuesFile.path) for
// formatIssuesForAi below to attach alongside that file's findings - so an AI
// reasoning about the report can see the actual surrounding code a
// diagnostic refers to (or, redacted, at least confirm which version of a
// file it's looking at) without opening the project itself. `outcomes`
// supplies the absolute path either command needs, matched back to `files`'
// relative display path via the same relativePath()/currentProjectDir
// pairing collectFilteredIssues used to produce it in the first place. A
// read/hash failure (e.g. the file was moved or deleted since linting)
// records an `error` entry instead of failing the whole export, since the
// rest of the report stays useful without it.
async function readIssueFileSources(
  files: FilteredIssuesFile[],
  outcomes: PscParseOutcome[],
  hashSource: boolean,
): Promise<Map<string, AiSource>> {
  const absolutePathsByDisplayPath = new Map<string, string>();
  for (const outcome of outcomes) {
    absolutePathsByDisplayPath.set(relativePath(outcome.path, currentProjectDir), outcome.path);
  }
  const sources = new Map<string, AiSource>();
  await Promise.all(
    files.map(async (file) => {
      const absolutePath = absolutePathsByDisplayPath.get(file.path) ?? file.path;
      try {
        if (hashSource) {
          const hash = await invoke<string>("hash_psc_file_md5", { path: absolutePath });
          sources.set(file.path, { type: "hash", algorithm: "md5", hash });
        } else {
          const content = await invoke<string>("read_psc_file", { path: absolutePath });
          sources.set(file.path, { type: "content", content });
        }
      } catch (error) {
        sources.set(file.path, { type: "error", message: String(error) });
      }
    }),
  );
  return sources;
}

// The rule id papyrus_lints::Diagnostics never define themselves:
// app/src-tauri/src/compile_diagnostics.rs's own `RULE` constant, attached
// to a diagnostic parsed out of PapyrusCompiler.exe's own error output
// (see lint_with_compile_check in app/src-tauri/src/lib.rs) rather than
// raised by one of Papyrus Lint's own lint rules. formatIssuesForAi below
// uses it to flag such a diagnostic as external in the AI export, since an
// assistant reading the export otherwise has no way to tell a
// compiler-reported syntax error apart from an ordinary lint finding.
// Kept in sync by hand with that Rust constant, the same convention
// FIXABLE_RULE_IDS follows for papyrus_lints::FIXABLE_RULE_IDS.
const COMPILER_ERROR_RULE = "compiler-error";

// Renders `files` as a single JSON document meant to be handed to an AI
// assistant alongside a question about the results: a header identifying
// the tool/version/website/target game and generation time (so the AI knows what produced
// these findings and where to look up anything not covered below), the
// findings themselves (see buildIssuesReport) with each file's `source`
// field set to whatever `sources` has for it - one of the four explicit
// AiSource shapes (see readIssueFileSources), or `null` when `sources` has
// no entry for that file at all - an `external: true`/`source: "compiler"`
// pair added to every diagnostic raised by PapyrusCompiler.exe itself
// rather than one of Papyrus Lint's own rules (see COMPILER_ERROR_RULE
// above), so the assistant can tell a compiler-reported error apart from
// an ordinary lint finding - a `repair` field added to every
// diagnostic from an auto-fixable rule Papyrus Lint could compute a fix
// preview for (see previewRepairPscLine; omitted when the rule doesn't
// actually change that line, e.g. type-casing's "no automatic fix" case, or
// its fix would shift the file's line count elsewhere), and the full tag
// metadata (kind(s), importance, and the rule's detailed
// description copied from its README.md row; see papyrus_lints::tags) for
// every rule id that actually appears among `files`' findings - giving the
// AI enough context about each triggered rule, in the same detail the
// README gives a human reader, the actual code each diagnostic refers to,
// and what its fix would look like, to answer follow-up questions precisely
// without needing the project's own files or documentation on hand.
// `version` is the running app's version (see loadAppVersion), or "" if
// that lookup failed.
export async function formatIssuesForAi(
  files: FilteredIssuesFile[],
  version: string,
  sources: Map<string, AiSource> = new Map(),
  configuration: LintConfig = currentLintConfig,
): Promise<string> {
  const sortedFiles = sortedByPosition(files);
  const triggeredRules = new Set<string>();
  for (const file of sortedFiles) {
    for (const finding of file.findings) {
      if (finding.rule) {
        triggeredRules.add(finding.rule);
      }
    }
  }
  const ruleDetails = [...triggeredRules]
    .sort((a, b) => a.localeCompare(b))
    .map((rule) => ruleTagsByRule.get(rule))
    .filter((info): info is RuleTagsInfo => info !== undefined)
    .map(({ rule, description, kinds, importance, auto_fixable, doc_url }) => ({
      rule,
      description,
      kinds,
      importance,
      auto_fixable,
      doc_url,
    }));

  // `level` carries the severity separately, so avoid repeating its internal
  // message prefix in the AI-focused representation.
  const baseReport = buildIssuesReport(sortedFiles, true);
  const ruleCounts = (diagnostics: { rule: string }[]): Record<string, number> => {
    const counts: Record<string, number> = {};
    for (const diagnostic of diagnostics) {
      counts[diagnostic.rule] = (counts[diagnostic.rule] ?? 0) + 1;
    }
    return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
  };
  const severityCounts = (diagnostics: { level: Severity }[]) => ({
    errors: diagnostics.filter((diagnostic) => diagnostic.level === "error").length,
    warnings: diagnostics.filter((diagnostic) => diagnostic.level === "warning").length,
    info: diagnostics.filter((diagnostic) => diagnostic.level === "info").length,
  });
  const findings = {
    files: await Promise.all(
      baseReport.files.map(async (fileReport, fileIndex) => ({
        ...fileReport,
        severity_counts: severityCounts(fileReport.diagnostics),
        rule_counts: ruleCounts(fileReport.diagnostics),
        source: sources.get(fileReport.path) ?? null,
        diagnostics: await Promise.all(
          fileReport.diagnostics.map(async (diagnostic, diagnosticIndex) => {
            const finding = sortedFiles[fileIndex].findings[diagnosticIndex];
            const tagged =
              diagnostic.rule === COMPILER_ERROR_RULE
                ? { ...diagnostic, external: true, source: "compiler" }
                : diagnostic;
            if (!finding.rule || !FIXABLE_RULE_IDS.has(finding.rule) || hasNoAutomaticFix(finding)) {
              return tagged;
            }
            const repair = await previewRepairPscLine(sortedFiles[fileIndex].path, finding.rule, finding.line);
            return repair === null ? tagged : { ...tagged, repair };
          }),
        ),
      })),
    ),
    total_diagnostics: baseReport.total_diagnostics,
    severity_counts: severityCounts(baseReport.files.flatMap((file) => file.diagnostics)),
    rule_counts: ruleCounts(baseReport.files.flatMap((file) => file.diagnostics)),
  };

  return JSON.stringify(
    {
      $schema: AI_EXPORT_SCHEMA_URL,
      header: {
        tool: TOOL_NAME,
        version: version || "unknown",
        website: WEBSITE_URL,
        target_game: TARGET_GAME,
        generated_at: new Date().toISOString(),
      },
      configuration: aiConfiguration(configuration),
      filters: activeFiltersForExport(),
      findings,
      rule_details: ruleDetails,
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
export function downloadTextFile(filename: string, contents: string, mimeType: string) {
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
