import { type PscParseOutcome } from "./backend-types";
import { filterOutcomes } from "./results-filter";
import { updateExportIssuesButtonState } from "./results-list-export";
import { buildPscResultItem } from "./results-list-item";
import { renderMassFixList } from "./results-list-mass-fix";
import { pscResultEl, pscResultListEl } from "./results-list-state";
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
