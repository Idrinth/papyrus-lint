import { execFile } from 'child_process';
import * as path from 'path';
import * as vscode from 'vscode';
import { normalizeDiagnostic, parseReport, type JsonDiagnostic, type JsonReport } from './diagnostics';

const PAPYRUS_LANGUAGE_ID = 'papyrus';

interface CliResult {
  /** The process exit code, or `-1` if the CLI executable itself couldn't be launched. */
  code: number;
  stdout: string;
  stderr: string;
}

function isPapyrusDocument(document: vscode.TextDocument): boolean {
  return document.languageId === PAPYRUS_LANGUAGE_ID && document.uri.scheme === 'file';
}

function isPscFile(uri: vscode.Uri): boolean {
  return path.extname(uri.fsPath).toLowerCase() === '.psc';
}

function cliPath(): string {
  return vscode.workspace.getConfiguration('papyrusLint').get<string>('cliPath', 'PapyrusLinterCLI');
}

function runCli(args: string[], cwd: string): Promise<CliResult> {
  return new Promise((resolve) => {
    execFile(cliPath(), args, { cwd, maxBuffer: 10 * 1024 * 1024 }, (error, stdout, stderr) => {
      if (error && typeof (error as NodeJS.ErrnoException).code !== 'number') {
        // The executable itself couldn't be launched (e.g. not found on PATH).
        resolve({ code: -1, stdout, stderr: error.message });
        return;
      }
      const code = error ? ((error as NodeJS.ErrnoException).code as unknown as number) : 0;
      resolve({ code, stdout, stderr });
    });
  });
}

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

function toDiagnostic(entry: JsonDiagnostic): vscode.Diagnostic {
  const normalized = normalizeDiagnostic(entry);
  const range = new vscode.Range(
    normalized.line,
    normalized.column,
    normalized.line,
    normalized.column + 1,
  );
  const diagnostic = new vscode.Diagnostic(range, normalized.message, severityOf(normalized.level));
  diagnostic.source = 'papyrus-lint';
  diagnostic.code = normalized.rule;
  return diagnostic;
}

/** Resolves the `.psc` file a command should act on: the given `uri` (e.g. from an
 * explorer/editor context menu), falling back to the active editor. Saves it first if
 * it's open and has unsaved changes, since the CLI only ever reads from disk. */
async function resolveTargetUri(uri: vscode.Uri | undefined): Promise<vscode.Uri | undefined> {
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

class PapyrusLinter {
  constructor(
    private readonly diagnostics: vscode.DiagnosticCollection,
    private readonly output: vscode.OutputChannel,
  ) {}

  async lintDocument(document: vscode.TextDocument): Promise<void> {
    if (!isPapyrusDocument(document) || document.isDirty) {
      return;
    }
    await this.lint(document.uri);
  }

  async lint(uri: vscode.Uri): Promise<void> {
    const result = await runCli(['--json', uri.fsPath], path.dirname(uri.fsPath));
    this.applyResult(uri, result);
  }

  async fix(uri: vscode.Uri): Promise<void> {
    const result = await runCli(['fix', '--json', uri.fsPath], path.dirname(uri.fsPath));
    const report = this.applyResult(uri, result);
    if (!report) {
      return;
    }

    const remaining = report.files[0]?.diagnostics.length ?? 0;
    const fixedCount = report.files_fixed ?? 0;
    void vscode.window.showInformationMessage(
      fixedCount > 0
        ? `Papyrus Lint: fixed ${path.basename(uri.fsPath)}. ${remaining} issue(s) remain.`
        : `Papyrus Lint: nothing to fix in ${path.basename(uri.fsPath)}. ${remaining} issue(s) remain.`,
    );
  }

  clear(uri: vscode.Uri): void {
    this.diagnostics.delete(uri);
  }

  /** Runs a CLI invocation's result through error handling and, on success, updates
   * `uri`'s diagnostics from the report. Returns the parsed report on success. */
  private applyResult(uri: vscode.Uri, result: CliResult): JsonReport | undefined {
    if (result.code === -1) {
      void vscode.window.showErrorMessage(
        `Papyrus Lint: could not run "${cliPath()}". Set the "papyrusLint.cliPath" setting to its ` +
          `location. (${result.stderr.trim()})`,
      );
      return undefined;
    }

    // Exit code 2 means a usage or I/O error (see PapyrusLinterCLI's USAGE text); 0/1
    // both mean linting ran and produced a report, so only bail out on 2.
    if (result.code === 2) {
      const message = result.stderr.trim() || 'failed to lint file.';
      this.output.appendLine(`papyrus-lint: ${message}`);
      void vscode.window.showErrorMessage(`Papyrus Lint: ${message}`);
      return undefined;
    }

    const report = parseReport(result.stdout);
    if (!report) {
      this.output.appendLine('papyrus-lint: failed to parse CLI output as JSON:');
      this.output.appendLine(result.stdout);
      void vscode.window.showErrorMessage(
        'Papyrus Lint: could not parse the CLI output; see the "Papyrus Lint" output channel.',
      );
      return undefined;
    }

    // A single .psc file is always linted as its own achlist's sole entry, so its
    // report always has exactly one file entry (or none, if resolution failed).
    const fileReport = report.files[0];
    this.diagnostics.set(uri, fileReport ? fileReport.diagnostics.map(toDiagnostic) : []);
    return report;
  }
}

export function activate(context: vscode.ExtensionContext): void {
  const diagnostics = vscode.languages.createDiagnosticCollection('papyrus-lint');
  const output = vscode.window.createOutputChannel('Papyrus Lint');
  const linter = new PapyrusLinter(diagnostics, output);
  context.subscriptions.push(diagnostics, output);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => void linter.lintDocument(document)),
    vscode.workspace.onDidSaveTextDocument((document) => void linter.lintDocument(document)),
    vscode.workspace.onDidCloseTextDocument((document) => {
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
  );
}

export function deactivate(): void {}
