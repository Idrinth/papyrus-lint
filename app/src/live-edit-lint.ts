import { lintPapyrusScript } from "./backend";
import { codeViewerEditTextareaEl, codeViewerMode } from "./code-viewer";
import { setLiveEditFindings, updateCodeViewerEditHighlight } from "./live-edit-highlight";
import { resetMemberCache } from "./live-edit-members";
import { invalidateHover } from "./live-edit-pointer";

let codeViewerLiveLintTimer: ReturnType<typeof window.setTimeout> | null = null;
let codeViewerLiveLintRequestId = 0;
const LIVE_EDIT_LINT_DEBOUNCE_MS = 400;

// Cancels any pending or in-flight live lint, e.g. when leaving edit mode
// (see `setCodeViewerMode`) - linting a textarea that's no longer being
// edited would be pointless, and a response arriving after that point must
// not overwrite whatever's shown next. Exported so tests can reset this
// module-level timer between runs the same way `resetConfirmedProjectDirs`
// resets other such state.
export function cancelLiveEditLint(): void {
  if (codeViewerLiveLintTimer !== null) {
    window.clearTimeout(codeViewerLiveLintTimer);
    codeViewerLiveLintTimer = null;
  }
  codeViewerLiveLintRequestId += 1;
  invalidateHover();
  resetMemberCache();
}

// Debounces a live lint of the edit-mode textarea's current contents via
// `lintPapyrusScript`, so a burst of keystrokes triggers one CLI-equivalent
// lint pass `LIVE_EDIT_LINT_DEBOUNCE_MS` after the last of them rather than
// one per keystroke. Called from the textarea's own "input" listener.
export function scheduleLiveEditLint(): void {
  if (codeViewerMode !== "edit") {
    return;
  }
  if (codeViewerLiveLintTimer !== null) {
    window.clearTimeout(codeViewerLiveLintTimer);
  }
  codeViewerLiveLintTimer = window.setTimeout(() => {
    codeViewerLiveLintTimer = null;
    void runLiveEditLint();
  }, LIVE_EDIT_LINT_DEBOUNCE_MS);
}

async function runLiveEditLint(): Promise<void> {
  if (!codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }
  const requestId = ++codeViewerLiveLintRequestId;
  const findings = await lintPapyrusScript(codeViewerEditTextareaEl.value);
  // A later keystroke may have started a new live lint (or left edit mode
  // entirely) while this one was in flight; don't clobber it with a stale
  // response.
  if (requestId !== codeViewerLiveLintRequestId || !codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }
  setLiveEditFindings(findings);
  updateCodeViewerEditHighlight();
}
