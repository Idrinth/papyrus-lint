import { type PscParseOutcome } from "./backend-types";
import { isFixableFinding } from "./finding-fixability";
import { RULE_SETTINGS } from "./config-types";
import { handleMassFixClick } from "./results-list-actions";
import { pscResultMassFixEl, pscResultMassFixListEl } from "./results-list-state";

export function massFixRuleDisplayName(rule: string): string {
  return RULE_SETTINGS.find((setting) => setting.id === rule)?.name ?? rule;
}

// Counts, per rule id, how many currently fixable findings (see
// isFixableFinding) exist across every outcome, for the "mass fix" panel to
// list. A rule with no fixable finding anywhere is omitted entirely.
export function massFixRuleCounts(outcomes: PscParseOutcome[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const outcome of outcomes) {
    addFixableCounts(counts, outcome);
  }
  return counts;
}

function addFixableCounts(counts: Map<string, number>, outcome: PscParseOutcome) {
  for (const finding of outcome.findings) {
    if (finding.rule && isFixableFinding(finding)) {
      counts.set(finding.rule, (counts.get(finding.rule) ?? 0) + 1);
    }
  }
}

interface MassFixRow {
  item: HTMLLIElement;
  label: HTMLSpanElement;
  button: HTMLButtonElement;
}

// Counts and row nodes survive across streamed files. Rebuilding this panel
// from every finding on every finished file was quadratic; a streamed file
// only bumps the rules it actually contains, and the button node stays put.
let massFixCounts = new Map<string, number>();
let massFixRows = new Map<string, MassFixRow>();

function setRowText(rule: string, count: number, row: MassFixRow) {
  row.label.textContent = `${massFixRuleDisplayName(rule)} (${count})`;
  row.button.textContent = count === 1 ? "Fix this issue everywhere" : `Fix all ${count} in project`;
}

function createRow(rule: string, outcomes: PscParseOutcome[]): MassFixRow {
  const item = document.createElement("li");
  item.classList.add("psc-result__mass-fix-item");

  const label = document.createElement("span");
  item.append(label);

  const button = document.createElement("button");
  button.type = "button";
  button.classList.add("psc-result__mass-fix-button");
  button.addEventListener("click", () => void handleMassFixClick(rule, outcomes, button));
  item.append(button);

  const row = { item, label, button };
  setRowText(rule, massFixCounts.get(rule) ?? 0, row);
  return row;
}

function sortedRules(): string[] {
  return [...massFixCounts.keys()].sort((a, b) => massFixRuleDisplayName(a).localeCompare(massFixRuleDisplayName(b)));
}

function showMassFixPanel() {
  pscResultMassFixEl?.removeAttribute("hidden");
}

// Builds/refreshes the "mass fix" panel listing every rule with at least
// one fixable finding somewhere in `outcomes`, each with a button that
// clears every occurrence of that one rule across every file at once (see
// handleMassFixClick). Hides the panel entirely once no rule qualifies
// (e.g. right after the last one has been mass-fixed). Replaces the row
// nodes, so call this when the set of files changed underneath the counts
// (a fix, a filter-driven re-render, a cleared batch) rather than for one
// newly streamed file.
export function renderMassFixList(outcomes: PscParseOutcome[]) {
  massFixCounts = massFixRuleCounts(outcomes);
  massFixRows = new Map();
  if (!pscResultMassFixEl || !pscResultMassFixListEl) {
    return;
  }

  if (massFixCounts.size === 0) {
    pscResultMassFixListEl.replaceChildren();
    pscResultMassFixEl.setAttribute("hidden", "");
    return;
  }

  const rules = sortedRules();
  pscResultMassFixListEl.replaceChildren(
    ...rules.map((rule) => {
      const row = createRow(rule, outcomes);
      massFixRows.set(rule, row);
      return row.item;
    }),
  );
  showMassFixPanel();
}

// One file finished streaming. Adjusts counts for that file only and
// updates (or appends) its rule rows. `outcomes` is the live batch array
// the mass-fix click handler closes over, so files that arrive later are
// still visible to a click on a button created earlier.
export function addStreamedMassFix(outcome: PscParseOutcome, outcomes: PscParseOutcome[]) {
  const touched = new Set<string>();
  for (const finding of outcome.findings) {
    if (finding.rule && isFixableFinding(finding)) {
      touched.add(finding.rule);
    }
  }
  if (touched.size === 0) {
    return;
  }
  addFixableCounts(massFixCounts, outcome);
  if (!pscResultMassFixEl || !pscResultMassFixListEl) {
    return;
  }
  for (const rule of touched) {
    const existing = massFixRows.get(rule);
    if (existing) {
      setRowText(rule, massFixCounts.get(rule) ?? 0, existing);
      continue;
    }
    const row = createRow(rule, outcomes);
    massFixRows.set(rule, row);
  }
  for (const rule of sortedRules()) {
    const row = massFixRows.get(rule);
    if (row) {
      pscResultMassFixListEl.append(row.item);
    }
  }
  showMassFixPanel();
}