import * as path from 'path';
import * as vscode from 'vscode';
import { runCli, showCliLaunchFailure, type CliResult } from './cli';
import { withConfigOverride } from './config';
import { parseReport, type JsonReport } from './diagnostics';
import { isPapyrusDocument } from './documents';
import { toDiagnostic } from './vscodeDiagnostics';

export class PapyrusLinter {
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
    const result = await runCli(await withConfigOverride(['lint', '--format', 'json', uri.fsPath], uri), path.dirname(uri.fsPath));
    this.applyResult(uri, result);
  }

  /** Live, as-you-type counterpart to `lint`: lints `document`'s current in-memory
   * contents directly via the CLI's `--blob` flag instead of its saved-to-disk
   * contents, so a diagnostic reflects what's actually in the editor even before
   * it's saved. Unlike `lint`, this skips every piece of project-level machinery
   * (cross-script resolution, and the CLI's own project-root discovery) the same
   * way the CLI's `--blob` flag itself does — but still picks up the project's own
   * papyrus-lint.yaml/.yml via `withConfigOverride`'s own workspace-folder-based
   * detection (see `config.ts`), so live linting isn't stuck on the built-in
   * defaults just because there's no on-disk file position for the CLI to walk up
   * from. Never pops an error message box: it runs on every pause in typing, so a
   * transient failure (e.g. a download hiccup) is logged to the output channel
   * instead of interrupting the user. */
  async lintBlob(
    document: vscode.TextDocument,
    shouldApply: () => boolean = () => true,
  ): Promise<void> {
    const result = await runCli(
      await withConfigOverride(['lint', '--format', 'json', '--blob', document.getText()], document.uri),
      path.dirname(document.uri.fsPath),
    );
    if (shouldApply()) {
      this.applyResult(document.uri, result, false);
    }
  }

  async fix(uri: vscode.Uri): Promise<void> {
    const result = await runCli(
      await withConfigOverride(['fix', '--format', 'json', uri.fsPath], uri),
      path.dirname(uri.fsPath),
    );
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

  /** Applies just the named `rule`'s automatic fix, and only on `line` (1-based, matching
   * the CLI's `--line`/the JSON diagnostics' own line numbers), leaving every other issue
   * in the file untouched. Used by the "Fix this issue" quick fix on a single diagnostic. */
  async fixIssue(uri: vscode.Uri, rule: string, line: number): Promise<void> {
    const result = await runCli(
      await withConfigOverride(['fix', '--type', rule, '--line', String(line), '--format', 'json', uri.fsPath], uri),
      path.dirname(uri.fsPath),
    );
    const report = this.applyResult(uri, result);
    if (!report) {
      return;
    }

    const fixedCount = report.files_fixed ?? 0;
    void vscode.window.showInformationMessage(
      fixedCount > 0
        ? `Papyrus Lint: fixed "${rule}" on line ${line} of ${path.basename(uri.fsPath)}.`
        : `Papyrus Lint: "${rule}" on line ${line} of ${path.basename(uri.fsPath)} has no automatic fix.`,
    );
  }

  clear(uri: vscode.Uri): void {
    this.diagnostics.delete(uri);
  }

  /** Runs a CLI invocation's result through error handling and, on success, updates
   * `uri`'s diagnostics from the report. Returns the parsed report on success. Every
   * failure is always logged to the output channel; `notify` (default `true`) also
   * controls whether it's additionally surfaced as an error message box, which
   * `lintBlob` above disables since it runs unattended on every pause in typing. */
  private applyResult(uri: vscode.Uri, result: CliResult, notify = true): JsonReport | undefined {
    if (result.code === -1) {
      this.output.appendLine(`papyrus-lint: ${result.stderr.trim()}`);
      if (notify) {
        showCliLaunchFailure(result);
      }
      return undefined;
    }

    // Exit code 2 means a usage or I/O error (see PapyrusLinterCLI's USAGE text); 0/1
    // both mean linting ran and produced a report, so only bail out on 2.
    if (result.code === 2) {
      const message = result.stderr.trim() || 'failed to lint file.';
      this.output.appendLine(`papyrus-lint: ${message}`);
      if (notify) {
        void vscode.window.showErrorMessage(`Papyrus Lint: ${message}`);
      }
      return undefined;
    }

    const report = parseReport(result.stdout);
    if (!report) {
      this.output.appendLine('papyrus-lint: failed to parse CLI output as JSON:');
      this.output.appendLine(result.stdout);
      if (notify) {
        void vscode.window.showErrorMessage(
          'Papyrus Lint: could not parse the CLI output; see the "Papyrus Lint" output channel.',
        );
      }
      return undefined;
    }

    // A single .psc file is always linted as its own achlist's sole entry, so its
    // report always has exactly one file entry (or none, if resolution failed).
    const fileReport = report.files[0];
    this.diagnostics.set(uri, fileReport ? fileReport.diagnostics.map(toDiagnostic) : []);
    return report;
  }
}
