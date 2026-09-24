import { type PscParseOutcome } from "./backend-types";
import { isFixableFinding } from "./finding-fixability";
import { handleMassFixClick } from "./results-list-actions";
import { pscResultMassFixEl, pscResultMassFixListEl } from "./results-list-state";
// Human-readable names for FIXABLE_RULE_IDS, used to label each rule in the
// "mass fix" panel instead of its raw id. Kept in sync by hand with each
// rule's own settings-tab checkbox label text in index.html.
const FIXABLE_RULE_DISPLAY_NAMES: Record<string, string> = {
  "identifier-casing": "Identifier casing",
  "slow-functions": "Slow function usage",
  semicolon: "Semicolon at end of line",
  indentation: "Formatting checks / Indentation",
  "property-sorting": "Property sorting",
  "comma-spacing": "Space after comma",
  "chain-whitespace": "Whitespace interrupting property/method chaining",
  "exclamation-spacing": "Exclamation mark spacing",
  "operator-spacing": "Spacing around logical/comparison operators",
  "assignment-operator-spacing": "Spacing around assignment operators",
  "type-casing": "Type name casing",
  "trailing-whitespace": "Trailing whitespace",
  "global-variable-increment": "GlobalVariable increment via SetValue(GetValue() + x)",
  "unused-import": "Unused import",
  "final-newline": "Final newline",
  "get-form-from-file-load-index": "Game.GetFormFromFile load index",
  "useless-downcast": "Useless downcast",
  "self-assignment": "Self-assignment",
};

export function massFixRuleDisplayName(rule: string): string {
  return FIXABLE_RULE_DISPLAY_NAMES[rule] ?? rule;
}

// Counts, per rule id, how many currently fixable findings (see
// isFixableFinding) exist across every outcome, for the "mass fix" panel to
// list. A rule with no fixable finding anywhere is omitted entirely.
export function massFixRuleCounts(outcomes: PscParseOutcome[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const outcome of outcomes) {
    for (const finding of outcome.findings) {
      if (finding.rule && isFixableFinding(finding)) {
        counts.set(finding.rule, (counts.get(finding.rule) ?? 0) + 1);
      }
    }
  }
  return counts;
}

// Builds/refreshes the "mass fix" panel listing every rule with at least
// one fixable finding somewhere in `outcomes`, each with a button that
// clears every occurrence of that one rule across every file at once (see
// handleMassFixClick). Hides the panel entirely once no rule qualifies
// (e.g. right after the last one has been mass-fixed).
export function renderMassFixList(outcomes: PscParseOutcome[]) {
  if (!pscResultMassFixEl || !pscResultMassFixListEl) {
    return;
  }

  const counts = massFixRuleCounts(outcomes);
  if (counts.size === 0) {
    pscResultMassFixListEl.replaceChildren();
    pscResultMassFixEl.setAttribute("hidden", "");
    return;
  }

  const rules = [...counts.keys()].sort((a, b) =>
    massFixRuleDisplayName(a).localeCompare(massFixRuleDisplayName(b)),
  );
  pscResultMassFixListEl.replaceChildren(
    ...rules.map((rule) => {
      const count = counts.get(rule) ?? 0;
      const item = document.createElement("li");
      item.classList.add("psc-result__mass-fix-item");

      const label = document.createElement("span");
      label.textContent = `${massFixRuleDisplayName(rule)} (${count})`;
      item.append(label);

      const button = document.createElement("button");
      button.type = "button";
      button.textContent = count === 1 ? "Fix this issue everywhere" : `Fix all ${count} in project`;
      button.classList.add("psc-result__mass-fix-button");
      button.addEventListener("click", () => void handleMassFixClick(rule, outcomes, button));
      item.append(button);

      return item;
    }),
  );
  pscResultMassFixEl.removeAttribute("hidden");
}
