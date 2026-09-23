import { type Diagnostic } from "./backend-types";

// Appends the triggered rule id in parentheses at the end of a finding's
// on-screen message. A finding with no rule id is labelled `(unknown)`.
export function findingMessageWithRule(finding: Diagnostic): string {
  return `${finding.message} (${finding.rule ?? "unknown"})`;
}
