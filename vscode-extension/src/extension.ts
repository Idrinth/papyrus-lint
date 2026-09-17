import * as vscode from 'vscode';
import { configureCli } from './cli';
import { FIX_ISSUE_COMMAND, PapyrusFixIssueActionProvider, papyrusCodeActionSelector } from './codeActions';
import { isPapyrusDocument, resolveTargetUri } from './documents';
import { initializeConfig } from './init';
import { PapyrusLinter } from './linter';
import { cancelLiveLint, resetLiveLint, scheduleLiveLint } from './liveLint';

export function activate(context: vscode.ExtensionContext): void {
  configureCli(context.globalStorageUri.fsPath, String(context.extension.packageJSON.version));
  resetLiveLint();

  const diagnostics = vscode.languages.createDiagnosticCollection('papyrus-lint');
  const output = vscode.window.createOutputChannel('Papyrus Lint');
  const linter = new PapyrusLinter(diagnostics, output);
  context.subscriptions.push(diagnostics, output);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => void linter.lintDocument(document)),
    vscode.workspace.onDidSaveTextDocument((document) => void linter.lintDocument(document)),
    vscode.workspace.onDidChangeTextDocument((event) => scheduleLiveLint(linter, event.document)),
    vscode.workspace.onDidCloseTextDocument((document) => {
      cancelLiveLint(document);
      if (isPapyrusDocument(document)) {
        linter.clear(document.uri);
      }
    }),
  );

  for (const document of vscode.workspace.textDocuments) {
    void linter.lintDocument(document);
  }

  context.subscriptions.push(
    vscode.commands.registerCommand('papyrusLint.lintFile', async (uri?: vscode.Uri) => {
      const target = await resolveTargetUri(uri);
      if (target) {
        await linter.lint(target);
      }
    }),
    vscode.commands.registerCommand('papyrusLint.fixFile', async (uri?: vscode.Uri) => {
      const target = await resolveTargetUri(uri);
      if (target) {
        await linter.fix(target);
      }
    }),
    vscode.commands.registerCommand(FIX_ISSUE_COMMAND, async (uri: vscode.Uri, rule: string, line: number) => {
      const target = await resolveTargetUri(uri);
      if (target) {
        await linter.fixIssue(target, rule, line);
      }
    }),
    vscode.commands.registerCommand('papyrusLint.initializeConfig', (uri?: vscode.Uri) =>
      initializeConfig(output, uri),
    ),
    vscode.languages.registerCodeActionsProvider(
      papyrusCodeActionSelector(),
      new PapyrusFixIssueActionProvider(),
      { providedCodeActionKinds: PapyrusFixIssueActionProvider.providedCodeActionKinds },
    ),
  );
}

export function deactivate(): void {
  resetLiveLint();
}
