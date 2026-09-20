import * as vscode from 'vscode';
import { PAPYRUS_LANGUAGE_ID } from './documents';
import { isIgnorableRule, fileDisableCovers, lineDisableCovers } from './suppressions';
import { ruleOfDiagnosticCode } from './vscodeDiagnostics';

export const FIX_ISSUE_COMMAND = 'papyrusLint.fixIssue';
export const IGNORE_ISSUE_FOR_LINE_COMMAND = 'papyrusLint.ignoreIssueForLine';
export const IGNORE_ISSUE_FOR_FILE_COMMAND = 'papyrusLint.ignoreIssueForFile';
export const IGNORE_ISSUE_FOR_PROJECT_COMMAND = 'papyrusLint.ignoreIssueForProject';

function actionFor(
  title: string,
  diagnostic: vscode.Diagnostic,
  command: string,
  args: unknown[],
): vscode.CodeAction {
  const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
  action.diagnostics = [diagnostic];
  action.command = { command, title, arguments: args };
  return action;
}

/** Offers a "Fix this issue" quick fix plus line/file/project ignore actions for
 * each papyrus-lint diagnostic under the cursor/selection. The fix filters the
 * CLI down to that diagnostic's own rule and line so fixing one issue never
 * touches any other; the ignore actions add `@disable`, `@disable-file`, or
 * turn the rule off in papyrus-lint.yaml. */
export class PapyrusFixIssueActionProvider implements vscode.CodeActionProvider {
  static readonly providedCodeActionKinds = [vscode.CodeActionKind.QuickFix];

  provideCodeActions(
    document: vscode.TextDocument,
    _range: vscode.Range,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    const actions: vscode.CodeAction[] = [];
    const source = document.getText();
    for (const diagnostic of context.diagnostics) {
      if (diagnostic.source !== 'papyrus-lint') {
        continue;
      }
      const rule = ruleOfDiagnosticCode(diagnostic.code);
      if (rule === undefined) {
        continue;
      }
      const line = diagnostic.range.start.line + 1;
      actions.push(
        actionFor(`Fix this issue (${rule})`, diagnostic, FIX_ISSUE_COMMAND, [document.uri, rule, line]),
      );
      if (!isIgnorableRule(rule)) {
        continue;
      }
      if (!lineDisableCovers(source, line, rule)) {
        actions.push(
          actionFor(
            `Ignore this lint for the line (${rule})`,
            diagnostic,
            IGNORE_ISSUE_FOR_LINE_COMMAND,
            [document.uri, rule, line],
          ),
        );
      }
      if (!fileDisableCovers(source, rule)) {
        actions.push(
          actionFor(
            `Ignore this lint for the file (${rule})`,
            diagnostic,
            IGNORE_ISSUE_FOR_FILE_COMMAND,
            [document.uri, rule],
          ),
        );
      }
      actions.push(
        actionFor(
          `Ignore this lint for the project (${rule})`,
          diagnostic,
          IGNORE_ISSUE_FOR_PROJECT_COMMAND,
          [document.uri, rule],
        ),
      );
    }
    return actions;
  }
}

export function papyrusCodeActionSelector(): vscode.DocumentSelector {
  return { language: PAPYRUS_LANGUAGE_ID, scheme: 'file' };
}
