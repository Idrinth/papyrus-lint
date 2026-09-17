import { invoke } from "@tauri-apps/api/core";
import { type Diagnostic } from "./backend";
import { isCodeViewerEditDirty } from "./live-edit";
import { hideCompileOutput } from "./code-viewer-compile";
import { hideDiffOutput } from "./code-viewer-diff";
import { setCodeViewerMode } from "./code-viewer-mode";
import {
  codeViewerCompileOutputEl,
  codeViewerDiffOutputEl,
  codeViewerEl,
  codeViewerFullscreenEl,
  codeViewerTitleEl,
  codeViewerViewEl,
  setCodeViewerState,
  updateCodeViewerFixButtonsVisibility,
} from "./code-viewer-state";
import { renderCodeViewerView } from "./code-viewer-view";

// Closes the code viewer, confirming first if edit mode has unsaved changes.
export function requestCloseCodeViewer() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  codeViewerEl?.close();
}

// Reads and syntax-highlights `path`'s source, then opens the code viewer
// dialog with `findings` marked on their lines. If `focusLine` is given,
// scrolls that line into view and briefly flashes it, so a click on a
// specific finding jumps straight to it.
export async function openCodeViewer(path: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerEl || !codeViewerTitleEl || !codeViewerViewEl) {
    return;
  }

  setCodeViewerState(null);
  setCodeViewerMode("view");
  hideCompileOutput(codeViewerCompileOutputEl);
  hideDiffOutput(codeViewerDiffOutputEl);
  codeViewerTitleEl.textContent = path;
  codeViewerViewEl.textContent = "Loading…";
  codeViewerEl.showModal();

  let source: string;
  try {
    source = await invoke<string>("read_psc_file", { path });
  } catch (error) {
    codeViewerViewEl.textContent = `Failed to read file: ${String(error)}`;
    return;
  }

  setCodeViewerState({ path, source, findings });
  updateCodeViewerFixButtonsVisibility();
  renderCodeViewerView(source, findings, focusLine);
}

// Toggles the code viewer between its default size and filling the window,
// keeping the button's label/state in sync.
export function toggleCodeViewerFullscreen() {
  if (!codeViewerEl || !codeViewerFullscreenEl) {
    return;
  }
  const isFullscreen = codeViewerEl.classList.toggle("code-viewer--fullscreen");
  codeViewerFullscreenEl.setAttribute("aria-pressed", String(isFullscreen));
  codeViewerFullscreenEl.setAttribute("aria-label", isFullscreen ? "Exit fullscreen" : "Enter fullscreen");
}

export function resetCodeViewerFullscreen() {
  codeViewerEl?.classList.remove("code-viewer--fullscreen");
  codeViewerFullscreenEl?.setAttribute("aria-pressed", "false");
  codeViewerFullscreenEl?.setAttribute("aria-label", "Enter fullscreen");
}
