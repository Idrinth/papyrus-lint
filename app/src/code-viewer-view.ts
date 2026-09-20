import { type Diagnostic, findingMessageWithRule, isFixableFinding } from "./backend";
import { configKeyForRuleId } from "./config-ui";
import { highlightPapyrusLines } from "./highlight";
import { escapeAttr, levelOf } from "./main-severity";
import { codeViewerViewEl } from "./code-viewer-state";
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

// Builds the per-line action buttons for `lineNumber`'s own table cell:
// "Fix" only when at least one of `lineFindings` has an automatic fix
// (isFixableFinding), "Ignore"/"File disable" only when at least one
// carries a rule id at all (a rule-less finding, e.g. a compiler
// diagnostic, can't be named in an `@disable`/`@disable-file` comment),
// and "Config disable" only when at least one maps onto a papyrus-lint.yaml
// `rules.*` switch. None of the buttons is shown when none apply, so an
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
    buttons.push(
      `<button type="button" class="code-viewer__line-action code-viewer__line-action--file-disable" data-line-action="file-disable" data-line="${lineNumber}">File disable</button>`,
    );
  }
  if (lineFindings.some((finding) => finding.rule !== undefined && configKeyForRuleId(finding.rule) !== undefined)) {
    buttons.push(
      `<button type="button" class="code-viewer__line-action code-viewer__line-action--config-disable" data-line-action="config-disable" data-line="${lineNumber}">Config disable</button>`,
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
      ? ` title="${escapeAttr(lineFindings.map((finding) => findingMessageWithRule(finding)).join("\n"))}"`
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