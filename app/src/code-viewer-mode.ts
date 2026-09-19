import { hideAutocomplete } from "./live-edit-autocomplete";
import { cancelLiveEditLint } from "./live-edit-lint";
import { codeViewerCancelButtonEl, codeViewerEditButtonEl, codeViewerEditEl, codeViewerSaveButtonEl, codeViewerSaveCompileButtonEl, codeViewerViewEl, setCodeViewerModeValue, updateCodeViewerFixButtonsVisibility } from "./code-viewer-state";
// Shows the view-mode table or the edit-mode textarea/highlight overlay,
// toggling the header's Edit/Save/Cancel buttons to match.
export function setCodeViewerMode(mode: "view" | "edit") {
  setCodeViewerModeValue(mode);
  if (mode !== "edit") {
    hideAutocomplete();
    cancelLiveEditLint();
  }
  if (codeViewerViewEl) codeViewerViewEl.hidden = mode !== "view";
  if (codeViewerEditEl) codeViewerEditEl.hidden = mode !== "edit";
  if (codeViewerEditButtonEl) codeViewerEditButtonEl.hidden = mode !== "view";
  if (codeViewerSaveButtonEl) codeViewerSaveButtonEl.hidden = mode !== "edit";
  if (codeViewerSaveCompileButtonEl) codeViewerSaveCompileButtonEl.hidden = mode !== "edit";
  if (codeViewerCancelButtonEl) codeViewerCancelButtonEl.hidden = mode !== "edit";
  updateCodeViewerFixButtonsVisibility();
}
