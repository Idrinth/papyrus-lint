// The Lint results progress bar shown while handleDroppedPaths/
// relintCurrentFiles (drop.ts) parse, resolve, and lint a batch of files.

let lintProgressEl: HTMLElement | null;
let lintProgressLabelEl: HTMLElement | null;
let lintProgressBarEl: HTMLProgressElement | null;
let lintProgressHideTimer: ReturnType<typeof setTimeout> | null = null;

// How long the finished progress bar stays visible before
// scheduleHideLintProgress() hides it, so a run that finishes quickly
// doesn't just flash on and off.
const LINT_PROGRESS_HIDE_DELAY_MS = 2000;

const LINT_PROGRESS_BUSY_CLASS = "lint-progress--busy";

function clearLintProgressBusy() {
  lintProgressEl?.classList.remove(LINT_PROGRESS_BUSY_CLASS);
}

// Shows the progress bar reset to 0/`total`, for a drop about to start
// parsing/linting `total` files. `phase` names the step this bar is
// currently tracking -- "Parsing" while each project file is read, then
// "Resolving" while preloadProjectScripts closes over referenced scripts
// (see runParseThenLint in drop.ts), then "Linting" once the per-file
// parsePscFiles loop actually starts.
export function showLintProgress(total: number, phase: string = "Linting") {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  if (total === 0) {
    lintProgressEl.hidden = true;
    clearLintProgressBusy();
    return;
  }
  clearLintProgressBusy();
  lintProgressBarEl.max = total;
  lintProgressBarEl.value = 0;
  lintProgressLabelEl.textContent = `${phase} 0 / ${total} files`;
  lintProgressEl.hidden = false;
}

// An in-between step with no known fraction yet (the reference walk inside
// preload_project_scripts, then indexing the function table once that walk
// finishes). A determinate bar left sitting at 100% reads as a hang; an
// indeterminate one keeps moving until the next counted update arrives.
export function showLintActivity(label: string) {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  lintProgressBarEl.removeAttribute("value");
  lintProgressLabelEl.textContent = label;
  lintProgressEl.classList.add(LINT_PROGRESS_BUSY_CLASS);
  lintProgressEl.hidden = false;
}

export function updateLintProgress(processed: number, total: number, phase: string = "Linting") {
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  clearLintProgressBusy();
  lintProgressBarEl.max = total;
  lintProgressBarEl.value = processed;
  lintProgressLabelEl.textContent = `${phase} ${processed} / ${total} files`;
}

export function hideLintProgress() {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
  clearLintProgressBusy();
  if (lintProgressEl) {
    lintProgressEl.hidden = true;
  }
}

// Hides the progress bar after a short grace period instead of instantly,
// so the finished state stays visible long enough to register before it
// disappears. A drop that starts again in the meantime (showLintProgress)
// cancels this timer, so the bar isn't hidden out from under it.
export function scheduleHideLintProgress(delayMs = LINT_PROGRESS_HIDE_DELAY_MS) {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
  }
  lintProgressHideTimer = window.setTimeout(() => {
    lintProgressHideTimer = null;
    hideLintProgress();
  }, delayMs);
}

export function bindLintProgress() {
  lintProgressEl = document.querySelector("#lint-progress");
  lintProgressLabelEl = document.querySelector("#lint-progress-label");
  lintProgressBarEl = document.querySelector("#lint-progress-bar");
}
