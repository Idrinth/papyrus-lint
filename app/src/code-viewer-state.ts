import { type Diagnostic } from "./backend-types";
import { hasFixableFindings } from "./finding-fixability";

export interface CodeViewerState {
  path: string;
  source: string;
  findings: Diagnostic[];
}

export let codeViewerState: CodeViewerState | null = null;
export let codeViewerMode: "view" | "edit" = "view";

export let codeViewerEl: HTMLDialogElement | null;
export let codeViewerTitleEl: HTMLElement | null;
export let codeViewerCloseEl: HTMLButtonElement | null;
export let codeViewerViewEl: HTMLElement | null;
export let codeViewerEditEl: HTMLElement | null;
export let codeViewerEditGutterEl: HTMLElement | null;
export let codeViewerEditHighlightEl: HTMLElement | null;
export let codeViewerEditTextareaEl: HTMLTextAreaElement | null;
export let codeViewerEditButtonEl: HTMLButtonElement | null;
export let codeViewerFixButtonEl: HTMLButtonElement | null;
export let codeViewerPreviewFixButtonEl: HTMLButtonElement | null;
export let codeViewerSaveButtonEl: HTMLButtonElement | null;
export let codeViewerSaveCompileButtonEl: HTMLButtonElement | null;
export let codeViewerCancelButtonEl: HTMLButtonElement | null;
export let codeViewerCompileOutputEl: HTMLElement | null;
export let codeViewerDiffOutputEl: HTMLElement | null;
export let codeViewerFullscreenEl: HTMLButtonElement | null;
export let codeViewerAutocompleteEl: HTMLUListElement | null;

export function setCodeViewerState(next: CodeViewerState | null) {
  codeViewerState = next;
}

export function setCodeViewerModeValue(mode: "view" | "edit") {
  codeViewerMode = mode;
}

// Shows the "Apply fixes"/"Preview fixes" buttons only in view mode, and
// only while the currently loaded file still has at least one fixable
// finding (the same check the Lint results list uses to decide whether to
// show its own per-file "Apply fixes" button), so they disappear on their
// own once nothing is left to fix. Called both on every mode change and
// whenever codeViewerState's findings change without a mode change (e.g.
// right after a fix is applied).
export function updateCodeViewerFixButtonsVisibility() {
  const hidden = codeViewerMode !== "view" || !hasFixableFindings(codeViewerState?.findings ?? []);
  if (codeViewerFixButtonEl) {
    codeViewerFixButtonEl.hidden = hidden;
  }
  if (codeViewerPreviewFixButtonEl) {
    codeViewerPreviewFixButtonEl.hidden = hidden;
  }
}

export function bindCodeViewerDom() {
  codeViewerEl = document.querySelector("#code-viewer");
  codeViewerTitleEl = document.querySelector("#code-viewer-title");
  codeViewerCloseEl = document.querySelector("#code-viewer-close");
  codeViewerViewEl = document.querySelector("#code-viewer-view");
  codeViewerEditEl = document.querySelector("#code-viewer-editor");
  codeViewerEditGutterEl = document.querySelector("#code-viewer-editor-gutter");
  codeViewerEditHighlightEl = document.querySelector("#code-viewer-editor-highlight");
  codeViewerEditTextareaEl = document.querySelector("#code-viewer-editor-textarea");
  codeViewerEditButtonEl = document.querySelector("#code-viewer-edit");
  codeViewerFixButtonEl = document.querySelector("#code-viewer-fix");
  codeViewerPreviewFixButtonEl = document.querySelector("#code-viewer-preview-fix");
  codeViewerSaveButtonEl = document.querySelector("#code-viewer-save");
  codeViewerSaveCompileButtonEl = document.querySelector("#code-viewer-save-compile");
  codeViewerCancelButtonEl = document.querySelector("#code-viewer-cancel");
  codeViewerCompileOutputEl = document.querySelector("#code-viewer-compile-output");
  codeViewerDiffOutputEl = document.querySelector("#code-viewer-diff-output");
  codeViewerFullscreenEl = document.querySelector("#code-viewer-fullscreen");
  codeViewerAutocompleteEl = document.querySelector("#code-viewer-autocomplete");
}
