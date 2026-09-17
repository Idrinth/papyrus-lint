import { isCodeViewerEditDirty } from "./live-edit";
import { handleCodeViewerFixClick, handleCodeViewerLineActionClick } from "./code-viewer-actions";
import { requestCloseCodeViewer, resetCodeViewerFullscreen, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
import { handleCodeViewerPreviewFixClick } from "./code-viewer-diff";
import { setCodeViewerMode } from "./code-viewer-mode";
import {
  bindCodeViewerDom,
  codeViewerCloseEl,
  codeViewerEl,
  codeViewerFixButtonEl,
  codeViewerFullscreenEl,
  codeViewerPreviewFixButtonEl,
  codeViewerViewEl,
} from "./code-viewer-state";

export type { CodeViewerState } from "./code-viewer-state";
export {
  codeViewerAutocompleteEl,
  codeViewerCancelButtonEl,
  codeViewerCloseEl,
  codeViewerCompileOutputEl,
  codeViewerDiffOutputEl,
  codeViewerEditButtonEl,
  codeViewerEditEl,
  codeViewerEditGutterEl,
  codeViewerEditHighlightEl,
  codeViewerEditTextareaEl,
  codeViewerEl,
  codeViewerFixButtonEl,
  codeViewerFullscreenEl,
  codeViewerMode,
  codeViewerPreviewFixButtonEl,
  codeViewerSaveButtonEl,
  codeViewerSaveCompileButtonEl,
  codeViewerState,
  codeViewerTitleEl,
  codeViewerViewEl,
  setCodeViewerState,
} from "./code-viewer-state";
export { findingsGroupedByLine, lineSeverityOf, renderCodeViewerView } from "./code-viewer-view";
export { setCodeViewerMode } from "./code-viewer-mode";
export { compileAndShowOutput, hideCompileOutput } from "./code-viewer-compile";
export { handleCodeViewerPreviewFixClick, hideDiffOutput } from "./code-viewer-diff";
export {
  handleCodeViewerFixClick,
  handleCodeViewerFixLineClick,
  handleCodeViewerIgnoreLineClick,
} from "./code-viewer-actions";
export { openCodeViewer, requestCloseCodeViewer, toggleCodeViewerFullscreen } from "./code-viewer-dialog";

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
