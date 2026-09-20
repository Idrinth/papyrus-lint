import * as vscode from 'vscode';
import { PAPYRUS_LANGUAGE_ID } from './documents';
import { isAlreadyNodiscard, isEligibleNodiscardHeader, nodiscardInsertion } from './nodiscardHeader';

export const ADD_NODISCARD_ACTION_TITLE = 'Add ; @nodiscard flag';

/** Offers an "Add ; @nodiscard flag" quick action on the function header at
 * the cursor/selection when it returns a value or is Native and isn't
 * flagged already. Unlike PapyrusFixIssueActionProvider, this doesn't
 * depend on any diagnostic being present -- it scans the document's own
 * text for an eligible header (see nodiscardHeader.ts) and applies the
 * edit itself, with no CLI round-trip needed. */
export class PapyrusNodiscardActionProvider implements vscode.CodeActionProvider {
  static readonly providedCodeActionKinds = [vscode.CodeActionKind.RefactorRewrite];

  provideCodeActions(document: vscode.TextDocument, range: vscode.Range): vscode.CodeAction[] {
    const lineNumber = range.start.line;
    const lines = document.getText().split(/\r?\n/);
    const header = lines[lineNumber];
    if (header === undefined || !isEligibleNodiscardHeader(header) || isAlreadyNodiscard(lines, lineNumber)) {
      return [];
    }

    const edit = new vscode.WorkspaceEdit();
    edit.insert(document.uri, document.lineAt(lineNumber).range.end, nodiscardInsertion(header));

    const action = new vscode.CodeAction(ADD_NODISCARD_ACTION_TITLE, vscode.CodeActionKind.RefactorRewrite);
    action.edit = edit;
    return [action];
  }
}

export function papyrusNodiscardActionSelector(): vscode.DocumentSelector {
  return { language: PAPYRUS_LANGUAGE_ID, scheme: 'file' };
}
