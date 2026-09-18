import { invoke } from "@tauri-apps/api/core";
import { type FilteredIssuesFile, toIssuesFileInput } from "./results-export-types";

// Renders `files` the same way the CLI's plain-text report does (see
// format_diagnostic_line in the shared papyrus-lint-output crate), one
// finding per line - via the format_issues_as_text Tauri command
// (app/src-tauri/src/export.rs), so the exported text can never drift from
// the CLI's own output.
export async function formatIssuesAsText(files: FilteredIssuesFile[]): Promise<string> {
  return invoke<string>("format_issues_as_text", { files: toIssuesFileInput(files) });
}
