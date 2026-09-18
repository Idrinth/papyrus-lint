import { invoke } from "@tauri-apps/api/core";
import { type FilteredIssuesFile, toIssuesFileInput } from "./results-export-types";

// Returns `files` with each file's findings sorted by (line, column), so a
// file's diagnostics list in reading order (top to bottom, left to right)
// rather than papyrus_lints::lint()'s own rule-registration order - the same
// ordering the CLI's `--json`/`--ai` output already gets from its own
// `diagnostics.sort_by_key(|d| (d.line, d.column))`
// (papyrus-lint-cli/src/output/mod.rs's finalize_diagnostics). A stable
// sort, so two findings at the exact same position (e.g. a lint diagnostic
// and a compiler-reported one) keep their original relative order. Shared
// with results-export-ai.ts, since the Tauri commands both call into
// (app/src-tauri/src/export.rs) don't sort themselves.
export function sortedByPosition(files: FilteredIssuesFile[]): FilteredIssuesFile[] {
  return files.map((file) => ({
    ...file,
    findings: [...file.findings].sort((a, b) => a.line - b.line || a.column - b.column),
  }));
}

// Renders `files` as JSON, mirroring the shape of the CLI's own `--json`
// report, via the format_issues_as_json Tauri command
// (app/src-tauri/src/export.rs) - built on the same papyrus-lint-output
// crate the CLI itself uses - so both can be consumed by the same tooling.
export async function formatIssuesAsJson(files: FilteredIssuesFile[]): Promise<string> {
  return invoke<string>("format_issues_as_json", { files: toIssuesFileInput(sortedByPosition(files)) });
}
