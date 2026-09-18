import { lintPscFile, writePscFile } from "./backend";
import {
  codeViewerCompileOutputEl,
  codeViewerDiffOutputEl,
  codeViewerEditTextareaEl,
  codeViewerMode,
  codeViewerSaveButtonEl,
  codeViewerSaveCompileButtonEl,
  codeViewerState,
  compileAndShowOutput,
  hideCompileOutput,
  hideDiffOutput,
  renderCodeViewerView,
  setCodeViewerMode,
  setCodeViewerState,
} from "./code-viewer";
import { currentPscOutcomes } from "./drop";
import { setLiveEditFindings, updateCodeViewerEditHighlight } from "./live-edit-highlight";
import { cancelLiveEditLint } from "./live-edit-lint";
import { renderPscResults } from "./results-list";

// A textarea's `value` getter always normalizes CR/CRLF line breaks to LF
// (per the HTML spec's "API value" transform), even though its `value`
// setter stores whatever was assigned verbatim. A CRLF-saved .psc file's
// `source` therefore no longer matches the textarea's own value right
// after `enterCodeViewerEditMode` sets it, with no edit having happened;
// normalizing both sides before comparing keeps that from reading as dirty.
function normalizeLineEndings(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

export function isCodeViewerEditDirty(): boolean {
  return (
    codeViewerMode === "edit" &&
    codeViewerState !== null &&
    codeViewerEditTextareaEl !== null &&
    codeViewerEditTextareaEl.value !== normalizeLineEndings(codeViewerState.source)
  );
}

export function enterCodeViewerEditMode() {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  cancelLiveEditLint();
  setLiveEditFindings(codeViewerState.findings);
  codeViewerEditTextareaEl.value = codeViewerState.source;
  updateCodeViewerEditHighlight();
  hideCompileOutput(codeViewerCompileOutputEl);
  hideDiffOutput(codeViewerDiffOutputEl);
  setCodeViewerMode("edit");
  codeViewerEditTextareaEl.focus();
}

export function cancelCodeViewerEditMode() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  setCodeViewerMode("view");
}

// Writes the editor's current contents to disk, re-lints the file, and
// refreshes both the code viewer's view mode and the Lint results list to
// match, switching the viewer back to view mode. Shared by the plain Save
// button and the Save & Compile button below; throws (without touching any
// UI) if the write itself fails, leaving the caller to report that.
async function persistCodeViewerEdits(): Promise<void> {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  const { path } = codeViewerState;
  const contents = codeViewerEditTextareaEl.value;

  await writePscFile(path, contents);
  const findings = await lintPscFile(path);
  setCodeViewerState({ path, source: contents, findings });

  const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
  if (outcome) {
    outcome.findings = findings;
    renderPscResults(currentPscOutcomes);
  }

  renderCodeViewerView(codeViewerState.source, codeViewerState.findings);
  setCodeViewerMode("view");
}

export async function saveCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveButtonEl) {
    return;
  }
  codeViewerSaveButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    const originalLabel = codeViewerSaveButtonEl.textContent;
    codeViewerSaveButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveButtonEl) {
        codeViewerSaveButtonEl.textContent = originalLabel;
      }
    }, 2000);
  } finally {
    codeViewerSaveButtonEl.disabled = false;
  }
}

// Saves the editor's contents (as saveCodeViewerEdits does) and, if that
// succeeds, immediately compiles the saved file, showing the compiler's
// output beneath the code viewer's view/editor area.
export async function saveAndCompileCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveCompileButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  const originalLabel = codeViewerSaveCompileButtonEl.textContent;

  codeViewerSaveCompileButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    codeViewerSaveCompileButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveCompileButtonEl) {
        codeViewerSaveCompileButtonEl.textContent = originalLabel;
      }
    }, 2000);
    codeViewerSaveCompileButtonEl.disabled = false;
    return;
  }

  if (codeViewerCompileOutputEl) {
    codeViewerSaveCompileButtonEl.textContent = "Compiling…";
    await compileAndShowOutput(path, codeViewerCompileOutputEl);
  }
  codeViewerSaveCompileButtonEl.disabled = false;
  codeViewerSaveCompileButtonEl.textContent = originalLabel;
}
