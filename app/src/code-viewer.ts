import { invoke } from "@tauri-apps/api/core";
import { highlightPapyrusLines } from "./highlight";
import {
  type Diagnostic,
  addDisableCommentToPscLine,
  compilePscFile,
  currentPscOutcomes,
  escapeAttr,
  hasFixableFindings,
  isFixableFinding,
  levelOf,
  previewRepairPscFile,
  repairPscFile,
  repairPscFinding,
} from "./main";
import { cancelLiveEditLint, hideAutocomplete, isCodeViewerEditDirty } from "./live-edit";
import { renderPscResults } from "./results-list";

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

export function lineSeverityOf(lineFindings: Diagnostic[] | undefined): "error" | "warning" | "info" | "flagged" | null {
  if (!lineFindings || lineFindings.length === 0) {
    return null;
  }
  const levels = new Set(lineFindings.map((finding) => levelOf(finding.message)));
  if (levels.has("error")) return "error";
  if (levels.has("warning")) return "warning";
  if (levels.has("info")) return "info";
  // No recognized level prefix (not expected from any built-in lint, but
  // possible from a malformed diagnostic); still mark the line so the
  // finding is visible in the viewer.
  return "flagged";
}

export function findingsGroupedByLine(findings: Diagnostic[]): Map<number, Diagnostic[]> {
  const findingsByLine = new Map<number, Diagnostic[]>();
  for (const finding of findings) {
    const forLine = findingsByLine.get(finding.line) ?? [];
    forLine.push(finding);
    findingsByLine.set(finding.line, forLine);
  }
  return findingsByLine;
}

// Builds the per-line "Fix"/"Ignore" buttons for `lineNumber`'s own table
// cell: "Fix" only when at least one of `lineFindings` has an automatic fix
// (isFixableFinding), "Ignore" only when at least one carries a rule id at
// all (a rule-less finding, e.g. a compiler diagnostic, can't be named in an
// `@disable` comment). Neither button is shown when neither applies, so an
// unremarkable line's actions cell stays empty. The click itself is handled
// by a single delegated listener on codeViewerViewEl (see
// handleCodeViewerLineActionClick), since this HTML is rebuilt from a
// string on every render rather than built up via individual DOM nodes with
// their own listeners.
function buildLineActionsHtml(lineNumber: number, lineFindings: Diagnostic[] | undefined): string {
  if (!lineFindings || lineFindings.length === 0) {
    return "";
  }
  const buttons: string[] = [];
  if (lineFindings.some((finding) => isFixableFinding(finding))) {
    buttons.push(
      `<button type="button" class="code-viewer__line-action code-viewer__line-action--fix" data-line-action="fix" data-line="${lineNumber}">Fix</button>`,
    );
  }
  if (lineFindings.some((finding) => finding.rule !== undefined)) {
    buttons.push(
      `<button type="button" class="code-viewer__line-action code-viewer__line-action--ignore" data-line-action="ignore" data-line="${lineNumber}">Ignore</button>`,
    );
  }
  return buttons.join("");
}

// Renders `source`'s syntax-highlighted, read-only table view with
// `findings` marked on their lines. If `focusLine` is given, scrolls that
// line into view and briefly flashes it, so a click on a specific finding
// jumps straight to it.
export function renderCodeViewerView(source: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerViewEl) {
    return;
  }

  const findingsByLine = findingsGroupedByLine(findings);

  const lines = highlightPapyrusLines(source);
  const rows = lines.map((lineHtml, index) => {
    const lineNumber = index + 1;
    const lineFindings = findingsByLine.get(lineNumber);
    const severity = lineSeverityOf(lineFindings);
    const rowClass = severity ? ` class="code-viewer__line--${severity}"` : "";
    const title = lineFindings
      ? ` title="${escapeAttr(lineFindings.map((f) => f.message).join("\n"))}"`
      : "";
    return (
      `<tr id="code-viewer-line-${lineNumber}"${rowClass}${title}>` +
      `<td class="code-viewer__line-number">${lineNumber}</td>` +
      `<td class="code-viewer__line-code">${lineHtml}</td>` +
      `<td class="code-viewer__line-actions">${buildLineActionsHtml(lineNumber, lineFindings)}</td>` +
      `</tr>`
    );
  });

  codeViewerViewEl.innerHTML = `<table class="code-viewer__table"><tbody>${rows.join("")}</tbody></table>`;

  if (focusLine) {
    const row = codeViewerViewEl.querySelector<HTMLElement>(`#code-viewer-line-${focusLine}`);
    row?.scrollIntoView({ block: "center" });
    row?.classList.add("code-viewer__line--flash");
  }
}

// Shows the view-mode table or the edit-mode textarea/highlight overlay,
// toggling the header's Edit/Save/Cancel buttons to match.
export function setCodeViewerMode(mode: "view" | "edit") {
  codeViewerMode = mode;
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

// Shows the "Apply fixes"/"Preview fixes" buttons only in view mode, and
// only while the currently loaded file still has at least one fixable
// finding (the same check the Lint results list uses to decide whether to
// show its own per-file "Apply fixes" button), so they disappear on their
// own once nothing is left to fix. Called both on every mode change and
// whenever codeViewerState's findings change without a mode change (e.g.
// right after a fix is applied).
function updateCodeViewerFixButtonsVisibility() {
  const hidden = codeViewerMode !== "view" || !hasFixableFindings(codeViewerState?.findings ?? []);
  if (codeViewerFixButtonEl) {
    codeViewerFixButtonEl.hidden = hidden;
  }
  if (codeViewerPreviewFixButtonEl) {
    codeViewerPreviewFixButtonEl.hidden = hidden;
  }
}
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

  codeViewerState = null;
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

  codeViewerState = { path, source, findings };
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
// The code viewer's own "Apply fixes" button: applies every automatic fix
// in the currently open file (the same repair handleFixClick performs from
// the Lint results list), then refreshes the viewer's source/findings in
// place and, since the fix also touches the file on disk, re-syncs the
// matching Lint results list entry - so acting on a file no longer requires
// closing the viewer first.
export async function handleCodeViewerFixClick() {
  if (!codeViewerState || !codeViewerFixButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  codeViewerFixButtonEl.disabled = true;
  try {
    const findings = await repairPscFile(path);
    const source = await invoke<string>("read_psc_file", { path });
    codeViewerState = { path, source, findings };
    renderCodeViewerView(source, findings);
    hideDiffOutput(codeViewerDiffOutputEl);

    const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
    if (outcome) {
      outcome.findings = findings;
      renderPscResults(currentPscOutcomes);
    }
  } catch (error) {
    console.error(error);
  } finally {
    updateCodeViewerFixButtonsVisibility();
    if (codeViewerFixButtonEl) {
      codeViewerFixButtonEl.disabled = false;
    }
  }
}

// Dispatches a click anywhere in the read-only view's table to the right
// per-line handler below, reading which line and which action
// (buildLineActionsHtml's "fix"/"ignore" buttons) off the clicked button's
// own data attributes — the buttons are rebuilt from an HTML string on
// every render, so they're wired up through one delegated listener on
// codeViewerViewEl rather than individual per-button listeners that would
// need reattaching each time.
async function handleCodeViewerLineActionClick(event: MouseEvent) {
  if (!(event.target instanceof Element)) {
    return;
  }
  const button = event.target.closest<HTMLButtonElement>("button[data-line-action]");
  if (!button) {
    return;
  }
  const line = Number(button.dataset.line);
  if (!Number.isInteger(line)) {
    return;
  }
  if (button.dataset.lineAction === "fix") {
    await handleCodeViewerFixLineClick(line, button);
  } else if (button.dataset.lineAction === "ignore") {
    await handleCodeViewerIgnoreLineClick(line, button);
  }
}

// The code viewer's per-line "Fix" button: applies every fixable finding's
// own automatic fix on `line` (see repairPscFinding), one rule at a time so
// a rule whose fix would shift other lines (e.g. property-sorting
// relocating a declaration) can fail and be skipped without blocking the
// rest, then refreshes the viewer and the matching Lint results list entry
// in place - the per-line counterpart of handleCodeViewerFixClick's
// whole-file "Apply fixes".
export async function handleCodeViewerFixLineClick(line: number, button: HTMLButtonElement) {
  if (!codeViewerState) {
    return;
  }
  const { path, findings: initialFindings } = codeViewerState;
  const rules = Array.from(
    new Set(
      initialFindings
        .filter((finding) => finding.line === line && isFixableFinding(finding))
        .map((finding) => finding.rule as string),
    ),
  );
  if (rules.length === 0) {
    return;
  }
  button.disabled = true;
  try {
    let findings = initialFindings;
    for (const rule of rules) {
      try {
        findings = await repairPscFinding(path, rule, line);
      } catch (error) {
        console.error(error);
      }
    }
    const source = await invoke<string>("read_psc_file", { path });
    codeViewerState = { path, source, findings };
    renderCodeViewerView(source, findings);
    hideDiffOutput(codeViewerDiffOutputEl);
    updateCodeViewerFixButtonsVisibility();

    const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
    if (outcome) {
      outcome.findings = findings;
      renderPscResults(currentPscOutcomes);
    }
  } finally {
    button.disabled = false;
  }
}

// The code viewer's per-line "Ignore" button: adds (or extends) an
// `; @disable <rules>` comment covering every rule id found on `line` (see
// addDisableCommentToPscLine), silencing those findings instead of fixing
// them, then refreshes the viewer and the matching Lint results list entry
// in place the same way handleCodeViewerFixLineClick does.
export async function handleCodeViewerIgnoreLineClick(line: number, button: HTMLButtonElement) {
  if (!codeViewerState) {
    return;
  }
  const { path, findings: initialFindings } = codeViewerState;
  const rules = Array.from(
    new Set(
      initialFindings
        .filter((finding) => finding.line === line && finding.rule !== undefined)
        .map((finding) => finding.rule as string),
    ),
  );
  if (rules.length === 0) {
    return;
  }
  button.disabled = true;
  try {
    const findings = await addDisableCommentToPscLine(path, rules, line);
    const source = await invoke<string>("read_psc_file", { path });
    codeViewerState = { path, source, findings };
    renderCodeViewerView(source, findings);
    updateCodeViewerFixButtonsVisibility();

    const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
    if (outcome) {
      outcome.findings = findings;
      renderPscResults(currentPscOutcomes);
    }
  } catch (error) {
    console.error(error);
  } finally {
    button.disabled = false;
  }
}

// The code viewer's "Preview fixes" button: computes the same fix
// handleCodeViewerFixClick would apply, but never writes it to disk,
// rendering a standard diff of what would change instead (see
// previewRepairPscFile/preview_repair_psc_file) - the GUI counterpart of
// `PapyrusLinterCLI fix --dry-run`. Leaves codeViewerState/the Lint results
// list untouched, since nothing on disk actually changed.
export async function handleCodeViewerPreviewFixClick() {
  if (!codeViewerState || !codeViewerPreviewFixButtonEl || !codeViewerDiffOutputEl) {
    return;
  }
  const { path } = codeViewerState;
  codeViewerPreviewFixButtonEl.disabled = true;
  try {
    const diff = await previewRepairPscFile(path);
    renderDiffOutput(codeViewerDiffOutputEl, diff);
  } catch (error) {
    console.error(error);
    renderDiffOutput(codeViewerDiffOutputEl, String(error), true);
  } finally {
    codeViewerPreviewFixButtonEl.disabled = false;
  }
}
// Shows `text` in `outputEl`, styling it as a success or failure so a
// failed compile is easy to spot at a glance.
function showCompileOutput(outputEl: HTMLElement, text: string, success: boolean) {
  outputEl.textContent = text;
  outputEl.hidden = false;
  outputEl.classList.toggle("psc-result__compile-output--ok", success);
  outputEl.classList.toggle("psc-result__compile-output--error", !success);
}

// Hides and clears a previous compile result, if any is showing (e.g. from
// an earlier file in the same code viewer session).
export function hideCompileOutput(outputEl: HTMLElement | null) {
  if (!outputEl) {
    return;
  }
  outputEl.hidden = true;
  outputEl.textContent = "";
  outputEl.classList.remove("psc-result__compile-output--ok", "psc-result__compile-output--error");
}

// Classifies one line of a unified diff (see papyrus_lint_core::diff's
// `--- `/`+++ `/`@@ `/`+`/`-` conventions) for `renderDiffOutput` below, so
// each kind of line can be colored distinctly the way a typical diff viewer
// does.
function diffLineClass(line: string): string {
  if (line.startsWith("@@")) return "code-viewer__diff-line--hunk";
  if (line.startsWith("+++") || line.startsWith("---")) return "code-viewer__diff-line--file";
  if (line.startsWith("+")) return "code-viewer__diff-line--add";
  if (line.startsWith("-")) return "code-viewer__diff-line--remove";
  return "code-viewer__diff-line--context";
}

// Renders `diffText` (a standard unified diff, as returned by
// previewRepairPscFile/PapyrusLinterCLI's `fix --dry-run`) in `outputEl`,
// coloring added/removed/context lines the way a typical diff viewer does.
// An empty `diffText` (nothing would change) is reported as such rather
// than left blank, and `isError` styles a failure to compute the preview
// the same way a failed compile is styled.
function renderDiffOutput(outputEl: HTMLElement, diffText: string, isError = false) {
  outputEl.hidden = false;
  outputEl.classList.toggle("code-viewer__diff-output--error", isError);
  if (isError) {
    outputEl.textContent = diffText;
    return;
  }
  if (diffText === "") {
    outputEl.textContent = "No changes would be made.";
    return;
  }
  const lines = diffText.replace(/\n$/, "").split("\n");
  // Each line is already `display: block` (see styles.css), so it needs no
  // separator to end up on its own line - joining with "\n" would add a
  // literal newline character between spans that, because the panel is
  // `white-space: pre-wrap`, renders as its own extra blank line on top of
  // each line's own block-level break, doubling up the spacing.
  outputEl.innerHTML = lines.map((line) => `<span class="${diffLineClass(line)}">${escapeAttr(line)}</span>`).join("");
}

// Hides and clears a previous fix preview, if any is showing (e.g. from an
// earlier file in the same code viewer session, or a stale preview left
// over from before a real "Apply fixes" ran).
export function hideDiffOutput(outputEl: HTMLElement | null) {
  if (!outputEl) {
    return;
  }
  outputEl.hidden = true;
  outputEl.textContent = "";
  outputEl.classList.remove("code-viewer__diff-output--error");
}

// Compiles `path` via PapyrusCompiler.exe and shows the result in
// `outputEl`, reporting both a successful compile and a compiler-reported
// failure (syntax errors, missing imports, etc.) as well as a failure to
// run the compiler at all (e.g. no path configured). Shared by the "Compile"
// button on the Lint results list and the code viewer's "Save & Compile"
// button.
export async function compileAndShowOutput(path: string, outputEl: HTMLElement): Promise<void> {
  try {
    const outcome = await compilePscFile(path);
    const lines = [outcome.stdout, outcome.stderr].filter((text) => text.trim().length > 0);
    if (outcome.personal_data_stripped) {
      lines.push("Removed your username/computer name from the compiled script.");
    }
    const output = lines.join("\n");
    showCompileOutput(
      outputEl,
      output || (outcome.success ? "Compiled successfully." : "Compilation failed."),
      outcome.success,
    );
  } catch (error) {
    showCompileOutput(outputEl, String(error), false);
    console.error(error);
  }
}

export function bindCodeViewer() {
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
  codeViewerEl?.addEventListener("close", () => {
    codeViewerEl?.classList.remove("code-viewer--fullscreen");
    codeViewerFullscreenEl?.setAttribute("aria-pressed", "false");
    codeViewerFullscreenEl?.setAttribute("aria-label", "Enter fullscreen");
  });
  codeViewerFixButtonEl?.addEventListener("click", () => void handleCodeViewerFixClick());
  codeViewerPreviewFixButtonEl?.addEventListener("click", () => void handleCodeViewerPreviewFixClick());
}
