import { isCodeViewerEditDirty } from "./live-edit-persist";
import { handleCodeViewerFixClick, handleCodeViewerLineActionClick } from "./code-viewer-actions";
import { requestCloseCodeViewer, resetCodeViewerFullscreen, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
import { handleCodeViewerPreviewFixClick } from "./code-viewer-diff";
import { setCodeViewerMode } from "./code-viewer-mode";
import { bindCodeViewerDom, codeViewerCloseEl, codeViewerEl, codeViewerFixButtonEl, codeViewerFullscreenEl, codeViewerMode, codeViewerPreviewFixButtonEl, codeViewerState, codeViewerViewEl, updateCodeViewerFixButtonsVisibility } from "./code-viewer-state";
import { renderCodeViewerView } from "./code-viewer-view";
import { updateCodeViewerEditHighlight } from "./live-edit-highlight";
export { compileAndShowOutput } from "./code-viewer-compile";
export { openCodeViewer } from "./code-viewer-dialog";

// Re-applies the Lint results tab's active filters to an already-open
// viewer or editor, so changing a filter while a file is open hides or
// shows the same findings the list just did, without re-linting.
export function refreshCodeViewerForActiveFilters() {
  if (!codeViewerState || !codeViewerEl?.open) {
    return;
  }
  if (codeViewerMode === "edit") {
    updateCodeViewerEditHighlight();
  } else if (codeViewerViewEl) {
    renderCodeViewerView(codeViewerState.source, codeViewerState.findings);
  }
  updateCodeViewerFixButtonsVisibility();
}

export function bindCodeViewer() {
  bindCodeViewerDom();

  codeViewerCloseEl?.addEventListener("click", () => requestCloseCodeViewer());
  codeViewerFullscreenEl?.addEventListener("click", toggleCodeViewerFullscreen);
  codeViewerViewEl?.addEventListener("click", (event) => void handleCodeViewerLineActionClick(event));
  codeViewerEl?.addEventListener("click", (event) => {
    if (event.target === codeViewerEl) {
      requestCloseCodeViewer();
    }
  });
  codeViewerEl?.addEventListener("cancel", (event) => {
    if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
      event.preventDefault();
    }
  });
  codeViewerEl?.addEventListener("close", () => setCodeViewerMode("view"));
  codeViewerEl?.addEventListener("close", () => resetCodeViewerFullscreen());
  codeViewerFixButtonEl?.addEventListener("click", () => void handleCodeViewerFixClick());
  codeViewerPreviewFixButtonEl?.addEventListener("click", () => void handleCodeViewerPreviewFixClick());
}
