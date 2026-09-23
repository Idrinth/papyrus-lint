import type { Diagnostic } from "./backend-types";
import { findingsGroupedByLine, lineSeverityOf } from "./code-viewer-view";
import { codeViewerEditGutterEl, codeViewerEditHighlightEl, codeViewerEditTextareaEl } from "./code-viewer-state";
import { highlightPapyrusLines } from "./highlight";
let codeViewerEditFindingsByLine: Map<number, Diagnostic[]> = new Map();
let codeViewerEditLiveFindings: Diagnostic[] = [];

// Replaces the findings shown by the next `updateCodeViewerEditHighlight`
// call, without re-rendering immediately - callers that also need to
// re-render right away (entering edit mode, a live lint resolving) call
// that themselves right after.
export function setLiveEditFindings(findings: Diagnostic[]): void {
  codeViewerEditLiveFindings = findings;
}

// The findings on one source line of the current highlight, as last
// rendered by `updateCodeViewerEditHighlight` - used by the hover tooltip
// (see live-edit-pointer.ts) to show only the hovered line's findings.
export function findingsForEditorLine(line: number): Diagnostic[] | undefined {
  return codeViewerEditFindingsByLine.get(line);
}

// Re-renders the edit mode's syntax-highlighted overlay and line-number
// gutter from the textarea's current value, keeping both in sync as the
// user types.
export function updateCodeViewerEditHighlight() {
  const code = codeViewerEditHighlightEl?.querySelector("code");
  if (!code || !codeViewerEditTextareaEl) {
    return;
  }
  const findings = codeViewerEditLiveFindings;
  const findingsByLine = findingsGroupedByLine(findings);
  codeViewerEditFindingsByLine = findingsByLine;
  const highlightedLines = highlightPapyrusLines(codeViewerEditTextareaEl.value);
  code.innerHTML = highlightedLines
    .map((lineHtml, index) => {
      const lineFindings = findingsByLine.get(index + 1);
      const severity = lineSeverityOf(lineFindings);
      const lineClass = severity
        ? `code-viewer__editor-line code-viewer__line--${severity}`
        : "code-viewer__editor-line";
      return `<span class="${lineClass}">${lineHtml}</span>`;
    })
    // Each line is already `display: block` (see styles.css), so it needs no
    // separator to end up on its own line; joining with "\n" here used to add
    // a literal newline character between spans that, because the highlight
    // layer is `white-space: pre`, rendered as its own extra blank line on
    // top of each blank line's own (empty, so zero-height) span - doubling
    // up and misaligning the overlay against the textarea underneath it.
    .join("");

  if (codeViewerEditGutterEl) {
    codeViewerEditGutterEl.innerHTML = highlightedLines
      .map((_, index) => `<span class="code-viewer__editor-gutter-line">${index + 1}</span>`)
      .join("");
  }

  // The textarea sits above the non-interactive highlighting layer, so its
  // per-line colours are the only ones actually visible; its own `title`
  // (see `updateCodeViewerEditTooltip`) is instead kept in sync with
  // whichever line the mouse currently hovers, the same as the read-only
  // view's per-row tooltips. The accessible description still summarizes
  // every finding at once, since a screen-reader user has no equivalent of
  // "hovering a line" to reveal them one at a time.
  const diagnosticSummary = findings
    .map((finding) => `Line ${finding.line}, column ${finding.column}: ${finding.message}`)
    .join("\n");
  codeViewerEditTextareaEl.setAttribute(
    "aria-label",
    diagnosticSummary ? `Papyrus source editor. Linter findings:\n${diagnosticSummary}` : "Papyrus source editor. No linter findings.",
  );
}
