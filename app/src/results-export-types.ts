import { type Diagnostic, type TagImportance } from "./backend";
import { type Severity } from "./main";

// One file's worth of findings that currently pass every active filter
// (filename search, severity, tag, rule), as gathered by
// collectFilteredIssues for the "Export issues" button.
export interface FilteredIssuesFile {
  path: string;
  findings: Diagnostic[];
}

// One finding as sent to the shared papyrus-lint-output crate's
// format_issues_as_text/format_issues_as_json/format_issues_for_ai_base
// Tauri commands (see app/src-tauri/src/export.rs): unlike Diagnostic,
// `rule` is never missing, matching that crate's OwnedDiagnostic (an
// IPC-friendly diagnostic can't carry an optional rule the way
// papyrus_lints::Diagnostic never does either).
export interface IssuesDiagnosticInput {
  line: number;
  column: number;
  rule: string;
  message: string;
}

export interface IssuesFileInput {
  path: string;
  findings: IssuesDiagnosticInput[];
}

// Converts already-filtered findings into the shape those Tauri commands
// expect, defaulting a finding with no rule id to "unknown" the same way
// the GUI's own export formatting always has.
export function toIssuesFileInput(files: FilteredIssuesFile[]): IssuesFileInput[] {
  return files.map((file) => ({
    path: file.path,
    findings: file.findings.map((finding) => ({
      line: finding.line,
      column: finding.column,
      rule: finding.rule ?? "unknown",
      message: finding.message,
    })),
  }));
}

// A serializable snapshot of the GUI-only result filters. The AI export
// includes this alongside the already-filtered findings so its reader can
// distinguish a genuinely clean category from one the user excluded.
export interface ActiveFilters {
  filename_pattern: string;
  severities: Severity[];
  importances: TagImportance[];
  rules: string[];
  auto_fixable_only: boolean;
}

// The four explicit shapes an AI export's per-file `source` field can take
// (see formatIssuesForAi): `null` when no source was attached at all;
// `content` carrying the script's full on-disk text; `hash` carrying only
// its md5 digest, selected via the "Redact source" checkbox next to the
// "Export for AI" button (or the CLI's --hash-source flag) so a report can
// be handed to an external AI without exposing proprietary script text
// while still letting it tell files apart, or notice a file changed between
// exports; and `error` describing why the source couldn't be read (e.g. the
// file was moved or deleted since linting). Kept as a discriminated union
// rather than a plain string so a consumer never has to guess which of
// "full content" or "error message" a given string represents.
export type AiSource =
  | { type: "content"; content: string }
  | { type: "hash"; algorithm: "md5"; hash: string }
  | { type: "error"; message: string };
