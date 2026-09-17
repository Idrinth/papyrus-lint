import { type FilteredIssuesFile } from "./results-export-types";

// Renders `files` the same way the CLI's plain-text report does (see
// format_diagnostic_line in papyrus-lint-cli), one finding per line, so the
// exported text stays familiar to anyone who's already used the CLI's
// output.
export function formatIssuesAsText(files: FilteredIssuesFile[]): string {
  const lines: string[] = [];
  for (const file of files) {
    for (const finding of file.findings) {
      lines.push(`${file.path}:${finding.line}:${finding.column}: [${finding.rule ?? "unknown"}] ${finding.message}`);
    }
  }
  return lines.join("\n");
}
