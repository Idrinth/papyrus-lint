import { promises as fs } from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { configPathForWrite } from './config';
import { isPapyrusDocument } from './documents';
import type { PapyrusLinter } from './linter';
import { addFileDisableComment, disableRuleInConfigYaml } from './suppressions';

function sameUri(left: vscode.Uri, right: vscode.Uri): boolean {
  return left.toString() === right.toString() || left.fsPath === right.fsPath;
}

function openDocument(uri: vscode.Uri): vscode.TextDocument | undefined {
  return vscode.workspace.textDocuments.find((document) => sameUri(document.uri, uri));
}

function fullDocumentRange(document: vscode.TextDocument): vscode.Range {
  const text = document.getText();
  return new vscode.Range(document.positionAt(0), document.positionAt(text.length));
}

async function replaceDocumentText(document: vscode.TextDocument, updated: string): Promise<boolean> {
  const edit = new vscode.WorkspaceEdit();
  edit.replace(document.uri, fullDocumentRange(document), updated);
  return vscode.workspace.applyEdit(edit);
}

async function readConfigText(configUri: vscode.Uri): Promise<string> {
  const open = openDocument(configUri);
  if (open) {
    return open.getText();
  }
  try {
    return await fs.readFile(configUri.fsPath, 'utf8');
  } catch {
    return '';
  }
}

async function writeConfigText(configUri: vscode.Uri, updated: string): Promise<boolean> {
  const open = openDocument(configUri);
  if (open) {
    const applied = await replaceDocumentText(open, updated);
    if (!applied) {
      return false;
    }
    return open.save();
  }
  await fs.mkdir(path.dirname(configUri.fsPath), { recursive: true });
  await fs.writeFile(configUri.fsPath, updated, 'utf8');
  return true;
}

async function relintOpenPapyrusDocuments(linter: PapyrusLinter): Promise<void> {
  for (const document of vscode.workspace.textDocuments) {
    if (!isPapyrusDocument(document)) {
      continue;
    }
    if (document.isDirty) {
      await linter.lintBlob(document);
    } else {
      await linter.lint(document.uri);
    }
  }
}

/** Inserts or extends a `; @disable-file <rule>` comment in the script, then
 * re-lints the (now dirty) buffer via `--blob` so the diagnostic disappears
 * without forcing a save. */
export async function ignoreIssueForFile(linter: PapyrusLinter, uri: vscode.Uri, rule: string): Promise<void> {
  let document = openDocument(uri);
  if (!document) {
    try {
      document = await vscode.workspace.openTextDocument(uri);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`Papyrus Lint: could not open ${path.basename(uri.fsPath)} (${message}).`);
      return;
    }
  }
  const updated = addFileDisableComment(document.getText(), rule);
  if (updated !== document.getText()) {
    const applied = await replaceDocumentText(document, updated);
    if (!applied) {
      void vscode.window.showErrorMessage(`Papyrus Lint: could not add @disable-file for "${rule}".`);
      return;
    }
  }
  await linter.lintBlob(document);
  void vscode.window.showInformationMessage(
    `Papyrus Lint: ignoring "${rule}" for ${path.basename(uri.fsPath)}.`,
  );
}

/** Turns `rules.<rule>: false` in the project's papyrus-lint.yaml (creating the
 * file if the workspace has none yet), then re-lints every open `.psc`. */
export async function ignoreIssueForProject(linter: PapyrusLinter, uri: vscode.Uri, rule: string): Promise<void> {
  const targetPath = await configPathForWrite(uri);
  if (!targetPath) {
    void vscode.window.showErrorMessage(
      'Papyrus Lint: open a folder or set papyrusLint.configPath to ignore a lint for the project.',
    );
    return;
  }

  const configUri = vscode.Uri.file(targetPath);
  const current = await readConfigText(configUri);
  const updated = disableRuleInConfigYaml(current, rule);
  if (updated !== current) {
    const written = await writeConfigText(configUri, updated);
    if (!written) {
      void vscode.window.showErrorMessage(
        `Papyrus Lint: could not disable "${rule}" in ${path.basename(targetPath)}.`,
      );
      return;
    }
  }
  await relintOpenPapyrusDocuments(linter);
  void vscode.window.showInformationMessage(
    `Papyrus Lint: ignoring "${rule}" for the project in ${path.basename(targetPath)}.`,
  );
}
