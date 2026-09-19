// Severity classification and HTML-escaping helpers used by the results
// list, the code viewer, and the filter UI. Extracted from main.ts so those
// modules (and their tests) don't need main.ts to re-export them.

// Diagnostic messages are prefixed with `[level] `; every built-in lint
// tags one, so a message with no recognized prefix never actually occurs in
// practice, but severityOf still classifies it as "error" (matching
// Diagnostic::level()'s own fallback in papyrus-lints/src/lib.rs) rather
// than misclassifying it as something less visible.
export type Severity = "error" | "warning" | "info";
export const SEVERITIES: Severity[] = ["error", "warning", "info"];

export function levelOf(message: string): "error" | "warning" | "info" | null {
  const match = /^\[(error|warning|info)\]/.exec(message);
  return match ? (match[1] as "error" | "warning" | "info") : null;
}

export function escapeAttr(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function severityOf(message: string): Severity {
  return levelOf(message) ?? "error";
}
