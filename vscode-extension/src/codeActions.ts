import * as vscode from 'vscode';
import { PAPYRUS_LANGUAGE_ID } from './documents';
import { ruleOfDiagnosticCode } from './vscodeDiagnostics';

export const FIX_ISSUE_COMMAND = 'papyrusLint.fixIssue';

/** Offers a "Fix this issue" quick fix for each papyrus-lint diagnostic under the
 * cursor/selection, filtering the CLI's fix down to that diagnostic's own rule and
 * line so fixing one issue never touches any other. */
export class PapyrusFixIssueActionProvider implements vscode.CodeActionProvider {
  static readonly providedCodeActionKinds = [vscode.CodeActionKind.QuickFix];

  provideCodeActions(
    document: vscode.TextDocument,
    _range: vscode.Range,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    return context.diagnostics
      .filter((diagnostic) => diagnostic.source === 'papyrus-lint' && ruleOfDiagnosticCode(diagnostic.code) !== undefined)
      .map((diagnostic) => {
        const rule = ruleOfDiagnosticCode(diagnostic.code) as string;
        const line = diagnostic.range.start.line + 1;
        const action = new vscode.CodeAction(`Fix this issue (${rule})`, vscode.CodeActionKind.QuickFix);
        action.diagnostics = [diagnostic];
        action.command = {
          command: FIX_ISSUE_COMMAND,
          title: `Fix this issue (${rule})`,
          arguments: [document.uri, rule, line],
        };
        return action;
      });
  }
}

export function papyrusCodeActionSelector(): vscode.DocumentSelector {
  return { language: PAPYRUS_LANGUAGE_ID, scheme: 'file' };
}
