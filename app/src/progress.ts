// The Lint results progress bar shown while handleDroppedPaths/
// relintCurrentFiles (main.ts) parse and lint a batch of files.

let lintProgressEl: HTMLElement | null;
let lintProgressLabelEl: HTMLElement | null;
let lintProgressBarEl: HTMLProgressElement | null;
let lintProgressHideTimer: ReturnType<typeof setTimeout> | null = null;

// How long the finished progress bar stays visible before
// scheduleHideLintProgress() hides it, so a run that finishes quickly
// doesn't just flash on and off.
const LINT_PROGRESS_HIDE_DELAY_MS = 2000;

// Shows the progress bar reset to 0/`total`, for a drop about to start
// parsing/linting `total` files. `phase` names the step this bar is
// currently tracking -- "Parsing" for preloadProjectScripts's bulk pass
// (see `handleDroppedPaths`/`relintCurrentFiles` in drop.ts), then "Linting"
// once the per-file parsePscFiles loop actually starts -- mirroring the
// CLI's own two progress bars for the same two-phase pipeline.
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
    return;
  }
  lintProgressBarEl.max = total;
  lintProgressBarEl.value = 0;
  lintProgressLabelEl.textContent = `${phase} 0 / ${total} files`;
  lintProgressEl.hidden = false;
}

export function updateLintProgress(processed: number, total: number, phase: string = "Linting") {
  if (!lintProgressEl || !lintProgressLabelEl || !lintProgressBarEl) {
    return;
  }
  lintProgressBarEl.value = processed;
  lintProgressLabelEl.textContent = `${phase} ${processed} / ${total} files`;
}

export function hideLintProgress() {
  if (lintProgressHideTimer !== null) {
    clearTimeout(lintProgressHideTimer);
    lintProgressHideTimer = null;
  }
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
