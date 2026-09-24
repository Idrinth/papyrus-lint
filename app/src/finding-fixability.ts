import { type Diagnostic } from "./backend-types";

// Rule ids with an automatic fix (papyrus_lints::FIXABLE_RULE_IDS), used to
// decide which findings offer the per-finding "Fix this issue" button. Kept
// in sync by hand with FIXABLE_RULE_IDS in papyrus-lints.
export const FIXABLE_RULE_IDS = new Set([
  "identifier-casing",
  "slow-functions",
  "semicolon",
  "indentation",
  "property-sorting",
  "comma-spacing",
  "chain-whitespace",
  "exclamation-spacing",
  "operator-spacing",
  "assignment-operator-spacing",
  "type-casing",
  "trailing-whitespace",
  "global-variable-increment",
  "unnecessary-function",
  "unused-import",
  "final-newline",
  "get-form-from-file-load-index",
  "useless-downcast",
]);

// Some findings from a fixable rule still require a substantive rename and
// carry this note so callers do not offer an automatic fix for that instance.
const NO_AUTOMATIC_FIX_NOTE = "no automatic fix";

export function hasNoAutomaticFix(finding: Diagnostic): boolean {
  return finding.message.includes(NO_AUTOMATIC_FIX_NOTE);
}

export function isFixableFinding(finding: Diagnostic): boolean {
  return finding.rule !== undefined && FIXABLE_RULE_IDS.has(finding.rule) && !hasNoAutomaticFix(finding);
}

export function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) => isFixableFinding(finding));
}
