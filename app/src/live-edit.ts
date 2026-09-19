import { codeViewerCancelButtonEl, codeViewerEditButtonEl, codeViewerEditGutterEl, codeViewerEditHighlightEl, codeViewerEditTextareaEl, codeViewerSaveButtonEl, codeViewerSaveCompileButtonEl } from "./code-viewer-state";
import { handleAutocompleteKeydown, handleEditorTabKeydown, hideAutocomplete, updateAutocomplete } from "./live-edit-autocomplete";
import { updateCodeViewerEditHighlight } from "./live-edit-highlight";
import { scheduleLiveEditLint } from "./live-edit-lint";
import { cancelCodeViewerEditMode, enterCodeViewerEditMode, saveAndCompileCodeViewerEdits, saveCodeViewerEdits } from "./live-edit-persist";
import { clearPointerPosition, recordPointerPosition, refreshPointerTooltip } from "./live-edit-pointer";
// Wires up edit mode's DOM event listeners; the actual feature behavior
// (live linting, highlighting, autocompletion, hover documentation,
// persistence) lives in the sibling live-edit-*.ts modules.
export function bindLiveEdit() {
  codeViewerEditButtonEl?.addEventListener("click", () => enterCodeViewerEditMode());
  codeViewerCancelButtonEl?.addEventListener("click", () => cancelCodeViewerEditMode());
  codeViewerSaveButtonEl?.addEventListener("click", () => void saveCodeViewerEdits());
  codeViewerSaveCompileButtonEl?.addEventListener("click", () => void saveAndCompileCodeViewerEdits());
  codeViewerEditTextareaEl?.addEventListener("input", () => updateCodeViewerEditHighlight());
  codeViewerEditTextareaEl?.addEventListener("input", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("input", () => scheduleLiveEditLint());
  codeViewerEditTextareaEl?.addEventListener("click", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleAutocompleteKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleEditorTabKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("blur", () => hideAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("mousemove", (event) => {
    recordPointerPosition(event.clientX, event.clientY);
  });
  codeViewerEditTextareaEl?.addEventListener("mouseleave", () => {
    clearPointerPosition();
  });
  codeViewerEditTextareaEl?.addEventListener("scroll", () => {
    if (codeViewerEditHighlightEl && codeViewerEditTextareaEl) {
      codeViewerEditHighlightEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
      codeViewerEditHighlightEl.scrollLeft = codeViewerEditTextareaEl.scrollLeft;
    }
    if (codeViewerEditGutterEl && codeViewerEditTextareaEl) {
      codeViewerEditGutterEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
    }
    refreshPointerTooltip();
  });
}
