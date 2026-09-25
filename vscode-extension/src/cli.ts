import { execFile } from 'child_process';
import * as vscode from 'vscode';
import { ensureReleaseCli, verifyConfiguredExecutable } from './cliDownload';
import { resolveCliPath } from './config';

export interface CliResult {
  /** The process exit code, or `-1` if the CLI executable itself couldn't be launched. */
  code: number;
  stdout: string;
  stderr: string;
}

let automaticCli: Promise<string> | undefined;
let automaticCliStorage = '';
let extensionVersion = '';
let configuredCliCheck: Promise<CliResult> | undefined;
let checkedConfiguredCli = '';

/** Records this activation's storage/version, drops any in-flight automatic
 * download so a later failure can be retried, and starts fetching this
 * release's CLI when the user hasn't overridden `papyrusLint.cliPath`.
 * Starting the download here — rather than waiting for the first lint —
 * is what picks up a new binary after the extension itself is updated. */
export function configureCli(storageDirectory: string, version: string): void {
  automaticCliStorage = storageDirectory;
  extensionVersion = version;
  automaticCli = undefined;
  configuredCliCheck = undefined;
  checkedConfiguredCli = '';
  if (!resolveCliPath()) {
    automaticCli = ensureReleaseCli(automaticCliStorage, extensionVersion);
    // The first lint/fix awaits this same promise; this extra handler only
    // prevents an unhandled rejection if that hasn't happened yet.
    void automaticCli.catch(() => undefined);
  }
}

export function showCliLaunchFailure(result: CliResult): void {
  void vscode.window.showErrorMessage(
    `Papyrus Lint: could not download or run its CLI. Set "papyrusLint.cliPath" to override it. ` +
      `(${result.stderr.trim()})`,
  );
}

export async function runCli(args: string[], cwd: string): Promise<CliResult> {
  let executable: string;
  try {
    const configured = resolveCliPath();
    if (configured) {
      executable = configured;
      if (checkedConfiguredCli !== configured) {
        checkedConfiguredCli = configured;
        configuredCliCheck = (async () => {
          await verifyConfiguredExecutable(configured);
          // PapyrusLinterCLI 2.x prints the version from the `version` subcommand.
          // `--version` is a usage error (exit 2) and dumps USAGE on stderr.
          return executeCli(configured, ['version'], cwd);
        })();
      }
      try {
        const versionResult = await configuredCliCheck!;
        const expected = `PapyrusLinterCLI ${extensionVersion}`;
        if (versionResult.code !== 0 || versionResult.stdout.trim() !== expected) {
          const actual = versionResult.stdout.trim() || versionResult.stderr.trim() || 'no version output';
          return {
            code: -1,
            stdout: '',
            stderr: `configured CLI version mismatch: expected "${expected}", got "${actual}"`,
          };
        }
      } catch (error) {
        return { code: -1, stdout: '', stderr: error instanceof Error ? error.message : String(error) };
      }
    } else {
      automaticCli ??= ensureReleaseCli(automaticCliStorage, extensionVersion);
      executable = await automaticCli;
    }
  } catch (error) {
    automaticCli = undefined;
    return { code: -1, stdout: '', stderr: error instanceof Error ? error.message : String(error) };
  }
  return executeCli(executable, args, cwd);
}

function executeCli(executable: string, args: string[], cwd: string): Promise<CliResult> {
  return new Promise((resolve) => {
    execFile(executable, args, { cwd, maxBuffer: 10 * 1024 * 1024 }, (error, stdout, stderr) => {
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
