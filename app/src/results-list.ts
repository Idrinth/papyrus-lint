// Lint results list and mass-fix. Implementation lives in the sibling
// results-list-*.ts modules; this file is the production façade so callers
// don't have to know which slice they need.

import { currentPscOutcomes } from "./drop";
import { bindResultsFilters } from "./results-filter";
import { handleExportAiClick, handleExportIssuesClick } from "./results-list-export";
import { renderPscResults } from "./results-list-render";
import * as resultsListState from "./results-list-state";

export { renderPscResults } from "./results-list-render";

export function bindResultsList() {
  resultsListState.pscResultEl = document.querySelector("#psc-result");
  resultsListState.pscResultListEl = document.querySelector("#psc-result-list");
  resultsListState.pscResultMassFixEl = document.querySelector("#psc-result-mass-fix");
  resultsListState.pscResultMassFixListEl = document.querySelector("#psc-result-mass-fix-list");
  resultsListState.exportFormatEl = document.querySelector("#export-format");
  resultsListState.exportIssuesButtonEl = document.querySelector("#export-issues-button");
  resultsListState.exportAiButtonEl = document.querySelector("#export-ai-button");
  resultsListState.exportAiHashSourceEl = document.querySelector("#export-ai-hash-source");

  bindResultsFilters(() => renderPscResults(currentPscOutcomes));

  resultsListState.exportIssuesButtonEl?.addEventListener("click", () => void handleExportIssuesClick());
  resultsListState.exportAiButtonEl?.addEventListener("click", () => void handleExportAiClick());
}
