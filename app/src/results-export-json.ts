import { ruleTagsByRule, severityOf } from "./main";
import { type FilteredIssuesFile } from "./results-export-types";

// Returns `files` with each file's findings sorted by (line, column), so a
// file's diagnostics list in reading order (top to bottom, left to right)
// rather than papyrus_lints::lint()'s own rule-registration order - the same
// ordering the CLI's `--json`/`--ai` output already gets from its own
// `diagnostics.sort_by_key(|d| (d.line, d.column))`
// (papyrus-lint-cli/src/lib.rs). A stable sort, so two findings at the exact
// same position (e.g. a lint diagnostic and a compiler-reported one) keep
// their original relative order.
export function sortedByPosition(files: FilteredIssuesFile[]): FilteredIssuesFile[] {
  return files.map((file) => ({
    ...file,
    findings: [...file.findings].sort((a, b) => a.line - b.line || a.column - b.column),
  }));
}

// Shared by formatIssuesAsJson and formatIssuesForAi: `files` as a plain
// object mirroring the CLI's own `--json` report shape
// (JsonReport/JsonFileReport/JsonDiagnostic in papyrus-lint-cli/src/lib.rs).
export function buildIssuesReport(files: FilteredIssuesFile[], stripSeverityPrefix = false) {
  let totalDiagnostics = 0;
  const jsonFiles = files.map((file) => {
    totalDiagnostics += file.findings.length;
    return {
      path: file.path,
      diagnostics: file.findings.map((finding) => ({
        line: finding.line,
        column: finding.column,
        rule: finding.rule ?? "unknown",
        level: severityOf(finding.message),
        message: stripSeverityPrefix
          ? finding.message.replace(/^\[(?:error|warning|info)\]\s*/, "")
          : finding.message,
        doc_url: (finding.rule ? ruleTagsByRule.get(finding.rule)?.doc_url : undefined) ?? null,
      })),
    };
  });
  return {
    files: jsonFiles,
    files_with_diagnostics: jsonFiles.length,
    total_diagnostics: totalDiagnostics,
  };
}

// Renders `files` as JSON, mirroring the shape of the CLI's own `--json`
// report so both can be consumed by the same tooling.
export function formatIssuesAsJson(files: FilteredIssuesFile[]): string {
  return JSON.stringify(buildIssuesReport(sortedByPosition(files)), null, 2);
}
