/** Mirrors `papyrus_lint_cli::JsonDiagnostic` (see `crates/papyrus-lint-cli/src/lib.rs`). */
export interface JsonDiagnostic {
  line: number;
  column: number;
  rule: string;
  level: 'error' | 'warning' | 'info' | null;
  message: string;
}

/** Mirrors `papyrus_lint_cli::JsonFileReport`. */
interface JsonFileReport {
  path: string;
  diagnostics: JsonDiagnostic[];
}

/** Mirrors `papyrus_lint_cli::JsonReport`, as printed by `PapyrusLinterCLI --json`. */
export interface JsonReport {
  files: JsonFileReport[];
  scripts_checked: number;
  files_with_diagnostics: number;
  total_diagnostics: number;
  files_fixed: number | null;
  success: boolean;
}

export type DiagnosticLevel = 'error' | 'warning' | 'information';

export interface NormalizedDiagnostic {
  line: number;
  column: number;
  level: DiagnosticLevel;
  message: string;
  rule: string;
}

export function normalizeDiagnostic(entry: JsonDiagnostic): NormalizedDiagnostic {
  return {
    line: Math.max(entry.line - 1, 0),
    column: Math.max(entry.column - 1, 0),
    level: entry.level === 'warning' ? 'warning' : entry.level === 'info' ? 'information' : 'error',
    message: entry.message,
    rule: entry.rule,
  };
}

export function parseReport(stdout: string): JsonReport | undefined {
  try {
    return JSON.parse(stdout) as JsonReport;
  } catch {
    return undefined;
  }
}
