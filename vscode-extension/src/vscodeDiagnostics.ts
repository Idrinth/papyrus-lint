import * as vscode from 'vscode';
import { normalizeDiagnostic, type JsonDiagnostic } from './diagnostics';

function severityOf(level: ReturnType<typeof normalizeDiagnostic>['level']): vscode.DiagnosticSeverity {
  switch (level) {
    case 'warning':
      return vscode.DiagnosticSeverity.Warning;
    case 'information':
      return vscode.DiagnosticSeverity.Information;
    default:
      return vscode.DiagnosticSeverity.Error;
  }
}

export function toDiagnostic(entry: JsonDiagnostic): vscode.Diagnostic {
  const normalized = normalizeDiagnostic(entry);
  const range = new vscode.Range(
    normalized.line,
    normalized.column,
    normalized.line,
    normalized.column + 1,
  );
  const diagnostic = new vscode.Diagnostic(range, normalized.message, severityOf(normalized.level));
  diagnostic.source = 'papyrus-lint';
  // A rule with a known documentation link gets a clickable {value, target}
  // code (VS Code renders it as a link to that rule's own explanation);
  // a rule with no tag metadata (e.g. a compiler-reported diagnostic) keeps
  // the plain string form.
  diagnostic.code = normalized.docUrl
    ? { value: normalized.rule, target: vscode.Uri.parse(normalized.docUrl) }
    : normalized.rule;
  return diagnostic;
}

/** Extracts a diagnostic's rule id from its `code`, whichever of the two
 * shapes `toDiagnostic` gave it (a plain string, or a `{value, target}`
 * object for a rule with a documentation link). Returns `undefined` for a
 * diagnostic with no code at all, or one not raised by papyrus-lint. */
export function ruleOfDiagnosticCode(code: vscode.Diagnostic['code']): string | undefined {
  if (typeof code === 'string') {
    return code;
  }
  if (typeof code === 'object' && code !== null && typeof code.value === 'string') {
    return code.value;
  }
  return undefined;
}
