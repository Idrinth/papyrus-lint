import { type Diagnostic, type PscParseOutcome, hasFixableFindings, hasNoAutomaticFix, isFixableFinding } from "./backend";
import { openCodeViewer } from "./code-viewer-dialog";
import { levelOf } from "./main-severity";
import { relativePath } from "./path";
import { currentProjectDir } from "./project-state";
import { handleCompileClick, handleFixClick, handleFixIssueClick } from "./results-list-actions";
import { tagsForFinding } from "./results-filter";
// Builds the small badge row surfacing `finding`'s own rule's tag metadata
// (kind(s), importance, and whether it has an automatic fix), or null if
// its rule carries no tag metadata (see tagsForFinding).
function buildFindingTagsEl(finding: Diagnostic): HTMLElement | null {
  const tags = tagsForFinding(finding);
  if (!tags) {
    return null;
  }

  const tagsEl = document.createElement("span");
  tagsEl.classList.add("psc-result__finding-tags");

  for (const kind of tags.kinds) {
    const badge = document.createElement("span");
    badge.classList.add("psc-result__tag-badge", "psc-result__tag-badge--kind");
    badge.textContent = kind;
    tagsEl.append(badge);
  }

  const importanceBadge = document.createElement("span");
  importanceBadge.classList.add("psc-result__tag-badge", `psc-result__tag-badge--importance-${tags.importance}`);
  importanceBadge.textContent = `${tags.importance} importance`;
  tagsEl.append(importanceBadge);

  if (tags.auto_fixable && !hasNoAutomaticFix(finding)) {
    const fixableBadge = document.createElement("span");
    fixableBadge.classList.add("psc-result__tag-badge", "psc-result__tag-badge--auto-fixable");
    fixableBadge.textContent = "auto-fixable";
    tagsEl.append(fixableBadge);
  }

  const docsLink = document.createElement("a");
  docsLink.classList.add("psc-result__tag-badge", "psc-result__tag-badge--docs-link");
  docsLink.href = tags.doc_url;
  docsLink.target = "_blank";
  docsLink.rel = "noopener noreferrer";
  docsLink.textContent = "docs";
  tagsEl.append(docsLink);

  return tagsEl;
}

// Builds the list item for an already-filtered file. `findings` is the
// subset filterOutcomes selected for display; `outcome` is the original
// parse/lint result, used by file-level actions (View code, Apply fixes)
// so those still see every finding in the file. Returns null if there's
// nothing to show (a successfully parsed file with no findings to list).
export function buildPscResultItem(
  outcome: PscParseOutcome,
  findings: Diagnostic[] = outcome.findings,
): HTMLLIElement | null {
  const { path, ok, detail } = outcome;

  if (ok && findings.length === 0) {
    return null;
  }

  const item = document.createElement("li");
  item.classList.add(ok ? "psc-result__item--ok" : "psc-result__item--error");

  const summary = document.createElement("span");
  summary.textContent = `${relativePath(path, currentProjectDir)}: ${detail}`;
  item.append(summary);

  const viewButton = document.createElement("button");
  viewButton.type = "button";
  viewButton.textContent = "View code";
  viewButton.classList.add("psc-result__view-button");
  viewButton.addEventListener("click", () => void openCodeViewer(path, outcome.findings));
  item.append(viewButton);

  if (hasFixableFindings(outcome.findings)) {
    const fixButton = document.createElement("button");
    fixButton.type = "button";
    fixButton.textContent = "Apply fixes";
    fixButton.classList.add("psc-result__fix-button");
    fixButton.addEventListener("click", () => void handleFixClick(path, outcome, fixButton));
    item.append(fixButton);
  }

  const compileButton = document.createElement("button");
  compileButton.type = "button";
  compileButton.textContent = "Compile";
  compileButton.classList.add("psc-result__compile-button");
  const compileOutputEl = document.createElement("pre");
  compileOutputEl.classList.add("psc-result__compile-output");
  compileOutputEl.hidden = true;
  compileButton.addEventListener("click", () => void handleCompileClick(path, compileButton, compileOutputEl));
  item.append(compileButton);

  if (findings.length > 0) {
    const findingsList = document.createElement("ul");
    findingsList.classList.add("psc-result__findings");
    findingsList.replaceChildren(
      ...findings.map((finding) => {
        const findingItem = document.createElement("li");
        findingItem.classList.add("psc-result__finding");
        const level = levelOf(finding.message);
        if (level) {
          findingItem.classList.add(`psc-result__finding--${level}`);
        }
        findingItem.addEventListener("click", () => void openCodeViewer(path, outcome.findings, finding.line));

        const label = document.createElement("span");
        label.textContent = `line ${finding.line}, col ${finding.column}: ${finding.message}`;
        findingItem.append(label);

        const tagsEl = buildFindingTagsEl(finding);
        if (tagsEl) {
          findingItem.append(tagsEl);
        }

        if (isFixableFinding(finding)) {
          const fixIssueButton = document.createElement("button");
          fixIssueButton.type = "button";
          fixIssueButton.textContent = "Fix this issue";
          fixIssueButton.classList.add("psc-result__finding-fix-button");
          const fixIssueErrorEl = document.createElement("span");
          fixIssueErrorEl.classList.add("psc-result__finding-fix-error");
          fixIssueErrorEl.hidden = true;
          fixIssueButton.addEventListener("click", (event) => {
            event.stopPropagation();
            void handleFixIssueClick(path, outcome, finding, fixIssueButton, fixIssueErrorEl);
          });
          findingItem.append(fixIssueButton, fixIssueErrorEl);
        }

        return findingItem;
      }),
    );
    item.append(findingsList);
  }

  item.append(compileOutputEl);

  return item;
}
