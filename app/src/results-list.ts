// Lint results list and mass-fix. Implementation lives in the sibling
// results-list-*.ts modules; this file is the production façade so callers
// don't have to know which slice they need.

import { currentPscOutcomes } from "./drop";
import { bindResultsFilters } from "./results-filter";
import { handleExportAiClick, handleExportIssuesClick } from "./results-list-export";
import { renderPscResults } from "./results-list-render";
import { bindResultsListDom, exportAiButtonEl, exportIssuesButtonEl } from "./results-list-state";

export { renderPscResults } from "./results-list-render";

export function bindResultsList() {
  bindResultsListDom();

  bindResultsFilters(() => renderPscResults(currentPscOutcomes));

  exportIssuesButtonEl?.addEventListener("click", () => void handleExportIssuesClick());
  exportAiButtonEl?.addEventListener("click", () => void handleExportAiClick());
}
