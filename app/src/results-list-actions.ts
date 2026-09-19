import { type Diagnostic, type PscParseOutcome, isFixableFinding, repairPscFile, repairPscFileRule, repairPscFinding } from "./backend";
import { compileAndShowOutput } from "./code-viewer-compile";
import { currentPscOutcomes } from "./drop";
import { renderPscResults } from "./results-list-render";
export async function handleFixClick(path: string, outcome: PscParseOutcome, button: HTMLButtonElement) {
  button.disabled = true;
  try {
    outcome.findings = await repairPscFile(path);
  } catch (error) {
    console.error(error);
  } finally {
    renderPscResults(currentPscOutcomes);
  }
}

// Mass-fixes `rule` (an id from FIXABLE_RULE_IDS) across every file in
// `outcomes` that currently has a fixable finding for it, then re-renders
// the whole results list (including this panel) once every file has been
// repaired. A single file's fix failing (e.g. an I/O error) is logged and
// otherwise ignored, the same way handleFixClick treats a failed whole-file
// repair, so one bad file can't stop the rest of the project from being
// fixed.
export async function handleMassFixClick(rule: string, outcomes: PscParseOutcome[], button: HTMLButtonElement) {
  button.disabled = true;
  try {
    const targets = outcomes.filter((outcome) =>
      outcome.findings.some((finding) => finding.rule === rule && isFixableFinding(finding)),
    );
    await Promise.all(
      targets.map(async (outcome) => {
        try {
          outcome.findings = await repairPscFileRule(outcome.path, rule);
        } catch (error) {
          console.error(error);
        }
      }),
    );
  } finally {
    renderPscResults(currentPscOutcomes);
  }
}

// Applies just `finding`'s own automatic fix (see repairPscFinding), rather
// than every fixable finding in the file. On success, re-renders the whole
// results list like handleFixClick does. On failure (e.g. the fix would
// change the file's line count elsewhere, like property-sorting relocating
// a declaration), leaves the list as-is and shows `error` inline next to
// the finding instead, since a full re-render would just discard it.
export async function handleFixIssueClick(
  path: string,
  outcome: PscParseOutcome,
  finding: Diagnostic,
  button: HTMLButtonElement,
  errorEl: HTMLElement,
) {
  if (!finding.rule) {
    return;
  }
  button.disabled = true;
  errorEl.hidden = true;
  errorEl.textContent = "";
  try {
    outcome.findings = await repairPscFinding(path, finding.rule, finding.line);
    renderPscResults(currentPscOutcomes);
  } catch (error) {
    console.error(error);
    errorEl.textContent = String(error);
    errorEl.hidden = false;
    button.disabled = false;
  }
}

// Compiles `path` via PapyrusCompiler.exe when the "Compile" button is
// clicked.
export async function handleCompileClick(path: string, button: HTMLButtonElement, outputEl: HTMLElement) {
  button.disabled = true;
  const originalLabel = button.textContent;
  button.textContent = "Compiling…";
  try {
    await compileAndShowOutput(path, outputEl);
  } finally {
    button.disabled = false;
    button.textContent = originalLabel;
  }
}
