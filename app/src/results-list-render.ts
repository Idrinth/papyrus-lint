import { type PscParseOutcome } from "./backend-types";
import { filterOutcomes } from "./results-filter";
import { addExportableFindings, updateExportIssuesButtonState } from "./results-list-export";
import { buildPscResultItem } from "./results-list-item";
import { addStreamedMassFix, renderMassFixList } from "./results-list-mass-fix";
import { pscResultEl, pscResultListEl } from "./results-list-state";

// How many finding rows a render may mount before later files stay
// collapsed. Small batches (and the tests) still show every finding.
// Past this, each file is a summary row until "Show N findings" — mounting
// every diagnostic up front, and doing it again from scratch per finished
// file, is what made a base-game lint spend its time in the webview.
export const FINDING_DOM_CAP = 200;

let findingBudget = FINDING_DOM_CAP;
// Paths the user opened by hand. A later full re-render (filter, fix)
// mounts those files even when the rest of the list stays collapsed.
const expandedPaths = new Set<string>();

function resetFindingBudget() {
  findingBudget = FINDING_DOM_CAP;
}

// Mounts this file's findings when the user already opened it, or when
// they still fit in the remaining cap. Returns whether the row should
// expand, and spends the cap either way once it expands.
function claimFindings(path: string, count: number): boolean {
  if (count === 0) {
    return false;
  }
  if (expandedPaths.has(path) || count <= findingBudget) {
    findingBudget -= count;
    return true;
  }
  return false;
}

function expandOptions(path: string, count: number) {
  return {
    expandFindings: false,
    onExpandFindings: () => {
      expandedPaths.add(path);
      findingBudget -= count;
    },
  };
}

export function renderPscResults(outcomes: PscParseOutcome[]) {
  renderMassFixList(outcomes);

  if (!pscResultEl || !pscResultListEl) {
    return;
  }

  if (outcomes.length === 0) {
    expandedPaths.clear();
    resetFindingBudget();
    pscResultListEl.replaceChildren();
    pscResultEl.setAttribute("hidden", "");
    updateExportIssuesButtonState(outcomes);
    return;
  }

  resetFindingBudget();
  const items = filterOutcomes(outcomes)
    .map(({ outcome, findings }) => {
      const expand = claimFindings(outcome.path, findings.length);
      return buildPscResultItem(
        outcome,
        findings,
        expand ? undefined : expandOptions(outcome.path, findings.length),
      );
    })
    .filter((item): item is HTMLLIElement => item !== null);
  pscResultListEl.replaceChildren(...items);
  pscResultEl.removeAttribute("hidden");
  updateExportIssuesButtonState(outcomes);
}

// One file of a streamed batch. Appends a single row (or nothing, when the
// file is clean and filtered out) and leaves every row already on the list
// alone. `outcomes` is the live batch, including `outcome`, so the mass-fix
// buttons created along the way still see files that finish later.
export function appendStreamedPscResult(outcomes: PscParseOutcome[], outcome: PscParseOutcome) {
  addStreamedMassFix(outcome, outcomes);
  if (!pscResultEl || !pscResultListEl) {
    return;
  }

  pscResultEl.removeAttribute("hidden");
  const [row] = filterOutcomes([outcome]);
  addExportableFindings(row?.findings.length ?? 0);
  if (!row) {
    return;
  }
  const expand = claimFindings(outcome.path, row.findings.length);
  const item = buildPscResultItem(
    row.outcome,
    row.findings,
    expand ? undefined : expandOptions(outcome.path, row.findings.length),
  );
  if (item) {
    pscResultListEl.append(item);
  }
}