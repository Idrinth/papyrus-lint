import { isCodeViewerEditDirty } from "./live-edit-persist";
import { handleCodeViewerFixClick, handleCodeViewerLineActionClick } from "./code-viewer-actions";
import { requestCloseCodeViewer, resetCodeViewerFullscreen, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
import { handleCodeViewerPreviewFixClick } from "./code-viewer-diff";
import { setCodeViewerMode } from "./code-viewer-mode";
import { bindCodeViewerDom, codeViewerCloseEl, codeViewerEl, codeViewerFixButtonEl, codeViewerFullscreenEl, codeViewerPreviewFixButtonEl, codeViewerViewEl } from "./code-viewer-state";
export { compileAndShowOutput } from "./code-viewer-compile";
export { openCodeViewer } from "./code-viewer-dialog";

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
