import { invoke } from "@tauri-apps/api/core";
import { type Diagnostic, addDisableCommentToPscLine, addNodiscardCommentToPscLine, isFixableFinding, repairPscFile, repairPscFinding } from "./backend";
import { currentPscOutcomes } from "./drop";
import { renderPscResults } from "./results-list-render";
import { codeViewerDiffOutputEl, codeViewerFixButtonEl, codeViewerState, setCodeViewerState, updateCodeViewerFixButtonsVisibility } from "./code-viewer-state";
import { hideDiffOutput } from "./code-viewer-diff";
import { renderCodeViewerView } from "./code-viewer-view";
// Re-reads `path` after a disk mutation, refreshes the viewer's source and
// findings in place, and re-syncs the matching Lint results list entry so
// acting on a file no longer requires closing the viewer first.
async function refreshViewerAfterMutation(path: string, findings: Diagnostic[], hideDiff: boolean) {
  const source = await invoke<string>("read_psc_file", { path });
  setCodeViewerState({ path, source, findings });
  renderCodeViewerView(source, findings);
  if (hideDiff) {
    hideDiffOutput(codeViewerDiffOutputEl);
  }
  updateCodeViewerFixButtonsVisibility();

  const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
  if (outcome) {
    outcome.findings = findings;
    renderPscResults(currentPscOutcomes);
  }
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
    await refreshViewerAfterMutation(path, findings, true);
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
export async function handleCodeViewerLineActionClick(event: MouseEvent) {
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
  } else if (button.dataset.lineAction === "nodiscard") {
    await handleCodeViewerNodiscardLineClick(line, button);
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
    await refreshViewerAfterMutation(path, findings, true);
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
    await refreshViewerAfterMutation(path, findings, false);
  } catch (error) {
    console.error(error);
  } finally {
    button.disabled = false;
  }
}

// The code viewer's per-line "Nodiscard" button: adds (or extends) an
// `; @nodiscard` comment on `line`'s function header (see
// addNodiscardCommentToPscLine), marking it for unused-nodiscard's own
// discarded-result check, then refreshes the viewer and the matching Lint
// results list entry in place the same way the "Fix"/"Ignore" buttons do.
// Only offered on a header nodiscardEligibleLines (nodiscard.ts) says is
// eligible - a function that returns a value or is Native - so unlike
// those two, this button isn't gated by any existing finding on the line.
export async function handleCodeViewerNodiscardLineClick(line: number, button: HTMLButtonElement) {
  if (!codeViewerState) {
    return;
  }
  const { path } = codeViewerState;
  button.disabled = true;
  try {
    const findings = await addNodiscardCommentToPscLine(path, line);
    await refreshViewerAfterMutation(path, findings, false);
  } catch (error) {
    console.error(error);
  } finally {
    button.disabled = false;
  }
}
