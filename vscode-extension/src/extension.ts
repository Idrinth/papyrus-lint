import * as vscode from 'vscode';

export function activate(context: vscode.ExtensionContext): void {
  const diagnostics = vscode.languages.createDiagnosticCollection('papyrus-lint');
  context.subscriptions.push(diagnostics);
}

export function deactivate(): void {}
