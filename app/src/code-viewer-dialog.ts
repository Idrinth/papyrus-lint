import { invoke } from "@tauri-apps/api/core";
import { type Diagnostic } from "./backend-types";
import { currentProjectLintContext } from "./backend-context";
import { isCodeViewerEditDirty } from "./live-edit-persist";
import { hideCompileOutput } from "./code-viewer-compile";
import { hideDiffOutput } from "./code-viewer-diff";
import { setCodeViewerMode } from "./code-viewer-mode";
import { codeViewerCompileOutputEl, codeViewerDiffOutputEl, codeViewerEl, codeViewerFullscreenEl, codeViewerTitleEl, codeViewerViewEl, setCodeViewerState, updateCodeViewerFixButtonsVisibility } from "./code-viewer-state";
import { renderCodeViewerView } from "./code-viewer-view";
// Closes the code viewer, confirming first if edit mode has unsaved changes.
export function requestCloseCodeViewer() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  codeViewerEl?.close();
}

// Re-lints `path` against the current on-disk contents so per-line Fix/Ignore/
// File disable/Config disable (including `; @disable` comments) use line
// numbers that still match the source we just read — the Lint results list
// can lag behind if the file changed after the last project lint. Falls
// back to `fallback` when the backend call fails so a viewer that opened
// from a snapshot still works.
async function lintOpenedPscFile(path: string, fallback: Diagnostic[]): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      context: currentProjectLintContext(),
    });
  } catch {
    return fallback;
  }
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

  const liveFindings = await lintOpenedPscFile(path, findings);
  setCodeViewerState({ path, source, findings: liveFindings });
  updateCodeViewerFixButtonsVisibility();
  renderCodeViewerView(source, liveFindings, focusLine);
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
