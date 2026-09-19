export let pscResultEl: HTMLElement | null = null;
export let pscResultListEl: HTMLElement | null = null;
export let pscResultMassFixEl: HTMLElement | null = null;
export let pscResultMassFixListEl: HTMLElement | null = null;
export let exportFormatEl: HTMLSelectElement | null = null;
export let exportIssuesButtonEl: HTMLButtonElement | null = null;
export let exportAiButtonEl: HTMLButtonElement | null = null;
export let exportAiHashSourceEl: HTMLInputElement | null = null;

export function bindResultsListDom() {
  pscResultEl = document.querySelector("#psc-result");
  pscResultListEl = document.querySelector("#psc-result-list");
  pscResultMassFixEl = document.querySelector("#psc-result-mass-fix");
  pscResultMassFixListEl = document.querySelector("#psc-result-mass-fix-list");
  exportFormatEl = document.querySelector("#export-format");
  exportIssuesButtonEl = document.querySelector("#export-issues-button");
  exportAiButtonEl = document.querySelector("#export-ai-button");
  exportAiHashSourceEl = document.querySelector("#export-ai-hash-source");
}
