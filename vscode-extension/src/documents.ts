import * as path from 'path';
import * as vscode from 'vscode';

export const PAPYRUS_LANGUAGE_ID = 'papyrus';

export function isPapyrusDocument(document: vscode.TextDocument): boolean {
  return document.languageId === PAPYRUS_LANGUAGE_ID && document.uri.scheme === 'file';
}

export function isPscFile(uri: vscode.Uri): boolean {
  return path.extname(uri.fsPath).toLowerCase() === '.psc';
}

/** Resolves the `.psc` file a command should act on: the given `uri` (e.g. from an
 * explorer/editor context menu), falling back to the active editor. Saves it first if
 * it's open and has unsaved changes, since the CLI only ever reads from disk. */
export async function resolveTargetUri(uri: vscode.Uri | undefined): Promise<vscode.Uri | undefined> {
  const target = uri ?? vscode.window.activeTextEditor?.document.uri;
  if (!target || target.scheme !== 'file' || !isPscFile(target)) {
    void vscode.window.showWarningMessage('Papyrus Lint: open or select a .psc file first.');
    return undefined;
  }

  const openDocument = vscode.workspace.textDocuments.find(
    (document) => document.uri.toString() === target.toString(),
  );
  if (openDocument?.isDirty) {
    const saved = await openDocument.save();
    if (!saved) {
      void vscode.window.showWarningMessage('Papyrus Lint: save the file before linting.');
      return undefined;
    }
  }

  return target;
}
