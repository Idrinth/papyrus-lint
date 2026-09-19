import { previewRepairPscFile } from "./backend";
import { escapeAttr } from "./main-severity";
import { codeViewerDiffOutputEl, codeViewerPreviewFixButtonEl, codeViewerState } from "./code-viewer-state";
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
